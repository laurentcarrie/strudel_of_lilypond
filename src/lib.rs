use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Note {
    pub name: char,
    pub octave: i32,
    pub accidental: Option<String>,
    pub duration: u32,
    #[allow(dead_code)]
    pub midi: i32,
    /// Additional notes if this is a chord (first note is self)
    pub chord_notes: Option<Vec<Note>>,
}

#[derive(Debug, Clone)]
pub struct DrumHit {
    pub name: String,
    pub duration: u32,
}

#[derive(Debug, Clone)]
pub enum PitchedEvent {
    Note(Note),
    BarLine,
    RepeatStart(u32),
    RepeatEnd,
}

#[derive(Debug, Clone)]
pub enum DrumEvent {
    Hit(DrumHit),
    BarLine,
    RepeatStart(u32),
    RepeatEnd,
}

#[derive(Debug, Clone)]
pub struct Tempo {
    pub beat_unit: u32,
    pub bpm: u32,
}

#[derive(Debug, Clone)]
pub enum StaffKind {
    Pitched,
    Drums,
}

#[derive(Debug, Clone)]
pub enum StaffContent {
    Notes(Vec<PitchedEvent>),
    /// Multiple drum voices that play simultaneously (each Vec<DrumEvent> is one voice)
    Drums(Vec<Vec<DrumEvent>>),
}

#[derive(Debug, Clone)]
pub struct Staff {
    pub kind: StaffKind,
    pub content: StaffContent,
}

impl Staff {
    pub fn new_pitched(events: Vec<PitchedEvent>) -> Self {
        Staff {
            kind: StaffKind::Pitched,
            content: StaffContent::Notes(events),
        }
    }

    pub fn new_drums(voices: Vec<Vec<DrumEvent>>) -> Self {
        Staff {
            kind: StaffKind::Drums,
            content: StaffContent::Drums(voices),
        }
    }

    pub fn events(&self) -> Option<&Vec<PitchedEvent>> {
        match &self.content {
            StaffContent::Notes(events) => Some(events),
            _ => None,
        }
    }

