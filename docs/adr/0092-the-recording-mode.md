# ADR 0092 — The recording mode: prepare to record, the edit lock, the baked master

**Status:** accepted (round 36, record builder; amended after the integrator's gaps and the
branch review) · **Date:** 2026-11-03 · **Builds on:** ADR 0064 (freeze: the render after
the inserts and before width/pan/fader, the content key, the streamed reader, the frozen
guard, constrained compensation), ADR 0050 (streamed renders), ADR 0004 (commands), ADR 0005
(the audio thread), ADR 0022 (what is heard), ADR 0087 (the dancer) · **BACKLOG:** §4c
steps 2–3 (the owner's decisions of 2026-10-10 are binding) · no schema change (the state
is the session's)

## Context

The owner (2026-10-10): *"a partial pre-render of every channel that feeds the master …
each one on its own, just before the master, so while recording audio only audio is played
and summed. Less lag, a smaller buffer … Manual, always clear that you are in it; to change
anything, leave the mode and pre-render again."* His decisions: the recording strips stay
live; the master is *live* (default) or *baked*; the mix controls are *live* or *locked*;
any other change is blocked with a notice; a bar that cannot be missed; re-entering
renders only what changed. Round 29 built the pieces it stands on: freeze and low-latency
monitoring with constrained compensation.

## Decision

### 1. The mode is freeze, applied to every strip that feeds the master

`cantus_core::record_mode::plan(project)`:

- baked: every **strip whose output is the master** (a bus is one render including what it
  sums; a return's render includes the sends into it) whose region has a track, and every
  **track straight to the master** with a clip;
- live, and named with the reason (`Why`): a region holding an **armed** track, a track
  monitoring *On*, a solo inside a bus, a dancer's track, a track that plays enabled
  **outboard gear** (ADR 0094: an offline render cannot play it; freeze refuses it); what the
  user froze is kept. Nothing the plan bakes is one a freeze would refuse, so the estimate
  counts only what is rendered and *stop on failure* stops only for a real failure. A
  render that waits (files loading, r36/ara's pending ARA renders) waits in the freeze
  queue the preparation goes through.

Each planned target is queued on the freeze machinery, so the mode inherits all of it
(renders on a worker, plug-in states taken first, a render on disk for the same sound used
at once, a plug-in that cannot render offline failing that target alone, the CPU of what
only feeds a baked strip freed). With the controls untouched the baked mix is the live mix
**to the bit** — compensated, at every block size, offline and in real time, serial and on
the pool. The mix controls act after the render, so they too give the live mix to the bit.

### 2. The edit lock

`record_mode::refuses(&EditLock, cmd, project)` is put on the `CommandBus`
(`set_record_lock`): every execute, merge, rebase, undo and redo is checked before it
applies (`CommandError::RecordingMode`); `CommandBus::locked_against` is public, and a
controller asks it for each target as it asks the frozen guard, so one locked target leaves
a gesture's others moving. The lock reuses `freeze::touches` (no wildcard: a new `Command`
does not build until classified) and judges **every touch on its own** — a command that
moves several strips (a group, r36/mix's VCA, a snapshot recall) is refused whole when one
of them is locked, with the same notice.

`EditLock` carries the **armed regions**' tracks and strips (`EditLock::new` from the armed
roots, and those brought live, §6). Allowed: what no sound depends on (names, colours,
markers, the loop, the metronome), every touch inside the armed regions (their takes land
there as edits), the master while it is live, a baked root's fader, pan, width, mute and
solo (and their automation: a gain curve) while the mix is live — a modulator's amount is
judged by each route it drives — and **a dancer's own settings**: ADR 0087's dancer is a
silent instrument (its unit only reports the song's position to the screen), so its
settings change what is seen, never a sample. **Everything else is refused**, live or not:
project-wide touches but the master's, un/freezing, pattern mode, everything inside a baked
region, and everything that stays live without being armed (a strip that could not be
rendered, a monitoring or soloed region, a track with no clip) — an edit there would change
what plays under the take.

**A strip inside another baked strip's region keeps its mix locked** — it outputs into a
baked bus, or sends post-fader into a baked return: that render holds its fader. **A
pre-fader sender keeps its fader and pan live** (`freeze::pre_only_senders`: a strip whose
only way into the region is pre-fader sends — its output elsewhere, no insert of the region
keyed from it, no modulator there listening to it): a pre-fader send taps before the fader
and the pan law, so they are nothing the region hears. The freeze subject leaves that
strip's `gain_db` and `pan` (and their automation and modulation) out of the region's key,
and `Locks::level` (a new `Touch::StripLevel` for `SetStripGain`/`SetStripPan` and their
automation) leaves them unlocked; its width, mute, sends and route stay locked (they shape
or silence the send). The subject of every region without such a sender is byte for byte
what it was — no `SUBJECT_VERSION` bump, the renders found again: the fixtures' frozen keys
are checked against round 35's (`cantus-project/tests/freeze_keys_r36.rs`). Null test: the
return baked, the sender's fader and pan moved and unmoved, the mix is the live one to the
bit. The question before the mode says how many strips keep a locked mix.

The interface says every refusal as *"Recording mode: leave it to edit"* (no freeze card,
no *Unfreeze*).

### 3. The master: live or baked

- **Live** (default): the master chain runs on the sum.
- **Baked**: once the strips' renders are in, one more render — the master's output as an
  export makes it (compensated), with what stays live muted and no solo, made from the
  frozen project (it reads the strips' renders: quick) — keyed by SHA-256 of that muted
  project (`record_mode::master_key`; every byte, so anything that could change its sound
  changes the key; a rename renders it again). `FrozenStore::set_master` attaches it to
  every snapshot (`Snapshot::master_frozen`); `record_mode::rest_for_master` rests every
  node a freeze holds and takes the master's chain out of the snapshot. The master stage
  (`graph/master_run.rs`, marked) gathers the live strips as ever, gives them the master's
  (locked) fader, pan and polarity, and **adds the render read at the output's latency
  behind the transport** (`engine::record_mode::MasterBake`, allocation-free, its own
  scratch). With nothing live the output is the live master's **to the bit** (tested with a
  look-ahead limiter and a master fader, at blocks 512, 64 and 1); with an armed track live,
  the reference alone (the armed track muted) is the live mix without it, to the bit. A
  baked master locks the master's controls and the mix's. A master render that fails
  leaves the master live, said.

### 4. Low-latency monitoring is forced on, and said

While the mode is on, `freeze::apply_low_latency` sends the user's limit whether or not the
switch is on: armed strips are monitored without waiting for latent plug-ins elsewhere
(constrained compensation), the take placed to the sample. The record popover and the
inspector say *forced by the recording mode*, and the bar names the inserts it leaves out.

### 5. The state is the session's: never an undo step

The mode's freezes, its leave and a strip brought live are applied with
`CommandBus::apply_session`: the command runs through the frozen guard, the revision moves
(the engine follows), but **no undo step is made and the project stays as clean as it
was**. We chose this over one folded undo step: the mode changes nothing the file holds
(`as_saved` thaws its targets in every copy written — save, autosave, template), so a step
for it would be an undo of something never saved, and undoing it while the mode runs would
leave the mode without its renders. So: entering adds no step, leaving adds none, an undo
after leaving undoes the user's last edit and never freezes anything again; while the mode
is on, undo and redo are judged by the lock. A crash reopens out of the mode; another
project in place ends it and takes the lock off (`CommandBus::reset` clears it too).

### 6. Arming during the mode

Arming a track whose strip is baked is refused by the lock and **offered** instead: *bring
its strip live to record on it*. Accepted, only the baked renders that hold that track are
unfrozen (a bus with what it sums, a return the track's strip sends into) — off the
history, no render made again: their live chains resume and the compensation aligns them
to the sample (the mix is the live one to the bit, tested) — they join the lock's armed
regions, the bar's counts follow, and the arm is the one undo step. An agent's arm (its call
already allowed) is brought live at once. A baked master holds every strip: bringing one
live makes the master live again (its strips from their renders), said in the offer.

### 7. What moves the tempo without a command

Link, MIDI clock and time code follow the tempo in the engine, and a session scene plays its
own tempo: renders would then play at another speed. The mode refuses to start while a sync
source is followed or a session plays (the question says why), and if one starts during the
mode, the take is stopped and the mode left, said.

### 8. On screen

- **The bar** under the toolbar while the mode is on or preparing: the lamp, the chip
  *RECORDING MODE*, *Master baked* when it is, strips baked, the buffer, **the round trip
  labelled for what it is** (*as the driver reports*, or *measured*: `record_mode::round_trip`
  is the one seam r36/external's latency ping wires a measured figure into), the CPU before
  and now, how many stay live, *Monitoring without: …*, and **Leave** (**Cancel** while
  preparing).
- **The question before it starts**: what is rendered, renders on disk, **up to** how much
  disk (each render the song through its tempo map plus the longest tail a render waits
  for, 30 s), what stays live and why, the master live/baked, the mix live/locked (and how
  many strips keep a locked mix), on failure stay live/stop, and the honest sentence (it
  does not shorten the interface's round trip; it lets the buffer go lower).
- **Leaving**: keep (default) or discard the renders, or stay. **The offer** (§6). The
  record popover's row. Dialog text in the kit's prose.

### 9. Agents

`prepare_to_record` (`master`, `mix_controls`, `on_failure`, `dry_run`), `get_recording_mode`
and `leave_recording_mode`, in `ai_tools_record.rs`.

### 10. What it buys, measured

`docs/round36/record.md`: 64 tracks with four effects each cannot play live on the test
machine at any buffer (serial and on the pool); baked, a few per cent of each block's
budget, the master baked less again. In one-minute real-time runs (the bench thread in
MMCSS as the callback is, paced at the device's rate, the readers never waited for) with
64 and 128 streams of 64-second songs at 32–1024 frames: **no disk underrun**. The device's
lowest stable buffer is the owner's to measure.

### 11. The stream reader for many streams

The first real-time run measured one underrun (64 streams, 256 frames). Its causes, and
what was changed:

- **The benchmark's warm-up ran unpaced** in real time: the playhead went a second ahead
  of the reader in a few milliseconds. It is paced now (and stopped a moment first, as a
  musician does, so the reader reads where play will start).
- **The reader's order beyond its own look-ahead was stream by stream**: wants were sorted
  by a priority capped at the look-ahead (3 blocks), so blocks 3–19 of the first stream
  came before block 3 of the last; with 128 streams the last streams' next block came too
  late and the run never caught up at 32 frames (342 M frames late). Wants now sort by
  class, then **distance**, then stream: every stream's next block before any stream's
  later one; duplicates are removed by set (the nearer want kept).
- **The reader thread runs above normal priority** (`cantus_rt::thread::raise_io_thread`:
  Windows `THREAD_PRIORITY_ABOVE_NORMAL`, Linux a nice of −5 when allowed), never in MMCSS
  (it blocks on its reads).

No ring depth by stream count was needed: the pool grows by chunks as wanted (up to 16 384
blocks) and a frozen reader already asks the most a cursor can (19 blocks, 1.6 s at
48 kHz); reads stay one block each, nearest first, which the runs show is enough at 128
streams. The offline-paced runs' spike was the offline reader waiting for a block: the
benchmark counts those waits per block (`freeze::offline_waits`), and the slowest block of
a run is the one that waited (the slowest that did not wait stays under 10 % of the budget
at 512 frames).

### 12. The mix cues and snapshots (ADR 0093) with freeze and the mode

The integrated tree's review found how r36/mix's cues met freeze; each is closed:

- **A cue's switches on a strip playing its render.** `freeze::settle` now neutralises, in
  every region of `Snapshot::mix_scenes`, a playing frozen strip's polarity, swap, inserts'
  switch, bypasses and EQ (they are in the render) and keeps its silence (mute and solo act
  after the render) with a solo's silencing; a token flips in its (unused) bypass list
  where its switches changed, so the cue's 16-sample fade, which acts after the render as
  live, still comes there. Null test: a frozen strip with cues that flip and swap it, move
  its EQ and mute another = the unfrozen mix, to the bit, at 512 and 64 frames.
- **A cue's solo in a render.** Renders are made unsoloed (`freeze::unsolo`); a cue's solo
  is now taken out of the snapshots for a render too.
- **The baked master and the live strips.** `record_mode::muted` takes the live strips out
  of every snapshot a cue recalls and every solo out of them, so no cue brings a live strip
  back into the master's render (it was there twice, +6 dB). Null test with a cue that
  raises, unmutes and solos the armed strip.
- **The master's hosted plug-ins.** The master's render waits for its plug-ins' render
  instances (`freeze::units_for`, from the states the project stores) and renders with
  them (`MasterJob::run_with`); one that cannot be had (not installed) or that stops in the
  render keeps the master live, named. Tested with the test CLAP gain on the master (the
  baked master is 6 dB down) and with a plug-in not installed.
- **Cues in the keys.** A region's subject holds the cues that reach it (`cue_part`: each
  cue's snapshot entries for the region's strips and the VCAs governing them, as the region
  hears them — a root's chain switches only, a pre-fader sender's without fader and pan,
  no solo; the master's when the region is the whole mix), written only when one does:
  every other key is unchanged. Adding, changing or removing a cue, retaking a snapshot a
  cue plays and making a strip safe from cues of everything are applied and every frozen
  key checked (`Touch::Global` with what they change): refused when a key would move, and
  refused in the mode as project-wide edits are.
- **A desk recall of faders.** A recall (or a solo-safe switch) that changes only faders,
  pans, widths, mutes and solos is a mix control: it passes with the mix live (judged strip
  by strip) and is refused with it locked.

- **A cue that solos a strip inside a region.** A render holds no solo, so it could not
  follow it. ADR 0064 §3 refuses to freeze a region with a strip soloed inside (and locks
  those solos after); cues take the same rule, chosen for that consistency over playing
  the region live while the cue is in force — a render that is never used would cost a
  preparation for nothing and hide the cue's effect behind a mode switch at its bar.
  `freeze::cue_solo_inside` finds the strip (any strip of the region but the root, or a
  VCA governing one, within the cue's scope; a safe strip sits out a cue of everything):
  a user's freeze is refused (`CannotFreeze::CueSolo`, *"a mix cue solos … inside it: take
  the solo out of the cue first"*), the recording mode leaves the region live
  (`Why::CueSolo`, named in the question, out of the estimate), and a solo of a strip
  inside stays in the region's key, so a cue edit that would add one to a frozen region is
  refused. A cue's solo of the frozen root itself is live (it acts after its render). Null
  test: a cue that solos the lead (inside the reverb return's region), the mode entered —
  the output is the live mix to the bit across the cue, at 512 and 64 frames.

## Consequences

- No undo step for the mode; the arm that brings a strip live is one.
- A strip feeding another baked strip by its output or a post-fader send keeps its mix
  locked; a pre-fader sender keeps its fader and pan (§2).
- The renders share freeze's folders, keys and pruning.

## What the tests hold

- `cantus-core/tests/record_mode.rs`: the plan; the lock (project-wide, inside a baked
  region, structure, un-freezing, pattern mode refused; names, metronome, armed track,
  master allowed); everything live but unarmed locked (a monitoring track, a track with no
  clip); mix live/locked; a modulator's amount by route; a batch refused whole; the mode
  never in the history, the project clean, an undo after leaving freezing nothing.
- `cantus-engine/tests/record_mode.rs`: the whole plan baked is the live mix to the bit at
  blocks 1–1024; live controls; an armed track live; pool = serial; a changed strip changes
  only its own key; the armed voice heard as soon as live; **the baked master = the live
  master to the bit**; an armed track over it; the master key.
- `cantus-engine/tests/rt_memory.rs::the_recording_mode_never_allocates`: renders, low
  latency, recording, a loop, the master baked and back, leave and enter — zero allocations.
- `cantus-ui/tests/record_mode.rs` (17): the mode end to end through the frame's path, the
  notice, locked controls, an armed track, leave and re-enter with nothing rendered,
  discard, no undo step, an undo after leaving, the baked master end to end (bits, locked
  master, disk said ≥ written), the offer and bring-live (counts, bits, one step), sync
  refusal and leaving when sync starts, forced low latency, an agent's arm, a file never
  frozen, another project, the agent's three tools, both languages.
- `cantus-engine/tests/record_mode_bench.rs` (by hand).
