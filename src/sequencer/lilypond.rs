use std::path::{Path, PathBuf};

use super::model::{Pattern, Bar, EBarSequence, BarSequence};
use crate::{LilyPondParser, StrudelGenerator};

pub fn lilypond_bar_of_snippet(patterns: &[Pattern]) -> String {
    patterns
        .iter()
        .map(|pattern| {
            let voices: Vec<String> = pattern.voices.iter()
                .map(|v| format!("  \\new DrumVoice {{ {} }}", v.trim()))
                .collect();
            format!("<<\n{}\n>>", voices.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_pattern(path: &Path) -> Result<Pattern, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read pattern file '{}': {}", path.display(), e))?;
    serde_yaml::from_str(&content)
        .map_err(|e| format!("Cannot parse pattern file '{}': {}", path.display(), e))
}

fn resolve_pattern(pattern_name: &str, libraries: &[PathBuf]) -> Result<Pattern, String> {
    for lib in libraries {
        let yml_path = lib.join(format!("{pattern_name}.yml"));
        if yml_path.exists() {
            return parse_pattern(&yml_path);
        }
    }
    Err(format!(
        "pattern '{}' not found in libraries: {:?}",
        pattern_name,
        libraries.iter().map(|l| l.display().to_string()).collect::<Vec<_>>()
    ))
}

fn generate_voice_content<F>(
    sequence: &[EBarSequence],
    libraries: &[PathBuf],
    get_voice: &F,
    indent: &str,
    comment: Option<&str>,
    need_bar_sep: &mut bool,
) -> Result<String, String>
where
    F: Fn(&Pattern) -> &str,
{
    let mut lines: Vec<String> = Vec::new();

    for (i, item) in sequence.iter().enumerate() {
        let c = if i == 0 { comment } else { None };
        match item {
            EBarSequence::Single(bar) => {
                let pattern = resolve_pattern(&bar.pattern_name, libraries)?;
                if *need_bar_sep {
                    lines.push(format!("{}|", indent));
                }
                if let Some(text) = c {
                    lines.push(format!("{}% @strudel-of-lilypond@ comment {}", indent, text));
                }
                lines.push(format!("{}{}", indent, get_voice(&pattern).trim()));
                *need_bar_sep = true;
            }
            EBarSequence::Group(items) => {
                let inner = generate_voice_content(items, libraries, get_voice, indent, c, need_bar_sep)?;
                lines.push(inner);
            }
            EBarSequence::RepeatBar(count, bar) => {
                let pattern = resolve_pattern(&bar.pattern_name, libraries)?;
                if *need_bar_sep {
                    lines.push(format!("{}|", indent));
                }
                if let Some(text) = c {
                    lines.push(format!("{}% @strudel-of-lilypond@ comment {}", indent, text));
                }
                lines.push(format!("{}\\repeat volta {} {{", indent, count));
                lines.push(format!("{}  {}", indent, get_voice(&pattern).trim()));
                lines.push(format!("{}}}", indent));
                *need_bar_sep = false;
            }
            EBarSequence::RepeatGroup(count, items) => {
                if *need_bar_sep {
                    lines.push(format!("{}|", indent));
                }
                if let Some(text) = c {
                    lines.push(format!("{}% @strudel-of-lilypond@ comment {}", indent, text));
                }
                lines.push(format!("{}\\repeat volta {} {{", indent, count));
                let mut inner_sep = false;
                let inner = generate_voice_content(items, libraries, get_voice, &format!("{}  ", indent), None, &mut inner_sep)?;
                lines.push(inner);
                lines.push(format!("{}}}", indent));
                *need_bar_sep = false;
            }
        }
    }

    Ok(lines.join("\n"))
}

fn find_first_bar(items: &[EBarSequence]) -> Option<&Bar> {
    for item in items {
        match item {
            EBarSequence::Single(bar) | EBarSequence::RepeatBar(_, bar) => return Some(bar),
            EBarSequence::Group(inner) | EBarSequence::RepeatGroup(_, inner) => {
                if let Some(bar) = find_first_bar(inner) {
                    return Some(bar);
                }
            }
        }
    }
    None
}

pub fn lilypond_of_sequence(bar_sequence: &BarSequence, libraries: &[PathBuf], _output_dir: &Path) -> Result<String, String> {
    let indent = "            ";
    let items: Vec<EBarSequence> = bar_sequence.sequence.iter().map(|si| si.item.clone()).collect();
    let descriptions: Vec<&str> = bar_sequence.sequence.iter().map(|si| si.description.as_str()).collect();

    let first_bar = find_first_bar(&items).ok_or("Empty sequence")?;
    let first_pattern = resolve_pattern(&first_bar.pattern_name, libraries)?;
    let num_voices = first_pattern.voices.len();

    let voice_directives = ["\\voiceOne", "\\voiceTwo", "\\voiceThree", "\\voiceFour"];

    let mut voice_blocks = Vec::new();
    for voice_idx in 0..num_voices {
        let mut parts = Vec::new();
        let mut sep = false;

        for (item, desc) in items.iter().zip(descriptions.iter()) {
            let v = generate_voice_content(
                std::slice::from_ref(item), libraries,
                &|p: &Pattern| p.voices[voice_idx].as_str(), indent, Some(desc), &mut sep,
            )?;
            parts.push(v);
        }

        let voice_content = parts.join("\n");
        let directive = voice_directives.get(voice_idx).copied().unwrap_or("");
        voice_blocks.push(format!(
            "        \\new DrumVoice {{\n          {}\n{}\n        }}",
            directive, voice_content
        ));
    }

    let voices = voice_blocks.join("\n");

    Ok(format!(
        r#"\version "2.24.4"

\paper {{
  #(include-special-characters)
  indent = 0\mm
  line-width = 180\mm
  oddHeaderMarkup = ""
  evenHeaderMarkup = ""
  oddFooterMarkup = ""
  evenFooterMarkup = ""
  #(add-text-replacements!
    '(("100" . "hundred")
      ("dpi" . "dots per inch")))
}}

\score {{
  <<
    \tempo 4 = {tempo}

    \new DrumStaff {{
      <<
{voices}
      >>
    }}
  >>

  \layout {{}}
}}
"#,
        tempo = bar_sequence.tempo,
        voices = voices,
    ))
}

pub fn strudel_of_sequence(bar_sequence: &BarSequence, libraries: &[PathBuf], title: &str) -> Result<String, String> {
    let dummy_dir = Path::new(".");
    let ly = lilypond_of_sequence(bar_sequence, libraries, dummy_dir)?;
    let parser = LilyPondParser::new();
    let result = parser.parse(&ly).map_err(|e| format!("LilyPond parse error: {e}"))?;
    Ok(StrudelGenerator::generate_html(&result.staves, &result.tempo, title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::{Bar, SequenceItem};
    use std::fs;

    #[test]
    fn test_lilypond_of_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let lib_dir = dir.path().join("library");
        fs::create_dir_all(&lib_dir).unwrap();

        fs::write(
            lib_dir.join("pattern1.yml"),
            "description: kick and snare\nvoices:\n  - bd4 sn4 bd4 sn4\n  - hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8\n",
        ).unwrap();
        fs::write(
            lib_dir.join("pattern2.yml"),
            "description: kick only\nvoices:\n  - bd4 r4 bd4 r4\n  - hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8\n",
        ).unwrap();

        let sequence = BarSequence {
            tempo: 120,
            sequence: vec![
                SequenceItem {
                    item: EBarSequence::Single(Bar { pattern_name: "pattern1".to_string() }),
                    description: "kick and snare".to_string(),
                },
                SequenceItem {
                    item: EBarSequence::Single(Bar { pattern_name: "pattern2".to_string() }),
                    description: "kick only".to_string(),
                },
            ],
        };

        let libraries = vec![lib_dir];
        let result = lilypond_of_sequence(&sequence, &libraries, dir.path()).unwrap();

        assert!(result.contains(r#"\version "2.24.4""#));
        assert!(result.contains(r"\tempo 4 = 120"));
        assert!(result.contains(r"\new DrumStaff"));
        assert!(result.contains(r"\voiceOne"));
        assert!(result.contains(r"\voiceTwo"));
        assert!(result.contains("bd4 sn4 bd4 sn4"));
        assert!(result.contains("bd4 r4 bd4 r4"));
        assert!(result.contains("hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8"));
        // Comment markers before each item
        assert!(result.contains("% @strudel-of-lilypond@ comment kick and snare"));
        assert!(result.contains("% @strudel-of-lilypond@ comment kick only"));
    }
}
