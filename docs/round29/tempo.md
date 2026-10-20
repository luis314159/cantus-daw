ENGINE READY 0dd848a

# Round 29 — tempo: the tempo track, metre changes and markers

Item 4 of round 29. Parts 1–3 (the model, schema 33; the real `TempoMap`; the engine
following it) landed at `0dd848a` (ENGINE READY); review 1's 27 findings are fixed (table
below); parts 4–7 (the interface, interchange, the tests, the finish) follow. Decisions:
ADR 0060.

## Done

1. **Model, schema 33**: `Project::{tempo_points, metre_changes, markers}` (skipped when
   empty); eleven commands with exact inverses, `cmd.tempo.*` in both languages; undo-fuzz
   arm 43 with twelve kinds counted (`SetTimeSignature` drawn at last); migration, fixture
   block, `v33.cantus`, `schema_33_…` test.
2. **`TempoMap`**: segments precomputed, binary search, no allocation on lookups,
   closed-form ramps with `cantus_dsp::math`, the first tempo and metre before tick 0,
   bars/beats/BBT/snaps through every metre change, `span_key`.
3. **Engine**: transport through the map (held stretches the old arithmetic to the bit),
   passes cut at tempo steps and at each late insert's own, the click and its accents,
   `SongPosition` (step tempo, exact tempo, `tempo_inc`, seconds, bar number, loop
   seconds), `earlier_in`, the modulation clock, modulators and the bake, Speculum, unwarped
   audio (`audio_time`, `clip_zero`), fades and seams, render/stem/trace lengths and stop
   samples, the recorder, the recovery journal, CLAP and VST3 transports, the exported
   plug-ins. Fixed on the way: a beat or note exactly on a loop's start was skipped after
   every wrap at tempos whose loop is no whole number of samples.
