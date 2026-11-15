# The effects side round, wave E — `purus`: Purus, the real-time repair unit (DSP phase)

This is branch `fx/purus`, made from `fx/integration` d3b51a09. It is **DSP only**: there is no
`EffectKind`, no core parameters, no engine unit, no panel, no schema bump and no ADR number. The
name is provisional (`cantus_dsp::purus::NAME`).

Purus is an **insert**, not the audio editor's offline repair (ADR 0096). It cleans a track as it
plays. Its references for *function* (inspired by, never copied) are iZotope RX's De-hum and
Dialogue De-reverb, Waves X-Hum and NS1, Accusonus ERA De-Reverb, and Acon DeVerberate.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/purus.rs`: the unit, its settings, meter, delta, latency and rest. `purus_hum.rs`: the de-hum. `purus_verb.rs`: the de-reverb and the decay estimator. `purus_testkit.rs`: synthetic hum, bass, voice and room, and the measures. Tests: `purus_tests.rs`, `purus_tests_measure.rs`, `purus_tests_verb.rs`, `purus_tests_shared.rs`, `cantus-dsp/tests/purus_alloc.rs`. Example: `cantus-dsp/examples/purus_sounds.rs`. | `cantus-dsp/src/lib.rs`: one line, `pub mod purus; // fx-e:purus`. `denoise_stft.rs`: `sqrt_hann` taken out of `Stft::new`. `denoise.rs`: `smooth_gains` taken out of `denoise_channel`. Both are marked `fx-e:purus`, and the editor's outputs are bit-identical (below). |
| Docs | this file | — |

### What is shared with the audio editor's denoiser, and what is not

- **Shared:**
  - The square-root-Hann window: `denoise::stft::sqrt_hann`, the same arithmetic the editor's `Stft::new` had.
  - The gain smoothing: `denoise::smooth_gains`. It smooths over three bins, then over time (rising at once, falling slowly). Its body is moved from `denoise_channel`, with the same operations in the same order.
  - The radix-2 FFT (`aequalis::fft`), which both already used.
- **Guarded:** `purus_tests_shared.rs` hashes the outputs of `denoise_channel` (three settings), `attenuate_region` and `Spectrogram::of`. The hashes were taken **before** the sharing, and the test still passes after it. The editor's own tests (`denoise::tests`, 12) are green.
- **Not shared, and why:**
  - The editor's noise model is a learned, stationary profile; a reverb's is predicted frame by frame from the signal.
  - Its `Stft` is offline: it allocates in every `run`, its N is fixed at 2048, and it works on whole takes. Purus needs a streaming transform that allocates only in `new`, sized to about 21 ms at every sample rate.

### The DSP API

```rust
use cantus_dsp::purus::{Purus, PurusSettings, HumSettings, HumMode, VerbSettings, DecayMode, latency_for};

let mut p = Purus::new(sample_rate);        // allocates here only
p.set(&settings);                           // between blocks; levels and mixes glide 10 ms
p.process(&mut left, &mut right);
p.latency();                                // 0, or N − 1 while the de-reverb is on
latency_for(&settings, sample_rate);        // the same without a unit
p.meter();                                  // PurusMeter (below)
p.dehum().response(hz);                     // the notches' magnitude at a frequency (the display)
p.dereverb().gains();                       // the de-reverb's gain per bin, last frame
p.at_rest(); p.rest_blocker(); p.skip(n); p.reset(); p.forget();
```

**`HumSettings`** (all in real units):

| Field | Range | Meaning |
|---|---|---|
| `on` | | The module switch. |
| `mode` | `Fixed`, `Track`, `Auto` | Fixed is exact. Track follows the hum within ±2 Hz. Auto picks 50 or 60 Hz, then follows it. |
| `freq_hz` | 40–70 | The fundamental, or the nominal frequency the tracker centres on. |
| `harmonics` | 1–16 | Counted from the fundamental. Harmonics above 0.45·fs are skipped. |
| `depth_db` | 0–80 | How deep each notch goes. |
| `width_hz` | 0.5–20 | The −3 dB width of each notch. |
| `gentle` | | Subtract the learned hum, holding the learning under a bass note, instead of notching. |
| `mix`, `out_db`, `delta` | | The module's mix, output level and delta. |

**`VerbSettings`:**