    pub fn drum_events(&self) -> Option<&Vec<Vec<DrumEvent>>> {
        match &self.content {
            StaffContent::Drums(voices) => Some(voices),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub staves: Vec<Staff>,
    pub tempo: Option<Tempo>,
}

impl ParseResult {
    /// For backwards compatibility: returns all notes flattened (excluding bar lines and repeats)
    pub fn notes(&self) -> Vec<Note> {
        self.staves
            .iter()
            .filter_map(|s| s.events())
            .flat_map(|events| {
                events.iter().filter_map(|e| match e {
                    PitchedEvent::Note(n) => Some(n.clone()),
                    _ => None,
                })
            })
            .collect()
    }
}

#[derive(Clone)]
enum VariableKind {
    Pitched(String),
    Drums(String),
}

pub struct LilyPondParser {
    note_to_midi: HashMap<char, i32>,
}

impl LilyPondParser {
    pub fn new() -> Self {
        let mut note_to_midi = HashMap::new();
        note_to_midi.insert('c', 0);
        note_to_midi.insert('d', 2);
        note_to_midi.insert('e', 4);
        note_to_midi.insert('f', 5);
        note_to_midi.insert('g', 7);
        note_to_midi.insert('a', 9);
        note_to_midi.insert('b', 11);

        LilyPondParser { note_to_midi }
    }

    pub fn parse(&self, code: &str) -> Result<ParseResult, String> {
        let tempo = self.parse_tempo(code);
        let variables = self.parse_variables(code);
        let marked = self.mark_repeats(code);
        let variables_marked: HashMap<String, VariableKind> = variables
            .into_iter()
            .map(|(k, v)| {
                let marked_content = self.mark_repeats(match &v {
                    VariableKind::Pitched(s) => s,
                    VariableKind::Drums(s) => s,
                });
                let new_v = match v {
                    VariableKind::Pitched(_) => VariableKind::Pitched(marked_content),
                    VariableKind::Drums(_) => VariableKind::Drums(marked_content),
                };
                (k, new_v)
            })
            .collect();

        // Try to parse score with staves first
        if let Some(staves) = self.parse_score_staves(&marked, &variables_marked)? {
            return Ok(ParseResult { staves, tempo });
        }

        // Fallback: parse as single staff
        let notes_section = self.extract_notes_section(&marked)?;
        let notes = self.parse_notes_from_section(&notes_section)?;

        Ok(ParseResult {
            staves: vec![Staff::new_pitched(notes)],
            tempo,
        })
    }

    fn parse_variables(&self, code: &str) -> HashMap<String, VariableKind> {
        let mut variables = HashMap::new();

        // Parse regular variables: name = { ... }
        let re = regex::Regex::new(r"(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*\{").unwrap();
        for caps in re.captures_iter(code) {
            let name = caps.get(1).unwrap().as_str().to_string();
            let brace_start = caps.get(0).unwrap().end() - 1;

            if let Some(content) = self.extract_braced_content(code, brace_start) {
                variables.insert(name, VariableKind::Pitched(content));
            }
        }

        // Parse drummode variables: name = \drummode { ... }
        let drum_re =
            regex::Regex::new(r"(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*\\drummode\s*\{").unwrap();
        for caps in drum_re.captures_iter(code) {
            let name = caps.get(1).unwrap().as_str().to_string();
            let brace_start = caps.get(0).unwrap().end() - 1;

            if let Some(content) = self.extract_braced_content(code, brace_start) {
                variables.insert(name, VariableKind::Drums(content));
            }
        }

        variables
    }

    fn extract_braced_content(&self, code: &str, brace_start: usize) -> Option<String> {
        let mut depth = 1;

        for (i, c) in code[brace_start + 1..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = brace_start + 1 + i;
                        return Some(code[brace_start + 1..end].to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn parse_score_staves(
        &self,
        code: &str,
        variables: &HashMap<String, VariableKind>,
    ) -> Result<Option<Vec<Staff>>, String> {
        // Find \score { << ... >> } blocks
        let score_re = regex::Regex::new(r"\\score\s*\{").unwrap();

        let Some(score_match) = score_re.find(code) else {
            return Ok(None);
        };

        let brace_start = score_match.end() - 1;
        let Some(score_content) = self.extract_braced_content(code, brace_start) else {
            return Ok(None);
        };

        // Find << >> block within score
        let Some(sim_start) = score_content.find("<<") else {
            return Ok(None);
        };
        let Some(sim_end) = score_content.rfind(">>") else {
            return Ok(None);
        };

        if sim_start >= sim_end {
            return Ok(None);
        }

        let simultaneous_content = &score_content[sim_start + 2..sim_end];

        let mut staves = Vec::new();

        // Find all \new Staff or \new TabStaff blocks (pitched)
        let staff_re = regex::Regex::new(r"\\new\s+(Staff|TabStaff)\s*\{").unwrap();
        for caps in staff_re.captures_iter(simultaneous_content) {
            let full_match = caps.get(0).unwrap();
            let brace_pos = simultaneous_content[..full_match.end()]
                .rfind('{')
                .unwrap();

            if let Some(staff_content) =
                self.extract_braced_content(simultaneous_content, brace_pos)
            {
                let resolved = self.resolve_variables(&staff_content, variables);
                // Check if resolved content is from a drum variable
                if self.is_drum_content(&staff_content, variables) {
                    let hits = self.parse_drums_from_section(&resolved)?;
                    if !hits.is_empty() {
                        staves.push(Staff::new_drums(vec![hits]));
                    }
                } else {
                    let notes = self.parse_notes_from_section(&resolved)?;
                    if !notes.is_empty() {
                        staves.push(Staff::new_pitched(notes));
                    }
                }
            }
        }

        // Find all \new DrumStaff blocks
        let drum_staff_re = regex::Regex::new(r"\\new\s+DrumStaff\s*\{").unwrap();
        for caps in drum_staff_re.captures_iter(simultaneous_content) {
            let full_match = caps.get(0).unwrap();
            let brace_pos = simultaneous_content[..full_match.end()]
                .rfind('{')
                .unwrap();

            if let Some(staff_content) =
                self.extract_braced_content(simultaneous_content, brace_pos)
            {
                let voices = self.parse_drum_voices(&staff_content, variables)?;
                if !voices.is_empty() {
                    staves.push(Staff::new_drums(voices));
                }
            }
        }

        // If no \new Staff blocks found, look for direct variable references
        if staves.is_empty() {
            let var_ref_re = regex::Regex::new(r"\\([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
            for caps in var_ref_re.captures_iter(simultaneous_content) {
                let var_name = caps.get(1).unwrap().as_str();
                if let Some(var_kind) = variables.get(var_name) {
                    match var_kind {
                        VariableKind::Pitched(content) => {
                            let notes = self.parse_notes_from_section(content)?;
                            if !notes.is_empty() {
                                staves.push(Staff::new_pitched(notes));
                            }
                        }
                        VariableKind::Drums(content) => {
                            let hits = self.parse_drums_from_section(content)?;
                            if !hits.is_empty() {
                                staves.push(Staff::new_drums(vec![hits]));
                            }
                        }
                    }
                }
            }
        }

        if staves.is_empty() {
            Ok(None)
        } else {
            Ok(Some(staves))
        }
    }

    fn is_drum_content(&self, content: &str, variables: &HashMap<String, VariableKind>) -> bool {
        let var_ref_re = regex::Regex::new(r"\\([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for caps in var_ref_re.captures_iter(content) {
            let var_name = caps.get(1).unwrap().as_str();
            if let Some(VariableKind::Drums(_)) = variables.get(var_name) {
                return true;
            }
        }
        false
    }

    fn resolve_variables(&self, content: &str, variables: &HashMap<String, VariableKind>) -> String {
        let mut result = content.to_string();
        let var_ref_re = regex::Regex::new(r"\\([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();

        // Keep resolving until no more changes (handles nested references)
        loop {
            let mut changed = false;
            let new_result = var_ref_re
                .replace_all(&result, |caps: &regex::Captures| {
                    let var_name = caps.get(1).unwrap().as_str();
                    if let Some(var_kind) = variables.get(var_name) {
                        changed = true;
                        match var_kind {
                            VariableKind::Pitched(s) => s.clone(),
                            VariableKind::Drums(s) => s.clone(),
                        }
                    } else {
                        caps.get(0).unwrap().as_str().to_string()
                    }
                })
                .to_string();

            result = new_result;
            if !changed {
                break;
            }
        }

        result
    }

    fn parse_notes_from_section(&self, section: &str) -> Result<Vec<PitchedEvent>, String> {
        let mut events = Vec::new();
        let tokens = self.tokenize(section);
        let repeat_start_re = regex::Regex::new(r"^__REPEAT_START_(\d+)__$").unwrap();

        for token in tokens {
            if token.starts_with('|') {
                events.push(PitchedEvent::BarLine);
            } else if let Some(caps) = repeat_start_re.captures(&token) {
                let count: u32 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
                events.push(PitchedEvent::RepeatStart(count));
            } else if token == "__REPEAT_END__" {
                events.push(PitchedEvent::RepeatEnd);
            } else if let Some(note) = self.parse_note(&token)? {
                events.push(PitchedEvent::Note(note));
            }
        }

        Ok(events)
    }

    fn parse_drums_from_section(&self, section: &str) -> Result<Vec<DrumEvent>, String> {
        let mut events = Vec::new();
        let tokens = self.tokenize(section);
        let repeat_start_re = regex::Regex::new(r"^__REPEAT_START_(\d+)__$").unwrap();

        for token in tokens {
            if token.starts_with('|') {
                events.push(DrumEvent::BarLine);
            } else if let Some(caps) = repeat_start_re.captures(&token) {
                let count: u32 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
                events.push(DrumEvent::RepeatStart(count));
            } else if token == "__REPEAT_END__" {
                events.push(DrumEvent::RepeatEnd);
            } else if let Some(hit) = self.parse_drum_hit(&token) {
                events.push(DrumEvent::Hit(hit));
            }
        }

        Ok(events)
    }

    fn parse_drum_voices(
        &self,
        staff_content: &str,
        variables: &HashMap<String, VariableKind>,
    ) -> Result<Vec<Vec<DrumEvent>>, String> {
        let mut voices = Vec::new();

        // Check if there's a << >> block inside the DrumStaff
        if let (Some(sim_start), Some(sim_end)) = (staff_content.find("<<"), staff_content.rfind(">>")) {
            if sim_start < sim_end {
                let simultaneous = &staff_content[sim_start + 2..sim_end];

                // Find all \new DrumVoice blocks
                let voice_re = regex::Regex::new(r"\\new\s+DrumVoice\s*\{").unwrap();
                for caps in voice_re.captures_iter(simultaneous) {
                    let full_match = caps.get(0).unwrap();
                    let brace_pos = simultaneous[..full_match.end()].rfind('{').unwrap();

                    if let Some(voice_content) = self.extract_braced_content(simultaneous, brace_pos) {
                        let resolved = self.resolve_variables(&voice_content, variables);
                        let events = self.parse_drums_from_section(&resolved)?;
                        if !events.is_empty() {
                            voices.push(events);
                        }
                    }
                }

                // If no DrumVoice blocks, look for direct variable references
                if voices.is_empty() {
                    let var_ref_re = regex::Regex::new(r"\\([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
                    for caps in var_ref_re.captures_iter(simultaneous) {
                        let var_name = caps.get(1).unwrap().as_str();
                        if let Some(VariableKind::Drums(content)) = variables.get(var_name) {
                            let events = self.parse_drums_from_section(content)?;
                            if !events.is_empty() {
                                voices.push(events);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: parse the whole content as a single voice
        if voices.is_empty() {
            let resolved = self.resolve_variables(staff_content, variables);
            let events = self.parse_drums_from_section(&resolved)?;
            if !events.is_empty() {
                voices.push(events);
            }
        }

        Ok(voices)
    }

    fn parse_drum_hit(&self, token: &str) -> Option<DrumHit> {
        let token = token.trim();

        // Skip bar lines and commands
        if token.starts_with('|') || token.starts_with('\\') {
            return None;
        }

        // Common LilyPond drum names
        let drum_names = [
            "bd", "sn", "hh", "hhc", "hho", "hhp", "hh", "cymc", "cymr", "cymca", "cymcb",
            "tom", "tomh", "tomm", "toml", "tomfl", "tomfh",
            "cb", "cl", "cp", "cr", "gui", "hc", "lc",
            "mc", "rc", "ride", "rb", "ss", "tamb", "tri", "whl", "whs",
            "pedalhihat", "hihat", "openhat", "closehat",
        ];

        let mut chars = token.chars().peekable();
        let mut name = String::new();

        // Read drum name (alphabetic characters)
        while let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }

        // Check if it's a valid drum name
        if !drum_names.contains(&name.as_str()) {
            return None;
        }

        // Map LilyPond drum names to Strudel drum names
        let strudel_name = Self::lilypond_to_strudel_drum(&name);

        // Parse duration
        let mut duration_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_numeric() {
                duration_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        let duration = if duration_str.is_empty() {
            4
        } else {
            duration_str.parse::<u32>().unwrap_or(4)
        };

        Some(DrumHit { name: strudel_name, duration })
    }

    /// Map LilyPond drum names to Strudel drum names
    fn lilypond_to_strudel_drum(name: &str) -> String {
        match name {
            "sn" => "sd".to_string(),      // snare drum
            "hhc" => "hh".to_string(),     // closed hi-hat
            "hho" => "oh".to_string(),     // open hi-hat
            "cymc" => "cr".to_string(),    // crash cymbal
            "cymr" => "rd".to_string(),    // ride cymbal
            "tomh" => "ht".to_string(),    // high tom
            "tomm" => "mt".to_string(),    // mid tom
            "toml" => "lt".to_string(),    // low tom
            _ => name.to_string(),         // keep as-is (bd, hh, etc.)
        }
    }

    fn mark_repeats(&self, code: &str) -> String {
        let mut result = code.to_string();
        let re = regex::Regex::new(r"\\repeat\s+\w+\s+(\d+)\s*\{").unwrap();

        loop {
            let Some(caps) = re.captures(&result) else {
                break;
            };

            let full_match = caps.get(0).unwrap();
            let count: usize = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
            let start = full_match.start();
            let brace_start = full_match.end() - 1;

            let mut depth = 1;
            let mut end = brace_start + 1;
            for (i, c) in result[brace_start + 1..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = brace_start + 1 + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            let content = &result[brace_start + 1..end];
            // Add markers instead of expanding
            let marked = format!(" __REPEAT_START_{}__ {} __REPEAT_END__ ", count, content);

            result = format!("{}{}{}", &result[..start], marked, &result[end + 1..]);
        }

        result
    }

    fn parse_tempo(&self, code: &str) -> Option<Tempo> {
        let re = regex::Regex::new(r"\\tempo\s+(\d+)\s*=\s*(\d+)").ok()?;

        if let Some(caps) = re.captures(code) {
            let beat_unit: u32 = caps.get(1)?.as_str().parse().ok()?;
            let bpm: u32 = caps.get(2)?.as_str().parse().ok()?;
            return Some(Tempo { beat_unit, bpm });
        }
        None
    }

    fn extract_notes_section(&self, code: &str) -> Result<String, String> {
        let start = code.find('{').ok_or("No '{' found")?;
        let end = code.rfind('}').ok_or("No '}' found")?;

        if start >= end {
            return Err("Invalid syntax".to_string());
        }

        Ok(code[start + 1..end].to_string())
    }

    fn tokenize(&self, section: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_chord = false;

        for c in section.chars() {
            if c == '<' {
                // Start of chord - save any pending token
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                }
                current = String::new();
                current.push(c);
                in_chord = true;
            } else if c == '>' {
                // End of chord bracket
                current.push(c);
                in_chord = false;
                // Continue collecting duration after >
            } else if c.is_whitespace() {
                if in_chord {
                    // Inside chord, keep spaces
                    current.push(' ');
                } else if !current.is_empty() {
                    // End of token
                    tokens.push(current.trim().to_string());
                    current = String::new();
                }
            } else {
                current.push(c);
            }
        }

        if !current.trim().is_empty() {
            tokens.push(current.trim().to_string());
        }

        tokens.into_iter().filter(|s| !s.is_empty()).collect()
    }

    fn parse_note(&self, token: &str) -> Result<Option<Note>, String> {
        let token = token.trim();

        if token.starts_with('|') || token.starts_with('\\') || token.starts_with('r') {
            return Ok(None);
        }

        // Check for chord syntax <note note note>duration
        if token.starts_with('<') {
            return self.parse_chord(token);
        }

        self.parse_single_note(token, None)
    }

    fn parse_chord(&self, token: &str) -> Result<Option<Note>, String> {
        // Parse <a c e>4 style chord
        let Some(close_bracket) = token.find('>') else {
            return Ok(None);
        };

        let chord_content = &token[1..close_bracket];
        let after_bracket = &token[close_bracket + 1..];

        // Parse duration after the >
        let mut duration_str = String::new();
        for c in after_bracket.chars() {
            if c.is_numeric() {
                duration_str.push(c);
            } else if c != '.' && c != '~' {
                break;
            }
        }

        let duration = if duration_str.is_empty() {
            4
        } else {
            duration_str.parse::<u32>().unwrap_or(4)
        };

        // Parse individual notes in the chord
        let note_tokens: Vec<&str> = chord_content.split_whitespace().collect();
        if note_tokens.is_empty() {
            return Ok(None);
        }

        let mut chord_notes = Vec::new();
        for note_token in &note_tokens {
            if let Some(note) = self.parse_single_note(note_token, Some(duration))? {
                chord_notes.push(note);
            }
        }

        if chord_notes.is_empty() {
            return Ok(None);
        }

        // First note becomes the main note, rest go in chord_notes
        let mut first_note = chord_notes.remove(0);
        first_note.chord_notes = if chord_notes.is_empty() {
            None
        } else {
            Some(chord_notes)
        };

        Ok(Some(first_note))
    }

    fn parse_single_note(&self, token: &str, override_duration: Option<u32>) -> Result<Option<Note>, String> {
        let token = token.trim();

        let mut chars = token.chars().peekable();

        let note_name = match chars.next() {
            Some(c) if c.is_alphabetic() && "abcdefg".contains(c) => c,
            _ => return Ok(None),
        };

        let mut accidental = None;
        if chars.peek() == Some(&'i') || chars.peek() == Some(&'e') {
            let first = chars.next().unwrap();
            if chars.peek() == Some(&'s') {
                chars.next();
                accidental = Some(format!("{first}s"));
            }
        }

        let mut octave = 4;
        while let Some(&c) = chars.peek() {
            match c {
                '\'' => {
                    octave += 1;
                    chars.next();
                }
                ',' => {
                    octave -= 1;
                    chars.next();
                }
                _ => break,
            }
        }

        let mut duration_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_numeric() {
                duration_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        while let Some(&c) = chars.peek() {
            if c == '.' || c == '~' {
                chars.next();
            } else {
                break;
            }
        }

        if chars.any(|c| c.is_alphabetic()) {
            return Ok(None);
        }

        let duration = override_duration.unwrap_or_else(|| {
            if duration_str.is_empty() {
                4
            } else {
                duration_str.parse::<u32>().unwrap_or(4)
            }
        });

        let mut midi = *self.note_to_midi.get(&note_name).unwrap();

        if let Some(ref acc) = accidental {
            match acc.as_str() {
                "is" => midi += 1,
                "es" => midi -= 1,
                _ => {}
            }
        }

        midi += octave * 12;

        Ok(Some(Note {
            name: note_name,
            octave,
            accidental,
            duration,
            midi,
            chord_notes: None,
        }))
    }
}

impl Default for LilyPondParser {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StrudelGenerator;

impl StrudelGenerator {
    fn calculate_cpm(total_beats: f64, tempo: Option<&Tempo>) -> Option<f64> {
        tempo.map(|t| t.bpm as f64 / total_beats)
    }

    fn format_note(n: &Note) -> String {
        let acc = match &n.accidental {
            Some(a) if a == "is" => "#",
            Some(a) if a == "es" => "b",
            _ => "",
        };
        format!("{}{}{}", n.name, acc, n.octave)
    }

    fn format_pitched_note(n: &Note) -> String {
        let weight = 4.0 / n.duration as f32;

        // Check if this is a chord
        let note_str = if let Some(ref chord_notes) = n.chord_notes {
            // Format as [note1,note2,note3]
            let mut all_notes = vec![Self::format_note(n)];
            for cn in chord_notes {
                all_notes.push(Self::format_note(cn));
            }
            format!("[{}]", all_notes.join(","))
        } else {
            Self::format_note(n)
        };

        if weight == 1.0 {
            note_str
        } else {
            format!("{}@{}", note_str, weight)
        }
    }

    fn generate_pitched_pattern(events: &[PitchedEvent], idx: &mut usize) -> String {
        let mut parts: Vec<String> = Vec::new();

        while *idx < events.len() {
            match &events[*idx] {
                PitchedEvent::Note(n) => {
                    parts.push(Self::format_pitched_note(n));
                    *idx += 1;
                }
                PitchedEvent::BarLine => {
                    *idx += 1; // Skip bar lines
                }
                PitchedEvent::RepeatStart(count) => {
                    *idx += 1;
                    let inner = Self::generate_pitched_pattern(events, idx);
                    parts.push(format!("[{}]*{}", inner, count));
                }
                PitchedEvent::RepeatEnd => {
                    *idx += 1;
                    break; // Exit this level of recursion
                }
            }
        }

        parts.join(" ")
    }

    fn calculate_pitched_beats(events: &[PitchedEvent], idx: &mut usize) -> f64 {
        let mut total = 0.0;
        while *idx < events.len() {
            match &events[*idx] {
                PitchedEvent::Note(n) => {
                    total += 4.0 / n.duration as f64;
                    *idx += 1;
                }
                PitchedEvent::BarLine => {
                    *idx += 1;
                }
                PitchedEvent::RepeatStart(count) => {
                    *idx += 1;
                    let inner_beats = Self::calculate_pitched_beats(events, idx);
                    total += inner_beats * (*count as f64);
                }
                PitchedEvent::RepeatEnd => {
                    *idx += 1;
                    break;
                }
            }
        }
        total
    }

    pub fn generate_pitched_staff(events: &[PitchedEvent], tempo: Option<&Tempo>) -> String {
        let notes: Vec<&Note> = events
            .iter()
            .filter_map(|e| match e {
                PitchedEvent::Note(n) => Some(n),
                _ => None,
            })
            .collect();

        if notes.is_empty() {
            return String::from("// No notes to convert");
        }

        let mut idx = 0;
        let pattern = Self::generate_pitched_pattern(events, &mut idx);

        let base = format!(
            "note(\"{}\")\n  .s(\"piano\")",
            pattern
        );

        let mut beat_idx = 0;
        let total_beats = Self::calculate_pitched_beats(events, &mut beat_idx);
        if let Some(cpm) = Self::calculate_cpm(total_beats, tempo) {
            format!("{base}\n  .cpm({cpm})")
        } else {
            base
        }
    }

    fn format_drum_hit(h: &DrumHit) -> String {
        let weight = 4.0 / h.duration as f32;
        if weight == 1.0 {
            h.name.clone()
        } else {
            format!("{}@{}", h.name, weight)
        }
    }

    fn calculate_drum_beats(events: &[DrumEvent], idx: &mut usize) -> f64 {
        let mut total = 0.0;
        while *idx < events.len() {
            match &events[*idx] {
                DrumEvent::Hit(h) => {
                    total += 4.0 / h.duration as f64;
                    *idx += 1;
                }
                DrumEvent::BarLine => {
                    *idx += 1;
                }
                DrumEvent::RepeatStart(count) => {
                    *idx += 1;
                    let inner_beats = Self::calculate_drum_beats(events, idx);
                    total += inner_beats * (*count as f64);
                }
                DrumEvent::RepeatEnd => {
                    *idx += 1;
                    break;
                }
            }
        }
        total
    }

    fn generate_drum_pattern(events: &[DrumEvent], idx: &mut usize) -> String {
        let mut parts: Vec<String> = Vec::new();

        while *idx < events.len() {
            match &events[*idx] {
                DrumEvent::Hit(h) => {
                    parts.push(Self::format_drum_hit(h));
                    *idx += 1;
                }
                DrumEvent::BarLine => {
                    *idx += 1; // Skip bar lines
                }
                DrumEvent::RepeatStart(count) => {
                    *idx += 1;
                    let inner = Self::generate_drum_pattern(events, idx);
                    parts.push(format!("[{}]*{}", inner, count));
                }
                DrumEvent::RepeatEnd => {
                    *idx += 1;
                    break; // Exit this level of recursion
                }
            }
        }

        parts.join(" ")
    }

    fn generate_single_drum_voice(events: &[DrumEvent], tempo: Option<&Tempo>) -> String {
        let hits: Vec<&DrumHit> = events
            .iter()
            .filter_map(|e| match e {
                DrumEvent::Hit(h) => Some(h),
                _ => None,
            })
            .collect();

        if hits.is_empty() {
            return String::from("// No drum hits to convert");
        }

        let mut idx = 0;
        let pattern = Self::generate_drum_pattern(events, &mut idx);
        let base = format!("sound(\"{}\")", pattern);

        let mut beat_idx = 0;
        let total_beats = Self::calculate_drum_beats(events, &mut beat_idx);
        if let Some(cpm) = Self::calculate_cpm(total_beats, tempo) {
            format!("{base}\n  .cpm({cpm})")
        } else {
            base
        }
    }

    pub fn generate_drum_staff(voices: &[Vec<DrumEvent>], tempo: Option<&Tempo>) -> String {
        if voices.is_empty() {
            return String::from("// No drum hits to convert");
        }

        if voices.len() == 1 {
            return Self::generate_single_drum_voice(&voices[0], tempo);
        }

        // Multiple voices: use stack()
        let voice_patterns: Vec<String> = voices
            .iter()
            .map(|events| {
                let mut idx = 0;
                let pattern = Self::generate_drum_pattern(events, &mut idx);
                format!("sound(\"{}\")", pattern)
            })
            .collect();

        let stacked = format!("stack(\n  {},\n)", voice_patterns.join(",\n  "));

        // Use the longest voice to calculate cpm
        let max_beats: f64 = voices
            .iter()
            .map(|events| {
                let mut idx = 0;
                Self::calculate_drum_beats(events, &mut idx)
            })
            .fold(0.0, f64::max);

        if let Some(cpm) = Self::calculate_cpm(max_beats, tempo) {
            format!("{stacked}\n  .cpm({cpm})")
        } else {
            stacked
        }
    }

    pub fn generate_staff(staff: &Staff, tempo: Option<&Tempo>) -> String {
        match &staff.content {
            StaffContent::Notes(events) => Self::generate_pitched_staff(events, tempo),
            StaffContent::Drums(voices) => Self::generate_drum_staff(voices, tempo),
        }
    }

    /// Generate Strudel code for a single staff (backwards compatibility)
    pub fn generate(notes: &[Note], tempo: Option<&Tempo>) -> String {
        let events: Vec<PitchedEvent> = notes.iter().cloned().map(PitchedEvent::Note).collect();
        Self::generate_pitched_staff(&events, tempo)
    }

    /// Generate Strudel code for multiple staves
    pub fn generate_multi(staves: &[Staff], tempo: Option<&Tempo>) -> String {
        if staves.is_empty() {
            return String::from("// No staves to convert");
        }

        staves
            .iter()
            .map(|staff| format!("$: {}", Self::generate_staff(staff, tempo)))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn generate_html(staves: &[Staff], tempo: Option<&Tempo>, title: &str) -> String {
        let pattern = Self::generate_multi(staves, tempo);
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>{title}</title>
  <script src="https://unpkg.com/@strudel/embed@latest"></script>
  <style>
    html, body {{ margin: 0; padding: 0; width: 100%; height: 100%; }}
    strudel-repl {{ width: 100%; height: 100%; display: block; }}
    strudel-repl iframe {{ width: 100%; height: 100%; border: none; }}
  </style>
</head>
<body>
  <strudel-repl>
<!--
{pattern}
-->
  </strudel-repl>
</body>
</html>"#
        )
    }
}

#[cfg(test)]
mod tests;
