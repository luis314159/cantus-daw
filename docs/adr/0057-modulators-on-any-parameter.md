# ADR 0057 — Modulators on any parameter, hosted plug-ins included

**Status:** accepted (round 28) · **Date:** 2026-10-15 · **Extends:** ADR 0027
(automation), 0004 (commands), 0005/0031 (the audio thread), 0019/0028 (hosted plug-ins),
0055 (the modulation family's clock and LFO)

## Context

BACKLOG §0.2 names two things the leaders have and Cantus lacked: Bitwig's modulators on
any parameter, and hosted plug-ins' parameters under automation at all (ADR 0027 left
`AutoTarget::Plugin` as "the next step"; a hosted parameter reached its plug-in as a
`ParamChange { offset: 0 }` once per block, and only from a snapshot).

## Decisions

### 1. Hosted parameters are targets, normalised, at sample offsets

`AutoTarget::Plugin { effect, id }` and `AutoTarget::PluginInstrument { track, id }`.
The project does not know a plug-in's ranges (the hosted unit does), so their curves are
the plug-in's **normalised** value (0…1 of its range; the unit rounds a stepped one) and
`base_value` is NaN: *the project's own value*. Nothing is ever cut for a plug-in: at each
16-sample grid point inside a pass, the engine evaluates the hosted lanes and queues the
value at its **sample offset** in the pass (`AutoQueue`, fixed capacity, in the
`EffectUnit` and `NativeUnit`); an insert hears them all before its one call, an
instrument (which the track splits at note times) hears each in the segment it falls in,
offset into that segment. `FxUnit::automate` / `InstrumentUnit::automate` carry a
`HostedValue { pos, modulation }`; the unit maps it onto the parameter's range (the
project's value as the base where no clip plays), sends it when it differs from what the
plug-in holds (CLAP `param_value` at its time, VST3 `IParamValueQueue` points), and while
a parameter is driven it wins over the project's value (FL's override). When the engine
plans its automation again (a snapshot), `automation_replanned` lets go of every parameter
the new plan does not drive: it returns to the project's value. CLAP's event list holds
4 096, a VST3 queue 80 points (a full pass at 16-sample resolution); CLAP blocks longer
than the plug-in's maximum give each piece its own events.

*Last tweaked* and recording see a plug-in window's gestures: the host writes them to the
bus itself, and the application reads the newest after each idle.

### 2. A modulator is a source and its routes, living on a host

`Modulator { id, name, source, routes: [ModRoute { target, depth −1…1 }], amount, muted }`
on `Track::modulators`, `MixerStrip::modulators` or `Master::modulators` (16 per host,
32 routes each) — the host is the panel it is shown in and where its notes and audio come
from by default, and it goes and comes back with its track or strip. Sources: **LFO**
(sine, triangle, square, saws; synced division or Hz; phase; retrigger by notes;
unipolar), **envelope** (ADSR, opened by notes or by an audio level with 3 dB hysteresis),
**follower** (a strip's or the master's peak, attack/release, gain), **steps** (1–32,
synced, glide), **random** (held or smooth, a hash of seed and step). A route into
`AutoTarget::ModAmount(m)` modulates another modulator's amount — one level of nesting
(the source reads its own unmodulated amount there).

Every edit is one of three commands: `InsertModulator`, `RemoveModulator`, `SetModulator`
(the whole modulator; a drag merges and supersedes). Each is validated (finite, in range,
one route per target, never into its own amount), with an exact inverse, in the fuzz.

### 3. What modulation does: an offset in the normalised domain

`position = clamp(base + Σ depth × amount × output, 0, 1)`, `base` the automation's
position where a clip plays, else the project's value. A modulated target that no clip
automates gets a lane with no pieces in the snapshot, so the engine finds every
destination in one list (`Snapshot::modulators` holds the plans: routes resolved to lane
indices, the note gates, the listened strip). A lane whose modulation sums to exactly 0
reads exactly as before, so a muted modulator, a depth of 0, a target that is gone or a
modulator driving nothing compile to **nothing** — the null test holds to the bit.

### 4. Determinism: functions of time where possible, stepped state where not

