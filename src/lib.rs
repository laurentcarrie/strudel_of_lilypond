pub mod sequencer;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
    Rest { duration: u32 },
    BarLine,
    RepeatStart(u32),
    RepeatEnd,
    Comment(String),
}

#[derive(Debug, Clone)]
pub enum DrumEvent {
    Hit(DrumHit),
    Rest { duration: u32 },
    BarLine,
    RepeatStart(u32),
    RepeatEnd,
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct DrumVoiceData {
    pub events: Vec<DrumEvent>,
    pub punchcard_color: Option<String>,
    pub gain: Option<String>,
    pub pan: Option<String>,
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
    /// Multiple drum voices that play simultaneously
    Drums(Vec<DrumVoiceData>),
}

#[derive(Debug, Clone)]
pub struct Staff {
    pub kind: StaffKind,
    pub content: StaffContent,
    pub punchcard_color: Option<String>,
    pub gain: Option<String>,
    pub pan: Option<String>,
}

impl Staff {
    pub fn new_pitched(events: Vec<PitchedEvent>) -> Self {
        Staff {
            kind: StaffKind::Pitched,
            content: StaffContent::Notes(events),
            punchcard_color: None,
            gain: None,
            pan: None,
        }
    }

    pub fn new_pitched_with_options(
        events: Vec<PitchedEvent>,
        punchcard_color: Option<String>,
        gain: Option<String>,
        pan: Option<String>,
    ) -> Self {
        Staff {
            kind: StaffKind::Pitched,
            content: StaffContent::Notes(events),
            punchcard_color,
            gain,
            pan,
        }
    }

    pub fn new_drums(voices: Vec<DrumVoiceData>) -> Self {
        Staff {
            kind: StaffKind::Drums,
            content: StaffContent::Drums(voices),
            punchcard_color: None,
            gain: None,
            pan: None,
        }
    }

    pub fn events(&self) -> Option<&Vec<PitchedEvent>> {
        match &self.content {
            StaffContent::Notes(events) => Some(events),
            _ => None,
        }
    }

