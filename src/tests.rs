use crate::*;

#[test]
fn test_parse_simple_notes() {
    let parser = LilyPondParser::new();
    let code = "{ c'4 d'4 e'4 }";
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].name, 'c');
    assert_eq!(notes[0].octave, 4);
}

#[test]
fn test_parse_with_accidentals() {
    let parser = LilyPondParser::new();
    let code = "{ cis'4 des'4 }";
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].accidental, Some("is".to_string()));
}

#[test]
fn test_accidentals_affect_midi() {
    let parser = LilyPondParser::new();
    let code = "{ c'4 cis'4 d'4 des'4 }";
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[0].midi, 60);
    assert_eq!(notes[1].midi, 61);
    assert_eq!(notes[2].midi, 62);
    assert_eq!(notes[3].midi, 61);
}

#[test]
fn test_generate_strudel() {
    let notes = vec![
        Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        },
    ];

    let strudel = StrudelGenerator::generate(&notes, None);
    assert!(strudel.contains("c4"));
}

#[test]
fn test_parse_tempo() {
    let parser = LilyPondParser::new();
    let code = r#"\tempo 4 = 120
    { c'4 d'4 e'4 }"#;
    let result = parser.parse(code).unwrap();

    assert!(result.tempo.is_some());
    let tempo = result.tempo.unwrap();
    assert_eq!(tempo.beat_unit, 4);
    assert_eq!(tempo.bpm, 120);
}

#[test]
fn test_generate_with_tempo() {
    let notes = vec![
        Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        },
    ];
    let tempo = Tempo { beat_unit: 4, bpm: 120 };

    let strudel = StrudelGenerator::generate(&notes, Some(&tempo));
    // 1 note with weight 1 at 120 BPM = 120 cpm
    assert!(strudel.contains(".cpm(120)"));
}

#[test]
fn test_generate_without_tempo() {
    let notes = vec![
        Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        },
    ];

    let strudel = StrudelGenerator::generate(&notes, None);
    assert!(!strudel.contains(".cpm"));
}

#[test]
fn test_repeat_structure() {
    let parser = LilyPondParser::new();
    let code = r#"{ \repeat unfold 3 { c'4 d'4 } }"#;
    let result = parser.parse(code).unwrap();

    // notes() returns unique notes only (not expanded)
    let notes = result.notes();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].name, 'c');
    assert_eq!(notes[1].name, 'd');

    // Check that the generated output uses *3 syntax
    let events = result.staves[0].events().unwrap();
    let strudel = StrudelGenerator::generate_pitched_staff(events, None);
    assert!(strudel.contains("[c4 d4]*3"));
}

#[test]
fn test_nested_repeat() {
    let parser = LilyPondParser::new();
    let code = r#"{ \repeat unfold 2 { \repeat unfold 2 { c'4 } } }"#;
    let result = parser.parse(code).unwrap();

    // Only 1 unique note
    assert_eq!(result.notes().len(), 1);

    // Check nested repeat syntax
    let events = result.staves[0].events().unwrap();
    let strudel = StrudelGenerator::generate_pitched_staff(events, None);
    assert!(strudel.contains("[c4]*2]*2") || strudel.contains("[[c4]*2]*2"));
}

#[test]
fn test_multi_staff_score() {
    let parser = LilyPondParser::new();
    let code = r#"
voicea = { c'4 d'4 }
voiceb = { e'4 f'4 }

\score {
  <<
    \new Staff { \voicea }
    \new Staff { \voiceb }
  >>
}
"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.staves.len(), 2);
    let events0 = result.staves[0].events().unwrap();
    let notes0: Vec<_> = events0.iter().filter_map(|e| match e {
        PitchedEvent::Note(n) => Some(n),
        _ => None,
    }).collect();
    assert_eq!(notes0.len(), 2);
    assert_eq!(notes0[0].name, 'c');
    let events1 = result.staves[1].events().unwrap();
    let notes1: Vec<_> = events1.iter().filter_map(|e| match e {
        PitchedEvent::Note(n) => Some(n),
        _ => None,
    }).collect();
    assert_eq!(notes1.len(), 2);
    assert_eq!(notes1[0].name, 'e');
}

#[test]
fn test_generate_multi_staff() {
    let staves = vec![
        Staff::new_pitched(vec![PitchedEvent::Note(Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        })]),
        Staff::new_pitched(vec![PitchedEvent::Note(Note {
            name: 'e',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 64,
            chord_notes: None,
        })]),
    ];

    let strudel = StrudelGenerator::generate_multi(&staves, None);
    assert!(strudel.contains("$: note(\"c4\")"));
    assert!(strudel.contains("$: note(\"e4\")"));
}

#[test]
fn test_parse_drum_staff() {
    let parser = LilyPondParser::new();
    let code = r#"
mydrums = \drummode { bd4 hh4 sn4 hh4 }

\score {
  <<
    \new DrumStaff { \mydrums }
  >>
}
"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.staves.len(), 1);
    let voices = result.staves[0].drum_voices().unwrap();
    assert_eq!(voices.len(), 1);
    let hits: Vec<_> = voices[0].events.iter().filter_map(|e| match e {
        DrumEvent::Hit(h) => Some(h),
        _ => None,
    }).collect();
    assert_eq!(hits.len(), 4);
    assert_eq!(hits[0].name, "bd");
    assert_eq!(hits[1].name, "hh");
    assert_eq!(hits[2].name, "sd");  // sn -> sd in Strudel
    assert_eq!(hits[3].name, "hh");
}

