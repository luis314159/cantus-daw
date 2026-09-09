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
  piano roll with FL-style tools (`P` Draw, `B` Paint, `E` Select, `D` Delete, `T` Mute,
  `C` Slice, `Y` Play, or `1`–`7`), Shift/Ctrl/Alt modifiers, clipboard, duplicate,
  nudge/transpose, quantize.
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
- A design system (ADR 0009): bundled IBM Plex Sans/Mono so the UI looks the same
  everywhere and every number is monospaced, a 4-px spacing grid, panel elevation, and a
  focus ring on whatever holds keyboard focus. Colour is yours to change — the theme lives
  in an editable JSON file that the app reloads while it is running.
- **Named layouts**: size the panels how you like, hide the ones you are not using, save
  the shape under a name and switch between them from *View → Layout*. Panel sizes survive
  a restart.
- **A settings dialog** (*View → Settings…*): output device, sample rate and buffer size
  with the latency in milliseconds, MIDI ports, language, theme and how often to autosave.
- **Every shortcut is yours** (ADR 0010): `keymap.toml` holds them all, with presets
  modelled on FL Studio, Ableton Live and Bitwig, and an editor under *Help → Shortcuts*
  that flags conflicts.
- The AI layer skeleton: provider trait, tool catalogue, permissions and a command bridge,
  so an agent can only act through the same undoable commands as a mouse click.

## Build and run

```
cargo run --release -p cantus
cargo run --release -p cantus -- --stress        # 160 000-note stress project (ADR 0003)
cargo run --release -p cantus -- --locale es
cargo run --release -p cantus -- song.cantus
cargo run --release -p cantus -- --mixer --window 1200 760   # start in the mixer, fixed size
cargo run --release -p cantus -- --shortcuts                 # open the shortcuts editor
cargo run --release -p cantus -- --settings                  # open the settings dialog
```

Requires stable Rust 1.85 or newer. No system libraries are needed on Windows or macOS;
on Linux install `libasound2-dev` (and `libxkbcommon-dev`, `libwayland-dev` for the window).

## Making it yours

The theme is a JSON file — `%LOCALAPPDATA%\Cantus\theme.json` on Windows,
`$XDG_DATA_HOME/Cantus` or `~/.local/share/Cantus` elsewhere. Edit it in any editor and
Cantus reloads it within a second, no restart. *View → Theme* switches between the shipped
presets (Ink & Gold, Parchment) or opens the file. A theme written by an older version is
upgraded in place on load, so new colour tokens appear in your file ready to edit.

Type, spacing and elevation are **not** in that file: they are the product's design
system (`crates/cantus-ui/src/design.rs`), and ADR 0009 explains the split.

Preferences — audio device, sample rate, buffer size, language and the autosave interval —
live in `settings.json` and are edited from *View → Settings…*.

Keyboard shortcuts live next to it in `keymap.toml` — plain TOML with comments, written
on first run. *Help → Shortcuts* edits it: choose a preset, click a shortcut to clear it,
click *Add* and press the keys you want. Conflicts are shown in red. The presets for other
DAWs follow their conventions for the actions Cantus has; they are a starting point, not a
copy.

Window layouts live next to it in `layouts.json`. *View → Layout* saves the current shape
under a name, switches between saved shapes, toggles the arrangement and inspector panels,
and resets to the built-in shape.

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
| `cantus-ui` | GPL-3.0-or-later | egui application, theme and design system |
| `cantus` | GPL-3.0-or-later | the binary |

See [ADR 0001](docs/adr/0001-license.md) for why the license is split.

The bundled UI fonts (IBM Plex Sans and IBM Plex Mono, in
`crates/cantus-ui/assets/fonts/`) are licensed separately under the
[SIL Open Font License 1.1](crates/cantus-ui/assets/fonts/LICENSE-OFL.txt) and are shipped
unmodified — "IBM Plex" is a Reserved Font Name (ADR 0009).

## Keyboard

These are the defaults; every one of them is rebindable in `keymap.toml` or under
*Help → Shortcuts* (ADR 0010).

Space play/stop · R record · Home to start · Ctrl+Z/Y undo/redo · Ctrl+S save · Ctrl+O open ·
Ctrl+N new · Ctrl+E export WAV · Ctrl+L loop · Ctrl+M metronome · Delete · Ctrl+A ·
Q quantize · P/B/E/D/T/C/Y or 1–7 tools · Ctrl+B duplicate · Ctrl+C/X/V · Arrows nudge/transpose (Shift: bar/octave) ·
Ctrl+T typing keyboard.

The mouse is not rebindable: Alt while dragging ignores the grid, Shift locks the pitch,
Ctrl locks the time, Alt+wheel edits velocity, Ctrl+wheel zooms, Shift+wheel scrolls
sideways.
