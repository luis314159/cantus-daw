# Round 26 — sampler: Exemplar streams from disk, loads off the interface's thread, and plays pedal noises

Builder: **sampler** · branch `r26/sampler` · base `1b4a942` (r26/integration), merged with
`r26/integration` at `619806c` before the end · ADR **0050** (new), ADR 0023 amended.

## Done

1. **Streaming from disk (ADR 0050).** A sample no zone loops or reverses, longer than its
   head, keeps only its head in memory (16 384 frames of level 0, rounded to blocks, plus
   the latest start any zone gives it) and streams the rest in 4 096-frame blocks:
   - decoded **once**, in parallel across cores, into a **cache file at the exact depth**
     (`i16`/`i24`/`f32` chosen from the values, so converting back gives the same bits; the
     Salamander is `i24`, 6 bytes a stereo frame instead of 8), named by a hash of the
     source's path, size and time, written beside its name and renamed; later loads read
     heads only;
   - **mip levels are not stored**: the reader computes a level-*k* block from level
     *k − 1* with `halve_span`, which `halve` itself now calls (bit-identical by
     construction; test);
   - **one reader thread** (plain `std`, no `unsafe`), a **seqlock block pool** of
     `AtomicU32` slots in `cantus-dsp` (safe Rust; grows in chunks of 256 blocks), **block
     tables** per stream (every voice of a sample shares its blocks), **boards** where each
     layer publishes (stream, level, block, near-a-level-change) every control step; the
     reader reads the first blocks after the head as soon as a voice starts, then always
     three ahead, nearest first, the neighbouring mip level only when the pitch is near the
     hysteresis edge, and evicts the least recently wanted block nobody wants;
   - each **layer copies** what its next control step can read (taps, the decimator's
     history and a 5 ms fade's worth more) into its own window, so the resampler reads plain
     slices on exactly the in-memory paths — **streamed = in memory to the bit**;
   - a **late block in real time** fades the voice out over ≤ 5 ms with audio the window
     already holds and counts it (`EngineStats::stream_late`, shown on the status line as
     *Disk late: n* with a tooltip, and in Exemplar's telemetry); **offline** (`render.rs`,
     the stems: `InstrumentUnit::set_offline`) a voice waits, so offline = real time whenever
     real time had no late read; a source that becomes unreadable is marked failed and ends
     its voices instead of waiting forever.
2. **Loading off the interface's thread (the review's item 15, ADR 0050 §7).** In a live
   engine every instrument file loads on loader threads (cores − 2, at most 8) — `attach`
   queues what is missing and returns at once; zones are silent until their sample is in
   and the next snapshot (every frame polls) carries it. *Loading samples… n of m* on the
   status line and in the instrument's footer (both languages); the UI wakes at 20 Hz while
   loading; an undo during the load takes the files nobody asks for off the queue (the
   progress counts only what is wanted); the export waits for loads
   (`EngineHandle::compile_for_render`); the loaded samples survive a device restart; the
   built-in sounds (285 ms, once per process) are computed beside the window at start-up.
   **A note that starts before its sample arrives** plays nothing for that zone and does
   not start it later — the missing-file rule, deterministic; the next note plays it.
3. **The importer's metadata reads skip the audio**: `meta_of` read every file whole (the
   Salamander's 641 FLACs: 7.9 s on the interface's thread); it now reads chunk headers and
   bodies except the audio (WAV/AIFF) and answers a FLAC from its first 12 bytes — **0.1 s**.
