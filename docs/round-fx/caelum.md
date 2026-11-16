# The effects side round, wave A — `caelum`: Caelum, the shimmer reverb (DSP phase)

Branch `fx/caelum` from `fx/integration` d3b51a09. **DSP only**: no `EffectKind`, no core
parameters, no engine unit, no panel yet. The name is provisional — one constant,
`cantus_dsp::caelum::NAME`. References for *function* (inspired, never copied): Valhalla
Shimmer and SuperMassive, Strymon BigSky's Shimmer/Cloud/Bloom, Eventide Blackhole,
Soundtoys SuperPlate. What Caelum adds to Cantus's reverbs (Spatium's rooms, Aula's
convolution, the legacy Reverb): pitch-shifted feedback, infinite washes, bloom, freeze,
modulated diffusion and ducking.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/caelum.rs` (the unit, its settings, meter, rest), `caelum_input.rs` (pre-delay, bloom, diffusers, freeze gate), `caelum_tank.rs` (the FDN, absorption, infinite, freeze, hold, governor, the shimmer's exchange), `caelum_shift.rs` (the shifter), `caelum_parts.rs` (sine table, filters, all-pass), `caelum_presets.rs` (the starting preset and eleven factory spaces); tests `caelum_tests.rs`, `caelum_tests_measure.rs`, `cantus-dsp/tests/caelum_alloc.rs`; example `cantus-dsp/examples/caelum_sounds.rs` | `cantus-dsp/src/lib.rs`: one line, `pub mod caelum; // fx-a:caelum` |
| Docs | this file | — |

### The DSP API

```rust
use cantus_dsp::caelum::{Caelum, CaelumSettings, CaelumMeter, Lines, presets, sync_ms};

let mut c = Caelum::new(sample_rate);        // allocates here only
c.set(&CaelumSettings { ..presets::starting() }); // every snapshot; redesigns only on change
c.process(&mut left, &mut right, key);       // key: Option<(&[f32], &[f32])> — the ducking's
c.meter();                                   // CaelumMeter, refreshed every block
c.latency();                                 // 0
c.at_rest(); c.rest_blocker(); c.skip(n); c.reset();
c.design();                                  // the tank's TankDesign (achieved_t60(hz), …)
sync_ms(beats, bpm);                         // a synced pre-delay, ms
```

