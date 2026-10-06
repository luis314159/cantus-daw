# Round 20 — automation clips and lanes (builder: automation)

Branch `r20/automation`, from the contract commit 403ca06. Decision: **ADR 0027**; schema
**19** (the integrator renumbers one of automation / hosted-inst to 20).

## What was built

**Model (cantus-core, `automation/`).** An automation clip *is* a `Clip` with
`automation: Option<Automation>` (a target and a curve), so move, split, trim, slip, loop,
mute, colour, copy/paste and undo all work unchanged. Targets (`AutoTarget`) by ids and keys:
track level and balance, strip fader / pan / width, sends, master gain / pan / width, any
effect parameter (strips and the master), native instrument keys, Firmus parameters.
Points (`AutoPoint`) in content ticks, normalised 0…1, with tension and FL's point modes
(`Shape`: single curve, double curve, hold, stairs, pulse, wave), defined once in
`Shape::at`. `AutoRange` maps to the parameter's units (linear, log, and the fader's taper —
`FADER_ANCHORS` is now the one table; the mixer's fader can delegate to it). Lanes compile
from every clip of a target (`compile`): the latest-starting clip wins overlaps, values hold
between clips, the knob's own value plays before the first. Articulators (`edit`): flip
vertical/horizontal, scale values/time, smooth, seeded randomise, normalise, LFO sequences
(sine, triangle, saw, square, random), splice, tension from a dragged midpoint.
Commands: `SetClipAutomation` (whole curve; merges over a drag), `SetTrackAutomationOpen`;
`InsertClip` validates curves. `AutoTarget::of_command` names the parameter any command
moved (*last tweaked*). Schema 19 with an empty migration and the fixture `v19.cantus`
(a bent fader ride, a looped stepped delay mix, a sine on the master's width, hidden lanes).

**Engine.** `Snapshot::automation` (one `AutoLane` per target). Values land on the
**16-sample control grid** counted from the transport's origin (ADR 0020's grid), so they are
the same for every block size and offline; the render loop splits only at control points,
and only while something moves (a held value costs nothing). Effects and instruments are
configured from parameter copies the UI thread builds (`automation::Runtime`, delivered by
`EngineMessage::Automation`, returned by `EngineEvent::GarbageAutomation`) — no allocation on
the audio thread. Gains and pans ramp **per sample** between control points (no zipper);
width and send levels step on the grid. Stopped, a seek evaluates once at the playhead.
`processor_auto.rs` holds it; `processor.rs` has six small hooks.

**UI.**
- *Create automation clip* from a parameter's right-click — one shared menu in the kit
  (`kit::automate`: create / edit automation, type a value, reset) on the kit's `Param`
  (every native instrument knob) and on the effects' cells (every native effect parameter,
  through `FxHost::{can_automate, automated, automate}`); the mixer's strip and master menus
  (volume, pan, width) and the track menu (volume, pan, channel level).
- *Last tweaked*: Clip ▸ *Create automation clip (last tweaked)*, which names it, and the
  action `automate-last-tweaked` (unbound) — any parameter command counts, so the EQ's
  nodes and every custom display are covered.
- Arrangement: automation lanes under the track (one per target, below the take lanes),
  headers with the parameter, the device and the value now; the clip body drawn as its curve
  with points and bend rings; points edited in place (drag, bend, press on the line to add,
  double-click to delete, right-click for shape); articulators in the clip's menu;
  double-click opens the large editor; marquee picks lane clips; glue joins one target's
  clips (split→glue exact).
- The large editor in the piano roll's area: window braced and outside dimmed, value axis
  in the parameter's units, Edit/Draw tools, Step/Slide, shape of the selection, flip,
  scale, smooth, random, normalise, LFO with cycles, Delete / Ctrl+A, wheel zoom.
- Automated knobs show the clip's value where the song is heard and a lit dot; the mixer's
  faders, pans and widths (strips and master) move with their automation.

## Tests

- core (`automation/tests.rs`, 13): shapes, holds, fader taper, lane windows and holds,
  overlap rule, loops, mute, split equivalence, snapshot, orphaned targets, command undo and
  refusal, last tweaked, articulators, tension inversion. Undo fuzz: arm 25 (automation clips
  for every kind of target incl. a vanished one, curves replaced/broken/cleared, lanes shown).
- engine (`tests/automation.rs`, 5): absent/muted/orphaned automation renders bit-identical
  to none; every kind of target at once is block-size independent (1…8192); real-time
  processing with irregular blocks is bit-identical to the offline render; a fader ride
  lands the gains the curve draws (held step exactly −6 dB); an effect parameter is
  untouched before its clip.
- ui (`tests/automation.rs`): a parameter makes a clip flat at its value in one undo step
  (and opens it the second time); last tweaked; the knob shows the automated value (the
  effect editor's host and the app); the lane's layout; dragging a point through published
  geometry (`autolanes::point_pos`), undo; a press on the line adds a point; the editor adds
  a point and Delete removes it; save/open; split then glue round trip.

## Files touched outside my area (all small, local)

`cantus-core`: `model.rs` (Clip field, Track flag, constructor), `command.rs` (two commands,
validation), `snapshot.rs` (lanes, automated sends kept, automation clips fire no notes),
`instrument.rs` (`FirmusParam: Ord`), `lib.rs`; `tests/undo_invariants.rs` (arm 25,
`below(26)`). `cantus-engine`: `processor.rs` (hooks), `messages.rs`, `backend.rs`,
`render.rs`, `stems.rs`, `transport.rs` (`position_at`), `lib.rs`. `cantus-project`:
`format.rs`, `midi/import.rs`, `examples/write_fixture.rs`. `cantus-ui`: `app.rs` (field,
`note_tweak` in `exec`, action arm), `keymap.rs` (one action), `clips.rs` (glue, split/copy
carry the curve, moves keep lanes), `record_pass.rs` / `channel_rack.rs` (exclude automation
clips), `views/arrangement.rs` (+ `arrangement_takes.rs` `TrackRow::auto`),
`views/piano_roll.rs` (branch), `views/transport.rs` (Clip menu entry),
`views/fx/{controls,host}.rs`, `views/inst/frame.rs`, `views/mixer_{menu,strip,parts}.rs`,
`kit/{mod,widgets}.rs`; `examples/shoot.rs`.

## Notes for others

- **Every path that sends a snapshot must send its runtime first** (`EngineHandle::
  set_snapshot`, `render`, `render_stems` do). A test pushing `SetSnapshot` by hand with
  automation in it needs `EngineMessage::Automation(AutomationRuntime::build(&snap))` first.
- device-ux / browser: `apply_pick` replacing an effect or an instrument: automation clips
  target the effect id and the instrument's keys. A `ReplaceEffect` that keeps the effect's
  id keeps its automation where the new kind has the same keys; the lane drops out (clips
  kept) where it has not. Worth offering "keep automation" deliberately.
- hosted-inst: hosted parameters are not automatable yet; the natural target is
  `AutoTarget::Plugin { effect | track, id }` sent as parameter events on the grid.
- The effects' cells now open a menu on right-click; *Type a value…* is in it.

## Left

- Hosted CLAP/VST3 parameters, Pulsus pad fields and **tempo** (with the tempo track).
- Recording automation (latch/touch: a hand on a knob while playing writes into the curve).
- Send and width ramps per sample (they step on the 16-sample grid today).
- Firmus's inspector rows have no right-click menu (they are automatable through *last
  tweaked* and the track/strip menus); the mixer's faders show automation but wear no dot.
- A looped clip glued to another flattens to its first pass.
- Automation clips are not in DAWproject/MIDI export yet.
