# Round 33 — session: the model, the commands, the engine, recording

Builder: **session** (`r33/session`), ADR 0076, schema 46.

## What was done

**Delivery 1** (merged as 20505f04): the model (`Project::session` — scenes and settings;
`Track::slots` — each track's column, the deviation the integrator accepted), the session
clip (an ordinary `Clip` plus launch settings, follow actions, a start marker and its own
envelopes), every command with its exact inverse, the builders (duplicate, capture and
insert scene, from/to the arrangement, consolidate a column, consolidate time, commit a
performance), the guard that keeps a session clip a session clip, schema 46 with its
migration, load repair and `fixtures/v46.cantus`, DAWproject scenes and clip slots in and
out, the engine's message/event API.

**The API change after it** (531f4d7f, told to the integrator): `SlotPhase::Stopping` is
`Stopping { since, from, at }`, and `SlotPhase::content_at` reads a stopping clip's position.
Nothing else in `SessionMsg`/`SessionEvent` changed.

**The review of delivery 1** (8edb4de6, each with a test that failed before):
`InsertTrack` checks a track's column (scenes there, in order, valid clips, ids its own)
and exploded takes leave it out; grooves a session clip plays are in use (and repaired on
load); the timeline's edits read the arrangement only (`Track::arranged`: cut, link,
pattern mode, crossfades, comps, the clip selection, `clips::clip_of`); envelopes follow
their clip to another track (swapped targets, so the inverse is exact; refused where they
cannot follow) and the targets of every carried envelope are touched (frozen locks); a
consolidate that would not play the same is refused; a session-only audio track is written
as DAWproject audio; a cut audio window fades at the cut (`CUT_FADE`); the fuzz's no-op
kind has its own index; `take_note_inputs` has its doc back.

**The engine** — the session is played as its **projection** (ADR 0076 §6–7):

- `cantus_core::session::play::plan`: the conductor as a pure function — launches on their
  boundaries (bar lines through every metre change), trigger/gate/toggle/repeat, legato,
  follow actions drawn from a hash of the seed, the track and the tick (stable as the
  horizon grows), scenes' tempo and metre, stops, back to arrangement, `Input::Resume`.
- `project_play` / `edit::perform`: the song with the performance written into its
  arrangement exactly as a commit writes it, with **keyed ids** — the clips heard are the
  clips a commit writes (voice tags and round-robin turns included).
- `cantus_ui::session_play`: the conductor beside the engine handle (`EngineHandle::send`
  keeps `EngineMessage::Session` for it): presses with an 80 ms lead, plans to a 32-bar
  horizon, the projection compiled through the `project_to_compile` path, events told when
  heard (queued, playing, stopping with position, stopped, scenes, tracks, follow actions,
  tempos, arrangement record, resend), a stop or a jump ends the performance.
- The audio thread runs nothing of the session; a snapshot arriving while playing is
  installed **on the automation grid** (`session_rt.rs`), so live equals offline to the bit.
- Recording into a slot (`session_play_rec.rs`): through the application's own pass
  (count-in, files, journal, recovery), punched to the slot's stretch, MIDI record-quantised,
  audio as its file's window, overdub into a looping MIDI clip, fixed length, the new clip
  playing on in time. Recording into the arrangement: what was heard, one undo step.
- Frozen tracks' clips do not launch; unwarped session audio keeps its seconds on tempo
  edits (`tempo::keep`); templates and the missing-files card see session clips.
- ADR 0076 written whole; the manual's engine-side pages (EN, ES: "The session: how
  launching, follow actions and recording work").

## The timing round (the integrator's review of the engine, after 6e848f7e)

The projection architecture was kept; the timing policy around it rebuilt (ADR 0076 §7):

- **D1 the lead** is derived: two device blocks, the worst of the last 64 compiles of a
  projection, a 2 ms floor (`lead_s`): at 48 kHz 4.7 / 12.7 / 44.7 ms at 64 / 256 / 1024
  frames before the compile (7.7 / 15.7 / 47.7 ms with a 3 ms one). An unquantised launch,
  a gate release or a stop with quantise off is heard the output latency + the lead + at
  most one interface frame after the press.
- **D2** a compile that ran past a press's tick is planned again from two blocks on and
  compiled again before it is sent (`restamp_late`); the launch tolerance (a 32nd note): a
  launch pressed just after its boundary — or that missed it to the lead or a late compile —
  plays as launched on it, caught up. An install that ends, behind the playhead, a note the
  old snapshot had sounding ends it (`release_cut_notes`: the hung-note case, tested).
