# ADR 0076 — The session view: model, commands, engine and recording

**Status:** accepted (round 33) · **Date:** 2026-10-29 · **Extends:** ADR 0004 (commands),
0005 (real time), 0025 (recording), 0027 (automation), 0031 (the worker pool), 0037
(linked clips), 0060 (the tempo map), 0064 (freeze) · schema 46

## Context

BACKLOG §0.2 *Clip launching / session view*: Ableton Live's and Bitwig's grid of clips per
track and scenes, launched quantised, with follow actions, recorded into the arrangement.
Cantus had only the arrangement (`Track::clips` *is* the arrangement) and FL's pattern
mode (a window of it played alone).

## Decision

1. **The session shares the tracks.** A column of the grid is a `Track` — the same
   instrument, strip, automation — so there is one mixer, as in Live. The grid is split in
   two, each half beside what it belongs to: `Project::session` holds the rows (`Scene`:
   name, colour, an optional tempo and metre launching it sets, its follow actions) and
   the settings (`SessionSettings`: global launch quantisation — a bar by default —,
   record quantisation, the follow actions' seed, a fixed recording length);
   **`Track::slots`** holds the track's column, one `Slot` per scene that holds something
   (a clip, or an empty slot without its stop button), sorted by scene id. (The contract
   had the whole grid in `Project::session`; the columns moved to the tracks so a track's
   removal, its undo, a copy and its freeze carry its column with no case of their own,
   and so `Track::clip` can find a session clip — the integrator accepted it.) An absent
   slot is an empty one with its stop button: a project with no session writes exactly
   what it wrote. `InsertTrack` takes a column only as a command could have made it.
2. **A session clip is an ordinary `Clip`** (`SessionClip::clip`) plus its launch settings
   (trigger/gate/toggle/repeat; quantisation global/none/8 bars…1/32 with triplets;
   legato; velocity amount), its follow actions (on; A and B — none, stop, again,
   previous, next, first, last, any, other, jump to a scene —; the chance of A; after a
   time or *linked* to its length; a count), a start marker and its own envelopes
   (`Automation` curves of mixer and device parameters in content ticks). `Track::clip`
   finds session clips too, so the piano roll and every content command edit one as an
   arrangement clip; the timeline's own edits (cut, split, link, crossfade, comp, pattern
   mode, the clip selection) read `Track::arranged`. The window keeps its meaning read
   from the launch: play from `offset`; with `content_length` loop it forever; without,
   play `length` and end (`length` stays as the end marker while the loop is on). A
   session clip sits at 0, on lane 0, in no link group or comp, with no automation
   content: `session_guard` refuses (and undoes) any clip command that would leave it out
   of that form, the load repairs a hand edit. Patterns as slot content were weighed and
   left out: Cantus has no pattern objects (ADR 0052's pattern is a window of the
   arrangement).
3. **Commands** (`Command::Session(SessionCommand)`), each with an exact inverse, a
   `cmd.session.*` label and an undo-fuzz arm (its own stream, 19 kinds applied, the first
   12 refused too): a slot's clip inserted, removed, moved (its envelopes of its track's
   own level, balance and instrument follow it to the other track — swapped, so the
   inverse is exact; a note effect's cannot follow and the move is refused), its launch
   settings, follow actions, region, envelopes, stop button; a scene inserted (with its
   row), removed, moved, renamed, recoloured, its tempo, metre, follow actions; the
   settings. A frozen track's column is locked (`Touch::TrackSource`) and every target a
   carried envelope drives is touched. The gestures of several commands are `Batch`es
   built in `session::edit`: duplicate a clip (envelopes retargeted) or a scene, capture
   and insert scene, arrangement clips into slots and a slot into the arrangement,
   consolidate a column (refused when the clips would not play the same joined),
   consolidate time to a new scene, commit a performance.
4. **DAWproject** carries scenes as `<Scene>` rows of `<ClipSlot track hasStop>` with their
   clips; a start marker comes in from a `playStart` that is not the loop's start. What the
   format has no place for travels in `cantus/session.json`, read only from a file Cantus
   wrote; clip envelopes are named in the export report as left out.
