# Round 29 — freeze: freeze, bounce in place, low-latency monitoring

Branch `r29/freeze` (worktree `F:\projects\daw-r29\freeze`), from `r29/integration` at
0933835 (the contract, tempo's whole work, isolation's). ADR 0064. Schema 37
(`migrate_36_to_37`). Undo-fuzz arm 47. Item 5 of the round (BACKLOG §4c steps 1–2).

## Done

1. **The model** (`cantus-core/src/freeze.rs`, `freeze_guard.rs`): `MixerStrip::frozen`,
   `Track::frozen` (`Frozen { content }`), `Command::{SetStripFrozen, SetTrackFrozen}` with
   exact inverses (`cmd.freeze` / `cmd.unfreeze`); the **region** (gathers, keys, note
   sources, modulator sources, to a fixed point), the **subject** and its **content key**
   (SHA-256 of canonical, key-sorted JSON); **the edit lock** in `Command::apply` — every
   command classified in a `match` with no wildcard (a new command does not build until it
   is), touches checked against the frozen regions' locks before applying, project-wide
   ones applied and checked; freezing disarms in the same step; a live *On* input refuses;
   `dormant` (what rests, what live consumers wake), `settle` (a playing frozen strip loses
   its chain in the snapshot; solo elsewhere mutes it), `isolate`, `region_through`,
   `thaw` (pattern mode plays live), `repair` (a hand-edited key re-keyed on load).
2. **Schema 37** (`cantus-project`): migration, the loader's repair, the fixture block,
   `fixtures/v37.cantus`, `schema_37_keeps_frozen_strips_and_tracks`.
