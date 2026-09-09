# Cantus — status and handoff

**Updated:** 2026-09-08 (round 5: design system) · **Phase:** 0/1 complete, Phase 2 (MIDI) and parts of 4 (mixer) and 8 (effects) done · **Tests:** 91 green + 1 real-device test (`--ignored`), clippy clean · **Code:** ~14 400 lines of Rust

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0009 | done |
| `cantus-core`: time, model, commands, bus, snapshots, **effects model** | done; 24 tests incl. 40-seed undo/redo invariant fuzz (now covering effect commands) |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, **stereo delay, Freeverb-style reverb**, null-test harness | done; 19 tests |
| `cantus-engine`: transport, processor with **insert chains**, **hardware MIDI in (midir)**, **recording feedback**, cpal backend, offline render | done; 11 null/behaviour tests + 4 transport/midi unit tests + 1 real-device test |
| `cantus-project`: versioned JSON (**schema 3**), SHA-256, migrations 0→1→2→3, atomic save, autosave, recovery, **fixture corpus** | done; 8 tests; fixtures `v1`–`v3.cantus` must always load |
| `cantus-ai`: `AiProvider`, tool catalogue (**+ add_effect**), permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport (**record**), arrangement, piano roll (**take preview, 7 mouse tools, clipboard, nudge/transpose, slice, mute**), **mixer view**, inspector (**effects rack**), keyboard piano, **MIDI recording**, **design system**, i18n EN/ES | done; 22 tests (headless egui frames, edit/undo flows, save→open→export, recording take, effects delivery, type scale / spacing grid / bundled fonts / theme upgrade) |

## Verified by running (Windows 11, Realtek WASAPI 48 kHz)

- Window opens maximized (or `--window W H`), audio device opens, MIDI ports are enumerated
  (`MIDI inputs: n` in the status bar; none were attached on the dev machine).
- Offline render is bit-identical to real-time processing for block sizes 1…8192, with
  and without effect chains.
- Real-device test: 200 blocks, 0 overruns, max callback 194 µs at 480-frame blocks.
- Fixture `v2.cantus` opens with a Delay on the Keys track; the effects rack edits it live.

## The design system (round 5)

`cantus-ui/src/design.rs` owns **form**; `theme.rs` owns **colour** (ADR 0009). The split
matters: colour is user data in `theme.json`, form is a product decision in code.

- **Typography.** IBM Plex Sans + IBM Plex Mono are bundled in
  `crates/cantus-ui/assets/fonts/` (SIL OFL 1.1, licence file alongside them; the faces are
  shipped unmodified because "IBM Plex" is a Reserved Font Name). Views ask for named text
  styles — `design::caption/strong/mono_small/readout/readout_small` — never a raw size.
  **Every number in the UI is monospaced** (`design::paint::ruler` for canvas text) so bar
  counters, dB values and timers do not shift horizontally as they count.
- **The 4-px grid.** `design::space::{S1,S2,S3,S4,S6,S8,ROW,ROW_DENSE}` plus `snap()`. A
  unit test fails if the installed spacing lands off the grid.
- **Elevation.** `design::Elevation::{Canvas,Panel,Card}` returns the `egui::Frame` for a
  surface. Panels are separated from the canvas by a hairline pulled flush with the panel
  edge (a −1 px outer margin), not by a drop shadow — shadows smear the dense grids. Only
  windows and menus cast one (`design::overlay_shadow`).
- **Focus rings.** egui returns the *active* visuals for a focused widget, so keyboard
  focus was invisible. `design::paint_focus_ring` runs once at the end of every frame,
  reads the focused id from egui memory and strokes a ring on a foreground layer — one
  implementation covering every widget in the app, with no call-site changes.
- **Interaction states.** `icons::button` paints rest / hover / press distinctly, and
  reserves the accent fill for *state* (playing, recording, tool selected), never for a
  press. Track colour swatches and mixer strip headers follow the same rule.

Two layout bugs the pass exposed and fixed: the transport and the status bar both let
left-hand content overrun the right-hand readouts at 1200 px. Both now nest a
`left_to_right` layout inside a `right_to_left` one, so the readouts claim their width
first and the flexible content truncates. The Loop / Metronome / Keys text toggles became
icon buttons, which is what freed the space.

## Piano roll tools (round 3)

Bug fixed: drags hit-test the **press origin**, not the current pointer, so resizing from a
note's right edge no longer turns into a marquee. Tools use FL's letters — `P` Draw, `B`
Paint, `E` Select, `D` Delete, `T` Mute, `C` Slice, `Y` Play — and also `1`–`7`. The typing
keyboard piano starts **off** (as in FL) because it claims the letter keys; `Ctrl+T` or the
*Keys* toggle in the transport turns it on, and then only the number keys switch tools. Modifiers follow FL:
Shift locks pitch, Ctrl locks time, Alt bypasses snap, right-click deletes, Ctrl+drag
selects, Alt+wheel edits velocity. Shortcuts: Ctrl+B/Ctrl+D duplicate, Ctrl+C/X/V, arrows
nudge (Shift: bar) and transpose (Shift: octave), Esc clears, Ctrl+T toggles the typing
piano. `Note.muted` is schema 3 (fixture `v3.cantus`). All operations live on `CantusApp`
(`slice_at`, `toggle_mute_notes`, `duplicate_selection`, …) so they are unit-tested without
a window.

## How the new pieces fit

