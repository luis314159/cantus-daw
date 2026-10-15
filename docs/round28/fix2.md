# Round 28 — fix2: the second reviews of voice, modfx, topbar, pitch and dynfx

Branch `r28/fix2` (from `r28/integration` at a1bbdf6; `r28/integration` merged in at
c5b602f before dynfx-2). Every confirmed finding of `reviews/{voice-2, modfx-2, topbar-2,
pitch-2, dynfx-2}.json`, deduplicated (the lenses overlap), with how it was closed and the
test that holds it. "Seen failing" means the test was run against the old code and failed.

**Schema:** 31 (`PitchNote::corrected`, `migrate_30_to_31`, `fixtures/v31.cantus`) — it was
30 on this branch until the merge; sound's Exemplar took 30. No golden hash moved.

## voice-2 (CLAP notes, Porta, Tonus) — crates/cantus-plugins, cantus-dsp, cantus-ui

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 3, 5, 9, 13 — an off on one channel / one note id released a key another channel or id still held | `Held` keeps id-less holds per key and channel and each id's (key, channel); a release emits `Off` only when the key's **last** holder goes, and an off for a key nothing holds emits nothing. An id forgotten past the 128-id table becomes a channel hold (its note is never lost) | `a_key_held_on_two_channels_is_released_by_the_last_of_them`, `two_note_ids_on_one_key_are_released_one_by_one`, `note_ids_past_the_table_forget_the_id_not_the_note` (clap/notes.rs) |
| 1, 2, 7, 12 — a channel's all-notes-off became per-key Offs a full 1 024-note buffer dropped, and `Held` had forgotten them | `push_note` says whether a note got in; `Held` records an on only when it got in, and a release with no room becomes an `AllOff` (always admitted) and clears `Held` — tracker and unit stay in step. VST3's reader does the same for a dropped note-off | `a_release_with_no_room_becomes_an_all_notes_off`; at the plug-in boundary `a_channels_all_notes_off_past_a_full_block_still_releases_its_key` (> 1 100 events, CLAP and MIDI dialects, three block patterns) — seen failing |
| 4 — Porta's lookahead keeps notes queued across blocks, so two full blocks overflowed the 1 024 queue (early notes, block-size dependent) | `MAX_TRIGGERS` 2 048; the compile-time check is `2 × NOTE_CAPACITY ≤ MAX_TRIGGERS`, and the comment says exactly when no note acts early (blocks at least as long as the key's wait, ≤ 10 ms) | `two_full_blocks_of_notes_waiting_through_the_lookahead_keep_their_timing` (porta_tests) — seen failing (opened 256 samples early) |
| 6 — the Spanish manual's "margen" | "tolerancia" | — |
| 8 — the octave-leap test passed on unvoiced frames | both halves also require ≥ 90 % of the frames voiced and on pitch (< 10 cents) | `a_legato_octave_leap_up_is_followed` |
| 10 — widened lanes hid every note name | names every 1, 2, 3, 4, 6 or 12 lanes (a divisor of the octave: the Cs always named) | `thin_lanes_are_named_every_few_never_none`; `the_display_keeps_a_transposed_output_in_view` asserts a stride with *More* open; shot `tonus-wide-*` |
| 11 — `SpeedWords` allocated per paint | writes into the formatter (`controls::write_readout` is generic over `fmt::Write`) | — (no allocation counter in UI paint) |

