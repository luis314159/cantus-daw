# Round 34 — stems: a song into its parts (ADR 0085)

Branch `r34/stems`. The first delivery (the helper, the runtime, models on demand) is in
`stems-d1.md`; this report covers the rest of the section and the first review's fixes.
ADR 0085 holds every decision and number.

## What was done

- **Separate stems…** on an audio clip's menu (the selection's audio clips when the clip is
  among them) and on a track's menu; a panel with the target in the user's words, 4 or 6
  stems, *Fast* or *Best*, the graphics processor if it can, the model's size, licence,
  runtime and provenance before its first download, progress, *Cancel*, *Hide*.
  Any sample rate: the file converted to 44.1 kHz and the stems back to its rate and
  length, so each stem clip is the original clip with another file under it. One undo step:
  files with provenance, a group, a bus named after the track (*Mix stems*) where the
  original fed, a strip and a track per part right under the original, the originals
  muted. The provenance card reads in the interface's language.
- **Schema 48**: `Project::provenance` beside the pool (the integrator's choice: no
  `AudioSource` literal changed), `SetAudioProvenance` with its exact inverse, removed and
  brought back with its source, carried by copy and paste, dropped at load for a file not
  in the pool; fixture `v48.cantus`; an undo-fuzz stream (`prng`); the "AI" badge.
- **Quality**: *Best* = demucs's shift trick with fixed offsets (four passes), held to
  PyTorch's at 70–90 dB.
- **GPU**: `device: auto` tries DirectML / CoreML and falls back, saying why. Measured:
  not usable on this machine (ADR 0085 §5); CoreML for the Mac session (row 17).
- **Settings ▸ Models**: every model with its card, accept-and-download, progress, cancel,
  retry, remove (the helper ended first).
- **The app's one ML service** (`ml_service.rs`) wired in `assemble`, polled in the frame,
  job updates forwarded off the frames; `MlStarter` for agents' workers (transcribe's
  name).
- **Packaging**: the helper on all three systems; the Visual C++ runtime beside it on
  Windows; mac-session row 17.
- **`separate_stems`**, an Edit-class job ending in `JobDone::Then`; its words name the
  target ("Separate “Song.wav” on “Mix” into 4 stems under a new bus").
- **SDR on MUSDB18-HQ's test set**: 8.80 dB average, the paper's numbers to their two
  decimals (below).
- The manual (`docs/manual/{en,es}/stems.md`), the strings (both locales), ADR 0085
  completed, screenshots in `docs/round34/stems/`.

## Review 1 (first delivery) — every finding fixed with a test

| # | Finding | Fix | Test |
|---|---|---|---|
| 1 | VC++ runtime not shipped | the four DLLs beside `cantus-ml.exe` (≥ 14.40), a missing one said in words | `package_ml_tests` (staged, not signed, not in the plug-ins' zip; the build machine's found) |
| 2 | a panic looked like a hang | `catch_unwind` → `Failed(Model("the job panicked: …"))` | `helper_life::a_panicking_job_fails_at_once…` (`panic_at`) |
| 3 | an idle helper never ended | ended after 3 idle minutes; `release()`; Remove ends it first | `helper_life::an_idle_helper_is_ended…`, `htdemucs_real::a_cancel…` (release, then remove) |
| 4 | Meta's MIT notice missing | notices' head, `models/LICENSE`, headers on the adapted files | `models::the_graphs_carry_demucss_licence` |
| 5 | library not hash-checked | `library_sha256` in the manifest; the helper checks it and the model's files before loading; `prepared()` by hash | `separate_pipeline::a_library_or_model_not_as_installed…`, `models::ready_names_every_hash…` |
| 6 | `kill()` kept the generation | bumped in `kill()` and `end_helper()` | `helper_life::a_hung_helpers_job_ends_once` |
| 7 | pickle without limits | depth 64, a million values (copies counted) | `models_pickle::a_nesting_bomb…`, `a_copying_bomb…` |
| 8 | stuck in Failed | weights lost with the maker's file gone → downloaded again | `models::lost_weights_are_downloaded_again` |
| 9 | test gaps | cancel inside a real run (34 ms); the helper dies with a killed Cantus; resume; runtime licence change; interrupted prepare | `htdemucs_real::a_cancel…`, `helper_life::the_helper_ends_when_cantus_is_killed`, `models::a_cut_download_resumes`, `…runtime_licence…`, `…interrupted_prepare…` |
| 10 | provenance card overclaimed | "the repository is under MIT; no separate licence stated for the weights" | the bundled list's test reads it |

