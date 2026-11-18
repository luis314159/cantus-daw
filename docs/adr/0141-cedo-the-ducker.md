# ADR 0141 — Cedo, the ducker

**Status:** proposed (fx side round wave B) ·
**Date:** 2026-11-17 · **Schema:** 75 ·
**Relates to:** ADR 0004 (commands), ADR 0018 (null defaults), ADR 0022 (an effect's
sidechain and `midi_from`), ADR 0031 §5.1 (at rest), ADR 0055 (Fluctus's shapes), ADR 0071

## Context

Ducking in Cantus meant a compressor keyed by another strip. Producers want the pump as a
shape (FL's Fruity Limiter and Peak Controller, LFOTool, Kickstart, VolumeShaper — function
only), broadcast wants a voice-over ducker. The DSP was built first
(`docs/round-fx/cedo.md`): a sample-exact envelope fired by the key (hold from its last
sample over the threshold, an S attack), a drawn shape on the song's grid or on notes, a
centred edge kernel, band ducking split at minimum or linear phase, lookahead; CPU ≤ 1 %.

## Decision

A new effect kind, `EffectKind::Ducker`, model `cantus_core::fx::cedo`, engine unit
`cantus_engine::fx::cedo`, DSP `cantus_dsp::cedo`.

- **Parameters** (24 keys, every default a null — depth 0 ducks nothing, to the bit, at
  every trigger): the trigger, depth (80 dB is silence, shown ∞), threshold, attack, hold,
  release and its shape, the key's filter and detector, lookahead, the shape's cycle, swing
  and edge, the band, its split and crossover, link, mix, out, listen, velocity.
- **The shape is content** (`EffectSpec::cedo`, at most 64 points with the automation
  editor's segment shapes; `None` is the classic pump and is not written).
  `Command::SetCedoShape` replaces it, sanitized, its inverse the shape it replaced; a
  hand-edited shape is repaired on load (schema 75).
- **Its three triggers use the routes the engine has**: the sidechain is the slot's key
  (another strip's post-fader output); Tempo reads the song's position each pass; MIDI
  plays the notes of the track the slot listens to (`midi_from`, another track, ADR 0022).
  No new routing was needed.
- **The rest check sees the key**: a keyed insert whose key strip sounded this pass keeps
  its strip awake (`graph::strip::may_rest`), so a ducker whose own input is silent while
  its key plays never rests wrongly; tested through the engine with Listen at Key.
- **Latency** = lookahead + the linear-phase kernel's at its **floor**, and **minimum
  phase is the default split** (no latency). The floor was first one constant (60 Hz,
  36.8 ms); the owner chose (2026-11-17, phase B below) **Auto** as the default — an
  octave under the crossover, committed when the crossover is let go — with fixed floors
  of 40, 60, 80 and 120 Hz for a constant latency (`cantus_dsp::cedo::band::Floor`, the
  table and the Auto rule that replaced `LIN_MIN_HZ`).
- **Starting preset** (ADR 0071): *Classic pump* — Tempo, quarters, 9 dB: it sounds on any
  track with no routing.

## Phase B (2026-11-17): the editor's pointer, presets, export, the floor (sheet v1 approved as shown)

- **The display answers the pointer** (`views/fx/cedo_edit.rs`), every edit one step of undo
  (a drag one gesture): the shape's points dragged (snapped to the screen's sixteenths,
  twelfths for a triplet; Shift free), a drag elsewhere on the plot bends the segment under
  it (up bulges it up whichever way it runs), a double-click adds or takes away a point, a
  right-click turns its segment Curve → S → Hold; in Sidechain the attack's, hold's and
  release's handles dragged in time and the threshold's line up and down the history; the
  band strip's crossover dragged and its split's word clicked; the trigger's flag on the bed
  clicked to the next trigger; the level gauge and its bellows dragged for the depth. The
  pilot lamp only shows. The flag, the split and the gauge offer their parameter's menu.
- **The shape everywhere a preset goes**: a preset's shape rides in its flat map
  (`cedo::to_flat` / `shape_of_flat`, keys `shape.n`, `shape.{i}.x|y|bend|seg`, no
  parameter's key starts `shape.`), through the factory files, the user's presets, A/B, the
  drift mark (compared without allocating, `flat_is`), the audition and the agents.
- **Seventeen factory presets** (`presets/effects/ducker.toml`), every trigger and band,
  five drawing shapes of their own; a Sidechain or MIDI preset is auditioned keyed by the
  material's drums.
- **The linear phase's floor** (the owner's decision): new parameters `lfloor` (Auto, 40,
  60, 80, 120 Hz; default Auto) and `lfhz` (Auto's floor as last committed, 40 … 1 000 Hz,
  default 60 — the default 120 Hz crossover's). Auto's floor is **committed, not derived**,
  so the latency is a function of the spec (offline = live, a render from any bar) and moves
  only at a release: the editor's host commits `lfhz = auto_floor(xhz)` into the crossover
  drag's own undo step at its end (`DawHost::end_gesture`, a board's Cedo pedals too;
  `PluginHost::end_gesture` inside the same host gesture), and with any discrete set of the
  crossover (a typed value, a preset naming none, an agent's `set_effect`). A crossover a
  host automates, or one dragged and still held, never moves the latency; under the floor in
  force it is heard at the floor. Measured at 48 kHz: 40 Hz 2 655 samples (55.3 ms), 60 Hz
  1 765 (36.8 ms), 80 Hz 1 325 (27.6 ms), 120 Hz 880 (18.3 ms); each choice shows its
  latency on the Band card. The default output is unchanged (the DSP's goldens hold), so no
  `DSP_VERSION` bump and no sound re-rendered. Schema 75 is wave B's own and unreleased: the
  two keys join it (null defaults; every v75 file loads).
- **Exported** as `Cantus Cedo` (CLAP and VST3): the sidechain input as the key, the host's
  transport for the pump, its notes (a CLAP note port in both dialects, VST3's event bus),
  the shape as content in the state (`ContentPost<Shape>`, copied from fx/wave-a's
  generalisation of Lapsus's post), the latency reported and re-reported on a committed
  floor (Lenis's pattern: CLAP's restart, VST3's `kLatencyChanged` from the UI tick on
  Windows).
- **The agents** draw the shape (`set_effect`'s `shape`: points with `x`, `y`, `bend`,
  `seg`; `null` the classic pump) and choose the notes' track (`notes_track_id`, for any
  effect that hears notes) — as Fractus's `lanes` set its content.
- **A Tabula pedal**: Cedo on a board pumps on the board's beat, ducks by a keyed board's
  key or fires on its notes; `Command::SetCedoShape` names its pedal (`pedal`, as
  `SetLapsusPattern` does), so the shape drawn in the pedal's editor is the pedal's own.
- **Its site card** (`web/src/units_data.rs`, EFFECTS 37).

## Consequences

- The pump is a function of the song's sample: a render from a bar line hears what the
  whole render hears there (tested from bar 5 in the engine, bar 3 on the boot path).
- Auto's floor is state (`lfhz`), not a rule applied on the fly: a crossover automated by
  a lane or by a host keeps the floor last committed, so its latency never jumps under the
  automation; the price is that such a crossover below the floor is heard at the floor
  until it is set by hand. The fixed floors are for whoever needs one latency always.
- A board preset does not yet carry a Cedo pedal's shape (as it does not carry a Lapsus
  pedal's bank): the board's flat map holds values and structure only.

## What the tests hold

`cantus-core`: the shape's sanitizing and its flat map
(`a_shape_rides_a_flat_map_and_comes_back`), the undo fuzz arms for `SetCedoShape` (on a slot
and on a board's Cedo pedal), `command_board_tests` (a pedal's shape round-trips, refused on
another kind);
`cantus-dsp`: `cedo::tests_floor` (each floor's latency, Auto's equal to the fixed floor's to
the bit, a fixed floor constant whatever the crossover, a crossover under the floor heard at
it, the A-2 accuracy at every floor, a floor changed while it runs heard as the input delayed
by the new latency to the sample);
`cantus-project`: `schema_65_keeps_wave_b_s_kinds_and_cedo_s_shape`;
`cantus-engine/tests/fx_cedo.rs` (the null at every trigger, the starting preset at every
block size, live and from bar 5, the sidechain from another strip, a silent input with a
playing key, the notes of another track, the lookahead and the linear phase compensated,
forks), `fx_contract` (split and at rest), `rt_memory::cedo_never_allocates`.

Phase B adds: `fx_cedo::as_a_pedal_it_is_the_insert_alone_on_the_beat_and_keyed` and the
floors' rows in the latency test; `cantus-ui/tests/cedo_drag.rs` (the shape's points, bends,
adding and removing, a segment's shape; the envelope's handles and the threshold; the
trigger on its card and flag; the crossover and the split; the gauge;
`dragging_the_crossover_moves_the_latency_only_at_the_release`), `cedo_e2e.rs` through
`CantusApp::assemble` (the pump to the sample at each beat, live = offline, an edit undone
to the bit, from bar 3, only the lows ducked, a kick strip keying the bass, a muted track's
notes firing it, the rest check seeing the key, the null with the key and notes routed; the
linear phase against a dry twin to the sample at the Auto and the 40 Hz floors; the presets),
`learn_everywhere::every_cedo_trigger_offers_its_stepped_parameters`, `preset_audit`; the
plug-ins' `clap.rs`, `vst3.rs`, `notes.rs`, `transport.rs` (the key, the latency and its
committed floor, the shape in the state, the notes in both dialects and VST3, the host's
tempo); `cantus-ai/tests/tools_music.rs::an_agent_draws_cedo_s_shape_and_routes_its_notes`.
