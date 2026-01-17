# strudel-of-lilypond

A Rust tool that converts LilyPond music notation to Strudel live coding patterns. LilyPond is a music engraving program that uses text-based notation; Strudel is a JavaScript library for live coding music.

## Installation

```bash
cargo install strudel-of-lilypond
```

## Usage

```bash
strudel-of-lilypond input.ly    # Creates input.html with embedded Strudel REPL
```

## Build from Source

```bash
cargo build          # Build the project
cargo run            # Run the converter with example input
cargo test           # Run all tests
cargo test <name>    # Run a specific test (e.g., cargo test test_parse_simple_notes)
```

## Architecture

The codebase is a Rust library (`src/lib.rs`) with a CLI frontend (`src/main.rs`).

### Data Structures

- **`Note`** - Pitched note with name, octave, accidental, duration, and MIDI number
- **`DrumHit`** - Drum sound with name (bd, hh, sn, etc.) and duration
- **`Staff`** - Either pitched (`Vec<PitchedEvent>`) or drums (`Vec<DrumVoiceData>` for simultaneous voices)
- **`Tempo`** - Beat unit and BPM from `\tempo` markings

### LilyPondParser

Parses LilyPond notation with support for:
- Variable definitions (`voice = { ... }`)
- Drum mode (`drums = \drummode { ... }`)
- Score blocks with simultaneous staves (`\score { << ... >> }`)
- Staff types: `\new Staff`, `\new TabStaff`, `\new DrumStaff`
- Drum voices: `\new DrumVoice` inside DrumStaff
- Repeat expansion (`\repeat unfold/percent N { ... }`) → Strudel `*N` syntax
- Notes with accidentals (`is`/`es`), octave markers (`'`/`,`), and durations
- Chords (`<c e g>4`) → Strudel `[c4,e4,g4]` syntax
- Punchcard visualization comments (see below)

### StrudelGenerator

Generates Strudel patterns:
- `generate_pitched_staff()` - `note("c4 d4 e4").s("piano")`
- `generate_drum_staff()` - `sound("bd hh sn hh")` or `stack()` for multiple voices
- `generate_multi()` - Multiple `$:` patterns for simultaneous staves
- `generate_html()` - HTML page with embedded Strudel REPL

## LilyPond Notation Quick Reference

- Note names: `c d e f g a b`
- Accidentals: `is` (sharp), `es` (flat) - e.g., `cis` = C#, `des` = Db
- Octave: `'` raises octave, `,` lowers octave (middle C = `c'`)
- Duration: number after note (4 = quarter, 8 = eighth, 2 = half, 1 = whole)
- Rests: `r` → `~`, `r2` → `~ ~` (half rest = two quarter rests)
- Bar lines (`|`) are skipped

## Strudel Modifiers

Add special comments inside a staff or voice to control Strudel output:

- `% @strudel-of-lilypond@ <color> punchcard` - Enable punchcard visualization with color
- `% @strudel-of-lilypond@ gain <value>` - Set gain/volume (supports patterns like `<0.5 1 1.5>`)
- `% @strudel-of-lilypond@ pan <value>` - Set stereo panning (supports patterns like `<0 .5 1>`)

```lilypond
\new TabStaff {
  % @strudel-of-lilypond@ red punchcard
  % @strudel-of-lilypond@ gain 2
  % @strudel-of-lilypond@ pan 0.25
  \voicea
}

\new DrumStaff {
  <<
    \new DrumVoice {
      % @strudel-of-lilypond@ cyan punchcard
      % @strudel-of-lilypond@ pan <0 .5 1>
      \kicks
    }
    \new DrumVoice {
      \hats
    }
  >>
}
```

This generates Strudel code with the specified modifiers:

```javascript
$: note("c4 d4 e4")
.gain(2)
.pan(0.25)
.color("red")
._punchcard()
  .s("piano")

$: stack(
  sound("bd bd")
  .color("cyan")
  ._punchcard(),
  sound("hh hh hh hh"),
)
```
