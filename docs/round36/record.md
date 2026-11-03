# Round 36 — record: the recording mode (ADR 0092)

Builder **record**, branch `r36/record` (r36/integration with r36/mix merged in). BACKLOG §4c
steps 2–3, built on freeze (ADR 0064).

## What was done

- **The plan** (`cantus-core/src/record_mode.rs`): every strip whose output is the master
  (a bus with what it sums, a return with the sends into it) and every track straight to the
  master is baked; a region with an armed track, a track monitoring *On*, a solo inside a
  bus or a dancer stays live and is named (`Why`); what the user froze is kept.
- **The edit lock** on the `CommandBus` (`set_record_lock`, public `locked_against`): every
  execute, merge, rebase, undo and redo checked before it applies (`RecordingMode`). It
  reuses `freeze::touches` (a new `Command` fails the build until classified), judges every
  touch alone, and carries the **armed regions** (`EditLock::new`): everything outside them
  is refused — baked or live-but-unarmed (a failed render, a monitoring/soloed region, a
  track with no clip) — but cosmetic edits, a dancer's settings (silent, ADR 0087), the
  master while live, and a baked root's fader/pan/width/mute/solo while the mix is live.
  A modulator's amount is judged per route. **VCA** (r36/mix, ADR 0093): a VCA's fader,
  mute and solo are a mix control, allowed while the mix is live and every member may move
  (a member whose mix another baked strip holds refuses it whole). **Desk recalls and cues**
  are judged per strip they reach; one reaching a baked strip is refused whole, said in its
  own words ("this recall reaches baked strips … or recall only the live strips").
