# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust tool that converts LilyPond music notation to Strudel live coding patterns. LilyPond is a music engraving program that uses text-based notation; Strudel is a JavaScript library for live coding music.

## Build and Test Commands

```bash
cargo build          # Build the project
cargo run            # Run the converter with example input
cargo test           # Run all tests
cargo test <name>    # Run a specific test (e.g., cargo test test_parse_simple_notes)
```

## Architecture

The codebase is a single-file Rust application (`src/main.rs`) with three main components:

1. **`Note` struct** - Represents a parsed music note with name, octave, accidental (sharp/flat), duration, and MIDI number

2. **`LilyPondParser`** - Parses LilyPond notation:
   - Extracts content between `{` `}` braces
   - Tokenizes on whitespace
   - Parses individual notes including accidentals (`is` = sharp, `es` = flat), octave markers (`'` = up, `,` = down), and duration numbers

3. **`StrudelGenerator`** - Generates Strudel code:
   - `generate()` - Basic output with note sequence
   - `generate_with_durations()` - Includes duration information

## LilyPond Notation Quick Reference

- Note names: `c d e f g a b`
- Accidentals: `is` (sharp), `es` (flat) - e.g., `cis` = C#, `des` = Db
- Octave: `'` raises octave, `,` lowers octave (middle C = `c'`)
- Duration: number after note (4 = quarter, 8 = eighth, 2 = half, 1 = whole)
- Rests (`r`) and bar lines (`|`) are skipped