What only a clean Windows VM can confirm: that the app-local VC++ DLLs are found on a
machine that never had the redistributable (the loader's order says so; this machine has it
system-wide).

## Review 2 (second delivery) — every point fixed with a test, each in its own commit

Tests in `crates/cantus-ui/tests/stems_review2.rs` unless named.

| # | Point | Fix | Test |
|---|---|---|---|
| 1 (high) | stems 6 dB quieter under the −3 dB law | the part strips and the bus start at the level that gives back the pan law's centre (+3.01 dB at −3, 0 at 0 dB) | `the_stems_play_at_the_originals_level_under_every_pan_law` — a **render null test**, original vs the sum of its stems, −3/−6/0 dB, with and without a strip: < 0.05 dB, > 40 dB SNR |
| 2 | muted clips came back | the clip's mute kept | `a_muted_clips_parts_stay_muted` |
| 3 | crossfades and comp seams lost | the clip cloned whole (fresh id, no link) | `crossfades_and_comp_seams_are_kept` (a comped track with a seam and an overlap; the render equal) |
| 4 | a run committed into the next project | `Run::epoch`; another project cancels it and closes the panel | `a_separation_never_lands_in_another_project` |
| 5 | an idle window stalled a run | workers wake the window; a frame with a run asks for the next in 100 ms | `a_worker_wakes_the_window_when_it_answers` |
| 6 | agent leftovers, late cancel, racing delete | converted input removed on every way out; `cancelled()` reaches the conversion; the panel removes files only after its worker answers | `an_agents_cancel_leaves_nothing_and_stops_the_conversion`, `a_panel_cancel_waits_for_its_worker` |
| 7 | no space check; Best's average silent | the peak room checked by disk before starting; the average says progress and asks cancel per block | `a_separation_is_refused_when_the_disk_lacks_room`; `separate_pipeline::the_average_says_its_progress_and_stops_on_a_cancel` |
| 8 | whole files in memory | conversion a block at a time (WAV read in pieces; the result the whole file's to the bit); files past a WAV's 4 GB refused up front | `a_block_at_a_time_is_the_whole_file_to_the_bit` (six rate pairs), `integer_wavs_convert_and_files_past_4_gb_are_refused` |
| 9 | hashes from the marker | from the build's compiled-in weight map | `models::the_hashes_come_from_the_build_not_the_marker` |
| 10 | Remove froze and raced | refused while the helper is in use; tried a frame at a time | `remove_waits_for_no_one_and_never_cuts_a_separation` |
| 11 | GPU switch dead on Windows | offered only where the runtime names a provider (CoreML on macOS); elsewhere "not available with this runtime" | `the_graphics_processor_is_asked_for_only_where_the_runtime_has_one` |
| 12 | licence row overclaimed | "{licence}: the model puts no limit on what you make; the music's rights are still its owner's" (and ES) | `the_licence_row_claims_no_more_than_the_licence` |
| 13 | several tracks took the first's routing | refused, in words, everywhere | `a_selection_over_several_tracks_is_refused` |
| 14 | a session clip id got an arrangement clip | arrangement clips only | `a_session_clip_is_not_an_arrangement_clip` |
| 15 | start hashed every model | marker, version, sizes at start; the hashes are the job's | `models::ready_names_every_hash_and_a_changed_file_is_not_ready` (rewritten) |
| 16 | VC++ notice and version | notice: Microsoft's terms, not the GPL, linked; version read from the DLL itself, `CANTUS_VC_REDIST` too | `package::ml_tests::the_runtimes_own_version_is_checked`, `…notice_keeps_microsofts_terms` |

## Review 3 (the review-2 fixes) — six small points, each with a test, each its own commit

| # | Point | Fix | Test |
|---|---|---|---|
| 1 | Remove allowed between an agent's helper jobs | a counter held around the agent's whole job, part of `in_use` | `remove_is_refused_between_an_agents_helper_jobs` (held in the gap) |
| 2 | one disk guessed from equal free bytes | `cantus_rt::disk::volume`: `GetVolumePathNameW` / `st_dev` | `disks_are_told_apart_by_their_volume`; `cantus-rt` `folders_on_one_volume_share_its_identity` |
| 3 | trim fixed at the pan law of the moment | `stems_ui::pan_law_command`: a law change trims every separation's strips in the same undo step (the mixer menu's one marked line) | `changing_the_pan_law_keeps_the_stems_at_the_originals_level` (render under −6, 0, −4.5 dB; undo exact) |
| 4 | refusals left empty `Stems/` folders | every check before any folder | `a_refused_separation_leaves_no_folder` |
| 5 | the panel-cancel test passed without its fix | `Run::hold_workers` holds the worker at a known point; cancelled there, the run stays and its folder stays | `a_panel_cancel_waits_for_its_worker` (rewritten) |
| 6 | the tool's words | one track, arrangement only, strips above 0 dB for the pan law | `the_tool_says_one_track_arrangement_clips_and_the_trim` |