#[test]
fn test_generate_drum_staff() {
    let voices = vec![DrumVoiceData {
        events: vec![
            DrumEvent::Hit(DrumHit { name: "bd".to_string(), duration: 4 }),
            DrumEvent::Hit(DrumHit { name: "hh".to_string(), duration: 4 }),
        ],
        punchcard_color: None,
        gain: None,
        pan: None,
    }];

    let strudel = StrudelGenerator::generate_drum_staff(&voices, None);
    assert!(strudel.contains("sound(\"bd hh\")"));
}

#[test]
fn test_parse_multi_voice_drum_staff() {
    let parser = LilyPondParser::new();
    let code = r#"
kicks = \drummode { bd4 bd4 }
hats = \drummode { hh8 hh8 hh8 hh8 }

\score {
  <<
    \new DrumStaff {
      <<
        \new DrumVoice { \kicks }
        \new DrumVoice { \hats }
      >>
    }
  >>
}
"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.staves.len(), 1);
    let voices = result.staves[0].drum_voices().unwrap();
    assert_eq!(voices.len(), 2);
    let hits0: Vec<_> = voices[0].events.iter().filter_map(|e| match e {
        DrumEvent::Hit(h) => Some(h),
        _ => None,
    }).collect();
    let hits1: Vec<_> = voices[1].events.iter().filter_map(|e| match e {
        DrumEvent::Hit(h) => Some(h),
        _ => None,
    }).collect();
    assert_eq!(hits0[0].name, "bd");
    assert_eq!(hits1[0].name, "hh");
}

#[test]
fn test_generate_multi_voice_drum_staff() {
    let voices = vec![
        DrumVoiceData {
            events: vec![DrumEvent::Hit(DrumHit { name: "bd".to_string(), duration: 4 })],
            punchcard_color: None,
            gain: None,
            pan: None,
        },
        DrumVoiceData {
            events: vec![DrumEvent::Hit(DrumHit { name: "hh".to_string(), duration: 8 })],
            punchcard_color: None,
            gain: None,
            pan: None,
        },
    ];

    let strudel = StrudelGenerator::generate_drum_staff(&voices, None);
    assert!(strudel.contains("stack("));
    assert!(strudel.contains("sound(\"bd\")"));
    assert!(strudel.contains("sound(\"hh@0.5\")"));
}

#[test]
fn test_mixed_pitched_and_drum_staves() {
    let parser = LilyPondParser::new();
    let code = r#"
voice = { c'4 d'4 }
drums = \drummode { bd4 sn4 }

\score {
  <<
    \new Staff { \voice }
    \new DrumStaff { \drums }
  >>
}
"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.staves.len(), 2);
    assert!(result.staves[0].events().is_some());
    assert!(result.staves[1].drum_voices().is_some());
}

#[test]
fn test_generate_mixed_staves() {
    let staves = vec![
        Staff::new_pitched(vec![PitchedEvent::Note(Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        })]),
        Staff::new_drums(vec![DrumVoiceData {
            events: vec![DrumEvent::Hit(DrumHit { name: "bd".to_string(), duration: 4 })],
            punchcard_color: None,
            gain: None,
            pan: None,
        }]),
    ];

    let strudel = StrudelGenerator::generate_multi(&staves, None);
    assert!(strudel.contains("$: note(\"c4\")"));
    assert!(strudel.contains("$: sound(\"bd\")"));
}

#[test]
fn test_parse_chord() {
    let parser = LilyPondParser::new();
    let code = "{ <a c e>4 g'4 }";
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 2);

    // First note is a chord
    assert_eq!(notes[0].name, 'a');
    assert!(notes[0].chord_notes.is_some());
    let chord_notes = notes[0].chord_notes.as_ref().unwrap();
    assert_eq!(chord_notes.len(), 2);
    assert_eq!(chord_notes[0].name, 'c');
    assert_eq!(chord_notes[1].name, 'e');

    // Second note is a regular note
    assert_eq!(notes[1].name, 'g');
    assert!(notes[1].chord_notes.is_none());
}

#[test]
fn test_generate_chord() {
    let notes = vec![
        Note {
            name: 'a',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 57,
            chord_notes: Some(vec![
                Note {
                    name: 'c',
                    octave: 4,
                    accidental: None,
                    duration: 4,
                    midi: 48,
                    chord_notes: None,
                },
                Note {
                    name: 'e',
                    octave: 4,
                    accidental: None,
                    duration: 4,
                    midi: 52,
                    chord_notes: None,
                },
            ]),
        },
    ];

    let strudel = StrudelGenerator::generate(&notes, None);
    assert!(strudel.contains("[a4,c4,e4]"));
}

#[test]
fn test_bar_line_parsed() {
    // Test that bar lines are parsed (but ignored in output)
    let parser = LilyPondParser::new();
    let code = "{ c'4 d'4 | e'4 f'4 }";
    let result = parser.parse(code).unwrap();

    let events = result.staves[0].events().unwrap();
    // Should have 4 notes and 1 bar line
    assert_eq!(events.len(), 5);
    assert!(matches!(events[2], PitchedEvent::BarLine));

    let strudel = StrudelGenerator::generate_pitched_staff(events, None);
    // Bar lines are skipped in output
    assert!(strudel.contains("c4 d4 e4 f4"));
}

#[test]
fn test_pan_modifier() {
    let parser = LilyPondParser::new();
    let code = r#"
voice = { c'4 d'4 }

\score {
  <<
    \new Staff {
      % @strudel-of-lilypond@ pan 0.25
      \voice
    }
  >>
}
"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.staves.len(), 1);
    assert_eq!(result.staves[0].pan, Some("0.25".to_string()));

    let strudel = StrudelGenerator::generate_staff(&result.staves[0], None);
    assert!(strudel.contains(".pan(0.25)"));
}