- **D3** measured (`session_bench.rs`): a 128-track scene launch 58 ms worst (quantised);
  one track launched while it plays 3.5 ms, 5.2 ms worst, with `Snapshot::compile_reusing`
  (unchanged tracks' notes and controllers reused, tested equal to a full compile) and a
  projection that applies its commands without copying them — 18 ms of lead at 256 frames.
- **D4** looping clips and held tracks are open-ended 64 bars past the horizon
  (`open_ended`); a follow chain stalls on its last clip, looping.
- **D5 / finding 7** every plan after the first carries on from the last as it stood before
  the lead (`plan_from`).
- Findings 1–12 fixed, each with a test: a launch from stop survives the frames before the
  engine plays; chained legato keeps the first launch's phase (`Played::origin`); overdub
  takes each note's own window; launching the recording slot ends it at the bar; a queued
  slot recording goes on stop; a MIDI take is laid ahead and plays on from its end, an audio
  take in time once whole; `within` brings the tempo and metre in force and the projection
  keeps unwarped audio through a scene tempo as the commit does; session audio preloaded;
  the deferred install's old snapshot left to the garbage (rt_memory with a draining thread
  — a race: it does not fail reliably before the fix); `settle` with a waiting snapshot; the
  lead and slot-record boundaries on the performance's tempo map. A crashed slot recording
  comes back into its slot. The rustfmt nits restored.

**The numbers after the timing round** (at 08ac608f): Windows, `-D warnings`, cantus-core +
cantus-project + cantus-engine + cantus-ui: **3 563 passed, 0 failed, 43 ignored**; workspace
clippy `-D warnings` clean; Linux (WSL, own clone, target deleted after): clippy clean, the
workspace's tests **4 920 passed, 0 failed, 59 ignored**.

## The second round (the integrator's review of the timing round, after 94f267a6)

Each with a test that fails before the fix (run against the old code where it could be:
the five planner tests all fail on it).

- **N1** `resolve`: a launch plays on the first boundary at or after its press (as heard)
  less the tolerance — also when the boundary is after the press and the lead (or a compile
  that overran, planned again by `restamp_late`) took `at` past it: caught up, never a bar
  late. Stops and releases pressed before a boundary they can no longer reach take effect
  as soon as they can (they took the next grid before; one pressed on the boundary still
  waits for the next). Tests: a press 10 ms before the bar
  with a 27 ms lead, an overrun a beat past the bar (core); an overrun compile through the
  conductor (`session_play3.rs`).
- **Minor** the tolerance is a 32nd, at most a quarter of a note grid (`play::tolerance`);
  the manual says so (EN/ES; its old "80 ms before a boundary takes the next" was wrong
  since the timing round). `Performance::within` gives one point per tick (`last_per_tick`).
- **N2** `plan_from` takes the presses in the order of their ticks (a stable sort of the
  unapplied presses; each keeps its index). Test: a recording's launch laid two bars on,
  then a quarter-quantised launch on a track following every beat — the plan equals the
  plan of the presses in tick order; every window ends after it starts.
- **N3** the cut notes are worked out where the snapshot is sent (`Snapshot::cut_from`,
  `snapshot_cuts.rs`, in `EngineHandle::compile_from` while playing, against
  `rec.snapshot`): notes the old one started and has not ended that the new one ends
  sooner, by voice tag and start; the install walks `Snapshot::cuts` (`CutNote::due`) — the
  O(n²) scan and the O(tracks²) lookup are gone. A pasted note is never cut. Tests: 32 dense
  tracks (chords of four on every 32nd, a beat long, all shortened; 16 beats behind the
  playhead) — the install's 64-frame block cost **+6.8 ms with the old scan** (7.66 ms
  against 0.87) and **+0.03 ms now**; the test's bound 1 ms on the median difference of
  nine measured pairs (see the seams: one min-of-five run failed under load); `rt_memory` (an install
  ending eight cut notes); a pasted note not cut; a shortened one cut only where it was
  playing.
- **N4** the compile estimate is the 95th percentile (nearest rank) of the last 64, counting
  neither a launch from stop's full compile nor a retry.
- **N5** `preload_session_warps` (`edit::warp_ahead`): every warped session clip laid as a
  launch lays it (first pass and one more, keyed windows) at the song's tempo and at its
  scene's, attached ahead with `Renders::preloading` and kept in `Renders::preload` (the
  live attach's protected set) — never evicting what plays, never the pitch renders.
  Test: compiled before any launch, the first projection of a launch on bar 3 plays the
  final render, frame for frame, and asks for nothing.
