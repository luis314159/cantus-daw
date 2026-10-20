# ADR 0064 — Freeze, bounce in place, and low-latency monitoring

**Status:** accepted (round 29) · **Date:** 2026-10-19 · **Amends:** ADR 0031 §4 (the render
cache: built here as freeze, keyed by content), ADR 0025 §7 (a take's latency is the one at
its start, not at its finish), ADR 0022 (a second kind of bypass: hard, for monitoring) ·
**Extends:** ADR 0050 (a frozen render streams on its pool and reader), ADR 0063 (the
plug-ins of a frozen chain are unloaded), ADR 0004 (an edit lock in `Command::apply`)

## Context

The owner's ask (2026-10-16, BACKLOG §4c steps 1 and 2): *freeze tracks, bounce in place,
and low-latency monitoring while recording — freezing frees CPU and unfreezes without
loss; bounce in place to audio with or without effects; an armed track is monitored
without the latency of other tracks' plug-ins.* §4c's own design: the render taken **after
the inserts, sends and effect automation and before the fader and pan**, keyed by a hash of
everything that makes the sound, streamed from disk, an edit that would change it blocked
with a notice, *every `Command` classified … and a new one fails the build until it is*.
It is the first two layers of §4c's recording mode (step 3, later), and the cache §4b's
automatic background freeze will share.

The references: Ableton (*Freeze* / *Flatten*, frozen tracks still mixable), Bitwig
(*Bounce in place*, with or without the chain), FL Studio (*Render to audio*, *Freeze* in
the mixer), Cubase (*Freeze* with tail; *Constrain Delay Compensation*), Logic (*Freeze*
pre / source only; *Low Latency Mode*), Studio One (*Transform to audio*; *Low latency
monitoring*), Pro Tools (*Commit*).

## Decisions

### 1. What freezes (schema 37)

- **A mixer strip** (`MixerStrip::frozen`): everything it gathers — the tracks into it, the
  strips sending or outputting into it, recursively — its polarity, swap, inserts and EQ.
  The render is the strip's pair **just before its width** (`graph/strip_run.rs`): width,
  pan, fader, mute, solo, the sends' levels and taps, where it goes, and every automation
  and modulation of those stay **live**. Pre-fader sends tap after the width, so they stay
  correct.