5. **The engine's API** (`cantus_engine::messages::session`): `SessionMsg` (launch with a
   velocity, release, launch/release a scene, stop a track, stop all, back to arrangement,
   record into a slot and stop it, arrangement record, resend) and `SessionEvent` (a slot's
   phase — stopped, queued at, playing since/from, stopping since/from/at, record queued,
   recording —, a scene's phase, whether a track follows the arrangement, a follow action
   taken, a scene's tempo, arrangement record, overflow). All `Copy`; events only on
   change; the playing position is read off the phase (`SlotPhase::content_at`), never
   streamed.

### 6. What is heard is a projection

The session is **played as arrangement**. `session::play::plan` — a pure function of the
project, the presses (each with the earliest tick it may take effect) and a horizon —
works out the performance: launches resolved on the bar lines (through every metre change,
a scene's metre included), launch modes, legato, follow actions, scene tempos and metres,
stops and *back to arrangement*. `session::play::project_play` is the song with that
performance written into its arrangement exactly as arrangement record writes it
(`edit::perform`): each track's held stretch cleared (crossing windows cut, an audio cut
given the click-free `CUT_FADE`), each clip heard laid where it was heard (one window, or
the run to the loop's end and then the loop), its envelopes as automation clips over the
automation of their targets, the scenes' tempos as the tempo track, the loop off while the
session holds a track (the session plays on through the timeline, as Live's does).

Why: the engine plays snapshots sample-accurately, at any block size, with the worker
pool, offline as in real time — all of it held by tests — and every feature an arrangement
clip has (warp, fades, gain envelopes, lanes, grooves, note effects, pitch edits,
automation) comes with no engine code of its own. The alternative — a launcher on the audio
thread re-implementing events, controller lanes, audio pieces with their sample cache,
automation lanes and note effects for clips placed at run time — would duplicate the
engine's playback and could only approximate *the recorded arrangement renders identical to
the live performance*: here it is the same project, identical by construction.

**Ids.** A performance's windows take **keyed** ids (`edit::keyed_id`: the clip and where
its window starts, mixed into the top quarter of the id space `Project::alloc_id` never
reaches) and keep their notes' ids, so the projection heard and the clips a commit writes
are the same clips with the same voice tags and round-robin turns — not merely the same
notes.

**Determinism.** Follow actions draw from a hash of the session's seed, the track (or
scene) and the tick — no state — so a longer horizon only adds after what was planned, a
performance replays the same, and offline equals real time.

### 7. The conductor, its timing policy, and the audio thread

The conductor (`cantus_ui::session_play`) runs beside the engine handle on the interface's
thread: `EngineHandle::send` keeps `EngineMessage::Session` for it (it never reaches the
audio thread); it plans to a horizon (32 bars on, extended when 16 are left) and a changed
performance is compiled and sent as any edit's snapshot is (`compile_to_play`: the path of
pattern mode and the count-in). The events are told when the playhead reaches them. A stop
or a jump of the playhead ends the performance; a launch from stop starts the transport at
the playhead (and the performance waits for the engine to say it plays before it treats
"not playing" as a stop — not past a stop sent meanwhile, nor past two seconds with no word
from an engine: then the launch from stop is over and the next one starts the transport).
Presses are planned in the order of their ticks, whatever order they were made in (a slot
recording's launch is laid at its boundary, up to a bar ahead, before later presses for
sooner).

