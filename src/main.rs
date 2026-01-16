use std::env;
use std::fs;
use std::path::Path;

use strudel_of_lilypond::{LilyPondParser, StrudelGenerator};

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
            eprintln!("Parsed {} notes", result.notes.len());
            if let Some(ref tempo) = result.tempo {
                eprintln!("Tempo: {} = {} BPM", tempo.beat_unit, tempo.bpm);
            }
            let html = StrudelGenerator::generate_html(&result.notes, result.tempo.as_ref(), stem);

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
