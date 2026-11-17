# Wave D — the X-Y pad and the formula controller (steps 1–3)

Branch `fx/wave-d` from `fx/integration` 82b55e97. Provisional numbers: **schema 67**
(64→65 and 65→66 are empty steps here, marked as waves B's and C's, filled at the merge),
**ADR 0147** (the X-Y pad), **ADR 0148** (the formula controller).

## The architecture (ADRs 0147 §1, 0148 §1)

Both controllers are **modulator sources** (ADR 0057), not effect kinds:
`ModSource::XyPad`, `ModSource::Formula`. They inherit routes into every target (hosted
plug-ins, sends, note effects, another modulator's amount), *Modulate…*, the handle drop,
depth, rings, the freeze guards, bake, templates and the engine's control grid. What is
new and shared:

- **Inputs are parameters**: `AutoTarget::ModInput { modulator, input }` (the pad's X/Y,
  the formula's a/b/c) with `Command::SetModulatorInput` — so recording automation writes
  a pad gesture as **two lanes**, MIDI learn maps an axis or a knob, a clip replays it,
  another modulator drives it (one level).
- **Links**: `RouteLink { target, output, range }` kept on the controller's source by
  target — which axis a route reads and FL's link range (from, to, bent by the automation's
  curve). `ModRoute` is untouched (88 literals across the workspace stay as they are).
- **The formula language** (`modulator::formula`): FL's syntax, compiled off the audio
  thread into a postfix program run on a fixed stack; at most 256 steps, 32 deep, 48
  nested, 512 characters; `cantus_dsp::math` only; output 0…1 (FL's) or bipolar.
- **Inertia**: two equal one-pole stages in closed form, stepped at each control point over
  the exact interval (16-sample grid accurate; every grid point a control point while a pad
  with inertia plays).

## Step 1 — contract, model, engine (numbers)

- `cantus-core` **827** tests green (808 before): `modulator::formula::tests` 9 (every FL
  manual example formula: 24 waves and shapes, verbatim, against the platform libm to
  1e-6; the boolean table; refusals and where), `modulator::fxd_tests` 10,
  `controller_targets` with `ModInput` as the 17th kind, the undo fuzz's own controller
  stream (`drng`, eight kinds; two always refused). **The fuzz found a real bug**: two
  controllers driving each other's inputs recursed without end in the freeze region's
  walk (stack overflow) — fixed (one level, as amounts), and a freeze region now takes in
  what drives a controller's input.
- `cantus-project` **194** green: schema 67, the v67 fixture (a pad "Sweep" and a formula
  with an automated knob on the master), `schema_67_keeps_the_pad_the_formula_and_their_input_lanes`.
- `cantus-engine`: `tests/fxd_controllers.rs` 6 (nulls to the bit — unlinked, at rest
  with inertia on and a symmetric range, a formula of 0 and one with knobs at 0; heard and
  block-size independent 1…8 192 at tolerance 0; live at irregular blocks = offline; a
  synced formula and a path from bar 5 = the full render there; inertia glides where its
  absence jumps; a listen checkpoint's forked processor carries the puck to the bit) and
  `rt_memory::controllers_never_allocate`. Whole engine suite **1 289** green.
- Words: `locales/fx/controllers.{en,es}.txt` (`cmd.fxd_set_input`, the kinds, the inputs,
  every formula error, the faces' words).

## Step 2 — design sheets v1

`compartir/xy/design/v1` and `compartir/formula/design/v1`, each with its `index.md`.
Shot with `cargo run -p cantus-ui --example shoot_fxd -- <dir>` (the clip-aware probe on
every shot); `tests/fxd_whole.rs` holds every window, language, scale and colourway to no
cut word, and a longer language to a taller window, never a smaller display.

Verified: `cantus-ui` lib 546 green (the side round's house-rule tests included: the
first palettes clashed with Cribrum, Fractus, the EQ, Caelum Aurora and Vetus Walnut and
were moved), `fxd_whole` 2, `fx_lint`, `kit_lint`, `kit_alloc`; clippy `-D warnings`
`--all-targets` clean on core, engine, project, ai and ui; 34 shots, no cut word.

## For the owner

- The names (proposals): **Tactus** (the X-Y pad, Latin *touch*) and **Calculus** (the
  formula controller, Latin *reckoning stone*).
- The sheets, the three colourways each (Titanium/Copper/Frost; Bakelite/Ivory/Ebony).
- The formula's changed names, so a render repeats: `Time` (ms since play), `Rand`
  (a hash of the position), `SeededRand` (our hash); refused: `MouseX`, `MouseY`, `Date`,
  `Tension`, `Warp`.

## Left for step 3 (after the owner's verdict) — done in step 3, below

The windows opened from the modulator's row, the pad's drag issuing `SetModulatorInput`
(merged per gesture) with recording, drawing and recording a path, the formula's typing
(draft, caret, keypad), the engine's telemetry of the puck (both axes) for the face, bake
of a pad with inertia (trace), presets, MCP/AI words for the two kinds and `ModInput`,
manuals, an e2e through `CantusApp::assemble`.

## Step 3 — the windows, presets, bake, the agents, the e2e

The owner's verdicts (`compartir/{xy,formula}/verdict.md`, 2026-11-17): both sheets
**approved**, the names **Tactus** and **Calculus**, the pad's polarity default *Both ways*,
the formula's rules for a repeatable render as proposed. His notes on the pad (the Spanish
cards stretched with empty space; *What it drives* left a band under its rows) are fixed.
ADRs 0147 and 0148 have a *Phase B (step 3)* section each; the manual has a section for
each controller in both languages (§5, after *Modulators*), and the AI page's *Movement*
row names them.

Commits: `10f0a3ed` + `92eb2d65` (AI), `0367f73a` (core: presets, `tidy_links`, bake),
`badab3aa` (engine: telemetry, `trace_route`; the UI's bake hooks), `635f3f22` +
`da3c0341` + `ebbb1ca4` (UI: the windows, the pointer tests, the shots), `95378678`
(recording over a playing path), `aad25a3f` (the e2e), the links' axis and range edited in
*What it drives* (the commit after these docs), and these docs.

What was built (files):

- **The windows** — `crates/cantus-ui/src/views/fx/fxd_window.rs` (the floating device
  window: opened from the modulator's row, *Window · Open* or a double-click on its
  picture; moved, closed by its key or `Esc`, minimised, maximised, scaled; one at a time;
  closed when its modulator goes), `fxd_edit.rs` (every gesture one command, one undo
  step), `fxd_xy.rs` and `fxd_formula.rs` (the faces), `fxd_art.rs` (the cards: a row's
  slack shared by item), `modulator_rows.rs` (the *Open* key). The pad's drag is one
  `Batch` of two `SetModulatorInput` a frame (one step a drag; **two lanes, X and Y**, when
  a pass writes); *Draw path* (≤ 64 points over the loop) and *Record path* (while the
  song plays, sampled at song beats, laid over the loop it began in, 16 points a beat;
  recording over a playing path switches it off for the drag, in the same step); the
  formula typed at a caret on its display (a draft apart from the project; each edit that
  compiles committed, one step a session; a broken draft shows its error at its character
  and never reaches the audio; `Esc` drops it; a long formula shrinks to 60 % then scrolls
  to the caret); the keypad (24 keys) and the a/b/c dials (vertical drag, double-click
  0.5); presets in the header (arrows, the list with descriptions, *Custom*); *What it
  drives* — the X/Y tag swaps a route's axis, a bar's ends set its range, its middle bends
  it, a double-click gives the whole travel back; sized to its rows, scrolls by the wheel.
- **Core** — `crates/cantus-core/src/modulator_presets.rs` (+ `_tests.rs`): four Tactus
  presets (*Filter sweep*, *Width and pan*, *Figure eight*, *Performance pad*), nine
  Calculus presets in FL's syntax on one knob plan (a the rate, `2^(Round(a·6)−4)` cycles a
  beat; b the shape; c the depth); `apply`, `matching` (look, links and inputs aside),
  `by_id`, `of_kind`. `Modulator::tidy_links` wherever a route goes.
  `modulator_bake.rs`: a controller baked as the engine plays its route. Words:
  `crates/cantus-ui/locales/fx/controllers.{en,es}.txt` (`fxd.preset.<id>[.what]`, the
  window's words).
- **Engine** — `crates/cantus-engine/src/modulator.rs`: `ModTelemetry.pads` (puck x, y,
  where it heads x, y), `pad_mask`, `pad(id)`, no allocation; `trace_route` (a route's value
  through its axis and range × amount per control point) for the bake of a pad with
  inertia or a driven controller. `processor_auto.rs`: the puck reported.
- **AI** — `crates/cantus-ai/src/tools_mod.rs`, `tools_read.rs`, `tools_words.rs`,
  `tools_json.rs`, `tools_remove.rs`, `tools_project.rs`: `add_modulator` kinds `xy_pad` and
  `formula` with their settings, routes' `axis` and `range`, `change_routes`, inputs moved by
  `set_modulator` as `SetModulatorInput` (one step a call), a bad formula refused at its
  character, a removed route takes its link, `settings` and each route's axis and range
  read back, `get_mixer` lists strip and master modulators. **Bug fixed**: a `ModInput`
  target read as the modulator's amount (`target_of` ignored `param`; `words::target` used
  the amount's phrase) — now `{modulator_id, param: "x"|"y"|"a"|"b"|"c"}`, round-tripping,
  read "X-Y pad 1 · Y" / "Formula 1 · knob b". The tools' golden regenerated on purpose
  (`crates/cantus-ui/tests/golden/mcp-tools-list.json`; only `automate`,
  `replace_automation`, `add_modulator`, `set_modulator`, `remove_items` changed).
- **Docs** — `docs/adr/0147-the-x-y-pad.md`, `docs/adr/0148-the-formula-controller.md`
  (Phase B, Consequences, tests), `docs/manual/{en,es}/README.md` (*Tactus, the X-Y pad*;
  *Calculus, the formula controller*, with the formula language for users and worked
  examples), `docs/manual/{en,es}/ai-tools.md` (the *Movement* row).

Numbers (tests written in step 3):

- `cantus-ai` `tests/tools_fxd.rs` **5** (the crate 56 at the AI's commit).
- `cantus-core` `modulator::presets::tests` **9** (8 on the presets, 1 on `tidy_links`),
  `modulator_bake` **3** new.
- `cantus-engine` `tests/fxd_bake.rs` **5**: each baked curve within **0.002** of the
  trace at the control points, the baked render within **1 % of peak** of the modulated
  render, the route audible; the telemetry of a gliding puck.
- `cantus-ui` `tests/fxd_drag.rs` **16** (through `CantusApp::assemble`, real pointer and
  key events: 11 on the pad and the window, 5 on the formula), `tests/fxd_whole.rs` **2**
  (step 2's, on the new windows).
- `cantus-ui` `tests/fxd_e2e.rs` **6** (through `assemble` with the real engine wiring,
  `EngineHandle::manual`): a pad on two EQ bands moved, each heard **±12 dB** (the other
  within 0.1 dB), dry to the bit at rest and after undo; a recorded gesture writes exactly
  **two lanes** equal to the hand at all **96** touched ticks, its render block-size
  independent (64/333/4096) and live = offline at odd blocks 1…1 024, undone to the bit; a
  path (`xy.figure_eight`) followed within **0.5 dB** over 16 beats; `formula.square` live
  = offline to the bit over 6 bars, its **±15 dB** followed within **0.3 dB**; an edit to a
  formula heard (difference RMS 0.25) and undone to the bit, knob c halving the crest
  (**+14.80 → +7.46 dB**); unlinked controllers and depth-0 routes the dry master to the
  bit.

Suites: (integrator fills in)

**Verified only in emulation** (material for `docs/beta-checklist.md`): the windows were
driven by egui's events through `assemble`, never by a person's hand on a screen; the sound
was measured offline and through `EngineHandle::manual`'s processor, not through a real
driver; no real touch surface or XY controller moved a pad. Read in the code but not
tested: `Alt+A` on an axis or a knob (an automation clip for it after its card knob moved)
and `Alt+Shift+A` (a MIDI controller linked to it the same way) — the window's own
controls have no right-click menu for automation or MIDI learn.

Shots: `C:\projects\daw-fx\compartir\xy\design\v2-step3\` (25 PNG + `index.md`) and
`C:\projects\daw-fx\compartir\formula\design\v2-step3\` (23 PNG + `index.md`): both
languages, Slate and Paper, 75–200 %, every colourway, the hand, a drawn and a recorded
path, the links, the presets; the formula typing, its error, both polarities.

**Findings for elsewhere** (pre-existing, measured by the e2e, **not fixed**: fixing them
likely changes the goldens and the `DSP_VERSION`, so they are the integrator's and the
owner's):

1. A native effect unit starts a play or a seek at its **stored** value, not the automated
   or modulated one, and glides there over its de-zipper (~20 ms for Aequalis): a render
   from bar N differs from the full render on the first note for any automated native
   effect — plain automation too (a lane holding an EQ band at +9 dB differs by up to
   5.2e-2 from bar 3). Strip pan/gain and track gain are bit-exact from bar N.
2. Firmus starts its cutoff and gain smoothers at the factory defaults on a render that
   begins on a note (it snaps them only in idle blocks).

The e2e therefore proves render-from-bar-N on the strip's pan and fader, and on the EQ
from the first full beat.

**Also open** (small, for the integrator): the controllers' colourways (Titanium, Copper,
Frost; Bakelite, Ivory, Ebony) are in the model and the sheets, but the window has no menu
to choose one (the other units choose theirs in the ⋯ menu).

**Exported plug-ins: nothing.** Modulators (and so both controllers) live in the host —
Cantus's project and engine; the exported CLAP/VST3 effects carry no modulators, and the
controllers have no DSP of their own to export.

**The site** (checked read-only on `fx/web`): it has **no place for modulators or
controllers**. `web/src/units_data.rs` holds only `INSTRUMENTS` (9) and `EFFECTS` (42), and
`web/src/units.rs`' `group()` has those two tabs; no other page of `web/src` names
modulators (Fluctus is there as an effect). Nothing to add to a card. For the integrator,
if the site should sell them: a third tab (*Controllers*: the LFO, envelope, follower,
steps, random, Tactus and Calculus) in `units.rs`/`units_data.rs`, or a feature block on
the studio page. The manual's pages on the site (`web/assets/manual/`) are built from
`docs/manual/` and take the new sections when they are rebuilt.