| Field | Range | Meaning |
|---|---|---|
| `on` | | The module switch. |
| `amount_db` | 0–30 | Default 12. The floor of the gain, which also scales the tail predicted (1 + amount/24). |
| `decay` | `Set`, `Estimate` | Where the decay time comes from. |
| `decay_s` | 0.1–10 s | The T60. |
| `direct_db` | −10 to +20 | Default 0: the direct-to-reverberant ratio the model assumes. Set it higher for a close microphone. |
| `smoothing` | 0–1 | The gains' smoothing. |
| `keep_ms` | 5–200 | Default 50: the early reflections kept. |
| `mix`, `out_db`, `delta` | | The module's mix, output level and delta. |

**`PurusSettings`** holds `{ hum, verb, out_db }`. **`Default` is the null**: every module is off
and the output is at 0 dB.

**`PurusMeter`:**
- The hum's fundamental now, and the nominal it centres on.
- Each harmonic's level in the input (its 3 Hz band) and its depth now.
- In gentle mode, which harmonics are held.
- The decay in force and the decay estimated.
- **The de-reverb's reduction in 48 log bands from 20 Hz to 20 kHz** (the display's "what is removed").
- The input and output peaks, and the level of what was taken out.

### How it works (each file's header says it in full)

**The signal flow:**
- The input goes through the de-hum (IIR, no latency), then its mix and output level.
- Then through the de-reverb (STFT, N − 1 samples), then its mix and output level.
- Then the unit's output level.
- The delta paths are kept beside it: the input delayed as the chain is, and the de-reverb's own input, delayed.

**Each module's mix** returns its input bit for bit at 0, and its output bit for bit at 1.

**Delta:**
- With every module that is on listening to its delta, the output is exactly `x(t − L) − y(t)`.
- With only some listening, the output is the sum of what those modules take out, each aligned to the output.

**De-hum, notches:**
- There is one section per harmonic: `H = 1 − (1 − g)·BP`.
- The band-pass's −3 dB width is exactly the width set (`α = tan(π·width/fs)`), and `g` is the depth's gain.
- It runs in `f64`, because the poles of a 0.5 Hz notch at 50 Hz sit 3·10⁻⁵ from the unit circle.
- Depth, width, and harmonics switched in or out glide on a 64-sample grid, so nothing clicks.

**De-hum, gentle:**
- A Widrow adaptive canceller per harmonic, at `μ = π·width/fs`. On its own it is a notch of that width.
- Its learning is **held** while a three-pole, 6 Hz band of *what the learned hum does not explain* stands above half the hum's level, or above twice the band's usual remainder. That remainder rises over 4 s and falls over 50 ms.
- The hold does not apply when the hum has simply gone.
- When a hold starts, the estimate goes back to a snapshot taken 40–60 ms of *learning time* earlier. This unlearns the first milliseconds of a note, before the hold could catch it.
- A hum that changes is learned again by the slow remainder (3.5 s, measured).
- Only the harmonics up to **260 Hz** (the bass range) can be held. Above that, chords crowd the harmonics: their partials 20–40 Hz away leak into a band fast enough to catch a note's onset. There the canceller always learns, and acts as a notch.
- For the first 0.3 s after sound starts, it learns at twice the rate with no hold.

**The tracker** is a frequency-locked loop on the mono sum:
- The first eight harmonics are each demodulated against the reference and low-passed (three poles, 3 Hz).
- Every 64 samples, each harmonic reads how far the hum's phase turned, divided by k.
- The reading used is the **weighted mean of the readings within 0.3 Hz of their weighted median**, weighted by strength. This is robust to a tone leaking into one band, without the median's own selection bias.
- The loop is **second order** (0.5 Hz, damping 0.8), so a steady drift is followed without lag. It is clamped to ±2 Hz of the nominal.
- **Auto** also listens at the other mains frequency's first four harmonics. It moves there when that is 6 dB stronger at two looks half a second apart.

**De-reverb, the transform** is the editor's, made streaming:
- The same window, at 75 % overlap.
- N is 1024 at 44.1/48 kHz, 2048 at 88.2/96 kHz and 4096 above. The latency is `N − 1`: **1 023 samples, 21.3 ms at 48 kHz**.
- The two channels share one set of gains, computed from their mean power, so the stereo image cannot wander.

