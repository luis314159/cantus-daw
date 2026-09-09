# Cantus — status and handoff

**Updated:** 2026-09-09 (round 10: light theme, configurable appearance) · **Phase:** 0/1 complete, Phase 2 (MIDI) and parts of 4 (mixer) and 8 (effects) done · **Tests:** 125 green + 1 real-device test (`--ignored`), clippy clean · **Code:** ~14 400 lines of Rust

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0011 | done |
| `cantus-core`: time, model, commands, bus, snapshots, **effects model** | done; 24 tests incl. 40-seed undo/redo invariant fuzz (now covering effect commands) |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, **stereo delay, Freeverb-style reverb**, null-test harness | done; 19 tests |
| `cantus-engine`: transport, processor with **insert chains**, **hardware MIDI in (midir)**, **recording feedback**, cpal backend, offline render | done; 11 null/behaviour tests + 4 transport/midi unit tests + 1 real-device test |
| `cantus-project`: versioned JSON (**schema 3**), SHA-256, migrations 0→1→2→3, atomic save, autosave, recovery, **fixture corpus** | done; 8 tests; fixtures `v1`–`v3.cantus` must always load |
| `cantus-ai`: `AiProvider`, tool catalogue (**+ add_effect**), permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport (**record**), arrangement, piano roll (**take preview, 7 mouse tools, clipboard, nudge/transpose, slice, mute**), **mixer view**, inspector (**effects rack**), keyboard piano, **MIDI recording**, **design system**, **named layouts**, **editable keymap**, **settings dialog**, i18n EN/ES | done; 49 tests (headless egui frames, edit/undo flows, save→open→export, recording take, effects delivery, type scale / spacing grid / bundled fonts / theme upgrade) |

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

## Appearance (round 10): white by default, and yours to change

**The default is now light.** `theme::paper_palette()` — white canvas, light grey panels,
ink text, the gold accent darkened until it is readable on white. Ink & Gold stays as the
dark preset, and Parchment as the warm light one; *Settings → Theme* switches between them.
An existing `theme.json` is left alone, so anyone already on a theme keeps it.

**Every colour is legible, and a test says so.** `every_shipped_theme_is_legible` computes
WCAG contrast for each preset: 4.5:1 for text and the accent, 3:1 for the focus ring, the
bright accent and the selection. Writing it immediately found two real defects in the
*existing* Parchment theme — its gold managed 2.88:1 against the panel, so every heading
was decorative rather than readable, and its dim text sat on exactly 4.50:1. Both fixed.

**The whole palette is editable in the app**, not just in the file. *Settings → Colours*
opens a window with a picker for each of the 22 tokens, grouped, applied as you drag and
written through to `theme.json` — which stays the source of truth and what `ThemeWatcher`
reloads. `Theme::tokens_mut` is what the dialog iterates, and a test asserts it exposes
every hex field the struct serializes, so a token cannot be added and silently left
unreachable.

**Interface size** is a setting too (`settings.ui_scale`, 70–200 %), applied through
egui's zoom factor so the spacing grid scales with the type instead of only the text
growing. Clamped, NaN included, because it comes from a file people edit.

Two defects the white ground exposed, both fixed:

- The piano roll dimmed everything past the end of the clip with a **black scrim**. On ink
  it was invisible; on white it was a heavy grey slab covering half the view. It now paints
  the panel colour — one step of the elevation scale, right in any theme.
- **Selected notes were painted white**, which was my own regression from the mesh
  batching: unmistakable on ink, invisible on paper. Selection now uses the palette's
  `selection` colour with a `text`-coloured outline, and the contrast test covers it.

**Faders**: egui defaults to a chunky rectangle on a thick rail — the "old Windows" look.
They are now a slim cap (`HandleShape::Rect { aspect_ratio: 0.38 }`) on a 4-px groove with
a defined edge. **Known limit:** egui paints a slider's groove and its cap from the same
`inactive.bg_fill`, so at rest they can only differ by that edge. A cap with its own fill,
a centre line and dB ticks needs a widget of our own — the next thing to do for the mixer.

## Piano roll performance (round 9) — and why we stayed on egui

Prompted by "would Slint be better?", ADR 0011. The honest answer needed a number, and the
number did not exist: the stress test ran at a zoom where culling left **139 notes** on
screen, so ADR 0003's "60 fps with 20 000 visible notes" was never being checked.

Measured properly, with tessellation included:

| | Frame | Triangles | Per note |
|---|---|---|---|
| Before | **74.7 ms** (13 fps) | 416 772 | 20.8 |
| After | **6.9 ms** (146 fps) | 96 772 | 4.8 |