## Integration review (r34/integration 381020aa) — six seams, each with a test

| # | Seam | Fix | Test |
|---|---|---|---|
| 1 | the VC++ DLLs in the install folder (an AI app's bridge held them during an update; plug-ins found our `msvcp140`) | `cantus-ml.exe` and its runtime in `ml\`: staged, installed, removed, waited for and zipped from the staged list; `ClientConfig::default_exe` looks there first | `package::ml_tests::the_helper_and_the_runtime_sit_beside_the_program` (the runtime only in `ml/`, the installer's lines), `helper_life::the_helper_is_found_in_its_own_folder` |
| 2 | an agent's stems left behind when its edit is not made | folders made on the worker; `Leftovers` until planned, then `Pending` until the edit is seen made — else removed | `an_agents_stems_go_when_its_edit_is_not_made` (denied, refused, made) |
| 3 | a model being removed could start | `stems_ui::models::ready` (not while `removing` names it), for the panel and the agent; `tend` drops a removal when something took the helper | `a_model_being_removed_cannot_be_started` |
| 4 | the agent's plan used the audio folder of prepare | read when the edit is planned | `an_agents_stems_follow_a_save_as_during_the_job` |
| 5 | `LICENSE` and the README's table without `cantus-ml` | added (and `cantus-sandbox`, missing too) | `package::ml_tests::every_crate_is_in_the_licence_summary` (LICENSE; the README is a document CI does not run for, so no test reads it) |
| 6 | `font_coverage` did not scan `models.json` | scanned beside `content.json` | `font_coverage::the_models_manifest_is_scanned` |

The agents' counter is now the service's: `MlService::hold_ml_agent()` (an `MlAgentHold`
guard) and `MlService::ml_agents()`; transcribe's `audio_to_midi` holds it too (one marked
line), and Remove's refusal names "a separation or a conversion". The font scan also found
a real gap: `▸` in the stems strings has no Plex glyph — now `›`, as the other areas write
it.

## Numbers

| What | Result |
|---|---|
| A 3-minute song in the helper (4 stems, *Fast*, processor) | 65 s, 2.66 GB peak (PyTorch: 59 s, 1.76 GB) |
| MUSDB18-HQ's 50 test tracks (3 h 8 min of audio), *Fast* | 1.96× real time with other builds on the machine (11 284 s in 5 752 s) |
| Our stems vs PyTorch's on the same song | 63–90 dB (a 3-min song: drums 84.9, bass 80.8, other 82.9, vocals 65.6; 6 stems 63.0–86.3); AM Contra's bass 75.7 dB |
| *Best* (shifts) vs `apply_model(shifts=…)` with the same offsets | 70–90 dB |
| A cancel inside a real run | stopped in 34 ms |
| *Separate stems…* at 48 and 96 kHz (converted to 44.1 kHz and back) | each stem the file's rate and exact length, its residual against the input > 50 dB SNR with the stand-in network |
| DirectML (ONNX Runtime 1.24.4, Radeon 680M) | refuses this graph (`0x80070057` at the 1-D transposed convolutions; as 2-D the next one fails); the community graph runs out of memory — so the processor (ADR 0085 §5) |

### SDR on MUSDB18-HQ's test set

