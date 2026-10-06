# ADR 0027 — Automation clips and lanes

**Status:** accepted (round 20) · **Date:** 2026-10-05 · **Extends:** ADR 0004 (commands),
0005 (the audio thread), 0006 (the project format), 0020 §4 (the 16-sample grid), 0024 (the kit)

## Context

The owner asked for automation clips. The playlist plan (STATUS, "Planned: the
arrangement") called FL's automation clips *take* and its articulators "worth stealing
wholesale; almost nobody else has them", and asked that the choice between clips and lanes
be made once, deliberately. The bar: FL Studio's automation clips (a clip in the playlist
whose curve drives a parameter: points with tension, point modes, the articulators) and the
lanes under a track that Ableton, Bitwig and Cubase draw.

## Decisions

### 1. An automation clip is a clip; it lives in a lane under a track

`Clip::automation: Option<Automation>` — a target and a curve. Everything a clip already is
comes with it unchanged: a window onto its content (move, split, trim from either side,
slip, loop with `content_length`), mute, colour, copy and paste, duplicate, delete, undo.
Its points are in the clip's **content** ticks, exactly as notes are, so split and glue,
a slipped window and a looping one need no special case: `Snapshot::compile` reads the
window the way it reads it for notes.

The clips sit in `Track::clips` (lane 0), but the arrangement draws them in **automation
lanes** under the track — one lane per distinct target, in target order, below the take
lanes — so a note clip and an automation clip never share a row (Ableton/Bitwig's lanes,
FL's clips). `Track::automation_open` shows or hides them. A drag moves an automation clip
in time only: its lane is not a row to drop it on, and its target does not depend on the
track it sits under. A track's lanes are found from its clips; there is no lane object to
keep in step.

What it means for the other clip operations: a recording take never cuts an automation
clip, the channel rack never writes steps into one, glue joins clips of one target into one
curve (halves of a split glue back to exactly the clip), and the piano roll opens one in
the automation editor.

### 2. What can be automated: `AutoTarget`, by ids and keys

`TrackGain`, `TrackPan` (the channel's own level and balance), `StripGain`, `StripPan`,
`StripWidth`, `Send { strip, to }`, `MasterGain`, `MasterPan`, `MasterWidth`,
`Effect { effect, key }` (any insert, on a strip or the master — an effect id is unique, so
its host is found from it), `Instrument { track, key }` (a native instrument's `NativeSpec`
key) and `Firmus { track, param }`. Stable across saves: ids and parameter keys, which ADR
0018/0023 already made forever. A target that names nothing any more (an effect removed)
drops out of the snapshot; its clips stay, and play again when undo brings the effect back.

**Not yet:** hosted plug-ins' parameters (`SetPluginParam` ids — the CLAP/VST3 units take
parameter events of their own; a target `Plugin { effect, id }` is the next step), Pulsus
kit fields (pads by `PadId`), and **tempo** — a tempo track changes the tick↔sample map the
whole engine is built on, and is its own block with warping (STATUS, round 19 item 2).

### 3. The curve: normalised points with FL's modes and tension

`AutoPoint { tick, value 0…1, tension −1…1, shape }`. Values are **positions on the
control**, mapped to the parameter through `AutoRange`: linear, logarithmic (frequencies,
times — the knob's own taper), or the **fader's** taper for gains (one table, `FADER_ANCHORS`,
now shared by the mixer's faders and automation, so a curve drawn at 0 dB plays 0 dB).
Flip, scale, the LFO shapes and moving a curve to another parameter are then the same
arithmetic for every target.

Shapes (FL's point modes), each defined once in `Shape::at`, which the editor draws and the
engine plays: **Curve** (single curve; tension bends it towards either end, `u^p` and its
mirror, `p` 1…8), **S-curve** (double curve), **Hold** (a step), **Stairs**, **Pulse** and
**Wave** (tension sets the count, 2…16). Step and slide are the editor's drawing modes: a
drawn point holds (Step) or ramps (Slide).

### 4. Which clip plays

All unmuted clips of one target, on any track, make one **lane**. Where clips overlap, the
one that **started last** plays, and a long clip comes back after a short one dropped on it.
Between clips the value **holds** where the last one left it (FL); **before the first clip
the parameter is what the project holds** (its knob), so a render from any start point is
the same and an unautomated stretch is the mix as set. Track mute does not mute
automation (the lanes merely live under a track); clip mute does.

### 5. The engine: the 16-sample control grid, ramps for gains

`Snapshot::automation` holds one `AutoLane` per target: pieces sorted by start, each a
piece of curve between two points placed on the timeline (`AutoSeg`), evaluated with a
binary search and at most one `powf`.

**When.** A lane's value is taken on the **16-sample grid every native DSP already updates
on** (ADR 0020 §4), counted from the transport's origin (the last start, seek, loop wrap or
tempo change) — not from the block — so it is the same for every block size and offline.
Sample-accurate evaluation would buy nothing for the effects: they act on their own grid,
and a parameter set between grid points lands on the next one anyway; for gains it would
cost a curve evaluation per sample. At a control point every lane is evaluated; the next
point is due at the next grid point while anything moves, and otherwise at the grid point
just before the next piece of any lane begins (a held value costs nothing). The render loop
splits its segment at the next control point; with no automation nothing is split.

**What.** Effects and instruments are configured from parameter **copies** with the new
values (their own glides smooth them, as they smooth a knob) — at most once per 16 samples,
and only when a value changed, which is what ADR 0020 already does for a host's automation.
Gains and pans (the track's level and balance, the strip's fader and pan through the pan
law, the master's level and balance) **ramp sample by sample** from this control point's
value to the next one's, computed from the control point (never accumulated), so a fader
ride has no zipper. Width and send levels step on the grid (0.33 ms at 48 kHz).

**Without allocating.** An effect or instrument is configured from a whole parameter set,
which the audio thread cannot copy (a map allocates). The UI thread builds an
`automation::Runtime` with a copy of each automated set — every automated key already in
it, so writing it is a write and never an insertion — and sends it ahead of its snapshot
(`EngineMessage::Automation`); the audio thread pairs them by the snapshot's address,
overwrites numbers in place, and hands the runtime back to be freed
(`EngineEvent::GarbageAutomation`). A snapshot whose runtime did not arrive plays
unautomated and is counted in `dropped_events`.

**Null.** No lanes → no runtime → nothing new runs: no split, no ramp, the master's values
the snapshot's. Muted clips and orphaned targets compile to no lanes at all. A gain whose
curve holds still has no ramp and takes the old constant path.

**Stopped.** At a seek while stopped the lanes are evaluated once at the playhead, so a
note played on the keyboard hears the automated value there (FL moves the knobs when the
playhead moves).

### 6. A hand on an automated knob: FL's override

While the song plays, automation wins: the knob follows the curve, and a hand that moves it
changes the project's own value — what plays before the first clip — which the next control
point overrides. That is FL's behaviour and the predictable one; a latch/touch mode that
writes the hand into the curve is recording automation, a later block.

### 7. The interface

- **Create automation clip** from any parameter's right-click: one shared menu in the kit
  (`kit::automate`) — *Create automation clip* (or *Edit automation* once there is one),
  *Type a value…*, *Reset to default* — on the kit's `Param` (every native instrument's
  knobs), the effects' cells (every native effect's parameters, through `FxHost`), the
  mixer's strip and master menus (volume, pan, width) and the track's menu (volume, pan,
  level). The effects' cells used to type on a right-click; typing is now an entry of the
  menu, as in FL.
- **From the last tweaked parameter**: every command that sets a parameter is remembered
  (`AutoTarget::of_command`), whatever widget sent it — so the EQ's nodes, the compressor's
  curve and every custom display are covered too. *Clip ▸ Create automation clip (last
  tweaked)*, which names the parameter, and an unbound action (`automate-last-tweaked`).
- A new clip spans the time selection, else the loop, else four bars from the playhead's
  bar, flat at the parameter's value now, under the track the parameter belongs to (for a
  strip, the first track feeding it; for the master, the selected track), selected, with the
  lanes shown — one undo step.
- **In the arrangement** the clip's body is its curve, filled, with its points and (selected
  or hovered) its bend rings; points are edited in place: drag a point, drag a ring to bend,
  press on the line to add a point, double-click to delete, right-click for the shape. The
  lane's header names the parameter and its device and shows its value where the song is.
