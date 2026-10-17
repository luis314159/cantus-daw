ENGINE READY 0dd848a

# Round 29 — tempo: the tempo track, metre changes and markers

*(In progress. Parts 1–3 — the model (schema 33), the real `TempoMap` and the engine
following it — are committed and green at `0dd848a`: the warp builder's part B and the
wave-2 builders may start from them. The interface, interchange and the rest follow.)*

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