**De-reverb, the model** is Habets's generalisation of Lebart's:
- `λ_rev(n) = (1−κ)·a·λ_rev(n−1) + κ·a·λ_y(n−1)`, with `κ = (1−a)/(a·DRR)` and `a = e^{−2δR/fs}`.
- `λ_late = a^(N_e−1)·λ_rev(n−N_e+1)`, where `N_e` is the early part kept.
- The gain is the Wiener gain on the decision-directed a-priori ratio (Ephraim & Malah), with the decision-directed weight 0.85–0.98 set by smoothing. It is floored at the amount, then passed through the editor's smoothing.

**De-reverb, the decay estimate:**
- It watches the frame energy from 300 Hz to 5 kHz, smoothed over 15 ms.
- A *run* goes on while no frame rises 2 dB above the run's lowest.
- Each run is fitted by least squares after its first 60 ms (the direct sound and the early reflections), without its last three frames. A run must give at least 80 ms and 6 dB.
- The estimate is the median of the last sixteen fits.

**Rest:**
- When the input has been +0.0 for long enough, the unit **drains** to a new unit's state. That needs the de-reverb's last four frames under −180 dBFS and every de-hum state under 10⁻¹⁵, checked on a 256-sample grid of the silence.
- At rest, silence in is silence out and nothing advances.
- The decay the de-reverb estimated survives the drain and `reset`, as the editor keeps its noise profile. `forget` clears it.

## Measured

Everything below is at 48 kHz. Run it with `cargo test -p cantus-dsp --lib purus -- --nocapture`.
The tests' bounds sit just under these numbers, so a regression fails.

### De-hum

The hum: harmonics 1–8 from −30 dBFS (50 Hz) down to −48 dBFS, plus music of three tones (440, 659
and 1 318 Hz) and noise.

**Notches:**
- At 40 dB and 2 Hz, every harmonic comes down **37–43 dB**.
- At 60 dB and 1 Hz, they come down 40–67 dB. The 8th reads 40, which is the measure's floor next to the 440 Hz tone.
- The music's tones lose **≤ 0.01 dB**.

**Gentle**, on the same steady hum: **37.6–42.6 dB**.

**A bass on the hum's own frequencies** (plucked notes on 50, 100, 150, 55, 98 and 50 Hz, peak −6 dBFS):

| Mode | The bass on 50/100/150 Hz | The hum under the bass |
|---|---|---|
| Notches | loses **6.5–8.8 dB** | ≥ 40 dB down |
| Gentle | loses **0.02–0.44 dB** (−0.10 dB in all; the 0.44 is the 150 Hz note, whose 300 and 450 Hz partials are above the 260 Hz gated range) | ≥ **39.5 dB** down at the worst harmonic |

The hum under the bass is measured with the bass played inverted too and the two outputs averaged.

**Tracking a drifting 50 Hz (±1 Hz)**, with the music, from 3 s on:

| The swing | Tracked: worst harmonic down | Tracked: fundamental within | Fixed: worst harmonic down |
|---|---|---|---|
| 60 s period (≤ 0.1 Hz/s) | **37.0 dB** | **0.004 Hz** (RMS 0.0015) | 0.1 dB |
| 10 s period (up to 0.63 Hz/s) | **25.2 dB** at the 8th (39 at the fundamental) | 0.041 Hz (RMS 0.028) | 1.1 dB |

Each harmonic is measured along the hum's true phase.

**Auto:**
- A 60.3 Hz hum is locked within 0.05 Hz in **1.23 s**, and its harmonics come down ≥ 34.1 dB.
- A 50.3 Hz hum is locked in 0.20 s, with ≥ 33.7 dB.

**A hum that changes** (it doubles and turns at 3 s): the gentle mode takes it out again (30 dB) **3.5 s** later.

### De-reverb

The scene: a synthetic spoken line (the formant voice, 16 syllables) in a synthetic room. The
room is a direct impulse, six early reflections, and a Gaussian tail that is the same in every
band, with a known T60 and DRR. Each component (early, late, direct, reverberant) is sent
through **the gains the mixture got** (`Dereverb::set_shadow`). So each figure is what the
processing does to that part. The musical-noise measure is the log of the kurtosis ratio of
the power spectra (Uemura et al. 2008), taken in the tail after the line; above 0 means
musical noise was added.

