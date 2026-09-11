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
- **Pulsus**, a synthesized drum kit (ADR 0013): a kick with a real pitch envelope, a
  click and soft drive; a snare with two tuned bodies and separately-decaying noise; the
  808's six-square hi-hat through a band-pass and a high-pass; and toms and a rim as
  presets of the same shapes. Seven pads on the General MIDI map, each with its own level,
  pan and **choke group** — the open hat is cut by the closed one — every parameter
  editable in the inspector, and null-tested like everything else.
- **A channel rack** (`F6`) in FL's shape: every track is a channel with its own mute,
  level, pan and mixer strip, a kit expands into its pads, and steps run across the bars of
  the selected clip on *every* track — a step on a channel with no clip there creates one.
  A step *is* a note: there is no second data model, so switching to the piano roll
  mid-edit converts and loses nothing, and every step undoes. Left-click on, right-click
  off, drag to paint, Alt+wheel for velocity. Any track can become a kit from the
  inspector, pads can be added and removed, and a pad can play a **WAV** instead of a
  synthesized voice (*Load…*), retuned, choked and mixed like the others.
- **Opens on a groove.** A new session starts with keys, kick, snare and hi-hat — each drum
  its own track, so it has its own strip — playing four bars in a loop, so the first thing
  that happens is music. On a kit the piano roll's lanes are the pads, named and sized to
  fill the grid.
- Arrangement view (tracks, clips, mute/solo, loop region) with **40-px tracks that have
  their own height** (drag the divider or pick a preset), a painted icon per track, drag to
  reorder, `Ctrl+↑/↓` to change track, and the fader / pan / width / polarity in the
  inspector where there is room for them. **Time selection**: drag in the ruler to select
  a stretch of the song across every track; the top strip of the ruler sets the loop.
  `Shift+L` loops the selection, `Z` zooms to it, and it is the anchor for what comes next
  (insert/delete space, ripple, quantize starts, render selection). Plus a painter-based
  piano roll that holds 60 fps with 20 000 notes on screen (ADR 0011) and zooms out far
  enough to see a whole piece, with FL-style tools (`P` Draw, `B` Paint, `E` Select, `D` Delete, `T` Mute,
  `C` Slice, `Y` Play, or `1`–`7`), Shift/Ctrl/Alt modifiers, clipboard, duplicate,
  nudge/transpose, quantize.
- **Clip editing**, built on a clip being a *window onto its content*: split (`Ctrl+E`) and
  glue (`Ctrl+J`), trim either edge without a note moving, slip the music inside the clip,
  repeat the content, drag between tracks, Alt-drag to copy, per-clip mute (`0`), colour
  (`Ctrl+Shift+C`) and name (`F2`), marquee and `Ctrl+click` multi-selection, and arrow-key
  nudge. Splitting a clip and gluing the halves gives back the clip, note for note. All of
  it is on a toolbar above the arrangement, in a **Clip** menu, and on the clip's own
  right-click menu — every entry showing the key bound to it.
- **Pan law, stereo width and polarity** (ADR 0012). The pan law — 0 / −3 / −4.5 / −6 dB —
  lives in the *project*, because it changes what the project renders as; mono is the
  width control's end stop rather than a switch of its own; and every channel and the
  master have a Ø. Projects written before this keep the law they were mixed with.
- Computer keyboard as a MIDI controller (`A`–`'` keys, `Z`/`X` octave) and **hardware MIDI
  input** (every port is opened at start).
- **MIDI recording** into the selected clip (press `R`): live take preview in the piano
  roll, loop-wrap aware, one undo step per take.
- **Insert effects** per track — a tempo-free stereo delay (ping-pong, tone) and an
  algorithmic reverb — plus a **mixer view** with console faders (a real cap, a read line
  and a dB scale, on a taper that gives 6 dB per tenth of travel through the useful range),
  pan and width bars that fill from their detent, peak-hold meters and a master strip.
- One command bus for every mutation, with unlimited undo/redo, gesture merging and an
  audit log that records who did what — user, script, AI or MCP (ADR 0004).
- Versioned JSON project files with SHA-256 integrity, schema migrations and atomic,
  backup-preserving saves plus autosave (ADR 0006).
- Offline WAV export that is **bit-identical** to real-time playback, verified by null tests.
- English and Spanish interfaces (ADR 0007); every string is in `crates/cantus-ui/locales`.
- A design system (ADR 0009): bundled IBM Plex Sans/Mono so the UI looks the same
  everywhere and every number is monospaced, a 4-px spacing grid, panel elevation, and a
  focus ring on whatever holds keyboard focus.
- **Make it look how you like.** Three themes — Paper (the light default), Parchment and
  Ink & Gold — a colour picker for all 22 palette tokens in *Settings → Colours*, and an
  interface size from 70 % to 200 %. Every shipped theme clears WCAG AA contrast, checked
  by a test. The theme is a JSON file the app reloads while running, so you can edit it by
  hand too.
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
cargo run --release -p cantus -- --rack --track 1            # start in the channel rack, on track 1
cargo run -p cantus-ui --example render_kit -- out/          # every drum and the welcome groove as WAV
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

Preferences — audio device, sample rate, buffer size, language, interface size and the
autosave interval — live in `settings.json` and are edited from *View → Settings…*.

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
block sizes, or if a render stops matching playback. They also hold the line that **a
control at its default must not touch a sample** — width at unity and polarity off have to
be free, not merely inaudible — and that a track summed with a polarity-inverted copy of
itself cancels to exact zero.

## Layout

| Crate | License | Purpose |
|---|---|---|
| `cantus-core` | MIT OR Apache-2.0 | musical time, project model, commands, undo/redo, snapshots |
| `cantus-dsp` | MIT OR Apache-2.0 | oscillators, envelopes, filters, Firmus, Pulsus, null-test harness |
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
Ctrl+↑/↓ change track · Shift+L loop the selection · Z zoom to it · F6/F7/F9 channel rack /
piano roll / mixer · Ctrl+T typing keyboard.

The mouse is not rebindable: Alt while dragging ignores the grid, Shift locks the pitch,
Ctrl locks the time, Alt+wheel edits velocity, Ctrl+wheel zooms, Shift+wheel scrolls
sideways.
