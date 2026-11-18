# ADR 0144 — The X-Y pad

**Status:** proposed (fx side round wave D) ·
**Date:** 2026-11-17 · **Schema:** 77 (67 on its branch; 75 and 76 are waves B's and C's) · **Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread),
ADR 0027 (automation), ADR 0057 (modulators on any parameter), ADR 0062 (MIDI learn),
ADR 0107 (listen and forks), ADR 0145 (the formula controller, the same architecture)

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

## Phase B (step 3, 2026-11-17): the window, its gestures, presets, bake, the agents

The owner approved design sheet v1 and the name **Tactus** (Latin *touch*), the polarity's
default *Both ways* (bipolar). His notes on v1 — the Spanish cards stretched with empty
space, *What it drives* left a band under its rows — are fixed: a card row's slack is
shared by item (*Horizontal* and *Vertical* on the Pad card, both cards on one row in
Spanish too), the window is shorter, and the list is sized to its rows. Shots:
`compartir/xy/design/v2-step3/`.

- **The window** (`views/fx/fxd_window.rs`, `fxd_xy.rs`, `fxd_edit.rs`): opened from the
  modulator's row in the inspector (*Window · Open*, or a double-click on its picture) as
  a floating device window like an effect's editor — moved by its header, closed by its key
  or `Esc`, minimised to the tray, maximised, scaled from the shell's grip; one controller
  window at a time, closed when its modulator goes (an undo, a removal). The header's lamp
  switches the modulator on and off (one step).
- **The pad's drag** is one `Command::Batch` (`cmd.fxd_set_input`) of two
  `SetModulatorInput`, X and Y, a frame, merged under the drag's key: **one undo step a
  drag**, and a pass writing automation (recording, or Write/Touch/Latch armed) writes
  **two lanes, X and Y** — `record_tweak` writes each member of a batch.
- **Paths, drawn** (the pencil key on the glass, *Draw path*): the drag's points, each at
  its share of the drag, over one loop of the path's length (or the *Loop in beats* card's,
  4 beats at first), resampled to at most 64 points; laid on release with `play_path` on,
  one step. **Paths, recorded** (the record key, *Record path*): while the song plays the
  drag is heard as the hand and sampled at the song's beats; on release
  `XyPath::from_gesture` lays it over the loop it began in, 16 points a beat (8 to 256), so
  it plays back where it was played — the moves and the path one step. Recording over a
  path that plays switches the path off for the drag, in the same step, so the hand is
  heard and not the old path; the new path plays after; a drag that lays nothing gives the
  old path back. With the song stopped the record key's drag is the hand's and lays nothing.
- **The cards**: *Pad* (Horizontal, Vertical — the inputs through `SetModulatorInput` —
  Polarity, Amount) and *Motion* (Inertia 0…10 000 ms, Path *Off*/*Plays*, *Loop in beats*
  0.25…64 in quarter beats, Line *Straight*/*Smooth*); a knob's drag one step, a switch's
  click one.
- **The links edited in *What it drives*** (`fxd_xy::LinkEdit`): each route's row — a
  click on its X/Y tag swaps the axis it reads, a drag on an end of its bar sets that end
  of the range (`RouteRange::from`, `to`), a drag on its middle up or down bends it
  (`curve`, −1…1, the automation's single curve), a double-click gives the whole travel
  back (no range). Written into the source's `links` by target (a link that reads X whole is
  no link); a drag one step, an axis click or a reset one each. The list is sized to its
  rows and scrolls by the wheel. Every removed route takes its link
  (`Modulator::tidy_links`, wherever a route is taken away — the window, the inspector,
  the agents, a bake).
- **The puck from the engine**: `ModTelemetry` tells each pad's puck and where it heads
  (`pads: [[x, y, heads x, heads y]; MOD_TELEMETRY]`, `pad_mask`, `pad(id)`), written at
  each control point without allocating. The face draws the puck there, the hand's
  crosshair where it heads, a 64-point trail (a fixed ring); it asks for frames only while
  something moves, so a window at rest allocates nothing and repaints nothing.
- **Presets** (`cantus_core::modulator::presets`, words `fxd.preset.<id>[.what]` in
  `locales/fx/controllers.*`): *Filter sweep* (`xy.filter_sweep`: puck at x 0.5, y 0, 80 ms;
  route 1 reads X through 0…1, route 2 Y through 0…0.6 bent 0.4), *Width and pan*
  (`xy.width_pan`: centred, 120 ms, X and Y as they are), *Figure eight*
  (`xy.figure_eight`: a 32-point 1:2 Lissajous over 8 beats, smooth, playing), *Performance
  pad* (`xy.performance`: centred, 400 ms). `apply` keeps the modulator's own (id, name,
  routes, amount, mute, colourway) and lays the preset's links over the routes in order;
  `matching` ignores the look, the links and the inputs, so the header keeps the preset's
  name while the pad is played, and says *Custom* when it matches none. The header's well
  steps through them (arrows) or lists them with their descriptions (a click on the name);
  choosing one is one `SetModulator`.