- **The baked master** (the owner's §4c choice): after the strips, one more render — the
  master's output, compensated, with what stays live muted — keyed by SHA-256 of that muted
  project (`record_mode::master_key`), attached by `FrozenStore::set_master`
  (`Snapshot::master_frozen`); everything a freeze holds rests and the master chain leaves
  the snapshot (`rest_for_master`); the master stage adds the render, read at the output's
  latency behind the transport, after the live strips (`engine::record_mode::MasterBake`,
  allocation-free). Master and mix controls locked while baked.
- **Arming during the mode**: the arm is refused and offered — *bring its strip live*: only
  the baked renders holding the track are unfrozen (no render made again; their chains
  resume, compensated to the sample), they join the armed regions, the bar's counts follow,
  the arm is the one undo step. An agent's arm is brought live at once. With the master
  baked, the master plays live again (said).
- **Never an undo step** (`CommandBus::apply_session`): the mode's freezes, its leave and a
  strip brought live are session state — no history, the project stays clean; an undo after
  leaving never freezes anything. Reasoned in ADR 0092 §5.
- **Session state**: `as_saved` thaws the mode's targets in every copy written; a crash
  reopens out of the mode; another project ends it; `CommandBus::reset` clears the lock.
- **Tempo moved outside commands**: refused to start while Link/MIDI clock/MTC is followed or
  a session scene plays; if one starts during the mode, the take stops and the mode is left.
- **Low-latency monitoring forced on**, said as *forced by the recording mode* in the record
  popover and the inspector; the bar names the inserts it leaves out.
- **On screen** (`views/record_bar.rs`): the bar (chip, *Master baked*, strips baked, buffer,
  round trip labelled *as the driver reports* or *measured* — `record_mode::round_trip` is
  the one seam for r36/external's ping —, CPU before/now, live count, *Monitoring without*,
  **Leave**/**Cancel**); the question (plan, renders on disk, **up to** the disk — the song
  through its tempo map plus the 30 s tail bound —, what stays live, master live/baked, mix
  live/locked with how many strips keep a locked mix, on failure stay live/stop, the honest
  sentence); leave (keep/discard/stay); the offer; the popover row. Dialog text in kit prose.
- **A pre-fader sender keeps its fader and pan** (ADR 0092 §2): `freeze::pre_only_senders`,
  `Touch::StripLevel`, `Locks::level`; the freeze subject leaves those two out of a region
  only where such a sender is, so every other key is unchanged (checked against round 35's
  keys of the fixtures: `cantus-project/tests/freeze_keys_r36.rs`).
- **Outboard gear** (r36/external, ADR 0094): a strip or track that plays an enabled
  external unit stays live (`Why::External`), named in the question, never planned.
- **The stream reader for many streams** (ADR 0092 §11): every stream's next block before
  any stream's later one; the reader above normal priority outside MMCSS
  (`cantus_rt::thread::raise_io_thread`).
- **Agents**: `prepare_to_record` (`master`, `mix_controls`, `on_failure`, `dry_run`),
  `get_recording_mode`, `leave_recording_mode`; `tools/list` golden regenerated.
- ADR 0092; manual `docs/manual/{en,es}/recording-mode.md`; locale area `record`; BACKLOG §4c.

- **The mix cues with freeze and the mode** (ADR 0092 §12, the integrated review): a cue's
  switches neutralised on strips playing their renders (its fade kept), cue solos out of
  renders, live strips out of the baked master's cues, cue data in the keys it reaches
  (cue edits, retakes of a cued snapshot and recall-safe applied then key-checked), desk
  recalls of faders as mix controls, the master's hosted plug-ins rendered into the baked
  master (or the master kept live, named).
- **A cue that solos a strip inside a region** (ADR 0092 §12): never frozen —
  `CannotFreeze::CueSolo` for a user freeze (as a solo inside is, ADR 0064 §3), `Why::CueSolo`
  in the mode (live, named, out of the estimate); a cue edit that would add one to a frozen
  region moves its key and is refused. Null test across the cue's solo, mode entered.
- **Spanish wording**: *pre-renderizado(s)* and *máster* in the bar, the questions and the
  manual (no *horneado*).

## How it was tested

| Where | What |
|---|---|
| `cantus-core/tests/record_mode.rs` (12) | plan; lock (project-wide, baked region, structure, unfreeze, pattern refused; cosmetic, armed, master allowed); live-but-unarmed locked (monitoring track, track with no clip); mix live/locked; mod amount by route; batch whole; never in the history, project clean, undo after leaving; VCA per member (live, locked, a locked member); a desk state reaching a baked strip refused, one of a live strip allowed |
| `cantus-engine/tests/record_mode.rs` (9) | whole plan baked = live mix to the bit, blocks 1–1024; live controls; armed live; pool = serial; a changed strip changes only its key; armed voice heard as soon as live; **baked master = live master to the bit** (limiter, master fader; blocks 512/64/1); armed track over the baked master; master key |
| `cantus-engine/tests/rt_memory.rs` | `the_recording_mode_never_allocates`: renders, low latency, recording, loop, baked master and back, leave/enter; serial and pool: 0 allocations |
| `cantus-ui/tests/record_mode.rs` (18) | end to end through the frame's path; notice, no freeze card; locked controls; armed track; leave and re-enter with nothing rendered; discard; no undo step; undo after leaving; baked master end to end (bits, locked master, disk said ≥ written); offer and bring-live (counts, bits, one step); sync refusal; leaving when sync starts; forced low latency; agent's arm; recall refused and said; a saved file and an autosave never frozen; another project; agent tools; both languages |
| `cantus-ui/tests/ai_mcp_boot.rs` | golden with the tools |
| `cantus-engine/tests/record_mode_bench.rs` | by hand (below) |

Not given a dedicated test: the controller pre-check (`locked_against` at both
`controllers_input.rs` sites, mirroring `freeze::refuses`) and the per-key failure reason
(provoking a render failure needs a plug-in that fails offline).

**Runs.** Windows: clippy `-D warnings --all-targets` on core, engine, ui clean; core all,
engine `record_mode`, `rt_memory`, `freeze`, `low_latency`, `null_tests`, `master_chain`, ui
`--lib`, `record_mode`, `freeze`, `freeze_boot`, `ai_mcp_boot`, `controllers_review2/3`: all
pass. Linux (WSL `~/ci-r36-record`, target removed): workspace clippy and plug-ins without
editor exit 0, touched tests pass. Both re-run after merging r36/integration (r36/mix), with r36/mix's `vca`, `mixer_vca` and `mixer_desk` tests too: all pass, clippy clean on both systems.

## The benchmark: 64 tracks with effects

64 tracks, each a Firmus synth (16 notes, 8 s) into its own strip with **EQ, compressor,
delay and reverb**. Per-block processor time against the block's budget (100 % = the whole
block), best of 3 by p99; Windows, 16 logical cores, debug-optimized profile.

**Live** (first run; the machine shared with two other builders' builds):

| buffer | live, serial mean / p99 | live, pool (15) mean / p99 | blocks over budget (pool) |
|---|---|---|---|
| 32 | 590 % / 839 % | 238 % / 837 % | 6215 of 9000 |
| 64 | 486 % / 628 % | 165 % / 408 % | 3322 of 4500 |
| 128 | 236 % / 367 % | 142 % / 1053 % | 508 of 2250 |
| 256 | 224 % / 340 % | 105 % / 606 % | 453 of 2000 |
| 512 | 236 % / 331 % | 83 % / 313 % | 549 of 2000 |

The session cannot play live at any buffer on this machine.

**Baked, on a quiet machine** (no other build or WSL check for the whole run; the slots
folder empty before and after): *Prepare to record* 81.6 s for 64 renders, 618 MB; the
master render 0.7 s.

| buffer | mode | mean load | p99 load | worst | blocks over budget |
|---|---|---|---|---|---|
| 32 (0.67 ms) | baked | 4.0 % | 5.6 % | 2203 % | 1 of 9000 |
| 32 | baked master | 0.9 % | 1.7 % | 58.6 % | 0 of 9000 |
| 64 (1.33 ms) | baked | 3.0 % | 4.2 % | 1276 % | 1 of 4500 |
| 64 | baked master | 0.6 % | 1.1 % | 3.2 % | 0 of 4500 |
| 128 (2.67 ms) | baked | 2.6 % | 3.3 % | 759 % | 1 of 2250 |
| 128 | baked master | 0.5 % | 0.8 % | 1.6 % | 0 of 2250 |
| 256 (5.33 ms) | baked | 2.0 % | 2.8 % | 8.8 % | 0 of 2000 |
| 256 | baked master | 0.5 % | 0.7 % | 2.0 % | 0 of 2000 |
| 512 (10.67 ms) | baked | 2.0 % | 3.3 % | 57.6 % | 0 of 2000 |
| 512 | baked master | 0.4 % | 0.5 % | 0.8 % | 0 of 2000 |

The offline-paced table, run again after the changes below, says per block whether it
waited for the reader thread (`freeze::offline_waits`): at 32–256 frames no block waited
and none went over budget (worst 4.8–46.8 % at 128 and below); at 512 frames 13 blocks
waited and **the slowest block (63.5 %) is one that waited**, while the slowest block that
did not wait stayed at 9.3 %. So the offline runs' spike is the offline reader's wait
(`set_offline(true)` waits for a block rather than playing silence), not work.

**Real time, one minute each** (64-second songs so the disk is read all the way; the bench
thread promoted as the audio callback is — MMCSS *Pro Audio* at High — and paced at the
device's rate by sleeping; the readers never waited for; the slots folder empty before
and after every run):

| buffer | streams | p99 load | worst | blocks over budget | frames late (disk underruns) |
|---|---|---|---|---|---|
| 32 | 64 | 21.0 % | 219.6 % | 2 of 90000 | **0** |
| 64 | 64 | 13.4 % | 59.9 % | 0 of 45000 | **0** |
| 128 | 64 | 8.5 % | 27.7 % | 0 of 22500 | **0** |
| 256 | 64 | 5.8 % | 13.1 % | 0 of 11250 | **0** |
| 512 | 64 | 4.2 % | 8.5 % | 0 of 5625 | **0** |
| 1024 | 64 | 3.4 % | 5.0 % | 0 of 2812 | **0** |
| 32 | 128 | 33.5 % | 411.1 % | 9 of 90000 | **0** |
| 64 | 128 | 21.5 % | 148.7 % | 1 of 45000 | **0** |
| 128 | 128 | 14.1 % | 80.5 % | 0 of 22500 | **0** |
| 256 | 128 | 10.0 % | 18.9 % | 0 of 11250 | **0** |
| 512 | 128 | 7.2 % | 10.4 % | 0 of 5625 | **0** |
| 1024 | 128 | 5.5 % | 7.2 % | 0 of 2812 | **0** |

*Prepare to record* for these sessions: 64 renders in 236 s, 128 in 334 s.

**What was wrong, and what changed** (ADR 0092 §11). The first real-time run's one
underrun (64 streams, 256 frames) came from the benchmark: its warm-up ran unpaced, so the
playhead was a second ahead of the reader within milliseconds — now paced. The 128-stream
run then found a real fault in the reader: beyond its own look-ahead it read stream by
stream (the far blocks of the first streams before the next block of the last), so at 32
frames the last streams never caught up (342 M frames late) — now every stream's next block
is read before any stream's later one. The reader also runs above normal priority, outside
MMCSS. With both, no underrun in any of the twelve minutes above.

**Blocks over budget without an underrun** (2 and 9 in 90 000 at 32 frames, 1 in 45 000 at
64 with 128 streams): single blocks of 1.5–2.7 ms, the work's own p99 at 21–34 %, no frame
late — the bench thread's scheduling on a desktop OS. Read as this machine's lowest buffer
with no block over budget in a minute: 64 frames for 64 streams, 128 frames for 128.

## Shots

`cargo run -p cantus-ui --example shoot_record -- <dir>`: the bar on and preparing, the
question (master live/baked, mix, failures), leave, Night and Paper, EN and ES, 100 % and
200 %, a narrow window; the best in `docs/round36/record/`.

## Marked lines (`// r36:record`, `<!-- r36:record -->`)

- `cantus-core`: `lib.rs` (module), `command.rs` (`RecordingMode`), `bus.rs` (field, init,
  `set_record_lock`, `record_lock`, `locked_against`, `apply_session`, the check in
  `apply_keeping_audio`, `undo`, `redo`, `reset`), `snapshot.rs` (`master_frozen`, its init).
- `cantus-engine`: `lib.rs` (module), `processor.rs` (field, init), `processor_snap.rs` (the
  master's render on install), `graph/master_run.rs` (the render added), `listen_fork.rs`
  (copied, accounted), `tests/rt_memory.rs` (one test).
- `cantus-ui`: `lib.rs`, `views/mod.rs`, `ai_tools.rs` (module, registry), `app.rs` (bar,
  questions' frame, the arm/notice lines and two `RecordingMode` arms in `exec`/
  `exec_merging`, undo/redo notice), `ai_host.rs` (arm/notice, arm), `controllers_input.rs`
  (two pre-checks, one arm), `save_job.rs` ×2, `audio_files.rs`, `views/template_name.rs`
  (`as_saved`), `views/inspector_record.rs` (forced low latency).
- Own files, marked where the reviewer asked: `freeze.rs` (ui), `views/freeze_view.rs`,
  `views/freeze_monitor.rs`; `cantus-engine/src/freeze.rs` (`set_master`, attach).
- `cantus-engine/src/inst/stream_reader.rs` (the wants' order, the priority call),
  `cantus-rt/src/thread.rs` and `thread/{windows,linux,macos,other}.rs` (`raise_io_thread`),
  `cantus-core/src/freeze.rs` and `freeze_guard.rs` (own: pre-fader senders).
- Docs: manual READMEs and `ai-tools.md` rows (EN/ES), BACKLOG §4c.

## Left, and what needs the owner

- **Owner (hardware):** the lowest stable buffer with and without the mode (and with the
  master baked) on a real interface — ASIO and WASAPI, duplex, the 64-track session,
  dropouts over 10 minutes — and that a take made in the mode lands on time.
- **Found and fixed in the round:** the stream reader's order for many streams (above), and
  a pre-fader sender now keeps its fader and pan (ADR 0092 §2).
