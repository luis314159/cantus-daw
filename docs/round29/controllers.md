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

## Review 1 (`reviews/controllers-1.json`, 57 confirmed) — how each was fixed

Merged `r29/integration` 3c9659c first. Every fix has a test; the tests were written with the
fixes and checked by breaking the fix again (core: three mutations, three failures; host
and engine: the behaviour the test pins was absent before; interface: six mutations — the
seal, the widening, the loop, the echo, the publish after learning, the 14-bit wait — six
failures, plus the learn offer and the latch from round one).

| # | Finding | Fix | Test |
|---|---|---|---|
| 0 | a button without a release (program change, a 127-only CC) pressed once ever | `Mapping::read`: a program change is a press and a release at once; a CC button's repeated press value is a new press | `a_program_change_presses_every_time`, `a_cc_button_without_a_release_presses_every_time`, UI `an_encoder_steps_and_a_button_toggles_and_fires_once` |
| 1, 19 | the tempo target read the ramp's tempo, wrote the governing point | `current_pos(Tempo)` reads the governing point (`point_at`), what `set_tempo_at` writes | `the_tempo_target_reads_what_it_writes_on_a_ramp` |
| 2, 54 | an encoder could not move a stepped target or a switch | `DriveState::rel`: the unrounded position, kept while nothing else moves the target; a switch flips with one turn | `an_encoder_moves_a_stepped_target_one_frame_at_a_time`, `an_encoder_flips_a_switch_with_one_turn` |
| 3, 14, 17, 25 | the gesture never sealed on an idle window; learns stalled | sealed by the messages' own time before driving; `controller_wakes` asks the ticker for a frame at the gesture's end, the learn window, the learn timeout, feedback's quiet | `bursts_apart_are_two_steps_without_a_frame_between` |
| 4 | a pad under pick up mostly did nothing | a button control has no takeover (its velocity is where it puts the target) | `a_pad_sets_its_velocity_under_pick_up` |
| 5, 33 | a modulator's amount moved while recording recorded nothing | `Command::SetModulatorAmount` (read back by `value_of_command`, fuzz arm 45, freeze guard); the controller records its target and position itself (`record_point_at`) | `a_modulator_amount_moved_while_recording_is_recorded`; the round trips without the ModAmount skips |
| 6 | a sign-bit encoder learnt one way jumped half the range | `Encoding::reinterpret`: a jump read in the learnt encoding is read in the one that makes it a step, kept per mapping | `an_encoder_learnt_one_way_is_read_right_the_other_way` |
| 7 | precedence by exact address; feedback ignored precedence | the project takes a message by any of its addresses (a 14-bit pair's halves); shadowed global mappings send no feedback | `the_project_takes_a_message_whatever_address_it_maps`, `a_shadowed_global_mapping_sends_no_feedback` |
| 8 | a malformed map from a file broke undo | `ControllerMap::repair` on every project read | `a_map_from_a_file_is_repaired` |
| 9 | a 7-bit-only 14-bit pair never reached its top; half-made values | the coarse value scaled to 14 bits until a fine half is seen; then the pair is read on its fine half only | `a_14_bit_pair_reaches_its_top_and_waits_for_its_fine_half` |
| 10 | a latching CC button learnt as toggle | one value heard (no release) → absolute; both → toggle | `a_latching_button_is_learnt_absolute` |
| 11 | 14-bit feedback sent six bytes in one call | `feedback_messages`: one channel message per send; `sent` only when every send went | `feedback_once_per_change_quiet_while_moving_and_deaf_to_its_echo` |
| 12, 38 | learning or a new mapping swallowed note-offs and the pedal: stuck notes | `PortState` per port: a release of what the engine holds always reaches it (and is teed) | `a_held_key_or_pedal_is_released_whatever_the_filter_says`, engine `learning_never_leaves_a_note_stuck` |
| 13, 26 | the callback took egui's lock to wake the window | a parked waker thread per tap; the callback sets a flag and unparks | `rt_memory::the_midi_callback_never_allocates`, `route_tees_what_is_taken_and_wakes_once` |
| 15 | feedback echoed back by a loopback port undid presses | feedback just sent, by port, is recognised on the input for 150 ms and dropped | `feedback_once_per_change_…` |
| 16 | a learnt control leaked into the instrument for a frame | the filter is published before the tap stops taking everything | `a_learnt_control_is_taken_at_once` |
| 18 | a multi-target gesture rebuilt untouched targets from a read-back | the gesture keeps each target's last command and re-sends exactly that | `an_untouched_tempo_target_is_not_rewritten` |
| 20 | a tempo gesture across a point composed every frame | the tempo is a gesture target per governing point | (the key; with #18's test) |
| 21 | an arm in a gesture made the batch grow | `SetTrackArmed` supersedes per track | `arming_and_a_modulator_amount_supersede_per_target`, `an_arm_in_a_gesture_keeps_the_step_small` |
| 22 | a knob joining later made several steps | `CommandBus::widen_merging`: the step widens, its inverse composed | bus `a_widened_gesture_is_one_step_and_seals_only_itself`, `a_knob_joining_later_is_the_same_step` |
| 23 | a move before a loop wrap landed before the loop | `tick_in_loop` | `a_message_before_the_wrap_lands_at_the_loops_end` |
| 24 | the seal ended whatever step was on top | `CommandBus::end_gesture_if(key)` | bus test above |
| 27 | the channel rack's knobs and lamp could not be learnt | `track_knob` (the parameter's menu, marks, learn mode); the lamp a learn spot | (the probe and shots) |
| 28 | the route menu had no learn row for a send level | *Learn / Change / Forget MIDI controller* rows | (the menu's rows; shots) |
| 29, 47 | the MIDI cell's words contradicted its click | without a port a click opens the settings and the tip says so; with ports the learn half carries the shortcut | `without_ports_the_midi_cell_opens_the_settings` |
| 30 | *Link to controller* only from the keyboard | the clip menu's last-tweaked block has it | (menu) |
| 31 | the global file written non-atomically; a damaged one lost | written through a file beside, synced, renamed; a damaged one kept as `.bad` and said | `a_damaged_global_file_is_kept_and_the_map_written_whole` |
| 32 | an armed learn never timed out | `LEARN_TIMEOUT` 15 s, woken for | `an_armed_learn_times_out` |
| 34 | the "every target" test only counted | `kind()` — an exhaustive match, no wildcard | `every_target_has_the_command_that_sets_it` |
| 35 | the mixer, top bar and spots were never probed | `note_learn` in `DeskMod` and every learn spot; mixer and top-bar probes; a spot's click test | `the_mixer_and_the_top_bar_offer_learn_everywhere`, `a_learn_spot_takes_the_click_inside_its_clip_only` |
| 36, 56 | no hosted mapping test; a mapping followed a swapped plug-in | a test through the real test CLAP; `Mapping::plugin` (learnt with its plug-in), dormant under another | `a_knob_drives_a_hosted_parameter_and_a_missing_one_is_dormant`, `a_hosted_mapping_is_dormant_under_another_plug_in` |
| 37 | no test of the message's own time | `EngineHandle::set_heard_for_tests`; a message 0.5 s old read at beat 4 recorded at beat 3; exactly one step | `a_move_is_recorded_at_its_message_time_in_one_step` |
| 39 | the feedback test held neither once nor quiet | a sink (`feedback_log`) counts the sends | `feedback_once_per_change_…` |
| 40 | the dormant test never drove the control | drives it; the global fallback moves, nothing else | `a_dormant_mapping_lets_the_global_one_hear_its_control` |
| 41 | bypass and mix targets untested | | `an_inserts_bypass_and_mix_set_and_read` |
| 42 | a typed curve read as a position | read as a pan's percent, −100…100 | `typed_values_read_their_units` |
| 43 | typing ignored units | a device's parameter through `parse_readout` with its unit | `typed_values_read_their_units` |
| 44 | *Send* on actions did nothing | playing, recording, loop, metronome and PAT are states fed back; no *Send* where there is none | `transport_buttons_feed_back_their_state` |
| 45 | no mapped mark on desk controls, pinned params, modulator rows, lamps | `learn_mark` there; a lamp's mark through `learn_spot_in` | (shots) |
| 46 | outlines spilled out of the mixer's and arrangement's clip | `learn_spot_in(…, clip)`: outlined and covered inside the view's clip only | `a_learn_spot_takes_the_click_inside_its_clip_only` |
| 48 | stepped device controls could not be learnt | `stepped_offer`: menu, mark, learn mode on LEDs and segmented choices | `learn_everywhere` (probes them) |
| 49 | the manual promised every action; only transport offered | *Add a mapping* offers every keymap action by its group; the manual says so | (pane; manual) |
| 50 | §12 lacked the shortcuts | both manuals | — |
| 51 | *Forget these controllers* made several steps | `forget_targets`: one batch, one status | `forgetting_several_controllers_is_one_step` |
| 52 | Spanish words | *Toma de control de las nuevas*, *CC {n} (14 bits)* (and the English caption) | i18n |
| 53 | VST3 program change asked IMidiMapping for 130 | the `kIsProgramChange` parameter (root unit first); the change goes out as the plug-in's own, so the project keeps it; the test pulse has a program list | `a_program_change_reaches_the_program_list_and_the_project`, `the_program_list_is_found_by_its_flag` |
| 55 | an unloaded hosted parameter swallowed MIDI and looked alive | `mapping_alive`: loaded and present, else dormant; the filter's cache keyed on the loaded parameters | `a_knob_drives_a_hosted_parameter_and_a_missing_one_is_dormant` |

Files outside my area touched by the review's fixes: `cantus-core` `bus.rs`
(`end_gesture_if`, `widen_merging`), `freeze_guard.rs` (the `SetModulatorAmount` arm),
`automation/{mod,set}.rs`; `cantus-engine` `backend.rs` (`set_heard_for_tests`, hidden);
`cantus-host` `vst3/{instance,main_thread,audio_thread}.rs`; `cantus-test-plugins`
`vst3/src/{pulse,lib}.rs` (a hidden program-list parameter, `cantus_test_last_program`);
`cantus-project` `format.rs` (repair on read); `cantus-ui` `views/channel_rack.rs`,
`views/route_menu.rs`, `views/transport_menus.rs` (clip menu), `views/inspector_pinned.rs`,
`views/modulator_section.rs`, `views/fx/controls.rs`, `views/arrangement_rec.rs`
(`header_input` takes the clip), `kit/automate.rs`, `app.rs` (the damaged-file status).

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