## modfx-2 (exported transport, Flexus, pages) — crates/cantus-plugins, cantus-dsp, cantus-engine, cantus-ui

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 9, 12 — a forward seek read as a huge tempo | a reading is believed only within the DAW's range (20–999 BPM) and either within ½–2× of the tempo found or confirmed by the next block (a seek never repeats, a tempo change does) | `a_forward_seek_is_not_read_as_a_tempo` — seen failing; `a_tempo_change_without_a_tempo_is_followed` |
| 1, 3 — in-block CLAP transport events fell back to 120 BPM and did not move `last_beats` | `push_transport_event` fills a missing tempo with the found one and measures the next block from the event's beats | `an_in_block_event_without_a_tempo_keeps_the_found_tempo` — seen failing |
| 4, 7 — CLAP's in-block events, its seconds timeline and VST3's `projectTimeSamples` untested at the entry points | the test host's `EventList` carries `CLAP_EVENT_TRANSPORT` at its time (`Plug::process_moves`) | `a_clap_transport_event_in_the_input_list_takes_effect_at_its_sample`, `a_clap_seconds_timeline_is_a_beat_timeline` (a seconds-only loop too), `a_vst3_host_without_beats_follows_its_project_time_in_samples` |
| 2, 14 — Flexus at its defaults was a 5 ms delay (no null) | latency 0 with through zero off, the centre with it on; the switch stays automatable — the engine re-plans compensation when a latency moves (dynfx's `replan_if_latency_moved`), an exported plug-in asks for a restart; the late line removed; the editor's latency chip follows the switch | `at_mix_zero_it_is_the_input_delayed_by_its_latency` (defaults bit-exact), `the_latency_follows_the_through_zero_switch` (no gap on the switch), engine `flexus_at_its_defaults_renders_exactly_as_no_flexus`, `the_through_zero_switch_sets_the_latency`, `an_automated_through_zero_switch_keeps_the_mix_in_line` |
| 8 — the comb test ran with feedback 0 | an impulse with feedback 0.5: echoes at 48 and every 49 samples after (the last wet waits a sample to go back in), each scaled, nothing between | `the_plain_flanger_is_its_comb` |
| 5, 6, 13 — the manuals' "Entrada" (Vibrans), Anulus's old card title, Tremor's page | "Subida"; "What a tone becomes" / "En qué se convierte un tono" with the 1 kHz on the display; the balance's checks | — |
| 10 — Translatio's "220 Hz" plate hid a peak | on the axis beside 100 and 1k, no plate | shot `translatio-*` |
| 11 — Fluctus's ring over "0%" | the plot inset 24/22 pt top/bottom, so a corner point's ring clears the axis words | shot `fluctus-*` |
| 14 — Flexus glue doc/page | rewritten to the switch-dependent latency (ADR 0055 table and null paragraph, the page, core and engine docs, both manuals); modfx.md marked superseded | `the_settings_read_every_parameter` asserts 0 plain |

## topbar-2 — crates/cantus-ui, cantus-engine, cantus-dsp

| Finding(s) | Fixed how | Test |
|---|---|---|
| 6, 16 — at 1920 px a running device's chip (204 pt) folded Inputs (EN) / more (ES) | the chip takes at least 150 pt while it reads a device (its line shortened in the middle, whole in its hint; a sentence — no audio, lost — up to 200, whole) and grows to 320 pt with spare room after the left block and the meter (the layout's short groups that can grow widen their column); folded groups are drawn at their most in ⋯. Measured with "Focusrite USB ASIO", in both languages | `the_default_bar_shows_every_group_at_1920` now with that device — seen failing; `a_short_group_that_can_grow_widens_its_column`; shots `topbar-bar-1920/2560-*` |
| 2, 4, 20 — the TP hold fed only while the meter was drawn | `meter::watch` once a frame beside `perf::watch`; `show` only reads | `an_over_while_the_meter_is_folded_is_held` (via a hidden `EngineHandle::set_meters` for the dummy engine) — seen failing |
| 5 — the per-block true peak cleared when its Meters event was dropped | `MasterLoud::take` no longer clears; `sent()` does, only after a successful push | `an_undelivered_reading_keeps_its_true_peak` |
| 1 — integrated LUFS counted pre-Play columns | `LoudnessMeter::restart_measurement` at the Play edge: gating blocks (and short-term values) only from windows that start there, the momentary running on | `the_integrated_reading_holds_nothing_from_before_play` — seen failing (−20.33) |
| 10 — the "only when it can change" test never opened the gate | 3 000 callbacks: 17 blocks, 17 scans, none more inside a step | `the_integrated_reading_is_worked_out_only_when_it_can_change` |
| 0 — a stale Position after a seek seeked twice in pattern mode | `should_seek`: a seek is waited for until a position inside the pattern is seen (or 30 frames) | `a_stale_position_after_a_seek_does_not_seek_again` |
| 21 — choosing a channel jumped to its first clip's pattern | `select_track` in pattern mode selects the track's clip in the pattern playing, or none | `choosing_a_channel_keeps_the_pattern` — seen failing |
| 17 — the audio popover covered the bar and the chip | hangs under the bar (beside ⋯ when the chip is folded in it); a second click on the chip closes it | `the_audio_popover_hangs_under_the_bar_and_the_chip_closes_it` |
| 18 — lamp cells too narrow for "10" | the number's rect is "88" + 2 pt (no width change) | `every_group_keeps_to_its_own_width` with twelve MIDI ports — seen failing |
| 7, 14, 19 — the clock cut from bar 1 000, the take's time while recording | a position longer than the metre's template steps down a size (Display → Hero); the time and the take's time step down to the condensed face when they do not fit; the dropout warning takes the lamp's place | `the_clock_reads_whole_in_every_metre` (bars 123, 1 000, 9 999 in five metres), `the_takes_time_reads_whole_while_recording` (11.5 and 105 min, with and without dropouts) — both seen failing |
| 13 — Compact density's rows touched | a row key is never taller than its row | `row_keys_fit_their_rows_at_every_density` |
| 8 — the TP figure took no click | the footer resets the hold in loudness (and shows the loudness hint) | covered by the manual's wording now true |
| 9, 15, 22 — `topbar.menus` and the comment | removed; "The overflow and customising"; `LEFT`'s doc | i18n test |
| 3, 12 — the fold-order test checked nothing | all-tall wants: every folded group less important than every placed one; the real bar: no folded group would fit back, Transport/Position/Tempo/Mode stay | `a_narrow_bar_folds_the_least_important_first` |
| 11 — undo/redo bringing pattern mode back mid-take | — (the code was right) | `an_undo_or_redo_that_brings_pattern_mode_back_ends_the_take` |

