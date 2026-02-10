use std::fs;
use std::path::{Path, PathBuf};

use argh::FromArgs;
use strudel_of_lilypond::sequencer::lilypond::{lilypond_of_sequence, strudel_of_sequence};
use strudel_of_lilypond::sequencer::model::BarSequence;

/// Generate LilyPond and Strudel HTML files from a YAML bar sequence
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

    // Write .ly file
    let output_path = Path::new(input_path).with_extension("ly");
    let output_dir = output_path.parent().unwrap_or(Path::new("."));

    let ly_output = match lilypond_of_sequence(&sequence, &libraries, output_dir) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match fs::write(&output_path, &ly_output) {
        Ok(_) => eprintln!("Wrote {}", output_path.display()),
        Err(e) => {
            eprintln!("Error writing {}: {e}", output_path.display());
            std::process::exit(1);
        }
    }

    // Generate Strudel HTML
    let stem = Path::new(input_path).file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let html = match strudel_of_sequence(&sequence, &libraries, stem) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Error generating Strudel: {e}");
            std::process::exit(1);
        }
    };

    let html_path = Path::new(input_path).with_extension("html");
    match fs::write(&html_path, &html) {
        Ok(_) => println!("{}", html_path.display()),
        Err(e) => {
            eprintln!("Error writing {}: {e}", html_path.display());
            std::process::exit(1);
        }
    }
}
