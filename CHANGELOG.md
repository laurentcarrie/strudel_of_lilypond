# Changelog

## [0.2.0] - 2026-01-17

### Added
- Per-voice punchcard visualization with `% @lilypond-to-strudel@ <color> punchcard` comments
- Strudel `*N` repeat syntax for LilyPond `\repeat unfold/percent N` instructions
- Chord support (`<c e g>4` → `[c4,e4,g4]`)
- Explicit `strudel-of-lilypond` binary target in Cargo.toml
- Documentation for punchcard visualization in README

### Changed
- `DrumVoiceData` struct now holds events and optional punchcard color
- `StaffContent::Drums` uses `Vec<DrumVoiceData>` instead of `Vec<Vec<DrumEvent>>`
- Renamed `drum_events()` to `drum_voices()` on `Staff`
- Repeats are no longer expanded; they use Strudel's native `*N` syntax

## [0.1.0] - 2026-01-16

### Added
- Initial release
- LilyPond parser supporting notes, accidentals, octaves, durations
- Drum mode parsing (`\drummode`)
- Multi-staff score parsing (`\new Staff`, `\new TabStaff`, `\new DrumStaff`)
- Multi-voice drum staff support (`\new DrumVoice`)
- Tempo parsing (`\tempo`)
- Variable definitions and references
- Strudel code generation with `note()`, `sound()`, `stack()`
- HTML output with embedded Strudel REPL
- CPM (cycles per minute) calculation from tempo
