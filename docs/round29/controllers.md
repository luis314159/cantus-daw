# Round 29 — controllers: MIDI learn and controller mapping

Branch `r29/controllers` (worktree `F:\projects\daw-r29\controllers`), from
`r29/integration` at 0933835 (tempo and isolation merged), with `r29/integration` at
5c17c80 (warp) merged in before the final runs. Item 2 of the round. ADR 0062. Schema 35
(and 39 after review 3: the strip switches as targets). Undo-fuzz arm 45.

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

## The runs (Windows, after review 2, on `r29/integration` 6e74f7e merged)

- `cargo clippy --workspace --all-targets -- -D warnings`: clean;
  `cargo clippy -p cantus-plugins --no-default-features --all-targets -- -D warnings`:
  clean. `cargo fmt --all`: clean.
- `cargo test -p cantus-core -p cantus-project -p cantus-host -p cantus-sandbox`: 592
  passed.
- `cargo test -p cantus-engine`: 786 passed.
- `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`: 790 passed.
- `cargo test -p cantus-ui`: 1 468 passed, the frame budget included (a new case: an idle
  frame with a hosted mapping allocates nothing).
- Shots: `cargo run -p cantus-ui --example shoot_controllers -- F:\projects\daw-r29\shots\controllers`
  (90 PNGs, Night and Paper, English and Spanish, 75/100/200 %), looked at.
- Not run here: the Linux check in WSL (`tools/wsl/check.sh`); nothing of mine is behind
  `#[cfg(windows)]`.

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
| 30 | *Link to controller* only from the keyboard | the top bar's **Clip** menu has it beside *Automate the last tweaked* | (menu) |
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

## Review 2 (`reviews/controllers-2.json`, 55 confirmed) — how each was fixed

Merged `r29/integration` 6e74f7e first. Every fix has a test, and each test was seen
failing with its fix taken out again. The fixes were broken on purpose, then put back:

- core: 4 broken, 4 caught (plus the bus test, written before its fix);
- engine: 3 broken, 2 tests caught them;
- interface: 25 broken in one run, 23 tests caught them; the 2 left were the bus fix and the
  commit's own guard, each caught on its own;
- the stepped-control test: 239 keys named when the offers were removed;
- VST3 program change: 1 broken, 1 caught;
- frame budget: 1 broken (the copy of a hosted parameter), 1 caught.

