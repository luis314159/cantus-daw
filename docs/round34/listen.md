# Round 34 — listen: hearing and numbers, then the mastering assistant

Builder **listen**, branch `r34/listen` (from the contract b4568467; r34/integration merged
at 2b5f1790 and 6af6430f). ADRs **0083** (listening: range renders from checkpoints,
analysis taps, live = offline) and **0084** (the mastering assistant). No schema change, no
keymap change, `DSP_VERSION` stays 5 (no sound changed: the golden hashes do not move).

## What was done

### Inside Cantus (engine and DSP, MPL)

- **Speculum's ear** (`cantus-dsp/src/speculum_ear.rs`): a headless analysis of one stereo
  stream kept fine enough to read any range afterwards — 10 ms columns (K-weighted energy of
  the pair and of its mono fold-down, 4× true peak, sample peak, energy, correlation sums,
  mid/side, sharpest second difference) and third-octave mid/side spectra every 4 096
  samples. Readings: integrated and LRA through the house meter's own `ColumnLoudness`,
  short-term and momentary maxima, peaks, RMS, crest, correlation and its lowest 400 ms
  window, width, mono loss, octave and third-octave spectra, sharpness near a sample.
- **The listen** (`cantus-engine/src/listen.rs`): `listen(snapshot, options, units, hosted,
  checkpoints, progress)` — the export's render (same set-up, same latency compensation,
  same early stop and tail), with **analysis taps** (the stems' `StemTap`, every source
  aligned by its own lateness) heard by ears, and the master's samples kept for a WAV.
- **Range renders from checkpoints**: `Processor::fork` (`listen_fork.rs`) copies the whole
  engine between two blocks — every unit (a new `fork()` on `FxUnit` and `InstrumentUnit`;
  every native effect and instrument implements it by `Clone`; the DSP structs that lacked
  it derive it), the transport, automation and modulator state, compensation lines, the
  notes to inserts, the master chain, the pass context's snapshot — into a processor with
  its own rings and pool. Checkpoints at section starts (markers, else every eight bars) on
  the 16-sample control grid; reused while the snapshot plays the same up to them
  (`same_until`, `listen_prefix.rs`); a snapshot changed later is installed on the copy as
  playback installs an edit. A hosted plug-in, a frozen render, a voice streaming from disk
  or a sync port cannot be copied: the range pre-rolls (8 s by default) and says so.
- **Live = offline, checked**: the application's own `EngineHandle` (`manual`) driven block
  by block at irregular sizes equals the listen, to the bit.
- `listen::run_chain`: audio through a fresh copy of an insert chain (the stems' `Through`,
  now also built from a bare chain) — the assistant's trials.

### The tools (`cantus-ui/src/ai_tools_listen.rs` and `mastering_*.rs`, GPL)