**The lead.** A press takes effect no sooner than a lead after the playhead the engine is
rendering (`engine_position`, already the output latency ahead of what is heard): **two
device blocks** (the snapshot is drained at the next block and installed on the grid inside
it), **the 95th percentile of the last 64 compiles of a projection** (plan,
`project_play`, `Snapshot::compile_reusing`, attach; nearest rank — one slow compile among
64 does not set the lead for the next 64, and with fewer than 20 it is their worst; the full
compile of a launch from stop and a compile again after one that ran late are not counted:
they are not what the next press waits for), and a **2 ms floor**. At 48 kHz with a
3 ms compile: 7.7 ms at 64 frames, 15.7 ms at 256, 47.7 ms at 1024 (`lead_s`; the test
prints them). An unquantised launch, a gate's release with quantise off, or a stop with
quantise off is heard **the output latency + the lead + at most one interface frame** after
the press (the frame is the launcher's to shorten: a controller press can wake it at once).

**A compile that runs late.** The press tick is fixed before the synchronous compile; after
it, the conductor estimates where the engine renders now and, if a press of this frame
would take effect before two blocks from there, moves it there and plans and compiles again
before anything is sent (`restamp_late`). So no projection lands after the tick it plays
from: what is planned is what is heard, and what arrangement record writes.

**Launch tolerance.** A quantised launch plays on the first boundary at or after the moment
it was pressed (as heard) less the tolerance — a 32nd note, at most a quarter of a note
grid (on a grid of a 32nd or finer a press is not caught up by a whole step) — even when
the lead or a compile that overran made it miss that boundary: it is caught up by its offset
from where it can take effect (Bitwig's launch tolerance), never a whole bar later. So a
press heard 10 ms before a bar line with a 27 ms lead plays on that bar, 17 ms in; and a
quantised press whose compile overran, planned again past the bar it meant, plays on that
bar. A stop or a release pressed before a boundary it can no longer reach takes effect as
soon as it can; one pressed on a boundary, or after it, waits for the next.

**Following a source** (Link, MIDI clock, time code — ADRs 0078 and 0079). A launch from
stop waits for the source's start — Link's next quantum, the master's Start — as long as it
takes: no timeout, only a stop calls it off (with no source an engine that never says it
plays is given 2 s). It is shown queued meanwhile, and nothing is told heard. A song
position the master sends while it waits moves the launch there; a Start behind where it
was launched (MIDI clock's Start is from the top) plays it from where the transport is
heard, caught up from the start, as a press just after a boundary is. A jump back of the
playhead while a source is followed and no locate was sent — the source's start, a relock
within Link's quantum, the master's song position — is the source's, not a locate: the
performance plays on (and arrangement record is not committed mid-set); a locate the user
sends still ends it. The lead, the frame's advance and the late-compile restamp turn
seconds into ticks at the followed tempo (the transport's ticks go at it): following Link
at 150 over a song at 120, the lead is not a fifth short. Following Link, a launched
scene's tempo is proposed to Link's session as its tick is heard (Live proposes it at the
launch): the transport follows Link's tempo, so that is how a scene's tempo is heard. The
projection carries its scenes' tempos (`Snapshot::scene_tempos`, set by the conductor on
the snapshot it just compiled); the audio thread, each block, proposes those its block
plays through (`Processor::propose_tempo`, timed to when the tick is heard, a no-op
unless Link is followed; a walk of a short list, nothing allocated). A projection installed
again proposes nothing more; the song a performance ends with carries none, so ending
proposes nothing (and the engine no longer proposes on a new map — sync's side).

**Planning on.** Every plan after the first carries on from the last one as it stood before
the lead (`plan_from`): what was planned before the cut — the clips, their phases, the
follow draws, the changes told — is kept; the project as it is now plays from there. An
edit of a follow chance, a loop, a quantisation, a scene's tempo, an undo, or a new press
changes what comes, never what was heard. A clip playing across the cut whose follow time
was shortened below what it already played follows the next time it comes round (a whole
multiple of its follow time from its launch; a pass, for a linked one), not at once mid-bar;
a scene's follow likewise.

**Stalls.** Looping clips (and the tracks the session holds) are open-ended in the
projection: carried 64 bars past the horizon (`open_ended`), so an interface that stalls —
a blocking dialog mid-set — leaves them looping, not falling back to the arrangement at the
horizon. A follow chain does not go on while the interface stalls: its last clip loops, and
when the conductor is back it plans on from what was heard.

**A measured bound** (`cantus-core/tests/session_bench.rs`, this machine, Windows, dev
profile): 128 tracks with a 64-bar arrangement each and four one-bar loops with follow
actions on every one, 16 of them audio — a scene launching all 128 tracks: plan 2 ms,
`project_play` 12–17 ms, compile 29–39 ms, **58 ms worst** (a quantised launch: it has its
bar). One track launched while that scene plays: plan, `project_play` and
`Snapshot::compile_reusing` — which reuses the compiled notes and controllers of every
track that did not change — **3.5 ms, 5.2 ms worst**: a lead of 18 ms at 256 frames.

**The audio thread** runs nothing of the session: launching, stopping and following allocate
nothing there because nothing of them happens there. What it does is play on
**seamlessly**: voices keep ringing across a snapshot and event cursors reseek where they
are (as for any edit); a snapshot arriving while the transport plays is installed on the
automation's control grid (`automation::GRID`, 16 samples; `session_rt.rs`), never between
two of its points, the messages after it waiting with it, so automation and envelopes
played live are bit-identical to the render; the pass lets its own copy of the old snapshot
go before the install, so its last reference leaves with the garbage; and a snapshot that
ends, behind the playhead, a note the old one still had sounding (an edit, or a projection
that came late) ends it at once — before, it rang on. Which notes is worked out where the
snapshot is sent, off the audio thread (`Snapshot::cut_from`, in `EngineHandle::compile`
while playing, against the snapshot sent last, for an install up to four beats on): the
notes the old snapshot had started and not yet ended that the new one ends sooner, matched
by voice tag and start, linear in the events of the tracks that changed. The install walks
that list (`CutNote::due`: started, its new end behind the first tick the cursor plays, its
old end not) — no search, no lookup, nothing allocated: 32 dense tracks, every note cut,
cost an install's 64-frame block about 0.1 ms (the median of nine pairs; the scan it
replaces: 7–8 ms, in the callback). A note pasted behind the playhead was never started by the old
snapshot: no note-off for it (on a hosted instrument that would end a held note of its
pitch). A note the new snapshot no longer has at all is left as an install always left it.
`settle`, which an offline render runs before its first block, installs a waiting snapshot
at once. The session's audio is loaded with every compile (`preload_session`), so a launch
plays its audio from its first projection; and its **warp renders are asked for with every
compile too** (`preload_session_warps`, `edit::warp_ahead`): each warped session clip laid
as a launch lays it — its first pass and one more, at the song's tempo and at its own
scene's — attached ahead and kept as what plays (`Renders::preload`). A window inside one
held tempo renders the same wherever it lies, so the first launch of a warped clip on a
boundary plays its render, not the varispeed bridge; a launch caught up, or under another
scene's tempo, plays the bridge until its own render arrives, as an edit does.

### 8. Recording

**Into a slot**: the record button arms the track and records through the application's
own pass (the count-in, inputs, files, the session journal and its recovery are the
arrangement's), punched from the boundary the press waits for to the bar line after it is
stopped — the record button again, or launching the slot — or after the fixed length, on
the performance's bar lines (its scenes' tempos and metres). When the pass ends its notes
(record-quantised) or its file's window become the slot's clip, one undo step
(`cmd.session.record`). A **MIDI** take plays on seamlessly: a lead and a few frames before
its end it is laid in the projection with the notes played so far and launched exactly on
the end (`Input::Resume` in step with it); the notes of its last moments are at its end, a
pass later, by when the take is whole (it keeps the laid clip's id). An **audio** take plays
on seamlessly too: 0.3 s and a lead before its end, what its file holds so far
(`EngineHandle::growing_takes`: the writer brings the header up to date every half second)
— its beginning, which is what plays at the end — is copied aside (the system's temporary
folder, removed when the performance ends) and laid in the slot as the clip, placed as the
whole take will be (the same window, the same fine offset), launched exactly at the end;
when the file is whole the take replaces it under the same id, the same samples where they
overlap. A take too short to have been written out by then plays on from when its file is
whole, its first moments lost, as before. Over a looping
MIDI clip that plays, each note goes into it where it was heard — in the window of the clip
that played it, whatever follow actions replayed it (overdub). A recording queued for its
boundary goes when the transport stops first. A pass cut short by a crash comes back into
its slot: the journal keeps the slot (`JournalPass::slot`, `PassPlan::slot`).

**Into the arrangement**: with arrangement record on, what was heard from its start to its
end (the button, a stop) is committed — `Performance::within` that stretch, which brings the
tempo and metre a scene set before it, and `edit::commit_performance` — as one undo step.
The projection and the commit both keep unwarped arrangement audio in its file through a
scene's tempo (`tempo::keep`), so they agree. The tests render the recorded arrangement and
the live performance (projections installed as the conductor makes them, at its lead, at
block sizes 128, 441 and 333 with three workers) and compare them bit for bit: follow
actions, legato, an envelope and a scene's tempo; and, in a second, an unwarped audio loop
cut by a stop (its click-free fade), an arpeggiator, a modulator, a scene's tempo heard
through the audio, a Pulsus kit's round robins (a snare's in turn, a shaker's at random) and
a warped loop heard at the song's tempo and the scene's (every compile there waits for its
warp renders: what the cache has made by when is N5's test). A hosted plug-in is not in it:
hosting lives in the crates above the engine (`cantus-host`, the sandbox), which the
engine's tests do not link; recording while launching is the conductor's, tested in
`cantus-ui`.

### 9. Freeze, bounce, stems, tempo edits

A frozen track plays its render: its column is locked and its clips do not launch (the
status line says why). Freeze, bounce and stems render the arrangement, as they always
did; a performance is in the arrangement once recorded into it. A tempo edit keeps an
unwarped session audio clip's stretch of its file in seconds (`tempo::keep` scales its
start marker, loop and end by the change of the tempo at bar 1); a warped one follows the
tempo by its warp. A scene's tempo during a performance is heard through the tempo track,
as any tempo point is.

## Consequences

- The view never needs a model change: what a slot, a scene and the settings show is in
  the model; what plays is in the events.
- An unquantised press is the output latency, the lead (two blocks, the recent compiles'
  95th percentile, 2 ms) and an interface frame late; quantised launches land on their
  boundary, and one pressed just after it, or just before it with a lead that misses it,
  plays on it, caught up.
- A long follow-action chain makes many windows in the projection (one per clip heard up to
  the horizon); compiling them is the cost of an edit, once per change, never per frame.

## What the tests hold

- `cantus-core` `session_tests*.rs`: the model, every command's inverse and refusal, the
  guard, the column checks, envelopes following a clip, the builders; the conductor's plan
  (boundaries, modes, legato, follow actions stable as the horizon grows and seeded, scene
  tempo, back to arrangement), the projection equal to the commit, cut audio fades.
- `undo_invariants.rs`: the session stream (`round33_session_command`).
- `cantus-engine` `tests/session.rs`: a session never launched sounds as the song without
  it (to the bit); a launch lands on its boundary as the same notes laid by hand; a live
  performance at three block sizes and with the pool equals its recorded arrangement.
- `cantus-ui` `tests/session_play.rs`: launch from stop, queued then heard, stopping with
  its position, arrangement record as one step, recording into a slot.
- The timing round: `session_tests4.rs` (chained legato, launch tolerance, planning on from
  a cut, the tempo in force at a late record start, projection = commit through a tempo
  with unwarped audio, frozen tracks, open-ended loops); `session_bench.rs` (the bound
  above); `cantus-engine` `tests/session.rs` (a note cut behind the playhead is ended,
  `settle` with a waiting snapshot, session audio loaded before its launch, the rich
  live = recorded equality) and `rt_memory.rs` (a deferred install with a thread draining
  the garbage); `cantus-ui` `tests/session_play2.rs` (a launch from stop that waits for the
  engine, the lead, a late compile planned again, launching the recording slot, a queued
  recording dropped on stop, overdub across follow actions, a fixed length laid ahead, a
  count-in, audio recovered into its slot).
- The second round: `session_tests5.rs` (a launch before a boundary it cannot reach, the
  tolerance on a fine grid, presses in tick order, a shortened follow at its next turn, one
  tempo per tick in a window); `snapshot_cuts.rs` (a pasted note is not cut, a shortened
  one is, where it was playing); `cantus-engine` `tests/session.rs` (an install cutting 32
  dense tracks, timed; the rich equality with a kit and a warped loop), `rt_memory.rs` (an
  install that ends cut notes), `tests/session_warp.rs` (the first launch of a warped clip
  plays its render); `cantus-ui` `tests/session_play3.rs` (an overrun compile on the bar it
  meant, the compile estimate, a launch from stop stopped before it played) and
  `tests/session_rec_audio.rs` (a real pass: the growing file's beginning is the take's,
  placed alike, and plays what the whole take plays).
- The integrated seams: `cantus-ui` `tests/session_play4.rs` (a launch from stop through
  Link's quantum and through the master's Start, a song position and a Start from the top;
  a relock is no locate, a user's locate is; the lead at the followed tempo; a scene's
  tempo carried by its projection, none by the song after; an MPE take into a slot and an overdub keep
  their expression, cut at the take's end); `cantus-engine` `tests/session_sync.rs` (a
  scene's tempo proposed to Link once, as its bar is heard, nothing for a projection
  installed again or for the song at the end).
- `format.rs` `schema_46_keeps_the_session`; `tests/dawproject.rs`: scenes in and out.