Tests live in `crates/cantus-ui/tests/controllers_review2.rs` unless named otherwise.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0, 19, 23, 40 (high) | an undo, a redo or a mouse edit during a two-knob gesture was overwritten by the gesture's stored commands | the gesture remembers the revision after its own step; another revision seals it and only this frame's targets are sent | `an_undo_during_a_gesture_sticks`, `a_mouse_edit_during_a_gesture_sticks` |
| 17 (high) | a move in the count-in, or heard before a pass at tick 0, failed the commit: the pass's automation lost | `record_point_at` keeps points inside the pass (record start, loop start, punch-in … punch-out); the commit's start never below 0 | `a_move_heard_before_a_pass_at_zero_keeps_the_pass`, `a_move_during_the_count_in_is_not_written_before_the_pass` |
| 20, 53 | a gesture sealed under a mouse step stayed open; an undo let a later turn merge into it | a serial in every gesture's key; `end_gesture_if` seals the key's step wherever it is | bus `a_buried_gesture_is_sealed_by_its_key`, `a_later_turn_is_its_own_step_after_an_undo` |
| 7, 18 | a move just before the loop's start landed at its end on the first entry | a wrap is seen (the playhead going back while looping) and only a message from before it is moved | `the_first_entry_into_a_loop_is_no_wrap`, `a_message_before_a_real_wrap_lands_at_the_loops_end` |
| 1, 24, 51 | learning again a swapped plug-in's parameter kept the old plug-in; the mark and *Forget* acted on the dormant mapping | `Mapping::fits` / `ControllerMap::of_target_in` (no allocation) in learn, the mark and *Forget*; learning again sets `plugin` | `learning_a_swapped_plug_ins_parameter_makes_a_live_mapping`, `learning_again_sets_the_plug_in`, core `the_map_finds_only_the_mappings_of_the_plug_in_loaded` |
| 2, 41 | a momentary CC button held past 300 ms learnt as absolute | a button heard at 127 only waits `HOLD_WINDOW` (1.5 s) for its release; a release learns at once | `a_held_momentary_button_is_learnt_as_a_toggle` |
| 2 (second half) | pick-up on a switch made presses dead | a switch target takes no takeover | core `a_switch_takes_no_takeover` |
| 6 | bank select before a program change learnt a 14-bit CC 0 | `learn::program_after_bank`: the program change is the control, learnt at once | core `a_program_after_bank_select_is_learnt_as_the_program`, `a_program_button_with_bank_select_is_learnt_as_the_program` |
| 4, 12, 27 | learning took a feedback echo for the control | echoes are dropped before learning or driving | `learning_takes_no_feedback_echo` |
| 3, 10, 28, 31 | the echo window ate real presses and was judged at drain time | judged by the message's own arrival, within 30 ms of the send | `a_press_after_feedback_is_a_press_and_a_late_read_echo_is_an_echo` |
| 11, 26 | the echo list grew without limit | pruned before each send (2 s) and bounded (256) | `the_echo_list_stays_bounded` |
| 5, 22, 38 | after a fine half, a coarse half alone was ignored; a fine-first sender lagged | a coarse half alone is read with a fine 0 at the end of the burst; a fine half first, then its coarse half, is the pair (the pairs are cleared on a rescan already) | `a_14_bit_pair_reads_a_coarse_half_alone_and_a_fine_half_first`, round 1's 14-bit test updated |
| 8 | *scale* never moved a stepped parameter | `DriveState::scaled`: its own unrounded position | core `scale_moves_a_stepped_target` |
| 9, 29, 39 | a global file not text, unreadable, newer, or with an unreadable mapping was lost on the next save | read as bytes; any loss keeps the file beside (`.bad`, an earlier one kept with the time) and is said | `a_global_file_that_loses_anything_is_kept_beside` |
| 13 | the rearm could lose the last wake | the rearm is a swap | engine `the_rearm_reports_a_pending_wake` (and the swap's ordering) |
| 14 | the learn hand-off used relaxed atomics | learning stored with release, read with acquire | (ordering; reviewed) |
| 15 | a half-pedal release reached the engine as it was; reset-all and sostenuto not kept | the release sent fully up; CC 121 lets the pedals go; CC 66 tracked | engine `a_pedal_let_go_while_taken_reaches_the_engine_fully_up` |
| 16 | a full ring dropped the newest messages | the newest per address kept aside, handed over after the ring | engine `a_full_ring_keeps_the_newest_value` |
| 21 | the amount row's mouse sent `SetModulator`: not recorded, not last tweaked | `edit_modulator` sends `SetModulatorAmount` when only the amount moved | `a_modulator_amount_moved_with_the_mouse_is_recorded` |
| 25 | a frozen target stopped the gesture and printed a raw error | `freeze::refuses` per target: left out, refused once in words | `a_frozen_target_in_a_gesture_leaves_the_others_moving` |
| 30 | an idle frame allocated per hosted mapping | `hosted_param_info` (looked up, not copied); feedback gathers nothing when none sends | `frame_budget.rs` `an_idle_frame_with_a_hosted_mapping_costs_no_allocation` |
| 32 | no test held the lock-free wake | the wake must run on another thread | engine `the_wake_runs_on_its_own_thread` |
| 33 | `controller_wakes` untested | the ticker's asked delay read after a frame | `an_idle_window_is_woken_for_the_controllers_clocks` |
| 34, 20 | the tempo test could not fail; no gesture across a point | a tempo knob across two points: one step, one command per point, no growth, undo restores both | `a_tempo_gesture_across_a_point_is_one_step` |
| 35 | the typed-curve test was vacuous | `parse_curve`, which the curve cell reads through | `a_curve_is_typed_in_percent` |
| 36, 45 | the rack, the hosted editor, modulator rows, the route menu never probed; the mixer probe near-tautological | probes per row and per strip | `the_channel_rack_offers_learn_on_every_row`, `the_mixer_offers_learn_on_every_strips_controls`, `a_modulator_row_offers_learn`, `a_sends_menu_learns_its_level_by_name`, `a_hosted_editor_offers_learn_on_its_parameters` |
| 37 | the clip test never clicked outside the clip | a spot half under its clip: below, the control's click; inside, the learn | `a_learn_spot_is_clipped_by_its_view` |
| 42 | no v35 fixture; `Mapping::plugin` in no fixture | `write_fixture 35` writes `fixtures/v35.cantus`; the schema-35 block has a hosted mapping with its plug-in | project `schema_35_keeps_the_controller_mappings`, `all_fixtures_load_and_migrate`, core `a_mappings_plug_in_survives_serde` |
| 43 | every action named *Transport* | named by its own group | `an_action_is_named_by_its_group` |
| 44 | stepped controls in instruments and many effects skipped learn | `stepped_param` / `stepped_inst`, `fx::controls::stepped_offer` and keyed segmented, field, card-lamp, selector, LED variants at every plain stepped control | `learn_everywhere.rs` `every_stepped_parameter_is_drawn_with_its_offer` (282 keys) |
| 45 | shots older than the fixes | `shoot_controllers` re-shot with the rack in learn mode, mapped marks on the mixer and an effect, a send's route menu, the pane's action row and idle pills | `F:\projects\daw-r29\shots\controllers\` (`controllers-learn-rack-*`, `-marks-mixer-*`, `-marks-editor-*`, `-route-menu-*`, `-pane-*`) |
| 46 | forgetting an insert named its Bypass | `forget_slot` names the insert | `forgetting_an_inserts_controllers_names_the_insert` |
| 47 | the send menu's learn row did not say what it learns | *Learn the send's level* / *Change …* / *Forget the send level's controller*, both languages | `a_sends_menu_learns_its_level_by_name` |
| 48 | Spanish caption without its noun | *Toma de control de las asignaciones nuevas* | (i18n; shots) |
| 49, 54 | an unloaded or frozen plug-in's mapping read *Missing*; a frozen hosted mapping leaked its control | `mapping_state`: *Missing*, *Other plug-in*, *Not loaded*, *Frozen*; frozen hosted mappings stay the map's and refuse | `a_hosted_mapping_on_a_frozen_track_is_frozen_not_missing` |
| 50 | toggles offered by Add fed nothing back | `action_state` for punch, count-in, low latency, tempo track, typing keyboard, meter wave, arm, strip mute and solo | `every_toggle_with_a_state_feeds_it_back` |
| 52 | a VST3 program change made two undo steps | no parameter event; the idle says the state changed | host `a_program_change_reaches_the_program_list_and_the_project` |

Files outside my area touched by review 2's fixes:
- `cantus-core`: `bus.rs` (`end_gesture_if` searches the history), `freeze_guard.rs` and
  `freeze.rs` (`refuses`, exported).
- `cantus-host`: `vst3/{audio_thread,main_thread}.rs` (the program change as a state change).
- `cantus-project`: `examples/write_fixture.rs` (a schema to stop at; the schema-36 and -37
  blocks run only from their own schema on), `fixtures/v35.cantus` (new),
  `fixtures/v37.cantus` (regenerated).
- `cantus-ui`, kit and models: `kit/automate.rs` (the offers probe),
  `views/modulator.rs` (`mod_offer` notes it; `edit_modulator`),
  `views/modulator_record.rs`, `views/mixer_menu.rs`, `views/route_menu.rs`,
  `views/channel_rack.rs`, `views/plugins.rs`.
- `cantus-ui`, effect editors: `views/fx/{controls,gate,limiter,maximizer_bands,
  maximizer_meters,stereo,stereo_map,tonus,tonus_keys,fluctus_curve,multiband,space,modfx,
  saturator}.rs`, `views/eq_editor_panel.rs`, `views/comp_editor.rs`,
  `views/comp_editor/key.rs`.
- `cantus-ui`, instrument editors: `views/inst/{analog_parts,organ_parts,organ_drawbars,
  wavetable_parts,wavetable_filter,sampler_parts,sampler_cards,fm_env,fm_ops,frame}.rs`.
- Tests and examples: `learn_everywhere.rs`, `frame_budget.rs`, `controllers_review.rs`
  (round 1's loop and 14-bit tests follow the new rules), `examples/shoot_controllers.rs`.

## Review 3 (`reviews/controllers-3.json`, 60 confirmed) — how each was fixed

Merged `r29/integration` df2cdd4 first (a fast-forward). Every fix has a test, and each
test was seen failing with its fix taken out again:

MUTATIONS

Tests live in `crates/cantus-ui/tests/controllers_review3.rs` unless named otherwise.
Numbers are the findings' places in the file, from 0, as in the earlier tables.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0 | two-state stepped parameters were not switches: a held or latching button under pick-up, a relative detent, a pad learnt absolute, the pane's numbers | `controller::Steps` (a switch when stepped with `max − min == 1`, the steps of a choice), computed by the interface (`target_shape`) and passed to `read_in` / `drive_in` / `detect_in`; no takeover, one detent sets it, a pad toggles, the pane says *Off* / *On* | core `a_two_state_parameter_is_a_switch`, `a_pad_on_a_two_state_parameter_is_learnt_as_a_toggle`; `a_two_state_parameter_is_a_switch` |
| 1 | an encoder on a choice moved 1/128 a detent | the step is the sensitivity over the choice's steps | core `an_encoder_moves_one_choice_per_detent`; `an_encoder_moves_one_style_per_detent` |
| 2, 15, 23, 24, 37, 42, 53 | From/To showed a choice's raw number (a hosted one in percent); a name could not be typed | `controllers_words`: an effect's choice names from the editors' table (in the application, not the MPL core), an instrument's from the words its editor drew (noted once per kind and key), a hosted one's from the plug-in's text (64 steps at most); `value_named` reads a typed name in either language | `a_stepped_choice_reads_its_names`, `an_instruments_choice_reads_its_editors_words`, `a_hosted_stepped_parameter_reads_the_plug_ins_words` |
| 3, 21, 52 | VST3: the host's own writes of the program parameter (the project, automation, a mapped knob) counted as program changes | an `AtomicBool` set only by a MIDI program change (0xC0) from an input; the idle swaps it | host `a_program_change_reaches_the_program_list_and_the_project` (a host-sent value makes no state step). A session test needs a VST3 with a program list; the backend test holds the rule |
| 4, 22, 27 | one fine-only update before a coarse change made the controller "fine first" for good | fine-first only after `FINE_FIRST` (3) fine-then-coarse pairs; a coarse-then-fine pair clears it | `a_lone_fine_half_proves_nothing`, `a_fine_first_controller_is_read_whole` |
| 5, 9, 38, 56 | a coarse half was read with a fine 0 at the end of its burst, whatever its age: a pair split between frames played and recorded a dip | a half whose partner is expected waits `PAIR_GAP` (5 ms) across frames (the wake asks for the frame), then is read alone; halves left waiting are resolved before a burst; a fine-first controller's fine half waits too | `a_pair_split_between_frames_is_never_half_made`, `a_split_pair_records_no_half_made_point`, `a_fine_first_controller_is_read_whole` |
| 6 | NRPN / RPN learnt as a plain CC 99 | the learn refuses CC 98–101 as a first control and says so | `an_nrpn_knob_is_not_learnt` |
| 7 | the selected strip did nothing with the master selected | `Focus::master`; the selected strip's targets are the master's then | core `the_selected_strip_can_be_the_master`; `the_selected_strip_can_be_the_master` |
| 8, 29, 39, 47 | *Frozen* only for hosted targets | `mapping_state`: any target a frozen sound refuses (`freeze::refuses`) reads *Frozen* | `a_native_target_on_a_frozen_sound_is_frozen` |
| 10, 26 | the overflow table keyed pitch bend and pressure by value | one slot per channel for pitch bend and channel pressure | engine `a_full_ring_keeps_order_and_the_last_bend_and_pressure` |
| 11 | parked values were handed over after newer ring messages | while anything is parked, newer messages park too; the take sorts by arrival and keeps each message's time | engine `a_full_ring_keeps_order_and_the_last_bend_and_pressure` |
| 12 | CC 121 let the pedals go, though native instruments ignore it | CC 121 is no release | engine `a_pedal_let_go_while_taken_reaches_the_engine_fully_up` |
| 13 | a half pedal and the soft pedal were not tracked | sustain, sostenuto and soft tracked whenever above 0 | engine `a_pedal_let_go_while_taken_reaches_the_engine_fully_up` |
| 14, 31, 33, 55 | feedback's echo was stamped after the send: a fast loopback's echo was taken for a press | stamped before the send, 2 ms of skew allowed | `an_echo_stamped_before_its_send_is_an_echo` (and review 2's `learning_takes_no_feedback_echo` without timing luck) |
| 16 | a punched pass's clip ran on to the stop, latched | the commit's end is clamped to the punch-out; the latch lets go there | `a_punched_pass_ends_its_clip_at_the_punch_out` |
| 17 | a gesture holding the tempo grew its step every frame | bounded by tempo's review 4 (`CommandBus::rebase`, gestures rebuilt from their start); held by a test | `a_tempo_gesture_stays_small` |
| 18 | a pass stopped before its start committed a one-tick clip | a false start writes nothing | `a_false_start_writes_no_automation` |
| 19, 34, 58 | count-in moves inside a loop were written before the record start; the punch bounds | the pass's start is the record start, the loop's start only once a wrap was seen in this pass (`last_wrap` reset at the pass's start); the punch wins | `a_count_in_inside_the_loop_lands_at_the_record_start`, `a_message_before_a_real_wrap_lands_at_the_end`, `the_punch_bounds_hold` |
| 20, 28 | the freeze refusal was said once per session | said again after a pause (the refusal is part of the gesture's time) | `a_frozen_refusal_is_said_again_after_a_pause` |
| 25 | a learn armed survived opening another project | the project's replacement cancels the learn and clears the controllers' states, sends and gesture (`follow_project`, an epoch) | `a_learn_lets_go_when_another_project_comes` |
| 30, 40 | a lossy global file was copied beside at every start; a failed copy let the next edit overwrite it; the status named the wrong file | `GlobalLoad` (whole / kept, naming the copy / unsafe: the session never saves over it); an identical copy is not made twice | `a_lossy_global_file_is_kept_once` |
| 32, 44 | an *Other plug-in* row was named after the plug-in loaded now and counted as shared | `mapping_name` names the plug-in it was learnt on (the catalogue); `sharing_in` skips mappings that do not fit | core `a_dormant_plug_in_mapping_is_not_shared`; `a_swapped_out_plug_ins_mapping_keeps_its_name_and_shares_nothing` |
| 35 | the stepped-offer test checked a hand-written list | exhaustive: every stepped key of every editor (FM's on each detail page, the sampler's selected zone) is offered, or `not_drawn` says why the demo draws no control for it; the plain controls found were wired (Organ's power lamps, the EQ's solo LED, the sampler zone fields, the wavetable's table selector) | `learn_everywhere.rs` `every_stepped_parameter_is_drawn_with_its_offer` |
| 36 | the probes' "learnable" flag was always true | the learn probe notes a name only after the visibility check; the spots probe records each outline drawn, and tests click every one and check what it arms | `clicking_the_racks_controls_arms_them`, `clicking_the_mixers_controls_arms_them`, `clicking_a_hosted_editors_cells_arms_them` |
| 41 | the curve test tested the parser; a typed bend under 2 % snapped to 0 | the snap is a drag's only (`CellDone::typed`) | `a_curve_is_typed_into_its_cell_as_typed` (typed through the pane) |
| 43 | Tonus in the learn mode: a black key's right half learnt the next white key; white outlines crossed the black keys | each key outlined and covered only where it shows (`shown_parts`), white keys first | `a_black_keys_right_half_learns_the_black_key` (and the shot) |
| 45 | Linux: ALSA's `client:port` numbers in a mapping's port | `clean_port_name` (the numbers and the client dropped) in `PortRef::of` and matching; a port not open reads *Not connected* | core `an_alsa_port_name_is_the_devices_name`; `a_port_not_open_is_not_connected` |
| 46 | the strip's effects switch and record dot could not be learnt | `MappingTarget::StripFx` / `StripArm` (schema 39, a no-op migration, `fixtures/v39.cantus`, fuzz targets), learn spots on the strip and rows in its menu | core `a_strips_effects_and_record_arm_set_and_read`; project `schema_39_keeps_the_strip_switch_mappings`; `a_strips_effects_switch_and_record_arm_are_learnable`, `clicking_the_mixers_controls_arms_them` |
| 48 | the learn mode and the show/hide toggles fed nothing back | `action_state` covers the learn mode, every pane's toggle, the instrument window and the takes | `the_learn_mode_and_the_areas_light_a_button` |
| 49 | an unknown action read *Transport · id* | *Unknown action*, and its row *Missing* | `an_unknown_action_is_missing` |
| 50 | Spanish *Otro plugin* | *Otro plug-in* | `the_spanish_pill_says_plug_in` |
| 51 | the offers allocated every frame; a card stored a `Response` in egui's memory | no target built while idle (learn off, nothing mapped there, no menu: `FxHost::learn_idle`, `stepped_inst_of`); the card's lamp response is returned | `frame_budget.rs` `an_editors_idle_stepped_controls_cost_no_allocation` (Compressor 191 / Wavetable 1 494 allocations a warm frame; 209 / 1 646 with the offers building) |
| 54 | swapping an insert left its controllers *Missing* | the swap's batch retargets them: bypass and mix, a native key the new effect has, a hosted parameter stays its plug-in's | `swapping_an_insert_keeps_its_controllers` |
| 57 | a crashed isolated plug-in's mappings looked live | *Not loaded* | `a_crashed_plug_ins_mapping_is_not_loaded` |
| 59 | the hosted editor and inspector kept the fallback number as a plug-in's words; the editor's words were not per effect | `ValueWords` (the editor) and `Texts` (the inspector) ask again while unanswered; the editor's are per effect | `a_hosted_values_words_are_asked_until_answered`; `modulator_hosted` `an_unanswered_value_is_asked_again` |

The coordinator's note on tempo's review 4 (a tempo knob waits while a take records): the
controllers' recording writes no tempo (it records parameters only), and a gesture that
turned the tempo before the take is sealed when the take begins, so no tempo command is
sent during it — `a_gesture_holding_the_tempo_sends_none_while_a_take_records`.

Files outside my area touched by review 3's fixes:
- `cantus-core`: `tests/undo_invariants.rs` (the strip switches).
- `cantus-engine`: `midi.rs` (the overflow's order and slots; the pedals).
- `cantus-host`: `vst3/{instance,audio_thread,main_thread}.rs` (the MIDI-only program
  flag), `session.rs` (`mark_loaded_crashed_for_tests`), `tests/vst3_backend.rs`.
- `cantus-project`: `format.rs` (schema 39), `examples/write_fixture.rs`,
  `fixtures/v39.cantus` (new).
- `cantus-ui`, app and models: `app.rs` (`GlobalLoad` at boot), `i18n.rs` (`t_in`),
  `kit/automate.rs` (the spots probe), `record_session.rs`, `swap.rs` (retargeting),
  `views/modulator_record.rs` (the pass's bounds), `views/modulator_hosted.rs`,
  `views/plugins.rs` (`ValueWords`), `views/transport.rs` and `transport_menus.rs`
  (`pane_of`), `views/inspector.rs` (`follows_master`), `views/mixer_menu.rs`,
  `views/mixer_strip.rs`.
- `cantus-ui`, editors: `views/fx/{controls,host,multiband,space,tonus_keys}.rs`,
  `views/eq_editor_panel.rs`, `views/inst/{analog_parts,frame,organ,organ_parts,
  sampler_map,sampler_parts,sampler_wave,wavetable_osc,wavetable_parts}.rs`.
- Tests: `learn_everywhere.rs`, `frame_budget.rs`, and the earlier controllers tests where
  the rules changed (RULES).

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
- Review 3: **schema 39** (`MappingTarget::StripFx` / `StripArm`; a no-op migration
  `migrate_38_to_39`, `fixtures/v39.cantus`, the fixture's schema-39 block). Renumber if
  another branch took 39 (freeze took 38 before me).
- Review 3: `Mapping::read_in` / `drive_in` / `controller::detect_in` take a `Steps` (the
  target's shape); `read` / `drive` / `detect` remain, with `Steps::default()` (the old
  behaviour). The interface's `CantusApp::target_shape` computes it.
- Review 3: a gesture that turned the tempo is sealed when a take begins (tempo's review-4
  rule, from the controllers' side).