3. **The engine** (`cantus-engine`): `freeze.rs` — render keys, the store (the project's
   `Freeze` folder and Cantus's cache, looked up in both, opened as ADR 0050 streams),
   the audio-thread reader (head + pool copy, a board cursor sixteen blocks ahead and the
   loop's start, offline waits, real-time lateness counted); `freeze_render.rs` — the
   tapped render (a strip before its width, a track before its level), everything outside
   the region resting, the tail rule, raw file → render file (`stream_cache::write_at`) or
   WAV, `prepare`/`FreezeJob::run`; the plan (`rest` nodes, frozen strips gather nothing,
   `topology_key`), `run_node` skips resting nodes, `run_strip`/`run_track` read renders,
   `compensate` (resting nodes delay nothing, frozen ones report the render's latency),
   `StemTap::with_pre`, `Processor::{set_offline, track_latency}`, `EngineHandle::frozen`
   attached at every compile.
4. **Low-latency monitoring** (`processor_monitor.rs`): `EngineMessage::SetLowLatency`, the
   monitored path, hard bypass (`ll_off`, strips and the master), constrained compensation
   (strips carrying monitored input only), `EngineStats::low_latency` (names, the
   monitored wait); the recorder keeps **the latency at the pass's start**
   (`EngineHandle::pass_latency`, used by `record_session::finish_pass`).
5. **Hosting** (`cantus-host`): `PluginHost::sync` and `sync_instruments` skip the dormant
   plug-ins (unloaded, state parked); the snapshot drops them too.
6. **The interface** (`cantus-ui`): `freeze.rs` (render-then-freeze jobs, one at a time,
   Stop, re-render on edit, background refresh of missing renders, hosted states written in
   the freeze's undo step, the folders following the project, low-latency to the engine,
   the refusal), `bounce.rs` (source / with inserts / a whole strip; whole track, selected
   clips, time selection — clips cut at its edges; new track under the original, muted or
   removed as remembered; one undo step), `views/freeze_view.rs` (snowflakes on headers
   and strip heads, frosted clips, the corner card with Stop / Unfreeze, the inspector's
   *Freeze* row, the track menu, keys), `views/freeze_monitor.rs` (switch, limit, the
   bypassed list, the chip's monitored round trip), `Glyph::Frozen`; keymap actions
   *Freeze track* (`Alt+Shift+F`), *Bounce in place* (`Alt+Shift+B`), *Bounce source*,
   *Low-latency monitoring* (`Alt+Shift+R`) in `NEW_IN_V9`; settings `low_latency`,
   `low_latency_ms` (3 ms), `bounce_removes`; wired in `assemble` and on an engine change;
   the export waits for a running freeze; every string in English and Spanish.
7. **Docs**: ADR 0064; the manual's *Monitoring with low latency* (§4) and *Freeze and
   bounce in place* (§6) in both languages; `shoot_freeze`.

## Measurements

See ADR 0064 §Measurements: a heavy strip frozen costs **8 %** of what it cost live
(2.477 s → 0.203 s for 300 blocks of 512, best of five); a voice through Tonus heard
**128 samples** late instead of 4 608 with low-latency monitoring; takes placed to the
frame with the mode on, off, and switched mid-pass (330 samples off without the capture).
No golden moved; `DSP_VERSION` unchanged.

## Tests and what each holds

See ADR 0064 §Tests for what each holds. The final run, after merging `r29/integration`
at bef6f62 (warp and controllers), all `--no-fail-fast`, 0 failed:

| crate | passed |
|---|---|
| `cantus-core` | 339 |
| `cantus-project` | 74 |
| `cantus-engine` | 745 |
| `cantus-engine --features fuzz` (`CANTUS_WORKERS=3`) | 749 |
| `cantus-host` | 73 |
| `cantus-sandbox` (`CANTUS_TEST_PLUGINS_PREBUILT=1`) | 19 |
| `cantus-ui` | 1 342 |

`cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all -- --check`
clean. `shoot_freeze` renders 98 PNGs to `F:\projects\daw-r29\shotsreeze\` (arrangement,
mixer, menu, inspector of a synth and of a voice, the record popover, the cards, a render
in progress, a narrow window; night and paper, English and Spanish, 75/100/200 %).

## Files outside my area (additive)

- `cantus-core`: `command.rs` (two variants, their arms and labels, `CommandError::Frozen`,
  `apply` → the guard), `model.rs` (two fields), `snapshot.rs` (two fields each),
  `pattern.rs` (`audition` thaws), `lib.rs`, `Cargo.toml` (`sha2`, `serde_json`),
  `tests/undo_invariants.rs` (arm 47, its counter, the invariant check after every command).
- `cantus-project`: `format.rs`, `examples/write_fixture.rs`, `src/midi/import.rs` (the
  new field), `fixtures/v37.cantus`.
- `cantus-engine`: `graph/{plan.rs, plan_compile.rs, ctx.rs, exec.rs, track.rs,
  master.rs, master_run.rs}`, `processor.rs`, `processor_pass.rs`, `processor_msg.rs`,
  `messages.rs`, `backend.rs`, `render.rs`, `stems.rs`, `inst/{stream.rs,
  stream_cache.rs}`, `lib.rs`; tests `fx_tonus.rs` (the monitoring test split), `rt_memory.rs`
  (two scenarios, `Rig::send`).
- `cantus-host`: `session.rs`, `session_inst.rs` (the dormant skip).
- `cantus-sandbox/tests/freeze.rs` (new).
- `cantus-ui`: `app.rs` (field, `assemble`, `tick`, `exec`/`exec_merging` arms,
  `run_action` pre-hook and arms, engine change), `lib.rs`, `keymap.rs`, `settings.rs`,
  `views/{mod.rs, arrangement.rs, mixer_strip.rs, mixer_menu.rs, inspector.rs,
  inspector_channel.rs, audio_notice.rs, audio_popover.rs, export.rs}`, `kit/glyph.rs`,
  `tests/fx_lint.rs`.

## Left

Nothing of mine. Owner-only: listen to a frozen heavy track and its unfreeze, and record a
voice through Tonus with low-latency monitoring on, with real hardware.

## Notes for the integrator

- **Schema**: mine is 36 → 37; regenerate `fixtures/v37.cantus` after chaining (the
  freeze block runs last in `write_fixture`, so the keys are of what the fixture holds).
- **Every new `Command`** must be classified in `freeze_guard::touches` (the build says
  so): controllers' controller-map commands are `vec![]` (they change no sound); warp's
  clip commands are `clip_touches`.
- **Warp** (merged here at 5c17c80): `SetClipWarp` is classified (`clip_touches`); the
  warp is in the subject (it is in `AudioRegion`); freeze and bounce renders wait for the
  warp renders (`compile_for_render`); `a_frozen_strip_with_a_warped_clip_plays_its_render_to_the_bit`.
- **Controllers** (merged here at bef6f62): `InsertMapping`, `RemoveMapping`, `SetMapping`
  are classified `vec![]` in `freeze_guard::touches` (a mapping moves no sound; the
  parameter it moves is its own command, guarded as such); the undo fuzz keeps both
  counters (`mappings`, `froze`).
- **Keymap 9**: four actions appended to `NEW_IN_V9` after controllers' (`ALL` 95,
  `NEW_IN_V9` 13).
- **Known issues** retired: none listed; the manual §14 *Performance* line may add "or
  freeze heavy tracks (§6)".
- `freeze::Locks::of` runs per guarded command while anything is frozen: cheap (a walk
  of the routing), but a project with hundreds of frozen strips would feel it.
