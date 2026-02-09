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

## Demo

See the [demo/](./demo/) directory for a complete example with:
- LilyPond source file with tab staff, drum staff, and multiple voices
- Generated Strudel REPL output
- Punchcard visualization, gain, and pan modifiers

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
- **Tempo (required)**: `\tempo 4 = 120` - must be present in input
- Variable definitions (`voice = { ... }`)
- Drum mode (`drums = \drummode { ... }`)
- Score blocks with simultaneous staves (`\score { << ... >> }`)
- Staff types: `\new Staff`, `\new TabStaff`, `\new DrumStaff`
- Drum voices: `\new DrumVoice` inside DrumStaff
- Repeat expansion (`\repeat unfold/percent N { ... }`) → Strudel `!N` syntax
- Bar grouping: each bar is wrapped in `[...]` brackets
- Multi-bar repeats include duration: `[[[bar1] [bar2]]!2]@4`
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

- **Tempo (required)**: `\tempo 4 = 120` - specifies beat unit and BPM
- Note names: `c d e f g a b`
- Accidentals: `is` (sharp), `es` (flat) - e.g., `cis` = C#, `des` = Db
- Octave: `'` raises octave, `,` lowers octave (middle C = `c'`)
- Duration: number after note (4 = quarter, 8 = eighth, 2 = half, 1 = whole)
- Rests: `r` → `~`, `r2` → `~ ~` (half rest = two quarter rests)
- Bar lines (`|`) define bar groupings in output
- Durations: whole=`@4`, half=`@2`, quarter=(none), eighth=`@0.5`, sixteenth=`@0.25`

## Strudel Modifiers

Add special comments inside a staff or voice to control Strudel output:

- `% @strudel-of-lilypond@ <color> punchcard` - Enable punchcard visualization with color
- `% @strudel-of-lilypond@ gain <value>` - Set gain/volume (supports patterns like `<0.5 1 1.5>`)
- `% @strudel-of-lilypond@ pan <value>` - Set stereo panning (supports patterns like `<0 .5 1>`)

```lilypond
\tempo 4 = 60

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
const tempo = 60;

$: note("[c4 d4 e4]")
.gain(2)
.pan(0.25)
.color("red")
._punchcard()
  .s("piano")
  .cpm(tempo/4/1)

$: stack(
  sound("[[bd bd]]!2")
  .pan("<0 .5 1>")
  .color("cyan")
  ._punchcard(),
  sound("[[hh@0.5 hh@0.5 hh@0.5 hh@0.5]]!2"),
)
  .cpm(tempo/4/2)
```

### Drum Name Mapping

LilyPond drum names are mapped to Strudel sound names:

| LilyPond | Strudel | Description |
|----------|---------|-------------|
| `sn`     | `sd`    | snare drum  |
| `ss`     | `rim`   | side stick  |
| `hhc`    | `hh`    | closed hi-hat |
| `hho`    | `oh`    | open hi-hat |
| `cymc`   | `cr`    | crash cymbal |
| `cymr`   | `rd`    | ride cymbal |
| `tomh`   | `ht`    | high tom    |
| `tomm`   | `mt`    | mid tom     |
| `toml`   | `lt`    | low tom     |

Other drum names (`bd`, `hh`, `cp`, `cb`, etc.) are passed through as-is.

### Bar Sequencer

A separate binary generates LilyPond and Strudel files from YAML sequence definitions that reference a pattern library.

```bash
strudel-of-lilypond-sequence seq1.yml --library demo
```

**Sequence file** (`seq1.yml`):
```yaml
tempo: 120
sequence:
  - description: "intro tick"
    item: !Single
      pattern_name: "library/count"
  - description: "repeat kick and snare"
    item: !RepeatBar
      - 4
      - pattern_name: "library/pattern1"
  - description: "two kick only bars"
    item: !Group
      - !Single
        pattern_name: "library/pattern2"
      - !Single
        pattern_name: "library/pattern2"
```

**Pattern file** (`library/pattern1.yml`):
```yaml
description: kick and snare
voices:
  - bd4 sn4 bd4 sn4
  - hh8 hh8 hh8 hh8 hh8 hh8 hh8 hh8
```

Sequence items support:
- `!Single` - a single bar from a pattern
- `!RepeatBar` - repeat a pattern N times (`\repeat volta N` in LilyPond, `!N` in Strudel)
- `!Group` - a group of bars played in sequence
- `!RepeatGroup` - repeat a group of bars N times

Each pattern can have a variable number of voices, which map to `\new DrumVoice` blocks in LilyPond (`\voiceOne`, `\voiceTwo`, etc.).

### Output Format

- Each bar is wrapped in `[...]` brackets
- Repeats use `!N` syntax: `[[bar content]]!3`
- Multi-bar repeats include total duration: `[[[bar1] [bar2]]!2]@4`
- CPM is calculated as `tempo/4/number_of_bars`
- The `tempo` constant is defined at the top of the generated code