The 50 test tracks of MUSDB18-HQ separated by the helper (*Fast*: no shifts, overlap 0.25,
the processor) and scored as the paper does: BSS Eval v4 through museval's own metrics
(1-second windows and hops, whole-track filters), each track's median over its windows,
then the median over the tracks (`crates/cantus-ml/tests/musdb_sdr.rs` separates,
`crates/cantus-ml/tools/musdb_sdr.py` scores).

| Source | Cantus, median (mean) | The paper (Table 1, HT Demucs + 800 songs) |
|---|---|---|
| Drums | **10.05** (10.14) | 10.05 |
| Bass | **9.78** (8.91) | 9.78 |
| Other | **6.42** (5.97) | 6.42 |
| Vocals | **8.93** (7.96) | 8.93 |
| Average of the medians | **8.80** | 8.80 |

Equal to the published numbers at their two decimals (drums 10.047, bass 9.785, other
6.424, vocals 8.934). The worst tracks are the music's, not the port's: on *AM Contra —
Heart Peripheral* (bass −9.59 dB) PyTorch's own separation scores the same, its stems 75.7
dB from ours. MUSDB18-HQ is for non-commercial research: downloaded under
`F:\projects\daw-r34\models\` only to measure, never committed.

## How it was tested

This branch's tests, all passing on Windows with `-D warnings` (84 new — 19 of them review 2's, 6 review 3's — and 4 ignored):

| Crate | Tests | What they hold |
|---|---|---|
| `cantus-ml` | 28 (+4 ignored) | protocol, STFT against PyTorch's, the job table, the pipeline with a fixture network (hash refusal, shifted passes, cancel, a crash, a hang, a missing helper), the helper's life (a panic, idle end and `release`, a hung helper's job ending once, the helper ending when Cantus is killed), *Best*'s average saying its progress and stopping on a cancel |
| `cantus-content` | 20 | the bundled list (licences, Spanish names and cards), install, resume, licence changes, interrupted prepare, lost weights, ready hashes, the pickle reader's limits, a plain-string text, the hashes from the build and not the marker |
| `cantus-core` | 3 + an undo-fuzz stream | `SetAudioProvenance`'s inverse, removal and undo bringing the card back, copy and paste |
| `cantus-project` | 2 | schema 48's migration and `v48.cantus`, a load dropping orphan cards |
| `cantus-ui` | 20 | the service made in `assemble` (fails if the wiring is left out), a job's updates arriving without a frame; *Separate stems…* at 48 and 96 kHz into aligned tracks in one undo step, a cancel leaving no file and no change, the agent tool's words and its one edit; review 2's fifteen (`stems_review2.rs`: the render null test, mute, comp seams, several tracks, session clips, block conversion to the bit, 4 GB, the room, the epoch, waking the window, cancels, Remove, the GPU switch, the licence row) |
| `xtask` | 5 | the helper and the four VC++ DLLs staged, not signed as plug-ins, a missing DLL said in words, the runtime's own version read from the DLL, the notice keeping Microsoft's terms |

Ignored, run here: `htdemucs_real` (4 and 6 stems, 20 s and 3 min against PyTorch;
*Best* against `apply_model(shifts=…)`; a cancel inside a real run), `install_and_separate`
(from a fresh folder over the network), `musdb_sdr` (the 50 test tracks).

Also run: `cargo test -p cantus-ui --lib i18n`, `cargo clippy --workspace --all-targets`
(`-D warnings`) on Windows, and the Linux check (WSL, `tools/wsl/check.sh`: workspace
clippy, then the tests of `cantus-ml`, `cantus-content`, `cantus-project`, `xtask`,
`cantus-core`'s provenance and undo fuzz, `cantus-ui`'s two test binaries and i18n; the
clone's `target` deleted afterwards). On Linux the first run found
one failure Windows hid: the feature tests shared the process's `CANTUS_DATA_DIR`, and a
test ending deleted its folder under another still writing there (Linux deletes open
files, Windows refuses). Fixed in its own commit (one test at a time, each with its own
folder) and run again on Linux: all passing. After review 2 the whole check ran again
(workspace clippy clean, plug-ins without their editor, the same tests with
`stems_review2.rs`): all passing, the clone's `target` deleted.

## Marked lines (files others own)

Every one carries `// r34:stems` (or `<!-- r34:stems -->`, `# r34:stems`):

