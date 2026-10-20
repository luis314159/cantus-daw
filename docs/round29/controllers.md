# Round 29 — controllers: MIDI learn and controller mapping

Branch `r29/controllers` (worktree `F:\projects\daw-r29\controllers`), from
`r29/integration` at 0933835 (tempo and isolation merged), with `r29/integration` at
5c17c80 (warp) merged in before the final runs. Item 2 of the round. ADR 0062. Schema 35.
Undo-fuzz arm 45.

## Done

1. **The input** (`cantus-engine/src/midi.rs`): every port's callback runs `route` — a
   lock-free bit-set filter (`MidiFilter`: port × channel × address, 32 ports) consumes
   what the maps take (it never reaches the engine: a mapped knob no longer bends the
   instrument, a mapped pad plays no note), tees it with its port and `Instant` into a
   per-port SPSC ring (`Heard`), and wakes the interface once until it drains; while a
   control is learnt, everything is taken. The lamp counts mapped messages too. Ports by
   **name + ordinal** (`PortRef`). `MidiInputs::open_with(sender, tap)`; the tap made in
   `boot.rs` (`Stage::Audio`, waking the window), kept by `assemble`, reused by
   `restart_audio` and `rescan_midi`. **Channel pressure, key pressure, program change**
   now reach the live instrument (`InstrumentUnit::program`, default nothing; hosted: CLAP
   as MIDI, VST3 through `kCtrlProgramChange` as `program / steps`). **MIDI out** for
   feedback (`MidiOutputs`, the device's output found by name).
2. **The model** (`cantus-core/src/controller*.rs`, MPL): `Mapping` (source, target, mode,
   range, curve, takeover, feedback, sensitivity), `MappingTarget` (any `AutoTarget`;
   track/strip mute, solo, arm; insert bypass and mix; the tempo through ADR 0060 §2's
   rule; any keymap action; selected track/strip; the focused device's knobs 1–8),
   `ControllerMap`; the drive arithmetic (absolute, relative ×3 encodings, toggle,
   momentary, trigger; range, bend, inversion; jump / pick up / scale), learning
   (knob, 14-bit pair, encoders by band, button, pad, program). `AutoTarget::command`
   (`automation/set.rs`), the inverse of `value_of_command`. `Project::controllers`
   (schema 35, skipped when empty), `InsertMapping` / `RemoveMapping` / `SetMapping` with
   exact inverses, `cmd.controllers.*`, fuzz arm 45 with its counter; fixture block and
   `schema_35_keeps_the_controller_mappings`.
3. **The interface** (`cantus-ui/src/controllers*.rs`, `views/controllers.rs`): learn in
   every parameter's menu (`Choice::Learn` / `Forget`, through `mod_offer`), the mapped
   mark, the **learn mode** (`Ctrl+Shift+M`, a click on the top bar's MIDI cell —
   Live's *MIDI* key; a right-click there opens the MIDI settings —, the pane's *Learn*): every learnable control outlined above its painting, a click arms it, the
   value does not move; mute/solo/arm lamps, the top bar's transport keys, PAT/SONG,
   metronome, tap and the tempo well learnable too; mixer strip/master/insert menus and the
   track header menu *Learn* rows; *Link the last tweaked parameter* (`Alt+Shift+A`);
   the per-frame controller input (one command per target per frame, one undo step per
   gesture, sealed after 500 ms; two knobs one batch), recording at the message's own
   time, the **latch** while a pass records, feedback, the project before the global map,
   dormant mappings; the **global map** `controllers.json` beside `keymap.toml`, read in
   the Files stage; **`Pane::Controllers`** (`Ctrl+Shift+K`, View menu) with both maps,
   live dots, every field edited in place, *Shared* / *Missing*, filter, *Add a mapping*,
   the takeover new mappings get; keymap 9 (`NEW_IN_V9` + 3); settings
   `controller_takeover`. Strings in English and Spanish (`locales/areas/controllers.*`).
4. **Shots**: `F:\projects\daw-r29\shots\controllers\` (`shoot_controllers`): the pane
   (one row waiting to learn again, one lit), the learn mode over Comprimo's editor, the
   mixer and the arrangement, the top bar, a parameter's menu — Night and Paper, English
   and Spanish, 75/100/200 %, and the pane narrow (two header rows, both-way scroll).
5. **Docs**: ADR 0062; the manual's *MIDI controllers* (§4, after *MIDI*) in both
   languages.

## Measurements

- Nothing new on the audio thread allocates (`rt_memory::live_pressure_and_program_changes_never_allocate`);
  the callbacks' added work is one relaxed atomic load and, for a mapped message, one SPSC
  push.
- The interface: an idle frame does a revision compare and two equality checks (the
  filter is republished only when the project's revision, the global map or the ports
  change); no allocation without MIDI.
- A mapped move costs what a mouse drag costs (one command and one snapshot per frame at
  most).

## Tests and what each holds

- Core `controller::tests` (20): encodings; shape/unshape inverse (range, bend,
  inversion); jump; pick up never jumps, picks up on crossing, retakes after an outside
  move; scale converges; relative steps, range, sensitivity; toggle once per press,
  release ignored, CC as button; momentary; action fires once per press, program change;
  pad velocity; feedback values; learning of each kind; ports by name and ordinal;
  overlap; relative targets; switch and tempo commands; the commands' inverses and
  refusals; supersedes per mapping; no field when empty.
- Core `tests/controller_targets.rs` (3): every kind of `AutoTarget` has its command,
  read back by `value_of_command` and held by the project; `command(value_of_command(c))
  == c`; a missing target has none. Seen failing with a broken arm.
- Core `undo_invariants` arm 45 (+ counter: insert, remove, set all applied).
- Project `schema_35_keeps_the_controller_mappings` (seen failing before the fixture).
- Engine `midi::tests` (5): the filter per port/channel/address and 14-bit pair; `route`
  tees, consumes, wakes once, counts; outputs found by name. `tests/midi_controllers.rs`
  (2): a mapped CC and pad never reach a live instrument through a real `Processor`, the
  same pad on another channel plays; pressure, key pressure and program change arrive.
  `rt_memory` (above).
- Host `vst3::instance::tests::a_program_change_picks_its_program`.
- UI `tests/controllers.rs` (18), `tests/learn_everywhere.rs` (4),
  `tests/controllers_boot.rs` (2): learn from the menu / again / forget (one step each);
  encoders, pads, 14-bit, a release ignored; a transport key into the global file; the
  global file's lenient read; pick up and one undo step per gesture, a new one after idle;
  two knobs one batch; encoder, pad toggle, action once; project first, dormant does
  nothing and consumes nothing; the filter follows maps, undo and ports; recording one
  clip at the right ticks; a message's tick through the map; the latch (lane held while
  recording, the clip again after); feedback once per change and its bytes; the keymap's
  three actions and a v8 file; learn mode on a kit knob (click learns, drag moves
  nothing); the pane's words/flags/filter and its remove and feedback keys; learn offered
  on every control of every native effect and instrument editor, the inspector's rows,
  sends and pinned strip; Chorda's tuning keys refused; the boot path (global map read,
  waking tap kept, ports reopened with it). Mutation-checked: learn offer removed (3
  probe tests fail), project-first removed and latch removed (6 tests fail).
- `fx_lint` lists `views/controllers.rs`; `modulators.rs` / `shoot_modulators.rs` literals
  gained `learn: None`.

## The runs (Windows, after merging `r29/integration` 5c17c80)

- `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --all`:
  clean.
- `cargo test -p cantus-core -p cantus-project -p cantus-host --no-fail-fast`: 472 passed.
- `cargo test -p cantus-engine --no-fail-fast`: 725 passed on the merged tree (before
  the merge, `inst_stream::the_suite_leaves_no_cache_folder_behind` once failed under the
  machine's load and passed alone — Exemplar's streaming cache, not this work).
- `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`: 720 passed (before the
  merge; the merge brought warp's engine files, none of mine).
- `cargo test -p cantus-ui --no-fail-fast`: 1 333 passed (the merged tree).
- Not run here: the Linux check in WSL (`tools/wsl/check.sh`); nothing of mine is behind
  `#[cfg(windows)]` (midir is cross-platform).

## Files outside my area (additive)

- `cantus-core`: `command.rs` (three variants, labels, a supersedes arm, three apply arms
  delegating to `controller::apply_*`), `model.rs` (`Project::controllers`), `lib.rs`
  (`pub mod controller`), `automation/mod.rs` (`pub mod set`).
- `cantus-engine`: `messages`-free; `processor_msg.rs` (0xA0 / 0xC0 / 0xD0 arms),
  `processor.rs` (`controller_of`'s 0xD0 arm, `LIVE_TAG` made `pub(crate)`),
  `graph/track.rs` (`expression_live`, `program`), `inst/mod.rs` (`program` with a
  default), `lib.rs` (re-exports).
- `cantus-host` (isolation's): `inst_unit.rs` (`program`), `vst3/instance.rs`
  (`kCtrlProgramChange`, `program_steps`, `program_value` and its test),
  `vst3/main_thread.rs` (the program-list parameter stays visible).
- `cantus-project`: `format.rs` (migration 34→35's doc, the schema-35 test),
  `examples/write_fixture.rs` (the schema-35 block), `fixtures/v36.cantus` regenerated.
- `cantus-ui`: `app.rs` (the field, `midi_inputs` `pub(crate)`, `assemble` wiring, `Files`
  fields, `restart_audio` / `rescan_midi` reopen with the tap, the `run_action` pre-hook
  and arm, `controller_input` in `frame`, the latch in `sync_engine`), `boot.rs` (the
  waking tap), `lib.rs`, `keymap.rs` (three actions, bindings, `NEW_IN_V9`),
  `settings.rs` (`controller_takeover`), `kit/automate.rs`, `kit/widgets.rs` (probe and
  mark), `kit/glyph.rs` (`Glyph::Controller`), `views/mod.rs`, `views/modulator.rs`,
  `views/modulator_record.rs` (`record_tweak_at`), `views/modulator_section.rs` and
  `views/inspector_pinned.rs` (probe), `views/fx/controls.rs`, `views/inst/frame.rs`,
  `views/mixer_menu.rs` (typed `Act`s appended), `views/mixer_strip.rs` (lamps),
  `views/arrangement.rs` (lamps, the chained track menu), `views/arrangement_rec.rs`
  (arm lamp), `views/transport_keys.rs` (the MIDI cell toggles learn mode, right-click
  opens settings; learn spots),
  `views/transport_metro.rs`, `views/transport_tempo.rs`, `views/transport_menus.rs`
  (View menu), `views/workspace.rs`, `views/workspace_frame.rs` (the pane);
  tests `fx_lint.rs`, `modulators.rs`; example `shoot_modulators.rs`.

## Left

Nothing of mine. Owner-only: trying it with a real controller (a keyboard with knobs and
pads; a controller with LED rings or motor faders for feedback, whose output name must
pair with its input — `output_for` covers the usual Windows names).

## Notes for the integrator

- Schema 35 is real; `fixtures/v36.cantus` regenerated here with 34, 35 and 36; write
  `v35.cantus` after the chain is final if you want a file per number.
- Keymap 9: appended `MidiLearnMode` (`Ctrl+Shift+M`), `LinkLastTweaked`
  (`Alt+Shift+A`), `ViewControllers` (`Ctrl+Shift+K`) to `NEW_IN_V9`; manual §12 gets
  these three.
- `ModOffer` gained `learn` (struct literals elsewhere need `learn: None`).
- `InstrumentUnit::program` is new (default nothing); a bridged hosted instrument gets it
  through `inst_unit.rs` like the other MIDI.
- Release notes / manual §14: the known issue "Not in this beta: … MIDI learn and control
  surfaces" becomes "… control surfaces (Mackie Control / HUI)" — MIDI learn is in.
- The top bar's MIDI cell now toggles the learn mode on a click (Settings on a
  right-click); its width is unchanged.