- **N6** audio takes are seamless: `EngineHandle::growing_takes` (what each file's header
  says it holds, the writer flushing every 500 ms, placed by the same `take_start`);
  0.3 s and a lead before the end the conductor copies the take's beginning aside (the
  temp folder; removed when the performance ends), lays the clip from it placed as the
  whole take will be, and launches it on the end; the whole take replaces it under the same
  id. Test (`session_rec_audio.rs`, a real pass through the recorder): the growing file's
  head is the take's beginning, placed alike, the same window and fine offset, and the
  clip from it plays the first 200 ms after the end exactly as the whole take does. A take
  shorter than the flush plays on as before (its first moments lost).
- **N7** `starting` is the handle's stop count and an instant: a stop since, or 2 s with
  no word from the engine, gives up — and ends the performance — so the next launch from
  stop starts the transport. `EngineHandle::stops()` counts the stops sent.
- **N8** a playing clip's follow (and a scene's) planned again past the cut goes at its
  next turn (`next_due`: a whole multiple of its follow time, a pass for a linked one), not
  at the cut.
- **Equality** the rich live-equals-recorded test now has a Pulsus kit (a snare's three
  round-robin layers in turn, a shaker's four at random) and a warped 100 BPM loop in both
  scenes (heard at 120 and at the second scene's 132); its compiles wait for their warp
  renders (`compile_for_render`). A hosted plug-in is not in it: hosting is in
  `cantus-host`/the sandbox, above the engine, which its tests do not link. Recording while
  launching is the conductor's (`cantus-ui` tests).

The checks: Windows, `RUSTFLAGS=-D warnings`, cantus-core + cantus-project + cantus-engine +
cantus-ui: **3 577 passed, 0 failed, 43 ignored**; workspace clippy `-D warnings` clean;
`cargo fmt -p` clean; Linux (WSL, own clone, target deleted after): clippy clean (and the
plug-ins without their editor), the workspace's tests **4 934 passed, 0 failed, 59
ignored**. The conductor's tests (`session_play3.rs`: N1's overrun, N4, N7) were not run
against the old code (a UI rebuild each); they fail on it by construction — the old
`resolve` sends the overrun to bar 3, the old estimate is the worst (58 ms), the old
`starting` flag never clears without the engine playing. The audio lay-ahead's wiring in
the conductor (`lay_ahead_audio`) has no end-to-end test with a live engine: its parts
are tested (`growing_takes`, the head, the window, the plays-alike).

## The seams (the integrator's review of r33/integration, ce23e294)

Each with a test that fails on the code before (`cantus-ui` `tests/session_play4.rs`: all
seven run against it and fail).

- **1** a launch from stop while a source is followed (`SyncNow`, read from the seam:
  `sync_service::shared().lock_state()`/`tempo()` and the chosen source) waits as long as
  the source's start takes — no `STARTING_S` timeout, a stop still calls it off; it is told
  queued and nothing is told heard meanwhile (so the launcher's `PRESS_SHOWN` needs no
  change: the queued event holds the press). A song position while it waits moves the
  launch there; a Start behind where it was launched plays it from where the transport is,
  caught up from the start. A jump back counts only when the engine played the frame
  before (a sync start's seek is not one). Tests: Link quantum 8 at 90 and MIDI clock,
  held 2.3 s (past the old 2 s), then started.
- **2** a slot take (new clip, laid ahead, and overdub) keeps `Note::expr`, the curve cut
  (`note_expr::ops::split`) where the take's end shortens a note. Tests: ExprInputs during
  a fixed-length slot recording (one note held across the end) and an overdub.
- **5** a jump back while a source is followed and no locate was sent (the handle's seek
  count did not move) is the source's — a relock, a song position — and the performance
  plays on; a user's locate still ends it. Test: a beat back while following Link.
- **6** the lead, `engine_now` and `restamp_late`'s two blocks turn seconds into ticks at
  the followed tempo while following (`heard_bpm`; the slot recording's margins too). Test:
  following at 240 over a song at 120 — the lead is its seconds at 240 (it was half).
- **7** following Link, a scene's tempo is proposed by the engine as its tick is heard,
  through sync's `Processor::propose_tempo` (3350fae0): the conductor sets
  `Snapshot::scene_tempos` on the projection it just compiled, and the audio thread
  (`session_rt::propose_scene_tempos`, after `sync_block`) proposes those its block plays
  through. A projection installed again proposes nothing more; the song after a
  performance carries none (the engine no longer proposes on a new map: sync's side).
  (A first take proposed from the interface when the change was told; replaced by this,
  timed on the audio thread.) Tests: `cantus-engine` `tests/session_sync.rs` (once, at the
  bar, nothing on a reinstall or at the end; fails without the walk) and `cantus-ui`
  `session_play4.rs` (the projection carries the scene's 140 at its bar; the song after
  carries none).
- Docs: ADR 0076 §7 "Following a source", its tests; the manual's session section (EN/ES).
- Checks (head e1571d17, r33/integration 3350fae0 merged): Windows `RUSTFLAGS=-D warnings`,
  core + project + engine + ui: **3 732 passed, 0 failed, 45 ignored** (one run under load
  failed sync's `sync_midi::the_out_thread_sends_on_time` once, 90th percentile 4.5 ms; it
  passed on the rerun); workspace clippy `-D warnings` clean; `cargo fmt -p` clean; Linux
  (WSL, own clone, target deleted after): clippy clean, the workspace's tests **5 120
  passed, 0 failed, 61 ignored**. The engine's scene-tempo test was not run against the
  code without the walk (under `-D warnings` the unused walk does not compile); without
  it nothing proposes a scene's tempo (sync's engine proposes only on purpose), so its
  "once" fails.
- The dense-install test failed once in the integrator's full suite under load (+0.66 ms
  against its 0.5 ms bound, the minimum install against the minimum plain block, taken
  from different moments). It now takes nine pairs — the best of the plain blocks just
  before each install, and the install's block — and bounds the median of their
  differences at 1 ms: the old scan measures a median +8.2 ms and fails it; the new walk
  +0.11 ms (three runs: 0.105, 0.110, 0.121 ms).

## How it was tested

- `cantus-core`: `session_tests.rs`, `session_tests2.rs`, `session_tests3.rs` (26 tests);
  the undo fuzz's session stream.
- `cantus-engine` `tests/session.rs`: a session never launched sounds as the song without it
  to the bit; a launch lands on its boundary exactly as the same notes laid by hand; a live
  performance — projections installed while playing as the conductor makes them — equals its
  recorded arrangement bit for bit at blocks of 128, 441 and 333 (three workers), with follow
  actions, legato, an envelope and a scene's tempo; offline at two block sizes.
  `rt_memory.rs` `the_session_never_allocates`.
- `cantus-ui` `tests/session_play.rs`: launch from stop, queued then heard, stopping with its
  position, arrangement record as one step, recording MIDI into a slot.
- `cantus-project`: `schema_46_keeps_the_session`, DAWproject scenes in (Bitwig-shaped) and a
  session out and back whole, valid against the official schema.

## The numbers

- Windows, `RUSTFLAGS=-D warnings`, cantus-core + cantus-project + cantus-engine + cantus-ui:
  **3 542 passed, 0 failed, 43 ignored** (one run under three builders' load failed
  `warp_bounce::a_transposed_clip_bounces_to_exactly_what_it_played` once — a render timing
  under contention; it passes alone, 6/6, and touches nothing of the session).
- `cargo clippy --workspace --all-targets` with `-D warnings`: clean.
- Linux (WSL Ubuntu 24.04, `tools/wsl/check.sh`, own clone `~/ci-r33-session`, its target
  deleted after): clippy clean (and the plug-ins without their editor); the workspace's
  tests **4 899 passed, 0 failed, 59 ignored**.
- The live-equals-recorded test: bit-identical at blocks of 128, 441 and 333 (three
  workers) over 24 s of a performance with follow actions, legato, an envelope and a
  scene's tempo; the engine's whole suite still green with installs deferred to the grid
  (895 tests).

## Marked lines in files session does not own

`// r33:session` in: cantus-core `lib.rs`, `model.rs`, `command.rs`, `freeze.rs`,
`freeze_guard.rs`, `groove.rs`, `notefx_cmd.rs`, `clipboard.rs`, `comp.rs`, `crossfade.rs`,
`links.rs`, `pattern.rs`, `tempo/keep.rs`, `snapshot.rs`; cantus-engine `backend.rs`,
`processor_pass.rs`, `processor_snap.rs`, `recorder.rs`, `samples.rs`, `warp_attach.rs`;
cantus-project `midi/import.rs`, `report.rs`, `dawproject/{mod,read,read_timeline,write}.rs`;
cantus-ui `app.rs`, `lib.rs`, `clips.rs`, `comping.rs`, `templates.rs`,
`views/samples_missing.rs`.

## What needs the owner

- Their ears on a performance: launching, legato and follow actions on real material.
