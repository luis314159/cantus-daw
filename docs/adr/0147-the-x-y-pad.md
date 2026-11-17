# ADR 0147 — The X-Y pad

**Status:** proposed (fx side round wave D, provisional number — renumbered at merge) ·
**Date:** 2026-11-17 · **Schema:** 67 (provisional; 65 and 66 are waves B's and C's, empty
steps on this branch) · **Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread),
ADR 0027 (automation), ADR 0057 (modulators on any parameter), ADR 0062 (MIDI learn),
ADR 0107 (listen and forks), ADR 0148 (the formula controller, the same architecture)

## Context

BACKLOG §3b's *X-Y controller* row: Fruity X-Y Controller and the XY pads of Kaoss-style
instruments — two (or more) parameters on one pad, inertia and paths, recordable, MIDI and
automation mapping, a gesture recorded as two automation lanes. FL's X-Y Controller is a
generator whose two outputs are *linked* to parameters (Link to controller: a range and a
mapping formula per link); Bitwig's XY modulator is a modulator whose outputs offset its
targets.

Cantus already has round 28's modulators (ADR 0057): a source with routes into any
`AutoTarget` (hosted plug-ins included), drawn in the inspector's Modulators section per
host (track, strip, master), *Modulate…* in every parameter's menu, a handle dropped on
any control, depth by Alt-drag, rings on modulated knobs, the engine's 16-sample control
grid, null rules, nesting into an amount, bake to automation, the undo fuzz.

## Decision

### 1. A modulator source, not an effect kind

The pad is **`ModSource::XyPad(XySource)`**, `ModKind::XyPad` — a sixth source of ADR
0057's modulators, living on a track, a strip or the master like the others. Not an
`EffectKind` with outputs: an insert has audio in and out and a place in a chain, and the
pad has neither; FL puts its X-Y on a channel only because FL's controllers must live
somewhere in the rack. As a modulator it inherits, unchanged and already tested: routes
into every target (natives, hosted plug-ins, sends, note effects, another modulator's
amount), *Modulate…* and the handle-drop to connect, the depth, rings and live dots, the
freeze guards, the AI's modulator tools, templates and bounce id remapping, the null rules
and the engine's grid. An FL user finds the same gesture: open the pad, link X to one
knob and Y to another.

### 2. Two outputs and FL's link range, kept by target on the source

Each route reads one axis and may carry a **range**: `RouteLink { target, output, range:
Option<RouteRange { from, to, curve }> }`, kept in `XySource::links` by target (a route
without a link reads X as it is). `ModRoute` itself is unchanged — 88 struct literals
across the workspace and every other source stay as they are, and a link left by a removed
route is ignored. A route adds `depth × amount × value`, where value is the axis output
(bipolar by default: the centre is the parameter's own value, as Bitwig's) or, with a
range, `from + (to − from) × curve(u)` over the axis position `u` (0…1) with the
automation's single curve (`Shape::Curve`) — FL's link range, *from 20 % to 80 %, bent*. A
range from −1 to 1 unbent is the bipolar output exactly; a symmetric range at the centre
adds exactly 0. A **unipolar** pad makes left and bottom the parameters' own values (FL's
way with the knob at its minimum).

### 3. The pad's X and Y are parameters: recording, MIDI, automation

`AutoTarget::ModInput { modulator, input }` (input 0 = X, 1 = Y; appended last in the enum)
with range 0…1 and its command **`Command::SetModulatorInput { id, input, value }`**
(exact inverse: the old value; a drag's frames merge per input; refused past the kind's
inputs, out of 0…1, not a number, or on a modulator that is not there; `cmd.fxd_set_input`).
Because it is an `AutoTarget` with `value_of_command`, everything that handles parameters
handles it: recording automation writes **a gesture as two lanes**, X and Y (ADR 0057 §6's
recording reads the commands the drag issues); MIDI learn maps a fader or an XY touch
surface to each axis (ADR 0062); an automation clip replays a gesture; another modulator
may drive an axis (one level of nesting, as an amount). A route into its own input is
refused like one into its own amount. The undo fuzz found an endless recursion in the
freeze region's walk (`freeze::target_in_strip_region`) through two controllers driving
each other's inputs; the walk now stops at one level for inputs as for amounts, and a
region takes in what drives a controller's input.

### 4. Inertia: control-grid accurate, stepped in closed form

