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
    assert_eq!(result.staves[0].notes.len(), 2);
    assert_eq!(result.staves[0].notes[0].name, 'c');
    assert_eq!(result.staves[1].notes.len(), 2);
    assert_eq!(result.staves[1].notes[0].name, 'e');
}

#[test]
fn test_generate_multi_staff() {
    let staves = vec![
        Staff {
            notes: vec![Note {
                name: 'c',
                octave: 4,
                accidental: None,
                duration: 4,
                midi: 60,
            }],
        },
        Staff {
            notes: vec![Note {
                name: 'e',
                octave: 4,
                accidental: None,
                duration: 4,
                midi: 64,
            }],
        },
    ];

    let strudel = StrudelGenerator::generate_multi(&staves, None);
    assert!(strudel.contains("$: note(\"c4\")"));
    assert!(strudel.contains("$: note(\"e4\")"));
}