- **Bake to automation** of a pad (`modulator_bake.rs`): its inputs read from their lanes,
  the route's axis and range × amount; a pad with inertia, or one whose inputs another
  modulator drives, is **traced** — `cantus_engine::modulator::trace_route` renders the
  route's value (both outputs through its link, as the lane hears it) × amount at every
  control point, and the bake reads that.
- **The agents** (`cantus-ai`, MCP): `add_modulator` kind `xy_pad` with `x`, `y`,
  `unipolar`, `inertia_ms` (0…10 000), `path {points, beats, smooth}`, `play_path`; a
  route's `axis` ("x"/"y") and `range {from, to, curve}`; `set_modulator` takes all of it
  and `change_routes` (a route's depth, axis, range by target); its `x`/`y` become
  `SetModulatorInput` (one step a call). A controller input is a target by name,
  `{modulator_id, param: "x"|"y"}`, that round-trips and reads as itself ("X-Y pad 1 · Y")
  — **a bug fixed**: `target_of` ignored `param` and `words::target` used the amount's
  phrase, so a `ModInput` read as the modulator's amount. Reading shows `settings` and
  each route's axis and range; `get_mixer` lists the strips' and the master's modulators.
  The tools' golden (`tests/golden/mcp-tools-list.json`) regenerated on purpose: only
  `automate`, `replace_automation`, `add_modulator`, `set_modulator` and `remove_items`
  changed.
- **Exported plug-ins**: nothing. Modulators live in the host's project and engine; the
  exported CLAP/VST3 effects carry none, and the pad has no DSP of its own to export.

## Consequences

- Schema 77: `ModSource::XyPad`, `AutoTarget::ModInput`; the migration writes nothing
  (nothing older has either). The v77 fixture: a pad "Sweep" on the master (X on the pan,
  Y on the width through 0…60 % bent 0.4, 250 ms of inertia, a playing four-point smooth
  path over two bars). Phase B changed no schema: the window, presets and bake are views
  of what the model already held.
- Step 1's deferrals are done: the face and its window (Phase B), *Bake to automation* of
  a pad with inertia (the trace), the puck in the engine's telemetry.
- **Found by the e2e, older than the pad, not fixed here** (for the integrator and the
  owner): (1) a native effect unit starts a play or a seek at its *stored* value, not the
  automated or modulated one, and glides there over its de-zipper (~20 ms for Aequalis) —
  so a render from bar N differs from the full render on the first note for any automated
  native effect (plain automation too: a lane holding an EQ band at +9 dB differs by up to
  5.2e-2 from bar 3), while strip pan/gain and track gain are bit-exact from bar N; fixing
  it likely changes the goldens (`DSP_VERSION`). (2) Firmus starts its cutoff and gain
  smoothers at the factory defaults on a render that begins on a note (it snaps only in
  idle blocks). The e2e proves render-from-bar-N on the strip's pan and fader, and on the
  EQ from the first full beat.
- The face's colourways (Titanium, Copper, Frost; the formula's Bakelite, Ivory, Ebony)
  are chosen in the window's ⋯ menu, as the other units' (the source's `look`, one
  `SetModulator`, one undo step).
- **The window's parameters are every Cantus parameter's** (follow-up, 2026-11-17): X and
  Y — each axis's rail on the pad and its knob on the *Pad* card — and *Amount* wear the
  kit's parameter menu (`kit::automate::menu`: *Create automation clip* / *Edit
  automation*, *Modulate…*, *Learn* / *Change* / *Forget MIDI controller*, *Reset to
  default*), its signs and rings, the learn mode's outline and Alt-drag depth; each
  mutation one undo step. `Alt+A` and `Alt+Shift+A` automate and learn what the window
  moved last. The pad's own settings (inertia, the path's switch, loop and line, the
  polarity) are no parameter — nothing automates a modulator's settings, as the
  inspector's rows of every modulator's — and are drawn plain (`Cell::plain`, the probes
  pass them by).
