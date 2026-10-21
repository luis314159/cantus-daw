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
Review 2: a take whose backing's heard latency moves mid-pass lands both its clicks where
they were played, to the frame (the second was the EQ's latency off without the shift); a
bounce through a linear-phase EQ with a fader ride and a key carried plays the song to the
bit in Inserts and Strip modes (trimmed, the ride landed the EQ's latency early); a time
range through the inserts within −120 dB (was −60 dB: the transport stopped at its end).
Review 3: with the bounce's file on the grid and the delay on the track, the song plays to
the bit as before, and the bounced clip moved or pasted to a plain track plays the song to
the bit too (it was the EQ's latency late); through DAWproject and back within 1e-4 of the
song; a frozen strip's note source running for its notes only plays 0 voices over 300
blocks. No golden moved; `DSP_VERSION` unchanged.

## Tests and what each holds

See ADR 0064 §Tests for what each holds. The first run, after merging `r29/integration`
at bef6f62 (warp and controllers), all `--no-fail-fast`, 0 failed (the runs after each
review are in their sections; the last, after review 3: core 416, project 112, engine 819,
fuzz 823, host 82, sandbox 64, ui 1 573):

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
clean. `shoot_freeze` renders 98 PNGs to `F:\projects\daw-r29\shots\freeze\` (arrangement,
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

## Review 1 — the fixes (62 confirmed: 15 high, 27 medium, 20 low)

`F:\projects\daw-r29\reviews\freeze-1.json`, merged with `r29/integration` at 3c9659c first.
Every fix has a test seen failing without it (the fix taken out, the test run, put back),
but where a row says otherwise. *core*, *engine*, *ui*, *ll*, *project* name
`cantus-core/tests/freeze.rs`, `cantus-engine/tests/freeze.rs`, `cantus-ui/tests/freeze.rs`,
`cantus-engine/tests/low_latency.rs` and `cantus-project`'s tests.

| # | Sev | Finding | Fix | Test |
|---|---|---|---|---|
| 0, 7 | H | Freezing a bus, a return or a keyed strip silenced what also plays live | `freeze::dormant` rests only what reaches nothing but playing frozen strips (a fixed point over outputs, sends, track routing; a missing output is the master's) | core `a_frozen_return_or_keyed_bus_never_silences_what_also_plays_live`, `what_rests_…` (updated); engine `a_frozen_return_keeps_the_dry_signal_…`, `a_frozen_keyed_bus_and_a_note_source_…` (to the bit) |
| 1, 8 | H | Solo baked into the render, not in the key | Renders made unsoloed (`freeze::neutral`; a strip's only input unmuted); a solo **inside** a region refuses the freeze (`CannotFreeze::Soloed{Track,Strip}`) | engine `a_solo_when_it_froze_is_not_in_the_render`; core `freezing_a_region_with_a_solo_inside_…` |
| 2 | H | A frozen track's region did not follow its modulators' sources | `region(Track)` takes in each followed strip's whole region (and `whole`); `Locks` and `reaches` follow | core `a_frozen_tracks_region_takes_in_the_audio_its_modulators_follow` |
| 3 | H | Saving moved a frozen take, then the path change was refused | `freeze::rekeyed`: collect and relink run through the frozen targets they move (unfreeze, paths, freeze with the new key — one step); renders copied under the new key beside the project; a refused collection puts the files back (`collect::undo`) | core `a_frozen_takes_file_moved_by_saving_rekeys_…`; ui `saving_a_frozen_take_into_its_folder_…`; project `a_collection_undone_puts_the_files_back` |
| 4 | M | Moving a strip/track changed the key unchecked | The root under its own key; automation clips in no order of tracks (ties ranked); sources by id; `MoveStrip`/`MoveTrack` applied and checked; a refresh of a stale key re-freezes first (SUBJECT_VERSION 2) | core `moving_strips_and_tracks_keeps_every_key_or_is_refused` |
| 5 | L | A follower of the frozen strip heard its live fader | `Region::hears_root`: the root's mix in the key and locked (a track's level likewise) | core `a_follower_of_the_frozen_strip_itself_locks_its_fader` |
| 6 | L | A file edited on disk: re-freezing reused the old render | `start` re-renders when a file the region plays is newer than the render | ui `a_render_older_than_a_file_it_plays_is_made_again` |
| 9, 34 | M/H | Live and offline renders shared one board | `compile_for_render` attaches boards of their own (`FrozenStore::attach(.., own_boards)`) | engine `an_offline_render_and_live_playback_read_one_render_with_their_own_cursors` (structural: one stream, two boards) |
| 10 | M | The modulator trace read renders in real time | `trace` sets `set_offline(true)` | engine `a_follower_of_a_frozen_strip_bakes_what_it_heard_live` (equal; the old failure depended on timing — not reproducible on demand) |
| 11 | M | A nested strip's refresh rendered silence | `prepare` thaws every frozen target whose region the target is not inside | engine `a_nested_frozen_strip_renders_its_own_sound_while_the_outer_one_plays` |
| 12, 36, 54 | M | Stale voices and effect state on thaw; notes fired into frozen tracks at a loop | Track slots reset when they start or stop resting/playing a render; resting strips' inserts and EQ reset; `fire_due_now` never plays a frozen or resting track | engine `thawing_while_playing_leaves_no_voice_and_no_tail_from_before` (thaw mid-song in a running processor; a looped frozen track) |
| 13, 16, 53 | L/M | The pure → mixed bus did not wait as the ADR says; the chip's figure was guessed | A strip of monitored input only raises a mixed destination's arrival and waits on its line; `monitored_wait` follows the planned delays (`Arrivals`) | ll `the_monitored_wait_is_measured_through_the_buses_it_goes_by` (impulse heard = figure, pure and mixed bus) |
| 14, 37 | L/M | Renders never let go; boards pinned blocks | A node that lets a render go clears its board (audio thread); a vanished slot drops its handle while the old snapshot holds it; live `attach` lets go of unused renders, `let_go_all` when nothing is frozen | engine `renders_no_longer_used_are_let_go`; rt_memory as before |
| 15 | M | An insert back from a hard bypass replayed old audio | `monitor_plan` resets an insert (and the master's) whose `ll_off` turns off | ll `an_insert_back_from_a_hard_bypass_holds_nothing_from_before` |
| 17 | M | Armed instrument tracks were not monitored | An armed track with no audio input is monitored (`live_install` carries its arming) | ll `an_armed_instrument_track_is_monitored_with_low_latency` |
| 18 | L | Synced inserts after a bypassed one dated by its latency | `scope_delays` skips `ll_off` latency (strips and master); `Processor::effect_late` | ll `an_insert_after_a_hard_bypassed_one_is_dated_without_it` |
| 19 | L | The bypass list cut at 8 silently | `LowLatency::off_total`; the UI says "and N more" (`freeze.ll.more`) | ll `every_bypass_is_counted_past_the_names_kept` |
| 20 | H | A range inside one clip was not cut at its end | `cuts` cut every sounding clip crossing either edge, at commit | ui `a_time_range_inside_one_clip_is_cut_at_both_edges_…` |
| 21, 40 | H | A bounce committed stale work | The bus's revision checked at commit: changed → the WAV goes and the bounce is made again; the cuts computed at commit | ui `an_edit_made_while_a_bounce_renders_is_bounced_again` |
| 22 | H | Solo rendered into Inserts/Strip bounces | Bounces render unsoloed (`freeze::unsolo`) | ui `a_solo_elsewhere_is_not_in_a_bounce_…` |
| 23, 49 | H/L | Automation and modulators of the original not carried; removed with it | Level/balance (source) and fader/pan/width/send (inserts, strip) rides and modulator routes copied to the new track or strip; removing moves the original's other automation and modulators to the new track; an original whose notes something hears is muted, not removed | ui `a_bounce_carries_the_rides_of_what_it_replaces` (to the bit, with *remove*) |
| 24 | M | Muting the original silenced keys | A whole strip: its keys and followers move to the new strip; with the inserts: refused when the strip keys or is followed, unless the track is all it gathers (then they move) | ui `a_strip_bounced_whole_hands_its_key_over` |
| 25 | M | A soloed original left its bounce silent | The solo moves to the new track/strip | ui `a_solo_elsewhere_…_hands_its_solo_over` |
| 26 | M | Ctrl+Alt+B ignored a time selection made after a clip click | The time selection wins when it was chosen last | ui `the_bounce_keys_take_the_time_selection_made_last_…` |
| 27 | M | The same bounce queued twice | `queue` skips the running want; a muted track/strip is refused (already bounced) | ui `a_bounce_asked_twice_while_it_renders_is_made_once` |
| 28 | M | Notes held across a range's start were lost | The range's edges move out to the notes held across them (a fixed point) | ui `a_time_range_takes_in_the_notes_held_across_its_edges` |
| 29 | M | Key/note-source tracks on the same strip leaked into an Inserts bounce | Those tracks' and strips' routes into the strip are taken away in the render's project | ui `a_track_that_shares_the_strip_for_its_notes_does_not_leak_into_a_bounce` |
| 30 | L | Source region ignored followed audio | `freeze::region(Track)` for the source tap | covered by core #2's test of the region |
| 31, 58 | L | Only the first track's selected clips bounced | One plan per track, queued | ui `the_bounce_keys_…_every_tracks_clips` |
| 32, 57 | L | *Remove* deleted muted clips the render skipped | `replaced` takes sounding clips only | ui `removing_the_original_of_a_time_range_keeps_a_muted_clip_…` |
| 33, 45 | L/M | Every hosted plug-in instantiated for one render | `freeze::units_for`: only what the render's snapshot runs (not resting nodes, not chains a render replaces, the master only when heard) | ui `a_render_that_needs_a_plugin_not_loaded_…` (a missing plug-in on a strip the render does not run is not needed) |
| 35 | M | An offline start before a block boundary stalled 10 s | The cursor points at the block read; an offline wait points at it again | engine `an_offline_render_starting_just_before_a_block_boundary_reads_its_block` |
| 38 | L | A failed render stayed "at hand" | `find` treats a failed stream as missing (deleted, never reopened while the file is that one); a frozen render's stream fails quietly (no "damaged sample") | engine `a_damaged_render_is_missing_and_made_again` |
| 39 | H | A job outlived its project | `Job::epoch`; another project drops the queue and the running job (a bounce's WAV deleted) | ui `a_render_asked_in_one_project_is_never_laid_into_another` |
| 41 | H | A refresh without a loaded plug-in wrote a wrong render | `units_for` refuses (named, translated) when a plug-in the render runs has no unit | ui `a_render_that_needs_a_plugin_not_loaded_is_refused_…` |
| 42 | M | Renders blocked the window waiting for loads | `start_next` waits while files, pitch or warp renders load | the suite's `settle` (needs a frame's `sync_engine` now) |
| 43 | M | Stop could not stop a refresh; a refresh blocked export | Stopped refreshes not made again until a freeze/unfreeze; an export stops a running refresh (`yield_refresh`), made again after | ui `stop_stops_a_background_refresh_…` |
| 44 | M | A bake and a freeze destroyed each other's instances | `start_next` waits for a bake; a bake waits for a render (`freeze.bake_waits`) | ui `a_modulator_bake_waits_for_a_freeze_rendering` |
| 46 | M | The first compile looked in the old project's folder | `follow_path` at the top of `sync_engine` | ui `the_renders_folder_follows_the_project_before_its_first_compile` |
| 47 | M | Alt+Shift+F froze the last track instead of the chosen bus | The key acts on the inspector's subject (what was chosen last) | ui `the_freeze_key_acts_on_what_was_chosen_last` |
| 48 | L | Partial files never cleaned; renders never pruned | `tidy` (`*.part*` over a minute old) at start and on a folder change; `prune` keeps Cantus's cache to 4 GB; the bounce WAV written under `.wav.part` and renamed; quitting stops the render (`freeze::close`); `FrozenStore::files` kept for tests only | ui `half_written_files_are_tidied_and_the_cache_kept_to_its_size` |
| 50 | L | A freeze dropped at commit said nothing; errors untranslated | `commit_freeze` says why; `freeze.gone`, `freeze.no_folder`, `bounce.gone` in both languages | ui `a_freeze_that_cannot_land_says_why` |
| 51 | L | 100 % shown while the tail renders | The total counts the tail at its longest, cut to what was done | progress as before (engine render tests) |
| 52 | H | After *Freeze track*, the header's M/S were refused | `Region::own` + `Touch::TrackAudibility`: a frozen strip's only input keeps mute and solo live (`freeze::silenced`); the manual, card and status say what stays live | core `a_frozen_strips_only_track_keeps_its_mute_and_solo_live`; ui `freeze_track_leaves_the_tracks_mute_and_solo_live` |
| 55 | M | The schema test could not catch a stale key | Compares the fixture's stored keys (raw JSON) with the content keys — it caught subject 2; the fixture regenerated | project `schema_37_keeps_frozen_strips_and_tracks` |
| 56 | M | No latent or time-range bounce test | Added (latent: to the bit; a range: within −100 dB from source, −60 dB through the inserts — a note's tail before the cut and the bounce after it are summed apart) | ui `a_bounce_through_a_latent_chain_…`, `a_time_range_inside_one_clip_…` |
| 59 | L | The modulator test did not test a wake | A live follower of a non-root region strip wakes it and what it gathers | core `a_live_follower_of_a_region_strip_wakes_it_and_what_it_gathers` (the old code passed it too: it covers, it did not fail) |
| 60 | L | Shots hid the strip menu's Freeze entries and the snowflake | The menus were held to egui's 400-point default area (`Area::default_size` now: the mixer menu, the route menu, the modulator popup); shots of the heads and of the whole strip menu | `freeze-mixer-*`, `freeze-strip-menu-*` |
| 61 | L | Spanish said *monitoreo*; the control is *Escucha* | The section and its strings use *Escucha* | `every_string_is_in_both_languages` |

### The run after review 1

Merged with `r29/integration` at 3c9659c (tempo's and isolation's review fixes), all
`--no-fail-fast`, 0 failed: `cantus-core` 354, `cantus-project` 85, `cantus-engine` 767, the
engine fuzz (`CANTUS_WORKERS=3`) 771, `cantus-host` 76, `cantus-sandbox` 43
(`CANTUS_TEST_PLUGINS_PREBUILT=1`), `cantus-ui` 1 394. Clippy `-D warnings` on the workspace
and `fmt --check` clean. Subject 2 (`SUBJECT_VERSION`), `fixtures/v37.cantus` regenerated;
schema still 37; no golden moved, `DSP_VERSION` unchanged. `shoot_freeze` adds
`freeze-strip-menu-*` (the whole strip menu, its Freeze band) and shoots the mixer's heads
without a menu over them.

Files outside my area touched by review 1: `cantus-project/src/collect.rs` (`undo`),
`cantus-engine/src/inst/stream.rs` (a frozen render's stream fails quietly),
`cantus-engine/src/modulator.rs` (`trace` offline), `cantus-engine/src/processor_rec.rs`
(an armed track with no input carries its arming), `cantus-ui/src/{audio_files.rs,
recovery_session.rs, views/samples_missing.rs, views/modulator_bake.rs, views/export.rs,
views/mixer_menu.rs, views/route_menu.rs, views/modulator_popup.rs}`.

Not run here: the Linux check (`tools/wsl/check.sh`); nothing new sits behind
`#[cfg(windows)]`.

## Review 2 — the fixes (58 confirmed: 3 high, 19 medium, 36 low)

`F:\projects\daw-r29\reviews\freeze-2.json`, merged with `r29/integration` at 07dfd8f first.
As in review 1, every test was seen failing with its fix taken out (the old code, or the
fix sabotaged in place, the test run, the source restored), but where a row says
otherwise. *core*, *engine*, *ll*, *ui*, *boot*, *sandbox*, *project* name
`cantus-core/tests/freeze.rs`, `cantus-engine/tests/{freeze,low_latency}.rs`,
`cantus-ui/tests/{freeze,freeze_boot}.rs`, `cantus-sandbox/tests/freeze.rs` and
`cantus-project`'s tests. The numbers are the review's indices.

| # | Sev | Finding | Fix | Test |
|---|---|---|---|---|
| 0 | H | A locked plug-in's own window changed it silently | `PluginHost::refused_by_freeze` (effects) / `instrument_refused_by_freeze`: the project's state and values given back to the plug-in, `HostNotice::Frozen` → the refusal card; renders from the stored states (`render_units_stored`, `render_instruments_stored`); a save never writes a window's state over a frozen region's (`project_for_save`, `instruments_for_save`) | sandbox `a_window_change_a_freeze_refuses_is_given_back_and_said` (the notice, the value given back, the save) |
| 1 | H | Missing files baked as silence under the true key | A freeze is refused naming the file (`freeze.file_missing`); a refresh waits for it, never marked failed; `prepare` refuses a missing file (`missing_file`); a file back on disk is read again (`SampleCache::has_failed`) | ui `a_missing_file_refuses_a_freeze_and_a_refresh_waits_for_it`; engine `a_missing_file_makes_no_render` |
| 2 | M | A follower of a bus the frozen strip plays into missed `hears_root` | `hears_root` when a listened strip is reached by the root (`reaches_strip`) | core `a_follower_of_a_bus_the_frozen_strip_plays_into_locks_its_fader` |
| 3 | M | An armed track rendered twice | Keyed and rendered as it will be frozen: `as_frozen` applies the disarm first | ui `freezing_an_armed_track_renders_once_under_the_key_it_keeps` |
| 4 | M | A nested target refreshed while resting, then marked failed | `scan` skips a target resting inside a playing render (refreshed when it wakes) | ui `a_strip_resting_inside_a_playing_render_is_not_refreshed_until_it_wakes` |
| 5 | M | A frozen strip's only track could not be moved | `MoveTrack` applied and checked (global); the subject's tracks grouped by strip id, in order only within one (`SUBJECT_VERSION` 3) | core `a_frozen_strips_only_track_can_be_moved_among_the_others` |
| 6, 9, 15 | M/L | Live solos silenced region inputs the render still had | `freeze::solo_play` → `SoloPlay::{Render, Silenced, Live}`: a solo that silences part of a region plays it live (`Snapshot::compile`, `FrozenStore::plays`); a soloed track its own follower hears cannot be frozen | core `a_solo_that_silences_part_of_a_region_plays_it_live`, `a_frozen_track_heard_by_its_own_follower_cannot_be_frozen_soloed`; engine `a_solo_that_silences_a_key_plays_the_frozen_bus_live` |
| 7, 42 | L | A plug-in turned while it rendered was set back, or its render parked apart | `commit_freeze` reads the hosted states again when the render lands; changed → rendered again | none of its own: it needs a plug-in window turned during a render, which no test can drive; the comparison is the content key's own |
| 8 | L | The pan law missing from a heard root's key | In the key when `hears_root` or `whole` | core `the_pan_law_is_in_the_key_of_a_frozen_strip_heard_by_its_own_follower` |
| 10, 11 | L | *Save as* stranded renders; the project's folder never pruned | `freeze::saved` after every save: the frozen targets' renders copied beside it; every other `.cfrz` there removed but those the undo or redo history wants (`freeze::keys_in`, `CommandBus::redo_history`), at the usual rates | ui `saving_elsewhere_brings_the_renders_and_keeps_only_those_wanted` |
| 12, 43, 55 | L | Bounce partials never tidied; the prune test blind | `tidy_bounces` on the project's `Audio` folder (when its path is taken) and the recordings' scratch folders; the prune test's renders dated apart | ui `half_written_bounce_files_are_tidied` (the function, and the session's call), `half_written_files_are_tidied_and_the_cache_kept_to_its_size` |
| 13, 29, 49 | L/M | A strip bounce mishandled solos inside and its new track's place | Refused while a track or strip inside is soloed; the new track takes only the strip's own solo; placed after the first track whose sound reaches the strip | ui `a_strip_bounce_refuses_a_solo_inside_it` |
| 14 | L | A loop's wrap is linear in a render | By decision, documented (ADR 0064 §4): the straight pass plays what led into the loop's start; the other DAWs' freezes share it | — |
| 16, 35 | M | A passing read error failed a render for good and deleted its file | A frozen render's `Io` fault is read again first, the file kept; only a damaged or vanished file fails it; `find` never deletes; a failed render asks for a new snapshot at once (`take_lost`) | engine `a_passing_read_error_keeps_a_render_and_its_file` (`inject_io_fault`) |
| 17, 37, 52 | L/M | A frozen root's EQ and a resting strip's inserts kept stale state | Inserts and EQ reset when a strip starts or stops playing a render or resting | engine `thawing_a_strip_while_playing_resets_its_eq_and_a_resting_strips_echoes` |
| 18, 22, 36 | L/M | Hard-bypass flags kept by position | The flag goes with its unit (`rebuild_chain`, `install_master`) | ll `an_insert_after_a_hard_bypassed_one_keeps_its_tail_when_the_chain_is_edited` |
| 19, 23 | L | Pure strips' arrival left out an instrument's latency and a second pure hop | A monitored track raises its pure strip's arrival; a pure strip raises the next one's (its own line still wants nothing) | ll `a_voice_two_buses_down_waits_with_the_backing_it_is_mixed_with`; the instrument case has no native latent instrument to build it with (the code path is the same `max`) |
| 20 | L | A resting frozen node kept its reader board | Its board released when it starts resting | no probe of its own (the reader's boards are not visible from a test); the engine freeze tests run through it |
| 21 | L | An off send and a note source woke a region | A send that carries nothing is no way out; a note source runs for its notes only (`Dormant::notes`, `TrackSnapshot::notes_only`: events to the listeners, no instrument, no output) | core `a_send_that_carries_nothing_and_a_note_source_do_not_wake_a_region`; engine `a_note_source_resting_but_for_its_notes_keeps_the_mix` |
| 24 | L | The chip counted the input's wait for MIDI | `LowLatency::audio`; `monitored_figure` | ui `the_chip_counts_the_input_only_for_an_audio_input` |
| 25 | L | The take's latency from the previous callback, and only once | `RecordMark::Started { tick, plugin, queue }` carries the plan's figure; a heard-latency change mid-pass moves what follows in the file (`Live::shift`: frames dropped or silence written) | ll `a_take_follows_the_backing_when_its_heard_latency_moves_mid_pass` |
| 26 | H | Bounced rides and keys acted early by the path's latency | The bounce keeps its latency: `TapOptions::bounce` untrimmed, padded to what the live plan waits (`Meters::{strip_late, track_late}`), the new track reporting it (`Track::lateness`, **schema 38**, fixture v38) | ui `a_bounce_through_latency_keeps_its_rides_and_keys_in_time` (Inserts and Strip, to the bit); project `schema_38_keeps_a_tracks_lateness_and_its_keys_as_written`. Source through a latent instrument: no native one to build it with |
| 27 | M | A range bounce muted clips whose notes were heard | Refused (`bounce.notes_heard`); the whole track can be | ui `a_range_bounce_refuses_clips_whose_notes_are_heard` |
| 28 | M | A range's tail rendered with the transport stopped | The render's project cut at the range's edges, the song playing on | ui `a_time_range_inside_one_clip_is_cut_at_both_edges_and_sounds_the_same` (now −120 dB through the inserts too) |
| 30 | L | A bounce started at its range (a free LFO out of phase) | Played from tick 0, the frames before dropped (`TapOptions::from`) | ui `a_bounce_of_a_clip_late_in_the_song_plays_a_free_lfo_as_the_song_does` |
| 31 | L | A live input bounced | Refused (`bounce.live_input`), a track's and a strip's | ui `a_bounce_refuses_a_live_input` |
| 32, 46, 57 | L | Any edit anywhere restarted a bounce | `freeze::region_key` of what it renders, compared when it lands | ui `an_edit_elsewhere_while_a_bounce_renders_does_not_make_it_again` |
| 33 | L | "Kept" overwritten by "done" | `bounce.done_kept` | ui `removing_an_original_whose_notes_are_heard_says_why_with_the_result` |
| 34, 50 | L | The Spanish source-only entry cut short | "Pasar a audio (solo la fuente)" | `freeze-menu-*-es-*` shots, looked at |
| 38 | M | The boot path loaded dormant plug-ins once | `Files::project_path`, `CantusApp::from_parts_at`: the path and the folders before the first compile | boot `a_frozen_project_opened_at_start_finds_its_renders_at_once` |
| 39 | M | The keys bounced a chosen track's first clip only | `SelKind::Track`: a header chosen last bounces the whole track | ui `the_bounce_keys_take_a_chosen_track_whole` |
| 40 | M | The keys ignored a chosen strip; on the master they froze a track | A chosen strip bounces whole; the master is never frozen nor bounced (`freeze.master`) | ui `the_bounce_keys_take_a_chosen_strip`, `the_freeze_and_bounce_keys_never_take_the_master` |
| 41 | M | Stop while its file was written was ignored | The writers stop within a block; `collect` commits nothing once Stop was pressed | ui `stop_pressed_while_its_file_is_written_commits_nothing`; engine `a_cancelled_write_leaves_no_render` (passes with either of the two checks: each holds the contract alone) |
| 44 | L | A refresh could start while closing | `FreezeState::closed` | ui `nothing_starts_once_the_session_closes` |
| 45 | L | A bypassed missing plug-in refused a freeze | Bypassed slots and strips with inserts off are not required | ui `a_bypassed_missing_plugin_does_not_stop_a_freeze` |
| 47 | L | A queued render was invisible | The card says what waits, offers Stop, "N more waiting" | ui `a_freeze_waiting_behind_a_bake_shows_its_card_and_can_be_stopped` |
| 48 | M | The snowflake over the strip number | The number laid out beside it (`strip_mark_room`) | `freeze-mixer-*`, `freeze-strip-menu-*` shots, looked at |
| 51 | L | The words said a locked fader stayed live | `live_words`: `freeze.done_heard`, `freeze.done_track` and the card's variants | ui `the_words_say_what_stays_live` |
| 53 | L | The ride test missed the original's own lanes | Rides on its own lanes and modulators on its strip and inserts, with *remove* on and off | ui `rides_and_modulators_on_the_original_go_with_its_bounce` |
| 54 | L | −60 dB accepted for a range through the inserts | The stopped transport (#28) was the cause: now −120 dB, as from the source | ui (as #28) |
| 56 | L | The damaged-render tests could pass blind | They assert the render opened and failed; the session queues it again | engine `a_damaged_render_is_missing_and_made_again`; ui `a_damaged_render_is_made_again_in_the_session` |

### The run after review 2

Merged with `r29/integration` at 07dfd8f, all `--no-fail-fast`, 0 failed: `cantus-core`
380, `cantus-project` 92, `cantus-engine` 791, the engine fuzz (`CANTUS_WORKERS=3`) 795,
`cantus-host` 82, `cantus-sandbox` 55 (`CANTUS_TEST_PLUGINS_PREBUILT=1`), `cantus-ui` 1 464.
Clippy `-D warnings` on the workspace and `fmt --check` clean. Schema 38
(`fixtures/v38.cantus`), `SUBJECT_VERSION` 3; no golden moved, `DSP_VERSION` unchanged.
`TapOptions::{trim, stop_at_end}` removed (no caller set them once the bounce kept its
latency and played on). `shoot_freeze` (100 PNGs in `F:\projects\daw-r29\shots\freeze\`)
now also freezes strip 12: the one- and two-digit numbers sit beside their snowflakes at
75, 100 and 200 % (#48), and the Spanish track menu shows *Pasar a audio (solo la fuente)*
whole (#34, #50).

How the tests were seen failing: the core, engine and low-latency ones against the code
before the fix (or, where the API is new, with the fix sabotaged in place: core
`solo_play` back to *silenced*, an off send counted, a note source woken whole — each
failing its own assertion); the interface's in three sabotage runs of their fixes, each
test failing alone (the refusal and key tests were split for that, so no assertion hides
behind another; the edit-elsewhere test now watches every frame, as the second render's
*done* hid the first's *again*). `freezing_an_armed_track_…` hung, not failed, with its
fix taken out in the batch run (a render made again forever) — it was seen failing on the
old code before; the stop-while-writing test's wait is now bounded.

A mistake to own: while stopping a sabotage run that had gone wrong mid-review, I ran
`taskkill //F //IM cargo.exe` (and the freeze test binaries by image name), which may have
ended another builder's cargo at that moment; later runs were stopped through their own
tasks only.

## Review 3 — the fixes (55 confirmed: 10 high, 25 medium, 20 low)

`F:\projects\daw-r29\reviews\freeze-3.json`, merged with `r29/integration` at dd71793 first
(warp's second review, schema 39 `WarpMarker::auto`; mine stays 38). Many findings are one
defect seen through several lenses (the lateness seven times, the shared folder six, solo six): the row says *as #n*. Every test was seen failing with its fix taken out — the fix
sabotaged in place, the tests run, the source restored (`sab3_*.py`, results below the
table) — but where a row says otherwise. *core*, *engine*, *ll*, *ui*, *rec*, *recovery*,
*sandbox*, *project* name `cantus-core/tests/freeze.rs`,
`cantus-engine/tests/{freeze,low_latency}.rs`, `cantus-ui/tests/{freeze,rec_pass,
recovery_flows}.rs`, `cantus-sandbox/tests/freeze.rs` and `cantus-project`'s tests.

**The lateness, as the integrator directed** (#1, #8, #19, #24, #34, #40, #48): the audio
on the grid, the delay on the track. The bounce's file is trimmed to the grid
(`TapOptions::bounce`); `Track::lateness` (schema 38 kept, reworked in place — the field
and the fixtures v38/v39 are unchanged, only what the engine does with it) is now a real
delay line before the track's level (`pdc::Edge::Source`, `TrackSlot::source_line`),
reported in the track's latency and compensated as any other, a monitored input joining
after it. So every path agrees: the bounced clip moved, copied, pasted, linked or exported
(DAWproject has no delay) plays on the grid (tested: moved, copied and pasted, through
DAWproject; a linked copy is the same audio); a take recorded, imported or drawn on the
bounced track is delayed with everything the track holds, and compensated; a track stem is
trimmed by its track's latency. The inspector shows the delay with *Clear*
(`SetTrackLateness`, locked as the track's source).

| # | Sev | Finding | Fix | Test |
|---|---|---|---|---|
| 0 | H | Saving pruned the renders of every project in the same folder | A folder of renders per project, `Freeze/<file stem>` (`project_dir`); the old shared folder is looked in, never pruned (`shared_dir`) | ui `saving_one_project_keeps_the_renders_of_another_beside_it`, `the_renders_folder_follows_the_project_before_its_first_compile` |
| 1 | H | `Track::lateness` on the track, the late frames in the clip's file | The file trimmed to the grid, the delay a source line before the track's level (see above) | engine `a_track_delay_plays_its_source_late_and_the_mix_on_the_grid`; ui `a_bounced_clip_moved_to_a_track_of_its_own_plays_on_the_grid`, `a_bounced_clip_copied_and_pasted_on_a_plain_track_plays_on_the_grid`, `a_clip_put_on_a_bounced_track_plays_on_the_grid`, `a_bounced_track_exported_to_dawproject_plays_on_the_grid_elsewhere`, `a_bounce_through_a_latent_chain_keeps_the_song_to_the_bit`, `a_bounce_through_latency_keeps_its_rides_and_keys_in_time` |
| 2 | H | The commit compared hosted states' bytes: a freeze of an automated plug-in never landed | The host counts edits from windows and browsers (`Hosted::edits`, `HostedInst::edits`, `PluginHost::edits_of`); `commit_freeze` compares the count at start and at landing | ui `a_freeze_lands_though_a_plug_ins_state_moved_without_an_edit`; sandbox `edits_count_what_windows_do_never_what_automation_does` |
| 3 | M | Pruning missed undo inverses, a render landing, history renders on Save As | `keys_in` reads `InsertTrack`, `InsertStrip`, `ReplaceProject`, both sides of every step; `wanted_by_jobs`; every kept key copied | core `the_keys_an_undo_wants_include_removed_frozen_tracks_strips_and_projects`; ui `a_removed_frozen_track_saved_and_undone_finds_its_render` (as opened: the freeze not in the undo), `a_save_while_a_freeze_is_landing_keeps_its_render`, `saving_elsewhere_brings_the_renders_an_undo_wants_off_the_frame` |
| 4 | M | `solo_play` woke regions silent anyway or only lending notes | Rewritten: the root asked first — silenced (not audible, or nothing audible bringing it sound) → `Silenced`; a frozen track not heard → `Render`; `Live` only when a track or strip whose sound reaches the render is silenced | core `an_unrelated_strip_solo_silences_a_frozen_bus_without_thawing_it`, `muting_a_keyed_frozen_strips_only_input_silences_it`, `a_note_source_silenced_by_a_solo_does_not_thaw_a_frozen_strip`, `a_frozen_track_silenced_by_a_solo_plays_its_render`; sandbox `a_solo_elsewhere_toggled_never_loads_a_frozen_strips_plug_in` |
| 5 | M | A state of its own in a locked plug-in (no value moved) neither refused nor given back | `capture_changes` / `capture_instrument_changes` probe the lock: refused, given back, said; counted as an edit where nothing locks it; `load_for` / `load_instrument` load the project's state when locked, never a parked one | sandbox `a_state_of_its_own_in_a_locked_plug_in_is_given_back_and_said`, `a_locked_instruments_own_state_is_given_back_never_saved_never_rendered`, `a_locked_plug_in_comes_back_as_the_project_holds_it_not_as_it_was_parked`, `a_locked_instrument_comes_back_as_the_project_holds_it_not_as_it_was_parked` |
| 6 | L | `missing_file` counted lanes that do not play | Only lane-0 unmuted clips and unmuted pads; `freeze.waits_for_file` said once | ui `a_take_on_a_lane_below_with_its_file_gone_does_not_stop_a_freeze` |
| 7 | L | A wire for a bypassed missing plug-in kept under the true key after it was installed | The render notes its wires beside it (`<render>.wires`, the uninstalled plug-ins only); a scan makes it again once one is installed (`wired_since_installed`, `FrozenStore::forget`) | ui `a_render_made_with_a_wire_for_a_bypassed_plug_in_is_made_again_once_it_is_installed` |
| 8 | H | As #1 (engine lens) | As #1 | As #1 |
| 9 | M | As #4 (engine lens: own track muted on a keyed strip, unrelated solos) | As #4 | As #4 |
| 10 | M | A heard-latency shift undone at the next loop wrap | The wrap carries the moves made so far (`RecordMark::Wrapped { shift }`, `FinishedTake::wrap_shifts`); passes placed by them (`record_pass`) | ll `a_loop_pass_after_a_heard_latency_move_lands_where_it_was_played`; rec `a_loop_pass_after_a_heard_latency_move_is_placed_by_it` |
| 11 | M | One latency for a render whose region's latency moves | The render watches the tap's latency and the unserved lines: moved → `RenderError::LatencyMoved` (`freeze.latency_moves`); `serve` answers PDC and effect needs during the render | engine `a_latency_that_moves_during_the_song_is_no_render` |
| 12 | M | As #0 (engine lens) | As #0 | As #0 |
| 13 | M | No engine-side missing-file check for a bounce | `missing_file(&snap)` at the bounce's render start; the compile waits for background loads (#43) | ui `a_bounce_with_a_file_there_but_unreadable_is_refused` |
| 14 | M | Track stems trimmed by nothing | Track stems trimmed by `track_latency(i)`; Direct stems summed after the lines (`StemTap::direct`) and trimmed by `master_arrival()` | engine `track_and_direct_stems_are_on_the_grid_with_a_kept_delay` (Track and Direct each sabotaged alone) |
| 15 | L | Back from Live to the render dropped out | A live target keeps its render attached (`FrozenPlay::live`, `plays()` false); the slots' `parked` keep its board at the playhead | core `a_target_played_live_keeps_its_render_in_the_snapshot`; engine `a_solo_that_thaws_a_frozen_strip_while_it_plays_drops_nothing_when_lifted` |
| 16 | L | One immediate retry of a read error, then failed for the session | `FROZEN_RETRIES` 0/50/200/1 000 ms; failed by io, the next scan looks again (`take_io_failed`), never makes it again; per-path fault injection | engine `a_read_error_that_outlasts_its_first_try_keeps_the_render_playing`, `a_read_error_that_outlasts_every_try_is_looked_at_again_not_made_again` |
| 17 | L | A notes-only frozen track kept its board pinned | Notes-only releases the board (`processor_snap`: `stops` counts `notes_only`) | engine `a_frozen_track_that_runs_for_its_notes_only_lets_its_blocks_go` |
| 18 | M | As #10 (low-latency lens) | As #10 | As #10; recovery `a_recovered_loop_pass_is_laid_by_the_wraps_the_engine_marked` |
| 19 | M | As #1 (low-latency lens) | As #1 | As #1 |
| 20 | L | The shift applied a round trip too early | The device round trip reaches the engine (`EngineMessage::SetDeviceRoundTrip`, sent from `sync_devices`); moves queue until the input that carries them (`due`, `take_due`) | ll `a_heard_latency_move_moves_a_take_only_once_the_musician_hears_it` |
| 21 | L | The recovery journal placed a pass by the stats' latency | The journal keeps the engine's marked figures (`JournalPass::marked_wraps`, the start's latency; `journal_tick` writes them as they change); recovery uses them | recovery `the_journal_keeps_the_figures_the_engine_marked_for_the_pass`, `a_recovered_loop_pass_is_laid_by_the_wraps_the_engine_marked` |
| 22 | L | The chip added the input's wait to the notes' way | `LowLatency::monitored_note_samples`; `monitored_figure` takes the slower of the audio way and the notes' way | ll `the_monitored_figures_of_an_input_and_of_notes_are_apart`; ui `the_chip_counts_the_input_only_for_an_audio_input` |
| 23 | L | A bounce's kept latency depended on the monitoring mode | The arrival is planned on a planning processor of its own (`strip_arrival`), never the live one: no monitoring mode in it | By construction (#25's planning); not seen failing — no code path left to sabotage |
| 24 | H | As #1 (bounce lens) | As #1 | As #1 |
| 25 | M | An Inserts bounce added the compensation after the chain, from live meters | The arrival planned on the whole song before the render (`freeze::render::strip_arrival`: hosted latencies from `PluginHost::latencies`, `EffectUnit::latency_stand_in`, instrument hints) and waited **before** the chain (`TapOptions::arrive`, `Processor::set_arrive_floor`); `Meters::{strip_late, track_late}` and `write_wav_padded` removed | ui `a_bounce_through_inserts_waits_for_a_latent_co_input_before_the_chain` |
| 26 | M | As #0 and #3 (bounce lens) | As #0, #3 | As #0, #3 |
| 27 | M | As #4 (each toggle reloading plug-ins) | As #4 | As #4 (sandbox) |
| 28 | L | Carried and rescued modulators lost what drives their amount | `amounts`: the `ModAmount` lanes copied onto the new host, the modulators driving them copied too; rescue skips moving lanes of rescued modulators twice | ui `what_drives_a_carried_modulators_amount_drives_its_copy` |
| 29 | L | A clips or time-range bounce of a comp window lost its seam crossfade | Clips: one window of a seam pair without the other is refused (`seam_refusal`, `bounce.seam`); a time range's edges move out past the crossfades, iterated with the held notes to a fixed point (`seam_edges`) | ui `a_bounce_never_cuts_a_comp_seams_crossfade` (both cases) |
| 30 | L | The live-input refusal looked only at tracks reaching the strip | `live_region_refusal` (key and note sources of a track bounce) and `strip_plan`'s check over every track of the region | ui `a_live_key_source_refuses_a_bounce_through_the_inserts_and_of_the_strip` |
| 31 | H | `solo_play` thawed a strip the solo or mute had silenced | As #4: the root asked first | As #4 |
| 32 | M | As #15 (realtime lens; pattern mode too) | As #15; pattern mode holds the renders in use (`FrozenStore::hold`, `held` keys) | As #15; ui `renders_stay_open_while_pattern_mode_plays_every_chain_live` |
| 33 | M | As #0 and #3; Save As copied renders on the UI thread | As #0, #3; copies and prune on `cantus-freeze-save` (`copy_and_prune`, `.copy.part` renamed; `FreezeState::copying`); no folder made when nothing to copy (review 4 made it one thread at a time, its prune safe, the close waiting, the sources noted) | ui `saving_elsewhere_brings_the_renders_an_undo_wants` (review 4: and `saving_elsewhere_returns_before_its_copies_…`, which can tell) |
| 34 | M | As #1 (realtime lens) | As #1 | As #1 |
| 35 | L | As #20 (the move a round trip early) | As #20 | As #20 |
| 36 | L | A refused edit reloaded the whole state per value event | Give-backs deferred to the end of the frame, and while the plug-in's gesture is open to its end (`give_back`, `inst_give_back`, `give_back_now`) | sandbox `a_drag_a_freeze_refuses_gives_back_once_at_its_end` |
| 37 | L | `rt_memory` missed the new audio-thread paths | Phases: Render → Silenced → Live (render kept) → Render twice while playing; notes-only; rest with its handle; a kept delay; the mode switched and a latent insert removed while recording into rings (`SetDeviceRoundTrip`) | `rt_memory::{frozen_playback_never_allocates, low_latency_monitoring_never_allocates}`: an allocation put on each new path (the parked board, the source line, notes-only, a move falling due) was counted in its phase (`sab3_rt.py`) |
| 38 | H | As #0 (reach lens) | As #0 | As #0 |
| 39 | H | As #2 (reach lens) | As #2 | As #2 |
| 40 | M | As #1 (reach lens) | As #1 | As #1 |
| 41 | M | As #3 (a deleted frozen track's undo) | As #3 | As #3 |
| 42 | M | A strip bounce committed without asking its refusals again | `plan_refusal` asked again at the bounce's commit | ui `a_solo_set_while_a_strip_bounce_renders_is_refused_at_its_end` |
| 43 | M | A bounce's compile did not wait for a background reload | `SampleCache::wait_audio(patience)` in `compile_for_render` | engine `an_offline_compile_waits_for_the_audio_it_asked_for_in_the_background`; ui as #13 |
| 44 | M | As #4 (reach lens) | As #4 | As #4 |
| 45 | L | The clip menu's Bounce acted on a strip or the master chosen earlier | `clip_menu_opened` makes the clip the inspector's subject always | ui `the_clip_menus_bounce_takes_the_clip_right_clicked` (a bus with no track, so the clip stays selected — the review's case) |
| 46 | L | `Track::lateness` unbounded from the file | `Lateness::is_valid` (8–768 kHz, ≤ 10 s), `at()` 0 for an invalid one, the engine clamps to `pdc::MAX_DELAY`; `InsertTrack` and `SetTrackLateness` refuse one; `Project::repair_lateness` on load (`LoadReport::lateness_repaired`, `freeze.lateness_repaired`) | core `a_track_delay_a_bounce_could_not_have_kept_is_refused_and_repaired`; project `an_out_of_range_track_delay_is_dropped_on_load` |
| 47 | H | As #4 (the ADR said *Silenced*) | As #4; ADR §3 rewritten to match | As #4 |
| 48 | H | As #1 (tests lens) | As #1 | As #1 |
| 49 | M | As #0 and #3 (tests lens) | As #0, #3 | As #0, #3 |
| 50 | M | The padding came from live meters (zero without a device), untested | As #25 | As #25 (no device in the ui tests: the plan, not meters) |
| 51 | L | The heard words promised live sends; a frozen track got the strip's words | `live_words`: a heard strip's words name what is locked (`freeze.done_heard` and the card's, reworded); a heard track its own (`freeze.done_heard_track`, `freeze.card.refused_body_heard_track`) | ui `the_heard_words_name_what_is_locked_and_nothing_else` (each control the words call locked is refused, each called live is not) |
| 52 | L | The first save and Save As copied renders on the UI thread | As #33 | As #33 |
| 53 | L | Three review-2 fixes checked by assertions that pass without them | (1) the save and the render now told by the state's bytes and a tag no value covers (`render_states_for_test`, `load_state_for_test`, the instrument hooks); (2) notes-only measured by voices | sandbox `a_change_the_host_never_heard_of_is_neither_saved_nor_rendered`, `a_render_of_a_locked_insert_is_made_with_the_state_the_project_holds`, `a_locked_instruments_own_state_is_given_back_never_saved_never_rendered`; engine `a_note_source_resting_but_for_its_notes_plays_no_voice` |
| 54 | L | *N más esperan* with N = 1; *the conversion*; *insertos* | `freeze.card.more_one` ("One more waiting." / "Espera uno más."); English names the bake; *inserciones* | ui `one_render_waiting_is_said_in_good_spanish_and_english_names_the_bake` |

### How the tests were seen failing (review 3)

Each batch: the fixes taken out in place, the tests run `--no-fail-fast`, the sources
restored from git (never the tests).

- **core** (old `solo_play`, a live target's render dropped, the `keys_in` arms, the
  lateness checks, the guard's `SetTrackLateness` arm): 9 failed — every review-3 core
  test. Then one at a time: the commands' checks alone (`…refused_and_repaired`), the old
  `solo_play` alone (sandbox `a_solo_elsewhere_toggled_…`), the `InsertTrack` arm alone (ui
  `a_removed_frozen_track_…`, once the test opened the project as saved — the freeze in its
  undo had kept the render), `repair_lateness` alone (project `an_out_of_range_…`).
- **engine** (the source line not run, notes fired into a notes-only instrument, track stems
  untrimmed, the latency not watched, a live render dropped, one immediate retry and io
  failures taken for damage, notes-only keeping its board, one monitored figure, the move
  applied at once and not carried by the wrap, no wait for audio): all 12 review-3 engine
  and low-latency tests failed; Direct stems untrimmed alone failed the stems test; the
  source line alone failed ui `a_clip_put_on_a_bounced_track_plays_on_the_grid`.
- **ui** (the file untrimmed, renders not held, no arrival floor, no engine-side file check,
  no refusal at commit, no live-region or seam refusal, no live inputs over the region, no
  amounts, no seam edges, the shared folder, no job keys, no history keys, lanes counted,
  bytes compared at commit, no wires noted, the clip menu not following, the track's words,
  *more == 1*, the notes' figure, the journal's figures and wraps, the pass shift): 21 freeze
  tests, 2 recovery and 1 pass test failed. Three passed and were made to tell: the
  DAWproject test now compares with the song as it played before the bounce (both sides had
  the file late), the clip-menu test uses a bus with no track (a strip with one selects the
  track, and the menu then selected the clip anyway), and the put-take test is the source
  line's (above). A paste test was added for the clipboard's path.
- **host** (a parked state over the project's when locked, give-backs every frame, a state of
  its own unheard, edits never counted, the save writing a locked plug-in's state): 6
  sandbox tests failed — two only once they compared states' bytes and a tag (the values
  the host gives back on load hid the gain). The render with live states alone, the
  instrument's lock and give-back, its save skip and its stored render — each alone — failed
  the new render and instrument tests.
- **rt_memory** (#37): an allocation on the parked board, the source line, the notes-only
  path and a move falling due was counted in the phases that run them.
- The undo fuzz: its new kept-delay arm first shifted every later draw, and a rare comp case
  (the seam batch) stopped happening in all 60 seeds; the arm now draws from a generator of
  its own, and counts its applied and refused delays.

### The run after review 3

Merged with `r29/integration` at dd71793, all `--no-fail-fast`, 0 failed: `cantus-core`
416, `cantus-project` 112, `cantus-engine` 819, the engine fuzz (`CANTUS_WORKERS=3`) 823,
`cantus-host` 82, `cantus-sandbox` 64 (`CANTUS_TEST_PLUGINS_PREBUILT=1`), `cantus-ui` 1 573.
(One run of the engine suite had `pool::a_parked_pool_is_woken_once_per_worker_…` fail on
a 28 µs wake while two other builds compiled — a timing test of the pool, not touched
here; it passed alone and in the full run after.) Clippy `-D warnings` on the workspace and
`fmt --check` clean (after it: two type aliases and a `then_some`, and the *Delay* row's
value shortened to its figure — the freeze and sandbox tests run again). Schema 38 as it
was (the fixtures untouched), `SUBJECT_VERSION` 3; no golden moved, `DSP_VERSION`
unchanged. `shoot_freeze` (112 PNGs in `F:\projects\daw-r29\shots\freeze\`) now also
shows a bounced pad's inspector with its *Delay* row and *Clear*, and the inspector shots
are tall enough for the *Freeze* row (they stopped at *Recording*). The test folders of
earlier runs are swept at a run's first test (`cantus-freeze-ui`), and the last run's
removed.

## Review 4 — the fixes (26 confirmed: 2 high, 10 medium, 14 low)

`F:\projects\daw-r29\reviews\freeze-4.json`, merged with `r29/integration` at 01fddb7 first
(a fast-forward: isolation's `render_ready`, PROTOCOL 6, and warp's third round with its
`late` flag in `commit_freeze`). Every test was seen failing with its fix taken out
(`sab4_*.py`; below the table) but where a row says otherwise. The names are as in review
3's table, plus *inspector* and *final_look* (`cantus-ui/tests/{inspector,final_look}.rs`).

| # | Sev | Finding | Fix | Test |
|---|---|---|---|---|
| 0 | H | The save's thread pruned with its save's list, deleting renders written while it copied | The prune removes only renders the folder held when its save began (`listed`, on the frame) and none the session wants now (`FreezeState::protect`: each save's wanted set, and each render started, `render_job`); one save thread at a time, a later one queued behind it (`saver`, joined first); queued freezes kept by their `as_frozen` key | ui `a_freeze_landing_while_a_save_copies_keeps_its_render` (failing only with both defences out: each alone keeps it), `two_saves_in_a_row_keep_what_the_later_one_wants`, `a_save_keeps_the_render_a_waiting_freeze_of_an_armed_track_will_land_with` |
| 1 | M | Nothing waited on the copy: a quit right after a save killed it | `freeze::finish_saving` (a join, bounded at two minutes) in `close_session` after the save lands; the folder notes where its copies come from (`copying-from.txt`) until they are in, and the next session looks there | ui `closing_the_session_waits_for_the_renders_copy`, `a_session_cut_short_while_copying_finds_the_renders_where_they_were` |
| 2 | M | A renamed or duplicated project lost its renders (`Freeze/<stem>`) | The other projects' folders beside it are looked in, read-only (`also_dirs`); its next save copies what it wants into its own; an orphan folder (no project file of its name) loses what the own folder now holds, and goes when empty | ui `a_renamed_project_finds_its_renders_and_its_save_brings_them`, `a_copied_project_finds_its_renders_and_leaves_the_originals_alone` |
| 3 | M | The `.wires` note was not copied with its render | Copied with it (part file, renamed); pruned with it, and an orphan note removed; `scan` reads every copy's note (`FrozenStore::paths_of`) and sets every copy aside (`set_aside`), removing those in its own folders | ui `a_render_made_with_a_wire_and_saved_elsewhere_is_made_again_once_it_is_installed` |
| 4 | L | `exec_through` copied re-keyed renders on the frame | On the save's thread, under the new key *inside* (`cantus_engine::freeze::copy_as`, `stream_cache::relabel` — the plain copy kept the old key in its header and never opened, a defect beyond the finding); the old render plays under the new key meanwhile (`FrozenStore::alias`) | ui `a_take_moved_by_saving_keeps_its_render_under_its_new_key`, `saving_a_frozen_take_…` (waits for the copy); engine `a_render_copied_under_a_new_key_opens_under_it_and_its_alias_plays_meanwhile` (the relabel and the alias each sabotaged) |
| 5 | L | The engine's `missing_file` counted a muted pad, and skipped kits with layers | Pads one by one, then every pad's layers in `attach`'s order; a muted pad's files skipped | engine `a_muted_pads_missing_file_refuses_no_render`, `a_layers_missing_file_refuses_a_render` |
| 6 | L | A read error looked up by `path_of`, so a good copy was marked damaged | `FrozenHandle::path` (the file opened): the io check and the setting aside are that copy's; copies set aside one by one, the first good one opened | engine `a_read_error_on_one_copy_never_sets_a_good_copy_aside`, `a_damaged_copy_is_passed_over_for_a_good_one` |
| 7 | H | A locked plug-in that echoes a host load was given back and said every frame | `session::Echo` on every hosted insert and instrument: a report that is the project's state, what it held after the host's last load, what it said last, or its first after a give-back is the load's echo — no edit, nothing refused. The test plug-ins learnt a *dirty* tag (every load marks the state dirty and asks for a values rescan, as JUCE's VST3 wrapper) | sandbox `a_locked_plug_in_that_echoes_its_give_back_is_given_back_once` (in process and isolated), `a_locked_instrument_that_echoes_its_give_back_is_given_back_once`, `a_locked_plug_in_loaded_again_that_echoes_its_load_is_left_alone` (passes on the old host too: CLAP's instantiate drops the load's flags — kept as the guard) |
| 8 | M | As #2 | As #2 | As #2 |
| 9 | M | As #0 (overlapping saves) | As #0 | As #0 |
| 10 | M | As #1 | As #1 | As #1 |
| 11 | L | As #3 | As #3 | As #3 |
| 12 | L | Synthesized notices (a late params answer, a title change) counted as edits or refused | As #7: a report of nothing new is no edit and no refusal | sandbox `an_echo_of_the_hosts_own_load_is_no_edit` |
| 13 | L | The Save As test could not fail for what it claimed | A hook holds the save's thread (`FreezeState::hold_copies`): the frame returns without the copies, the render — let go of — is found where it was | ui `saving_elsewhere_returns_before_its_copies_and_the_old_folder_is_looked_in`; the old test now claims only what it checks (`saving_elsewhere_brings_the_renders_an_undo_wants`) |
| 14 | L | `commit_freeze`'s edit count had no interface test | The test, on a strip and on the master of a whole-mix region — which found a defect: `hosted_states` never wrote the master's plug-in states for a whole-mix freeze (now it does) | ui `a_state_of_its_own_changed_while_it_renders_renders_it_again` (the edit check out, and the master's states out, each fail it) |
| 15 | M | As #1 | As #1 | As #1 |
| 16 | M | As #3 | As #3 | As #3 |
| 17 | M | The manual said a frozen target's mix always stays live | Both manuals: unless a modulator inside follows it (or the bus it plays into), and the card says so | Docs (read against `Locks::of` and the strings) |
| 18 | M | The Delay row's tip too long for the hint bar | Short tips (EN 68, ES 77 characters with the figure); the key's own hint (`freeze.lateness.clear`) | final_look `the_delay_rows_hints_read_whole` (both languages, 1280 × 720 and 200 %) |
| 19 | L | A bounce whose latency moves said "Bouncing X stays live" | `bounce.latency_moves`, with the track's or strip's name; both manuals say the status line says it, for bounces too | ui `a_bounce_whose_latency_moves_is_said_as_a_bounce` |
| 20 | L | As #0 | As #0 | As #0 |
| 21 | L | As #2 (the old folder never searched nor cleaned) | As #2 (the orphan tidied) | As #2 |
| 22 | L | The Delay row allocated three strings a frame; no inspector test drew it | Its figure from the sheet's cache, its tip made only when pointed at; on a square key (`Sheet::readout_clear_row`, `glyph_key` with the close glyph) — the word button cut the figure at 220 pt, seen by the new collision run | inspector `no_two_texts_collide_…` (a bounced track in the scene: "96.0…" was cut), `the_delay_row_allocates_nothing_of_its_own` (9 more allocations with the old row) |
| 23 | L | "Espera uno más." read as an order | "Uno más en espera." and "{n} más en espera." | ui `one_render_waiting_is_said_in_good_spanish_and_english_names_the_bake` |
| 24 | L | The manual's clip menu sentence contradicted itself | Both manuals: the menu acts on clips, never a strip or the master chosen before — the clip right-clicked, with the clips selected with it | Docs (read against `clip_menu_opened` and `bounce_selection`) |
| 25 | L | ADR 0064's review-3 text did not match the code | `Freeze/<file stem>`; *meanwhile* and the copying flag rewritten with what the code does now; 9 more sandbox tests, not 10; review 3's table rows 0 and 33 corrected | Docs |

### How the tests were seen failing (review 4)

- **host**: the review-3 host sources put back (`git checkout HEAD~1 -- crates/cantus-host/src`):
  the three echo tests that can fail did (30 give-backs in 30 frames; one edit counted).
- **engine** (`sab4_eng.py`: a muted pad's file counted and layers skipped, the first copy
  found blamed, only the first copy tried, no alias, no relabel): all five failed; the
  relabel alone failed the new-key test too.
- **interface saves** (`sab4_ui.py` a: the prune over the folder's listing at its moment,
  no note of the sources, no sibling folders, no wires carried, the old plain copy on the
  frame without an alias, a queued freeze keyed as the project holds it, no wait at the
  close — 8 failed; b: the prune without the session's wants, orphans never tidied — 4
  failed). The freeze landing during a copy is kept by either defence, and failed with both
  out.
- **interface words and rows**: the old Spanish tip and *Espera uno más.*, and the bounce's
  words through the freeze's, each failed its test; the old Delay row failed both inspector
  tests; the edit check and the master's states, each taken out, failed the edit-count test.

### The run after review 4

RUN4_RESULTS

## Left

Nothing of mine (review 1: all 62 closed; review 2: all 58 closed — #14 by decision,
documented; review 3: all 55 closed — #23 by construction, said in its row; review 4: all
26 closed). Owner-only:
listen to a frozen heavy track and its unfreeze, and record a voice through Tonus with
low-latency monitoring on, with real hardware.

Not testable here, said in review 2's table (its numbers): a plug-in window turned *during*
a render (#7, #42 — review 3's edit count is tested by the host instead),
a native latent instrument for the Source-bounce and pure-strip cases (#19, #26), the
reader board released on rest (#20).

## Notes for the integrator

- **Schema**: mine is 36 → 37, and review 2 adds 37 → 38 (`Track::lateness`,
  `migrate_37_to_38` writes nothing, `fixtures/v38.cantus`,
  `schema_38_keeps_a_tracks_lateness_and_its_keys_as_written`). If another branch took 38
  meanwhile, renumber mine after it and regenerate the fixtures (the freeze block runs
  last in `write_fixture`, so the keys are of what the fixture holds). `SUBJECT_VERSION`
  is 3: every content key changed on purpose (old renders are made again in the
  background, as after a format change).
- **Files outside my area in review 2**: `cantus-host/src/{session.rs, session_inst_idle.rs}`
  (`refused_by_freeze`, `HostNotice::Frozen`, `render_*_stored`, the save skip);
  `cantus-engine/src/{inst/stream.rs, inst/stream_reader.rs (the test-only
  `inject_io_fault`), recorder.rs (`RecordMark::Started` fields), processor_rec.rs,
  samples.rs (`has_failed`), messages.rs (`Meters::{strip_late, track_late}`), lib.rs}`;
  `cantus-ui/src/{app.rs (`Files::project_path`, `from_parts_at`, `HostNotice::Frozen`,
  `SelKind::Track`), boot.rs, clipboard.rs, save_job.rs (`freeze::saved`)}`;
  `cantus-core/src/{bus.rs (`redo_history`), model.rs (`Track::lateness`, `Lateness`),
  snapshot.rs (`notes_only`, `lateness`), lib.rs}`; `cantus-engine/src/{graph/track.rs,
  graph/master.rs, processor.rs, processor_pass.rs, processor_snap.rs}`;
  `cantus-project/src/{format.rs, midi/import.rs}`, `examples/write_fixture.rs`;
  `cantus-ui/src/views/mixer_strip.rs`; `cantus-engine/tests/recording.rs` (the `Started`
  match).
- **Review 3, schema 38 reworked in place** (as directed): `Track::lateness` keeps its field
  and its JSON; it now means a delay before the track's level, the bounce's file trimmed to
  the grid. The fixtures v38 and v39 are unchanged (`write_fixture` rewrote v39 with only
  its timestamp different, so it was left as it was). A bounce made by an earlier round-29
  build (late file *and* kept delay) would now play late twice — no release had one.
- **Files outside my area in review 3**: `cantus-core/src/{command.rs (`SetTrackLateness`,
  the `InsertTrack` check), model.rs (`Lateness::is_valid`, `Project::repair_lateness`),
  snapshot.rs (`FrozenPlay::of`)}`; `cantus-project/src/{format.rs, store.rs}`
  (`lateness_repaired`); `cantus-engine/src/{pdc.rs (`Edge::Source`), graph/{track.rs,
  strip.rs} (`source_line`, `parked`), processor.rs, processor_{snap,pass,msg,monitor,
  rec}.rs, messages.rs (`Meters::{strip_late, track_late}` removed;
  `EngineMessage::SetDeviceRoundTrip`), recorder.rs (`RecordMark::Wrapped { shift }`,
  `FinishedTake::wrap_shifts`), devices.rs, effects.rs (`latency_stand_in`), render.rs
  (`RenderError::LatencyMoved`), stems.rs, samples.rs (`wait_audio`), backend.rs,
  inst/{stream.rs, stream_reader.rs}}`; `cantus-host/src/{session.rs, session_inst.rs,
  session_inst_idle.rs, session_iso.rs}` (edits, give-backs, `locked` loads, `latencies`,
  `installed`, test hooks); `cantus-ui/src/{app.rs, boot.rs, record_pass.rs, recovery.rs,
  recovery_restore.rs, recovery_session.rs (`JournalPass::marked_wraps`),
  views/{arrangement.rs (`clip_menu_opened`), inspector_record.rs, inspector_sheet.rs
  (`Sheet::text_button_row`)}}`; tests `cantus-ui/tests/{rec_e2e.rs, rec_flows.rs,
  rec_pass.rs, recovery_flows.rs, tempo_record.rs}` and `examples/shoot_recovery.rs` (the
  new fields), `cantus-engine/tests/rt_memory.rs` (phases),
  `cantus-core/tests/undo_invariants.rs` (the kept-delay arm). Another branch that builds
  `FinishedTake`, `JournalPass` or `RecordMark::Wrapped` literals adds the new fields.
- **Review 4, files outside my area**: `cantus-test-plugins/{clap-gain,clap-synth}/src/{ext.rs,
  lib.rs}` (the *dirty* tag: a load that marks the state dirty and asks for a values rescan —
  the sandbox tests need them rebuilt into `target/test-plugins`, which a run without
  `CANTUS_TEST_PLUGINS_PREBUILT` does); `cantus-host/src/{session.rs (`Echo`, `ECHO`,
  `render_states_for_test`), session_inst.rs, session_inst_idle.rs}`;
  `cantus-engine/src/{freeze.rs (`FrozenHandle::path`, `copy_as`, `open_as`, `paths_of`,
  `set_aside`, `alias`; `damaged` by copy), inst/stream_cache.rs (`relabel`),
  freeze_render.rs}`; `cantus-ui/src/{recovery_session.rs (`finish_saving` in
  `close_session`), views/inspector_sheet.rs (`readout_clear_row` replaces
  `text_button_row`), views/inspector_record.rs}`; tests `cantus-ui/tests/{inspector.rs,
  final_look.rs}`. `FreezeState` lost `left` (the copies' sources are noted on disk now).
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
