use std::env;
use std::fs;
use std::path::Path;

use strudel_of_lilypond::{LilyPondParser, StrudelGenerator, PitchedEvent, DrumEvent};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.ly> [output.html]", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_path = if args.len() > 2 {
        args[2].clone()
    } else {
        format!("{stem}.html")
    };

    let input = match fs::read_to_string(input_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading {input_path}: {e}");
            std::process::exit(1);
        }
    };

    let parser = LilyPondParser::new();

    match parser.parse(&input) {
        Ok(result) => {
            let pitched_count: usize = result.staves.iter()
                .filter_map(|s| s.events())
                .flat_map(|events| events.iter())
                .filter(|e| matches!(e, PitchedEvent::Note(_)))
                .count();
            let drum_count: usize = result.staves.iter()
                .filter_map(|s| s.drum_events())
                .flat_map(|voices| voices.iter())
                .flat_map(|voice| voice.iter())
                .filter(|e| matches!(e, DrumEvent::Hit(_)))
                .count();
            eprintln!(
                "Parsed {} staves ({} notes, {} drum hits)",
                result.staves.len(), pitched_count, drum_count
            );
            if let Some(ref tempo) = result.tempo {
                eprintln!("Tempo: {} = {} BPM", tempo.beat_unit, tempo.bpm);
            }
            let html = StrudelGenerator::generate_html(&result.staves, result.tempo.as_ref(), stem);

            match fs::write(&output_path, &html) {
                Ok(_) => println!("{output_path}"),
                Err(e) => {
                    eprintln!("Error writing {output_path}: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            std::process::exit(1);
        }
    }
}