Two causes, both ours. Notes were drawn as rounded rectangles with a stroked outline at
every zoom, and epaint anti-aliases every filled shape into a feathered ring — about twenty
triangles for a rectangle. About forty lines fixed it: `NoteLod` decides what a note is
worth at its size on screen, and anything below the thresholds joins one `egui::Mesh` at
two triangles each. Tessellation fell from 60.4 ms to 2.9 ms. At editing zoom nothing
changed visually — verified by screenshot.

**Convention this establishes:** anything drawn once per note, clip, sample or automation
point goes into a `Mesh` and gets a level-of-detail rule. Not an optimisation to remember —
a rule, because the convenience painters cost ~10x per item.

**What the speed bought:** the piano roll's zoom-out limit was 4 px/beat — about 80 bars —
and it was there because drawing more was ruinous. It is now 0.5 px/beat
(`MIN_PX_PER_BEAT`), so a 1 300-px grid shows some 650 bars and a whole piece fits on
screen. That needed the ruler to thin its bar numbers out, which it now does by doubling
(every bar, every 2, every 4 …) so the numbers shown stay ones a musician counts in, never
every 7th; a test asserts it. Verified by running: the 20 000-note stress project at the
widest zoom draws 6 540 notes in a 6.8 ms frame in a *debug* build, ruler legible, zero
audio overruns.

Known refinement: at that zoom the velocity lane's per-note bars overlap into a solid
block. Truthful about density, but it could show the peak per column instead.

Note on verification: driving the running app with synthetic clicks is unreliable on
Windows — twice now a stray event has landed in the app during a screenshot (once dragging
the autosave slider, once switching theme and selecting every note). When a capture
disagrees with the code, check `settings.json` / `theme.json` before believing the
screenshot, and prefer the headless render tests, which assert on the text actually
painted.

Also fixed: `looping_wraps_and_keeps_playing` asserted `stats().overruns == 0`, which
compares wall clock against the block budget. On a loaded machine a 1 000-block run
exceeds it and the test fails, in an otherwise fully deterministic null test. Real-time
behaviour belongs in `tests/hardware.rs`; a flaky guardrail teaches people to re-run
instead of read.

## Settings (round 8)

`cantus-ui/src/settings.rs` and the *View → Settings…* dialog. Preferences — the ones a
dialog can hold in a handful of rows — live in `settings.json` beside the other user files:
audio device, sample rate, buffer size, language, autosave interval. The theme deliberately
stays in `theme.json`: it is a whole document the user edits by hand and the app reloads
live, not a row in a dialog.

- **Audio changes are applied on *Apply*, never as you drag.** Reopening the device on
  every frame of a combo box would stutter the audio.
- **A settings change can never leave the user in silence.** `EngineHandle::start` falls
  back to the system default when the saved device is not there (unplugged since last
  time), and if nothing can be opened at all, the engine already running is left alone.
  Either way the status line names the device actually in use, so a fallback is visible
  rather than mysterious — there is a test for exactly that.
- **The buffer size is shown in milliseconds** next to the frame count, because that is
  what a musician is actually choosing between.
- **The autosave interval is clamped on the way out** (10 s–10 min): a `settings.json`
  hand-edited to `0` must not turn autosave into a per-frame write of the whole project.
- **Language and theme moved out of the *View* menu** into Settings. Two places to change
  one thing is how they drift apart.
- A test asserts that *opening* the dialog changes nothing and writes no file. A
  preferences window that rewrites preferences by being looked at is a nasty bug, and every
  widget in it is bound to a value that could report "changed" on its first frame.
- `--settings` and `--shortcuts` open those dialogs at startup, next to `--mixer`. They
  exist so that looking at a view does not depend on driving menus, which synthetic clicks
  do not do reliably on Windows.

## The keymap (round 7)

`cantus-ui/src/keymap.rs` and ADR 0010. An `Action` is a thing the app can do, a `Binding`
is a key plus exactly those modifiers, and `keymap.toml` joins them — next to the theme and
the layouts, and written on first run so it is there to be found. *Help → Shortcuts* is now
a real editor: pick a preset, click a shortcut to clear it, click *Add* and press a key to
bind it, with conflicts flagged in red.

- **`CantusApp::run_action` is the single dispatch.** Shortcuts, menus and any future
  script go through one `match`, so a binding cannot reach something a menu cannot. The old
  `if cmd && z` chain in `global_shortcuts` is gone.
- **Modifiers match exactly**, or `Ctrl+Z` would fire on `Ctrl+Shift+Z`.
- **Presets are conflict-free by construction**: a preset is a shared spine plus its own
  bindings, and a later binding takes the shortcut from whoever held it. Tests assert that
  no shipped preset conflicts with itself and that none leaves an essential action unbound
  — that is how the Live preset's `Ctrl+E` (split) colliding with WAV export was caught
  before it shipped; export moved to `Ctrl+Shift+E` there.