- **A track's source** (`Track::frozen`): its instrument and clips, before its own level
  and balance (Logic's *source only*); level, balance, mute, solo and its strip stay live.
- **Freeze on a track** freezes the strip it feeds when that strip is its own (no other
  track feeds it — Ableton's track freeze: instrument and devices), otherwise its source
  (`Project::freeze_target_of_track`). The strip menu freezes the strip.

`Frozen { content }` stores the target's **content key** and nothing else; the model change
is schema 37 (`migrate_36_to_37` writes nothing: a v36 strip or track plays live; an older
build refuses v37). A loaded file whose stored key is not what the target holds (a hand
edit) is **re-keyed**, never unfrozen (`freeze::repair`, after the links' repair).

### 2. What makes the sound: the subject and its keys

`cantus_core::freeze::region` is a target's upstream to a fixed point: the strips and tracks
gathered (by output or send, enabled or not — enabling one would change the sound), the
**strips its inserts are keyed from** (and theirs), the **tracks whose notes its inserts
hear**, and what the **modulators driving anything in it** listen to (a follower's strip,
an envelope's notes; a follower of the master puts the whole mix, master included, in).

The **subject** (`freeze::subject`) is the canonical JSON of the region: the frozen strip
without its live controls, the other strips and tracks whole but for what is never heard
(names, colours, heights, docks, groups, panels shown, solo, where a plug-in runs — ADR
0063 isolation is bit-identical), the automation clips and modulators that reach it (wherever
they sit), the audio sources its clips play (path and SHA-256 when known), the tempo
map, and the pan law when a non-root strip's pan uses it. A clip's warp (ADR 0061) is part
of its `AudioRegion`, so it is in the subject; a freeze's or a bounce's render compiles
through `compile_for_render`, which waits for the warp renders. Objects are written with their
keys **sorted** (a removed key must not reorder the rest — the undo fuzz found that one).
The **content key** is SHA-256 of it, lower-case hex (`sha2` and `serde_json` join
`cantus-core`'s dependencies; both MIT/Apache-2.0, already in the workspace's notices).
`SUBJECT_VERSION` changes it on purpose.

The **render key** (`cantus_engine::freeze::render_key`) adds the rate, `DSP_VERSION` and
the file format: the same sound at the same rate from the same build is the same file,
for any project. Files are `<render key>.cfrz` — ADR 0050's cache-file layout (every block
checksummed; a damaged block is never played), labelled with the key and the latency the
frames arrive after — in the **project's `Freeze` folder** (beside its file, as `Audio`)
when it has one, else **Cantus's cache** (`<data>/cache/freeze`), and looked up in both.
They are **regenerable**: *Collect* does not copy them; a project opened without them (on
another computer, at another rate, after the format changed) plays **live** while they are
made again in the background (`freeze::scan`, one at a time), then plays them.

An audio source changed on disk *outside* Cantus after freezing is not noticed (the key
holds its path and hash): the frozen render plays what was frozen — as in the other DAWs —
until it is unfrozen.

### 3. The edit lock

`Command::apply` runs every command through `freeze::guard` (`freeze_guard.rs`). Each
command is **classified by what it touches** in one `match` with **no wildcard arm** — a
new command does not build until it is classified (§4c's rule, enforced by the compiler):

- `TrackSource` (instrument, clips, notes, input, arming), `TrackMix` (level, balance, mute,
  solo, place), `StripChain` (polarity, swap, inserts, EQ), `StripMix` (width, pan, fader,
  mute, solo, sends, output, place), `StripInput` (a route into a strip made or taken), an
  automation or modulation `Target` (mapped to the above), or `Global`.
- The **locks** of the frozen targets: a frozen strip locks its region's chains and the
  mixes of every region strip but its own; its tracks' sources and mixes. A frozen track
  locks its source only.
- A touch inside a lock is refused with `CommandError::Frozen(target)` before anything is
  applied. **Global** commands (the tempo and the metre, the pan law, a track or strip
  inserted or removed, a whole project replaced, the master — in a region only through a
  follower) are applied and **checked**: if a frozen key moved, the inverse is applied and
  the command refused.

The invariant — *after any command, applied or refused, every frozen target's stored key is
its content key* — is what the undo fuzz checks after every one of its commands (arm 47
freezes, unfreezes and tries wrong keys; the other 47 arms run against the frozen
regions). Freezing itself is validated: a `SetStripFrozen`/`SetTrackFrozen` with a key that
is not the target's is refused. Freezing **disarms** the region's tracks in the same undo
step (a take could not land on a frozen track); a track that monitors its input *On*
cannot be frozen (a render cannot hold a live input; the notice says which track).

The interface says a refusal on the status line and in the corner card with **Unfreeze**
(`views::freeze_view::card`).

**Solo.** A track soloed elsewhere silences what a frozen strip's render holds: a frozen
strip that plays its render is muted while any track is soloed (the tracks inside it cannot
be soloed: they are locked). Strip solo is the strips' own, as live.

**Pattern mode** plays every chain live (`pattern::audition` thaws): a render is the song's.

### 4. Playing a render

`EngineHandle::compile` attaches the renders at hand (`FrozenStore::attach`) and settles the
snapshot (`freeze::settle`):

- A frozen strip that plays its render **loses its chain in the snapshot** (inserts, EQ,
  polarity, swap — the render holds them) and **gathers nothing** (`graph::plan`: inputs
  into it are dropped). In `run_strip` it reads its render's frames at the transport into
  its pair; width, pan, fader and the rest follow — the same arithmetic as live.
- **What rests** (`freeze::dormant`): every node whose sound only reaches a playing frozen
  strip, unless something live needs it — an insert keyed from it or hearing its notes on a
  strip that runs (or the master), or a modulator driving something live that follows it
  or hears its notes — and then what *that* node gathers runs too. A resting node is not
  run (`TaskMap::rest`, `run_node` returns at once), delays nothing (`compensate` skips it),
  and its meters read zero. Its hosted plug-ins are **unloaded** (`PluginHost::sync` and
  the snapshot drop them; ADR 0063: an isolated one's helper ends when empty; the state is
  parked and the project holds it — the freeze writes the live states into the project in
  its own undo step).
- A frozen track plays its render in place of its instrument and clips; its notes still go
  to the inserts that hear them.
- **The latency is kept.** Frame *f* of a render is what reached the tap at the transport's
  frame *f* — the region's compensation and the chain's latency included — and the frozen
  strip (or track) reports that latency as its own. Every frame then meets its fader, its
  pan and their automation at the sample it did live, so the mix is the live mix **to the
  bit, before compensation too** (trimming it would have moved the fader's automation
  against the music by the chain's latency: the first design did, and the null test caught
  it). Freeze frees CPU, not latency; §4c's recording mode may trade that later.
- **Streaming** (ADR 0050): a render is a stream (its head in memory, its blocks in the
  shared pool); a frozen node's board points the reader at the playhead with the most a
  cursor asks (sixteen blocks, about 1.4 s at 48 kHz) and keeps the loop's start ready in a
  second cell; stopped, it points where play will start. Allocation- and lock-free on the
  audio thread and the workers (`rt_memory::frozen_playback_never_allocates`). In real
  time a block not there yet is silence, counted (`stream_late`); offline (export, stems,
  a freeze or bounce render: `Processor::set_offline`) the pass waits for it.
- Stems: a strip or track inside a frozen strip has no stem of its own while it is frozen —
  its sound is in the frozen strip's.

### 5. The render

`cantus_engine::freeze::render` drives the processor offline, as `render` and `stems` do,
with a tap where the frozen sound ends (`StemTap::with_pre`: a strip's pair before its
width, a track's source before its level) and **everything outside the region resting**
(`freeze::isolate`), so only what makes the sound is computed. It runs from tick 0 to the
song's end (or the loop's, if later), then on until the tap has been **exactly zero for one
second** (denormals are flushed: a tail ends in zeros) or for 30 s; the frames kept end at
the last one that was not zero. Frames go to a raw file as they come and the render file is
written from it in blocks — never whole in memory. Hosted plug-ins render through
`render_units` / `render_instruments` (fresh instances, where they run live; isolated ones
wait without a deadline, ADR 0063).

**Render first, freeze after.** *Freeze* renders on a worker (the corner card shows how
far, with **Stop**: nothing changes); when it is done the freeze is one undo step. An edit
made while it rendered moves the key: it is rendered again for what it holds now. One
render runs at a time, never beside an export (their hosted instances are one set).

### 6. Bounce in place

On a **new track right under the original**, at the same place (Ableton's, FL's), the
original **muted** — or removed, the user's choice, remembered (`Settings::bounce_removes`):

- *Bounce in place*: this track alone through its strip's chain up to the width (the
  strip's other inputs resting, its keys and note sources running); the new track feeds a
  **new strip** taking the old one's width, pan, fader, mute, output and sends (Bitwig's
  *post-chain, pre-fader*: the fader stays a mixing control; its automation is not copied).
- *Bounce in place (source only)*: before the track's level; the new track takes its level
  and balance and **feeds the same strip** — the song sounds the same, to the bit.
- Ranges: the whole track, the **selected clips** (only those render; they are the ones
  muted or removed), or the **time selection** (the clips it crosses cut at its edges;
  the transport stops at its end and the tail rings out). The strip menu bounces a whole
  strip (everything it gathers; the original strip muted).
- The latency is **trimmed** (frame 0 is the range's first tick: the new audio track has
  no latency of its own); the tail is kept as for a freeze; the audio clip is rounded up a
  tick so it never ends before its file. 32-bit float WAV in the project's `Audio`
  folder. One undo step. A track a freeze holds is refused (the notice says to unfreeze it
  first).
- *An instrument track becomes an audio track when asked*: bounce with *remove the
  original*.

### 7. Low-latency monitoring (§4c step 2)

A **session setting** (`Settings::low_latency`, `low_latency_ms` — no schema change: it
belongs to the machine and the moment, not the song), a switch where a recording musician
looks (the record options, the audio chip's popover, the inspector's recording rows) and a
key (`Alt+Shift+R`). Default limit **3 ms**: above Tonus's 2.7 ms (the voice's own tuner
stays), below every linear-phase or look-ahead mastering unit. Engine:
`EngineMessage::SetLowLatency(Option<u32>)`, planned in `Processor::compensate`
(`processor_monitor.rs`):

- **Monitored**: a track whose input monitors *On*, or *Auto* and armed — the rule that does
  not change when recording starts, so nothing re-plans under a take. The **monitored path**:
  its strip, what that strip feeds (outputs and sends), the master.
- **Hard bypass**: an insert on the path (the master's included) whose latency is above the
  limit is not run, has no latency and no dry line (`ll_off`) — unlike ADR 0022's soft
  bypass, on purpose: the point is the latency. They are named on screen (*Monitoring
  without: Aequalis, Culmen*) from `EngineStats::low_latency`.
- **Constrained compensation** (Cubase's): a strip that carries **monitored input only**
  (the singer's own strip, a bus of monitored strips) is not held back — its output and
  sends want no delay and do not set anyone's arrival — while the backing, aligned among
  itself, sets the master's. **A bus that mixes monitored input with backing keeps the
  backing aligned**: the monitored input waits there with it (to hear yourself soonest,
  monitor through a strip of your own — the manual says so). The armed track's own clips
  go the monitored way too (heard early while monitoring, as Ableton accepts).
- **The take is placed to the sample**: what is recorded is the raw input, placed by the
  backing's latency — **captured when the pass starts** (`RecordMark::Started`), not read
  at its finish (this amends ADR 0025 §7): a mode switched, or a bypass restored on stop,
  must not move the take. The interface places takes by `EngineHandle::pass_latency`.
- **The audio chip** shows the monitored path's round trip while it is on: the driver's,
  the input ring's wait and the monitored chain's (`LowLatency::monitored_samples`).

## Measurements (Windows, the machine shared with other builds; best of five)

- **CPU freed**: a strip with a heavy Wavetable (32 voices, thick settings) and two inserts
  — 300 blocks of 512 frames played serially: **2.477 s live, 0.203 s frozen (8 %)**
  (`freeze::what_only_reaches_a_frozen_strip_is_not_run`, asserted below 60 %).
- **Low-latency monitoring**: a voice through Tonus with a linear-phase EQ on another
  strip: heard **128 samples (2.7 ms) late instead of 4 608 (96 ms)**; with an EQ on the
  voice's own strip too, it is bypassed and named, still 128.
- **Take placement**: identical, to the frame, with the mode on and off, and when the mode is
  switched off mid-pass (the master's look-ahead limiter returning) — without the capture
  at the start the take was 330 samples off (seen failing).
- Real-time streaming at twice real time: no late block; the output equals the offline
  render.

## Tests (what each holds)

- `cantus-core/tests/freeze.rs` (12): the key's form and stability through the file; it
  changes for what changes the sound and for nothing else (fader, pan, width, mute, solo,
  output, names, colours, others); freeze/unfreeze one undo step each, exact; a wrong key
  refused; a frozen strip refuses inside its region and names itself, allows its live
  controls and everything outside; a frozen track keeps its level live; freezing disarms
  in the same step and refuses a live input; freeze on a track picks strip or source; what
  rests and what a live insert wakes; settling a snapshot; solo; pattern mode; modulators.
- Undo fuzz arm 47 and the driver's invariant check after every command (all seeds).
- `cantus-project`: `schema_37_keeps_frozen_strips_and_tracks`, a hand-edited key repaired.
- `cantus-engine/tests/freeze.rs` (13): the frozen mix is the live mix to the bit with its
  fader ride, pan, width, send level and mute moved after freezing; unfreezing gives the
  live chain back to the bit; block sizes 1, 37, 64, 512; a frozen track's source with its
  level and balance live; a latent insert frozen away (compensated and not); no render →
  live; the render key and streaming (only the head in memory); the tail rings out;
  cancelling writes nothing; pool = serial at W 1 and 3; real time = offline with the reader
  ahead; the CPU freed.
- `cantus-engine/tests/low_latency.rs` (3), `fx_tonus` (the old test kept for *off*, a new
  one for *on*), `rt_memory::{frozen_playback_never_allocates,
  low_latency_monitoring_never_allocates}` (serial and pool).
- `cantus-sandbox/tests/freeze.rs`: a frozen chain with the hosted test gain (in process and
  isolated) renders through `render_units`, is unloaded, plays the live mix to the bit, and
  comes back thawed.
- `cantus-ui/tests/freeze.rs` (8): Freeze renders then freezes in one step, the mix equal;
  a refused edit's card with Unfreeze; Stop changes nothing; an edit during the render is
  rendered again; reopened elsewhere it plays live and remakes its render; both bounces
  lay the audio under the original and the song is the same to the bit; selected clips;
  low latency's key reaches the engine; both languages. `tests/freeze_boot.rs`: the boot
  path (`assemble`) sets the folder and tells the engine.

## Consequences

- Every new `Command` must say what it touches (`freeze_guard::touches`) — the build fails
  otherwise. Round 29's controllers builder's commands get theirs at the merge.
- `cantus-core` depends on `sha2` and `serde_json`.
- §4b's automatic background freeze can reuse the key, the store and the render as they
  are; §4c's recording mode is these two layers held as a mode.
- Not done, by decision: a frozen track's render does not follow a later tempo change
  (the tempo is locked while anything is frozen); hosted plug-ins that render differently
  offline than live would make a different render (none of ours do; ADR 0063's isolated
  renders are bit-identical).