An LFO, a step sequence, a random source and an envelope opened by the sequenced notes are
**pure functions** (`cantus_core::modulator::eval`, shared with the bake): of the song
position when synced (a render from bar 5 equals bars 5… of a full render — tested), of
the time since playback started when free (`modfx_lfo::wave` and `random_bipolar`, the
modulation family's own, ADR 0055). The envelope over sequenced notes is a closed form over
the note gates (each note-on retriggers from the level the previous gate left), so it too
is a function of the position. A follower, an audio-opened envelope and live notes carry
**state**, stepped at each control point over the window since the last one (the strip's
post-fader peak, read and emptied by the control phase), restarted when playback starts,
carried from one snapshot's runtime to the next by id (an edit while playing never drops
a follower to zero), and one control point behind so a gain ramps from the last value to
this one. A modulator that follows audio forces a control point at every grid point (its
window must be whole); so does any modulated native destination while playing; a hosted
one never does. Stopped, the modulators run on for what is played live — a control point
every pass, ramps over the pass — from the playhead at the tempo.

Measured: block sizes 1, 7, 16, 37, 64, 333, 1 024, 8 192 at tolerance 0; the real-time
processor at irregular blocks equals the offline render to the bit; no allocation on the
audio thread through every source, a new snapshot, live notes, a seek, a stop and a
restart (`rt_memory`); a hosted step sequence lands on its samples at blocks 1…4 096.

### 5. Bake to automation

`cantus_core::modulator::bake` samples the target's position (base and this modulator)
every 1/96 beat — every synced division starts on that grid, so steps and held randoms
keep their jumps as a hold and the next value — and thins the rest to 1/500 of the travel.
A source that follows audio is traced by an offline render (`cantus_engine::modulator::
trace`, on a worker) and baked from the trace when it returns, if the project has not
moved. The application inserts the clip over the time selection (else the loop, else the
song), clears what would play over it (`automation::edit::cover`), and removes the route:
one undo step. Measured: a baked LFO, steps and a follower render within 1 % of the
modulated render (the thinning).

### 6. The interface

