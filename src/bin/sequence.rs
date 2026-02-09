use std::fs;
use std::path::{Path, PathBuf};

use argh::FromArgs;
use strudel_of_lilypond::sequencer::lilypond::lilypond_of_sequence;
use strudel_of_lilypond::sequencer::model::BarSequence;
use strudel_of_lilypond::{expand_includes, LilyPondParser, StrudelGenerator, PitchedEvent, DrumEvent};

/// Generate LilyPond files from a YAML bar sequence
#[derive(FromArgs)]
struct Args {
    /// path to a library root directory (can be repeated)
    #[argh(option)]
    library: Vec<String>,

    /// input YAML sequence file
    #[argh(positional)]
    input: String,
}

fn main() {
    let args: Args = argh::from_env();

    let input_path = &args.input;
    let libraries: Vec<PathBuf> = args.library.iter().map(PathBuf::from).collect();

    let content = match fs::read_to_string(input_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {input_path}: {e}");
            std::process::exit(1);
        }
    };

    let sequence: BarSequence = match serde_yaml::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error parsing YAML: {e}");
            std::process::exit(1);
        }
    };

    let output_path = Path::new(input_path).with_extension("ly");
    let output_dir = output_path.parent().unwrap_or(Path::new("."));

    let output = match lilypond_of_sequence(&sequence, &libraries, output_dir) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match fs::write(&output_path, &output) {
        Ok(_) => eprintln!("Wrote {}", output_path.display()),
        Err(e) => {
            eprintln!("Error writing {}: {e}", output_path.display());
            std::process::exit(1);
        }
    }

    // Now convert the generated .ly file to Strudel HTML
    let ly_path = &output_path;
    let base_dir = ly_path.parent().unwrap_or(Path::new("."));
    let stem = ly_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let raw_ly = match fs::read_to_string(ly_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {e}", ly_path.display());
            std::process::exit(1);
        }
    };

    let expanded = match expand_includes(&raw_ly, base_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error expanding includes: {e}");
            std::process::exit(1);
        }
    };

    let parser = LilyPondParser::new();
    match parser.parse(&expanded) {
        Ok(result) => {
            let pitched_count: usize = result.staves.iter()
                .filter_map(|s| s.events())
                .flat_map(|events| events.iter())
                .filter(|e| matches!(e, PitchedEvent::Note(_)))
                .count();
            let drum_count: usize = result.staves.iter()
                .filter_map(|s| s.drum_voices())
                .flat_map(|voices| voices.iter())
                .flat_map(|voice| voice.events.iter())
                .filter(|e| matches!(e, DrumEvent::Hit(_)))
                .count();
            eprintln!(
                "Parsed {} staves ({} notes, {} drum hits)",
                result.staves.len(), pitched_count, drum_count
            );
            eprintln!("Tempo: {} = {} BPM", result.tempo.beat_unit, result.tempo.bpm);

            let html_path = ly_path.with_extension("html");
            let html = StrudelGenerator::generate_html(&result.staves, Some(&result.tempo), stem);
            match fs::write(&html_path, &html) {
                Ok(_) => println!("{}", html_path.display()),
                Err(e) => {
                    eprintln!("Error writing {}: {e}", html_path.display());
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
