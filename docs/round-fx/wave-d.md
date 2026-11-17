# Wave D — the X-Y pad and the formula controller (steps 1–2)

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

## Left for step 3 (after the owner's verdict)

The windows opened from the modulator's row, the pad's drag issuing `SetModulatorInput`
(merged per gesture) with recording, drawing and recording a path, the formula's typing
(draft, caret, keypad), the engine's telemetry of the puck (both axes) for the face, bake
of a pad with inertia (trace), presets, MCP/AI words for the two kinds and `ModInput`,
manuals, an e2e through `CantusApp::assemble`.