4. **Interface**: `views/bars.rs` (every ruler, grid, BBT, bar/beat snap, nudge, default
   window, count-in, punch, clock); the tempo track lane; metre chips; the marker lane
   (jump, rename on the letters, drag, colour, loop/select/export to the next, go to by
   name); the top bar at the playhead; keys Alt+M, Alt+, Alt+., Alt+Shift+O (review 2: was Alt+Shift+L, taken; review 3: not Alt+Shift+, , which ES/DE/FR keyboards cannot type), Alt+Shift+M,
   Alt+Shift+T (keymap 9, `NEW_IN_V9`); View ▸ *Tempo track*; the timeline menu's entries;
   *Export between markers* (`Scope::Markers`); `shoot_tempo` (PNGs in
   `F:\projects\daw-r29\shots\tempo\`).
5. **Interchange**: MIDI export writes the whole conductor (ramps as averaged sixteenths,
   metres, markers 0x06); MIDI import's default `TempoMode::Map` keeps the file's map;
   DAWproject `TempoAutomation`/`TimeSignatureAutomation`/`Markers` both ways, XSD-valid;
   the AI bridge sees the map and has `set_tempo_point`.
6. **Tests**: below. 7. **Finish**: ADR 0060; the manual's *The tempo track and markers*
   (§5) in English and Spanish; this file.

## Measurements

- Ramp steps add ~16 % passes at 90–140 BPM, costing **1.8 %** (one-tempo song in 16 %
  more passes); the ramped demo costs 9–15 % more per song second, mostly denser notes at
  the faster tempo (`tempo_map::measure_the_cost_of_ramp_steps`, best of five).
- Inverses within 1e-9 s over 300 random maps; a ramp within 1e-9 s of Simpson's rule; a
  loop over a tempo change within 1e-3 sample; a MIDI ramp round trip within 1e-4 s.
- No golden moved; `DSP_VERSION` unchanged.

## Tests and what each holds

- Core: constant map = old arithmetic to the bit; steps/ramps exact; inverses and
  monotonicity over random maps; ramp steps on ticks; bars through metres and off-line
  changes; `span_key`; no change at 0 before the song; every command and inverse; metre
  edits keep untouched ticks; a drag is one undo step; top-bar helpers; unwarped clips at
  natural speed; modulators through a moving map; an audio paste keeps its seconds; fuzz
  arm 43.
- Engine (`tempo_map.rs`, `tempo_audio.rs`, `transport.rs`, `recorder.rs`, `rt_memory.rs`):
  block sizes 1/37/64/512 offline and live (click, loop over a tempo change, automation);
  offline = real time; parallel = serial; the click on every beat of a ramp, bar 1
  accented; a note after a ramp at its sample; synced gate and automation on the beat; loop
  wraps exactly; the loop-start beat every pass; a late synced unit at every block size;
  render length through the map; a hand-computed event; unwarped audio = `audio_time`, a
  clip on a point from both seeks, fades; hosts keep their one-tempo sample position; a late
  unit and a take across a ramp; no allocation.
- Host: CLAP event and VST3 context under a map. Project: schema 33; MIDI and DAWproject
  round trips; a foreign file's markers and tempo kept. AI: the map in context, a tempo
  point set. UI (`bars.rs`, `tests/tempo.rs`): snapping/ladders through metres; keys via
  the assembled app (boot path); the top bar never writes its rounded tempo back (found
  shooting it, seen failing); export between markers; a v8 keymap gets the keys.

## Files outside my area (additive)

`cantus-core`: command.rs, model.rs, snapshot.rs, lib.rs, clipboard.rs, modulator_*.rs.
`cantus-engine`: automation.rs, graph/strip_run.rs, graph/master_run.rs,
processor_auto.rs, processor_rec.rs, modulator.rs. `cantus-plugins/src/instance.rs`.
`cantus-ui`: app.rs (pre-hook, one arm, nudge), lib.rs, keymap.rs, views/mod.rs,
views/arrangement.rs (mods, fields, frame lanes, geometry, calls, timeline menu list,
lengths, snap), views/transport.rs, transport_menus.rs, export.rs, export_rows.rs,
interop.rs, and the migrated sites; tests fx_lint.rs, export_options.rs,
recovery_flows.rs, comping.rs. `cantus-export/src/settings.rs`, `cantus-ai/src/bridge.rs`,
`cantus-project/src/report.rs`.

## Left

Nothing of mine. Owner-only: listening to a ramp with synced effects and a hosted plug-in.

## Notes for the integrator

- Regenerate `fixtures/v33.cantus` after chaining schemas.
- Keymap 9: `NEW_IN_V9` (six actions) — others append.
- Export dialog *What*: Song, Selection, Loop, **Markers**, Stems.
- MIDI import default `TempoMode` is now `Map`.
- `SongPosition` + `tempo_inc`, `tempo_now`, `seconds`, `bar_number`, `loop_seconds`;
  `TransportInfo` + `tempo_inc`, `position_seconds`, `bar_number`, `loop_seconds` — the
  isolation IPC must carry them.
- Retires the known issue "one tempo and one time signature per project" (release notes,
  manual §14, both languages); manual §12 shortcuts are yours.
- Review 2: **an `FxUnit` that overrides `set_position` must also return `true` from
  `follows_song`** (only those are told the song and cut passes at their late steps).
  `SongPosition` + `project_samples`, `TransportInfo` + `project_samples` (VST3's
  `projectTimeSamples`); isolation's `WireTransport` carries it, bridge protocol 3.
  `clipboard::split` and `clips::move_to` take the project; `audio_clips::glue_in` /
  `clips::glue_in` glue under the map. Unwarped audio plays from its content origin
  (ADR 0060 §4); a tempo edit re-anchors them in its own undo step (`tempo::keep`, review
  3). *Loop between markers* is Alt+Shift+O (review 3).

## What the shared API gives (for warp, freeze and controllers)

- `cantus_core::tempo::TempoMap`: the contract's methods, now real (ramps linear in ticks,
  integrated in closed form; the metre counted in bars through every change), plus
  `held_at`, `in_ramp`, `slope_at`, `seconds_between_f64`, `tick_after_seconds`,
  `step_start`, `next_step_after`, `is_one_metre`, `bar_end`, `bar_len_at`,
  `beat_len_at`, `bar_tick`, `tick_of_bbt`, `beat_lines`, `beat_at_or_after`,
  `beat_after`, `next_beat`, `snap`, `snap_floor`, `snap_bar`, `add_bars`;
  `cantus_core::tempo::RAMP_STEP`. `Project::tempo_map()` compiles the project's;
  `Snapshot::tempo` carries it to the engine.
- `span_key(from, to)` is the tempo function over the span measured from `from`: a span
  inside one held tempo keys on that tempo alone (the contract's value), so moving a clip
  along a held tempo keeps its key; points outside the span never change it.
- `cantus_core::audio_time`: the one mapping of an **unwarped** audio clip's ticks to its
  source (natural speed, anchored at the clip's start, `offset` read at the tempo where
  the clip starts): `offset_seconds`, `source_seconds`, `tick_at_source`, `frame_at`,
  `tick_at_frame`, `length_ticks`, `offset_for`. The engine's `Transport::clip_zero` uses
  it; warp extends this file for warped clips.
- Engine: `Transport::set_map`, `sample_of_f64`, `samples_between`, `clip_zero`,
  `tick_before`, `step_tick`, `samples_until_step`; `transport::sample_at_map` (what a
  render's stop sample is); `fx::position_at_tick`, `SongPosition::earlier_in`.

## Review 1 of the engine part (`tempo-engine-1.json`, 27 confirmed) — how each was fixed

Every fix has a test seen failing first where the finding was a defect; coverage findings
got the tests they asked for.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0 | `clip_zero` read a clip's offset at the previous stretch's tempo when the clip starts exactly on a point | the held path only for a start strictly inside the stretch | `transport::a_clip_on_a_tempo_point_reads_its_offset_at_the_tempo_there` (failed: 24 000 vs 48 000), `tempo_audio::a_clip_on_a_tempo_point_plays_the_same_from_wherever_playback_started` |
| 1, 9, 15, 21 | `Clipboard::at` never set | the arrangement's copy sets it (`clipboard::copy_origin`, the time selection's start); a mapped paste converts only where the tempo over the clip differs (`span_key`) | `clipboard::an_audio_clip_pasted_under_a_tempo_track_keeps_its_seconds` (failed without `at`) |
| 2, 3, 13 | metre edits rebuilt every tick from bar indices | `SetMetreAt`/`RemoveMetreAt`/`MoveMetre` keep every earlier change's tick, land the edited change exactly where asked (a move counted in the metre before it, between its neighbours), and move the later changes so each keeps its bar number and its short-bar remainder | `edit_tests::metre_edits_keep_the_ticks_they_do_not_move` (failed) |
| 14 | `SetTimeSignature` with changes left later changes off the lines | routed through `SetMetreAt` at tick 0 (inverse: the whole list) | same test |
| 4 | `span_key` skipped the first ramp for a span starting before 0 | the loop starts at the first segment after `from` | `tests::the_span_key_before_zero_sees_the_first_ramp` (failed) |
| 5, 12 | `next_change_after` said 0 before tick 0 on a held first tempo | a held first segment is no change; only a ramp from 0 is | `tests::a_held_first_tempo_has_no_change_at_zero` (failed) |
| 6, 8, 11 | late inserts crossed steps mid-pass | passes also end at each late insert's own step (`Transport::samples_until_step_late`, as the late wraps do); a step reached within a hair counts as reached (`fx::step_of`) | `tempo_map::a_late_synced_unit_hears_the_steps_at_every_block_size` (failed at block 1) |
| 7 | Speculum chunks placed late audio at the constant tempo | `scope` takes the map; the first frame's beat through `earlier_in` | covered by the late-unit and rt tests |
| 10 | CLAP told a sawtooth tempo | hosts are told the exact tempo at the block's first sample (`SongPosition::tempo_now`) with `tempo_inc` from it; native units keep the step tempo | `unit::a_hosted_plug_in_hears_the_tempo_map` |
| 16 | `cmd.tempo.*` had no strings | `locales/areas/tempo.{en,es}.txt` | the i18n test (both languages) |
| 17 | no engine test of unwarped audio under a map | `tests/tempo_audio.rs`: the engine's frame within one of `audio_time`'s through a ramp, the clip on a point from both seeks, block sizes 1/37/64/512, fades measured through the map | — |
| 18 | render length/stop untested | `tempo_map::a_rendered_range_is_as_long_as_the_map_says` | — |
| 19 | block-size test missed click, loops, automation | `tempo_map::the_click_a_loop_and_automation_live_are_the_same_at_every_block_size` (live) | — |
| 20 | modulators through a moving map untested | `modulator_tests::modulators_read_time_through_a_moving_tempo_map` (gates by hand, Hz retrigger, one tempo to the bit) | — |
| 22 | hosted transport only via `position_at_tick` | `transport::a_late_unit_hears_the_ramp_through_the_map`; `vst3_backend` pins `projectTimeSamples` to the song's seconds under a map | — |
| 23 | recording across a ramp untested | `recorder::take_start` extracted; `a_take_across_a_ramp_lands_where_it_was_played` | — |
| 24 | transport tests compared the map with itself | `tempo_map::an_event_after_a_step_is_due_at_the_hand_computed_sample` (by hand) | — |
| 25 | no merge test | `edit_tests::a_drag_is_one_undo_step` | — |
| 26 | one-tempo `projectTimeSamples` changed formula | one-tempo `seconds` is now the expression `samples` rounds, so `round(seconds·rate) == samples` | `transport::one_tempo_tells_hosts_the_sample_position_they_had` |

## Review 2 of the whole branch (`tempo-2.json`, 67 confirmed) — how each was fixed

Merged `r29/integration` (5c17c80: tempo + isolation + warp) first. Every test named below
was run and seen failing before its fix, except where the row says *coverage* (a test the
review asked for of code that was right; it passed at once).

**One design change.** Findings 0, 3 and 43 (an audio clip's split, trim, move and glue
across a tempo change made the audio jump) asked for the offset to be re-derived in
seconds at every edit. Doing so showed the root: the rule "a clip is anchored at its start,
its offset read at the tempo there" makes every window of a take disagree with the others
after any edit or tempo change inside the take (comp windows, `clear_range`'s pieces, seam
moves, glue, comping's content identity all do tick arithmetic). The rule is now the
**content origin** (`start − offset`, laid on the timeline once through the map; ADR 0060
§4 rewritten): every window edit's tick arithmetic is exact, all of a take's windows agree,
one tempo is the old rule to the bit. Moves and copies elsewhere, and a reversed window's
edits, re-derive the offset (`audio_time::relocate`, `reanchor_reversed`).

| # | Finding | Fix | Test |
|---|---|---|---|
| 0, 43 | split / left trim / glue of an audio clip across a tempo change jumped | the content-origin rule (above): forward edits keep the origin; reversed splits and trims re-derive (`reanchor_reversed`); `clipboard::split` takes the project | `audio_edits::a_split_across_a_tempo_change_plays_on_without_a_jump`, `…reversed…`, `a_time_range_cut_…` (failed: 312 037 vs 528 037), `tempo_edits::a_trim_across_a_tempo_change_keeps_what_plays`, `a_split_across_a_tempo_change_glues_back_to_the_clip` (failed) |
| 3 | a move or drag-copy differed from a paste to the same tick | `clips::relocate` (one `audio_time::relocate` for paste, move, drag-copy, duplicate; linked clips move their window only) | `tempo_edits::a_move_a_drag_copy_and_a_paste_to_one_tick_agree`, `a_duplicate_across_a_tempo_change_plays_the_same_seconds` (failed: 4 vs 2) |
| — | a comp seam moved across a tempo change (found while fixing 43) | exact under the origin rule | `audio_edits::a_seam_moved_across_a_tempo_change_keeps_the_right_windows_audio` (failed on the start rule) |
| — | review 1's row 0 test | renamed `transport::a_clip_on_a_tempo_point_plays_from_its_content_origin` (24 000: a beat at 120 before the point); `tempo_audio`'s clip-on-a-point test now has its origin on the point | — |
| 1, 55 | a dragged tempo point ran away | the lane's axis is frozen at the press (`Drag{lo, hi}`), in input and paint | `tempo_review2::a_dragged_tempo_point_held_still_stays_put` (failed: 175.8 vs 158.6) |
| 2, 44 | the top bar's wheel/drag in a ramp rewrote the ramp's start | `CantusApp::nudge_tempo_now`: the governing point moves by the gesture's own delta (tenths, the rest carried); a wheel notch is one BPM from raw events | `tempo_review2::a_wheel_step_inside_a_ramp_moves_its_start_by_one` (failed: 140.4) |
| 4, 9 | late cuts ignored loop wraps | `fx::heard_tick`, shared by `earlier_in` and `samples_until_step_late`; a step at or past the wrap's end is the late wrap's cut | `tempo_review2::a_late_unit_after_a_wrap_hears_the_steps_at_every_block_size` (failed at block 1) |
| 5, 14, 31, 30 | Alt+Shift+L was Select linked's in every preset | *Loop between markers* is Alt+Shift+, ; manual (EN/ES), ADR, this file | `keymap::every_new_action_has_a_key_in_every_preset`, `tempo::a_version_8_keymap_gets_the_marker_keys` (all six stripped and asserted; failed) |
| 6, 19, 57 | DAWproject seconds content placed at one tempo | the tempo automation first (seconds points placed in order through the map they make), then every clip, note, marker, metre, loop and fade through `TempoMap` (`Ctx::at`/`len`); a seconds `linear` ramp reported as approximated; `d_seconds` only at one tempo, `tempo.report.seconds_map` otherwise | `dawproject_tempo::seconds_content_after_a_tempo_change_lands_where_it_sounds`, `tempo_points_in_seconds_are_placed_one_after_another` (failed); the Bitwig test asserts where the points and markers land |
| 7, 24, 41, 53 | the inspector's fades at the clip start's tempo | `audio_clips::fade_seconds` / `fade_ticks`: a fade in from the start, a fade out back from the end, through the map | `tempo_review2::fades_are_measured_where_they_lie` (failed) |
| 8, 63 | a ramp at beat 0 dropped | `add_tempo_point` at tick ≤ 0 with a track sets point 0 with its ramp | `edit_tests::a_ramp_from_the_start_ramps` (failed), `bridge::the_agent_can_ramp_from_the_start` |
| 10 | every late insert cut passes in a ramp | `FxUnit::follows_song` (the modulation family, hosted plug-ins; a watched Speculum counts); `follow_lates` cut, the other lates only wrap; non-followers are not told the position | `tempo_review2::only_the_units_that_follow_the_song_cut_passes_at_their_late_steps` (failed: 576 vs 484); measured in ADR §8 (1 703 → 1 193 passes) |
| 11 | reach tolerance in ticks vs cut epsilon in samples | `fx::step_of` takes the rate: a millionth of a sample | `tempo_review2::a_late_step_is_reached_exactly_at_its_cut` (failed at seek 1054) |
| 12 | Speculum stamped late frames at the transport's tempo | the heard position's tempo for `per_frame`, the stamp and the carry-on test | `tempo_review2::a_late_speculum_stamps_its_frames_at_the_tempo_it_hears` (failed: 64 vs 128) |
| 13 | one-tempo CLAP seconds changed operation order | `seconds = beats·(60/tempo)` (round 28's); VST3 takes the new `project_samples` (SongPosition, TransportInfo, the bridge's `WireTransport`; protocol 3) | `transport::one_tempo_tells_hosts_the_sample_position_they_had` (bits; failed) |
| 15, 32, 38 | the ruler's menu acted at the playhead; ▸ at a stale point; two menu boxes | a ruler right-click records its tick; `arrangement::menu_point`: the point for a right-click, the playhead for ▸ (labelled *at the playhead*); the tempo entries are in the one menu | `tempo_review2::the_ruler_menu_acts_where_it_was_opened` (failed: tick 0) |
| 16, 33 | *Add a metre change* at bar 1 did nothing visible | at bar 1 or on a change: no command; the chip's editor, or the top bar's metre (`TopBar::open_metre`); a stale `open_metre` is dropped | `tempo_review2::a_metre_change_at_bar_one_opens_the_metre` (failed) |
| 17, 36, 49 | export between one marker twice exported the song | the pickers refuse a pair with no range (`tempo.export.two_markers`); `start_with` and `choose_and_start` refuse an unavailable range with a status, never the song | `tempo_edits::an_export_between_markers_never_falls_back_to_the_song` (failed) |
| 23 | the dialog's refusal assertion could not fail | scope unchanged, not Markers, `asked == Markers`; a second case with two markers | `export_options::the_scope_and_stems_choices_choose`, `the_markers_scope_takes_the_markers_around_the_playhead` |
| 18 | the lanes' state carried into another project | the state is the project's (`epoch`; `exec(ReplaceProject)` bumps it): another project starts afresh | `tempo_review2::another_project_shows_its_own_tempo_track` (failed) |
| 20 | the UI copy origin untested | coverage: a clip copy and a time-range copy through the app set `Clipboard::at`; a paste inside the held tempo keeps offset, length, fades and slip | `tempo_edits::copy_and_paste_inside_one_tempo_keep_the_clip` |
| 21 | the range test could not see the stop | coverage: tail 0.5, the end-tick note silent, the one before it sounding from the map's sample; stems and a modulator trace the same length | `tempo_map::a_rendered_range_is_as_long_as_the_map_says` |
| 22 | unwarped audio test lenient and partial | coverage: within half a frame; a clip inside the ramp (offset 1 beat, slip 37) by hand and from a seek; the fade out's first sample at the map's beat 7 | `tempo_audio::*` |
| 25 | modulator bake untested; gate from-level from the code | coverage: release 2 s, the second gate's level by hand; bake vs render through a ramp (envelope, 1 Hz retriggered LFO) | `modulator_tests::modulators_read_time_through_a_moving_tempo_map`, `modulators::a_baked_route_renders_what_the_modulator_did_through_a_ramp` |
| 26 | off-line metre lists never fuzzed | fuzz kind 7 draws 2–3 changes off the bar lines; a unit test of an edit before a short bar (SetMetreAt, RemoveMetreAt, MoveMetre) | `undo_invariants`, `edit_tests::an_edit_before_a_short_bar_keeps_its_remainder_and_bar_number` (coverage) |
| 27, 62 | Map import at a drop position | the file's map starts at the bar line nearest the drop (notes too); the song's tempo and metre before it, a ramp into it ending a tick before; dropped at the start the file's map replaces the song's (a file of one tempo too); the song's markers stay; stated in ADR §6 | `midi_tempo::a_map_dropped_into_a_song_starts_at_its_bar` (failed), `midi_file::an_import_is_one_step_of_the_undo_history` amended |
| 28 | transport block test true by construction; no ramp by hand | coverage: a loop over the ramp's end at 1/37/64/512; beat 6 in closed form | `tempo_review2::the_transport_loops_over_a_ramps_end_and_meets_the_closed_form` |
| 29 | loop recording and recovery under a ramp untested | coverage: three passes' windows put their recorded frames at their starts; the recovered start by hand; the recorder's test has a real ramp | `tempo_record::*`, `recorder::a_take_across_a_ramp_lands_where_it_was_played` |
| 34 | colour swatches closed by their own click | the pass they were offered in is kept; a click away counts from the next pass; Esc closes | `tempo_review2::a_markers_colours_stay_open_through_the_click_that_opened_them` (failed) |
| 35, 50 | a click on a marker renamed | a click anywhere on the tab seeks; a double-click or the menu renames; manual EN/ES | `tempo_review2::a_click_on_a_marker_seeks_and_markers_on_one_tick_sit_side_by_side` (failed) |
| 51 | same-tick markers hid each other | laid side by side, each keeping its name | same test |
| 37 | the import words said Cantus has one tempo | `interop.midi_import.tempo_changes` ("… Keep:"), `interop.report.tempo_changes` reworded EN/ES; the dead `d_tempo_automation`/`d_metre_automation` deleted; the manual cites the real option | the i18n test |
| 39 | the manual's click accent | EN/ES reworded as asked | — |
| 40 | the tempo lane's hint caps | `menu`, `typed` over a point only; no *automatable* | — (one line) |
| 42 | default marker names repeated | the first unused "Marker N" from the count on | `tempo_review2::a_new_markers_name_is_never_taken` (failed) |
| 45 | automation editors snapped content ticks to the timeline's bars | `auto_curve::Frame::origin`; ladder, ruler and snapping through `snap_point` (`bars::snap_content`) | `tempo_review2::automation_points_snap_to_the_timelines_bars` (failed) |
| 46 | pitch quantise counted from tick 0 | `bars::Snapper::tick`, as the ladder | — (the helper is the arrangement's) |
| 47 | a bar nudge used the bar at the playhead | the earliest selected item's bar (`bars::add_bars`), one delta for all | `tempo_edits::a_bar_nudge_is_the_bar_the_clip_is_in` (failed: 7200) |
| 48 | Terminus' song view and summaries at one tempo | `SongCtx::map`; `State::measure_in` (each bucket's seconds through the map, bars by `bbt`); bar lines, beat ticks and the hover readout through it; Porta's history bar lines by `bar_start` | `fx_limiter` tests unchanged (one tempo) |
| 52 | a metre chip dragged too far showed errors | `tempo::clamp_move` holds it between its neighbours; the drag's command never reports | `tempo_review2::a_metre_chip_stops_at_its_neighbours` (failed) |
| 54 | the inspector's length in the first metre | `bars::length_bbt` through the bars it spans | `tempo_review2::a_length_is_counted_in_the_bars_it_spans` (failed) |
| 56 | the pattern clock at the song's first tempo | `clock::clock_seconds` from the pattern's start where it lies | `tempo_review2::the_pattern_clock_runs_at_the_patterns_tempo` (failed) |
| 58 | DAWproject kept the first of two points at one time | the later wins (tempo and metres); a ramp into the earlier ends a tick before | `dawproject_tempo::a_jump_written_as_two_points_keeps_the_later`, `two_metre_points_at_one_time_keep_the_later` (failed) |
| 59 | kept markers counted as dropped texts | not in Map mode | `midi_tempo::markers_kept_are_no_texts_dropped` (failed) |
| 60, 61 | odd metres failed the import; a first metre a few ticks in | see 27's row: dropped and counted; equal ticks keep the later; a first signature before the first bar line is tick 0 | `midi_tempo::odd_and_crowded_metres_are_tidied` (failed) |
| 64 | an SMPTE file promised a map | no lists for SMPTE; the dialog's hint only in Map mode | `midi_tempo::an_smpte_file_keeps_no_map` (failed) |
| 65 | MIDI export dropped markers at the end | the song's file runs to its last marker | `midi_tempo::markers_at_and_after_the_end_are_exported` (failed) |
| 66 | the tempo lane's `unit` ignored | bpm / missing as is, `normalized` through the transport's range, another dropped with `tempo.report.tempo_unit` | `dawproject_tempo::the_tempo_lanes_unit_is_read` (failed) |

**Shared files reviewed with warp's changes:** `audio_time.rs` (warp's `frame_through`,
`tick_through`, `clip_frame_at` untouched: a warped clip reads content ticks, which is the
origin rule's own arithmetic; `warp_attach`'s `zero_slip = −offset_seconds` still puts the
window's start at the render's frame 0 under the origin rule), `clipboard.rs` (a warped clip
is never relocated), `audio_clips.rs` glue (warp equality kept). Isolation's `WireTransport`
carries `project_samples` (protocol 3).

**Verified after merging `r29/integration` (bbcc062, freeze):** core, dsp, project, engine,
host, export, ai and plugins 2 087 passed; UI 1 366 passed; the engine with `fuzz`
(`CANTUS_WORKERS=3`) 756 passed; `cargo clippy --workspace --all-targets -D warnings` and
`cantus-plugins --no-default-features` clean; `cargo fmt --check` clean. No Command was
added or changed (freeze's `freeze_guard::touches` untouched); no golden moved,
`DSP_VERSION` unchanged.

## Review 3 of the fix round (`tempo-3.json`, 56 confirmed) — how each was fixed

Merged `r29/integration` (2e23325) first, then, before the final run, `r29/integration` again (6e74f7e: freeze's and warp's review
fixes; one conflict, in the DAWproject reader, resolved to warp's equivalent change). Every
test named below was seen failing before its fix — on the old code, or with the fix
reverted in the tree (*mutation*: the fix undone, the test run, the fix put back) —
except where the row says *coverage* (a test asked for of code that was right) or *not
tested* (drawing only).

**The central decision (findings 0, 13, 37; ADR 0060 §4 rewritten).** Two invariants: a
tempo edit never changes which source frame plays at an unwarped clip's start, and every
window edit (split, trims, comp, glue, loop passes, pastes, block moves) stays exact. Design
(b) holds both: the content-origin rule stays for reading and editing windows, and **the
command bus re-anchors on every command that changes the tempo map**
(`CommandBus::apply_keeping_audio`, `cantus_core::tempo::keep`): before the command it notes
each unwarped window's source second at its start (clips and alternate comps' windows; a
warp that follows the tempo is skipped), after it emits `SetClipOffset` / `SetClipAudio`
(slip) / `SetTrackComps` re-anchors, composed into the command and its inverse — one undo
step, exact inverse; a refused re-anchor rolls the whole command back. A window the command
moved itself is left alone; a linked window moves its offset alone (its slip is its
group's), within half a tick. No new `Command`, so `freeze_guard::touches` needs no arm; the
undo fuzz gained the invariant (every unwarped window's frame at its start kept after every
tempo command, the count asserted non-zero). Design (a), the left edge in source frames, was
rejected: every window edit back to seconds (review 2's problem) and a schema change.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0, 13, 37 | a tempo edit before a take (pre-roll, earlier loop passes, a count-in) slid its audio; comp windows re-cut | the bus re-anchoring above | `tempo_keep::a_take_recorded_after_play_from_bar_one_keeps_its_frame`, `loop_passes_keep_their_frames_after_an_edit_before_the_loop` (passes 1..3, agreeing), `comp_windows_of_one_take_keep_their_frames_and_their_origin`, `a_reversed_window_and_whole_track_edits_keep_their_frames` (`SetTempoTrack`, `SetTempo` without a track), `a_linked_window_moves_its_offset_alone`, `an_alternate_comps_windows_keep_their_frames`, `a_window_the_edit_moves_itself_is_left_as_it_put_it` (6 of 7 failed); `undo_invariants` (fuzz invariant); `tempo_review3::a_tempo_edit_keeps_what_a_clip_plays_at_its_start` (the top bar, one undo). A punch-in is the same window (a take whose offset is its pre-roll): the bar-17 test |
| 1, 9 | the warp render key ignores the tempo before a non-following warp's window | **warp's file** (`warp_render::key`): relayed — hash `map.span_key(start − offset, end)` (or the bits of `offset_seconds`) when `!warp.follow` | — (relayed) |
| 2, 8, 17 | several windows moved, nudged, drag-copied or pasted across a change tore apart | one block: the earliest clip moves by the delta, every unwarped window lands the same seconds after it (`audio_time::block_start`; `clips::transpose_selection`, `clipboard::paste_clips`) | `tempo_review3::halves_moved_together_across_a_change_still_meet` (move, glue, nudge, drag-copy; mutation: failed, 7680 vs 9600), `halves_pasted_across_a_change_still_meet` (mutation: failed) |
| 3 | a linked paste across a change became unlinked | a linked paste keeps its window only (`relocate`'s fades and slip not applied to a linked copy) | `tempo_review3::a_linked_paste_across_a_change_stays_linked` (not seen failing: with the guard undone the paste still links, `same_origin` keeps the group; the guard keeps the group's fades and slip) |
| 4, 42 | a reversed waveform mirrored in ticks | `arrangement_audio::reversed_frame` (the start's frame plus the seconds from the tick to the end), as the engine and the pitch editor | `tempo_review3::a_reversed_window_is_drawn_as_it_plays` (mutation: failed, 360 037 vs 372 037) |
| 5 | a MIDI map dropped later left the song's later points when it collapsed to one tempo | `map_from` clears the tempo track then; `apply_tempo` without a map clears it too | `midi_tempo::a_one_tempo_file_dropped_later_replaces_the_songs_later_points` (failed) |
| 6 | `relocate` ignored a looping clip's period | a looping clip (`content_length < length`) keeps its ticks | `tempo_review3::a_looping_clip_keeps_its_ticks` (mutation: failed, length 1 920) |
| 7, 15, 48 | a metre chip drag swallowed a freeze's refusal | the drag goes through `exec_merging`, which says the refusal | `tempo_review3_ui::a_chip_drag_moves_by_its_travel_and_says_a_freeze` (written with the fix; not seen failing) |
| 10, 50 | DAWproject seconds audio offset at the clip start's tempo | the `playStart` second at the clip's start through the map (`audio_time::anchor`: offset in ticks, the rest in slip). Warp's review made the same change for a bare file (`ClipData::slip`); at the merge mine gave way to it | `dawproject_tempo::a_seconds_audio_clip_starts_at_its_second_through_the_map` (mutation: failed, 2.5 s; passes on warp's code) |
| 11 | comments and a test header described the old rule | `processor_rec.rs`, `audio_edits.rs` header rewritten | — |
| 12, 23, 38 | Alt+Shift+, cannot be typed on ES/DE/FR layouts | *Loop between markers* is **Alt+Shift+O** (a letter: the same key on every layout; free in every preset — the duplicate test); manual EN/ES, ADR | `keymap::loop_between_markers_fires_on_every_layout` (every preset; failed with the comma) |
| 14 | *Add a metre change* could open an editor out of view, later | the editor opens on the ruler where the change is drawn, in the pass it was asked (`metre_at`); a stale request is dropped; the top bar no longer opens it | `tempo_review3_ui::the_metre_editor_opens_where_it_is_seen` (written with the fix; not seen failing) |
| 16, 28, 30 | import words: "one time signature", "Keep: Keep", "pista" | `interop.midi_import.tempo_changes` "What to keep:" / "¿Qué conservar?", `metre_changes` reworded, `tempo.es` "franja de marcadores" | the i18n test |
| 18, 24 | the drag ghost showed what the drop did not keep | `clips::drag_preview` applies the commit's own commands (`clips::drag_commands`, shared with `commit_clip_drag`); the painter draws the landed clips | `tempo_review3::the_drag_draws_what_lands` (written with the fix; not seen failing) |
| 19 | forward split/trim tests allowed a frame | exact: the origin and slip asserted equal | `audio_edits::*` (coverage) |
| 20 | relocate's fades untested; the duplicate's tolerance 0.01 s | half a tick; a clip whose fades straddle the change moved and drag-copied (unlinked: a linked copy keeps its group's fades) | `tempo_edits::moved_fades_and_lengths_keep_their_seconds`, `a_duplicate_across_a_tempo_change_plays_the_same_seconds` (coverage) |
| 21 | the loop-over-a-ramp test was true by construction | a run that crosses the loop's end with overshoot at blocks 37/512/1000, each wrap against the closed form | `tempo_review2::a_wrap_inside_a_ramp_carries_its_overshoot_through_the_map` (mutation: the overshoot carried in ticks — failed, 1920.469 vs 1920.371) |
| 22 | the top bar's drag inside a ramp untested | coverage | `tempo_review3_ui::the_tempo_drag_inside_a_ramp_moves_its_start_by_the_travel` |
| 25, 35 | one metre: automation points snapped from content tick 0 | the content's own grid when the song has one metre (`auto_editor`: shift 0) | `tempo_review3_ui::one_metre_automation_snaps_to_its_content_grid` (written with the fix; not seen failing) |
| 26, 44 | same-tick tabs ran over the next marker; the rename field on a neighbour | a same-tick run shares the room before the next tick; a run too close behind another starts after it; the rename field at the drawn tab | `tempo_review3_ui::same_tick_markers_share_the_room_before_the_next` (failed: 436–442 under 439.8) |
| 27, 46 | *Add a marker here* off the grid | `arrangement::menu_point` snaps when snap is on | `tempo_review3_ui::the_ruler_menu_point_is_on_the_grid` (mutation: failed, 31 008) |
| 29 | the Spanish manual's menu names | "Cambiar el nombre…", "Quitar el marcador" (EN "Remove the marker") | — |
| 31 | a touchpad moved the tempo several BPM | raw wheel events summed, fractions carried into whole notches | `tempo_review3_ui::the_tempo_wheel_counts_whole_notches_and_leaves_ctrl_alone` (written with the fix; not seen failing) |
| 32 | a point dragged past the range stuck to the edge | `Axis::of_lane` slides lo/hi with the value, the scale held | `arrangement_tempo::tests::a_point_dragged_past_the_range_slides_it` (mutation: failed, 110–130) |
| 33 | the manual did not say what a tempo edit does to a trimmed clip | EN/ES: a tempo change never moves what a clip plays at its start | — |
| 34 | the first warp edit's identity at the song's first tempo | **warp's** (`warp_to_edit`): fixed by the warp builder (its review 1 merged: "a first edit starts from what plays") | — |
| 36 | a slip did not follow the pointer under another tempo in the hidden part | `audio_time::slipped`: the source moves by the seconds the travel lasts at the window's start (`clips::slip`, the reversed slip too) | `tempo_review3::a_slip_follows_the_pointer_at_the_window` (mutation: failed, 288 037 → 264 037) |
| 39 | a tab or chip jumped by its grab offset | both drag by the pointer's travel from the press (`grab`, `metre_drag`) | `tempo_review3_ui::a_marker_tab_moves_by_its_travel` (mutation: failed, 11 520), the chip test (mutation: failed, 19 200) |
| 40 | the gate's history numbered bars from tick 0 | `map.bbt` | not tested (drawing only) |
| 41 | the compressor's history counted bars from tick 0 | `bar_beat` through the map for lines, numbers and the hover | not tested (drawing only) |
| 43, 51 | MIDI *keep the time* placed notes at one tempo in a song with a tempo track | `MidiImport::seconds_bpm`; the notes placed through the song's map from the drop | `midi_tempo::notes_kept_at_their_seconds_go_through_the_songs_map` (failed) |
| 45 | the export dialog never found a range when markers shared a tick | `default_markers` takes the next marker on another tick | `tempo_review3_ui::the_export_dialog_finds_two_markers_that_make_a_range` (mutation: failed) |
| 47 | Ctrl/Shift+wheel changed the tempo | modified wheels left to the view | the wheel test |
| 49 | DAWproject `<Warps>` of unwarped audio at one tempo | **warp's file** (`dawproject/audio.rs`): relayed — see the report | — (relayed) |
| 52 | MIDI export dropped the map past the last clip | the end chains tempo points and metres; a point at the end is written | `midi_tempo::a_map_without_clips_round_trips` (failed) |
| 53 | a first tempo a few ticks in | within a sixteenth it is tick 0's, the song's tempo | `midi_tempo::a_first_tempo_a_few_ticks_in_is_the_songs` (failed) |
| 54 | any first metre inside a bar moved to 0 | only within a sixteenth; odd metres reported (`interop.report.odd_metres`) | `midi_tempo::a_pickups_metre_keeps_its_place` (failed) |
| 55 | the AI's ramp at beat 0 with no track dropped | `add_tempo_point` with no track at tick ≤ 0 and a ramp makes the track | `edit_tests::a_ramp_from_the_start_ramps` (failed), `bridge::the_agent_can_ramp_from_the_start_asked_first` |

**Relayed (not my files).** *Warp render key (1, 9):* in `warp_render::key`, when the warp
does not follow, hash `plan.map.span_key(plan.start − plan.offset, plan.end)` (or the bits
of `audio_time::offset_seconds(map, start, offset)`) instead of `span_key(start, end)`.
*DAWproject unwarped audio (49):* export an unwarped clip's natural line through the map —
`<Warp>` points at the content origin, at each tempo point and every sixteenth inside a
ramp, at `audio_time::source_seconds(map, start, offset, origin + b·PPQ) + slip`; on import
call a clip natural when every |s − (map.seconds_between(origin, origin + b·PPQ) + slip)|·rate
< 0.5, and place seconds-unit `<Warp>` times through the map from the clip's origin.

**Verified on the merged tree (`r29/integration` 6e74f7e in):** core, dsp, project, engine,
host, export, ai and plugins 2 185 passed; UI 1 455 passed; the engine with `fuzz`
(`CANTUS_WORKERS=3`) 787 passed; `cargo clippy --workspace --all-targets -D warnings` and
`cantus-plugins --no-default-features` clean; `cargo fmt --check` clean. No `Command` added
or changed (`freeze_guard::touches` untouched); no golden moved, `DSP_VERSION` unchanged.
One behaviour change beyond the tempo track: editing the tempo of a song *without* a tempo
track now keeps each trimmed unwarped clip's frame at its start too (round 28 slid it);
playback of every saved project is unchanged.

## Review 4 (`tempo-4.json`, 53 confirmed) — how each was fixed

Merged `r29/integration` (b68a07f, controllers' second fix round) first: a fast-forward.
Every test named below was seen failing before its fix — on the old code, or with the fix
undone in the tree (*mutation*) — except where the row says otherwise.

**The design, as the integrator directed (ADR 0060 §4 rewritten).**

1. *Re-anchor by take, not by window* (`tempo::keep`). Before a command that changes the
   map, the unwarped windows (clips and alternate comps' windows) are grouped into
   **takes**: one file, one direction, one *line* — the same source second at every tick
   within a frame (its frame at its entry less, or reversed plus, the song's frames to
   it). Each take keeps the frame where it comes in (forwards: its earliest window's
   start; reversed: its latest window's end — the frame a reversed sound lands on) and
   runs on from there through the new map; the windows that shared an origin and fine
   offset get one shift (one offset delta, one fine offset), so a split stays a split,
   `comp::is_seam` stays false and glue still glues. A change inside a take moves its later
   windows against the bars, never against each other. Left alone: looping windows (their
   period is in ticks, as moves keep it), windows whose file is gone (never refusing the
   edit), warps that follow the tempo. Clips are found by id once per edit and the
   sources' rates once (no lookup quadratic in a track's clips).
2. *A gesture is measured from where it began* (`CommandBus::rebase`). A merging command
   that changes the tempo — or merges into a step that did — applies the step's inverse
   (exact: the song before the gesture), runs the gesture's own commands again (the
   latest when it supersedes, else all; a widened controller gesture's from scratch) and
   lets `keep` measure once. A hundred frames equal one edit to the last value, a drag
   back to its start leaves every window as it was to the bit, the step holds one fix
   (no growth per frame), and redo after a widen is what the gesture did. A step keeps its
   gesture's own commands while it merges (`HistoryEntry::gesture`, dropped when sealed).
3. *Moves keep every clip's bars.* `audio_time::lay_chains`: only the windows of one take
   moved together (one row, one file, one direction; sharing origin and fine offset, or
   meeting end to start and playing on there) keep their shape in seconds — first window
   by the delta, the others the same seconds after it, *both edges* mapped, the shared
   origin kept exactly; every other clip lands its delta later and is relocated on its
   own. Used by `clips::transpose_selection` (drag, drag-copy, nudge) and
   `clipboard::paste_clips`.

No new `Command` (`freeze_guard::touches` untouched); no schema change; no golden moved.

| # | Finding | Fix | Test |
|---|---|---|---|
| 12, 15, 39, 46 | each window re-anchored alone: splits and comp seams of one take tore apart (repeat/skip, a seam, glue refused), even after a top-bar change | the take rule (1) | `tempo_keep_groups::a_take_cut_in_three_stays_whole_through_edits_before_and_inside_it` (lane take + three comp windows; an edit in the count-in and a ramp inside the take; failed: one origin), `the_songs_tempo_keeps_a_split_take_whole` (no tempo track, SetTempo; failed) |
| 11, 29 | a reversed window kept the wrong frame; its test measured the forward frame with no point inside | a reversed take keeps the frame it lands on (its end — the musical choice, ADR §4 (i) says so) and its windows stay on one reversed line | `tempo_keep_groups::a_reversed_take_keeps_the_frame_it_lands_on_and_its_split` (a point inside the right half; failed: 336 037 then 346 680); `tempo_keep`'s comment corrected |
| 10 | a looping window re-anchored through its first repetition | looping windows are left alone | `tempo_keep_groups::a_looping_window_keeps_its_ticks` (failed: offset 432) |
| 30 | a window whose file is gone made every tempo edit fail | such windows are left alone; the fuzz re-runs a refused tempo edit without the bus and asserts it is refused for itself | `tempo_keep_groups::a_window_without_its_file_never_refuses_a_tempo_edit` (failed: AudioSourceNotFound) |
| 27 | the comp test never checked the fine offset; the fuzz skipped comps; the linked reach was 160 frames | a comp window with a rest left; the fuzz checks every window (comps too) against the take rule, linked within half a tick at the song's slowest tempo | `tempo_keep_groups::an_alternate_comps_window_keeps_its_frame_with_its_fine_offset` (coverage: passed at once); `undo_invariants` (mutation: the old `keep.rs` fails seed 19) |
| 1, 8, 16, 23, 33, 41, 47 | merged gestures re-anchored frame by frame: rounding added up, slow drags lost the change, a drag back left the audio moved | the rebase (2) | `tempo_keep_groups::a_hundred_frame_drag_is_one_edit` (failed), `a_drag_back_to_the_start_leaves_everything_as_it_was` (fast up, slow back; top bar and lane; mutation: fails without the rebase); `undo_invariants`' gesture arm (a tempo command in four merged over 1–6 jittered frames, widened half the time, must equal its last frame as one edit; mutation: fails seed 9) |
| 2, 14, 19, 26, 48 | the step grew by a whole fix every frame; quadratic lookups | one fix per step (the rebase); clips by id, rates once | `a_hundred_frame_drag_is_one_edit` asserts the step holds ≤ 1 + 2 × windows commands (failed). A slip-only command was not added (the fix no longer accumulates; one `SetClipAudio` per moved window per frame remains) |
| 0, 9, 17, 24, 32, 44 | a widened controller gesture dropped the earlier re-anchors from redo | the widened step is rebuilt from the gesture's start | `tempo_keep_groups::a_widened_tempo_gesture_redoes_what_it_did` (undo, redo == do, == the last frame as one edit; failed); the fuzz's widen |
| 3 | three windows at an uneven ratio left 1-tick gaps | both edges of every chained window mapped; the shared origin kept | `tempo_review4::three_windows_at_an_uneven_ratio_still_meet` (move, drag-copy, paste at 120 → 100; failed on the old code) |
| 18, 25, 31, 40 | every unwarped clip of a selection laid by seconds from the earliest: separate clips off their bars | moves clip by clip, takes kept (3) | `tempo_review4::clips_that_are_not_one_take_keep_their_bars` (audio + MIDI + a separate audio clip: drag, drag-copy, nudge, paste; failed on the old code) |
| 5 | a cut and paste with *Link copies* on kept the old fades and dropped the fine offset | the group is found first; a copy that joins none takes the relocated fades and fine offset | `tempo_review4::a_cut_and_paste_with_link_copies_on_plays_what_it_played` (failed on the old code) |
| 28 | the linked-paste test could not fail | a trimmed clip (offset 1 001) with fades across the change; linked, `links::check`, the original untouched | `tempo_review4::a_linked_paste_of_a_trimmed_clip_with_fades_stays_in_its_group` (mutation: with the region given to a copy joining a group, the paste fails) |
| 6 | a reversed window's offset clamped at 0 after `anchor` stopped clamping | the clamp removed | `tempo_review4::a_reversed_clip_widened_keeps_what_it_showed` (failed) |
| 7, 22, 35 | the drag preview rebuilt and cloned the whole selection every frame, quadratically | built from the selection's unwarped audio alone (the move's clamps over all), by id, cached in the painter's scratch per (revision, anchor, result) | `tempo_review4::the_drag_preview_is_the_audio_alone_and_what_lands` (not run against the old code: it returned every clip, so it would fail by construction) |
| 4, 13, 20, 42 | a tempo edit during a take misplaced it | refused while a take records — `exec`, `exec_merging` (top bar, lane, tap, wheel) and a controller knob on the tempo (the other knobs of the gesture still move) — with `tempo.status.recording` (EN/ES) | `tempo_review4_ui::the_tempo_stays_while_a_take_records` (failed). The AI bridge is not wired into the application; its bus gets no such check |
| 43 | a tempo point dragged straight up jumped to the grid | the point moves along the lane by the pointer's travel (from the press), once past 4 points; snapped as the lane snaps | `tempo_review4_ui::a_tempo_point_dragged_straight_up_keeps_its_place` (failed: 7 680 vs 8 680) |
| 21, 34, 45 | *Add a metre change* panicked under 160 points | `min`/`max` (never `clamp`); the area keeps itself on screen | `tempo_review4_ui::a_metre_change_in_a_narrow_arrangement_opens_its_editor` (300 and 120 points; failed: the panic) |
| 36 | a same-tick run cut every name but the last even when all fit | the run is measured first; whole names while it fits | `tempo_review4_ui::a_run_of_markers_that_fits_keeps_every_name_whole` (failed on the old code: 68 of 150) |
| 37 | the round's visuals never rendered | `shoot_tempo` renders the ruler's metre editor (bar 1, the right edge, 420 points, 200 %), a run of markers before a close one, a take cut in three dragged with a MIDI clip across the change, a reversed take — night and paper, EN and ES (`tempo-r4-*`); looked at | — |
| 38 | the Spanish manual's sentence | rewritten (EN too) with the take rule, a reversed clip, the tempo held while recording | — |
| 49 | a MIDI file on a song with only a tempo track wiped it | `interop::empty_song`: a tempo track, metre changes or markers are content (the dialog's default and a drop, one rule) | `tempo_review4_ui::a_midi_file_on_a_tempo_track_alone_leaves_it_alone` (failed) |
| 50 | a file with no metre at bar 1 wiped the song's metre changes | only a file that states a metre replaces them | `midi_tempo::a_file_without_a_metre_leaves_the_songs_metre_changes` (failed) |
| 51 | imported clips sized in bars of the first metre | `TempoMap::bars_around` through the map the import leaves (the old arithmetic in one metre, to the bit); DAWproject notes on a lane through the file's map | `midi_tempo::imported_clips_end_on_the_songs_bar_lines` (failed: 10 560); the DAWproject lane-notes path is **not tested** |
| 52 | a DAWproject seconds loop reported moved | compared with `playStart` in the content's own unit (half a frame, half a tick) | `dawproject_tempo::a_looped_seconds_clip_across_a_change_reports_no_loop_moved` (failed on the old code: "Loop" reported) |

**Verified on the merged tree (`r29/integration` e1c1d71 in: freeze's second round,
schema 38):** core, dsp, project, engine, host, export, ai and plugins 2 227 passed; UI
1 528 passed; the engine with `fuzz` (`CANTUS_WORKERS=3`) 800 passed; `cargo clippy
--workspace --all-targets -D warnings` and `cantus-plugins --no-default-features` clean;
`cargo fmt --check` clean. Shots: `cargo run -p cantus-ui --example shoot_tempo -- <dir>`
(`tempo-r4-*`, looked at in `F:\projects\daw-r29\shots\tempo-r4`).

## Review 5 (`tempo-5.json`, 29 confirmed) — how each was fixed

Merged `r29/integration` (a5053f8: warp's second round, schema 39; isolation's third)
first. Every test named below was seen failing before its fix — on the old code, or with
the fix undone in the tree (*mutation*) — except where the row says otherwise.

| # | Finding | Fix | Test |
|---|---|---|---|
| 0, 9, 20 | a linked piece, re-anchored by whole ticks, fell out of its take and the next edit tore the take | `keep::before` joins a linked window to its take's line within half a tick at its entry (in frames) plus a frame; `lay_chains`' meeting test the same; the fuzz's `takes_kept` mirrors it, half a frame for windows sharing the head's origin | `tempo_keep_groups::a_linked_piece_stays_in_its_take_through_two_edits` (120 → 121.3 → 128; failed: the seam 372 037 then 381 974). The fuzz's random edits do not build this shape: with the slack undone it still passes (the unit test is the check) |
| 1, 2, 3, 10, 15, 21, 27 | undo and redo went around the hold | `CantusApp::undo`/`redo` refuse, while audio records, a step whose inverse/command changes the map (or puts another song in place), with the hold's word | `tempo_review5_ui::undo_and_redo_hold_the_tempo_while_audio_records` (undo, Ctrl+Z, redo; works after the take; failed) |
| 6, 16 | the hold locked the tempo during notes-only passes | the hold is `audio_take_running()` (an audio pass); `tempo.status.recording` says "while audio records" (EN/ES); the review-4 test now records audio | `tempo_review5_ui::a_notes_only_pass_lets_the_tempo_move` (failed); `tempo_review4_ui::the_tempo_stays_while_a_take_records` on an audio pass |
| 4 | a DAWproject import replaced the song under a take | the import stops the take first, as `replace_project` does | `tempo_review5_ui::a_dawproject_import_stops_the_take_first` (failed) |
| 5, 14, 22 | a tap said "tapped" over the refusal | `set_tempo_now` returns whether it took; *tapped* only then | `tempo_review5_ui::a_tap_while_audio_records_says_it_is_held` (failed) |
| 7 | a MIDI drop during the first take was refused whole | `interop::sets_tempo_by_default`: an empty song not recording audio (the dialog's default and a drop) | `tempo_review5_ui::a_midi_file_during_an_audio_take_keeps_the_tempo` (failed) |
| 8 | a refused tempo knob got no feedback | its feedback keys are forgotten, so the tempo is sent back | `tempo_review5_ui::a_refused_tempo_knob_is_sent_the_tempo_back` (failed) |
| 11 | no test held a moved take's shape in seconds | the test (move, drag-copy, paste; a take with a gap): the first window by the delta, every edge's frame within the half tick whole-tick edges allow, the seconds between starts kept | `tempo_review4::a_moved_take_keeps_its_shape_in_seconds` (mutation: windows laid by ticks — failed, 25 037 became 30 037) |
| 12 | each drag frame O(W²) through linear clip lookups | `Command::SetClipAnchors` — a track's windows re-anchored in one pass (offset and fine offset; refused for a missing or non-audio clip and for a linked clip's fine offset; exact inverse; `supersedes` the same windows); `keep` writes it; its undo-fuzz arm (`tempoed` kind 12) and freeze arm (`TrackSource`); `cmd.set_clip_anchors` EN/ES | `tempo_keep_groups::a_drag_over_two_thousand_windows_keeps_to_its_budget` (best of three, 3 ms a frame: 5.46 ms before, 0.81 ms now) |
| 13, 26 | the DAWproject lane-notes path never ran; the MIDI drop into a song's metres untested | the metre automation is read before the clips and the map they are read with holds it (`read::metre_list`, shared); lane notes use `bars_around` always (the old arithmetic with one metre) | `dawproject_tempo::notes_on_a_lane_get_a_clip_on_the_files_bar_lines` (4/4 → 7/8; a metre at bar 1 that differs from the transport's; failed: 11 520); `midi_tempo::a_clip_dropped_into_a_songs_metre_change_ends_on_its_bars` (coverage: the review-4 code was right) |
| 17 | a tempo point's drag measured travel from a screen x | the grab is a song position (`Drag::grab`), the dead zone measured where it is now | `tempo_review5_ui::a_tempo_point_stays_under_the_pointer_through_a_zoom` (failed: 731 vs 496) |
| 18 | the ruler's metre editor named no bar and covered the chips | titled `tempo.metre_at_bar` ("Metre at bar 14", EN/ES; `band_row_src`), opened under the ruler and the tempo lane; re-shot | `tempo_review5_ui::the_rulers_metre_editor_names_its_bar` (failed); shots `F:\projects\daw-r29\shots\tempo-r5` |
| 19 | the manual's reversed-clip sentence | EN/ES: "the point of its file it lands on" / "el punto de su archivo que suena en su final"; "what a take plays where it comes in"; the hold "while audio records", undo and redo too | — |
| 23 | a tab or chip released out of view kept its grab and its step open | a grab with the button up ends the drag (`markers::input`, `metre_input`) | `tempo_review5_ui::a_marker_released_out_of_view_ends_its_drag` (failed: one step, not two) |
| 24 | the *Bar* grid stepped four quarters | nudges (clips and notes) step a bar of the metre; a duplicate of notes the fewest whole bars that hold it; a drawn or painted note and a resize's minimum `bars::grid_len` | `tempo_review5_ui::the_bar_grid_steps_a_bar_of_the_metre` (3/4: nudge and duplicate; failed: 6 720). The piano roll's drawn length is not tested |
| 25 | the warp marker hint's speed at the song's first tempo | **warp's file** (`arrangement_warp.rs`): relayed | — (relayed) |
| 28 | far tempo points hung a MIDI export; the AI's unsaid ramp cleared one | `tempo::HORIZON` (a million beats): the AI's `set_tempo_point` refuses past it and keeps a point's own ramp when unsaid; a DAWproject's points, metres and markers past it are left out (`tempo.report.too_far`, EN/ES); a ramp exports in at most 4 096 runs of sixteenths, each its run's average tempo | `bridge::a_far_point_is_refused_and_an_unsaid_ramp_is_kept` (failed), `dawproject_tempo::points_past_the_horizon_are_left_out_and_said` (failed), `midi_tempo::a_very_long_ramp_exports_in_bounded_steps` (mutation: 50 001 events) |
