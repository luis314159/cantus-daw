# Cedo — ducker

*Effect kind `Ducker`. References for function: the sidechain compression and "LFO tool"
ducking producers do with FL Studio's Fruity Limiter and Peak Controller, Nicky Romero's
Kickstart, Cableguys' VolumeShaper and broadcast voice-over duckers — read about and
listened to, nothing copied; the design is our own (`docs/round-fx/cedo.md`, ADR 0141,
provisional number).*

*Cedo* is Latin for "I make way". It ducks a track three ways:

- **Sidechain**: the sound of another strip is the key — a kick, a voice. The key through
  an optional 24 dB/oct high-pass or a band-pass, its sample peak or its RMS against a
  threshold; the duck attacks along an S over exactly the attack, holds from the key's
  **last** sample over the threshold (a kick's low half-cycles under it never let go
  early), and lets go over exactly the release in its shape (linear, exponential,
  S-curve). Up to 10 ms of lookahead: the audio waits, the detector does not.
- **Tempo**: a **shape you draw** over one cycle of a division (8/1 … 1/32, dotted,
  triplets) plays on the song's own sample grid — cycle `j` starts on song sample
  `round(x_j · len)`, swing moving every second one late — so the pump is a function of
  the song's sample: the same in every render, from any bar line, at any block size.
- **MIDI**: another track's notes fire the shape once, a cycle long; the velocity can scale
  the depth in decibels.

The amount the shape (or the envelope) gives is a gain `G = 1 − a·mix·(1 − 10^(−depth/20))`
— linear in the amount, so the shape drawn is the level heard; 80 dB of depth is silence
(shown ∞). Its edges are smoothed by a centred Hann kernel (no latency, no splatter:
−97 dB). It acts on the whole sound or only the **lows** or the **highs**, split at
**minimum phase** (two shelving sections whose depth moves, no bump, no latency) or **linear
phase** (a sharpened B-spline in integers: the bands sum to the delayed input exactly).