The puck follows its target through **two equal one-pole stages** (critically damped: it
starts gently, never overshoots, settles), time constant `inertia_ms` (0…10 000; 0 is no
state at all). It is **state** (`xy::Puck`, in the engine's `ModRt`), stepped at each
control point over the exact interval to the next — the closed form of the two stages for
a target held over it, `p₁′ = T + (p₁ − T)a`, `p₂′ = T + (p₂ − T + (p₁ − T)·dt/τ)a`,
`a = e^(−dt/τ)` with `cantus_dsp::math::exp` — and the gains ramp linearly between points
like every modulated value. **Block-accurate on the 16-sample grid, not sample-accurate**:
the grid is what every native parameter already updates on (ADR 0020 §4), the closed form
makes the step length irrelevant to where the puck lands, and a pad with inertia makes
every grid point a control point while playing (`Runtime::puck_mods`) so the steps are
the same at every block size and offline. A puck on its target stays there to the bit.
Playback starts with the puck on its first target (offline as live); an edit while it
glides carries it on (`ModRt::carry`); a fork copies it (the processor's automation state
is cloned whole).

### 5. Paths: looping gestures synced to the song

`XySource::path: Option<XyPath { points, beats, smooth }>` — 2 to 256 points evenly spaced
in time over a loop of 0.25 to 256 beats, joined last to first, straight or Catmull–Rom,
read as a **function of the song position** (a render from bar 5 is bars 5… of a full
render), driving the puck's target while the song plays and `play_path` is on (stopped,
the hand has it). `XyPath::from_gesture` resamples a recorded gesture (beat, x, y) to
evenly spaced points; a drawn path is the same points. Inertia applies on top.

### 6. The engine

`ModPlan` carries the controller's `inputs`, the lanes that automate them
(`input_lanes`), each route's link (`links`, in route order); the runtime keeps per lane
the links of the routes into it (`links_in`, parallel to `routes_in`) and per modulator the
second output, the inputs at the control point and the puck. Controllers are computed
after every other source (their inputs may be modulated). Hosted destinations' inner grid
points read the pad as a function of time at the control point's inputs; a pad with
inertia holds its control point's value there.

## Consequences

- Schema 67: `ModSource::XyPad`, `AutoTarget::ModInput`; the migration writes nothing
  (nothing older has either). The v67 fixture: a pad "Sweep" on the master (X on the pan,
  Y on the width through 0…60 % bent 0.4, 250 ms of inertia, a playing four-point smooth
  path over two bars).
- The pad's face (its own editor, the puck, the trail, the path drawn) is step 3, after the
  owner approves the design sheet (`compartir/xy/design/v1`). Until then a pad is reachable
  through the model, the commands, MCP and the modulator rows' generic handling.
- Not in this step: *Bake to automation* of a pad with inertia (it is stateful: needs the
  trace path, `bake::needs_trace` — step 3); the pad's own puck position in the engine's
  telemetry for the face (step 3); a third axis (FL has two; Kaoss-style corner morphs are
  the backlog's).

## What the tests hold

- `cantus-core` `modulator::fxd_tests`: the input command and its exact inverse, merging,
  refusals; inputs as targets (`range_of`, `base_value`, `command`, `value_of_command`);
  link validation (two axes, ranges in −1…1, one per target, no route into its own input);
  route ranges equal the output where full and unbent and add exactly 0 at the centre;
  the puck's closed form splits exactly, never overshoots, rests to the bit, 0 ms is the
  target; a path loops on the song position and joins its ends; a gesture resampled; the
  snapshot carries links, inputs and the lanes; a save round-trips and writes nothing new
  for a default pad.
- `tests/controller_targets.rs`: `ModInput` is one of the 17 kinds of target, each with the
  command that sets it.
- `tests/undo_invariants.rs`: the controllers' own stream (`drng`, eight kinds: pads and
  formulas made with routes, links, ranges, paths and inertia; inputs set; an input routed
  from another modulator; two kinds always refused).
- `cantus-engine` `tests/fxd_controllers.rs`: unlinked, at rest (centred, inertia on, a
  symmetric range) or at zero — bit-exact with none; moving — heard, blocks 1…8 192 at
  tolerance 0; the real-time processor at irregular blocks equals the offline render; a
  path from bar 5 equals the full render there; inertia glides (under half way 50 ms after
  a jump, there after 1.6 s) where its absence lands within a grid step; a section heard
  from a listen checkpoint (a forked processor) is the whole render's to the bit.
  `tests/rt_memory.rs::controllers_never_allocate`: a gliding pad on a path, a new
  snapshot, a seek, a stop, a restart — no allocation.
- `cantus-project` `schema_67_keeps_the_pad_the_formula_and_their_input_lanes`.
