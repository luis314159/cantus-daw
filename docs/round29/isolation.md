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
- A *Plug-ins late* count of a few blocks after a crash is expected: the blocks between the
  crash and the helper's end are counted.
