use crate::*;

#[test]
fn test_parse_simple_notes() {
    let parser = LilyPondParser::new();
    let code = "{ c'4 d'4 e'4 }";
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 3);
    assert_eq!(notes[0].name, 'c');
    assert_eq!(notes[0].octave, 5);
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
fn test_repeat_expansion() {
    let parser = LilyPondParser::new();
    let code = r#"{ \repeat unfold 3 { c'4 d'4 } }"#;
    let result = parser.parse(code).unwrap();

    let notes = result.notes();
    assert_eq!(notes.len(), 6);
    assert_eq!(notes[0].name, 'c');
    assert_eq!(notes[1].name, 'd');
}

#[test]
fn test_nested_repeat() {
    let parser = LilyPondParser::new();
    let code = r#"{ \repeat unfold 2 { \repeat unfold 2 { c'4 } } }"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.notes().len(), 4);
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
    assert_eq!(result.staves[0].notes().unwrap().len(), 2);
    assert_eq!(result.staves[0].notes().unwrap()[0].name, 'c');
    assert_eq!(result.staves[1].notes().unwrap().len(), 2);
    assert_eq!(result.staves[1].notes().unwrap()[0].name, 'e');
}

#[test]
fn test_generate_multi_staff() {
    let staves = vec![
        Staff::new_pitched(vec![Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        }]),
        Staff::new_pitched(vec![Note {
            name: 'e',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 64,
            chord_notes: None,
        }]),
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
    let voices = result.staves[0].drums().unwrap();
    assert_eq!(voices.len(), 1);
    assert_eq!(voices[0].len(), 4);
    assert_eq!(voices[0][0].name, "bd");
    assert_eq!(voices[0][1].name, "hh");
    assert_eq!(voices[0][2].name, "sd");  // sn -> sd in Strudel
    assert_eq!(voices[0][3].name, "hh");
}

#[test]
fn test_generate_drum_staff() {
    let voices = vec![vec![
        DrumHit { name: "bd".to_string(), duration: 4 },
        DrumHit { name: "hh".to_string(), duration: 4 },
    ]];

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
    let voices = result.staves[0].drums().unwrap();
    assert_eq!(voices.len(), 2);
    assert_eq!(voices[0][0].name, "bd");
    assert_eq!(voices[1][0].name, "hh");
}

#[test]
fn test_generate_multi_voice_drum_staff() {
    let voices = vec![
        vec![DrumHit { name: "bd".to_string(), duration: 4 }],
        vec![DrumHit { name: "hh".to_string(), duration: 8 }],
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
    assert!(result.staves[0].notes().is_some());
    assert!(result.staves[1].drums().is_some());
}

#[test]
fn test_generate_mixed_staves() {
    let staves = vec![
        Staff::new_pitched(vec![Note {
            name: 'c',
            octave: 4,
            accidental: None,
            duration: 4,
            midi: 60,
            chord_notes: None,
        }]),
        Staff::new_drums(vec![vec![
            DrumHit { name: "bd".to_string(), duration: 4 },
        ]]),
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
fn test_pattern_compression() {
    // Test single element repetition
    let parser = LilyPondParser::new();
    let code = "{ c'4 c'4 c'4 c'4 }";
    let result = parser.parse(code).unwrap();
    let strudel = StrudelGenerator::generate(&result.notes(), None);
    assert!(strudel.contains("c5*4"));

    // Test pattern repetition
    let code2 = "{ c'4 d'4 c'4 d'4 }";
    let result2 = parser.parse(code2).unwrap();
    let strudel2 = StrudelGenerator::generate(&result2.notes(), None);
    assert!(strudel2.contains("[c5 d5]*2"));
}