| T60 0.8 s, DRR 0 dB, early 50 ms | Late reverb, over the line | In the tail after it | Early part | Early-to-late gained | DRR gained | Musical noise (log kurtosis ratio) |
|---|---|---|---|---|---|---|
| **12 dB, the defaults** | **−6.8 dB** | **−12.0 dB** | −2.25 dB | **+4.6 dB** (4.9 → 9.4) | +1.9 dB | −0.001 |
| 24 dB | −9.5 | −23.7 | −3.00 | **+6.5** | +2.5 | −0.035 |
| 24 dB, smoothing 0 | −9.1 | −22.6 | −2.44 | +6.7 | +2.5 | −0.188 |
| 24 dB, smoothing 1 | −9.5 | −24.0 | −3.48 | +6.0 | +2.5 | −0.036 |
| 12 dB, direct +6 dB (6 dB too high) | −3.7 | −11.7 | −1.05 | +2.7 | +1.1 | −0.034 |
| 24 dB, direct −10 dB (≈ Lebart's model) | −10.9 | −20.6 | −4.58 | +6.3 | +2.5 | −0.171 |
| **T60 1.6 s, DRR +6 dB, 24 dB** | −6.8 | −22.7 | −1.71 | +5.1 | +3.4 | −0.081 |

- **No musical noise added** in any setting by this measure: every ratio is at or below 0.
- **The decay estimated** from the line alone: 0.56 s for a 0.5 s room, 0.84 s for 0.9 s and 1.29 s for 1.5 s. That is within 14 %, and the longest room reads low.
- **A dry voice** (no room) loses level as follows:
  - At the defaults, which assume a direct sound only as strong as the reverberation: 2.86 dB, with the change 10.2 dB under the voice.
  - At 24 dB: 4.21 dB.
  - With **direct +6 dB**: 0.96 dB, with the change 20.2 dB under.
  - With **direct +12 dB**: **0.31 dB**, with the change 29 dB under.

### The contract

- **Null:**
  - With every module off (the defaults, or settings moved but off), the output is the input, bit for bit and **undelayed**.
  - The de-hum at mix 0 hands on its input bit for bit.
  - The de-reverb at mix 0 hands on its input bit for bit, **delayed by exactly the latency stated** (1 023 samples, which are +0.0 before it). This also holds with amount 0.
  - At amount 0 and mix 1, the transform's round trip is within 2·10⁻⁶ of the input.
- **Block-size independence**, at tolerance 0: blocks of 1, 37, 64 and 512, over four settings. These cover the tracked gentle mode with 16 harmonics, Auto with the de-reverb estimating, and the de-reverb alone. Settings of every kind are also changed mid-way, at the same samples whatever the blocks.
- **Delta:**
  - With every module on listening to its delta, the output is `x(t − L) − y(t)` to the bit (at an output level of 0 dB; within 10⁻⁶ with the level moved).
  - The de-hum's delta alone is `x(t − L) − h(t − L)`, the de-reverb's alone `h(t − L) − y(t)`, each to the bit.
- **Rest:**
  - The unit comes to rest after **4.8–5.05 s of silence** with the de-hum on (the high-Q notches and the canceller must decay under 10⁻¹⁵), and after **0.04 s** with only the de-reverb.
  - The sample the drain comes on is silent (< 10⁻⁹).
  - The twins agree to the bit on music afterwards: processing zeros, skipped, reset, and new (new only with the decay set, since reset keeps an estimate).
  - A new unit is at rest, and silence keeps it there.
- **No allocation after `new`**, under a counting allocator. This covers five settings twice, blocks of 1, 37, 64, 512 and 1 024, `set`, `meter`, `at_rest`, `latency`, rest, `skip` and `reset`: 0 allocations.
- **Deterministic maths:** only `crate::math` is used (clippy's `disallowed-methods`), and `sqrt` is IEEE.
- **CPU** (one core, 48 kHz, a minute of program in blocks of 256, best of three, `--release`):

  | Setting | Share of a core |
  |---|---|
  | Notches, 8 harmonics, fixed | **0.42 %** |
  | Notches, 16, tracked | 0.53 % |
  | Gentle, 16, Auto | 1.93 % |
  | De-reverb | **1.01 %** |
  | Both (gentle 16 Auto, plus the de-reverb estimating) | **2.90 %** |

  The machine was a Ryzen 7 7735HS with no other `rustc` or `cargo` running before or after, at 7–18 % load.

### Tests and what each holds

- **`purus_tests.rs`:**
  - `every_module_off_is_a_null_to_the_bit_and_undelayed`
  - `a_module_at_mix_zero_hands_its_input_on_to_the_bit`
  - `the_output_does_not_depend_on_the_block_size`
  - `the_delta_is_the_input_delayed_minus_the_output_to_the_bit`
  - `one_modules_delta_is_what_that_module_takes_out`
  - `the_dereverb_reconstructs_its_input_when_it_takes_nothing`
  - `it_comes_to_rest_exactly_and_its_twins_agree`
  - `a_new_unit_is_at_rest_and_silence_keeps_it_there`
- **`purus_tests_measure.rs`:**
  - `the_hum_and_its_harmonics_come_out`
  - `the_gentle_mode_keeps_a_bass_on_the_hums_frequencies`
  - `tracking_follows_a_mains_drifting_one_hertz_either_way`
  - `auto_finds_sixty_hertz_and_stays_on_fifty`
  - `the_gentle_mode_learns_a_hum_that_changes`
  - `purus_cpu` (`#[ignore]`; run it alone, in release)
- **`purus_tests_verb.rs`:**
  - `the_late_tail_comes_out_of_a_known_room`
  - `the_decay_is_estimated_from_the_signal`
  - `a_dry_voice_is_nearly_left_alone` (with direct +12 dB)
- **`purus_tests_shared.rs`:** `the_denoisers_outputs_are_bit_identical_to_before_the_sharing`.
- **`tests/purus_alloc.rs`:** `purus_allocates_nothing_after_new`.

**The suite:** `cargo test -p cantus-dsp --no-fail-fast` passes 937 in the library (18 ignored)
plus the integration tests. `cargo clippy -p cantus-dsp --all-targets -- -D warnings` is clean,
and `cargo fmt -p cantus-dsp` is applied.

## Shown to the owner

`C:\projects\daw-fx\compartir\purus\sound\` holds:
- The guitar (Carbonthief, CC0) with synthetic hum and buzz, de-hummed by the notches and by the gentle mode, each with its delta, plus the gentle mode with a lead-in.
- A synthetic bass on the hum, through both modes.
- A synthetic spoken line in a synthetic room, de-reverberated at 12 and 24 dB with the decay estimated, with deltas.
- Dry twins, a README saying what to listen for, and `render-log.txt` with the numbers.

## Misses against the targets, honestly

- **The gentle mode on a dense strummed guitar** leaves harmonics 2–3 (or 3–5 with a lead-in) 10–20 dB higher than the notches. Here is what is left, in dBFS:

  | Harmonics 1–8 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
  |---|---|---|---|---|---|---|---|---|
  | Notches | −63 | −61 | −57 | −61 | −56 | −76 | −61 | −65 |
  | Gentle | −63 | −39 | −46 | −53 | −40 | −75 | −61 | −65 |

  The chords' notes within a few hertz of 100, 150 and 250 Hz hold its learning most of the time. A held estimate cannot follow the small phase errors of a tracked, drifting hum. On a bass line (its purpose) it does what it should. On full chords, the notches are the mode to use, and the panel must say so.
- **Latency** is 21.3 ms at 48 and 96 kHz, but **23.2 ms at 44.1 kHz** (1 023 samples). That is slightly over the "about 20 ms or less" asked for.
- **On a dry voice, the de-reverb's defaults cost 2.9 dB**, because they assume a far microphone. Close-miked dialogue needs **Direct sound** at +6 to +12 dB. Whether the default should be higher is a choice for the owner's ears.
- **Late reverberation during speech** comes down 6.8 dB at the defaults and 9.5 dB at 24 dB, while the tail between and after words comes down 12–24 dB. This is the family's known limit (a Wiener gain cannot take reverb out from under a strong bin). The early part pays 2–3 dB.
- **A fast swing of the mains** (±1 Hz every 10 s) leaves the 8th harmonic at 25 dB down. Real mains drift far more slowly (the 60 s case reaches 37 dB).
- **A hum that changes is relearned by the gentle mode in 3.5 s**, and so are its first seconds when music starts at once.
- **Rest takes about 5 s of silence with the de-hum on**, because the 2 Hz notches ring down slowly.
- **De-click / de-crackle is not built.** It is listed for later (below).

## Verified only in emulation / needs the owner

- Every signal is synthetic except the guitar's DI. That includes the hum, the bass, the voice and the room.
- These need checking on real recordings:
  - A real mains hum (a guitar amp, a ground loop, a field recording with a generator).
  - A real voice in a real room (a public-domain recording or the owner's own).
  - The owner's ears on the gentle mode, the de-reverb's amount, and whether the default Direct sound should be higher.
- A `docs/beta-checklist.md` entry belongs to Phase B: "record your guitar through an amp with a ground loop, de-hum; record speech in a tiled room, de-reverb".

## Phase B — the plan

1. **Core.**
   - Add an `EffectKind` (the integrator's contract) and `PARAMS` mapping every field.
   - The nominal frequency and mode become a choice of 50, 60, Track or Auto.
   - Harmonics, depth and width are knobs. Gentle is a switch with an honest help line ("for a bass; for chords use the notches").
   - The de-reverb gets amount, decay, a Set/Estimate switch, Direct sound, smoothing and Early reflections kept.
   - Each module gets mix, output and delta; the unit gets the `out` key.
   - Also: a schema bump and fixture, commands with undo and the parameter menu, the undo fuzz arm, and the starting preset (ADR 0071): the de-hum on at 50 Hz, tracked, 8 harmonics, notches; the defaults stay the null.
2. **The engine unit** (`cantus-engine/src/fx/purus.rs`): an `FxUnit` over `Purus`.
   - `latency` comes from `latency_for`, so it changes when the de-reverb is switched. The engine's delay compensation must follow that, as it does for Sibilus's split modes.
   - `at_rest`, `skip` and `reset` pass straight through; `must_rest` holds.
   - The telemetry: the hum's fundamental, each harmonic's level, depth and held flag, the 48-band reduction, the decay estimated, and the removed level.
   - Tests: `tests/fx_purus.rs` (the null), the rows in `fx_contract.rs`, `rt_memory.rs`, and the bench kernel.
   - A per-bin reduction readout beyond the 48 bands, if the display wants more.
3. **A panel with its own personality**, unlike every existing unit.
   - The look: fine strokes, a **dark central display in both themes**, and a clinical, precise air, like a repair bench. It must not look like FabFilter's or iZotope's.
   - The display shows the **spectrum**, with **what is removed drawn in its own colour** (a cool cyan-white "residue" falling out of a warm spectrum, say). The hum's harmonics are a comb of fine needles at their true frequencies, each with its depth and a held marker in gentle mode. The de-reverb's reduction is a translucent band under the spectrum.
   - **Drag on the display:** the fundamental (in Fixed mode), the comb's width, and the de-reverb's amount as a level.
   - Three module strips (De-hum, De-reverb, and De-click when built), each with an on switch, mix, output and a **Listen to what is removed** button.
   - Readouts: "Hum 50.02 Hz · Auto: 50 Hz" and "Decay 0.83 s (estimated)".
   - **Nothing abbreviated in EN or ES**, clear label and value units, at 75–200 %, legible in every theme.
   - The design sheet first: Slate and Paper, at rest and playing, beside the references' public screenshots (kept under `refs\`, never committed). Then the owner's approval, then the full editor.
4. **Presets** (ten or more, named and made with care), for example:
   - Mains 50 Hz, Mains 60 Hz, Guitar amp buzz (16 harmonics, tracked).
   - Bass DI (gentle).
   - Dialogue in a room, Close voice in a small room (Direct +9), Lecture hall (estimated).
   - Video call, Field recording generator (Auto), Strong cleanup.
5. **CLAP/VST3 export**, bit-identical to the engine, with the golden ids appended. The latency must be reported to the host and follow the de-reverb's switch.
6. **Docs:**
   - An ADR on the design decisions above: the gentle canceller's held learning and its bass range, the robust FLL, the Habets model with Direct sound, amount as floor plus over-subtraction, the decay estimator, and the drain.
   - `docs/plugins/purus.md`, the manual in EN and ES, the AI tools' description, and the site's card.
7. **End to end:** insert it from the picker on a playing track with hum through `CantusApp::assemble`, switch the de-hum on, and hear the hum leave the master. The de-reverb's latency must be compensated on the track.
8. **Later:**
   - De-click / de-crackle, light. A real-time impulse detector on a linear-prediction residual, with a few milliseconds of look-ahead and AR interpolation, after the editor's `denoise_declick` (sharing its detector where it fits).
   - A **gentle mode with a little look-ahead** (≈ 50 ms, optional): it would see a note before it reaches the canceller, and so could be selective on chords too.
   - A per-band decay (T60 by octave) and an estimated Direct sound.

## Left (this phase)

- In the DSP: the misses above, which are the owner's to judge, and de-click (later).
- The owner's listening.
