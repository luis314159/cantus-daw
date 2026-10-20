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
   name); the top bar at the playhead; keys Alt+M, Alt+, Alt+., Alt+Shift+L, Alt+Shift+M,
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
