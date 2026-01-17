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
    Notes(Vec<Note>),
    /// Multiple drum voices that play simultaneously (each Vec<DrumHit> is one voice)
    Drums(Vec<Vec<DrumHit>>),
}

#[derive(Debug, Clone)]
pub struct Staff {
    pub kind: StaffKind,
    pub content: StaffContent,
}

impl Staff {
    pub fn new_pitched(notes: Vec<Note>) -> Self {
        Staff {
            kind: StaffKind::Pitched,
            content: StaffContent::Notes(notes),
        }
    }

    pub fn new_drums(voices: Vec<Vec<DrumHit>>) -> Self {
        Staff {
            kind: StaffKind::Drums,
            content: StaffContent::Drums(voices),
        }
    }

    pub fn notes(&self) -> Option<&Vec<Note>> {
        match &self.content {
            StaffContent::Notes(notes) => Some(notes),
            _ => None,
        }
    }

    pub fn drums(&self) -> Option<&Vec<Vec<DrumHit>>> {
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
    /// For backwards compatibility: returns all notes flattened
    pub fn notes(&self) -> Vec<Note> {
        self.staves
            .iter()
            .filter_map(|s| s.notes())
            .flat_map(|n| n.clone())
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
        let expanded = self.expand_repeats(code);
        let variables_expanded: HashMap<String, VariableKind> = variables
            .into_iter()
            .map(|(k, v)| {
                let expanded_content = self.expand_repeats(match &v {
                    VariableKind::Pitched(s) => s,
                    VariableKind::Drums(s) => s,
                });
                let new_v = match v {
                    VariableKind::Pitched(_) => VariableKind::Pitched(expanded_content),
                    VariableKind::Drums(_) => VariableKind::Drums(expanded_content),
                };
                (k, new_v)
            })
            .collect();

        // Try to parse score with staves first
        if let Some(staves) = self.parse_score_staves(&expanded, &variables_expanded)? {
            return Ok(ParseResult { staves, tempo });
        }

        // Fallback: parse as single staff
        let notes_section = self.extract_notes_section(&expanded)?;
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

    fn parse_notes_from_section(&self, section: &str) -> Result<Vec<Note>, String> {
        let mut notes = Vec::new();
        let tokens = self.tokenize(section);

        for token in tokens {
            if let Some(note) = self.parse_note(&token)? {
                notes.push(note);
            }
        }

        Ok(notes)
    }

    fn parse_drums_from_section(&self, section: &str) -> Result<Vec<DrumHit>, String> {
        let mut hits = Vec::new();
        let tokens = self.tokenize(section);

        for token in tokens {
            if let Some(hit) = self.parse_drum_hit(&token) {
                hits.push(hit);
            }
        }

        Ok(hits)
    }

    fn parse_drum_voices(
        &self,
        staff_content: &str,
        variables: &HashMap<String, VariableKind>,
    ) -> Result<Vec<Vec<DrumHit>>, String> {
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
                        let hits = self.parse_drums_from_section(&resolved)?;
                        if !hits.is_empty() {
                            voices.push(hits);
                        }
                    }
                }

                // If no DrumVoice blocks, look for direct variable references
                if voices.is_empty() {
                    let var_ref_re = regex::Regex::new(r"\\([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
                    for caps in var_ref_re.captures_iter(simultaneous) {
                        let var_name = caps.get(1).unwrap().as_str();
                        if let Some(VariableKind::Drums(content)) = variables.get(var_name) {
                            let hits = self.parse_drums_from_section(content)?;
                            if !hits.is_empty() {
                                voices.push(hits);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: parse the whole content as a single voice
        if voices.is_empty() {
            let resolved = self.resolve_variables(staff_content, variables);
            let hits = self.parse_drums_from_section(&resolved)?;
            if !hits.is_empty() {
                voices.push(hits);
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

    fn expand_repeats(&self, code: &str) -> String {
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
            let expanded = content.repeat(count);

            result = format!("{}{}{}", &result[..start], expanded, &result[end + 1..]);
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

    /// Compress a sequence by finding repeating patterns and using *N syntax
    fn compress_sequence(items: &[String]) -> String {
        if items.is_empty() {
            return String::new();
        }

        // Try to find the smallest repeating unit
        let len = items.len();
        for unit_size in 1..=len / 2 {
            if len % unit_size == 0 {
                let unit = &items[0..unit_size];
                let repeat_count = len / unit_size;

                // Check if the entire sequence is this unit repeated
                let mut is_repeating = true;
                for i in 1..repeat_count {
                    let start = i * unit_size;
                    if &items[start..start + unit_size] != unit {
                        is_repeating = false;
                        break;
                    }
                }

                if is_repeating && repeat_count > 1 {
                    let pattern = unit.join(" ");
                    if unit_size == 1 {
                        return format!("{}*{}", pattern, repeat_count);
                    } else {
                        return format!("[{}]*{}", pattern, repeat_count);
                    }
                }
            }
        }

        // No repeating pattern found, return as-is
        items.join(" ")
    }

    fn format_note(n: &Note) -> String {
        let acc = match &n.accidental {
            Some(a) if a == "is" => "#",
            Some(a) if a == "es" => "b",
            _ => "",
        };
        format!("{}{}{}", n.name, acc, n.octave)
    }

    pub fn generate_pitched_staff(notes: &[Note], tempo: Option<&Tempo>) -> String {
        if notes.is_empty() {
            return String::from("// No notes to convert");
        }

        let note_sequence: Vec<String> = notes
            .iter()
            .map(|n| {
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
            })
            .collect();

        let base = format!(
            "note(\"{}\")\n  .s(\"piano\")",
            Self::compress_sequence(&note_sequence)
        );

        let total_beats: f64 = notes.iter().map(|n| 4.0 / n.duration as f64).sum();
        if let Some(cpm) = Self::calculate_cpm(total_beats, tempo) {
            format!("{base}\n  .cpm({cpm})")
        } else {
            base
        }
    }

    fn generate_single_drum_voice(hits: &[DrumHit], tempo: Option<&Tempo>) -> String {
        let hit_sequence: Vec<String> = hits
            .iter()
            .map(|h| {
                let weight = 4.0 / h.duration as f32;
                if weight == 1.0 {
                    h.name.clone()
                } else {
                    format!("{}@{}", h.name, weight)
                }
            })
            .collect();

        let base = format!("sound(\"{}\")", Self::compress_sequence(&hit_sequence));

        let total_beats: f64 = hits.iter().map(|h| 4.0 / h.duration as f64).sum();
        if let Some(cpm) = Self::calculate_cpm(total_beats, tempo) {
            format!("{base}\n  .cpm({cpm})")
        } else {
            base
        }
    }

    pub fn generate_drum_staff(voices: &[Vec<DrumHit>], tempo: Option<&Tempo>) -> String {
        if voices.is_empty() {
            return String::from("// No drum hits to convert");
        }

        if voices.len() == 1 {
            return Self::generate_single_drum_voice(&voices[0], tempo);
        }

        // Multiple voices: use stack()
        let voice_patterns: Vec<String> = voices
            .iter()
            .map(|hits| {
                let hit_sequence: Vec<String> = hits
                    .iter()
                    .map(|h| {
                        let weight = 4.0 / h.duration as f32;
                        if weight == 1.0 {
                            h.name.clone()
                        } else {
                            format!("{}@{}", h.name, weight)
                        }
                    })
                    .collect();
                format!("sound(\"{}\")", Self::compress_sequence(&hit_sequence))
            })
            .collect();

        let stacked = format!("stack(\n  {},\n)", voice_patterns.join(",\n  "));

        // Use the longest voice to calculate cpm
        let max_beats: f64 = voices
            .iter()
            .map(|hits| hits.iter().map(|h| 4.0 / h.duration as f64).sum())
            .fold(0.0, f64::max);

        if let Some(cpm) = Self::calculate_cpm(max_beats, tempo) {
            format!("{stacked}\n  .cpm({cpm})")
        } else {
            stacked
        }
    }

    pub fn generate_staff(staff: &Staff, tempo: Option<&Tempo>) -> String {
        match &staff.content {
            StaffContent::Notes(notes) => Self::generate_pitched_staff(notes, tempo),
            StaffContent::Drums(hits) => Self::generate_drum_staff(hits, tempo),
        }
    }

    /// Generate Strudel code for a single staff (backwards compatibility)
    pub fn generate(notes: &[Note], tempo: Option<&Tempo>) -> String {
        Self::generate_pitched_staff(notes, tempo)
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
