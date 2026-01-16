use crate::*;

#[test]
fn test_parse_simple_notes() {
    let parser = LilyPondParser::new();
    let code = "{ c'4 d'4 e'4 }";
    let result = parser.parse(code).unwrap();

    assert_eq!(result.notes.len(), 3);
    assert_eq!(result.notes[0].name, 'c');
    assert_eq!(result.notes[0].octave, 5);
}

#[test]
fn test_parse_with_accidentals() {
    let parser = LilyPondParser::new();
    let code = "{ cis'4 des'4 }";
    let result = parser.parse(code).unwrap();

    assert_eq!(result.notes.len(), 2);
    assert_eq!(result.notes[0].accidental, Some("is".to_string()));
}

#[test]
fn test_accidentals_affect_midi() {
    let parser = LilyPondParser::new();
    let code = "{ c'4 cis'4 d'4 des'4 }";
    let result = parser.parse(code).unwrap();

    assert_eq!(result.notes.len(), 4);
    assert_eq!(result.notes[0].midi, 60);
    assert_eq!(result.notes[1].midi, 61);
    assert_eq!(result.notes[2].midi, 62);
    assert_eq!(result.notes[3].midi, 61);
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
    assert!(strudel.contains(".cpm(30)"));
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

    assert_eq!(result.notes.len(), 6);
    assert_eq!(result.notes[0].name, 'c');
    assert_eq!(result.notes[1].name, 'd');
}

#[test]
fn test_nested_repeat() {
    let parser = LilyPondParser::new();
    let code = r#"{ \repeat unfold 2 { \repeat unfold 2 { c'4 } } }"#;
    let result = parser.parse(code).unwrap();

    assert_eq!(result.notes.len(), 4);
}
