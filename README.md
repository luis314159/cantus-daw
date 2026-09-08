# Cantus

A free, open-source digital audio workstation written in Rust.

> *"Whatever you do, work at it with all your heart."* — Colossians 3:23

Cantus takes its name from the *cantus firmus*: the fixed melody on which polyphony is built.
The engine is that fixed line — deterministic, sample-accurate, allocation-free — and
everything else is built on top of it. The full vision lives in [idea.md](idea.md); the
decisions that shape the code live in [docs/adr/](docs/adr/); where the project stands right
now is in [docs/STATUS.md](docs/STATUS.md).

## What works today (MVP)

- Real-time audio on WASAPI / CoreAudio / ALSA through `cpal`, with an audio thread that
  never allocates, locks or panics (ADR 0005), instrumented for load and overruns.
- **Firmus**, a built-in 16-voice subtractive polysynth (2 PolyBLEP oscillators, SVF
  filter, two ADSRs, key tracking).
- Arrangement view (tracks, clips, mute/solo/gain/pan, loop region) and a painter-based
  piano roll (draw, move, resize, velocity, marquee, quantize, snap grid, zoom).
- Computer keyboard as a MIDI controller (`A`–`'` keys, `Z`/`X` octave) and **hardware MIDI
  input** (every port is opened at start).
- **MIDI recording** into the selected clip (press `R`): live take preview in the piano
  roll, loop-wrap aware, one undo step per take.
- **Insert effects** per track — a tempo-free stereo delay (ping-pong, tone) and an
  algorithmic reverb — plus a **mixer view** with faders, pan, peak-hold meters and master.
- One command bus for every mutation, with unlimited undo/redo, gesture merging and an
  audit log that records who did what — user, script, AI or MCP (ADR 0004).
- Versioned JSON project files with SHA-256 integrity, schema migrations and atomic,
  backup-preserving saves plus autosave (ADR 0006).
- Offline WAV export that is **bit-identical** to real-time playback, verified by null tests.
- English and Spanish interfaces (ADR 0007); every string is in `crates/cantus-ui/locales`.
- The AI layer skeleton: provider trait, tool catalogue, permissions and a command bridge,
  so an agent can only act through the same undoable commands as a mouse click.

## Build and run

```
cargo run --release -p cantus
cargo run --release -p cantus -- --stress        # 160 000-note stress project (ADR 0003)
cargo run --release -p cantus -- --locale es
cargo run --release -p cantus -- song.cantus
cargo run --release -p cantus -- --mixer --window 1200 760   # start in the mixer, fixed size
```

Requires stable Rust 1.85 or newer. No system libraries are needed on Windows or macOS;
on Linux install `libasound2-dev` (and `libxkbcommon-dev`, `libwayland-dev` for the window).

## Tests

```
cargo test --workspace                          # unit, property and null tests
cargo test -p cantus-engine -- --ignored        # real-device smoke test
cargo clippy --workspace --all-targets
```

The null tests in `crates/cantus-engine/tests/null_tests.rs` are the project's most
important guardrail: they fail if the engine ever produces different output for different
block sizes, or if a render stops matching playback.

## Layout

| Crate | License | Purpose |
|---|---|---|
| `cantus-core` | MIT OR Apache-2.0 | musical time, project model, commands, undo/redo, snapshots |
| `cantus-dsp` | MIT OR Apache-2.0 | oscillators, envelopes, filters, Firmus, null-test harness |
| `cantus-engine` | MIT OR Apache-2.0 | audio thread, transport, sequencer, cpal backend, offline render |
| `cantus-project` | MIT OR Apache-2.0 | project file format, migrations, atomic save |
| `cantus-ai` | GPL-3.0-or-later | `AiProvider` trait, tool specs, command bridge |
| `cantus-ui` | GPL-3.0-or-later | egui application |
| `cantus` | GPL-3.0-or-later | the binary |

See [ADR 0001](docs/adr/0001-license.md) for why the license is split.

## Keyboard

Space play/stop · R record · Home to start · Ctrl+Z/Y undo/redo · Ctrl+S save · Ctrl+O open ·
Ctrl+N new · Ctrl+E export WAV · Ctrl+L loop · Ctrl+M metronome · Delete · Ctrl+A ·
Q quantize · Ctrl+scroll zoom · Shift+scroll horizontal scroll.
