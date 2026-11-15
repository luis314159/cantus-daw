# Round 40 — `fix-c`: the second review's engine, Delay and dancer findings

Branch `r40/fix-c`, base `1357827a` (r40/integration with everything of the round merged
except Chorda). The findings of `review-2.md` §fix-c, each reproduced by a failing test
first (run against the unfixed sources: the fix stashed, the test red), fixed at the root,
the test then green. ADR 0119 (§4, its tests) and ADR 0122 (§4, its tests) are amended;
ADR 0120 needed nothing (no finding touched the private bytes). `DSP_VERSION` stays 6; no
golden hash moved; the sync off on the free 2 s line runs the instructions it always ran.

## R2-A1 (HIGH) — a second reset brought the old line back

- **Reproduced:** yes, three ways. `delay_tests::a_second_reset_never_brings_the_old_line_back`
  (a loud free 2 s line and a loud synced 48 s one, reset, 0/1/7 calls of sound, reset again:
  the output differed from a new unit's at once);
  `transport::a_host_reset_then_a_value_set_at_the_start_leaves_the_delay_empty` (the
  plug-in: the host's `reset()`, then a value from the main thread before the first sample,
  which makes `pull_main` reset the unit again — 5 600 samples differed from a fresh
  processor's, the first by 1.19); `fx_delay_sync::two_panics_at_once_clear_a_delay_as_one_does`
  (two panics in one block against one, free and synced, through the processor).
- **Cause:** `reset()` decided whether the line held sound from `quiet`, which the previous
  reset had set to *silent* because the reads are masked — while the stale cells still held
  the old sound.
- **Fix:** `crates/cantus-dsp/src/delay.rs` — `None => self.stale > 0 || !self.silent_lines()`:
  a line still stale stays masked, cleared a stretch per call as before.
- **Freeze then unfreeze** (`processor_snap.rs` resets a resting strip's inserts on both
  edges) is the same `StereoDelay::reset` twice with no call between — the DSP test's
  "0 calls between" case; no separate freeze test was written (a freeze needs a rendered
  file; the path to the unit is the one the panic test drives).

## R2-A2 (MEDIUM) — `listen` was not the export after an `EffectNeed` mid-song

- **Reproduced:** yes. `listen::memory_asked_for_mid_song_is_answered_as_the_export_does`
  (the demo song, an EQ on the lead made linear phase by a lane from bar 2 and its
  resolution raised by another starting 4 ticks before bar 3's checkpoint): the listen
  differed from the export from frame 90 112 (by 1.09).
- **Cause:** `listen.rs` drained and dropped the engine's events after each block; the unit's
  `asked` stayed set, so it never asked again and played on its old path.
- **Fix (three parts, each found by the test):**
  - `Rig::answer` builds and sends what was asked after each block, taken at the next block,
    as `render` does (this alone made the whole listen equal the export only by chance of
    where its blocks fell);
  - the answer is taken at the next block, so the sample where the old path ends depends on
    the block boundaries — and a listen cut its blocks at its checkpoint places (the section
    starts on the 16-sample control grid) where the export does not. The places now sit on
    a whole number of blocks from the song's start as well as on the grid
    (`lcm(GRID, block)`), so a listen's blocks are the export's. A checkpoint is now at most
    a block before its section (`a_section_from_its_checkpoint_is_the_whole_render_to_the_bit`'s
    bound of rendered frames widened from 16 to 512 for that);
  - a section resumed from a checkpoint trimmed the latency the copy had by then; once an
    EQ is linear phase mid-song that is not the latency the export trims (the one it began
    with). The rig keeps its lineage's start latency (`Rig::latency`; taken again after an
    edit is installed on a copy, as before);
  - and a checkpoint image taken while an answer is on its way carries it (`Rig::asked`);
    `resume` sends it to the copy — the copy's unit asked already and would not ask again.
  The test runs with the mode lane alone and with a second lane starting 4 ticks before the
  third bar's checkpoint (the answer in flight at the checkpoint): the listen is the export
  to the bit, and each section from its checkpoint is the whole render to the bit.
- **What is left (as before this round's fix-a):** a need the runtime cannot foresee
  switches at the next block's start, so a render at another block size, or real time,
  switches a block or two apart, and an effect whose latency grows mid-song moves the
  export's later audio by the difference (the export trims only its start latency). A
  Delay's lanes are foreseen (its floor), so Delay is not affected. Noted in ADR 0119.
- **Nit:** the `// fix-a` comment in `listen_fork.rs` rustfmt had displaced is on its own line.

## R2-A3 (LOW) — free time precision on a long line; the floor's memory for modulated notes

- **Precision, reproduced:** `delay_tests::the_free_time_on_a_long_line_keeps_its_fraction`
  (the sync off on a line a floor kept at 48 s, 46 s in, a delay of 4 800.3 samples: the
  echo's two cells were 0.75 and 0.25 — the `f32` position's quarter-sample step — for
  0.70 and 0.30).
- **Fix:** on a line longer than the free one, the free time is read by its whole and
  fraction as the synced head reads (`sync::read_masked`, which does not depend on the
  line's length or the write position's size); on the free 2 s line it is the old `f32`
  read, instruction for instruction (a separate monomorphised loop, chosen per call), so the
  null and every golden hash stand.
- **Memory, reproduced:** `fx_delay_sync::a_modulated_note_reserves_only_the_line_its_modulator_reaches`
  (a quarter at 60 BPM, an LFO a step either side on `note`): the engine asked for the 48 s
  line (18 MB a side), the reach of every note value.
- **Fix:** `automation.rs` — a modulated lane's reach is the positions it plays moved by
  `modulation_reach`: `Σ depth × amount × output` with the output −1 … 1 (bipolar) or 0 … 1
  (unipolar) and the amount its own (1 when a lane or another modulator moves it), in the
  normalised domain, clamped — instead of the lane's whole range. Offline still equals real
  time (same test).

## R2-C1 (MEDIUM) — the dancers' analyser ran at rest

- **Reproduced:** by reading and by the new e2e test's design (below); the at-rest assertion
  is the one that held the stuck tap. Cause: `DanceLive::update` returned early when no
  dancer was left, before `hears` was written, so it stayed true; and the tap was wanted
  whenever a dancer heard, playing or not — `vis.update` ran every frame with the companion
  closed.
- **Fix:** `dance_live.rs` resets `hears` on that path; `stage_companion.rs` wants the tap
  for a project's dancer only while the engine plays (stopped, Espectro's bars dance to the
  beat, as designed). `Visualiser::counts` (updates, analyses) lets the tests see it.
- **Test:** `espectro_live_e2e::a_projects_espectro_hears_the_song_and_nothing_is_analysed_at_rest`
  — through `CantusApp::assemble` on a real processor: Espectro added and chosen as the
  picker does; stopped, the tap off and the analyser's counts still; playing, the tap on,
  the closed companion's analyser analysing, the pose carrying the song's bands; stopped
  again and, playing, with its track deleted: tap off, counts still.

## R2-C2 (MEDIUM) — Espectro in the companion window never heard the song

- **Cause:** the companion posed its own character with `pose_at` alone; `hears` looked only
  at the project's dancers, so with the visualiser off not even the tap ran; and the
  companion window's view (`views/stage_companion_view.rs`) poses the chosen character
  again on its own clock, which dropped any bands it was given.
- **Fix:** its own character, when it hears the spectrum, is posed with the companion
  analyser's four bands (while the companion's song frame plays), the window's view keeps
  them when it poses on its own clock, and the tap is wanted while the window shows such a
  character.
- **Test:** `espectro_live_e2e::espectro_in_the_companion_window_hears_the_song` (visualiser
  off and on: the tap on, the shared pose's bands from the song).

## R2-C3 — the missing tests

- live Espectro wiring through `assemble` — `espectro_live_e2e.rs` (above);
- the video export's spectrum branch — `video_cast::an_export_analyses_its_render_once_for_espectro_and_the_visualiser`
  (the job's `analysed`: the spectrum is the render's, the frames pose Espectro with it) and
  `video_cast::a_song_with_espectro_exports_with_the_visualiser_on` (`vx::start` → file);
- the picker scrolled — `dancer_window::the_picker_scrolled_chooses_a_tile_from_its_lower_rows`
  (at the editor's default 1 040 × 680 the twenty characters and *Import* fit their card; in a
  smaller editor the grid scrolls: the wheel over the tiles, a tile of the lower rows clicked
  chooses that character in one undo step; the same place at the top holds another).
  `PickState::hidden_rows` says how many rows the card hides.

## R2-C4 (LOW) — the export analysed its render twice

- **Fix:** `video_export::job::analysed` makes one analysis and gives it to both Espectro's
  spectrum and the visualiser (`vis_video::for_render` now takes the analysis); none when
  neither wants it. `vis_video::analyses_on_this_thread` counts, and the test above asserts
  1 with both, 1 with either, 0 with neither.

## What was run (Windows, this worktree)

- Each new test against the unfixed sources first (the fix reverted in the tree, the test
  red, the fix restored): R2-A1 (DSP, plug-in, two panics), R2-A2, R2-A3 (both), R2-C1 and
  R2-C2 (the behaviour reverted, `espectro_live_e2e` red: "stopped: the tap is off",
  "the companion's Espectro wants the tap"). R2-C3 and R2-C4 add tests and an API
  (`job::analysed`), so they had no red run of their own.
- `cargo test -p cantus-dsp`: green (lib 841 passed, 14 ignored; golden hashes unmoved,
  `DSP_VERSION` 6).
- `cargo test --no-fail-fast -p cantus-engine -p cantus-plugins -p cantus-dance -p
  cantus-video`: 1 486 passed, 0 failed, 18 ignored (`rt_memory`, the null and golden tests,
  `fx_delay_sync`, `listen`, Delay's plug-in equivalence, `transport.rs` among them).
- cantus-ui: `delay_sync_e2e delay_window_e2e dancer_agents dancer_cost dancer_default_e2e
  dancer_demo dancer_window dance_live dance_import dance_import_e2e stage stage_companion
  stage_video video_export video_cast cast_e2e espectro_bands espectro_live_e2e ai_listen
  listen_measure at_rest_e2e idle_repaint_e2e`: 121 passed, 0 failed, 6 ignored.
  `stage_video.rs` now builds the export's visualiser through `job::analysed` (the
  signature of `vis_video::for_render` changed).
- `RUSTFLAGS=-D warnings cargo clippy --all-targets -p cantus-dsp -p cantus-engine -p
  cantus-plugins -p cantus-dance -p cantus-video -p cantus-ui`: clean.
- `cargo fmt -p` on cantus-dsp, cantus-engine, cantus-plugins, cantus-ui; it also reformats
  `cantus-engine/src/backend.rs` and `tests/pdc_lazy.rs` (unformatted on the base, as fix-a
  noted): left as they were. `i18n.rs` untouched.
- **Disk:** F: was down to 3.6 GB free after the last suite (other builders' targets share
  the drive); no build was started after that.

## Verified only in emulation

Everything: the engine on processors driven by hand and offline renders, the plug-in's
processor driven directly (no host), the companion window embedded in egui's test pass (the
Windows layered window takes its dancers from the same shared state; not run on screen). Not
run on Linux (the integrator runs it).

## What the owner should look at

- Espectro in the companion window (View ▸ companion, its character set to Espectro, the
  visualiser off and on): the bars follow the song while it plays; stopped, they dance to
  the beat.
- A project's Espectro, the song stopped: nothing should move in Task Manager's CPU column
  for Cantus beyond the at-rest figure (the analyser and the master tap are off).
- A synced or automated Delay frozen and unfrozen, or panicked twice: no ghost of the old
  echoes.

## What is left

- An effect need the automation cannot foresee (an EQ's mode or resolution lane, an Aula
  response lane) still switches at the next block's start — a render at another block size
  or real time switches a block or two apart — and a latency that grows mid-song moves the
  export's later audio by the difference. Both were so after fix-a; a listen now matches
  the export exactly. Foreseeing those needs (as Delay's floor does) is a follow-up for the
  integrator to schedule (ADR 0119 §4 says so).
- No freeze-specific Delay test (a freeze needs a rendered file); its path is the reset the
  DSP and panic tests drive.
