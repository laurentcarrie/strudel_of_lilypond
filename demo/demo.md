# Demo

This demo shows how to convert LilyPond notation to Strudel live coding patterns.

## Files

- [demo.ly](./demo.ly) - LilyPond source file
- [demo.html](./demo.html) - Generated Strudel REPL

## Usage

Generate sheet music PDF with LilyPond:
```sh
lilypond demo.ly
```

Install the converter:
```sh
cargo install strudel-of-lilypond
```

Convert to Strudel:
```sh
strudel-of-lilypond demo.ly
```

This generates `demo.html` with an embedded Strudel REPL.