## pitch-2 — crates/cantus-core, cantus-project, cantus-ui

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0 — Correct pitch guessed its own notes from the current key | `PitchNote::corrected` (schema 31): set by the macro, cleared by a hand move | `correct_pitch_follows_a_key_change_and_keeps_a_hand_move`; `schema_31_keeps_which_pitch_is_the_macros` |
| 14, 15, 21 (HIGH) — the lane's hit area ran under the ruler/panels and read the raw press under windows | interaction on the lane's visible part only; a press starts a drag only through the lane's own (layer-aware) response | `the_pitch_lane_takes_presses_only_where_it_is_seen` (out of view, under a window) — seen failing |
| 1 — sideways jiggle moved a note / no-op undo steps | the gesture latches its direction past 3 px; only vertical moves pitch; never commits an edit equal to the press's | `a_sideways_slip_on_a_lane_note_changes_nothing` |
| 19 — range re-fit during a drag | the press's range holds (grown only if the note leaves it) | `a_dragged_lane_note_stays_under_the_pointer` |
| 18 — per-frame clones | the lanes cached per (revision, timeline, rect), rebuilt only while a take is analysed or a drag runs; the hover reads the cached centre | `an_idle_pitch_lane_is_laid_out_once` |
| 6, 22 — analysed but unedited takes not offered | listed when a lane-0 unmuted take's analysis is ready | `quantise_onto_an_analysed_unedited_take` |
| 2, 17, 23 — a deleted target | `CantusApp::quantise_target` (exists and is not this track) used by the bar, the menu and Quantise | `a_deleted_quantise_target_falls_back_to_the_grid` |
| 3 — the core `quantise_to` unused | it takes f64 onsets, a reach and `reverse`; the editor's Quantise runs it | core test with a reversed case |
| 5 — `has_gain` rescanned per sample | worked out once in `Curves::new` | — |
| 16, 24 — the bar's fold budget 20 pt short; core controls cut on a narrow bar | the budget counts both dividers and ⋯'s 44; Original, key and snap fold into ⋯ too | `the_pitch_bar_keeps_its_more_key_at_every_width` (380–1 600 pt, ES) — seen failing with the old budget |
| 7 — MIDI onto a track unreachable when folded | ⋯ lists "Melody onto a track" with every instrument track | `a_very_narrow_bar_offers_everything_in_its_more_menu` |
| 20 — the A/B row's help | by state, naming the click | `the_inspectors_ab_row_says_what_a_click_does` |
| 4, 8, 9, 25 — `v27.cantus.bak` | already gone from integration; `crates/cantus-project/fixtures/*.bak` ignored | — |
| 10, 11, 12, 13 — tests | glue of two halves with the same edit keeps it; MIDI out of a looped clip (one copy per period, the last cut at the clip's end); unequal gains across a short gap; *Edit pitch* disabled on a missing file | `halves_with_the_same_edit_glue_and_keep_it`, `midi_out_repeats_a_looped_clips_notes`, `a_notes_gain_never_steps…`, `edit_pitch_is_disabled_while_the_file_is_missing` |

## dynfx-2 — crates/cantus-engine, cantus-plugins, cantus-ui

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 15 — the exported clock read apart from its frames | the tap counts the frames it took (`Shared::tapped`); `publish_transport` stores that count with the block's last beat under a sequence lock; the editor counts what it drains and reads `clock_of_drained` right after the drain | `the_exported_speculums_clock_travels_with_its_frames` — seen failing |
| 1, 2, 9 — a re-watched Speculum kept its stale chunk and losses | the `WatchScopes` handler resets every tap whose slot changed (released, handed over, watched again) | `a_speculum_watched_again_starts_afresh` — seen failing (4 416 frames "lost") |
| 5, 14 — the allocation test never ran the pool | `Workers::parallel(2)` (unpromoted) on the two-track demo, a Speculum on a strip and the master; asserts the armed blocks ran on the pool | `the_copy_allocates_nothing` |
| 6 — beat stamps: only inserts-before tested | a bus fed by a latent strip (its arrival) and the master behind a latent strip are 93 samples late; a strip *beside* a latent one is not (its music is heard at its slot as it plays — the review's expectation for that case was wrong: the line is after the strip) | `the_beat_stamp_counts_another_strips_latency_and_the_masters` |
| 7 — no chunk across a jump | blocks of 100 frames, a seek, a loop wrapping mid-chunk, a stop: chunks end at each, stamps exact | `a_chunk_ends_where_the_song_jumps` |
| 8 — the reused-ring test reached neither mechanism alone | the pending reset is held by `a_handed_over_ring_counts_only_its_new_owners_frames` (the frame count). The drain-side owner filter guards a cross-thread race (the processor pushing the old owner's chunk after the UI's pop, before the message lands) that the single-threaded manual engine cannot produce; with the `WatchScopes` reset the window is that race alone. Not unit-tested: it would need a test hook writing into the processor's ring | — |
| 10 — master chain off: frames not compared | every frame compared, both channels, irregular blocks | `a_master_speculum_measures_with_the_chain_off_or_bypassed` |
| 11 — exported losses and *Gaps* untested | the tap full, the window full, counted once; the overview draws *Gaps* and its ms only when frames were lost | `the_exported_speculum_counts_what_it_lost`, `a_docked_tabs_overview_keeps_its_levels_in_view` |
| 12 — a docked tab's overview cut the loudness panel | a short display (< 300 pt) gives the loudness panel the whole height; its rows by rank (M, S, I, peak/RMS, gaps, then TP, range, scale); the legend only beside the title when it fits; scale marks by width; the vectorscope keeps its words' width | `a_docked_tabs_overview_keeps_its_levels_in_view` (560×420, 760×460, EN/ES) — seen failing; `shoot_dynfx --docked` |
| 13 — Impetus's key field "own input" vs its list's "own signal" | the closed field names the effect's own choice | `the_key_field_reads_as_its_list` |
| 3, 4 — Spanish manual's Impetus card, speculum.md's "L+R" | "Un golpe procesado"; "L/R" (two curves, not a sum) | — |

## Files outside the reviewed areas

`crates/cantus-plugins/tests/common/mod.rs` (the event list carries transport events),
`crates/cantus-engine/src/backend.rs` (`set_meters`, hidden), `crates/cantus-dsp/src/terminus_loudness.rs`
(`restart_measurement`), `crates/cantus-ui/src/app.rs` (`select_track`), `crates/cantus-ui/src/views/record_options.rs`
(the take's time), `crates/cantus-ui/src/views/fx/controls.rs` (`write_readout` generic),
`crates/cantus-project` (schema 31), `.gitignore`.

## Shots

`F:\projects\daw-r28\shots\fix2\` — topbar (1920/2560 with a device), tonus (`wide`),
translatio, fluctus, dynfx (`speculum-docked-*`).

## Left

Nothing that does not need the owner.