The inspector's **Modulators** section per host (the track's, its channel's, the
master's): a title row (handle, name in the kind's hue, live meter, *map*, on/off,
remove), the movement drawn from the engine's own functions with where it is now, the
settings in the sheet's row grammar, the amount, a row per route (target, depth, bake,
disconnect). **Modulate…** in every parameter's menu (kit `automate::menu`: native
instruments' knobs, effects' cells, hosted plug-ins' generic editor, a hosted instrument's
*Parameters* list, the inspector's track and Firmus rows, the desk's strip and master
menus, the track menu) opens a list beside the control: connect or disconnect each
modulator, *New …* (made on the parameter's own host, already connected), bake. A
modulator's handle dropped on any modulatable control connects it; **Alt+drag** on a
modulated control (or any drag while a modulator is *mapping*) edits the depth; modulated
knobs wear rings by kind, a live dot and a mark.

**Recording automation**: while a pass records (`RecordSettings::record_automation`, on by
default, a switch in the record options) every parameter moved — knob or plug-in window —
is written where it was heard and becomes a clip per parameter at the end of the pass, one
undo step; a loop pass writes over what the previous one wrote.

## Consequences

- Schema **28**: `Track/MixerStrip/Master::modulators` and the three new targets (28's
  fixture: one modulator of each kind, nesting, a hosted route, a curve on a hosted
  parameter); `RecordSettings::record_automation` rides schema 29 (ADR 0058).
- New: `cantus_core::modulator::{eval, compile, bake}`, `automation::edit::{cover,
  from_samples}`, `AutoTarget::value_of_command`; `cantus_engine::modulator::{ModRt,
  AutoQueue, ModTelemetry, trace}`, `EngineEvent::Modulation`, `EngineHandle::modulation`,
  `FxUnit/InstrumentUnit::{automate, automation_replanned}`, `fx::HostedValue`,
  `Processor::trace_modulator`; `cantus_ui::views::modulator` and the inspector's
  section; `kit::automate::{ModOffer, ModHandle, Choice::{Modulate, Depth, DepthDone,
  Connect}}`, `KnobLook::live`.
- Fixed on the way: an EQ band raised from 0 dB by automation (or anything else that
  glides it) never moved — a flat EQ never ran the update that starts the glide
  (`Aequalis::is_active`).
- Not shared yet: the follower is our own few lines (`eval::Follower`); dynfx's
  `dynfx_follow.rs` arrived on another branch.

## Amended by review 1 (round 28)

- **Every parameter control offers modulation, through two shared paths.** An effect's
  cell goes through `controls::Cell::show_in` (its automation and its modulation answered
  through the `FxHost`), an instrument's knob through `inst::frame::{wire, wired}`: no
  editor builds its own. The desk's pan, fader and width (strips and master) go through
  `mixer::strip::DeskMod` (the handle's drop, Alt-drag, rings, live dot, the fader's
  mark); an effect with no editor of its own is modulated from its inspector rows; a
  modulator's **amount** is a parameter row (nesting is reachable). A kit probe
  (`kit::automate::probe`) records every parameter control drawn and whether it offered
  modulation; `tests/modulate_everywhere.rs` opens every native effect and instrument
  editor, in every view, and fails on any that does not. A number that is no parameter of
  anything (a drum pad's field) says so (`Param::plain`).
- **Hosted queues take only changes**: an inner grid point queues a hosted value only when
  it differs from the last one queued for that lane, and a unit's queue holds 8 192 — every
  grid point of a full pass for 128 parameters moving at once (VST3's own limit per block).
  Whatever finds no room anywhere on the way — the engine's queue, the plug-in host's
  merge, a CLAP event list (now `HOSTED_QUEUE + 1 024`), a VST3 block's 128 queues — is
  counted in `EngineStats::dropped_events`, never silent.
- **A parameter the project holds no value for** swings around what the plug-in held when
  it was first driven (`Known::auto_base`), never around what was last sent.
- **Modulators that drive only hosted parameters** get a control point at least once a pass
  while playing (on the grid, so block sizes still agree), so their state steps and the
  interface hears them; an envelope opened by live notes steps at every hosted grid point.
- **A live retrigger belongs to where it was played**: a wrap, a seek or a stop lets it go,
  and a live note-on ahead of the song is ignored.
- **The note envelope is exact over any number of gates**: the level each gate starts from
  is computed once per envelope and tempo when the modulator is compiled
  (`Gates::with_levels`), not by walking back eight gates.
- **Bake** includes the modulation of the baked modulator's amount (one level of nesting,
  as the engine plays it), traces the render when anything there follows audio
  (`bake::needs_trace`), and finds every jump by its value a tick before (so a jump on a
  slope is held at its own tick).
- **A follower whose strip is gone hears silence** (`ListenAt::Silent`), and the panel
  says so; **a new random source takes its seed from its id** (1…999), so two do not move
  together.
- **Recording from a plug-in's window** records every parameter the window moved in a
  frame: the host keeps each command it ran (`PluginHost::take_tweaks`).


## Amended by fix3 (round 28: review 2)

- **A parameter the project stores no value for goes back to its base** when its drive lets
  go — a modulation sum of exactly 0 or a replan alike (`Known::release`); the base is taken
  once per drive, so it never climbs.
- **The engine queues only a change, and only what found room**: `AutoQueue::push` says
  whether it took the change and `Lane::sent` is stamped only then; a unit swapped in place
  (a plug-in's restart) has every lane queued again at the next grid point. Every hosted
  unit's losses are counted where it runs, automated or not.
- **Sends, the pinned strip**: a send's level is modulated and automated from the desk's
  send knob and rows, the cable pills, the inspector's send and receive rows and the route
  menu (*Automate the send's level*, *Modulate the send's level…*); the inspector's pinned
  strip (a channel's or the master's) takes modulation as the desk's copy does.
- **While a modulator maps, only the drag is the depth's**: a double-click, the arrows and
  Ctrl+wheel still set the value, on the desk and in the sheet as in the kit's `Param`.
- **Every continuous parameter of an editor-less effect has its row** (Reverb's pre-delay).
- **A hosted instrument's words** are keyed by its instance and the values the project
  stores for it.
- **Synced modulation effects hear the position their audio is at**: each insert's song
  position is moved back by how late its audio is (`EffectUnit::late`: its strip's
  compensation and the inserts before it; on the master likewise), folded into the loop.