- **The large editor** is the piano roll's area for an automation clip: the curve over the
  content with the clip's window braced and the outside dimmed, the value axis in the
  parameter's units, Edit and Draw tools, Step/Slide, the selection's shape, and FL's
  articulators — flip vertical and horizontal, scale values or time, smooth, randomise a
  little (seeded, so undo and redo are exact), normalise, and *create sequence* (sine,
  triangle, saw, square, random steps, 1–16 cycles over the selection or the clip).
- **Automated knobs show it**: the value the clip gives where the song is heard, and a
  lit dot at the knob's corner (effects' cells and the kit's `Param`); the mixer's faders,
  pans and widths move with their automation.

### 8. The format

Schema **19**: `Clip::automation` and `Track::automation_open` (written only when false).
The migration writes nothing: a clip without the field is a note or audio clip, and a track
without the flag shows its lanes — exactly the old project. The fixture `v19.cantus` holds
a fader ride with a bent curve, a looped stepped effect mix and a sine on the master's
width, and a track with its lanes hidden.

## Consequences

- New: `cantus_core::automation` (`AutoTarget`, `Shape`, `AutoPoint`, `Automation`,
  `range`, `compile`, `edit`), `Command::{SetClipAutomation, SetTrackAutomationOpen}`,
  `Snapshot::automation`; `cantus_engine::automation` and `processor_auto.rs`,
  `EngineMessage::Automation`, `EngineEvent::GarbageAutomation`, `Transport::position_at`;
  `cantus_ui::automation`, `views::{auto_curve, auto_editor}`,
  `views::arrangement::autolanes`, `kit::automate`; the locale area `automation`.
- `FirmusParam` is ordered (targets sort); `TrackRow` gained `auto`.
- Every path that sends a snapshot must send its runtime first: `EngineHandle::set_snapshot`,
  `render`, `render_stems` do; a test that pushes `SetSnapshot` by hand with automation in it
  must push `Automation(Runtime::build(&snap))` before it.
- What the tests hold: absent (no clips, muted, orphaned) renders bit-identical to none;
  every kind of target at once is block-size independent (1…8192) and bit-identical
  between real-time processing and the offline render; a fader ride lands the gains the
  curve draws; an effect's parameter is untouched before its clip; the core's curve, lane,
  split, loop, overlap and articulator rules; undo of every edit in the fuzz.
