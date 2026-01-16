use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Note {
    pub name: char,
    pub octave: i32,
    pub accidental: Option<String>,
    pub duration: u32,
    #[allow(dead_code)]
    pub midi: i32,
}

#[derive(Debug, Clone)]
pub struct Tempo {
    pub beat_unit: u32,
    pub bpm: u32,
}

#[derive(Debug)]
pub struct ParseResult {
    pub notes: Vec<Note>,
    pub tempo: Option<Tempo>,
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
        let expanded = self.expand_repeats(code);
        let notes_section = self.extract_notes_section(&expanded)?;
        let mut notes = Vec::new();
        let tokens = self.tokenize(&notes_section);

        for token in tokens {
            if let Some(note) = self.parse_note(&token)? {
                notes.push(note);
            }
        }

        Ok(ParseResult { notes, tempo })
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
        section
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn parse_note(&self, token: &str) -> Result<Option<Note>, String> {
        let token = token.trim();

        if token.starts_with('|') || token.starts_with('\\') || token.starts_with('r') {
            return Ok(None);
        }

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
                '\'' => { octave += 1; chars.next(); }
                ',' => { octave -= 1; chars.next(); }
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

        let duration = if duration_str.is_empty() {
            4
        } else {
            duration_str.parse::<u32>().unwrap_or(4)
        };

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
    pub fn generate(notes: &[Note], tempo: Option<&Tempo>) -> String {
        if notes.is_empty() {
            return String::from("// No notes to convert");
        }

        let note_sequence: Vec<String> = notes
            .iter()
            .map(|n| {
                let acc = match &n.accidental {
                    Some(a) if a == "is" => "#",
                    Some(a) if a == "es" => "b",
                    _ => "",
                };
                let weight = 4.0 / n.duration as f32;
                if weight == 1.0 {
                    format!("{}{}{}", n.name, acc, n.octave)
                } else {
                    format!("{}{}{}@{}", n.name, acc, n.octave, weight)
                }
            })
            .collect();

        let base = format!(
            "note(\"{}\")\n  .s(\"piano\")",
            note_sequence.join(" ")
        );

        if let Some(t) = tempo {
            let cpm = t.bpm as f64 / t.beat_unit as f64;
            format!("{base}\n  .cpm({cpm})")
        } else {
            base
        }
    }

    pub fn generate_html(notes: &[Note], tempo: Option<&Tempo>, title: &str) -> String {
        let pattern = Self::generate(notes, tempo);
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
