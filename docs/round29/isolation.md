# Round 29 — isolation: third-party plug-ins out of process

Branch `r29/isolation` (worktree `F:\projects\daw-r29\isolation`), from the contract commit
7fde16b. ADR 0063. Schema 36 (on this branch the chain 32→33→34→35 are placeholders that
write nothing — the integrator replaces them with tempo's, warp's and controllers'). Undo
fuzz arm 46.

## Done

- **The design, by measurement** (ADR 0063 §1, §5): lock-step at the `Backend` seam — a
  `BridgedBackend` whose halves are proxies; the engine and everything above the contract
  unchanged. ADR 0005 rule 3 amended for this case (a bounded spin, then a kernel wait to a
  deadline; offline renders wait without one). ADR 0031 §6's external task and R15's async
  option weighed and not taken, with the reasons.
- **`cantus-rt` (MPL)**: `shm` (named shared memory, Windows and Unix) and `ipc` (the
  exchange, the lobby, bells as eventcounts, `serve`, `Closer`, `Completer`), model-checked
  over every interleaving of two callers and two workers on both bell semantics, and run
  between real processes.
- **`cantus-host` (GPL)**: `bridge::{slot, proto, sandbox, audio, main}` — the shared job
  area, the main-thread protocol with time limits, the helper processes (spawn without a
  console, Windows job object with kill-on-close, reader / writer / watchdog threads,
  heartbeat, hang limit, crash reasons), the proxies (value mirror, cached texts in the
  background, last-good state, crash notice, a dry block that keeps the plug-in's latency).
  `session_iso.rs`: where each plug-in runs (`backend_for_plugin` replaces every hard-coded
  `backend_for` of the session), moves, crashes, reloads, the CPU figure, crash counts.
  `PluginDb::crashes`. The scanner too now starts without a console window.
- **`cantus-sandbox`** (new GPL crate and program, its own version resource): the main
  thread (calls, windows, COM, news, heartbeat), a pool of processing threads per helper
  sharing a lobby.
- **The model** (schema 36): `ExternalPlugin::isolation: Option<PluginIsolation>`
  (`Within`, `Together`, `ByVendor`, `Individually`), `Command::{SetPluginIsolation,
  SetInstrumentIsolation}` with exact inverses, `cmd.set_plugin_isolation`, fuzz arm 46 with
  its counter, `fixtures/v36.cantus`, `schema_36_keeps_where_a_hosted_plugin_runs`.
- **The test plug-ins**: a hidden *Fault* parameter (abort or hang in `process`, abort in
  save / load state, activate, editor; armed only from `process`, so a reload comes back
  healthy) and a hidden read-only *Process* parameter (the pid) in `clap-gain`,
  `clap-synth` and both VST3 plug-ins.
- **The interface**: the crash card in the one corner stack shared with the audio notices
  (`audio_notice.rs` rebuilt as a stack; buttons wrap rather than overflow); *Reload* /
  *Keep it off* / *Reload all* / *Run individually* (after three crashes this session);
  reasons in both languages; *stopped* chip on the slot, a sign on a stopped instrument's
  track icon and an outlined channel button; the slot menu's *Runs* rows and *Reload
  plug-in*; the generic editor's *Runs* card and stopped card; a hosted instrument's *Runs*
  row, CPU row and Reload in the inspector; the slot's hint with where it runs and its CPU
  (and its own part when isolated, and late blocks); *Plug-ins late* in the status bar;
  Settings' plug-in manager: *Where third-party plug-ins run* and the plug-ins that
  stopped here (*Forget*). The machine default applied in `CantusApp::assemble`.
- **Measured**: the bench's `isolation` group (round trips hot and paced at 64/128/256,
  helpers' CPU per instance, the 32-Comprimo session), `docs/perf/isolation.md`, ADR §5.
- **Shipped**: `xtask package` builds, stages, checks and zips `cantus-sandbox.exe`; the
  installer installs and waits for it; signing policy list; notices `ROOTS`;
  `windows-release.yml` builds it; DEVELOPING says how to build it in development.
- **Docs**: ADR 0063; the manual's *When a plug-in crashes* (§10, both languages).
- **Shots**: `F:\projects\daw-r29\shots\isolation\` — `isolation-crash-*` (the whole window
  after a real crash: the card above an audio notice, the stopped slot, *Plug-ins late*),
  `isolation-crash-narrow-*`, `isolation-editor-*`, `isolation-manager-*`, in Cantus Night
  and Paper, English and Spanish, at 0.75, 1 and 2.

## Measurements (one run each; the machine shared with two builds)

| | 64 | 128 | 256 |
|---|---|---|---|
| Together, hot round trip (µs median) | 0.6 | 0.7 | 1.2 |
| Together, paced round trip (µs median / p99) | 11.7 / 54.8 | 36.5 / 52.3 | 34.0 / 276 |
| Together, helpers' CPU per instance | 0.55 % | 0.35 % | 0.20 % |
| Individually, helpers' CPU per instance | 5.8 % | 2.4 % | 1.5 % |

32 × Comprimo CLAP at 128 frames: within 593 µs, Together 773 (1.30×), Individually 1 125
(1.90×); native 563. The first design (one spinning thread per instance) measured 23.7 %
of a core per instance and 1.75×: the lobby replaced it. ADR 0031's per-group round-trip
budget is met; its 1.2× target for 32 instances is not (1.30× in the serial engine) — said
in the ADR, with why the default is *Together* anyway.

## Tests and what each holds

- `cantus-rt/tests/ipc_model.rs` — every interleaving of the protocol: no lost wake-up,
  one worker per job, results read only after publication; the model catches the classic
  late announcement. The full two-jobs-each model is `--ignored` (seven minutes).
- `cantus-rt/tests/ipc_process.rs` — round trips between two processes; two callers
  served together; a dead worker process closes and wakes; a late job finishes later.
- `cantus-host` unit tests — the slot carries a job and its result intact; a protocol
  frame round-trips.
- `cantus-sandbox/tests/bridge.rs` (9) — bits at 1/37/64/512 for CLAP and VST3 effects and
  instruments; each fault (process abort and hang, save, load, activate, editor) survived,
  told once, the last-good state answered; *Together* vs *Individually*; a missing helper.
- `cantus-sandbox/tests/session.rs` (8) — through the engine: bits; crash mid-song (the
  song plays on, equal to the bypassed reference from the crashing block), Reload restores
  the values to the bit; the saved project keeps the gesture's state; an instrument goes
  silent and comes back; moves and undo; the default moves followers; offline render equal;
  a hang never blocks the frames; crashes counted and remembered.
- `cantus-sandbox/tests/rt_alloc.rs` — the audio half allocates nothing (normal, latency
  line, reset, stop, dead).
- `cantus-core/tests/plugin_isolation.rs` (5), the fuzz's arm 46 and its counter.
- `cantus-project` `schema_36_keeps_where_a_hosted_plugin_runs`.
- `cantus-ui/tests/plugin_isolation.rs` (5) — the boot path applies the default; one card
  and its answers; the card and two audio notices never overlap (English and Spanish);
  the menu's choice is one undo step; the strings exist in both languages. `fx_lint`
  lists `plugin_notice.rs` and `audio_notice.rs`.
- `xtask` package tests — the helper staged, zipped, installed, removed and waited for.
- The existing `cantus-host` VST3 tests updated for the two hidden parameters.

## The runs

- Windows: `cargo test --no-fail-fast` of `cantus-rt`, `cantus-core`, `cantus-project`,
  `cantus-host`, `cantus-sandbox`, `cantus-bench`, `xtask` — 514 passed; `cantus-ui` — 1 296
  passed (after the card test was taught the translated reason). `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo fmt --all` applied.
- Linux (WSL, `tools/wsl/check.sh r29/isolation clippy`): clippy on the workspace and on
  `cantus-plugins --no-default-features` clean under `-D warnings`; the tests of
  `cantus-rt`, `cantus-host` and `cantus-sandbox` on Linux — 130 passed, the IPC model, the
  real-process exchange (shm + futex), the bridge and the session tests included (the
  editor test is Windows-only).
- The engine is untouched, so its fuzz was not rerun.

## Files outside my area (additive)

- `crates/cantus-ui/src/app.rs` — the `plugin_crashes` field and its default; the three
  `HostNotice` arms into `plugin_notice::on_notice`; `plugin_notice::apply_default` in
  `assemble`.
- `crates/cantus-ui/src/views/mod.rs` — `pub mod plugin_notice`.
- `crates/cantus-ui/src/views/inspector.rs` — `Act::Hosting` and its arm.
- `crates/cantus-ui/src/views/inspector_menus.rs` — `MenuId::Hosting`, `Choice::Hosting`,
  the slot's *Runs* rows, their arms.
- `crates/cantus-ui/src/views/inspector_channel.rs` — the *stopped* chip, the slot's hint.
- `crates/cantus-ui/src/views/inspector_hosted.rs` — stopped / Reload / Runs / CPU rows.
- `crates/cantus-ui/src/views/arrangement.rs` — one call: the stopped sign on the icon.
- `crates/cantus-ui/src/views/channel_rack.rs` — one call: the outlined channel button.
- `crates/cantus-ui/src/views/workspace_status.rs` — *Plug-ins late*.
- `crates/cantus-ui/src/kit/glyph.rs` — `Glyph::PluginStopped` (appended).
- `crates/cantus-ui/tests/fx_lint.rs` — two files on the kit.
- `crates/cantus-ui/examples/shoot_hosted.rs` — the isolation cases.
- `crates/cantus-core/src/{external.rs, command.rs, lib.rs}`, `tests/undo_invariants.rs`
  (arm 46, its counter and assert, in the driver).
- `crates/cantus-project/src/format.rs` (placeholders 33–35, migration 36, the test),
  `examples/write_fixture.rs`, `fixtures/v36.cantus`.
- `crates/cantus-bench` — `isolation.rs`, the group, the dependency on `cantus-host`.
- `.github/workflows/windows-release.yml`, `xtask/installer/cantus.nsi` (a comment),
  `docs/signing/code-signing-policy.md`, `docs/DEVELOPING.md`, `docs/manual/{en,es}`.

## Left

Nothing of mine. Owner-only: run the test version with a real third-party plug-in
installed, isolated, and judge the card and the feel of a crash; decide whether a session
built like the bench's (dozens of third-party inserts) should default to *Within* on a
weak machine — the setting is there either way.

## Notes for the integrator

- **Schema**: migrations 32→33, 33→34, 34→35 here are placeholders (no-ops): take tempo's,
  warp's and controllers' in their place and keep mine as 35→36; regenerate the fixtures
  after chaining (v36 holds the absent insert *Individually* and the hosted synth
  *Within*).
- **Known-issue lines this retires**: `docs/releases/v0.1.0-beta.1.md` l.142 (*Plug-ins
  made by others run inside Cantus: one that crashes closes Cantus*), the manual's §14
  first bullet (EN *A plug-in made by someone else that crashes closes Cantus*, ES *Un
  plug-in de terceros que falla cierra Cantus*), BETA.md blocker 3's known limitation,
  ADR 0019's "Not built: crash isolation".
- **`TransportInfo`**: the slot's `WireTransport` carries its fields one by one
  (`bridge/slot.rs`). If tempo adds a field to `TransportInfo`, add it to `WireTransport`
  and both conversions, and bump `bridge::proto::PROTOCOL`.
- **Freeze**: a frozen chain's hosted plug-ins can simply be unloaded as today; isolated
  ones then leave their helper (a helper with no instance quits).
- **Controllers**: hosted parameters still go through `SetPluginParam` / `automate`;
  nothing changes for an isolated plug-in.
- Run `CANTUS_TEST_PLUGINS_PREBUILT=1` to skip the nested cargo builds of the test plug-ins
  if the package lock is contended; the sandbox tests need nothing else (they use
  `CARGO_BIN_EXE_cantus-sandbox`).
- *Plug-ins late* counts jobs that missed their deadline, one per late job: a hang counts
  one, a crash none (its block is closed, not late).
- **Freeze and bounce** (merged from bbcc062): their renders go through `render_units` /
  `render_instruments` like an export, so they get the render helpers, the sliced waits and
  `cancel_render` (wired into `cancel_freeze`); `freeze::collect` asks `render_failures`
  before `finish_render` and commits nothing from a render that lost a plug-in (the file
  is removed, the job's content marked failed). A dormant slot is unloaded like any other:
  its `BridgeMain` drops, its exchange closes first, its last-good state is parked; a crash
  card for it is pruned. No command was added in the fix round (`freeze_guard` needs no
  arm).

## The fix round (review isolation-2: 53 findings, all closed)

Merged `r29/integration` first (bef6f62, then bbcc062 with freeze). Each fix has a test
seen failing first — by running it against the code with that fix disabled (marked *✗→✓*),
or, where the old code waited forever or lacked the API, by its absence. Tests:
`sandbox/tests/bridge_faults.rs` (13), `session_faults.rs` (8), `bridge.rs`, `session.rs`,
`rt_alloc.rs`, `cantus-rt/tests/ipc_process.rs`, `cantus-host` unit tests, `cantus-ui`
`plugin_isolation.rs` (10) and `plugin_isolation_boot.rs`.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0, 48 | Heartbeat only from the main loop: a modal dialog, a drag or a long main-thread task got a healthy helper killed | A pulse (message-only window, 100 ms timer, posted wake-ups) runs inside any message loop and bumps progress; a heartbeat thread sends *alive* while progress moves; calls are served from nested loops (all but unload/drop/restart/close/quit, kept in order) | `a_modal_loop_while_saving_is_not_taken_for_a_hang`, `calls_are_served_while_a_plugin_window_runs_a_modal_loop` ✗→✓ (new faults 10, 12) |
| 1, 11, 39, 46 | A hung plug-in in a render blocked the render thread for ever; Stop could not end it | Offline waits in 100 ms slices; a cancel flag (`PluginHost::cancel_render`, wired to export and freeze Stop) and an offline hang limit (30 s) end the helper | `an_offline_render_through_a_hung_plugin_ends_within_the_limit`, `a_cancelled_render_stops_waiting_for_a_hung_plugin`, `a_render_that_loses_a_plugin_says_so` (the old wait was INFINITE) |
| 2, 38 | `since_ms` outlived a late job when nothing called again (bypass, restart): a healthy helper killed | `Status` holds the late job's number; the hang is judged by the exchange; a restarted half shares it and never submits over a running job | `a_late_job_that_finished_is_not_a_hang_when_nothing_calls_again` ✗→✓ |
| 3, 10 | Notes and values of dry blocks lost: stuck notes, values out of step | `Carry` in `BridgeAudio` (made once): last value per parameter, note-offs/chokes/expressions/MIDI, a reset; merged at offset 0 into the next job; a full buffer resets | `a_late_block_keeps_the_notes_and_values_of_the_blocks_that_pass_without_it` ✗→✓ |
| 4 | NaN/±inf broke the JSON frame and the helper | `proto::Finite`: every frame's numbers made finite (NaN→0, ±inf→±MAX) in `write_frame` | `non_finite_numbers_cross_as_finite_ones` ✗→✓ |
| 5 | A block after its slot left waited a whole deadline | `Sandbox::unregister` closes the exchange before telling Unload | `a_block_after_its_instance_left_passes_at_once` ✗→✓ |
| 6, 47 | A render that lost a plug-in reported success | Render instances in helpers of their own (`render-<group>`); `PluginHost::render_failures` checked by export, bake, freeze and bounce | `a_render_that_loses_a_plugin_says_so` ✗→✓ |
| 7, 50 | An older background state overwrote a newer one | Background answers tagged with their request id; dropped when older than the last state loaded or saved | `a_background_state_older_than_a_loaded_one_is_dropped` ✗→✓ |
| 8, 50 | One failed background save stopped every refresh | Failures delivered; asked again 2 s later | `a_refused_background_save_is_asked_again` ✗→✓ (new fault 13) |
| 9, 40, 44 | A failed `load_state` kept the old last-good state, and save/reload wrote it over the project's | The state asked for becomes the last-good one before the call | `a_plugin_that_hangs_or_dies_loading_a_state_keeps_the_state_asked_for` ✗→✓ |
| 12 | Deadline rounded up to the 15.6 ms tick | `Caller` sleeps on its event and a high-resolution waitable timer | `a_late_wait_ends_within_two_milliseconds_of_its_deadline` ✗→✓ (median 15.9 ms before) |
| 13, 42 | Processing threads freed memory and unmapped slots | Replaced table snapshots are retired and freed by the main thread once no thread holds them; workers borrow, never clone | `a_retired_snapshot_is_freed_by_the_main_thread_once_no_thread_holds_it` |
| 14 | Start-up plug-ins loaded within Cantus before the default applied | `CantusApp::from_parts_with(.., isolation)`: the setting is in the host before the first sync | `a_start_up_projects_plugin_first_loads_where_this_computer_says` ✗→✓ |
| 15 | A plug-in that crashed loading was stuck as missing | `Missing` keeps where it was tried and whether it crashed; a changed choice retries; `reload` loads a missing slot; a crash on load is told and counted as a crash; the editor shows its *Runs* card | `a_plugin_that_crashes_while_loading_can_be_reloaded_or_moved` ✗→✓ |
| 16, 30 | Stale crash cards | `plugin_notice::prune` each frame; cleared when a project is replaced or opened | `the_crash_card_leaves_with_the_plugin_it_names` |
| 17, 33 | Mixer's slot menu without *Runs*/*Reload*; chain line and mini inspector unaware | `mixer_menu` `Act::Hosting` rows; chain line in the danger colour with the stopped tips; mini inspector's danger status row | shots |
| 18, 52 | A scan wiped crash records | The live `db.crashes` kept with the folders when a scan's result is taken | (one line beside the folders' own) |
| 19 | No main-thread hang test | Faults 7–9 (hang in save/load/editor); `Sandboxes::set_limits` | `…hangs_while_saving…`, `…hangs_or_dies_loading…`, `…hangs_opening_its_editor…`, `a_background_save_that_hangs…` ✗→✓ |
| 20 | Hang test did not catch re-waiting | Every block after the late one timed (median < 1 ms, none a deadline), at the bridge and through the engine | `a_plugin_that_hangs_never_holds…`, `a_hang_never_blocks…` |
| 21 | Dry assertions vacuous | −6 dB first, fresh noise per block; a latency variant checks the delayed dry path to the bit | `a_plugin_that_aborts_in_process…`, `a_crash_with_latency_plays_the_input_delayed_by_the_latency` |
| 22 | Crashing block skipped | The first dry block is the one that carried the fault; every later one equals the bypassed reference; the instrument silent from that block | `a_crash_mid_song…`, `an_instrument_that_crashes…` |
| 23 | Parallel = serial untested | Four isolated gains (Together, Individually) under `Workers::parallel(3)` against serial and in-process, and a crash under the pool | `the_worker_pool_plays_isolated_plugins_to_the_bit_of_the_serial_engine` |
| 24 | Instruments not bit-tested at the claimed sizes | Notes spread over blocks at 1/37/64/512 (CLAP and VST3); Within vs Together through the engine at 64/256/1000 | `an_isolated_instrument_plays_its_notes…`, `an_isolated_instrument_plays_the_project_to_the_bit…` |
| 25, 49 | A shared helper's death blamed every neighbour; *Run individually* never offered there | Blame: the crash handler's `CRASH_AT`, else `MAIN_AT` and the running exchanges, else the only instance; `Blame::{This, Neighbour, Unknown}` drives counting (database only for *This*); the card offers *Run {name} individually* for the culprit and reloads the others | `a_crash_in_a_shared_helper_is_counted_once…` (abort and access violation), `two_instances_of_one_plugin_count_one_crash` ✗→✓, `run_individually_names_the_plugin_that_keeps_crashing` |
| 26 | Missing-helper fallback untested | — (it behaved) | `without_the_helper_plugins_run_within_cantus_and_it_is_said_once` |
| 27 | `rt_alloc` gaps | The late block, the dry blocks that carry, the delivery, the crashing block and a live stop are counted | `the_audio_half_of_an_isolated_plugin_never_allocates` |
| 28 | Card answers and three-button layout unexercised | A hidden seam (`mark_crashed_for_tests`); answers tested; EN and ES at 1280×720 and 420×600: cards on screen, buttons inside, nothing overlapping | `the_cards_answers_do_what_they_say`, `the_crash_card_and_the_audio_notices_share_one_stack…` |
| 29 | `group_key` and the sidechain untested; makers differing in punctuation shared a helper | The by-maker key is the maker's name (case aside) | `each_choice_has_its_group` ✗→✓, `each_choice_makes_the_processes_it_says`, `an_isolated_gain_hears_its_sidechain_as_in_process` |
| 31 | A stopped plug-in's window: a mixed-language error, no reload | Window key disabled with the reason; `window_of_stopped` says why in the interface's words and brings the card back (editor, rack, F8) | `asking_for_a_stopped_plugins_window_brings_its_card_back` |
| 32 | Instrument inspector greyed, Reload under *Runs*, a stale CPU | Danger status row; Reload under its own *Plug-in* band (slot menus too); CPU hidden while stopped | shots |
| 34 | Rack and header said nothing | Rack button and header icon tips | shots |
| 35 | Late tip blamed the cure; a hang inflated the count | Reworded with full stops; one count per late job | `…late_block_keeps…` (`late_blocks() == 1`) |
| 36 | Long names cut the verb; the reason only on the picture | Name elided in its middle, the verb at full width; the reason in the body; the hint on the whole card | `a_crash_is_one_card…` |
| 37 | Linux signals dropped | The last parenthesised detail kept; SIGSEGV/BUS/ABRT/ILL/FPE named | `the_reasons_keep_their_detail_on_every_system` |
| 41 | Background requests not counted | Every request (calls, background saves, texts, unloads) is in flight with its limit | `a_background_save_that_hangs_is_ended…` |
| 43 | Linux shm leaked when Cantus died | Names unlinked once the helper has mapped them | `an_unlinked_region_still_shares_its_pages_and_cannot_be_opened_again` |
| 45 | `settle_new` swallowed the crash notice | `MainThread::stopped()`, asked every frame | `…hangs_or_dies_loading…` (told after a crash in `load_state`) |
| 51 | Saving froze on a troubled helper | The last-good state at once when a job is unfinished or the heartbeat is a second old | `a_background_save_that_hangs_is_ended_and_saving_meanwhile_does_not_wait` ✗→✓ |

Faults added to the test plug-ins: 7–9 hang in save, load, editor; 10 a 3 s modal loop in
save; 11 one 50 ms block; 12 a 3 s modal loop in the editor's window (CLAP); 13 a refused
save (CLAP); 14 an access violation in `process` (CLAP).

Runs after the fix round (Windows, the machine shared with two builds): `cantus-rt`, `cantus-host`, `cantus-sandbox` 170 passed (2 ignored: the full model, as before); `cantus-ui` 1 348 passed; `cargo clippy --workspace --all-targets -D warnings` and `cargo fmt --all --check` clean. Linux (WSL, `tools/wsl/check.sh r29/isolation clippy`) clean, and `cantus-rt`/`cantus-sandbox`/`cantus-host` 158 passed there (the Windows-only modal-loop and editor tests do not build on Linux). Shots: `F:\projects\daw-r29\shots\isolation\` (`isolation-instrument-*` new: a stopped hosted instrument with a long name — the card, the inspector, the header sign).

## The second fix round (review isolation-3: 50 findings, all closed)

Merged `r29/integration` first (8286139: tempo's `project_samples` and PROTOCOL 3, freeze's
renders, controllers' `SetModulatorAmount`). PROTOCOL is now 4 (`Started`, `Deferred`).
Seen failing first: the round's new tests ran against the merged code before any fix (6 of
8 in `bridge_faults3.rs` failed), and every other one with its fix taken out again
(*✗→✓*). Duplicates share a row.

| # | Finding | Fix | Test |
|---|---|---|---|
| 36, 44, 11, 14, 17, 4 (high) | Calls held for a plug-in's modal loop aged into a kill after 6× their limit; synchronous calls froze the interface meanwhile; every call after one held was held | Held only: quit, and the calls ending or replacing the instance whose window runs the loop (and its later calls), each said with `Message::Deferred`; a held request is never judged; a live helper is never killed; Cantus waits a call's own limit at most (`HostError::Busy`: a save answers with the last-good state, a restart is picked up later) | `a_long_modal_loop_kills_nothing_and_freezes_nothing` ✗→✓ (fault 15: a 10 s dialog) |
| 37, 3 (high) | Requests timed from posting: one queued behind a slow one was killed and blamed | `Message::Started`: only the request being served is judged, from when it started; every helper frame counts as life | `a_request_queued_behind_a_slow_one_is_not_late` ✗→✓ (fault 17: a 3 s save) |
| 0, 38, 45, 20 | A known culprit's neighbours, the main thread's instance and every running job were blamed too; two instances of one plug-in counted twice | `culprits()`: a known culprit alone; else the handler's word; else one candidate or nobody (`Unknown`); `This` counted once per helper death and plug-in | `an_end_is_laid_on_one_plugin_or_on_nobody` (unit; the old rules answer otherwise in four of its eight cases), `under_the_pool_a_crash_is_laid_on_the_plugin_that_did_it` |
| 19 | The crash handler was never held by a test | Two plug-ins processing at once, one crashes: only it is blamed | `the_crash_handler_names_the_plugin_whose_thread_crashed` ✗→✓ (handler removed) |
| 1, 18, 39 | A crash in an editor (window messages, outside a call) was laid on whoever was processing; a plug-in's own thread the same; Windows swallowed crashes in timer callbacks; a plug-in's crash reporter replaced the filter; no signal stack on Linux | Window messages run as calls into their instance (root window → instance); the handler writes *outside* for a thread of no instance; `UOI_TIMERPROC_EXCEPTION_SUPPRESSION` off; the filter put back after each load and handing on; the CRT abort signal handled; `SA_ONSTACK` with a stack per thread | `a_crash_in_an_editor_is_laid_on_its_plugin` ✗→✓ (fault 16) |
| 2, 24 | A slow neighbour made every other save answer with an old state | Only this instance's own trouble takes the shortcut (its job, its overdue request, a hung helper) | `a_slow_neighbour_does_not_make_a_save_stale` ✗→✓ |
| 5, 10, 13, 21, 48 | A bake beside an export (or freeze) cleared the other's render instances; one Stop killed both | One render at a time: a bake waits for an export, an export for a bake (freeze already waited for both) | `a_bake_and_an_export_never_render_at_once`; the consumers: `every_render_that_lost_a_plugin_says_so`, `a_bake_that_lost_a_plugin_bakes_nothing` |
| 6, 47 | A render instance that could not be loaded was skipped silently | `render_lost`: reported by `render_failures`, cleared by `finish_render` | `a_render_that_cannot_load_a_plugin_says_so` ✗→✓ |
| 7, 40 | A helper that never started, or a neighbour's crash during a load, counted as this plug-in's crash | A handshake failure is `HostError::Load` ("could not start"); a helper of another build answers the handshake; a load that died carries the sandbox's blame (`HostError::Ended`) | `a_helper_that_cannot_start_is_a_load_failure` ✗→✓ |
| 8, 9, 16, 23, 41 | Carried controllers crowded out the resuming block's note-off | Controllers and expressions folded to their last value; the block's own events first, carried note-offs next; a note-off with no room resets first; the rest counted lost; an overflow's reset drops what it supersedes | `a_controller_flood_while_late_never_drops_the_resuming_note_off` ✗→✓ |
| 12, 42, 46 | A late job's own events (gestures, values) were thrown away | `Slot::take_events` when it finishes; delivered with the next block | `a_late_jobs_events_reach_the_host` ✗→✓ |
| 15, 31 | A reload that crashed again was neither counted nor carded, and said in English | Counted by its blame; the card comes back; the reason in the interface's words | `a_reload_that_crashes_again_is_counted` ✗→✓ (the gain aborts on activate when told) |
| 22 | The latency crash test could not tell dry from processed | −6 dB first; the crash asserted | `a_crash_with_latency_plays_the_input_delayed_by_the_latency` |
| 25 | Weaker assertions than claimed | Every post-late block under the deadline; the note sounded first; the key heard by far more than a rounding; the worker test says what it does not hold | the four tests |
| 26, 32 | A plug-in name's parentheses read as a system code; English system words in Spanish; nested parentheses | Only a code closing the text counts (`0x…`, `signal N`, `exit code`, `exit status`), said through `isolation.why.signal/exit`; the four "stopped" messages use a dash | `a_reasons_detail_is_a_system_code_only` |
| 27 | A long name cut the Run button's verb | The name shortened in its middle (16 letters) | `a_long_name_never_cuts_the_run_buttons_words` |
| 28 | Two rows labelled *Plug-in* | Reload under *Recover* / *Recuperar* | shot `isolation-instrument-loaded-*` |
| 29 | No shot of the mixer, its menu, the selected insert, the rack, a loaded instrument that crashed, the three-button card | `review3_shots` in `shoot_hosted` (XL mixer's chain line, the desk's insert menu, the open insert tile, the instrument inspector and rack, the card at 420×600); the shots' apps start *Together* | the shots |
| 30 | The tip still said blocks | *Times it was late* / *Veces que llegó tarde*; `CpuFigure::late` documented as jobs | strings test |
| 33 | The chain hint stacked labels | One line per stopped insert (name: why) and one closing line | shot `isolation-mixer-*` |
| 34 | The instrument's window row stayed on when stopped | Off, with the reason as its hint | shot |
| 35 | Spanish bake words | *mientras se convertía el modulador … No se convirtió nada en automatización* | strings |
| (Linux run) | A neighbour's notice that came first used up the helper death's one count, so the plug-in that crashed was not counted (seen on Linux, where the notices came the other way round) | Only a counted blame uses up the death's count | `a_neighbours_notice_first_still_counts_the_plugin_that_did_it` ✗→✓, `two_instances_of_one_plugin_count_one_crash` on Linux |
| 43 | A frame cut short read as a broken protocol | `UnexpectedEof` after a header is the stream's end | `a_frame_cut_short_is_the_end_of_the_stream` ✗→✓ |
| 49 | A laptop asleep killed every helper | A gap longer than the silence limit since the last judgement restarts the clocks | `a_pause_of_cantus_is_not_the_helpers_silence` |

Faults added: 15 a 10 s modal loop in the editor's window, 16 an access violation in the
editor's timer, 17 a 3 s save without pumping; and the gain aborts on activate when
`CANTUS_TEST_GAIN_ABORT_ON_ACTIVATE` is set (a plug-in that crashes on every load).

Runs after the second fix round (Windows, the machine shared with two builds): `cantus-rt`, `cantus-host`, `cantus-sandbox` 187 passed (2 ignored, as before); `cantus-ui` 1 420 passed and one other area's test failed once under load (`font_fallback_live`: a system font not in time; it passes alone, twice). `cargo clippy --workspace --all-targets -D warnings` and `cargo fmt --all --check` clean. Linux (WSL): `tools/wsl/check.sh r29/isolation clippy` clean, and `cantus-rt`/`cantus-sandbox`/`cantus-host` 144 passed there. New shots in `F:\projects\daw-r29\shots\isolation\`: `isolation-{mixer,mixer-menu,insert-selected,instrument-loaded,rack,three-buttons}-night-{en,es}-1x.png`.

## The third fix round (review isolation-4: 67 findings, all closed)

Merged `r29/integration` first (e1c1d71: every fix round so far, freeze's second with
schema 38 `Track::lateness`). Freeze's `render_units_stored` / `render_instruments_stored`,
`refused_by_freeze`, `HostNotice::Frozen` and `project_for_save`'s frozen states work
unchanged with isolated plug-ins (a stored render takes the project's states, so it never
asks a busy plug-in; `freeze.rs` says a stale or never-loaded plug-in in its own words).
PROTOCOL is now 5 (`Message::Behind`, `Call::Surface`). **Seen failing first**: each new
test against the code with its fix put back as it was (a script swaps the old lines in,
runs the one test, swaps them out — *✗→✓*), or, for the carry, the tests written before
the change ran against the old carry (7 of its 10 failed). Tests: `bridge_faults4.rs`
(12), `bridge_blame.rs` (10), `session_faults4.rs` (8), unit tests in `bridge/audio.rs`
(13), `bridge/sandbox.rs` (6), `session_iso.rs` (3), `serve.rs` and `platform.rs` (the
helper's own), `rt_alloc.rs` (a carry and its delivery), and `cantus-ui`
`plugin_isolation.rs` (+9). Duplicates share a row.

| # | Finding | Fix | Test |
|---|---|---|---|
| 54, 43, 13 (high) | A dialog inside a served call (a licence nag in a load, a progress window in a state) served nothing: every other call waited its full limit, saves went stale, loads into the helper failed | The helper keeps each instance in a cell of its own and holds none across a call, so its pulse serves from **any** nested loop: other instances' calls are served there; a call on the busy instance waits behind it, told `Message::Behind`; a request served past its limit while alive makes the calls behind it give up at once (else after a 2 s grace); a card *X is waiting on a window of its own* with **Show it** (`AllowSetForegroundWindow` before every call that may open a window; `Call::Surface`) and **Stop it** | `a_dialog_inside_a_call_never_holds_the_neighbours` ✗→✓ (neighbours' states taken from inside the dialog, the save within one limit), `what_a_nested_loop_serves` ✗→✓ (`must_wait` unit), `a_plugin_waiting_on_its_window_has_a_card` |
| 10 (high) | An offline instance whose state load ran past 10 s rendered with the wrong state, silently | Offline `load_state` and `flush` wait while the helper lives (`Wait::WhileAlive`: until Stop, or the offline limit, which ends the helper); a refused state fails the instance (`RenderLoss::NeverLoaded`); a busy live source state is said (`RenderLoss::Stale`, `MainThread::fresh_state`) | `a_render_waits_for_a_state_that_takes_its_time` ✗→✓ (a state only the state carries: the gain's `x2` tag), `a_render_whose_state_never_comes_says_so` |
| 0, 11 | A late job's events were lost when the interface's frame saw the job finish first | `Status::unfinished_for` only reads; the audio half keeps its late job's number (`late_job`) and takes its events whatever the interface saw | `a_late_jobs_events_survive_the_interface_seeing_it_finish_first` ✗→✓ |
| 1 | A save queued behind a slow one gave up and wrote a stale state, ignoring the fresh one that came | A call's limit runs from its start; `save_state` takes an answered state after Busy; a dropped `SaveState` lands as the last-good state | `a_save_behind_a_slow_one_waits_from_its_own_start` ✗→✓ |
| 4, 23, 26, 51, 60, 13 | A pending restart re-waited its full limit every frame (50 ms while held, 5 s while slow) | Asked: waited 100 ms (`Wait::For`); pending: only looked at (`Wait::Poll`); every wait looks before it sleeps | `a_restart_is_polled_never_waited_for` ✗→✓ (behind its own dialog and behind a neighbour's slow save) |
| 20, 39 | A call given up on threw its answer away (a parameter list never refreshed, a window opened that Cantus thought closed); Busy said in English with a false time | Late answers kept: `Background::{State, Params}`; an editor asked for stays pending and is adopted when it opens (`Notice::EditorOpened`, `PluginWindow::floating`); Busy says whether its own window held it; the interface says *{name} is busy … its window opens when it is free* (`isolation.busy`) | `a_parameter_list_given_up_on_is_taken_when_it_comes` ✗→✓ (fault 22), `a_window_asked_for_during_a_dialog_opens_after_it` ✗→✓, `a_busy_window_is_said_in_its_words` ✗→✓ |
| 27 | Saving while its own background save hung froze up to 10 s; the test was loosened to 3.5 s | Its own started request with the helper silent a second is its own trouble: the last-good state at once | the 300 ms bound restored in `a_background_save_that_hangs_is_ended…`, `saving_while_its_own_save_hangs_answers_at_once_with_real_limits` ✗→✓ |
| 66 | A long non-pumping state load got the helper killed at project load and remembered as a crash | `Limits::heavy` (120 s) judges a silent load, state or unload; the caller gives up at its own limit, the helper is not killed | `a_slow_state_past_its_limit_is_not_a_hang` ✗→✓ |
| 50 | A helper slow to start was killed by the 5 s silence limit | Until its first frame a helper is judged by its handshake's own limit | `a_helper_slow_to_start_is_not_taken_for_hung` ✗→✓ (`CANTUS_SANDBOX_START_DELAY_MS`) |
| 57 | A load given up on as busy stayed missing for the session, with no Reload | `Missing::retry_at`: tried again after 2 s, doubling to a minute (`PluginHost::wants_sync`, honoured by the app); *Reload* for any plug-in that was found and not loaded (`load_error`), in the menus and the instrument inspector; a load the helper serves while it lives (its own dialog) is waited for up to `Limits::modal` | `a_load_given_up_on_while_busy_is_tried_again` ✗→✓ (`CANTUS_TEST_GAIN_PUMP_ACTIVATE_ONCE`), `a_plugin_that_never_loaded_can_be_reloaded_from_its_menu` ✗→✓ |
| 2, 45, 56 | A crash in a departing plug-in's destructor was laid on the plug-in that stayed | The handler's word stands for an instance leaving (its destructor runs as a call into it; its libraries forgotten after); `main_at` of a leaving instance too; its plug-in kept on record until its unload is answered and counted (`Sandboxes::take_departed_crashes`, `PluginHost::count_departed`) | `a_plugin_crashing_as_it_leaves_is_laid_its_own_end` ✗→✓ (unit), `a_crash_in_a_destructor_is_the_leaving_plugins` ✗→✓ (fault 19), `a_plugin_that_crashes_as_it_leaves_is_counted` ✗→✓ |
| 3, 30, 44, 55 | A hang outside any call was laid on the waiting caller or a bystander's running job | An end Cantus decides (`End::Decided`) is laid on what the main thread was doing (`hung_on`) or on nobody — never `p.instance` for a request not started, never the running jobs; the reader's protocol end too | `a_hang_with_nothing_named_is_nobodys` ✗→✓ (unit), `a_hang_in_a_framework_timer_is_its_plugins` ✗→✓ (fault 21, B's jobs running) |
| 6, 18, 29, 49 | *Outside* overrode the only-instance rule: an *Individually* plug-in's thread crash was nobody's | The only instance of a helper is laid any end | `the_only_plugin_is_laid_its_end_from_any_thread` ✗→✓ |
| 37, 48, 55 | Messages dispatched by a plug-in's own loop ran under the loop's owner; framework timers (JUCE's hidden window, VSTGUI's thread timers) were nobody's | A `WH_GETMESSAGE` hook attributes every retrieved message (root owner, else the library of its window class or timer procedure); the crash handler names the instance whose library the faulting code is in (libraries learnt at load: the plug-in's and those from its folder) | `a_neighbours_window_crashing_inside_a_dialog_is_the_neighbours` ✗→✓ (hook removed), `a_crash_in_a_framework_timer_is_its_plugins` ✗→✓ (fault 20; module lookup removed), `a_hang_in_a_frameworks_hidden_window_is_its_plugins` ✗→✓ (fault 24; the window class's library not looked at) |
| 7, 16, 19, 28, 47, 61 | The chained crash filter could recurse for ever, call into an unloaded library, keep a resumed record, lose to a decided hang; reinstalled only after loads | A re-entry guard; a filter called only while its library is loaded; a resumed (`CONTINUE_EXECUTION`) record taken back; the handler's word over a decided culprit; ours put back after every call | `a_crash_reporter_that_hands_back_changes_nothing` ✗→✓ (guard removed: a stack overflow), `an_exception_a_plugin_handled_is_no_crash_of_its` ✗→✓ (fault 23), `a_recorded_crash_stands_over_a_decided_hang` ✗→✓, `a_filter_in_no_library_is_never_called` |
| 17, 52, 19 | On Linux every load replaced a plug-in's own fault handlers | Ours goes back only where the default was restored; a plug-in's handler stays on top | (Linux: clippy and the Linux run; the rule is four lines) |
| 58 | Unknown deaths were counted for every plug-in and *Run individually* offered for an arbitrary one | Only `This` is counted and offered | `an_end_nothing_lays_on_one_plugin_is_counted_for_none` ✗→✓, `deaths_are_counted_once_per_helper_and_never_for_nobody` |
| 35, 53, 59 | Deaths de-duplicated by process id: a reused pid hid a crash | A serial per helper, never reused (`MainThread::helper_id`) | `helpers_are_numbered_for_good`, `deaths_are_counted_once_per_helper_and_never_for_nobody` |
| 64 | A failed reload kept the first death's reason and blame | The reload's error sets why it is off now, and its blame when it crashed | `a_reload_that_fails_says_why_now_and_whose` ✗→✓ |
| 46 | The watchdog killed a helper at once when its last user let go: the last destructor cut | Told to quit, a helper is given the call limit to end by itself; a crash then is still counted | `the_last_plugins_destructor_runs_to_its_end` ✗→✓ (`CANTUS_TEST_GAIN_DESTROY_MARKER`, a 300 ms destructor) |
| 62 | A stalled helper made one callback wait a deadline per instance | A job not even taken by its deadline marks the helper stalled (lobby words `JOBS_DONE`, `STALLED`); its other instances pass dry until a job finishes | `a_stalled_helper_costs_one_deadline_not_one_per_plugin` ✗→✓ (one processing thread, fault 18) |
| 12, 22, 36 | The pause guard missed the job clocks: after sleep a late plug-in was killed and remembered | `Sandbox::pauses`; each instance's late-job clock restarts (`Status::rebase`); an offline slice far longer than asked restarts its job's clock | `a_pause_while_a_job_is_late_ends_nothing` ✗→✓, `a_pause_is_not_a_silence_but_a_real_silence_still_is` ✗→✓, `a_long_slice_is_a_pause` |
| 8, 9, 14, 15, 33, 65 | The carry folded pedals and poly pressure, reordered events, dropped controllers after a reset, lost a late reset's merge, counted nothing | Arrival order; values folded by removal and append; poly pressure per key; switches keep a lift and a press; (N)RPN, data entry, mode messages unfolded; a reset supersedes only voices; reset-first decided before merging, a late one keeps the carry whole; dropped controllers counted | 10 unit tests in `bridge/audio.rs` (7 ✗ on the old carry), `a_late_reset_first_keeps_the_carry_and_the_blocks_events` ✗→✓, `rt_alloc.rs` (a carry and a reset-first delivery, counted) |
| 24, 34, 63 | A render instance that refused its state or failed to settle was dropped silently | Every offline `instantiate` error is a render that never had it | `a_render_whose_state_is_refused_says_so` ✗→✓, `a_render_whose_plugin_crashes_loading_says_it_never_had_it` |
| 41 | A never-loaded plug-in read as stopped "from that point on" | `RenderLoss::{Stopped, NeverLoaded, Stale}`; words per kind for export, bake and freeze (`isolation.{export,bake,freeze}.{never,stale}`) | `a_render_that_never_had_a_plugin_says_so_in_its_words` ✗→✓ |
| 5, 21 | A second bake while one traced threw the first away | One bake at a time (a pending trace or a bake waiting for samples) | `two_bakes_in_a_row_never_overlap` ✗→✓ |
| 25, 40 | Reload failures (and load failures) said in the host's English; the fallback key unused | `stopped_words` / `load_words`: the host's forms in the interface's words, else *it could not be loaded (again)*; new forms (busy, no helper, not restarted, not installed); used by the card, tips, status line, missing cards | `a_reload_that_fails_without_crashing_says_so_in_its_words` ✗→✓, `a_plugin_that_could_not_be_loaded_is_said_in_its_words` ✗→✓ |
| 38 | No interface test of a reload that crashes again | — | `a_reload_that_crashes_again_brings_its_card_back_in_its_words` ✗→✓ (card return removed) |
| 31 | The modal-loop test held neither the safety half nor nested unloads | Asserts the held load is Busy at once and runs after, in order; the neighbour removed during the dialog; `must_wait` unit-tested | `a_long_modal_loop_kills_nothing_and_freezes_nothing`, `what_a_nested_loop_serves` |
| 32 | Crash-handler tests passed on one processing thread; the pool test held nothing new | `Sandboxes::set_threads` (`--threads`); two threads in the handler test; a fail-fast with two jobs running | `the_crash_handler_names_the_plugin_whose_thread_crashed` ✗→✓ (handler's record removed), `a_fail_fast_with_two_jobs_running_is_nobodys` (coverage) |
| (Linux run) | A wait's slice could sleep past a limit shorter than itself (a 1 ms handshake limit waited 20 ms; a fast Linux helper answered within it) | A slice never sleeps past the wait's own end | `a_render_that_cannot_load_a_plugin_says_so` on Linux ✗→✓ |
| 42 | The shots did not show the fixes | `three_button_app` lists two plug-ins (the named Run button); EN shots taken again; the chain hint is `views::mixer::chain_hint`, tested; a waiting-card shot | `the_chain_line_says_which_insert_stopped_and_how_to_bring_it_back` ✗→✓, shots |

Faults added (CLAP gain and synth): 18 a one-second `process`; 19 an access violation in the
destructor; 20 and 21 an access violation and a hang in a thread timer (no window); 22 a
rescan whose second list read takes 2 s; 23 an exception its own filter resumes; 24 a hang in
a hidden window of its own (its class from its library, as a framework's). Window
faults are per instance now (the timer's id is its instance). From the environment:
`CANTUS_TEST_GAIN_DESTROY_MARKER`, `…_PUMP_LOAD_MS`, `…_PUMP_ACTIVATE_ONCE`,
`…_CHAIN_FILTER`; the gain doubles its output when its state's tag says `x2`; the helper
waits `CANTUS_SANDBOX_START_DELAY_MS` before it starts.

**For the integrator.** PROTOCOL 5. New `MainThread` methods with defaults (`helper_id`,
`waiting`, `surface`, `stop`, `fresh_state`); a new `Notice::EditorOpened` (an exhaustive
`match` on `Notice` needs its arm); `RenderFailure::kind` and `RenderLoss` (public);
`lose_in_render_for_tests` takes the kind; `Limits` has `heavy` and `modal` (struct
literals add them or use `..Limits::default()`); `PluginHost::{wants_sync, load_error,
instrument_load_error, waiting, surface, surface_instrument, stop, stop_instrument}` and
test seams `fail_next_reload_for_tests`, `mark_missing_for_tests`,
`set_waiting_for_tests`; the app syncs when `wants_sync` says. `windows-sys` features
added: `Win32_System_ProcessStatus`, `Win32_UI_Input_KeyboardAndMouse` (cantus-sandbox),
`Win32_Graphics_Gdi`, `Win32_System_Diagnostics_Debug`, `Win32_System_Kernel` (the CLAP
test plug-ins). No
schema change, no new command.

Runs after the third fix round (Windows, the machine shared with two builds):
`cantus-rt`, `cantus-host`, `cantus-sandbox` 238 passed (2 ignored, as before); `cantus-ui`
1 615 passed (and its isolation tests again after the last two small fixes); `cargo clippy
--workspace --all-targets -D warnings` and `cargo fmt --all --check` clean. Linux (WSL):
`tools/wsl/check.sh r29/isolation clippy` clean (with the plug-ins built without their
editor), and `cantus-rt`/`cantus-host`/`cantus-sandbox` 211 passed there. The sandbox
tests' copies of the CLAP test plug-ins (a static never dropped) are now named
`cantus-test-clap-*` and cleared after an hour, so the temporary folder no longer fills with
them. Shots in `F:\projects\daw-r29\shots\isolation\`, English taken again with Spanish:
`isolation-three-buttons-night-{en,es}-1x.png` (two plug-ins: *Run Spectras…Edition
individually*), `isolation-waiting-night-{en,es}-1x.png` (new: the waiting card, and a busy
window said on the status line), the mixer at XL with its chain line in both languages.

## The fourth fix round (review isolation-5: 74 findings — a simplification first)

The fourth review's count was not converging (53 → 50 → 67 → 74; highs 14 → 3 → 2 → 9):
each round had added a mechanism — calls served from loops nested in plug-in code, a
*heavy* and a *modal* limit, a message hook for blame, load retries — and each opened edge
cases of its own. So this round **rebuilt the call handling on four rules and removed
what they made unnecessary**, then closed what still applied. ADR 0063 §3 has the rules
and the list of what went, and why. Merged `r29/integration` (0c26d0e) first; PROTOCOL
is now 6.

**The four rules.**

1. *The interface thread never waits on a helper* beyond a frame's share (30 ms, only for
   a window opened or a restart). Every request is posted; its answer lands in the
   instance's inbox and is taken each frame. A load is **one request** (the plug-in, its
   state, its values; the helper settles a restart before the first job). Saves, reloads,
   moves, windows, parameter lists, a render's plug-ins: requests answered later, looked
   at frame by frame. A request served for more than 2 s, or a plug-in's own loop on the
   helper's main thread, shows that plug-in's waiting card.
2. *No stale state is ever reported as fresh.* A save asks every plug-in
   (`PluginHost::ask_states`), looks each frame (`states_in`), and is written when each
   answered or passed its own limit from its start; a request held behind a dialog waits
   with it. One kept as it was last said is named on the status line
   (`isolation.save.*`). New/Open/close take what each said last at once, and name it.
   Autosaves gather the same way; renders, moves too.
3. *Nothing that ends or replaces an instance is served from a nested loop* — the helper
   serves **nothing** nested; requests wait for its main loop, in order.
4. *No unbounded retries.* A load past its limit (helper alive, no dialog of its) is a
   failed load with Reload, never retried; its late answer is not taken. A load held by
   its own dialog waits for it, with its card.

**Removed.** The re-entrant serving (`Here`'s cells, the pulse's serving,
`Message::{Deferred, Behind}`), `Limits::{heavy, modal}`, `troubled`/`in_trouble`/
`fresh_state`/`held_up`, synchronous `Sandbox::call` on the interface thread, the load
retry (`Missing::retry_at`, `wants_sync`, `Failed::busy`), the audio half's offline load
wait, the `WH_GETMESSAGE` hook. In lines that are not comments: the helper's serving
(`serve.rs`) 859 → 685, the sandbox (`sandbox.rs`) 1 519 → 1 345; the main half
(`main.rs`) 590 → 817, as it now keeps every request's state itself; the session gained
the render preparation (`session_render.rs`, which took the two render functions out of
`session.rs` and `session_inst_idle.rs`). The mechanisms are fewer; the rules they keep
are written where they are kept.

**Found by the tests on the way** (each seen failing in the whole-engine suite before its
fix): a replaced audio half (the first one, made before the load was answered) sent its
*stop* to the helper as the engine let it go, so the plug-in played **dry** from then on;
the values a fresh insert records when it loads overwrote a value set meanwhile; a plug-in
counted as loaded before `idle` had said so. And: the stalled mark was 0 — *no mark* —
once every 2^32 jobs (`mark_of`).

**Seen failing first.** New behaviour is held by tests written for it; each was seen
failing by putting the old behaviour back in the code (`sabotage5.py` swaps the old lines
in, runs the test, restores the file byte for byte) — every one below failed so:
*overdue, nested, heldup ×2, blame, heard, sync, fill, stale, lost, stop, 46, 49, 45, 48,
words, quit, userstop, surface, whose, mark* (the names in the table's last column). The
carry's two tests failed against the old carry before its fix. The rewritten earlier
tests (async loads, `Rig::until_loaded`, `prepare_render`, `state_now`/`fresh_state`,
`open_window`) hold what they held, under the new rules.

| # | Finding | Fix | Test |
|---|---|---|---|
| 19, 43, 63, 12, 52 (high ×3) | A load whose plug-in shows a dialog blocked the interface up to 10 min; its card could never appear | Rule 1: a load is one request answered later; while its dialog is up (`Message::Nested`) the card shows, *Show it* / *Stop it*; nothing waits | `a_load_held_by_its_own_dialog_waits_for_it_and_loads` (*nested*), `nothing_the_interface_does_waits_on_a_helper` (*sync*) |
| 20, 62, 3, 11, 31, 51 (high ×2) | A load past 30 s without pumping was retried forever, freezing the interface 30 s each try | Rule 4 + rule 1: a failed load (`Notice::LoadFailed`), Reload, never retried; its late answer ignored; same limits live and offline | `a_load_past_its_limit_is_a_failed_load_never_retried` (*overdue*) |
| 0, 65 (high) | A healthy slow background save made a project save write the older state silently after 1 s | Rule 2: judged only by its own limit from its start; the save waits for the refresh under way; stale only past the limit, and then named | `a_slow_neighbour_does_not_make_a_save_stale`, `a_save_behind_a_slow_one_waits_from_its_own_start`, `a_save_names_a_plugin_kept_as_it_was_last_said` (*heldup*, *stale*) |
| 30, 61, 4, 13, 21, 53 (high ×2) | `Limits::heavy` (120 s) kept a hung helper alive and made every neighbour's call wait | Moot: `heavy` and `modal` removed — nothing waits, so no limit is stretched for a waiter | (removed; `a_dialog_inside_a_call_holds_the_helper_and_freezes_nothing`) |
| 50 (high), 1, 69, 15, 56 | A nested loop served Unload/Close/Restart under an instance's own stack; re-entered backend locks; an Unload answered before its destructor | Moot (rule 3): the helper serves nothing nested; an unload runs from the main loop and is answered after the destructor | `an_instance_removed_during_a_dialog_is_ended_after_it`, `a_long_modal_loop_holds_the_helper_kills_nothing_and_freezes_nothing` (*heldup*), `a_plugin_that_crashes_as_it_leaves_is_counted` |
| 2, 23, 33, 64 | The message hook's mark outlived the message: a plug-in's own crash laid on a neighbour | Moot: the hook removed; blame is the thread's own call, and inside a plug-in's loop the faulting library's one instance (else nobody) | `a_neighbours_window_crashing_inside_a_dialog_is_the_neighbours` (*blame*), `a_fault_is_the_calling_plugins_and_in_a_loop_its_librarys` (*whose*) |
| 7, 32, 55 | A library's first owner overrode the faulting thread's own instance | The thread's own instance first; a library several instances share names none (`whose`, `by_module`) | `a_fault_is_the_calling_plugins_and_in_a_loop_its_librarys` (*whose*), `a_library_two_instances_share_names_neither` |
| 5, 17, 26, 35, 47, 57, 68 | *Show it* raised the bottom-most window, not the dialog; the path had no test | `waiting_window`: the topmost window whose owner is disabled (the modal dialog), else the topmost visible enabled one; served by the helper's reader thread at once | `show_it_picks_the_dialog_its_owner_waits_on` (*surface*) |
| 6, 14, 22, 34, 60, 66 | Offline state loads were capped at 30 s, live at 120 s: renders lost slow samplers | Moot: the same limits live and offline; a render's instance loads with its state in one request, judged by the load limit from its start | `a_render_waits_for_a_state_that_takes_its_time`, `a_render_whose_state_never_comes_says_so` |
| 59, 70 | A render's setup asked a busy plug-in twice, waiting each time on the interface | Moot: `render_ready` asks once and never waits | `nothing_the_interface_does_waits_on_a_helper` (its render) |
| 24, 54 | Quitting Cantus killed every helper before its plug-ins' destructors ran | `Sandboxes::drop` waits up to `QUIT_GRACE` (3 s) for the helpers to end | `quitting_lets_the_helpers_end_their_plugins` (*quit*) |
| 28, 44, 67 | *Stop it* counted and remembered as the plug-in's crash; neighbours told they waited; the card hid that neighbours stop too | A user's stop is never counted; the plug-in: *you stopped it*; the others: *its process was ended to stop X*; the card says how many stop with it (`isolation.waiting.body_shared`); the manual in both languages | `stopping_a_plugin_is_never_its_crash` (*userstop*) |
| 8, 71 | A lost-race stalled mark, and a mark never cleared, could leave a helper dry for good | The mark is a count read before the look, cleared by the next finished job; never 0 at the count's wrap | `a_stalled_mark_never_outlives_a_finished_job` (*mark*), `a_stalled_helper_costs_one_deadline_not_one_per_plugin` |
| 9, 18, 25, 42, 58, 72 | The Unix module walk did not build on macOS | Linux-only (`dl_iterate_phdr`), a no-op elsewhere | WSL clippy/test; the macOS build is CI's |
| 73 | The abort signal was put back after every call, replacing a plug-in's reporter | Ours only from the start; the crash filter alone is put back on top | (reviewed; `a_crash_reporter_that_hands_back_changes_nothing`) |
| 10 | Folding the carry moved a program change ahead of its bank select | A bank select a carried program was sent under stays with it | `a_program_keeps_the_bank_it_was_sent_under` (✗→✓) |
| 16 | A repeated pedal position moved the press after carried note-offs | The newer value takes the kept event's place | `a_repeated_pedal_position_keeps_its_place` (✗→✓) |
| 27 | Export and bake dropped a heard plug-in not loaded live, silently | A heard plug-in that could not be loaded is the render's loss (`RenderLoss::NeverLoaded`); a frozen chain's rest is not | `a_render_says_a_plugin_that_was_not_loaded` (*lost*) |
| 29 | `words_or` logged every frame from drawing | No log there; the host logs its text where it records it | (no side effect left to test) |
| 45 | Installed plug-ins that could not be loaded were told to be installed | *Not loaded* in the slot, the editor's card (with Reload), the inspector (Reload and *Runs*), the status line and the window's refusal | `a_plugin_that_could_not_be_loaded_is_never_told_to_be_installed` (*45*) |
| 46 | A bake's or bounce's stale warning was written over by its own word | Said after it: `{done} {warning}` | `a_render_from_a_state_as_last_said_says_so_after_its_own_word` (*46*) |
| 48 | The waiting card's title left 8 letters for the name, with no hint | *X is waiting for you*; the hint names it whole with the card's words | `the_waiting_cards_hint_names_its_plugin_whole` (*48*), shots |
| 49 | Window failures other than Busy were in the host's English | A stop as a stop, anything else in the interface's words (`isolation.why.no_window`) | `a_window_that_could_not_open_is_said_in_its_words` (*49*) |
| 36 | The reload test's *whose* half proved nothing | The neighbour reloaded crashes by itself and becomes the culprit, each counted once | `a_reload_that_fails_says_why_now_and_whose` (rewritten) |
| 37 | The save-behind test passed with a limit from posting | Under rule 2 the second save starts at ~3 s with a 4 s limit: from posting it would be stale, and the test asks for it fresh | `a_save_behind_a_slow_one_waits_from_its_own_start` |
| 38 | No test that a given-up state's late answer becomes the last-good one | — | `a_state_answered_late_becomes_the_last_good_one` |
| 39 | `rt_alloc` never reached the reset-first delivery | Its resuming block has 400 notes of its own: the 200 carried note-offs find no room | `the_audio_half_of_an_isolated_plugin_never_allocates` |
| 40 | The long-dialog test never checked where B's unload ran | Moot (rule 3): now checked that it runs after the dialog | `an_instance_removed_during_a_dialog_is_ended_after_it` |
| 41 | A render test checked notices nothing collected | Frames run; no crash told, none counted, the live plug-in untouched | `a_render_whose_plugin_crashes_loading_says_it_never_had_it` |

`words` — a save that kept a plug-in as it was last said says so in its words
(`a_save_names_the_plugins_it_kept_as_they_were_last_said`). The interface: saves and
autosaves gather (`save_job.rs`, `autosave.rs`), renders wait for their plug-ins as for
samples (`export.rs`, `modulator_bake.rs`, `freeze.rs` — one line each where the samples'
wait already was), a fresh insert's values join its undo step (`load_key`,
`inst_load_key`: `plugins.rs`, `swap.rs`, `hosted_inst.rs`).

**Files outside my area this round** (each a few lines where an existing wait or step
already was): `cantus-ui/src/views/export.rs` (the waiting state also waits for the render's
plug-ins), `views/modulator_bake.rs` (the same; its warning after *baked*), `freeze.rs`
(`units_for` asks `render_ready`; `start_next` keeps the job first in the queue while they
load; the warning after the commit), `save_job.rs` and `autosave.rs` (the gather),
`app.rs` (`autosave_gathering`, the `wants_sync` block removed, the not-loaded status),
`views/plugins.rs`, `swap.rs`, `hosted_inst.rs` (`exec_merging` with the load keys; the
not-loaded words), `views/inspector_hosted.rs` (the not-loaded branch), `views/audio_notice.rs`
(the waiting card's hint), `tests/controllers_review3.rs` (the fault parameter's steps run
to 24 since round four's faults), `cantus-bench/src/isolation.rs` (waits for loads), the
manual in both languages.

**Notes for the integrator.** PROTOCOL 6 (a helper of another build refuses the handshake
as before). `MainThread` gained `ask_state_now`, `poll_state`, `state_waiting`,
`state_current`, `loaded`, `take_audio` (all with defaults: an in-process backend is
unchanged); `Notice` gained `Loaded` and `LoadFailed`; `Backend::load_with` (default
`None`). A render's units now come after `PluginHost::render_ready` says yes: any new
caller of `render_units`/`render_instruments` must ask it first, frame by frame (as the
export, the bake and the freeze do), or isolated plug-ins are reported as never loaded.

**Measured again** (`cantus-bench --full --only isolation`, release; `docs/perf/isolation.md`
run 3), since the audio half now reads its *loaded* flags: hot round trip *Together*
0.6 / 0.9 µs (median / p99) at 64 frames, paced 14.0 / 43.2 µs; the helpers' CPU 0.51 %
of a core per instance *Together*, 5.7 % *Individually*; 32 Comprimo CLAP instances
*Together* 1.16× within on this run. Bit-identity: every to-the-bit test holds (isolated
against in process at 1/37/64/512 frames, through the engine at 64/256/1 000, the worker
pool, the instruments, and offline renders).

**The runs** (bad27bc). Windows: `cargo test -p cantus-rt -p cantus-host -p cantus-sandbox`
251 passed, `cargo test -p cantus-ui` 1 614 passed, `cargo clippy --workspace --all-targets
-- -D warnings` and `cargo fmt --all -- --check` clean. WSL (Ubuntu 24.04):
`tools/wsl/check.sh r29/isolation clippy` clean, the isolation crates' tests 221 passed
(the one test of a plug-in's own dialog is Windows'). Shots, both languages:
`isolation-waiting`, `isolation-waiting-long` (the 43-letter name, its hint whole),
`isolation-not-loaded` (the editor's card with Reload), with the earlier ones.