- **Effects** (`cantus_core::effect`): a generic `kind + key→f32` parameter model. The
  audio thread never allocates: `EngineHandle::set_snapshot` builds `EffectUnit`s on the UI
  thread for ids the engine has not seen and sends `AddEffect` before the snapshot; units
  dropped by a snapshot come back as `GarbageEffect`. `Processor::rebuild_chain` re-orders
  a track's chain with pre-sized Vecs only.
- **Recording** (`cantus_ui::recording::Recorder`): the engine stamps live notes
  (`EngineEvent::NoteInput`) with the transport tick while `SetRecording(true)`; the UI
  pairs on/off, handles loop wrap and retriggers, and commits each take as **one undo step**.
  `CommandBus` gained *composing* gesture merges for accumulating gestures
  (`Command::supersedes` decides replace-vs-compose).
- **Hardware MIDI** (`cantus_engine::midi`): every input port is opened at start; callbacks
  push into an SPSC ring the audio thread drains each block. The selected track is the
  live track (`SetLiveTrack`).
- Timing note: live notes are stamped at block start (≤ 10 ms at 480 frames). Sub-block
  timestamps from `midir`/cpal are a later refinement.

## Design & personalization plan (requested 2026-09-08)

Goal: a look that says "best DAW in the world" and a UI the user can make their own.
Ordered plan, each step shippable on its own:

1. ~~**Theme tokens as data**~~ **done (round 4)**: `theme::Theme` is serialized to
   `%LOCALAPPDATA%/Cantus/theme.json` (created from "Ink & Gold" on first run), reloaded
   live by `ThemeWatcher`; presets Ink & Gold (dark) and Parchment (light) under
   *View → Theme*, plus "Open theme file". Views read colours via `theme::p()` (a copied
   `Palette`), never constants. `icons.rs` paints all toolbar/transport icons as shapes;
   the piano roll tools are icon buttons with a tooltip "Name (letter / number)" and the
   long hint moved to the status bar.
2. ~~**Design pass**~~ **done (round 5)**: see "The design system" above — bundled IBM
   Plex Sans/Mono, the 4-px grid, `Elevation`, a global focus ring, distinct hover/active
   states, and three more painted icons (Loop, Metronome, Keyboard). ADR 0009 records the
   typography and licensing decision.
3. **Layout personalization** — dockable/resizable panels with saved layouts (egui already
   persists panel sizes; add named layouts and a "reset layout" action), per-view zoom
   presets, configurable track colours and clip colours.
4. **Keyboard map file** — every shortcut in a `keymap.toml` (FL / Live / Bitwig presets),
   editable in a Settings dialog with conflict detection.
5. **Settings dialog** — audio device / sample rate / buffer, MIDI ports, theme, language,
   keymap, autosave interval.
6. **Piano roll polish** — scale highlighting + snap-to-scale, ghost notes, chord stamps,
   note properties popup, glue/legato, strum/arpeggiate, magic lasso.

## Known gaps / next steps (in order)

1. The design & personalization plan above — **next: layouts** (named, saved panel sizes
   and a "reset layout" action), then keymap → settings dialog → piano roll polish.
2. **Automation lanes** (parameter curves, sample-accurate) and a **tempo track**.
3. **Sends/buses**: the mixer has strips but no routing yet; add bus tracks + sends.
4. More native devices: EQ, compressor, chorus; a sampler instrument.
5. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
6. **AI providers** (Anthropic/OpenAI/Ollama over `reqwest`), disclosure dialog, MCP server.
7. Recording polish: count-in, punch in/out, overdub vs replace, sub-block MIDI timestamps.
8. Accessibility pass (semantic nodes for the painted views), audio settings dialog
   (device / sample rate / buffer size — `AudioConfig` already supports it).
9. `docs/adr/0001`: trademark search for the name before any public release.
10. Light-theme gap the design pass surfaced: the piano roll dims the region outside the
    clip with a dark scrim, which is subtle on Ink & Gold but heavy on Parchment. Fold the
    fix into the piano roll polish step (dim by blending toward `ink`, not by overlaying
    black).

## Conventions

- Every mutation is a `Command`; never touch `Project` fields directly from the UI.
- Audio-thread code lives only in `cantus-engine::processor` and `cantus-dsp`; both are
  `#![forbid(unsafe_code)]` and must not allocate after construction.
- Add a null test before adding a DSP module; effects must be block-size independent.
- New UI strings go in `crates/cantus-ui/locales/en.txt` **and** `es.txt`; a unit test
  fails otherwise. The bundled fonts lack many symbol glyphs: paint icons or use words.
- Spacing comes from `design::space`, type from the named text styles, surfaces from
  `design::Elevation` — never a bare `12.0` or `.size(15.0)` at a call site. Numbers are
  monospaced.
- egui **panics** on a text style it cannot resolve, so any `egui::Context` that renders a
  view must have had `design::install_style` called on it (the test harness has a `ctx()`
  helper that does what `CantusApp::new` does).
- A new colour token goes in `Palette`, in `Theme` with `#[serde(default)]`, **and** in
  `Theme::fill_missing`, so older `theme.json` files keep loading and get the new token
  written back for the user to edit.
- Bump `SCHEMA_VERSION`, append a migration and regenerate a fixture
  (`cargo run -p cantus-project --example write_fixture`) whenever `Project` changes shape.
- Screenshots on this machine: PrintWindow from a DPI-aware process (see session notes);
  pick the newest `cantus` process, never kill instances you did not start.
