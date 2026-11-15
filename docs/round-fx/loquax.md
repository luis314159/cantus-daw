# The effects side round, wave A — `loquax`: Loquax, the vocoder (DSP phase)

Branch `fx/loquax` from `fx/integration` d3b51a09. Name provisional: `cantus_dsp::loquax::NAME`.
No `EffectKind` yet — this phase is the DSP alone (`crates/cantus-dsp/src/loquax*.rs`, one
`mod` line in `lib.rs` marked `// fx-a:loquax`), its measured tests and offline sounds. FL
Studio's Vocodex is the bar; Arturia Vocoder V, Waves OVox/Morphoder, the Roland VP-330 and
SVC-350 and the EMS Vocoder 2000 the character references — read about, nothing copied.

## Done

| File | What |
|---|---|
| `loquax.rs` | the unit: `LoquaxSettings`, `Source`, `Loquax::{new, set, reset, process, latency, at_rest, rest_blocker, skip, bands, telemetry}`, `BandView`, telemetry `t::*` |
| `loquax_run.rs` | the sample loop and the banks' tuning (cached by layout; retuned every 64 samples while a glide moves) |
| `loquax_bank.rs` | Butterworth band-pass design (order 4/6/8, centre exact, edges by Newton), the band layout with a 50 Hz floor, the structure-of-arrays bank of linear TPT state-variable sections, a two-pole SVF |
| `loquax_carrier.rs` | the polyphonic carrier: 16 voices + the robot's drone, saw/pulse/noise, 1–7 unison copies, polyBLEP, attack/release, held keys kept while disengaged |
| `loquax_voicing.rs` | the voiced/unvoiced detector, the sibilance high-pass (4th order) and the gate |
| `loquax_measure.rs` | the speech-like test phrase; NCM (Goldsworthy and Greenberg 2004, STI octave weights); spectral-envelope correlation; Goertzel response |
| `loquax_tests.rs`, `loquax_tests2.rs` | 22 tests (below) |
| `examples/loquax_sounds.rs` | the listening examples and the CPU measure |

## The DSP API (what the engine unit will call)

```rust
let mut q = Loquax::new(sample_rate);
q.set(&settings);                      // allocation-free, any time between blocks
q.process(&mut l, &mut r, side, notes); // side: Option<(&[f32], &[f32])>, notes: &[NoteEvent]
q.latency();                           // 0
q.at_rest(); q.rest_blocker(); q.skip(frames); q.reset();
q.telemetry(&mut [f32; 64]);           // the family's header + t::*
q.bands(&mut BandView);                // per band: centres, modulator/carrier/output dB
```

`NoteEvent { frame, kind: On { key, velocity } | Off { key } | AllOff }`, frames within the
block in order. Notes are always taken (held keys tracked at mix 0); a held key keeps the
unit from rest, so the engine must deliver note events to a resting unit (or wake it).

## Measured

- **Band response**: every band of 8/16/32/64-band banks, orders 4/6/8, at 44.1/48/96 kHz,
  is the analytic Butterworth band-pass within **0.0027 dB** down to −60 dB; centres 0 dB
  (±0.02); edges −3.01 dB within **0.0056 dB**. An octave above the edge: −32.3 / −48.5 /
  −64.7 dB (orders 4/6/8). Summed power of the bank at width 1: ripple 0.73 dB (16 bands),
  0.69 dB (32).
- **Intelligibility proxies** on the speech-like phrase (chord C3 G3 C4 E4): NCM 8 bands
  0.50, 16 → 0.74, 32 → 0.72, 64 → 0.66 (saturates at 16–32, as the vocoded-speech
  literature reports; at 64 a chord's sparse partials leave narrow bands to noise); a
  carrier shaped by noise instead of the voice: 0.00. Consonant help: the 8 kHz octave's
  envelope correlation 0.91 → 0.95. On the real voice (Gettysburg excerpt): 16-band robot
  NCM 0.72, 32 bands 0.71 (0.69 without the consonant help), formant ±5–6 st 0.50–0.54,
  choir 0.57; spectral-envelope r 0.95–0.98.
- **Formant shift**: centroid 466 / 731 / 1022 Hz at −7 / 0 / +7 st, pitch unchanged
  (110 Hz within 1 Hz). Level: the vocoded phrase −1.5 dB from the voice (flatten 1).
- **CPU** (release, one core, 48 kHz, speech + four-note chord with unison 3 — measured
  while another builder compiled on the machine): order 4 — 1.6 / 2.2 / 4.2 % of real time
  at 16 / 32 / 64 bands; order 6 — 2.0 / 2.8 / 4.7 %; order 8 — 2.1 / 2.9 / 4.5 %. Worst
  case, formant and range automated every block: 4.6 % (32 bands) / 8.5 % (64).
- **Latency** 0. **Rest**: 3.3 s into a silence from the busiest settings.

## Tests and what each holds

- `the_defaults_touch_no_sample` / `at_mix_zero_every_other_control_moved_is_still_a_null` —
  the null to the bit (−0.0 kept), with notes and a sidechain.
- `every_band_is_the_butterworth_band_pass_it_was_designed_as`, `the_slopes_are_their_order`,
  `the_bank_sums_flat_at_width_one`, `a_new_band_count_relayouts_…`.
- `block_size_does_not_change_a_sample` — 1/37/64/512 at tolerance 0, glides and retuning
  running, notes on, off and all-off mid-block, sidechain and synth both.
- `it_comes_to_rest_and_its_rest_is_a_new_unit` — the exact rest twin; `a_held_key_…`.
- `a_value_that_is_not_a_number_is_silence`.
- `the_output_follows_the_voice_and_more_bands_carry_more`, `the_consonants_come_through`,
  `the_detector_tells_a_vowel_from_a_sibilant`, `a_formant_shift_moves_the_voice_and_not_the_notes`,
  `a_band_shift_moves_a_band_to_its_neighbour`, `hold_keeps_the_vowel_after_the_voice_stops`,
  `the_robot_drones_its_note_while_the_voice_speaks`, `a_sidechain_carries_and_swap_trades_the_roles`,
  `the_vocoded_voice_is_about_as_loud_as_the_voice`, `spread_sends_alternate_bands_left_and_right`,
  `the_synthesiser_steals_the_oldest_voice_and_a_zero_velocity_releases`.

## Sounds

`C:\projects\daw-fx\compartir\loquax\sound\` — FLAC, README with sources, measures and what to
listen for. Voice: *Gettysburg Address* read by Britton Rea, Wikimedia Commons, public-domain
dedication by its author (not in the repository). Sung voice: our synthetic singer.

## Verified only in emulation / needs the owner

- The sound, by ear. No allocation on the audio path is by construction (stack arrays,
  no `Vec` in `process`/`set`); `rt_memory.rs` proves it once the engine unit exists.

## Left (later phases)

Core params (`EffectKind`, PARAMS keys incl. the 64 band levels), the engine unit
(`TAKES_SIDECHAIN` and `TAKES_NOTES`, note delivery to a resting unit), the null and contract
tests in `cantus-engine`, `rt_memory`, the panel, presets, export, docs, e2e — see the report.
