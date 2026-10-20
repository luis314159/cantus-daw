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