- **FL / Live / Bitwig presets are approximations**, following each program's conventions
  for the actions Cantus has. The dialog says so. Only the Cantus preset is authoritative —
  claiming parity we have not verified would be worse than saying nothing.
- **A damaged file never stops the app**: unknown ids and unreadable shortcuts are logged
  and skipped, and something that is not TOML falls back to the default map.
- Views no longer read keys. The piano-roll tooltips ask `Keymap::display_for`, so they
  show whatever is bound today instead of a typed-in "P / 1". `Tool::shortcut()` and
  `Tool::key()` are gone; `Tool::action()` replaces them.
- Mouse and wheel modifiers are not bindings and are not in the file; they are listed
  below the scrolling list — not inside it, where a newcomer would never scroll far enough
  to find them.
- `cantus --shortcuts` opens the editor at startup, next to the existing `--mixer`. Both
  exist so a look at a view does not depend on driving menus.

## Named layouts (round 6)

`cantus-ui/src/layout.rs`. A `Layout` is the shape of the window — the inspector's width,
the arrangement's height, which of the two are showing, and which view fills the centre.
`Layouts` holds the current one plus the user's named ones, in
`%LOCALAPPDATA%/Cantus/layouts.json`. *View → Layout* switches between them, toggles the
panels, saves the current shape under a name, deletes one, or resets to the built-in shape.

Two things about egui 0.36 shaped the design, both found by experiment and both recorded in
tests so a future upgrade tells us if they change:

- **`Panel::default_size` cannot impose a size** once a panel has stored state, and eframe
  is built here **without its `persistence` feature**, so egui does not carry panel sizes
  across restarts at all. The layouts file is what makes sizes survive: the app captures the
  layout on exit and forces it on the first frame.
- **Applying a layout uses `Panel::exact_size` for exactly one frame** (`Layouts::forced` /
  `end_frame`). `exact_size` pins the size range to a point, so it wins whether or not the
  panel has state; the frame after, the panel is resizable again from there. There is no
  setter for `PanelState`, and forging egui's internal state would break the first time it
  changed shape.

A bare `egui::Context` does not keep panel sizes between frames the way eframe's does, so
the tests assert what a *forced* frame produces and leave "a dragged size stays put" to be
checked by running the app. That is noted at the top of the module so the next person does
not read the gap as an oversight.

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
3. ~~**Layout personalization**~~ **done (round 6)** for panels: named layouts, saved panel
   sizes, panel visibility and *Reset layout* — see "Named layouts" above. Still open from
   this step: per-view zoom presets and configurable clip colours (track colours are
   already editable in the inspector).
4. ~~**Keyboard map file**~~ **done (round 7)** — see "The keymap" above. The editor is a
   self-contained window today; the settings dialog will host it rather than redefine it.
5. ~~**Settings dialog**~~ **done (round 8)** — see "Settings" above. It hosts the keymap
   editor rather than redefining it.
6. **Piano roll polish** — scale highlighting + snap-to-scale, ghost notes, chord stamps,
   note properties popup, glue/legato, strum/arpeggiate, magic lasso.

## Known gaps / next steps (in order)

1. The design & personalization plan above — **next and last of it: piano roll polish**
   (scale highlighting and snap-to-scale, ghost notes from other clips, chord stamps, a
   note properties popup, glue/legato, strum, arpeggiate).
2. **Automation lanes** (parameter curves, sample-accurate) and a **tempo track**.
3. **Sends/buses**: the mixer has strips but no routing yet; add bus tracks + sends.
4. More native devices: EQ, compressor, chorus; a sampler instrument.
5. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
6. **AI providers** (Anthropic/OpenAI/Ollama over `reqwest`), disclosure dialog, MCP server.
7. Recording polish: count-in, punch in/out, overdub vs replace, sub-block MIDI timestamps.
8. Accessibility pass (semantic nodes for the painted views).
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
- No view reads keys. A view that wants a shortcut adds a `keymap::Action` and a
  `run_action` arm; a view that wants to *show* one asks `Keymap::display_for`. Adding an
  action also means an `action.<id>` string in both locales.
- Anything a test writes to disk must go through a `tempfile::tempdir()`. `CantusApp`'s
  `layouts_path`, `keymap_path` and `settings_path` are public so tests can redirect them;
  never let a test write to the real `%LOCALAPPDATA%/Cantus`.
- The locale is process-wide and tests run in parallel: a test must not leave it changed,
  or it races every test that asserts an English string.
- Bump `SCHEMA_VERSION`, append a migration and regenerate a fixture
  (`cargo run -p cantus-project --example write_fixture`) whenever `Project` changes shape.
- Screenshots on this machine: PrintWindow from a DPI-aware process (see session notes);
  pick the newest `cantus` process, never kill instances you did not start.