- `crates/cantus-ui/src/app.rs`: the fields `ml` and `stems`, `ml()`/`ml_mut()`,
  `ml_service::wire` in `assemble`, `self.ml.poll()` and `stems_ui::frame` in the frame.
- `crates/cantus-ui/src/lib.rs`: `pub mod ml_service;`, `pub mod stems_ui;`.
- `crates/cantus-ui/Cargo.toml`: `cantus-ml`.
- `crates/cantus-ui/src/settings.rs`: `Settings::stems` (additive, `#[serde(default)]`).
- `crates/cantus-ui/src/views/workspace_settings.rs`: `SettingsPage::Models` (enum, `ALL`,
  key, body).
- `crates/cantus-ui/src/views/arrangement.rs`: the track menu's section; the badge's
  `ai` argument.
- `crates/cantus-ui/src/views/arrangement_audio.rs`: `badge_width`/`badge_text` take `ai`,
  the "AI" badge.
- `crates/cantus-core/src/command.rs`: `SetAudioProvenance` (variant, label, apply),
  `RemoveAudioSource`'s inverse carrying the card; `model.rs`: `Project::provenance`;
  `audio.rs`: `Provenance`; `lib.rs`: its export; `clipboard.rs`: the board's cards;
  `freeze_guard.rs`: the arm; `tests/undo_invariants.rs`: the `prng` stream and its
  assertion.
- `crates/cantus-project/src/format.rs`: schema 48, `migrate_47_to_48`, the load's repair;
  `examples/write_fixture.rs`: the v48 block; `fixtures/v48.cantus`.
- `xtask/src/package.rs`: `Inputs::beside`, `build_ml`, `CRT`/`find_crt`, the staging and
  file list, `mod ml_tests`; `package_tests.rs`: `beside: Vec::new()`; `notices.rs`:
  `ROOTS`; `xtask/notices/head.md`: three sections.
- `packaging/linux/build-appimage.sh`, `packaging/linux/flatpak/com.cantusdaw.Cantus.yml`,
  `packaging/macos/build-dmg.sh`, `.github/workflows/release.yml`: the helper.
- `docs/mac-session.md`: row 17; `docs/manual/{en,es}/README.md`: the link.
- `Cargo.toml` (root): from the first delivery.

## For the integrator

- **`MlStarter`** is in `ml_service.rs` from this branch too, with the same name and shape
  as transcribe's (`MlStarter(Arc<Shared>)`, `start`, `cancel`, `MlService::starter()`):
  keep one of the two at the merge; `stems_ui_run::separate_blocking` and the agent tool
  use it.
- **Manifest texts**: `cantus_content::manifest::Text` now also reads a plain string (as
  English), and a model's provenance card's `weights`, `training`, `quality` and
  `conversion` are `Text` (shown in the interface's language). A model entry written with
  plain strings still loads; code that reads those four fields as `String` needs
  `.get(lang)` (or `.en`).
- **Schema 48** is this branch's (`Project::provenance`); renumber if another branch took
  48. `fixtures/v48.cantus` comes from `write_fixture`.
- **`THIRD-PARTY-NOTICES.md`**: regenerate after the merge (`cargo run -p xtask --
  notices`); `xtask/notices/head.md` gained Demucs (MIT), ONNX Runtime and the Visual C++
  runtime.
- **rustfmt** realigned the two `// r34:mcp` comments next to `SettingsPage::Models` in
  `workspace_settings.rs`; nothing else of mcp's changed.
- The Windows release build needs the Visual C++ redistributable (14.40 or later) on the
  build machine: `xtask package` copies its four DLLs beside `cantus-ml.exe` and stops with
  a message when one is missing (GitHub's Windows runners have it).
- The ignored tests need the downloads under `F:\projects\daw-r34\models\` (their
  headers say which variables); `musdb_sdr` also needs MUSDB18-HQ, which is for
  non-commercial research and is never committed — only its numbers are.

## Owner-only

- Confirm on a clean Windows machine (or VM) that never had the Visual C++ redistributable
  that a separation runs — the DLLs beside `cantus-ml.exe` are found by the loader's
  order, but this machine has the redistributable system-wide.
- The Mac session (`docs/mac-session.md` row 17): the helper inside the signed bundle with
  library validation off, a separation on CoreML and on the processor, the time of each.