`CaelumSettings` (real units): `size` 0…1 (lines of 15 → 320 ms on average), `decay_s`
0.3…60, `infinite`, `predelay_ms` 0…1000, `diffusion` 0…1, `high_hz` + `high_decay`
(the highs' T60 as a fraction of the middle's), `low_hz` + `low_decay` (a multiple),
`mod_depth` 0…1 (to 4 ms) and `mod_rate_hz`, `shimmer` 0…1, `pitch_a` and `pitch_b`
(semitones, ±36), `pitch_b_level` 0…1, `shimmer_low_hz` / `shimmer_high_hz` (the tone in
the loop; ≤ 20 and ≥ 20 000 are off), `bloom_ms` 0…4000, `freeze`, `duck_db` (0 off),
`duck_thr_db`, `duck_att_ms`, `duck_rel_ms`, `low_cut_hz` / `high_cut_hz` (the wet's tone),
`width` 0…1.5, `lines` (`Eight`, `Sixteen`), `mix` (Spatium's law: dry at unity to 50 %).
**`Default` is the null** (mix 0); `presets::starting()` is what an insert should start
with (ADR 0071), `presets::factory()` the eleven spaces with names and one-line
descriptions.

`CaelumMeter`: wet peaks, input peak, ducking's reduction (dB), freeze 0…1, bloom 0…1, the
tank's energy (dB), the shimmer's share of it, the governor's trim (dB), pre-delay, mean
line length and modulation depth now (ms), keyed, and the achieved decay at eight octaves.

### How it works (each file's header says it in full)

```text
       ┌──────────────────────────── dry ────────────────────────────────────────┐
 in ─┬─ pre-delay ─ bloom taps ─ diffusers ─ freeze gate ─ tank ─ wet tone ─ width ─ duck ─ mix ─ out
     │                                          (8 or 16 lines, ↺ shimmer)       │
     └─ (or the sidechain key) ─ ducking detector ───────────────────────────────┘
```

- **The tank**: 8 or 16 delay lines (distinct primes spread over 1.2 octaves around the
  size's mean, ±4 % jitter), each read through an **all-pass interpolator** whose length a
  slow sine modulates (lossless, so the chorus costs the decay nothing), then **two
  Schroeder all-passes in the line** (3–12 ms and 1–4 ms, gain up to 0.7 with Diffusion —
  what makes a vast tank dense), then **Jot's absorption**: a middle gain and two
  first-order shelves (high damping, low damping), each line losing `60·L/(fs·T)` dB a pass
  in each band's own T, `L` its whole loop. The matrix is `P·H·D` (signs, Walsh–Hadamard,
  a permutation): orthogonal and not an involution. **Early taps** read each line a second
  time part of the way along into two more output rows, so a vast tank speaks before its
  shortest line has gone round.
- **The shimmer** (four channels: voice A on two orthonormal rows of the matrix, voice B on
  two more): each channel's component `s` of the lines is replaced by `√(1−a)·s + √a·z`,
  `z` the shifted `s` through the loop's tone. The shifter's **power normaliser** makes
  `z`'s power `s`'s, and `z` is uncorrelated with `s`, so **the exchange moves energy and
  makes none** — at any amount, at infinite decay. `z` also owes the decay of the time it
  spent in the shifter (its mean delay), so a shimmered tail decays at the set rate.
- **The shifter** (`caelum_shift.rs`, chosen over PSOLA and the phase vocoder — a tail has
  no pitch to lock grains to, and the vocoder costs four times the tank): three delay-line
  heads a third of a turn apart through Hann windows (their sum is exactly 1 — a coherent
  tone keeps its amplitude — and their squares' sum exactly ½ — a diffuse tail keeps a
  constant power, so no flutter either way), 120 ms grains, each grain started at its own
  offset (0–12 ms, drawn from the grain's number) so a pure tone's phase turns randomly at
  the hand-overs instead of steadily (a steady turn detunes it, up to 40 cents), and the
  normaliser (½…2, 150 ms). The phase is an integer count: a skipped shifter's heads are a
  processed one's to the bit.
- **Infinite** holds the middle (gain 1) and keeps the shelves as at 60 s, so damping still
  darkens; once nothing more comes in, a **hold** keeps the energy where it was (±0.5 % a
  pass). **Freeze** cross-fades each line to its raw output ×0.99999, mutes the shimmer,
  shuts the tank's door (the gate is after the pre-delay and the bloom, so a frozen tank
  takes nothing and what is on its way waits), and holds the level 150 ms after closing.
  **The governor**: above a wet of −6 dBFS RMS the loop gain comes down by the square root
  of the excess — never on a normal tail; what makes an infinite tank fed for minutes
  plateau.
- **Bloom**: 12 taps a side over the bloom time after the pre-delay, sparse first and dense
  at the end (`T·((k+1)/K)^0.6`), rising (`u^1.2`), normalised to one tap's energy, the
  sides' patterns apart; cross-faded with the plain input (equal power).
- **Diffusers**: four all-passes a side (20 ms in all), always running (a diffusion of 0 is
  four delays, so the onset never jumps), the two longest modulated (±0.6 ms).
- **Ducking**: a peak detector (50 ms fall) on the input or the key; the reduction it asks
  (the level over the threshold, up to the range) reached at the attack's speed and let go
  at the release's — the release is how fast the wet comes back.
- **The wet's level** follows `(T / 2 s)^¼` of the input's (a quarter of the dB its energy
  would; Spatium's half is too much for a minute-long wash), capped at 30 s; an infinite
  tank is set 8 dB under an 8 s one (it keeps all it is fed).
- **Exact rest**: once the input has been silent for as long as the way in reaches and
  the tank has written nothing above −120 dB for as long as its longest line, the tank, the
  diffusers and the shifters are **drained** (written +0.0 for one more line's length, the
  small states cleared) and the unit is at rest — a new unit, skipped. Infinite and frozen
  tanks never drain while they hold; an empty one is at rest.

## Measured (48 kHz; `cargo test -p cantus-dsp --lib caelum -- --nocapture`)

- **RT60 per octave against the design** (125 Hz – 8 kHz, four responses averaged, against
  `expected_band_t60` of the design's achieved curve), four tanks — small 1.5 s; large 4 s
  dark (4 kHz ×0.4, 200 Hz ×1.5); vast 10 s bright; 8 lines 3 s: **worst band 3.1 %**
  (ISO 3382's audible difference is 5 %). The design itself: every line's loss within 3 %
  of `60·L/(fs·T)` at both ends and 6 % in the middle (far from both corners).
- **Infinite, five minutes**: fed 4 s and left, within **+0.4 dB** of its first ten seconds
  after the input for the whole five minutes (a slow drift up of ~0.1 dB a minute —
  the hold keeps the lines' energy while the damping takes the highs; bound 1 dB).
  **Infinite + full shimmer** (both voices, octave and fifth): never above its first ten
  seconds; it climbs out through the loop's tone and fades (−27 dB in 10 s). **Fed noise
  for five minutes without a pause**: plateaus at −3.2 dB (L + R; the governor's ceiling
  −3.0), spread 0.08 dB over the last two minutes; with the shimmer, at −16 dB.
- **Freeze, five minutes**: within **−0.15…+0.06 dB** of the level where it froze; notes
  played into a frozen tank change its wet by exactly 0.
- **The shimmer makes no energy**: with the absorption flat and the loop's tone open, the
  broadband RT60 is 3.01 s without the shimmer, 3.01 / 3.00 / 2.99 s with it at 0.5, 1 and
  1 with both voices (before the shifter's delay was charged its decay: up to +16 %).
- **The shimmer's artefacts in the loop** (a 440 Hz tone, the tail 1 s later, power within
  ±30 cents of the partials the voices make against the rest, 60 Hz – 12 kHz): no shimmer
  96.6 % (the modulation's own spread); octave **95.2 %** (artefacts −13.0 dB under the
  partials, 50 % of the tail above 600 Hz — it climbs); octave and fifth 95.1 % (−12.9 dB);
  octave down **82.6 %** (−6.8 dB: a shift down hands over half as often and its sidebands
  land wider). The shifter alone on pure tones (220 Hz – 1 kHz, ±5, 7, 12 semitones): the
  centre within **13 cents** worst (was 40 before the drawn offsets), 54 % – 99 % of the
  power within 25 cents (the worst an octave down at 220 Hz). Noise through it: power
  within 0.1 dB, 20 ms windows within 0.65 dB (no flutter).
- **Echo density** (Abel & Huang ≥ 0.9, 20 ms windows, from the impulse): small 20 ms,
  large 120 ms, **vast 260 ms** (lines of 210–480 ms), vast without diffusion 980 ms;
  8 lines large 120 ms.
- **Bloom**: the wet's envelope after a 60 ms burst peaks at 120 ms with no bloom, and at
  640, 1 600 and 3 040 ms with 500, 1 500 and 3 000 ms; 100 ms in it is −2.7 dB under its
  peak without bloom, −64 dB / below −170 dB with it.
- **Ducking** 12 dB: −12.0 dB while the dry plays at −10 dBFS, −3.8 dB 0.4–0.6 s after it
  stops, −0.4 dB 1.2–1.8 s after (release 350 ms).
- **The wet's level** on noise against `(T/2)^¼`: within **0.10 dB** (sizes 0.3–0.9, 1–16 s).
- **Latency 0**: the dry at unity on the first sample; at a pre-delay of 0 the wet starts
  637 samples (13 ms) in — the earliest of the early taps.
- **Rest**: at rest after 6.0 s (Glass Plate), 7.2 s (Ducked Vocal), 10.1 s (8 lines),
  12.9 s (the starting preset), 18.0 s (Bloom Pad) of silence; the last sound before the
  drain −130 to −158 dBFS; the four twins (processing zeros, skipped, reset, new) agree to
  the bit on music afterwards.
- **No allocation after `new`**: a counting allocator over every factory preset and three
  more (frozen, 8 lines with the longest pre-delay and bloom, the defaults), blocks of 1, 37,
  64, 512, 1024, the key, `meter`, `at_rest`, `skip`, `reset`: 0.
- **CPU** (one core, 48 kHz, a minute of program in blocks of 256, best of three, dev
  profile with `cantus-dsp` at opt-level 3; one other builder's `rustc` running at the start,
  none at the end): **16 lines 3.10 %** of a core without the shimmer, 3.59 % with one
  voice, 3.98 % with two, 3.84 % with bloom and shimmer; **8 lines 1.59 %**, 2.01 % with
  the shimmer. Spatium (Room) in the same run 1.76 %.
- **The factory spaces**: all eleven stable on six seconds of loud program (the dry alone
  peaks at −4.4 dBFS; with each space −3.5 to +2.9 dBFS — to be fitted to level with the
  presets' file); each makes sound, each with a mix above 0.
- **The suite**: `cargo test -p cantus-dsp --no-fail-fast` — 948 passed in the library
  (Caelum's 28 among them, one ignored: the CPU measure), the integration tests and
  `caelum_alloc` green; `cargo clippy -p cantus-dsp --all-targets -- -D warnings` clean;
  `cargo fmt -p cantus-dsp` applied.

### Tests and what each holds

`caelum_tests.rs`: `mix_zero_is_a_null_to_the_bit` (the defaults, a heavy setting at mix 0,
infinite and frozen at mix 0 — the output is the input, bit for bit);
`the_output_does_not_depend_on_the_block_size` (1/37/64/512 at tolerance 0, four settings,
one keyed, a change of every kind mid-way — size, decay, shimmer, freeze, the line count);
`the_latency_is_zero_and_the_dry_is_not_delayed`; `a_synced_pre_delay_is_its_note_at_the_tempo`;
`it_comes_to_rest_exactly_and_its_twins_agree` (six settings); `a_new_unit_is_at_rest_even_infinite_or_frozen`;
`infinite_never_comes_to_rest_while_it_holds`; `freeze_holds_and_lets_nothing_in`;
`every_factory_space_is_stable_and_sounds`. `caelum_tests_measure.rs`: the measurements
above (`the_decay_follows_the_setting_in_every_band`, `infinite_does_not_grow_over_five_minutes`,
`freeze_holds_its_level_for_five_minutes`, `the_shimmer_moves_energy_and_makes_none`,
`the_shimmer_moves_a_tail_by_its_interval`, `the_tail_turns_dense_quickly`,
`bloom_is_a_slow_attack`, `ducking_steps_aside_and_comes_back`,
`the_wet_level_follows_the_decay`, `cpu_at_eight_and_sixteen_lines` — `#[ignore]`, run
alone). In the parts: the sine table, the phase increment, the all-pass's losslessness, the
matrices orthogonal and not involutions, the lengths distinct primes, every line's band
rates, infinite never gaining, the shifter's interval on tones, its power on noise, its skip.
`tests/caelum_alloc.rs`: no allocation.

## Shown to the owner

- `C:\projects\daw-fx\compartir\caelum\sound\` — the guitar (Carbonthief, CC0) through the
  octave shimmer, the fifth cathedral, the bloom pad and the freeze swell; a synthetic sung
  line through the ducked space, with and without ducking; a synthetic pad through the
  octave shimmer, the endless ascent and the sub-octave cloud; dry twins; README and the
  render log.

## Verified only in emulation / needs the owner

- Everything is DSP-only and measured; **the sound is for the owner's ears**: the shimmer's
  amount at the starting preset (it climbs clearly in tails, gently under playing), the
  octave down's roughness on low notes, the infinite level, the bloom's shape.

## What the later phases need

1. **Core**: an `EffectKind` (a new `ALL` index; the contract's), `PARAMS` mapping every
   `CaelumSettings` field (the decay knob's top position as Infinite, or a separate switch;
   the line count as a quality choice; the pre-delay's sync — note values to `sync_ms` with
   the tempo from `SongPosition`), the `out` key, schema bump and fixture, commands with
   undo and the parameter menu, the undo fuzz arm, the starting preset (`presets::starting`)
   in `starting.rs`.
2. **Engine unit** (`cantus-engine/src/fx/caelum.rs`): `FxUnit` over `Caelum` — `latency` 0,
   `at_rest`/`skip`/`reset` straight through, the sidechain for ducking
   (`takes_sidechain`), the meter to the editor's telemetry, tempo sync of the pre-delay;
   `tests/fx_caelum.rs` (the null), its rows in `fx_contract.rs` and `must_rest` (it rests
   at its defaults; heavy at infinite or frozen does not), `rt_memory.rs`, the bench kernel.
3. **A panel with its own personality** — fine strokes, a dark display in both themes,
   nothing abbreviated: the display is its centre — e.g. the tank's energy over time as a
   rising sky (the shimmer's octaves as layers climbing, bloom as the swell's curve, the
   freeze as a held horizon), the achieved decay per octave drawn and dragged (high and low
   damping as two handles), the voices' intervals on a small staff or ladder; the design
   sheet first (Slate and Paper, rest and playing, beside the references), the owner's
   approval before the full editor; English and Spanish at 75–200 %.
4. **Presets**: the eleven factory spaces as the file (`presets::factory()` is their
   source; a test should compare the file with it), ten or more, named, fitted to level.
5. **Export**: the CLAP/VST3 descriptor, bit-identical to the engine; golden ids appended.
6. **Docs**: an ADR (the design decisions above: the energy-exchanging shimmer, the
   three-head shifter with drawn offsets, the drain for exact rest, the hold and governor),
   `docs/plugins/caelum.md`, the manual EN/ES, the AI tools' description, the site's card.
7. **End to end**: insert it from the picker on a playing track through
   `CantusApp::assemble`, move a control, hear it in the master.

## Left (this phase)

- Nothing in the DSP phase; the owner's listening, and the phases above.
