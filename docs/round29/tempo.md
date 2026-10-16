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