`audition`, `analyse_tracks`, `masking_matrix` (Read jobs), `set_mix_targets` (Edit job →
one `Batch` of `SetTrackGain`, its sentence naming every track and trim), `master_assist`
(Read job: target with its source, the mix's numbers, the chain with every setting and
why, the trial's prediction, a `proposal_id`), `apply_master_chain` (Edit: one `Batch`,
naming the platform and the units). Ranges as beats, `"bar:beat"` or a marker's name;
solo/mute sets; the master on or off; a genre curve or a reference file (inside the
project's folder or an allowed one, `cantus_ai::may_touch`). WAVs go to Cantus's cache
(`data_dir/cache/listen`, the newest eight kept). The prompt **Master for streaming**
(`master_for_streaming`, one argument, both locales) is registered in `ai_catalog.rs`.
`ListenHost` (`CantusApp` and the tests' host) refuses with what to do while samples or a
render's plug-ins load (`RenderFor::Listen`, new); a listen holding hosted instances lets
them go on the UI thread when it ends (a drop guard and `Service::frame`).

### The mastering assistant (ADR 0084)

`mastering.rs` (the decisions), `mastering_targets.rs` (nine platforms with sources and
dates; six genre curves), the panel `views/mastering.rs` + `views/mastering_draw.rs`:
opened from the master strip's menu and the master chain's *Add effect* list; platform and
balance, a reference to load, whole song or loop, replace or add after; *Listen and
propose* on a worker with progress and *Stop*; the numbers for mix, proposal and target;
the octave balance drawn; the chain and why; *Apply* (one undo step); **A/B** through the
preview voice at matched loudness, switching in place; the platform's source.

## How it was tested

Tests first, null tests before wiring. **31 new tests** (+2 measurements, ignored):

- `cantus-dsp` `speculum_ear_tests.rs` (8): the ear's integrated loudness, range, true peak
  and loudest short-term equal `LoudnessMeter`'s to the bit; block sizes 1/7/333/4 096/8 192
  give identical columns, spectra and readings; a −20 dBFS 1 kHz sine reads −23.0 dB in its
  octave (mean square), ~−20 LUFS, crest 3.01 dB, nothing within 50 dB elsewhere; opposite
  phase reads −1 and folds to silence; independent channels: width 0 dB, mono −3 dB; ranges
  read their own part; a click 20× sharper than the music; NaN is silence.
- `cantus-engine` (13): `listen_prefix` (2), `listen_fork` (1: a copy plays on as the
  original, 400 blocks to the bit), `tests/listen.rs` (9) over a project with **every native
  instrument and every effect kind** at their heaviest, sends, a reverb return, a master
  chain with the limiter's look-ahead, a fader ride, an effect lane and an LFO: the taps
  change no sample (listen = `render`, every track, strip and the direct sum tapped); a
  section from its checkpoint equals the whole render to the bit (from its start, from inside
  it, from an earlier checkpoint); after an edit later in the song (notes rewritten, a note
  on the checkpoint's beat, an automation point moved) it plays from the checkpoint as a
  whole render of the edited song, to the bit; an edit before it falls back to an earlier
  checkpoint, a fader to the start; block sizes 1/77/4 096; an early end is the export of the
  range; a hosted unit pre-rolls and says why; a pre-roll is the render from where it starts;
  a listen stops when asked; **live = offline**: the application's `EngineHandle` driven at
  irregular blocks equals the listen to the bit. `rt_memory`'s
  `a_forked_processor_never_allocates`.
- `cantus-ui` (10): `mastering_targets` (1), `mastering_tests.rs` (6: the tools through their
  `ToolRun`s on a test host — audition's numbers, WAV, sections and checkpoint reuse; a solo,
  refused ranges and a file outside the folders; analyse and masking (a doubled lead found
  first, overlap > 0.5); mix targets within 0.1 LU in one undo step, undo exact; the
  assistant for Spotify, Apple Music and Club measured on a real render after applying),
  `tests/ai_listen.rs` (3: through the MCP server's in-process client with `CantusApp` as
  host — the tools and the prompt listed, `prompts/get` worded; listen, set targets,
  master, then the user's Undo restores the project; the panel driven by `CantusApp::frame`
  itself, the boot's path: its worker's proposal back through the frames, *Apply* one undo
  step).

**Measured** (`cargo test -p cantus-ui --test listen_measure -- --ignored --nocapture`,
this machine, dev profile with the DSP and engine at opt-level 3):

| What | Number |
|---|---|
| Showcase, whole song (169.7 s) with 10 checkpoints | 144.3 s (round 28: 157–217 s without) |
| Its last section from the checkpoint at bar 73 | 20.7 s of audio in **14.2 s**, identical to the whole render |
| Pre-roll 2 / 4 / 8 / 16 s on the same section (after review 1's whole-sample start) | 2.0 / 2.5 / 1.4 / 1.7 dB under its peak: does not converge — Space's modulation (below); hence `exact: true` |
| Pre-roll by unit, 3 s (`preroll_by_unit`) | none, the six LFO effects, compressor, limiter, reverb: 129–139 dB; delay 62.5 dB; **Space 8.3 dB** (its FDN's modulation rotors turn from its reset, not the song) |
| Assistant, 7 demos × Spotify / Apple Music / Club (21), real render after applying | worst **0.07 LU** from the target; true peak **≥ 0.09 dB under the ceiling** in every case; prediction = real reading to 0.01 |
| Tools' own tests (6 tools, a 16 s band) | 11 s for the six tests |

Windows: `cargo clippy --workspace --all-targets` with `-D warnings` clean.
Linux (WSL Ubuntu 24.04, `F:\projects\daw-r34\listen-wsl.sh`): clippy (workspace, all targets, and the plug-ins without their editor)
clean; `cantus-dsp`, `cantus-engine`, `cantus-host` **1 836 passed**, 0 failed, 22 ignored;
`cantus-ui` (headless) **2 056 passed**, 1 failed — `font_coverage`: an error sentence of
`mastering_tools.rs` said "Settings ▸ AI" and the interface's fonts have no ▸; fixed, and the
test (the fonts are embedded, the scan reads the sources: the same on both systems) passes.
Windows: `cantus-dsp`, `cantus-engine`, `cantus-host` 1 832 passed, 22 ignored, 1 failed under
load — `freeze::in_real_time_the_reader_keeps_ahead_of_the_playhead`, paced at twice real
time while the WSL build ran beside it; it passes alone (twice), and on Linux; nothing of
this round touches a frozen render's reader. `cantus-ui --lib` 386 passed (with the
catalogue's and the i18n tests), `ai_listen` 3, `ai_mcp_boot` 1, `ai_mcp_pump` 5. The WSL
clone's `target/` was deleted..

**Looked at** (`cargo run -p cantus-ui --example shoot_mastering -- <dir> [scale] [locale]`;
the best in `docs/round34/listen/`): the panel just opened, its platform list open, listening (42 %), and
with its proposal, in Slate and Paper, English at 100 % and Spanish at 200 % (32 PNGs, the
mixer under it). Fixed after looking: the reasons ran off the window's edge in one line (now
wrapped in their own column); the octave chart's labels were unreadable on the meter well in
Paper (now on the strip's ground in both themes); "62 Hz", "1000 Hz" → the octaves' nominal
names (ISO 266: 63 Hz, 1 kHz); the ‹ › fields cycled on a click though they showed a list
arrow (now they open the list); Spanish says *máster* as the rest of the interface does.
Nothing overlaps; the body scrolls when the window is short (200 %).

## Marked lines

Engine (no other builder's files, marked for the integrator): `cantus-engine/src/lib.rs:21`
(`pub mod listen`), `processor.rs:92-93` (`mod listen_fork`), `effects.rs` (`EffectUnit::
fork`), `graph/master.rs` (`MasterChain::fork`), `inst/mod.rs` (`fork` on the trait and
`NativeUnit::fork`), `fx/mod.rs` (`fork` on the trait, `Wire`), one `fork` in each of the 21
native `fx/*.rs` units and the 8 `inst/*.rs` instruments; `Clone` added to derives in
`graph/{track,strip,exec,notes}.rs`, `pdc.rs`, `automation.rs`, `modulator.rs`,
`notefx/{mod,arp,table}.rs`, `processor_rec.rs`, `processor_auto.rs`, `preview.rs`,
`inst/mpe_live.rs`, `stems.rs` (`StemTap`; `Tapped` and `pair` `pub(crate)`; `through`
`pub(crate)`), `stems_master.rs` (`Through::of_chain`) and in `cantus-dsp`'s `Calor`,
`Gravis`, `StringEngine`, `Chorda`, `Lamina`, `Modulus`, `Exemplar`, `Unda`, `Comprimo`;
`cantus-dsp/src/speculum.rs:26` (`mod ear`); `cantus-engine/tests/rt_memory.rs:564` (a test).

Elsewhere, `// r34:listen`: `cantus-host/src/session_render.rs:64` (`RenderFor::Listen`);
`cantus-ui/src/app.rs:307` (the `listen` field), `:768` (its default), `:4069` (the
epoch synced before the agents' calls — review 3), `:4121` (`panel::frame` in
`CantusApp::frame`); `cantus-ui/src/ai_catalog.rs` (the prompt list);
`cantus-ui/src/views/mixer_menu.rs:80-81` (`Act::MasteringAssistant`), `:589` (the master
strip's menu), `:885-886` (the master chain's *Add effect* list), `:1422` (its action).

## Review 1 (12 findings, each fixed with a test that fails without it)

1. **Pre-roll on a fractional sample** — `SeekFrac` starts the transport on the whole sample a
   render from the top reaches; a range's length is counted from the top. Test
   `a_preroll_is_the_render_from_where_it_starts` at 116 BPM with 0.73 / 1.37 s pre-rolls
   (failed: frames off). Re-measured: the showcase still does not converge — the cause
   is **Space's modulation** (`preroll_by_unit`), and ADR 0083 says so instead of the
   unproven "free-running modulation".
2. **Replacing the user's master inserts** — `apply_master_chain` (Edit) refuses it;
   `replace_master_chain` (Destructive) does it and names the inserts it removes; both refuse
   a proposal from an older revision (`Stale`); the panel's *Apply* names what it takes out.
   Test `replacing_the_users_master_chain_is_destructive_and_a_stale_proposal_is_refused`.
3. **Nothing forgot a project** — the application's epoch: checkpoints, proposals and the
   panel's answer forgotten when another project replaces the open one (every listen, apply
   and frame checks it). Test `another_project_forgets_the_last_ones_checkpoints_and_proposals`.
4. **Variants evicted the song's checkpoints** — kept side by side, least recently used
   first. Test `a_variant_keeps_the_songs_own_checkpoints`.
5. **Uncopiable for other reasons rendered from the top in silence** — streamed samples and
   frozen strips caught up front; a failed fork remembered with its reason
   (`Processor::fork_refusal`) and the same setup pre-rolls saying why. Tests
   `a_streamed_sample_pre_rolls_and_an_inexact_render_keeps_no_checkpoint`,
   `a_fork_that_fails_says_why`.
6. **The master strip after the chain** — `Post` (width, fader, balance, polarity, as the
   engine) in every trial; a trial over the ceiling lowers the limiter's; automation of the
   master's level/balance/width left out of the mix and warned of. Test
   `the_master_strip_after_the_chain_is_in_the_prediction` (width 1.6, +2 dB: the real
   render under −1 dBTP).
7. **No length limit** — 20 minutes per listen, 15 when audio is kept; refused with
   "choose a range"; in the schema. Test `a_range_too_long_is_refused`.
8. **Checkpoints from an inexact render** — never; the streamed-sample test puts a section
   inside the pre-roll.
9. **A forgotten processor field would compile** — `every_field_is_accounted_for` names every
   field of the processor and its nodes without `..` (it found `input` and `record_rings`).
10. **`file_uri`** percent-encodes (RFC 8089). Test with a space, an accent and `#`.
11. **The reference** — claimed in review 1 but **not in the code** (the edit was lost
    before its commit; review 2 found it): now opened through `cantus_ai::open_touch` and
    decoded from that file; test `the_reference_heard_is_the_file_opened_at_the_call` (a file
    swapped in at the path after the call is not what is heard).
12. **`correlation_min`** counts whole windows only. Test
    `the_lowest_correlation_ignores_a_last_scrap_shorter_than_a_window`.

## Review 2 (13 points)

1. **HIGH — a project switch could freeze the window**: `forget()` locked the checkpoints a
   worker holds for its whole render. Now it raises the old runs' `stale` flag (agents'
   listens and the assistant's trials stop at their next block, a late proposal lands in the
   old store), cancels the panel's worker, and swaps in fresh stores without locking. Test
   `another_project_never_waits_for_a_running_listen` (the switch under 200 ms while a listen
   holds the lock; the listen ends cancelled).
2. **Medium — #11 was missing**: done (above).
3. **Medium — the 15-minute cap**: in frames now (20 minutes at 48 kHz, five at 192 kHz), so a
   whole long song can be mastered; the panel refuses in the interface's language and says
   to set a loop (`listen.panel.too_long`). The trials read the mix in blocks (no
   whole-length channel copies). Test `the_cap_on_kept_audio_is_in_frames`.
4. **Medium-low — variants still evicted**: room for 24 (two setups of a dozen sections); a
   same-setup checkpoint from before an earlier edit is replaced at its place. Test
   `a_dozen_sections_and_a_variant_fit_and_a_late_edit_replaces_only_its_own`.
5. **The last pass lowered the ceiling without searching**: lowered only when another
   search follows; the limiter's reason quotes its own ceiling. Test
   `the_limiters_reason_quotes_its_own_ceiling`.
6. **The Apply key overflowed**: a count on the key ("replace the master's 6 effects"), the
   names in full under the row; shot with six master inserts, the body scrolled to its end
   (`docs/round34/listen/mastering-proposal-end-*.png`, English 100 %, Spanish 200 %).
7. **The prompt** names `replace_master_chain`; `ai_listen` lists it and checks the prompt.
8. **`forget()` left the A/B playing**: stopped at the next frame. Test
   `another_project_stops_the_ab`.
9. **The cells** (`TrackCells`, `StripCells`) are in the fork's pattern.
10. **Transient refusals**: `Refusal::lasting`; only lasting ones remembered; an uncopiable
    setup never forks nor checkpoints. Tests `a_snapshot_waiting_for_the_grid_is_a_passing_refusal`
    and the exact render in `a_streamed_sample_pre_rolls_and_an_inexact_render_keeps_no_checkpoint`.
11. **The project-wide stream check**: its trade-off stated in ADR 0083.
12. **`SeekFrac` in `rt_memory`**: counted on the fork (no allocation).
13. **No DSP change this round** (the integrator's decision). ADR 0083 attributes the
    pre-roll's limits as measured — Space's FDN rotors (8.3 dB) and, now that
    `preroll_by_unit` covers the instruments, Organum's rotary (−1.4 dB; Chorda 36, Modulus
    43, Lamina 64 dB with tails still ringing after 3 s; the rest 132 dB or exact). BACKLOG
    §4b: free-running unit state anchored to song time.

After review 2 (88d8d0b6): Windows — my tests pass (`cantus-ui` listening 17, `ai_listen`
3; engine `listen` 14, `rt_memory` 25, the fork's 2); workspace clippy clean but for
`cantus-ui/tests/transcribe_convert.rs:486` (`SaidArg` has no `Display`: the transcribe
builder's test against mcp's contract change). Linux (WSL): clippy the same one error;
`cantus-dsp`, `cantus-engine`, `cantus-host` **1 855 passed**, 0 failed; that test keeps
`cargo test -p cantus-ui` from building, so a targeted run (`listen-wsl-ui.sh`: the lib,
`ai_listen`, `listen_measure`, `ai_mcp_pump`, `ai_mcp_boot`) — **418 passed**, 1 failed (the
top bar's layout test, below). The WSL clone's `target/` was deleted.

After the fixes (3c2ca4b8): workspace clippy clean on Windows and Linux; Linux `cantus-dsp`,
`cantus-engine`, `cantus-host` 1 840 passed, 0 failed; `cantus-ui` 2 076 passed, 5 failed —
none in this builder's code, all in the merged tree's other areas: `views::transport::layout::
tests::short_groups_share_a_column_two_high` and `topbar_layout::every_group_keeps_to_its_own_width`
(the top bar's new agent chip), `ship_frictions::every_settings_page_reads_whole_in_both_languages`
(Settings ▸ AI), `font_coverage` (Δ ♭ ♯ in `cantus-core/src/harmony_chord.rs`) and
`agent_tools::compose_a_song_through_the_server_render_it_and_undo_it_all` (`set_session`:
"slot.scene_id must be at least 1"). The WSL clone's `target/` was deleted.

## Review 3 (six items)

1. **The proposal lost across a switch** (a regression of review 2's store swap):
   `master_assist` took the proposals store before `prepare()`, whose epoch sync replaced
   it. `Prepared` carries the store it was prepared for; the frame syncs the epoch before
   `ai_mcp::frame` (`app.rs:4069`). Test `a_proposal_made_across_a_switch_is_found`.
2. **The panel's copies**: the proposal's own readings give both loudnesses (no unzipping
   closure), and the audio stays `Arc<Vec<f32>>` (no `.into()` copy). **Measured** at the
   cap — a 19-minute song through the panel's worker (`listen_measure::
   the_assistants_memory_at_the_cap`, debug): **2.8 GB peak** of working set, 2.8 GB held
   after. Of it, 0.9 GB is the mix and the mastered audio, and **1.9 GB the 24 checkpoints
   — 80 MB each, whatever the song** (`the_checkpoints_memory_on_a_long_song`: 71 places,
   24 kept, 1 937 MB held, 21 MB once dropped): a fork copies every cell of the engine's
   256 tracks and 256 strips. The "0.9 GB" in `MAX_AUDIO_FRAMES`' doc, ADR 0084 (which
   still said 15 minutes) and ADR 0083 ("twelve at most") corrected; BACKLOG §4b: fork only
   the cells in use, or budget checkpoints in bytes. The integrator decided to fix it this
   round: below.
3. **Over the ceiling when the searches ran out**: after the search, the gain comes down by
   the excess until the true peak is at or under the ceiling, and the proposal says how far
   under the target that leaves it (`short_of_target_lu`, a warning in the tool's answer, a
   line in the panel, `listen.panel.short`). Test `the_proposal_is_never_over_the_ceiling`
   (`propose_passes(.., 1)` with the master strip at width 2 and +4 dB: one search leaves
   it over; the shortfall said matches the loudness got).
4. **"replace the master's 1 effects"**: `tf_count` with `listen.panel.apply_replace_one`;
   the counted sentences of `set_mix_targets` nest a counted phrase (`listen.phrase.tracks.
   one/many`), both locales. Test `counts_of_one_read_in_the_singular`.
5. **The rounded too-long message**: `min_sec` — "the range is 33 min 16 s long; choose a
   range of at most 20 min" from the tools; the panel says "The song is 20 min 20 s long …
   Set a loop", or with Loop chosen "The loop is 20 min 18 s long … Make the loop shorter"
   (`listen.panel.too_long_loop`). Tests `the_panel_says_a_song_is_too_long_and_what_to_do`
   and `a_range_too_long_is_refused`.
6. **Test gaps**: a passing refusal not remembered and a later place still checkpointing
   (`Checkpoints::note_refusal`, engine test
   `a_passing_refusal_is_not_remembered_and_a_lasting_one_is`); fix 5, the switch and the
   panel's too-long path (above).

After review 3 (bb183e7f): Windows — `cantus-ui` listen tests 21 passed, `ai_listen` 3,
`ai_mcp_boot` 1, the engine's `listen` unit tests 5; workspace clippy clean, `cargo fmt
--check` clean for the crates changed. Linux (WSL): workspace clippy clean; `cantus-dsp`,
`cantus-engine`, `cantus-host` **1 856 passed**, 0 failed; `cantus-ui` **2 111 passed**, 1
failed — `ai_mcp_boot`'s tools-list golden, still holding the range description from before
review 2 (written again in bb183e7f, passing on Windows). The WSL clone's `target/` was
deleted.

## Checkpoint memory (the integrator's decision after review 3)

Merged `r34/integration` at 381020aa first (no conflict).

1. **Only the cells in use.** Where a whole engine's 75 MB went
   (`listen_fork::tests::where_a_processors_memory_goes`): 256 track cells, 46 MB (a drum
   machine's pads, 120 KB, in every one), 256 strips, 7 MB, and 192 delay lines of 64 KB,
   12.6 MB. `Cells` names the slots a render's lineage was ever given (noted after every
   install and every block); a checkpoint keeps an *image* — `Processor::with_cells`,
   `Nodes::with_cells` with only those cells, fresh lines left out (`DelayLine::is_fresh`)
   — never played; a render resumes from a full processor copied back from it. The fork
   still names every field of the processor, its nodes and the cells. Bit-identity: every
   test of `tests/listen.rs` (13) passes through images, `rt_memory` (25) and the new
   `an_image_holds_the_cells_in_use_and_plays_on_from_them_to_the_bit`. Marked lines:
   `processor.rs` (`with_cells`, `pub(crate) use listen_fork::Cells`), `graph/exec.rs`
   (`Nodes::with_cells` in place of `new`), `pdc.rs` (`is_fresh`).
   **Measured, MB per checkpoint** (working set with the checkpoints and without, debug):
   a one-track song 5.7; the 19-minute one-track song 7.9 (24 kept: 205 MB held, 17 MB
   without; was 80 each, 1.9 GB); **the showcase (*heart*, 170 s, 17 tracks) 40.3** (10
   checkpoints); house (124 s) 26.4, ballad 32.5, funk 19.0, lofi 21.7, ambient 25.7, trap
   26.9. At the assistant's cap the peak is **1.1 GB** (was 2.8): 0.9 GB of mix and
   mastered audio, 190 MB of checkpoints.
2. **A budget in bytes** on top of the count: `DEFAULT_CHECKPOINT_BYTES` 512 MB
   (`Checkpoints::budget_bytes`, internal), each image estimated when taken
   (`Processor::approx_bytes`: cells, lines as they are, a fixed part and a figure per unit
   kind measured four of a kind sixteen times — `what_each_unit_adds_to_an_image`), the
   least recently used going past it. The estimate against the working set: −8 % (trap) to
   +25 % (the showcase: 50 for 40). Test `the_checkpoints_keep_to_their_budget_in_bytes`.
   `a_dozen_sections_and_a_variant_fit…` now sets no budget: the rich test project's every
   unit makes its images ~40 MB, and it tests the count.
3. **Released when quiet**: after ten minutes with no listen begun or ended
   (`listening::QUIET`), checked at every frame's epoch sync, never waiting for a running
   listen (it holds the lock). Test `quiet_listening_lets_the_checkpoints_go`.
4. ADR 0083 (decision 3, consequences, tests), ADR 0084 and `MAX_AUDIO_FRAMES`' doc carry
   the measured figures; the BACKLOG §4b item is gone (nothing of it is left).
5. **The review-3 tests against the old code**, reverted locally all at once, run, restored
   (`git checkout`): `a_proposal_made_across_a_switch_is_found` FAILED ("the proposal was
   kept where apply looks": `apply_master_chain` found nothing in the new store);
   `the_proposal_is_never_over_the_ceiling` FAILED at the true-peak assertion (over −1
   dBTP with the gain-lowering loop off); `counts_of_one_read_in_the_singular` FAILED at
   its first `assert_eq` ("Apply: replace the master's 1 effects" without the `_one` key).
   The other 16 listen tests passed reverted.

## Only the owner's

- **Listening**: the assistant's masters on real material, and the A/B — the curves are
  guides drawn from published averages, not anyone's ears.
- The platforms' numbers are checked against public documentation as of 2026-10; Apple
  Music, YouTube, Amazon Music and SoundCloud do not publish theirs (the table says so).