- **The last thing touched is the pad** (follow-up 2): a drag sets X and Y in one batch,
  and *last tweaked* keeps both (`AutoUi::last_tweaked_with`, set when a batch's parameters
  are exactly two inputs of one modulator). `Alt+A` makes the clips of the axes that have
  none in **one undo step** (`automate_pair`, as a recorded gesture writes two lanes), and
  shows X's when both have one. `Alt+Shift+A` learns the pair **X, then Y** — the kit arms one
  target at a time, so the second is queued (`ControllerUi::learn_next`) and armed when the
  first is learnt; the control just learnt is passed over while the second waits
  (`learn_skip`), so a knob still turning is not taken twice; `Esc` drops both; each mapping
  is its own undo step. Chosen over learning the pair from one two-control gesture (a
  hardware XY pad's two CCs): that needs a second detection mode, and X-then-Y serves both a
  joystick and two knobs. A card knob is one axis and stays one.
- **The inertia knob covers the model's range** (follow-up 2): 0–10 000 ms
  (`MAX_INERTIA_MS`, the agents' tool's and the presets' range) on a swell taper
  (`Taper::Swell(6.41)`: three quarters of the travel to 2 s, half to ~390 ms), so a pad set
  past 2 s reads truly and a touch never pulls it back; the flywheel's weight follows the
  knob's travel.
- **The chip, the list and the rails count the same routes**: the footer's chip and the
  rails come from the routes the list lists (`fxd_xy::drives_chip`, `fxd_xy::rail_words`); a
  rail names as many as it holds (at most two) and counts the rest, never cutting the
  count; an axis that drives nothing says only `X` / `Y`.
- Not here: a third axis (FL has two; Kaoss-style corner morphs are the backlog's).

## What the tests hold

- `cantus-ui` `tests/fxd_drag.rs` (follow-up): `the_chip_the_list_and_the_rails_count_the_same_links`
  (0, 1, 4 and 7 routes on both axes, pad and formula, through `assemble`);
  `the_pads_axes_and_amount_offer_the_parameter_menu` (right-click on each rail and knob:
  automate, edit, learn with a turned controller, forget, reset — one step each where it
  mutates); `a_controllers_own_settings_offer_no_automation`;
  `the_learn_mode_reaches_every_parameter_of_the_windows`;
  `alt_a_automates_and_alt_shift_a_learns_what_was_moved_last_on_the_window`;
  `alt_a_after_a_pad_drag_automates_both_axes_in_one_step`;
  `alt_shift_a_after_a_pad_drag_learns_x_then_y`;
  `the_inertia_knob_reaches_the_models_whole_range`. `cantus-ai` `tests/tools_fxd.rs`:
  `an_agent_sets_a_pads_inertia_over_the_models_whole_range`.
  `tests/learn_everywhere.rs`: `the_controllers_windows_offer_learn_on_every_parameter`.

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

Phase B (step 3):

- `cantus-core` `modulator::presets::tests` (9): every preset valid and every formula
  compiled; `apply` keeps the modulator's own and is matched again; a preset applied over
  another kind or after playing is still named; `tidy_links` drops what no route reads;
  the figure-eight loops over two bars (and the formulas' shapes, ADR 0145); both
  languages name every preset. `modulator_bake` (3 new): a pad bakes the axis and range
  its route reads with its input lanes; a formula with its automated knob and its compiled
  program; a pad with inertia or a driven controller is traced.
- `cantus-engine` `tests/fxd_bake.rs` (5): a pad on its Y through a bent range, a formula
  with an automated knob, a pad with inertia (from its trace), a formula whose knob an LFO
  drives (from its trace) — each baked curve within 0.002 of the trace at the control
  points, the baked render within 1 % of peak of the modulated one, the route audible; the
  telemetry tells where a gliding puck is and where it heads.
- `cantus-ui` `tests/fxd_drag.rs` (17, through `CantusApp::assemble` with real pointer and
  key events): the window opens from the row and a drag sets X and Y in one step; a drag
  while a pass writes automation writes two lanes; a drawn path is one step; a gesture
  recorded while the song plays becomes the path; recording over a playing path hears the
  hand and lays the new path; a link's axis and range set in *What it drives*, one step
  each; the cards one step each; the puck drawn where the telemetry says; a preset chosen
  from the header is one step and named; the window closes by its key and `Esc` and opens
  by a double-click on the picture; many links scroll by the wheel (the formula's own six:
  ADR 0145). `tests/fxd_whole.rs` (2, step 2's): no cut word in any window, language,
  scale or colourway.
- `cantus-ui` `tests/fxd_e2e.rs` (6, through `assemble` and the real engine wiring): a pad
  linked to two EQ bands, moved, heard on each (±12 dB, the other within 0.1 dB), dry to
  the bit at rest and after undo; a recorded gesture writes exactly two lanes equal to the
  hand at all 96 touched ticks, its render block-size independent (64/333/4096) and live =
  offline at odd blocks 1…1 024, undone to the bit; the figure-eight path followed within
  0.5 dB over 16 beats, from bar 3 the full render's to the bit on the strip's pan and
  fader; unlinked controllers and depth-0 routes the dry master to the bit (and the
  formula's two, ADR 0145).
- `cantus-ai` `tests/tools_fxd.rs` (5): an agent makes a pad with its settings, axes and
  ranges; inputs move as the hand moves them, each call one step; a controller input is a
  target that reads as itself and round-trips; links follow their routes.
