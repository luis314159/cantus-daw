# Round 31 — workflow (item 0, then item 4)

Seam fix: d040035c (track templates carry the note-effect chain with renewed ids; `AutoTarget::NoteFx` mapped; `r31/integration` merged first, the workspace compiles)

## Item 0: three flaky sandbox tests — the causes, fixed (commits `76878b0c` tests, `87806277` fixes)

Each test had a cause; none was a wait. ADR 0063 §8 holds the decisions.

| Test (where it failed) | Cause | Fix | Before → after (each alone, 12-process CPU hog) |
|---|---|---|---|
| `session_faults5::an_instance_removed_during_a_dialog_is_ended_after_it` (Windows runners, beta.5 release: "2 destroyed") | The test assumed an order the code does not promise: the test plug-ins are described by a scan **in the test process** (`support::infos`, once per binary), which makes and ends an instance of each CLAP. When the dialog test was the first of its binary to take the serial lock — always when run alone — the scan ran after it had set `CANTUS_TEST_GAIN_DESTROY_MARKER`, and the scan's two instances were counted as destroyed under the dialog. | `support::serial` describes the plug-ins before any test runs; `support::infos` refuses to scan with a test fault armed in the environment. | 20/20 failed → 0/50 |
| `session_faults5::a_state_answered_late_becomes_the_last_good_one` (macOS VM) | A real race in the host: a value the helper said moved made the last-good state "not current" **as of the frame that read it**. The fault's value, set just before the save and echoed with the answer to the call that set it, reached the frame after the save had asked for the state — though the helper saw it before it saved — so the late answer was taken as older than the change, stale until the next refresh (a user moving a knob and saving at once met it). Reproduced deterministically as `session_faults8` (set and ask in one frame). | The helper's main thread serves one request at a time and says news only between requests: Cantus stamps each move with the first request answered after it (`Sandbox::served`; a *surface*, answered out of turn, aside); the state answered to that request or later holds it (`BridgeMain::good_at`). A save says what moved before it first (`Server::news_of`), so only a value that moves during the save leaves its state not current (`session_faults8`'s second test). | `session_faults8` 20/20 failed → 0/50; the late-state test itself 0/25 with late timers (it now looks frame by frame for the answer instead of waiting 6 s) |
| `bridge_faults4::a_pause_while_a_job_is_late_ends_nothing` (macOS VM) | Life was inferred from a heartbeat every 200 ms — a timer, through two pipe threads — judged against the test's 300 ms silence limit: a VM's late timers (or App Nap) brought one late and a live helper was ended as "stopped answering". Reproduced on Windows with `CANTUS_TEST_SANDBOX_LATE_TIMERS_MS=150` (a test knob: the helper's waits that much longer). | The main thread's progress count is also a lobby word (`PROGRESS_AT`) that every judgement reads (`Sandbox::silent_for`): no timer between the thread and the judge. A main thread that truly stops is found hung as before (`bridge_faults5`). macOS: the helper holds a latency-critical activity for its life (`NSProcessInfo`, no App Nap). | with late timers 20/20 failed → 0/50; `bridge_faults5` 20/20 → 0/50 |

The macOS cause is reasoned and reproduced under the same timing on Windows; the
integrator watches the next `macos.yml` run (the `FLAKY on macOS VM` line should be gone
for these two). `cargo check`/`clippy -D warnings` of `cantus-sandbox` for
`aarch64-apple-darwin` pass here.

Suites after item 0 (Windows, `RUSTFLAGS=-D warnings`): `cantus-sandbox` and `cantus-host`
all passed; clippy clean on both, and on `cantus-sandbox` for macOS.

## Item 4: the command palette, the undo history, project and track templates (ADR 0072)

**What was built** (ADR 0072 holds the decisions; the manual's chapters 3 and 11 the use):

- **The command palette** — `Ctrl+Shift+P` on every preset (⌘⇧P on a Mac), *View ▸
  Command palette…*. Every `Action` with its shortcut, the areas, the windows, every
  Settings page, every effect (on the selected strip) and instrument (a new track after
  the selected one) of the device catalogue, the recent projects, the project templates
  and the track templates — fuzzy, in the interface's language and in English, ranked by
  use (`palette.toml`). ↑ ↓ / PageUp PageDown, Enter, Esc, click. `palette_index.rs`
  (pure), `views/palette*.rs` (kit).
- **The undo history** — `Pane::History` (`Alt+H`, *View ▸ Show the undo history*), a tab
  beside the inspector that docks, stacks and floats. Every step by name (*Move 3 notes*,
  *Insert Comprimo on Lead*, …), the current one lit, the steps to redo dimmed, the saved
  one tagged, the unsaved ones dotted; a click undoes or redoes each step between, one
  at a time. The bus numbers its states (`HistoryEntry::step`, `CommandBus::saved_step`)
  so the saved mark survives undo, redo and saves landing off the thread.
- **Project templates and the start page** — *File ▸ New from template…*, *Save as
  template…*; four starters built from our own instruments and effects (*Empty song*,
  *Beat*, *Band recording*, *Mastering*); the user's in `<data>/templates/projects`,
  listed and deleted on the start page, beside the recent projects (`recent.toml`, new).
- **Track templates** — *File ▸ Save track as template…*; inserted from the device
  picker's *Track templates* shelf, the devices panel and the palette as one undo step,
  every id new, sends/outputs/keys found by name; listed and deleted on the start page.

**Shared files** (additive lines only): `keymap.rs` (five actions, their ids, categories,
two default bindings, version 11 with `NEW_IN_V11`, a test), `app.rs` (the `workflow`
field and its default, `workflow::wire` in `assemble`, `workflow::frame` in the frame, the
`run_action` hook and its arm, the recent note and forget in `open`), `workspace.rs`
(`Pane::History`: variant, title, toggle, glyph, place), `workspace_frame.rs` (one arm),
`transport_menus.rs` (File: three items; View: the History check and the palette;
`pane_of`), `macos/menu.rs` (five entries), `unsaved.rs` (`Then::Template`), `save_job.rs`
(the recent note), `picker.rs` / `picker_catalog.rs` / `picker_query.rs` / `swap.rs` /
`views/picker.rs` / `views/picker_details.rs` (`PickChoice::TrackTemplate`,
`Source::Template`, its shelf), `lib.rs` and `views/mod.rs` (module lines),
`cantus-core/src/bus.rs` (the step numbers).

**Numbers taken:** keymap **11**. No schema bump (templates are projects; step numbers
live in memory) — schema 45 left unused. ADR **0072**.

**Shots** (`cargo run -p cantus-ui --example shoot_workflow -- <dir> [es] [light]`), kept
here: `workflow-palette.png`, `workflow-palette-filter-es-paper.png`,
`workflow-history.png`, `workflow-history-es-paper.png`, `workflow-start.png`,
`workflow-start-es-paper.png`, `workflow-picker-templates.png`,
`workflow-name-es-paper.png`.

**Tests added:** `cantus-core` `bus.rs` (2: the saved state through undo, redo, late saves,
merges and replacements; the base following the history's end), `tests/workflow.rs` (9),
`tests/workflow_boot.rs` (1: the boot's `assemble` wires it all), `keymap.rs` (1: a
version-10 file, every preset, ⌘⇧P free on a Mac), `history_text.rs` (1), `recent.rs` (1).

**Suites:**
- Windows (`RUSTFLAGS=-D warnings`): clippy of the whole workspace clean; `cantus-core` and
  `cantus-ui` 2 180 passed — but for 17 tests that host the test CLAP in the UI tests
  (`controllers_review*`, `hosted_instruments`, `hosted_plugins`, `frame_budget`,
  `modulators`), which fail the same way on the round's contract commit in this worktree
  (the plug-in does not load here — F: was full at the time: 1.4 GB free) and pass on
  Linux; and the font test, fixed (a `▸` no font has).
- Linux (WSL, `tools/wsl/check.sh`, own clone): clippy clean (and the plug-ins without
  their editor); the workspace 4 472 passed, 2 failed under load — `cantus-dsp`'s
  `a_sketch_of_a_minute_is_quick` and `cantus-engine`'s `a_snapshot_waits_beside_for_its_starts`,
  timing tests of crates this branch does not touch — both pass run alone.

**Open, the owner's only:** whether the start page should also open when Cantus starts
(it does not: the first run is unchanged); the macOS runner's next run for item 0.

## After the review and the merge of `r31/integration`

- **The midi seam:** track templates carry the note-effect chain with renewed ids
  (`notefx::renew_ids`), and `AutoTarget::NoteFx` is mapped to the new track and effect
  (dropped if it rode something the template does not bring). Clips are not kept, so a
  clip's groove and `skip_note_fx` do not arise; a project template is a whole project
  (its key and groove pool come along). Test: an arpeggiator with an LFO on one of its
  parameters, inserted twice, gives two chains with ids of their own.
- **Hermetic hosted tests:** `CantusApp::from_parts_at` read the data folder's
  `plugins.json`, so every app built from parts knew the plug-ins this computer had
  scanned (Odin2, Surge XT, ValhallaVintageVerb: "5, not 1"). It now starts with no
  database; the boot loads it and `assemble` puts it in place as before. The 15 tests pass
  here with the real plug-ins installed.
- **Review 2–11:** templates ask hosted plug-ins for their state and wait as a save does
  (`template_name::begin`/`poll`, `project_for_save`, stale ones named); a track template
  routes by a strip's name (its own or its track's), never its number, and never to the
  blank strip it replaces; it drops on a track's header (after that track), and from the
  picker or the devices panel it lands after the selected track; the start page and the
  name panel read the disk when they open or a template changes, not each frame; no step
  is *Saved* for a project never saved, and the count says "1 step"; a save names the
  history state it took (`CommandBus::mark_saved_state`), so a long one keeps its mark;
  `recent.toml` and `palette.toml` are written through a temporary file; the late-timers
  knob is `CANTUS_TEST_SANDBOX_LATE_TIMERS_MS`, capped at 1 s; a track template saved over
  under its name is offered as it is now; the palette's own key closes it from its field.
  Each has a test (the atomic writes' only shows that nothing is left behind).

## The bridge tests under load (2026-10-27)

`bridge::a_plugin_that_aborts_in_process_leaves_the_block_dry_and_says_so_once` ("processed
at −6 dB") and `bridge::a_plugin_that_hangs_never_holds_the_audio_thread_past_the_deadline`
("a block waited out a deadline again") had one cause, in the helper: `Server::grow` said it
returned "once each [processing thread] is waiting, so a first block finds it ready", but
only spawned the thread. Under load the new thread — ordinary priority until it promotes
itself — ran tens of milliseconds later, so the first job after a load was late: its block
passed dry (the first test) or the hang asked for two blocks later landed in the block
after that late job finished, which waited its deadline (the second; round 30's
`late_blocks == 1` flake). A user inserting an isolated plug-in on a busy machine heard its
first block dry. Now `worker::serve` says when it is promoted and serving, and `grow` waits
for that (at most 1 s) before the load is answered. Under a 16-process CPU hog, each test
alone: before 30/30 and 27/30 failed; after 0/50 and 0/50. The tests are unchanged.

## The audio cores as engines (2026-10-27)

*Audio cores* shows each count with its engine tier and a model known for it, by the
model's name only (`engines.rs`, one table, `TABLE`): 1 single · Grom, 2 twin · Scrambler,
3 inline-3 · GR Yaris, 4 V4 · Fulvia, 6 V6 · GT-R, 8 V8 · Mustang, 10 V10 · Viper,
12 V12 · Enzo, 16 W16 · Chiron, 17 and up turbo-ramjet · SR-71 Blackbird; a count between
two tiers takes the one below. Each model is an illustration of our own (an SVG in the
theme's colours, rasterised at its exact size; redrawn after the owner found the first
silhouettes ugly — see `vehicles.md`). At the right of each row (as tall as
the row allows; a row whose words would reach it shows none) and large under the field,
on a well, with the model and the engine beside it. `shoot_ship -- <dir> engines` draws all
ten; the `-200` scenes shoot at 200 %.

Shots: `workflow-engines-night-en.png`, `workflow-engines-paper-es.png` (all ten);
`workflow-cores-night-en.png`, `workflow-cores-paper-es.png` (the list, 100 %),
`workflow-cores-200-night-en.png`, `workflow-cores-200-paper-es.png` (200 %);
`workflow-cores-picture-night-es.png` (100 %), `workflow-cores-picture-200-paper-en.png`
(200 %). Nothing overlaps at either size.

**After the merge (f5f774e7).** Three things on the page fell short, fixed:
- The field counted helpers as cores while the "now" line said "4 cores helping" under
  "Automatic (6) · V6": two engines on one screen. Both now count cores at work — the
  audio thread and its helpers ("Automatic (7) · V6 · GT-R", "7 cores at work: the audio
  thread and 6 more · V6 · GT-R"; "1 (the audio thread alone)"), the status tooltip too
  (`perf_status::pool_words`). Test: `the_cores_field_and_the_now_line_count_alike`.
- The real-time priority line read "audio thread audio thread not started yet" (in
  Spanish "hilo de audio el hilo de audio…") and gave the system's English (MMCSS, time
  constraint, SCHED_FIFO, the driver's, refused) in both languages; now "Real-time
  priority — audio thread: {what} · helpers: {n of m real-time | none}" with every form in
  each language's words (`audio_setup::realtime_words`, `promotion_words`). It ran past the
  page's right edge: every indented note stacked its wrapped lines in the row's horizontal
  layout, so the second line went to the right (the record offset's hint too); they now
  stack under each other. Test: `the_real_time_priority_line_reads_in_every_form`.
- The record offset knob's caption was "CORR…" in Spanish; it is *Ajuste* (English
  *Offset*). Test: `the_audio_page_reads_whole_in_both_languages` renders the page in both
  languages at 1500×940 and 1100×760 and fails on any text past its edge or cut to "…" — it
  fails on the old wrapping and the old caption.

The status line's "(no device)" of an engine with no device open is in each language
too (`devices.no_device`). Shots of the page, shot again (both themes, both languages,
100 % and 200 % checked; four kept): `workflow-cores-picture-night-es.png`,
`workflow-cores-picture-paper-en.png` (100 %), `workflow-cores-picture-200-night-es.png`,
`workflow-cores-picture-200-paper-en.png` (200 %).

## Templates: no automation mode, starters' effects by hand (review 5–6)

A template carried its tracks' automation mode: a track saved in Write, inserted with the
global arm on, overwrote its curves on the first Play. Now a track template's track and a
project template's tracks and master are saved reading (`templates::read_only`). The
starters' reverb returns were at Space's 25 % wet default though every return Cantus makes
is full wet; they are now full wet with a preset chosen for each (*Dark tail* for Beat's
keys, *Vocal plate* for Band), and Mastering's chain carries its presets (*Mastering tilt*,
*Mastering glue*, *Master: tighten the low end*, *Transparent master*; Speculum as it
comes) — `BEAT_REVERB`, `BAND_REVERB`, `MASTERING_CHAIN`, ADR 0072 §3. Tests:
`a_template_never_carries_an_automation_mode`,
`the_starters_returns_are_full_wet_and_the_mastering_chain_has_its_presets`.