**The linear phase's floor** (the owner's choice, 2026-11-17) is what its latency is sized
for. **Auto** (the default) puts it an octave under the crossover, so the latency is always
the least the crossover needs; the floor is *committed* when the crossover is let go (end of
a drag, a value typed, a preset, an agent's set), so the latency never moves during a drag —
a host is told once per release. The fixed floors keep one latency whatever the crossover:

| Floor | Latency at 48 kHz |
|---|---|
| 40 Hz | 2 655 samples, 55.3 ms |
| 60 Hz | 1 765 samples, 36.8 ms (Auto at the default 120 Hz crossover) |
| 80 Hz | 1 325 samples, 27.6 ms |
| 120 Hz | 880 samples, 18.3 ms |

Every floor keeps the A-2 accuracy: the lows −23.92 dB an octave under the floor (24 dB of
depth, crossover an octave over it), the highs leaking −0.23 dB an octave under the
crossover (`cantus_dsp::cedo::tests_floor`). A crossover under the floor in force is heard at
the floor.

**The null**: a depth of 0 (the default), a mix of 0, or no trigger hands every sample on
untouched — to the bit, delayed by exactly the latency. CPU ≤ 1 % of a core.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/cedo.rs` (26 keys; the shape as content, its flat-map form for presets) |
| DSP | `crates/cantus-dsp/src/cedo.rs`, `cedo_run.rs`, `cedo_env.rs`, `cedo_shape.rs`, `cedo_band.rs` (the floors); tests `cedo_tests*.rs`, `tests/cedo_alloc.rs` |
| Engine glue | `crates/cantus-engine/src/fx/cedo.rs` (settings, the shape's points, the telemetry) |
| Editor | `crates/cantus-ui/src/views/fx/cedo.rs` (the window, the cards), `cedo_display.rs`, `cedo_engine.rs` (the beam engine), `cedo_screen.rs`, `cedo_trace.rs`, `cedo_plate.rs`, `cedo_art.rs`, `cedo_view.rs`, `cedo_edit.rs` (the pointer) |
| Strings | `crates/cantus-ui/locales/fx/cedo.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/ducker.toml` (seventeen); starts on *Classic pump* |
| Tests | `crates/cantus-engine/tests/fx_cedo.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/cedo_e2e.rs`, `cedo_drag.rs`, `fxb_whole.rs`, `learn_everywhere.rs`; the plug-in suites (`clap.rs`, `vst3.rs`, `notes.rs`, `transport.rs`) |
| Exported | `Cantus Cedo` (CLAP and VST3) |

## The window

A Victorian beam engine on its bed beside a dark screen, over a row or two of cards:

- **The engine**: in Tempo and MIDI a **cam** on a spoked flywheel whose profile *is* the
  drawn shape, turning with the cycle; in Sidechain a push **solenoid** fed by the key. The
  walking beam squeezes a **bellows** as the track is ducked, and the **level gauge** beside
  it reads the decibels — drag the gauge (or the bellows) down for a deeper duck, up for
  less (Shift for fine).
- **The bed**: the cast nameplate, the readout flags (the trigger — click it for the next
  one — the cycle or the key's level, the reduction now, the latency) and the pilot lamp
  that flashes on every hit.
- **The screen**, dark in every theme: in Tempo and MIDI the shape over one cycle (drag a
  point — snapped to the screen's sixteenths, Shift free — drag between points to bend the
  curve, double-click to add or remove a point, right-click a point to turn its segment
  curve → S → hold), with what is ducked now swept in by the playhead; in Sidechain the
  envelope on a hit, its attack, hold and release handles dragged in time. Under it the
  last four seconds of reduction with the hits — in Sidechain the key's level against the
  threshold's line, dragged up and down — and in Lows or Highs the band strip: the split's
  response now and at full depth, the crossover dragged along it, the split's word clicked
  to switch minimum and linear phase.
- **The cards**: Trigger (the switch, the key's strip or the notes' track, the trigger's
  own controls), Envelope and Key filter in Sidechain, Band (Whole, Lows, Highs; the
  crossover; the split; the **linear-phase floor**, each choice with its latency), Output
  (depth, link, mix, output, Listen: Output, Delta, Key).

Every edit is one step of undo; each control's menu offers automation and MIDI learn.

## Presets

Seventeen, fitted to the dry's loudness (`preset_tool fit`): *Classic pump* (the starting
preset: Tempo, quarters, 9 dB — it sounds with no routing), *Half-time pump*, *Eighths
pump*, *Swing pump*, *Triplet breathe*, *Trance gate*, *Reverse swell*, *Lows-only pump*,
*Linear-phase lows pump*, *MIDI pump*, *Kick ducks the bass*, *Kick on the bass lows*,
*Hard sidechain*, *Voice-over*, *Podcast music bed*, *Gentle radio*, *Snare room*. A preset's
shape rides in its values (`"shape.n"`, `"shape.0.x"` …), so it travels the menus, the
browser, A/B, a user's own presets and the agents. The Sidechain and MIDI presets are
auditioned keyed by the material's drums. Roles: bus, creative, vocal, drums, master.

## Exported

`Cantus Cedo` in CLAP and VST3, bit-identical to the DSP (the Direct path in
`tests/common`):

- **The key**: the plug-in's sidechain input when its switch is on the external key
  (`clap::cedo_s_external_key_ducks_the_input_only_when_asked`,
  `vst3::cedo_s_key_is_the_vst3_sidechain_bus`).
- **The tempo**: the host's transport (CLAP's `clap_event_transport`, VST3's
  `ProcessContext`), the pump on the host's beat to the bit
  (`transport::cedo_s_pump_follows_the_hosts_tempo_with_its_drawn_shape`,
  `vst3::cedo_s_pump_follows_the_hosts_tempo_through_vst3`).
- **The notes**: a CLAP note port (CLAP's own events and MIDI bytes) and VST3's event bus,
  each note at its sample (`notes::clap_notes_arrive_at_their_sample_in_either_dialect`,
  `notes::vst3_notes_arrive_at_their_sample_on_the_event_bus`).
- **The shape** is content in the state, as the project saves it, posted to the audio
  thread without a lock or an allocation (`ContentPost<Shape>`, Lapsus's post made generic)
  (`clap::cedo_s_shape_travels_in_the_state`).
- **The latency** (lookahead plus the floor's kernel) is reported before activation; a
  crossover the host moves never changes it, the floor committed does — once: CLAP asks for
  a restart, VST3 gets `restartComponent(kLatencyChanged)` from the UI thread's tick (on
  Windows; elsewhere at the next activation)
  (`clap::cedo_reports_its_latency_and_a_committed_floor_re_reports_it`,
  `vst3::cedo_s_latency_follows_its_committed_floor_through_vst3`). In the plug-in's own
  editor a crossover let go commits the floor inside the same gesture.

Its ids are blessed in `crates/cantus-plugins/tests/golden/ids.txt`.

## A pedal on Tabula

Cedo may be a pedal: a pump or a gate on the board's beat, a duck by the board's key when
the board's slot is keyed, or by the notes the board hears. Its face shows the trigger, the
depth, the cycle and the threshold; the shape, the envelope and the band stay on its
editor's screen, opened from the pedal — the shape drawn there is the pedal's own
(`Command::SetCedoShape` names its pedal). A board of one Cedo is Cedo inserted alone, to
the bit, on the beat and keyed, live as offline
(`fx_cedo::as_a_pedal_it_is_the_insert_alone_on_the_beat_and_keyed`).

## Ours, better

- Sample-exact: the duck lands on the beat's own sample, the hold counts from the key's last
  sample over the threshold, and a render from any bar line is that bar of the whole.
- A drawn shape and a sidechain envelope in one unit, the shape heard as drawn (the gain is
  linear in it) with click-free edges and no latency.
- Band ducking at minimum or linear phase, its latency the least the crossover needs, never
  moving while you drag.
- The machine shows the duck as it happens: the cam is the shape, the beam the amount.

## Left for the owner's ear

- The linear-phase floor's default rule (Auto, an octave under the crossover) and the fixed
  floors are the owner's choice of 2026-11-17; the latencies are measured, the sound is his
  to judge.
- The presets are fitted for level; their character is our reading of what each is for.