4. **Pedal noises: zones a controller starts.** `Trigger::Controller` (`z.trig` = 5) with
   `z.cc`, `z.cclo`, `z.cchi` (**schema 24**, migration writes nothing, fixture `v24.cantus`
   with a pedal zone). A zone starts on the sample after its controller crosses into its
   range from outside, at its root, full velocity, one round-robin turn and random number
   per trigger, chokes first, owned by no note; a looping zone is held while in range and
   released when the value leaves it, one that does not loop plays to its end (as SFZ
   players do — Salamander's pedal-up silences pedal-down by its class). Units hear every
   controller by MIDI number (sustain = 64). The SFZ importer maps `on_loccN`/`on_hiccN`
   (`-1` = no trigger, consistent with the content builder's rule on integration) and
   writes them back; the map leaves such zones out, the zone card shows *CC · CC from ·
   CC to*, the trigger menu has *Controller*. **The four pedal-noise files are back in the
   pack** (no `exclude`; 643 files, 746 854 368 bytes — the real archive installs with
   exactly that).

## Measurements (this machine, test profile — `cantus-dsp`/`-engine` at opt-level 3,
`claxon` at 1; two other builders compiling at the same time make the cold loads vary)

| Salamander | Before (content builder) | After: first load (decode + cache) | After: later loads | Memory after |
|---|---|---|---|---|
| 8 layers (401 zones) | 16.9 s on the UI thread, 1 914 MiB | 2.4 s quiet … 14.2 s busy, off the UI thread | **0.24 s** | **75 MiB** heads + 16 MiB pool |
| 16 layers (641 zones) | 32.4 s on the UI thread, 3 732 MiB | 7.5 … 13.0 s (busy), off the UI thread | **0.28 s** | **120 MiB** heads + 16 MiB pool |

- Cache on disk: 963 MB for the 8-layer instrument, 1.9 GB for all 641 files (FLAC: 745 MB).
- Same sound: triad −24.4 LUFS / riff peak −0.5 dBFS (8 layers), −26.6 / −0.5 (16) —
  exactly the in-memory figures (`pack_sounds.rs` ignored test, `CANTUS_STREAM=1`).
- Import (SFZ + metadata): 7.9 s → 0.07–0.1 s. Pick + snapshot with a slow decoder:
  ~5 ms (the test holds < 100 ms).
- **Reader latency** (`inst_stream_stress.rs`): 40 voices on 40 samples starting at once —
  every first block after the head in the pool within **18–21 ms**; the head lasts 171 ms
  at twice the rate, 341 ms at unity: an 8× margin, the basis of `HEAD_FRAMES`.
- **Late reads under stress**: every block read slowed by 4 ms, 40 voices paced in real
  time: all 40 ended late, each faded (steepest output step 0.0126, against 0.018 the sines
  and 5 ms fades allow; a cut voice would step by 0.02); plain disk: 0 late.
- **CPU per voice** (`exemplar_stream_tests.rs::measure_cpu_per_voice…`, best of seven,
  24 stereo voices, every block in the pool): transposed 0.57–0.88 % of a core in memory,
  −2 … +14 % streamed; at unity 0.137 % in memory, +12 … +18 % streamed (the window's copy is
  a larger share where the read is a plain copy). Within this shared machine's noise for the
  resampled case.

## Tests (each seen failing first, or a guard checked by mutation)

`cantus-dsp`:
- `exemplar_tests3.rs` (8): a controller crossing into its range starts its zone on the
  next sample, keys never do, no crossing no layer; a value already inside triggers
  nothing; pedal-up chokes pedal-down without a click; a random pair plays one of two and
  repeats exactly; a controller zone plays its file at its root bit for bit; block-size
  independence with controller zones; a looping zone held while in range, released when it
  leaves; at rest again once the pedal is up and the count forgotten. (All 8 failed with the
  trigger removed.)
- `exemplar_stream_tests.rs` (6 + 1 ignored): the pool's seqlock hands back what was written
  and never a recycled block; `halve_span` = `halve` over any range; **streamed = in memory
  to the bit** (unity mono and stereo, resampled, nineteen semitones up on level 1, a bend
  crossing into level 1, the parallel render; blocks of 512, 37, 4 096); a late block in
  real time fades over ≤ 5 ms, never louder than the voice, counted once; offline waits for
  blocks arriving from another thread and equals in memory; the board names the block read
  and forgets it when the voice ends. (Streamed tests fail with the window disabled.)
- `cache`/`stream` unit tests in the engine: exact depth i16/i24/f32 round trip, a cache
  file refused once its source changes, the prefix covers the filter's reach.

`cantus-engine`:
- `inst_sampler.rs::the_sustain_pedal_and_any_controller_start_the_zones_they_cross_into`.
- `inst_stream.rs` (4): a streamed instrument renders offline exactly as in memory (which
  files stream, the head only in memory, the status names the whole file, shared streams;
  **fails with offline waiting off**); paced real time with the real reader: no late read,
  same bits; background loading with progress, the wait, and an undo that empties the
  queue; a note before its sample stays silent and the next one plays.
- `inst_stream_stress.rs` (2): a slow disk's late voices faded and counted; the reader's
  latency for 40 new voices well within the head.
- `rt_memory.rs::streaming_voices_never_allocate` — serial and with three workers, through
  copies, cursor publication and late fades.

`cantus-project`: `schema_24_keeps_a_zone_the_sustain_pedal_starts` (+ every fixture loads).

`cantus-ui`:
- `pack_sounds.rs`: the pedal regions become controller zones (and write back);
  `-1` keeps a keyed region (the content builder's test, adapted: `0…1` is now a zone);
  the fixture pack plays its pedal noise on the pedal; choosing the piano loads off the
  interface's thread and shows how far (EN/ES, status line and footer); undo during the
  load lets go, and the export waits; the real Salamander streams (`CANTUS_STREAM=1`).
- `inst_sampler.rs::file_metadata_skips_the_audio_and_finds_chunks_after_it`.
- `cantus-content/tests/tar.rs` (ignored, real archive): 643 files, the pedal noises in.

## Pictures

`cargo run -p cantus-ui --example shoot_sampler -- <dir> <scale> <en|es>` →
`F:\projects\daw-r26\shots\sampler\` (24 PNGs: 75/100/200 %, EN/ES, Night/Paper): the zone
card of a pedal-started zone (*Controller* in the trigger menu, *CC · CC from · CC to*;
looked at — the first labels read *FROM / TO VALUE*, unequal, now *CC FROM / CC TO*) and an
Exemplar loading (*Loading samples… 0 of 12* in the status line and the footer, nothing
overlapping at 200 %).

## Files outside my area (all additive)

- `crates/cantus-engine/src/{samples.rs, backend.rs, render.rs, stems.rs, messages.rs,
  processor_pass.rs, graph/exec.rs, graph/track.rs, inst/mod.rs}` — the asset loader, the
  streaming switch, `compile_for_render`, offline units, `stream_late`, trait hooks.
- `crates/cantus-ui/src/app.rs` — the cache folder and built-ins warmed at start-up, the
  samples kept across `restart_audio`, the 20 Hz wake while loading.
- `crates/cantus-ui/src/views/{workspace_status.rs, export.rs, inst/frame.rs}` — two status
  readouts, the export's waiting compile, one line for the footer chip.
- `crates/cantus-content/{manifest/content.json, tools/make_manifest.py, tests/tar.rs}` —
  pedal files included, size, the description's memory.
- `crates/cantus-ui/src/views/inst/sampler_import.rs` (`meta_of`).

## Notes for the integrator

- **Schema 24** (round rule said none; my brief allows it because the project file must hold
  the new zone fields — an older build would otherwise play the pedal noises on the keys).
  Renumber if another branch bumped.
- No golden hash moved, no `DSP_VERSION` bump: `halve` now runs through `halve_span` with the
  same arithmetic; every engine null/golden test passes.
- No undo-fuzz arm: no new command shape (the new zone fields go through the existing
  parameter commands).
- `SampleCache::attach` loads native-instrument files in parallel even when not in the
  background; in a live engine (`compile` when `is_live`) it now also streams and loads in
  the background. Tests and tools are unchanged unless they ask (`set_streaming`).
- The reader and the hub are process-wide (`inst::stream::hub()`); there is no registry any
  more — a stream travels with its head (`SampleData::stream`, a new field in `cantus-core`:
  code constructing `SampleData` literally needs `stream: None`). The cache folder is
  `data_dir()/cache/samples` in the app, set in `CantusApp::assemble` — the boot's path —
  (limit in Settings: automatic by default, the smallest of 16 GB, a tenth of the disk and
  what leaves 5 GB free), `%TEMP%/cantus-stream-cache` otherwise. Tests that build the real
  app point `CANTUS_DATA_DIR` at a folder of their own (a test enforces it).
- Cache files are format 3 (`CNTSTRM3`, the whole source's hash): the format-2 files of
  earlier builds are swept and decoded again once.
- `cantus-dsp` gained `std::sync::OnceLock` and atomics in `exemplar_stream.rs` — still no
  threads, no I/O, no `unsafe`. `cantus-rt` gained `disk::space` (a volume's size and free
  space: `GetDiskFreeSpaceExW` / `statvfs`, the `Win32_Storage_FileSystem` feature).
- **Linux**: the cache's file locks are `std`'s (`flock`), `statvfs` behind `cfg(unix)`;
  not run through `tools/wsl/check.sh` here (it clones from the owner's tree, which builders
  do not touch) — please include it in the integration's Linux check.
- The real-Salamander measurements need `CANTUS_SALAMANDER`, `CANTUS_LAYERS`, `CANTUS_GAIN`,
  `CANTUS_STREAM=1`, `CANTUS_STREAM_CACHE`.

## After the review (2026-10-14)

Every finding was fixed on `r26/sampler`, each with a test (seen failing first by mutation or
by the reviewer's own reproduction where the old API cannot express the test):

| # | What | Fix | Test |
|---|---|---|---|
| 1, 3 | A read span wider than the window stalled offline and was cut as "late" | Runs are cut to what the window holds, down to one frame; reads wider still come from the pool into the same dot product; offline waits are bounded by time (10 s) and skipped while fading | `extreme_ratios_stream_exactly_as_in_memory_and_are_never_late` (Draft 5 octaves up, 96 kHz file, past the top level, +24 st bend; failed at frame 192 before), `an_offline_wait_for_a_block_that_never_comes_is_bounded` |
| 2, 21 | Reader woke at 1 kHz forever, locking and cloning | Own copies refreshed only on change; 1 ms while voices read, 20 ms while idle, parked when no stream exists | (behavioural; the stress tests still pass) |
| 4, 15, 26 | Loads an export waited for never reached the live engine | `wait_assets` flags the arrival for the next `poll_loaded`; the export no longer waits on the interface thread at all (16) | `loads_an_offline_render_waited_for_still_reach_the_next_poll` |
| 5 | A shared file gave zones more mip levels streamed than in memory | A stream holds every level; each zone keeps its own (`Source::limit_levels`, the in-memory `max_shift`) | `late_starts_and_shared_files_stream_exactly_as_in_memory` (legato across the lower zone's headroom) |
| 6, 13, 14 | Edits re-planned (and silenced) every file; streams never freed; a full registry played heads only | One stream per file as it is; zone starts kept ready by start pins; no registry — the stream travels with its head and is released when nothing holds it | `edits_keep_every_streamed_zone_playing_and_make_no_new_stream`, `a_stream_nothing_holds_is_released`, `a_late_start_is_kept_ready_and_plays_in_real_time` (late = 1 without the pins) |
| 7 | The cache could serve the old file for a new one | Identity (size, time, fingerprint of first/last 4 KiB) taken before decoding and checked after | `a_file_changed_while_it_is_read_is_never_cached_as_the_new_one`, `a_changed_source_is_never_answered_from_the_cache` |
| 8, 17 | An unwritable cache failed the sample | Plays from memory, said once on the status line | `an_unwritable_cache_plays_from_memory_and_an_old_file_streams` |
| 9 | Pre-1970 files failed | Signed times | `a_file_dated_before_1970_has_an_identity` |
| 10 | No integrity after a crash | Block checksums checked on every read (damaged: voice ends, file deleted, next load decodes); flushed beside the loads (before the rename it doubled a first load: 9.1 → 20.7 s per GB here) | `a_damaged_block_is_refused_and_so_is_a_damaged_header`, `a_damaged_cache_block_is_never_played_and_the_source_is_decoded_again` |
| 11, 20 | The cache grew without limit; stray parts | Settings ▸ *Sample cache* (limit, size now, *Clear cache*); sweep at start, after each batch and after a pack removal: stale parts, gone or changed sources, least recently used | `a_sweep_removes_stale_parts_gone_sources_and_the_least_recently_used`, `settings_show_the_sample_cache_and_its_limit_reaches_the_cache` |
| 12 | `-0.0` stored as `+0.0` | `F32` when any negative zero | `the_depth_is_the_narrowest_that_gives_the_bits_back_exactly` |
| 16 | Export blocked the window while samples loaded | The export waits beside the window: the dialog's progress reads *Waiting for samples*, *Stop* drops it | `an_export_while_loading_waits_beside_the_window` |
| 18 | No device: loads on the interface thread | `CantusApp::load_beside_the_window` at start, device or not | `without_a_device_instruments_still_load_beside_the_window` |
| 19 | Selecting a streamed zone decoded it whole on the interface thread, for good | Read on a thread of its own (from the cache file, else decoded), kept only while shown | `a_streamed_zones_waveform_is_read_beside_the_window_and_not_kept` |
| 22 | A controller's continuous loop stopped at its release | As a key's release: envelopes released, only a sustain loop lets go | `a_continuous_loop_keeps_looping_through_its_release_like_a_key_note` |
| 23 | A crossfaded controller zone was silent but counted | Crossfades skipped for controller layers | `a_crossfaded_controller_zone_still_sounds` |
| 24 | The importer took the first CC key even when unassigned | The first assigned controller | `the_first_assigned_controller_trigger_is_the_one_used` |
| 25 | The inspector drew controller zones on their root key | Left out (as in the map) | `the_inspector_preview_leaves_controller_zones_off_the_keys` |

### Measurements after the review (release build, quiet machine — no other build running)

| Salamander | Before (in memory, release) | First load (decode + write the cache) | Later loads | Memory |
|---|---|---|---|---|
| 8 layers | 7.8 s on the interface thread, 1 922 MiB | 10.0 s beside the window | **0.28 s** | **99 MiB** heads + 16 MiB pool |
| 16 layers | — | 7.8 s (the 240 files not yet cached) | **0.47 s** | **159 MiB** heads + 16 MiB pool |

- The first load is bound by this machine's write speed: decoding 400 files takes 1.76 s on
  8 threads, writing 400 × 2.5 MB takes 9.1 s even without our code (an isolated test of
  `File::write_all`; 20.7 s with `sync_all` before each rename — hence the flush beside the
  loads). It happens once per library, beside the window, with progress.
- Heads are larger than before the review (99 vs 75 MiB for 8 layers): a stream now holds every
  mip level of its file so that edits never re-plan it.
- **CPU per voice** (best of 15, quiet machine, 24 stereo voices, every block in the pool):
  before the window rework, resampled +1.2 … 1.5 %, unity +4.6 … 5.7 %; after it (runs, the
  early-out when the window holds the reads), resampled +3.3 … 4.0 %, unity +10.9 … 11.6 %
  (0.007 % of a core per voice, ≈ 3 ns per sample). The earlier "+12–18 %" was the busy
  machine. Reading in place from a pool block was rejected (ADR 0050 §4: atomics and the
  seqlock make it scalar and unable to reject a torn read already played).

## The last two, closed (2026-10-14)

- **A first load plays from its decode** while its cache file is written beside it, then
  switches to the file with nothing heard changing, and lets the decode go (one writer thread,
  at most 1 GiB of decoded files held; a loader waits beyond). An unwritable cache keeps the
  stream playing from its decode. Test: `a_first_load_plays_from_its_decode_until_the_cache_file_is_written`
  — a 1.5 s write delay; the load returns at once; renders before and after the switch both
  equal the in-memory render; the decode let go after it. Two writer threads (1, 2 and 8
  measured: 17.4 / 15.2 / 16–18 s for the 8-layer piano's first load). The first load's wall
  time here is ≈ 15 s, the same as the version that wrote in the loaders measured in the same
  conditions (13–15 s; 10.0 s on an earlier, quieter run): it is bound by this machine's
  writes (≈ 110 MB/s, a scanner looking at every new file). In the application each file is
  playable as soon as it is decoded, before its cache file is written; later loads 0.18–0.30 s.
- **Late starts at higher levels**: start pins carry the levels a zone's notes can start at
  (top key, tunes, transpose, a full bend up), the same time ahead at each; bound `count ×
  levels` blocks per zone. Test: `a_late_start_two_or_three_octaves_up_is_kept_ready_at_its_level`
  — +24 and +36 semitones, starting 200 000 frames in: no late read, the in-memory samples
  (it failed with level-0 pins only).

## The second review (2026-10-14)

Seventeen findings, all fixed on `r26/sampler`, tests first — each seen failing on the old
code or, where the API is new, by mutating the fix back out:

| # | What | Fix | Test |
|---|---|---|---|
| 4, 11 | The desktop app never set the cache folder, the Settings limit, the start sweep nor loading beside the window (they lived in `CantusApp::new`, which nothing calls) | `CantusApp::assemble` — the boot's `Stage::Assemble` path — starts them (`start_window_services`); `new` goes through it. Nothing else this round lived only in `new` (the font fallback is in the boot's font stage already) | `boot_window.rs`: assemble with a dummy engine — the cache under the (test's) data folder, the limit, streaming and background on, the start sweep tidying that folder; a source check keeps the services in `assemble` before `from_parts` and out of `new`, and every test that builds the real app off the user's folder |
| 12 | Without a device the opening project decoded whole on the interface thread, and stayed whole | The services start before `from_parts`' first `sync_engine` | the same test: assemble returns at once with the project's samples loading (a 400 ms decoder) |
| 3, 5, 15 | A damaged/vanished/unreadable block failed the stream for the session, silently in exports | The reader recovers: the source decoded again beside it, the stream plays on from the decode, a fresh cache file (only damaged or gone files are deleted); live, at most one note fades (counted); offline waits; an unreadable source fails the stream — the cache reloads (reports it missing), `render` returns `RenderError::Unreadable` naming the file; status messages once | `inst_stream_review2.rs`: `a_damaged_block_is_decoded_again_in_the_session_and_every_export_equals_memory`, `live_a_damaged_block_cuts_one_note_and_the_next_plays_as_in_memory`, `an_unreadable_source_fails_the_export_naming_the_file` (all three fail with the old reader) |
| 1 | The reader kept dropped units' boards alive (and their stale pins wanted) | The hub and the reader keep `Weak` boards, upgraded only while read; an Exemplar zeroes its cells as it goes; a unit's drop wakes the reader | `a_dropped_units_boards_are_let_go_and_zeroed` |
| 2 | Removed zones' sources and ended voices' playheads kept streams alive | `clear_zones` resets the sources; an ended layer lets go of its stream (`Playhead::let_go`) — never the last drop on the audio thread | `a_stream_is_released_while_its_unit_lives` (a zone removed; a voice ended), `rt_memory::streaming_voices_never_allocate` (a zone removed while its voices play: zero allocations and frees) |
| 6 | The sweep could delete a file a stream had just opened, or another Cantus's | A stream holds its cache file open with a shared lock (the reader reads through it: no reopen, no 64-handle limit); the sweep deletes only what it can lock exclusively, asks the hub at each deletion, re-checks the grace window | `a_sweep_never_deletes_a_file_a_stream_holds`, `a_file_touched_while_the_sweep_runs_is_kept` |
| 7 | An unplugged drive's sources counted as gone | Gone = changed, or absent from a folder that is there and holds something; a removed pack's folder is passed explicitly (`Hub::sweep_removed`) | `a_source_whose_drive_is_away_is_not_gone` |
| 8 | Concurrent sweeps over-deleted | One sweep per folder at a time (a mutex here, a lock file across processes); a file already gone counts | `sweeps_at_once_delete_only_the_excess` (4 sweeps at once) |
| 9 | A mid-file change with size and time kept was served old; the ADR overclaimed | ADR wording exact; cache format 3 stores the whole source's hash (the writer reads it beside the loads); a stream opened from its cache file is checked once beside the reader, paced (~500 MB/s at most): found changed → let go, file deleted, said once, decoded anew | `a_source_changed_in_its_middle_with_its_time_kept_is_found_out` |
| 10 | 16 GB flat could fill a small system disk | Automatic limit (default): min(16 GB, 10 % of the disk, free + held − 5 GB); no cache file written that would leave the disk under 5 GB free; Settings shows the disk, *Automatic* returns to it | `the_automatic_limit_keeps_to_16_gb_a_tenth_of_the_disk_and_5_gb_free`, the settings test ("This disk … GB free", "Automatic") |
| 13 | Waiting on audio clips showed 0 of 0 | `progress()` counts the audio loader too; a bar without a fraction and a plain line when nothing is counted | `the_loading_progress_counts_audio_clips` |
| 14 | `export_to` left the dialog open after a wait | The wait keeps its own choices and starts with them (`start_with`); the dialog is left as it was | `an_export_while_loading_waits_beside_the_window` (no dialog after the wait) |
| 16 | A note right after a start edit (or a new instrument) was silent in real time | `attach` computes the pins every Exemplar will publish (a superset: every level its zones keep), registers them and waits — ≤ 250 ms, only when they changed — for their blocks; `configure` wakes the reader | `a_note_right_after_a_start_edit_plays_in_real_time_as_in_memory` (late = 1 before) |
| 17 | Draft at an extreme ratio stalled offline 10 s | Decided once at the voice's start: a control step reading more than the reader keeps ready (> 384 frames a frame) plays its head and ends, alike offline and in real time, never late; wide reads after the start (a bend) are cut to what the pool holds, down to one frame | `draft_at_an_extreme_ratio_plays_its_head_alike_offline_and_in_real_time`, `a_bend_that_widens_the_reads_mid_voice_streams_offline_as_in_memory` (both 10 s stalls before) |

Suites after it: `cantus-engine` 443 passed (the fuzz build 447/447, `CANTUS_WORKERS=3`);
`cantus-dsp`, `-rt`, `-core`, `-project`, `-content`, `xtask` 812 passed; `cantus-ui` 958
passed, 1 failed — `font_coverage`, the mixer branch's `▸` in `cables.en.txt`, not this
branch's. Clippy (`--workspace --all-targets -D warnings`) and `cargo fmt` clean.

## The third review (2026-10-14)

Ten findings (one refuted), all fixed on `r26/sampler`, tests first — each seen failing on
the old code or by putting the old behaviour back:

| # | What | Fix | Test |
|---|---|---|---|
| 1 | One open file per streamed file, no budget: past 256 (macOS) / 1 024 (Linux) every open in the app failed | `cantus_rt::files` raises the soft `RLIMIT_NOFILE` to the hard limit (macOS: ≤ `OPEN_MAX`) when the hub starts (and in `init_process`); cache-file handles keep to a budget — a quarter of the limit, ≤ 1 024 — the least recently read closed and opened again (lock retaken) on demand, a file gone by then recovered | `inst_stream_fds.rs`: five times the budget's files stream on, handles ≤ budget, an unrelated file opens — on Windows (budget 8) and on Linux in WSL (a clone from a bundle of this branch, its own target folder, `RLIMIT_NOFILE` lowered to 128: budget 32, 160 files) |
| 2 | A removed pack's entry held at removal stayed for good | Removed folders remembered beside the cache (`removed-roots.txt`) until nothing under them is left; a folder there again is forgotten | `a_removed_packs_entry_held_at_removal_goes_at_a_later_sweep` |
| 3 | Stems came out cut short, silently | `stems::run` checks `stream_late` as `render` does: `StemsError::Unreadable` naming the file, the files written removed; `cantus-export` maps it (the mix and stems are the only export paths) | `stems_fail_naming_an_unreadable_sample_and_leave_no_file` |
| 4 | A recovery thread and a whole decode per failing stream; an Io fault kept the decode for the session | One recovery queue; each decode reserved against the writers' 1 GiB bound before it is made; an Io fault first reopens and rereads the kept file; every decode is written anew | `many_damaged_files_at_once_are_recovered_one_at_a_time` (peak 6 with a thread each) |
| 5 | The whole-source check re-hashed on every reopen | Once per (file, identity, hash) per session | `the_whole_source_check_runs_once_per_file` |
| 6 | The start-pin wait (≤ 250 ms) ran on the interface thread | `attach` hands the pins over and returns; `EngineHandle::set_snapshot` holds the snapshot back while they are read and `flush_pending` sends it from a later frame (≤ 250 ms) | `a_start_drag_never_blocks_the_compile` (slowest attach of a 40-step drag on a 5 ms-a-block disk: 34 µs), `a_snapshot_waits_beside_for_its_starts`, `a_note_right_after_a_start_edit_plays_in_real_time_as_in_memory` |
| 7 | Deep-level pins stalled the reader ~200 ms: another sample's voice went late | Two start blocks a look, then the cursors again; a level built from the one below in the pool, not the file again for every level | `start_edits_never_make_a_playing_voice_late` (10 ms-a-block disk; late = 1 with the old reader) |
| 8 | Live, a fresh note was cut from ratio ~16 (the reader slept 16–20 ms; the head lasted less) | Voices wake the reader on a new block (`Pool::with_wake`: `unpark`, no lock, no allocation); a cursor asks for as many blocks ahead as a 1 024-frame callback reads (`Cursor::ahead`); fast zones keep the blocks after their heads ready (Draft: level 0 only); head-only from a quarter of what the reader keeps ready per ms of output (64 at 48 kHz) | `fast_fresh_notes_stream_in_real_time_as_in_memory` (Draft 16, 32, 48; Normal: failed at 16 before the wake, at 32 before the lookahead) |
| 9 | A head-only note was silent about it; the ADR's "~103 semitones" | Counted (`stream_cut`): said once a session on the status line, a chip with its tip on the status bar, in an export's result (`Rendered::cut_notes`, `StemsDone::cut_notes`, `Done::cut_notes`); the Draft tip says it; the rule in the ADR and BETA.md by file rate and output rate | `an_export_counts_the_notes_that_played_their_start_only`, `stream_cut_status.rs` (told once; an export's summary), `draft_at_an_extreme_ratio…` (counted alike live and offline) |
| 10 | The automatic line was cut in Spanish; the knob showed a rounded/clamped figure | Two lines (the limit in force and its rule; the disk); the knob reads *Auto* | `the_automatic_cache_limit_reads_whole_in_spanish` (fails when the line elides); shot `sampler-cache-settings-*-es-100.png` looked at |

Also: Exemplar's memory chip shows `smp.footer_tip` on the window's help line when hovered
(the kit shell hands back its chips' rects) — `inst_sampler_footer.rs`, picture looked at.
Merged `r26/integration` (the plural helper): the new counted texts have `_one` keys.

Suites after it: `cantus-engine` 453 passed (fuzz below); `cantus-dsp`, `-rt`, `-core`,
`-project`, `-content`, `xtask` 813 passed; `cantus-ui` — see the report. Clippy and fmt
clean.

## The fourth review (2026-10-14)

Six findings, all fixed, tests first (seen failing on the old code or by putting it back):

| # | What | Fix | Test |
|---|---|---|---|
| 2, 3, 4 | The whole snapshot was held back (≤ 250 ms, restarted by every change): mute, arming, tempo, effects waited; a recording started meanwhile missed its armed track; the window recompiled every frame | A snapshot always goes at once; only the tracks whose new starts are not read keep their previous zone table (parameters, texts, samples); the newest follows from `flush_pending` (no compile) when the blocks are in, at most `HOLD_BOUND` 250 ms from the first change that waited — never restarted, then sent anyway and counted (`tables_late`) | `a_start_drag_never_holds_back_the_rest_of_the_project` (2 s drag, 20 ms-a-block disk: every mute in the same frame, the table at most 248 ms behind; 1.96 s with the bound restarted), `recording_during_a_waiting_zone_change_records_the_armed_track`, UI `a_waiting_zone_table_costs_the_window_nothing` (no compile in 30 frames, an added effect reaches the engine at once) |
| 1 | The voices' `unpark` was a steady-state, uncounted system call on the audio thread | A voice wakes the reader only when needed sooner than its next look (a fast voice, a new level, a block ahead not in the pool); every wake counted (`EngineStats::stream_wakes`); ADR 0050 §4 amends rule 3: the call per OS, when, its bound, where counted, the RTSan exception | `a_voice_wakes_the_reader_only_when_it_is_needed` (0 wakes crossing resident blocks; 25 with the old rule), `rt_memory` (counted) |
| 5 | A read error stuck on one block looped reader and recovery for good | The block that failed is read again, once per stream; an error that comes back decodes the source | `a_block_that_never_reads_is_decoded_from_the_source_once` (an injected bad block; the export equals memory, ≤ 2 recoveries) |
| 6 | Pins' mip levels were read from the file level by level | A start's levels are built in one chain: the level-0 span the deepest level covers read once, halved level by level | `a_starts_mip_levels_come_from_one_read` (four starts: 1.07 M frames each for a 1.05 M deepest span; the per-block path read up to 1.43 M; every level as in memory) |

Also: `boot_window`'s timing bound became a logical one — background loads held by a gate
(`assets::hold_loads`) until the test lets them go; assemble returns with them pending, then
they arrive. The streaming tests drain the hub's notes earlier tests left.

## The last pass (2026-10-14)

| # | What | Fix | Test |
|---|---|---|---|
| 1 | A bad block inside a start's chain span (outside the pin's own blocks) was dropped: the span re-read every look (1.25 GB in 3 s), the starts never ready | The chain's read fault goes to `recover()`; a multi-block read that fails finds its failing block, read again once, then the source decoded | `a_bad_block_in_a_starts_chain_is_recovered` (starts ready, ≤ 2 recoveries, < 6 M frames read, an octave up as in memory; the starts were never ready before) |
| 2 | Ordinary playback still woke the reader once a block (the check reached the newly wanted far block) | Only the blocks the previous cursor asked for are checked | `ordinary_playback_wakes_the_reader_not_once` (real reader, fresh note at unity, real-time pace: 0 wakes in 3 s on a normal and a 2 ms-a-block disk; 32 before) |

Also: `inst_stream.rs`'s cache folder is a guard — removed when the last test using it ends,
a panic included; folders of dead runs swept at the start (a lock on `.alive` tells a live
run); `the_suite_leaves_no_cache_folder_behind` runs the binary twice and finds none. A
free-space floor hook (`stream::set_free_floor`) lets the streaming tests write their small
cache files on a nearly full machine.

No test writes the shared fallback cache folder any more: a debug build (every test run)
refuses to stream without a folder set — the load fails, naming why — and neither reads nor
sweeps the fallback (`inst_stream_fallback.rs`, seen failing with the old fallback). The
pack tests' fixture sets its own folder and removes it (streams let go first); the
`shoot_sampler` example uses and removes its own.

## Left

Nothing of the brief or the reviews.