    pub fn drum_voices(&self) -> Option<&Vec<DrumVoiceData>> {
        match &self.content {
            StaffContent::Drums(voices) => Some(voices),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub staves: Vec<Staff>,
    pub tempo: Tempo,
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

/// Expand `\include "file.ly"` directives by recursively inlining file contents.
pub fn expand_includes(code: &str, base_dir: &Path) -> Result<String, String> {
    let mut seen = HashSet::new();
    expand_includes_recursive(code, base_dir, &mut seen)
}

fn expand_includes_recursive(
    code: &str,
    base_dir: &Path,
    seen: &mut HashSet<PathBuf>,
) -> Result<String, String> {
    let re = regex::Regex::new(r#"\\include\s+"([^"]+)""#).unwrap();
    let mut result = code.to_string();

    loop {
        let Some(caps) = re.captures(&result) else {
            break;
        };

        let full_match = caps.get(0).unwrap();
        let file_name = caps.get(1).unwrap().as_str();
        let file_path = base_dir.join(file_name);

        let canonical = file_path
            .canonicalize()
            .map_err(|e| format!("Cannot resolve include \"{}\": {}", file_name, e))?;

        if !seen.insert(canonical.clone()) {
            return Err(format!("Circular include detected: \"{}\"", file_name));
        }

        let content = std::fs::read_to_string(&canonical)
            .map_err(|e| format!("Cannot read include \"{}\": {}", file_name, e))?;

        let child_base = canonical.parent().unwrap_or(base_dir);
        let expanded = expand_includes_recursive(&content, child_base, seen)?;

        result = format!(
            "{}{}{}",
            &result[..full_match.start()],
            expanded,
            &result[full_match.end()..]
        );
    }

    Ok(result)
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
        let tempo = self.parse_tempo(code)
            .ok_or("Missing tempo: LilyPond input must include a \\tempo directive (e.g., \\tempo 4 = 120)")?;
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

    fn parse_punchcard_color(&self, content: &str) -> Option<String> {
        // Look for % @strudel-of-lilypond@ <color> punchcard comment
        let re = regex::Regex::new(r"%\s*@strudel-of-lilypond@\s+(\w+)\s+punchcard").unwrap();
        re.captures(content).map(|caps| caps.get(1).unwrap().as_str().to_string())
    }

    fn parse_gain(&self, content: &str) -> Option<String> {
        // Look for % @strudel-of-lilypond@ gain <value> comment
        // Value can be a number (2) or a Strudel pattern (<0.5 1 1.5>)
        let re = regex::Regex::new(r"%\s*@strudel-of-lilypond@\s+gain\s+([^\n]+)").unwrap();
        re.captures(content).map(|caps| caps.get(1).unwrap().as_str().trim().to_string())
    }

    fn parse_pan(&self, content: &str) -> Option<String> {
        // Look for % @strudel-of-lilypond@ pan <value> comment
        // Value can be a number (0.5) or a Strudel pattern (<0 .5 1>)
        let re = regex::Regex::new(r"%\s*@strudel-of-lilypond@\s+pan\s+([^\n]+)").unwrap();
        re.captures(content).map(|caps| caps.get(1).unwrap().as_str().trim().to_string())
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
                let punchcard_color = self.parse_punchcard_color(&staff_content);
                let gain = self.parse_gain(&staff_content);
                let pan = self.parse_pan(&staff_content);
                let resolved = self.resolve_variables(&staff_content, variables);
                // Check if resolved content is from a drum variable
                if self.is_drum_content(&staff_content, variables) {
                    let hits = self.parse_drums_from_section(&resolved)?;
                    if !hits.is_empty() {
                        let voice_data = DrumVoiceData { events: hits, punchcard_color, gain, pan };
                        staves.push(Staff::new_drums(vec![voice_data]));
                    }
                } else {
                    let notes = self.parse_notes_from_section(&resolved)?;
                    if !notes.is_empty() {
                        let staff = Staff::new_pitched_with_options(notes, punchcard_color, gain, pan);
                        staves.push(staff);
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
                                let voice_data = DrumVoiceData { events: hits, punchcard_color: None, gain: None, pan: None };
                                staves.push(Staff::new_drums(vec![voice_data]));
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

    fn mark_comments(&self, section: &str) -> String {
        let re = regex::Regex::new(r"(?m)%\s*@strudel-of-lilypond@\s+comment\s+(.+)$").unwrap();
        re.replace_all(section, |caps: &regex::Captures| {
            let text = caps.get(1).unwrap().as_str().replace(' ', "\x01");
            format!("__COMMENT_{}__", text)
        }).to_string()
    }

    fn parse_notes_from_section(&self, section: &str) -> Result<Vec<PitchedEvent>, String> {
        let mut events = Vec::new();
        let section = self.mark_comments(section);
        let tokens = self.tokenize(&section);
        let repeat_start_re = regex::Regex::new(r"^__REPEAT_START_(\d+)__$").unwrap();
        let comment_re = regex::Regex::new(r"^__COMMENT_(.+)__$").unwrap();

        for token in tokens {
            if let Some(caps) = comment_re.captures(&token) {
                events.push(PitchedEvent::Comment(caps.get(1).unwrap().as_str().replace('\x01', " ")));
            } else if token.starts_with('|') {
                events.push(PitchedEvent::BarLine);
            } else if let Some(caps) = repeat_start_re.captures(&token) {
                let count: u32 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
                events.push(PitchedEvent::RepeatStart(count));
            } else if token == "__REPEAT_END__" {
                events.push(PitchedEvent::RepeatEnd);
            } else if let Some(rest) = self.parse_rest(&token) {
                events.push(rest);
            } else if let Some(note) = self.parse_note(&token)? {
                events.push(PitchedEvent::Note(note));
            }
        }

        Ok(events)
    }

    fn parse_drums_from_section(&self, section: &str) -> Result<Vec<DrumEvent>, String> {
        let mut events = Vec::new();
        let section = self.mark_comments(section);
        let tokens = self.tokenize(&section);
        let repeat_start_re = regex::Regex::new(r"^__REPEAT_START_(\d+)__$").unwrap();
        let comment_re = regex::Regex::new(r"^__COMMENT_(.+)__$").unwrap();

        for token in tokens {
            if let Some(caps) = comment_re.captures(&token) {
                events.push(DrumEvent::Comment(caps.get(1).unwrap().as_str().replace('\x01', " ")));
            } else if token.starts_with('|') {
                events.push(DrumEvent::BarLine);
            } else if let Some(caps) = repeat_start_re.captures(&token) {
                let count: u32 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
                events.push(DrumEvent::RepeatStart(count));
            } else if token == "__REPEAT_END__" {
                events.push(DrumEvent::RepeatEnd);
            } else if let Some(rest) = self.parse_drum_rest(&token) {
                events.push(rest);
            } else if let Some(hit) = self.parse_drum_hit(&token) {
                events.push(DrumEvent::Hit(hit));
            }
        }

        Ok(events)
    }

    fn parse_drum_rest(&self, token: &str) -> Option<DrumEvent> {
        let token = token.trim();

        // Must start with 'r' and not be a command like \repeat
        if !token.starts_with('r') || token.starts_with("repeat") {
            return None;
        }

        // Parse duration after 'r'
        let mut chars = token[1..].chars().peekable();
        let mut duration_str = String::new();

        while let Some(&c) = chars.peek() {
            if c.is_numeric() {
                duration_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        // Verify no alphabetic characters follow (would indicate this isn't a rest)
        if chars.any(|c| c.is_alphabetic()) {
            return None;
        }

        let duration = if duration_str.is_empty() {
            4 // Default to quarter note
        } else {
            duration_str.parse::<u32>().unwrap_or(4)
        };

        Some(DrumEvent::Rest { duration })
    }

    fn parse_drum_voices(
        &self,
        staff_content: &str,
        variables: &HashMap<String, VariableKind>,
    ) -> Result<Vec<DrumVoiceData>, String> {
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
                        let punchcard_color = self.parse_punchcard_color(&voice_content);
                        let gain = self.parse_gain(&voice_content);
                        let pan = self.parse_pan(&voice_content);
                        let resolved = self.resolve_variables(&voice_content, variables);
                        let events = self.parse_drums_from_section(&resolved)?;
                        if !events.is_empty() {
                            voices.push(DrumVoiceData { events, punchcard_color, gain, pan });
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
                                voices.push(DrumVoiceData { events, punchcard_color: None, gain: None, pan: None });
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
                voices.push(DrumVoiceData { events, punchcard_color: None, gain: None, pan: None });
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
            "ss" => "rim".to_string(),     // side stick
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
        // Try literal: \tempo 4 = 120
        let re = regex::Regex::new(r"\\tempo\s+(\d+)\s*=\s*(\d+)").ok()?;
        if let Some(caps) = re.captures(code) {
            let beat_unit: u32 = caps.get(1)?.as_str().parse().ok()?;
            let bpm: u32 = caps.get(2)?.as_str().parse().ok()?;
            return Some(Tempo { beat_unit, bpm });
        }

        // Try variable reference: \tempo 4 = \varname where varname = 120
        let var_re = regex::Regex::new(r"\\tempo\s+(\d+)\s*=\s*\\([a-zA-Z_][a-zA-Z0-9_]*)").ok()?;
        if let Some(caps) = var_re.captures(code) {
            let beat_unit: u32 = caps.get(1)?.as_str().parse().ok()?;
            let var_name = caps.get(2)?.as_str();
            // Look for simple scalar assignment: varname = <number>
            let val_re = regex::Regex::new(
                &format!(r"(?m)^{}\s*=\s*(\d+)", regex::escape(var_name))
            ).ok()?;
            if let Some(val_caps) = val_re.captures(code) {
                let bpm: u32 = val_caps.get(1)?.as_str().parse().ok()?;
                return Some(Tempo { beat_unit, bpm });
            }
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

    fn parse_rest(&self, token: &str) -> Option<PitchedEvent> {
        let token = token.trim();

        // Must start with 'r' and not be a command like \repeat
        if !token.starts_with('r') || token.starts_with("repeat") {
            return None;
        }

        // Parse duration after 'r'
        let mut chars = token[1..].chars().peekable();
        let mut duration_str = String::new();

        while let Some(&c) = chars.peek() {
            if c.is_numeric() {
                duration_str.push(c);
                chars.next();
            } else {
                break;
            }
        }

        // Verify no alphabetic characters follow (would indicate this isn't a rest)
        if chars.any(|c| c.is_alphabetic()) {
            return None;
        }

        let duration = if duration_str.is_empty() {
            4 // Default to quarter note
        } else {
            duration_str.parse::<u32>().unwrap_or(4)
        };

        Some(PitchedEvent::Rest { duration })
    }

    fn parse_note(&self, token: &str) -> Result<Option<Note>, String> {
        let token = token.trim();

        if token.starts_with('|') || token.starts_with('\\') {
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

        let mut octave = 3; // LilyPond base octave (c = C3, c' = C4 middle C)
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

        midi += (octave + 1) * 12; // MIDI octave offset: C4 = 60

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
    /// Count bars in pitched events (including repeats)
    fn count_pitched_bars(events: &[PitchedEvent], idx: &mut usize) -> u32 {
        let mut bars = 0;
        let mut has_content = false;

        while *idx < events.len() {
            match &events[*idx] {
                PitchedEvent::Note(_) | PitchedEvent::Rest { .. } => {
                    has_content = true;
                    *idx += 1;
                }
                PitchedEvent::BarLine => {
                    if has_content {
                        bars += 1;
                        has_content = false;
                    }
                    *idx += 1;
                }
                PitchedEvent::RepeatStart(count) => {
                    if has_content {
                        bars += 1;
                        has_content = false;
                    }
                    *idx += 1;
                    let inner_bars = Self::count_pitched_bars(events, idx);
                    bars += inner_bars * count;
                }
                PitchedEvent::RepeatEnd => {
                    *idx += 1;
                    break;
                }
                PitchedEvent::Comment(_) => {
                    *idx += 1;
                }
            }
        }

        // Count final bar if there's content
        if has_content {
            bars += 1;
        }

        bars
    }

    /// Count bars in drum events (including repeats)
    fn count_drum_bars(events: &[DrumEvent], idx: &mut usize) -> u32 {
        let mut bars = 0;
        let mut has_content = false;

        while *idx < events.len() {
            match &events[*idx] {
                DrumEvent::Hit(_) | DrumEvent::Rest { .. } => {
                    has_content = true;
                    *idx += 1;
                }
                DrumEvent::BarLine => {
                    if has_content {
                        bars += 1;
                        has_content = false;
                    }
                    *idx += 1;
                }
                DrumEvent::RepeatStart(count) => {
                    if has_content {
                        bars += 1;
                        has_content = false;
                    }
                    *idx += 1;
                    let inner_bars = Self::count_drum_bars(events, idx);
                    bars += inner_bars * count;
                }
                DrumEvent::RepeatEnd => {
                    *idx += 1;
                    break;
                }
                DrumEvent::Comment(_) => {
                    *idx += 1;
                }
            }
        }

        // Count final bar if there's content
        if has_content {
            bars += 1;
        }

        bars
    }

    /// Generate CPM expression as tempo/4/bars
    fn format_cpm_expression(bars: u32) -> String {
        format!("tempo/4/{}", bars)
    }

    fn format_note(n: &Note) -> String {
        let acc = match &n.accidental {
            Some(a) if a == "is" => "#",
            Some(a) if a == "es" => "b",
            _ => "",
        };
        format!("{}{}{}", n.name, acc, n.octave)
    }

    /// Format modifier value - wrap in quotes if it's a Strudel pattern
    fn format_pattern_value(value: &str) -> String {
        if value.contains('<') {
            format!("\"{}\"", value)
        } else {
            value.to_string()
        }
    }

    /// Format weight as explicit numeric value (4, 2, 1, 0.5, 0.25)
    fn format_weight(duration: u32) -> Option<String> {
        match duration {
            1 => Some("4".to_string()),       // whole note = 4 quarter notes
            2 => Some("2".to_string()),       // half note = 2 quarter notes
            4 => None,                         // quarter note = 1 (no weight needed)
            8 => Some("0.5".to_string()),     // eighth note = 0.5 quarter notes
            16 => Some("0.25".to_string()),   // sixteenth note = 0.25 quarter notes
            _ => {
                let weight = 4.0 / duration as f32;
                Some(weight.to_string())
            }
        }
    }

    fn format_pitched_note(n: &Note) -> String {
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

        match Self::format_weight(n.duration) {
            Some(w) => format!("{}@{}", note_str, w),
            None => note_str,
        }
    }

    fn format_rest(duration: u32) -> String {
        // Convert rest duration to number of quarter note rests
        // duration 4 = 1 quarter note = "~"
        // duration 2 = half note = 2 quarter notes = "~ ~"
        // duration 1 = whole note = 4 quarter notes = "~ ~ ~ ~"
        let quarter_notes = 4 / duration;
        if quarter_notes <= 1 {
            "~".to_string()
        } else {
            vec!["~"; quarter_notes as usize].join(" ")
        }
    }

    /// Returns (pattern_string, bar_count)
    fn generate_pitched_pattern_with_bars(events: &[PitchedEvent], idx: &mut usize) -> (String, u32) {
        let mut bars: Vec<String> = Vec::new();
        let mut current_bar: Vec<String> = Vec::new();
        let mut bar_count: u32 = 0;

        while *idx < events.len() {
            match &events[*idx] {
                PitchedEvent::Note(n) => {
                    current_bar.push(Self::format_pitched_note(n));
                    *idx += 1;
                }
                PitchedEvent::Rest { duration } => {
                    current_bar.push(Self::format_rest(*duration));
                    *idx += 1;
                }
                PitchedEvent::BarLine => {
                    // Save current bar and start a new one
                    if !current_bar.is_empty() {
                        bars.push(format!("[{}]", current_bar.join(" ")));
                        current_bar = Vec::new();
                        bar_count += 1;
                    }
                    *idx += 1;
                }
                PitchedEvent::RepeatStart(count) => {
                    // Save current bar content before repeat
                    if !current_bar.is_empty() {
                        bars.push(format!("[{}]", current_bar.join(" ")));
                        current_bar = Vec::new();
                        bar_count += 1;
                    }
                    *idx += 1;
                    let (inner, inner_bars) = Self::generate_pitched_pattern_with_bars(events, idx);
                    let total_bars = inner_bars * count;
                    // If more than one bar in repeat, add duration
                    if inner_bars > 1 {
                        bars.push(format!("[[{}]!{}]@{}", inner, count, total_bars));
                    } else {
                        bars.push(format!("[{}]!{}", inner, count));
                    }
                    bar_count += total_bars;
                }
                PitchedEvent::RepeatEnd => {
                    *idx += 1;
                    break; // Exit this level of recursion
                }
                PitchedEvent::Comment(_) => {
                    *idx += 1;
                }
            }
        }

        // Don't forget the last bar
        if !current_bar.is_empty() {
            bars.push(format!("[{}]", current_bar.join(" ")));
            bar_count += 1;
        }

        (bars.join("\n"), bar_count)
    }

    fn generate_pitched_pattern(events: &[PitchedEvent], idx: &mut usize) -> String {
        Self::generate_pitched_pattern_with_bars(events, idx).0
    }

    pub fn generate_pitched_staff(events: &[PitchedEvent], tempo: Option<&Tempo>) -> String {
        Self::generate_pitched_staff_with_options(events, tempo, &None, &None, &None)
    }

    fn generate_pitched_staff_with_options(
        events: &[PitchedEvent],
        tempo: Option<&Tempo>,
        punchcard_color: &Option<String>,
        gain: &Option<String>,
        pan: &Option<String>,
    ) -> String {
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

        // Build modifiers with newlines
        let mut modifiers = String::new();
        if let Some(g) = gain {
            modifiers.push_str(&format!("\n.gain({})", Self::format_pattern_value(g)));
        }
        if let Some(p) = pan {
            modifiers.push_str(&format!("\n.pan({})", Self::format_pattern_value(p)));
        }
        if let Some(color) = punchcard_color {
            modifiers.push_str(&format!("\n.color(\"{}\")", color));
            modifiers.push_str("\n._punchcard()");
        }

        let base = format!(
            "note(`\n{}`){}\n  .s(\"piano\")",
            pattern, modifiers
        );

        if tempo.is_some() {
            let mut bar_idx = 0;
            let bars = Self::count_pitched_bars(events, &mut bar_idx);
            if bars > 0 {
                format!("{base}\n  .cpm({})", Self::format_cpm_expression(bars))
            } else {
                base
            }
        } else {
            base
        }
    }

    fn format_drum_hit(h: &DrumHit) -> String {
        match Self::format_weight(h.duration) {
            Some(w) => format!("{}@{}", h.name, w),
            None => h.name.clone(),
        }
    }

    /// Returns (pattern_string, bar_count)
    fn generate_drum_pattern_with_bars(events: &[DrumEvent], idx: &mut usize) -> (String, u32) {
        let mut bars: Vec<String> = Vec::new();
        let mut current_bar: Vec<String> = Vec::new();
        let mut bar_count: u32 = 0;

        while *idx < events.len() {
            match &events[*idx] {
                DrumEvent::Hit(h) => {
                    current_bar.push(Self::format_drum_hit(h));
                    *idx += 1;
                }
                DrumEvent::Rest { duration } => {
                    current_bar.push(Self::format_rest(*duration));
                    *idx += 1;
                }
                DrumEvent::BarLine => {
                    // Save current bar and start a new one
                    if !current_bar.is_empty() {
                        bars.push(format!("[{}]", current_bar.join(" ")));
                        current_bar = Vec::new();
                        bar_count += 1;
                    }
                    *idx += 1;
                }
                DrumEvent::RepeatStart(count) => {
                    // Save current bar content before repeat
                    if !current_bar.is_empty() {
                        bars.push(format!("[{}]", current_bar.join(" ")));
                        current_bar = Vec::new();
                        bar_count += 1;
                    }
                    *idx += 1;
                    let (inner, inner_bars) = Self::generate_drum_pattern_with_bars(events, idx);
                    let total_bars = inner_bars * count;
                    // If more than one bar in repeat, add duration
                    if inner_bars > 1 {
                        bars.push(format!("[[{}]!{}]@{}", inner, count, total_bars));
                    } else {
                        bars.push(format!("[{}]!{}", inner, count));
                    }
                    bar_count += total_bars;
                }
                DrumEvent::RepeatEnd => {
                    *idx += 1;
                    break; // Exit this level of recursion
                }
                DrumEvent::Comment(_) => {
                    *idx += 1;
                }
            }
        }

        // Don't forget the last bar
        if !current_bar.is_empty() {
            bars.push(format!("[{}]", current_bar.join(" ")));
            bar_count += 1;
        }

        (bars.join("\n"), bar_count)
    }

    fn generate_drum_pattern(events: &[DrumEvent], idx: &mut usize) -> String {
        Self::generate_drum_pattern_with_bars(events, idx).0
    }

    #[allow(dead_code)]
    fn generate_single_drum_voice(events: &[DrumEvent], tempo: Option<&Tempo>) -> String {
        Self::generate_single_drum_voice_with_options(events, tempo, &None, &None, &None)
    }

    fn generate_single_drum_voice_with_options(
        events: &[DrumEvent],
        tempo: Option<&Tempo>,
        punchcard_color: &Option<String>,
        gain: &Option<String>,
        pan: &Option<String>,
    ) -> String {
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
        let base = format!("sound(`\n{}`)", pattern);

        // Build modifiers with newlines
        let mut modifiers = String::new();
        if let Some(g) = gain {
            modifiers.push_str(&format!("\n.gain({})", Self::format_pattern_value(g)));
        }
        if let Some(p) = pan {
            modifiers.push_str(&format!("\n.pan({})", Self::format_pattern_value(p)));
        }
        if let Some(color) = punchcard_color {
            modifiers.push_str(&format!("\n.color(\"{}\")", color));
            modifiers.push_str("\n._punchcard()");
        }

        let with_modifiers = format!("{}{}", base, modifiers);

        if tempo.is_some() {
            let mut bar_idx = 0;
            let bars = Self::count_drum_bars(events, &mut bar_idx);
            if bars > 0 {
                format!("{with_modifiers}\n  .cpm({})", Self::format_cpm_expression(bars))
            } else {
                with_modifiers
            }
        } else {
            with_modifiers
        }
    }

    fn format_voice_modifiers(punchcard_color: &Option<String>, gain: &Option<String>, pan: &Option<String>) -> String {
        let mut modifiers = String::new();
        if let Some(g) = gain {
            modifiers.push_str(&format!("\n  .gain({})", Self::format_pattern_value(g)));
        }
        if let Some(p) = pan {
            modifiers.push_str(&format!("\n  .pan({})", Self::format_pattern_value(p)));
        }
        if let Some(color) = punchcard_color {
            modifiers.push_str(&format!("\n  .color(\"{}\")", color));
            modifiers.push_str("\n  ._punchcard()");
        }
        modifiers
    }

    pub fn generate_drum_staff(voices: &[DrumVoiceData], tempo: Option<&Tempo>) -> String {
        if voices.is_empty() {
            return String::from("// No drum hits to convert");
        }

        if voices.len() == 1 {
            let voice = &voices[0];
            return Self::generate_single_drum_voice_with_options(
                &voice.events,
                tempo,
                &voice.punchcard_color,
                &voice.gain,
                &voice.pan,
            );
        }

        // Multiple voices: use stack() with per-voice punchcard
        let voice_patterns: Vec<String> = voices
            .iter()
            .map(|voice| {
                let mut idx = 0;
                let pattern = Self::generate_drum_pattern(&voice.events, &mut idx);
                let modifiers = Self::format_voice_modifiers(&voice.punchcard_color, &voice.gain, &voice.pan);
                format!("sound(`\n{}`){}", pattern, modifiers)
            })
            .collect();

        let stacked = format!("stack(\n  {},\n)", voice_patterns.join(",\n  "));

        if tempo.is_some() {
            // Use the longest voice to calculate bars
            let max_bars: u32 = voices
                .iter()
                .map(|voice| {
                    let mut idx = 0;
                    Self::count_drum_bars(&voice.events, &mut idx)
                })
                .max()
                .unwrap_or(0);

            if max_bars > 0 {
                format!("{stacked}\n  .cpm({})", Self::format_cpm_expression(max_bars))
            } else {
                stacked
            }
        } else {
            stacked
        }
    }

    pub fn generate_staff(staff: &Staff, tempo: Option<&Tempo>) -> String {
        match &staff.content {
            StaffContent::Notes(events) => {
                Self::generate_pitched_staff_with_options(events, tempo, &staff.punchcard_color, &staff.gain, &staff.pan)
            }
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
        let tempo_const = tempo
            .map(|t| format!("const tempo = {};", t.bpm))
            .unwrap_or_else(|| "const tempo = 120;".to_string());
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
{tempo_const}

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
