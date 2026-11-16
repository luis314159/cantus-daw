# The effects side round, wave E — `purus`: Purus, the real-time repair unit (DSP phase)

This is branch `fx/purus`, made from `fx/integration` d3b51a09. It is **DSP only**: there is no
`EffectKind`, no core parameters, no engine unit, no panel, no schema bump and no ADR number. The
name is provisional (`cantus_dsp::purus::NAME`).

**Phase A-2** (the second delivery) added three things:
- The gentle de-hum got a 50 ms look-ahead and was tuned on the CC0 guitar.
- The **de-click / de-crackle** module, on the editor's own click detector and rebuilding.
- The de-reverb's **Direct sound** choices measured for the owner.

Each is in the sections below.

**Phase A-3** (the third delivery). Before and after, on the CC0 guitar:

| | Before (A-2) | After (A-3) |
|---|---|---|
| Gentle de-hum, steady hum: the 3rd harmonic left (notches −58 dBFS) | −40 dBFS | **−75 dBFS** |
| Gentle de-hum, steady hum: the reference's worst error from the hum's frequency | 0.15 Hz | **0.006 Hz** |
| Gentle de-hum, drifting hum: worst harmonic against the notches | within 5.1 dB | **as good as the notches or better** (−70 −71 −64 −69 −60 −68 −63 −67 against −63 −61 −57 −61 −56 −76 −61 −65) |
| Gentle, the bass test on a steady hum | −0.43 dB worst note | −0.43 dB (unchanged) |
| Gentle, the bass on the drifting hum (listening set) | −1.1 dB worst note | **−0.7 dB** |
| Crackle alone (300 a second) | −9.1 dB (de-click only: it hurts) | **11.8 dB taken out** |
| Clicks and crackle | −1.4 dB | **16.2 dB** (the editor's offline declick: 5.3 dB) |
| The clean guitar through the de-click and de-crackle | change 42.8 dB under it | **83 dB under it** (16 places in 21 s) |

- **The gentle mode's reference is phase-locked.** A second-order phase-locked loop at 0.1 Hz drives the phase the hum has turned from the reference back to zero.
  - Its readings are weighted by how well the hum learned explains each band, so a band where a note sounds counts for little.
  - Harmonics a note holds are left out of the readings.
- **The steady-hum miss was the reference, not the hold.** Against a steady 50 Hz hum, the loop strayed up to 0.15 Hz, pulled by the chords. A held harmonic then turned away from the hum, and the residual hum looked like a note, so it stayed held. With the reference within 0.006 Hz, the guitar holds nothing and the canceller is exact.
- **The de-crackle** (`purus_crackle.rs`) **subtracts** each impulse instead of rebuilding the samples under it.
  - It is found in the 5 kHz high band, where an impulse stands far above the music.
  - Its amplitudes at four samples are fitted by least squares to the high band. The fit's matrix is the high-pass's own autocorrelation, factored once.
  - It is accepted only if that shape explains 60 % of the high band's energy over its support.
  - The whole impulse is taken out of the samples.
  - With it on, the de-click's pass leaves impulses of four marks or fewer to it. The de-click rebuilds only what stands out of the music: 6 more robust scales, and a peak error at least 1.5 × the block's level. The rebuilding of small things under loud music was what made things worse.
- **The de-crackle adds no latency** (it works in the de-click's window).
- **Direct sound's default is +6 dB**, by the owner's choice. Far-microphone presets set 0 dB.

Purus is an **insert**, not the audio editor's offline repair (ADR 0096). It cleans a track as it
plays. Its references for *function* (inspired by, never copied) are iZotope RX's De-hum and
Dialogue De-reverb, Waves X-Hum and NS1, Accusonus ERA De-Reverb, and Acon DeVerberate.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/purus.rs`: the unit, its settings, meter, delta, latency and rest. `purus_hum.rs`: the de-hum. `purus_click.rs`: the de-click. `purus_crackle.rs`: the de-crackle. `purus_verb.rs`: the de-reverb and the decay estimator. `purus_testkit.rs`: synthetic hum, bass, clicks, crackle, voice and room, and the measures. Tests: `purus_tests.rs`, `purus_tests_measure.rs`, `purus_tests_verb.rs`, `purus_tests_click.rs`, `purus_tests_shared.rs`, `cantus-dsp/tests/purus_alloc.rs`. Example: `cantus-dsp/examples/purus_sounds.rs`. | `cantus-dsp/src/lib.rs`: one line, `pub mod purus; // fx-e:purus`. `denoise_stft.rs`: `sqrt_hann` taken out of `Stft::new`. `denoise.rs`: `smooth_gains` taken out of `denoise_channel`. `denoise_declick.rs`: the detector and the rebuilding made allocation-free cores (`ArScratch`, `ar_fit_into`, `block_errors`, `threshold`, `widen`, `GapScratch`, `rebuild`), with the editor's functions now wrappers over them. All are marked `fx-e:purus`, and the editor's outputs are bit-identical (below). |
| Docs | this file | — |

### What is shared with the audio editor's denoiser, and what is not

- **Shared:**
  - The square-root-Hann window: `denoise::stft::sqrt_hann`, the same arithmetic the editor's `Stft::new` had.
  - The gain smoothing: `denoise::smooth_gains`. It smooths over three bins, then over time (rising at once, falling slowly). Its body is moved from `denoise_channel`, with the same operations in the same order.
  - The radix-2 FFT (`aequalis::fft`), which both already used.
- **Shared with the click repair** (`denoise::declick`), all used by the de-click:
  - The model's fit: Welch-windowed autocorrelation, Levinson–Durbin, order 32.
  - The prediction errors and the robust scale (the median absolute error / 0.6745, floored at a thousandth of the block's level).
  - The threshold (12 to 4 robust scales with the sensitivity), the merging distance (24) and the widening (2 samples each side).
  - The least-squares autoregressive rebuilding (Vaseghi, Godsill & Rayner).

  Each now works in scratch the caller allocates once. The editor's `ar_fit`, `detect`, `interpolate` and `declick` are those cores with their scratch allocated per call: the same arithmetic in the same order. The one change is that the median now comes from an unstable sort, which never allocates. It is the same value, since `total_cmp` equality is bit equality.
- **Guarded:**
  - `purus_tests_shared.rs` hashes the outputs of `denoise_channel` (three settings), `attenuate_region` and `Spectrogram::of`.
  - It also hashes the declicker's `detect` and `declick` (sensitivity 0, 0.5 and 1, on a take with 40 clicks) and `ar_fit`.
  - Every hash was taken **before** its sharing, and the tests still pass after it. The editor's own tests (`denoise::tests`, 12) are green.
- **Not shared, and why:**
  - The editor's noise model is a learned, stationary profile; a reverb's is predicted frame by frame from the signal.
  - Its `Stft` is offline: it allocates in every `run`, its N is fixed at 2048, and it works on whole takes. Purus needs a streaming transform that allocates only in `new`, sized to about 21 ms at every sample rate.

### The DSP API

```rust
use cantus_dsp::purus::{Purus, PurusSettings, HumSettings, HumMode, VerbSettings, DecayMode, latency_for};

let mut p = Purus::new(sample_rate);        // allocates here only
p.set(&settings);                           // between blocks; levels and mixes glide 10 ms
p.process(&mut left, &mut right);
p.latency();                                // the modules' latencies added (below)
latency_for(&settings, sample_rate);        // the same without a unit
p.meter();                                  // PurusMeter (below)
p.dehum().response(hz);                     // the notches' magnitude at a frequency (the display)
p.dereverb().gains();                       // the de-reverb's gain per bin, last frame
p.declick().repaired();                     // clicks rebuilt: the last block, and in all
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
| `gentle` | | Subtract the learned hum, holding the learning under a bass note, instead of notching. It has a **50 ms look-ahead**, which is its latency. |
| `mix`, `out_db`, `delta` | | The module's mix, output level and delta. |

**`VerbSettings`:**

| Field | Range | Meaning |
|---|---|---|
| `on` | | The module switch. |
| `amount_db` | 0–30 | Default 12. The floor of the gain, which also scales the tail predicted (1 + amount/24). |
| `decay` | `Set`, `Estimate` | Where the decay time comes from. |
| `decay_s` | 0.1–10 s | The T60. |
| `direct_db` | −10 to +20 | Default **+6 dB** (phase A-3; far-microphone presets set 0): the direct-to-reverberant ratio the model assumes. Higher for a close microphone. |
| `smoothing` | 0–1 | The gains' smoothing. |
| `keep_ms` | 5–200 | Default 50: the early reflections kept. |
| `mix`, `out_db`, `delta` | | The module's mix, output level and delta. |

**`ClickSettings`:**

| Field | Range | Meaning |
|---|---|---|
| `on` | | The module switch. |
| `sensitivity` | 0–1 | Default 0.5: the editor's sensitivity, from 12 robust scales at 0 to 4 at 1. |
| `crackle` | 0–1 | The de-crackle (phase A-3). 0 is off. An impulse in the high band is taken at 9 robust scales (at 0+) to 5 (at 1). |
| `mix`, `out_db`, `delta` | | The module's mix, output level and delta. |

**`PurusSettings`** holds `{ hum, click, verb, out_db }`. **`Default` is the null**: every module is off
and the output is at 0 dB.

**`PurusMeter`:**
- The hum's fundamental now, and the nominal it centres on.
- Each harmonic's level in the input (its 3 Hz band) and its depth now.
- In gentle mode, which harmonics are held.
- The decay in force and the decay estimated.
- **The de-reverb's reduction in 48 log bands from 20 Hz to 20 kHz** (the display's "what is removed").
- The clicks rebuilt in the last block and since the reset.
- The input and output peaks, and the level of what was taken out.

### How it works (each file's header says it in full)

**The signal flow:**
- The input goes through the de-hum, then its mix and output level. The notches have no latency; the gentle mode has 2 400 samples (50 ms at 48 kHz).
- Then through the de-click (767 samples, 16 ms), then its mix and output level.
- Then through the de-reverb (STFT, N − 1 samples: 1 023, 21.3 ms), then its mix and output level.
- Then the unit's output level.
- **The latency** is the sum of the modules that are on. With all three (the gentle de-hum, the de-click and the de-reverb) it is 4 190 samples, 87 ms.
- Three delay rings keep the input, the de-hum's output and the de-click's output, so each can be read delayed to meet the output. Those are the deltas.

**Each module's mix** returns its input bit for bit at 0, and its output bit for bit at 1.

**Delta:**
- With every module that is on listening to its delta, the output is exactly `x(t − L) − y(t)`.
- With only some listening, the output is the sum of what those modules take out, each aligned to the output. The first term is used as it is, so a single module's delta is exact.

**De-hum, notches:**
- There is one section per harmonic: `H = 1 − (1 − g)·BP`.
- The band-pass's −3 dB width is exactly the width set (`α = tan(π·width/fs)`), and `g` is the depth's gain.
- It runs in `f64`, because the poles of a 0.5 Hz notch at 50 Hz sit 3·10⁻⁵ from the unit circle.
- Depth, width, and harmonics switched in or out glide on a 64-sample grid, so nothing clicks.

**De-hum, gentle:**
- A Widrow adaptive canceller per harmonic, at `μ = π·width/fs`. On its own it is a notch of that width.
- **The look-ahead (phase A-2).** The canceller works 50 ms late, on the input and the reference as they were then, while the gate listens to the input as it is now. So a note holds the learning **before** it reaches the canceller. The hold lasts until the note has passed the canceller too (its own 50 ms, plus the release).
- Its learning is **held** while a three-pole, **3 Hz** band of *what the learned hum does not explain* (measured on the input as it is now) stands above half the hum's level, or above twice the band's usual remainder. That remainder rises over 4 s and falls over 50 ms. The look-ahead pays for the narrower, slower band, which neighbours 10–20 Hz away no longer trip.
- The hum's level and phase are kept as an estimate smoothed at **0.5 Hz**. That is the reference the gate compares with, and what a hold goes back to. It is seeded from the canceller when learning ends. A 0.5 Hz smoothing rejects the jitter a neighbouring note 10–20 Hz away puts into the raw estimate.
- **Held harmonics turn with the free ones.** The reference follows the hum's frequency, not its phase, so against a drifting hum the phase between them wanders. Every 64 samples, the angle the free harmonics turned (each over its harmonic number, weighted by strength) turns the held ones, each by its own number.
- **In the gentle mode the reference is phase-locked** (phase A-3). The tracker's readings are summed into the phase the hum has turned from the reference, and a second-order loop (0.1 Hz, damping 0.8) drives that phase back to zero. Each reading is weighted by how well the hum learned explains its band, and the harmonics a note holds are left out. The notches keep the frequency loop at 0.5 Hz.
- The hold does not apply when the hum has simply gone.
- When a hold starts, the estimate goes back to a snapshot taken 40–60 ms of *learning time* earlier. This unlearns the first milliseconds of a note, before the hold could catch it.
- A hum that changes is learned again by the slow remainder (3.5 s, measured).
- Only the harmonics up to **160 Hz** (the bass range: 50, 100 and 150 Hz) can be held. Higher up the canceller always learns, and acts as a notch of the width. That is where the chords' partials crowd the harmonics, and a held estimate there did worse than a notch.
- For the first 0.3 s after sound starts, it learns at twice the rate with no hold.

**The tracker** is a frequency-locked loop on the mono sum:
- The first eight harmonics are each demodulated against the reference and low-passed (three poles, 3 Hz).
- Every 64 samples, each harmonic reads how far the hum's phase turned, divided by k.
- The reading used is the **weighted mean of the readings within 0.3 Hz of their weighted median**, weighted by strength. This is robust to a tone leaking into one band, without the median's own selection bias.
- The loop is **second order** (0.5 Hz, damping 0.8), so a steady drift is followed without lag. It is clamped to ±2 Hz of the nominal.
- **Auto** also listens at the other mains frequency's first four harmonics. It moves there when that is 6 dB stronger at two looks half a second apart.

**De-click and de-crackle** (`purus_click.rs`):
- Each channel keeps its input and its repaired output in rings.
- Every 512 samples, it judges the block that ended 256 samples ago:
  - The model is fitted to the whole window (1 024 samples of repaired context before the block, plus the block).
  - The suspects are marked and merged as the editor merges them.
  - Each click is widened and rebuilt from up to 1 024 samples before it and 192 after (the look-ahead).
- Stretches longer than 96 samples (2 ms) are left alone: they are a drum or a consonant. At most 32 clicks are rebuilt per block per channel. A block with no click is handed on bit for bit.
- **The latency** is `512 + 256 − 1 = 767` samples (16 ms), fixed.
- **De-crackle** (phase A-3, `purus_crackle.rs`) runs first, in the same window.
  - Impulses are found in a 5 kHz linear-phase high band (33 taps at 48 kHz), as local peaks over 9 to 5 robust scales.
  - Each is fitted as four samples of unknown amplitude against the high-pass's shape (least squares, the matrix factored once).
  - It is accepted only if it explains 60 % of the high band's energy over its support, then subtracted from the samples it sits on.
  - The de-click's pass then leaves impulses of four marks or fewer to it, and rebuilds only clicks 6 robust scales higher whose peak error is at least 1.5 × the block's level.
  - At crackle 0 the judgement is the editor's.

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
- When the input has been +0.0 for long enough, the unit **drains** to a new unit's state, checked on a 256-sample grid of the silence. That needs:
  - the de-reverb's last four frames under −180 dBFS;
  - the de-click's whole window and pending output under −180 dBFS;
  - every de-hum state under 10⁻¹⁵;
  - the silence longer than the latency.
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

**Gentle**, on the same steady hum: **37.6–42.6 dB**. This is unchanged by the look-ahead, and measured aligned to its latency.

**A bass on the hum's own frequencies** (plucked notes on 50, 100, 150, 55, 98 and 50 Hz, peak −6 dBFS):

| Mode | The bass on 50/100/150 Hz | The hum under the bass |
|---|---|---|
| Notches | loses **6.5–8.8 dB** | ≥ 40 dB down |
| Gentle (look-ahead) | loses **0.00–0.43 dB** (−0.12 dB in all; the 0.43 is the 150 Hz note, whose partials at 300 and 450 Hz are above the gated range) | ≥ **39.5 dB** down at the worst harmonic |

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

**The gentle mode on the CC0 guitar** (phase A-2): the owner's progression with the drifting hum
and buzz of the listening set, tracked, 16 harmonics. The figures are the hum left in dBFS,
followed along the hum's true phase, from 4 s on:

| Harmonics 1–8 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| Before | −30 | −34 | −33 | −40 | −39 | −45 | −43 | −48 |
| Notches | −63 | −61 | −57 | −61 | −56 | −76 | −61 | −65 |
| Gentle, first delivery | −63 | −39 | −46 | −53 | −40 | −75 | −61 | −65 |
| **Gentle, look-ahead (48 kHz, the listening set)** | **−68** | **−67** | **−55** | **−65** | **−58** | **−71** | **−62** | **−67** |

- **Within 5 dB of the notches on every harmonic**: 2 dB at the worst at 48 kHz, and 5.1 dB at the strums' own 44.1 kHz (`purus_gentle_guitar`, `#[ignore]`, which needs the strums on disk).
- The bass test still loses at most 0.43 dB.
- **What did it:** the look-ahead alone gained nothing on the guitar. The gains came from the 0.5 Hz reference, from turning the held harmonics, from the slower loop, and from narrowing the gated range to 160 Hz.

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

- **No musical noise added** at Direct sound 0 or +6 dB by this measure (every ratio is at or below 0). At +12 dB the ratio is +0.043.

**The Direct sound default, for the owner to choose** (phase A-2):

| Direct sound | A dry voice: its level | A dry voice: the change under it | The 0.8 s / 0 dB room at 12 dB: late reverb over the line | The room: tail after the line | The room: early-to-late gained | The room: the early part |
|---|---|---|---|---|---|---|
| **0 dB (now)** | −2.86 dB | 10.2 dB | −6.8 dB | −12.0 dB | +4.6 dB | −2.25 dB |
| **+6 dB** | −0.96 dB | 20.2 dB | −3.7 dB | −11.7 dB | +2.7 dB | −1.05 dB |
| **+12 dB** | −0.31 dB | 29.0 dB | −1.4 dB | −5.4 dB | +1.0 dB | −0.36 dB |

The trade-off:
- +6 dB keeps a dry voice almost intact and still takes the tail after words (−11.7 dB) nearly as well as 0 dB, but takes about half as much under the words.
- +12 dB is almost transparent and does little in a real room.
- My suggestion is **+6 dB** for a dialogue-first default, and 0 dB for "a far microphone" presets.
- **The decay estimated** from the line alone: 0.56 s for a 0.5 s room, 0.84 s for 0.9 s and 1.29 s for 1.5 s. That is within 14 %, and the longest room reads low.
- **A dry voice** (no room) loses level as follows:
  - At the defaults, which assume a direct sound only as strong as the reverberation: 2.86 dB, with the change 10.2 dB under the voice.
  - At 24 dB: 4.21 dB.
  - With **direct +6 dB**: 0.96 dB, with the change 20.2 dB under.
  - With **direct +12 dB**: **0.31 dB**, with the change 29 dB under.

### De-click and de-crackle (phase A-2)

**On synthetic music** (`purus_tests_click.rs`): a bass line, three tones with vibrato and noise at
−60 dBFS. It gets 40 clicks (0.1–0.5 peak, 1–8 samples) and/or crackle (300 impulses a second,
0.005–0.03 peak):

| Setting | Clicks down | Crackle down | Both down | Clean music: rebuilt, and the change under it |
|---|---|---|---|---|
| **sensitivity 0.5 (default)** | **26.5 dB** | 6.4 dB | 12.5 dB | **0 rebuilt** (the output is the input, delayed) |
| 0.5, crackle 1 | 26.2 dB | 6.8 dB | 12.8 dB | 0 |
| 1, crackle 1 | 18.8 dB | −3.1 dB | 5.6 dB | 0 |

The editor's offline declick on the same crackle: −4.5 dB at 0.5 and −6.8 dB (worse) at 1.

**On the CC0 guitar** (the listening set): 60 clicks, plus crackle:

| | Clicks alone down | Clicks and crackle down | On the clean DI |
|---|---|---|---|
| De-click, sensitivity 0.5 | **14.4 dB** | −1.4 dB | 64 rebuilt, the change 42.8 dB under the music |
| De-click and de-crackle (1) | 14.4 dB | −1.4 dB | 138 rebuilt, 42.8 dB |
| The editor's offline declick at 0.5 | 18.0 dB | 5.3 dB | — |

### The contract

- **Null:**
  - With every module off (the defaults, or settings moved but off), the output is the input, bit for bit and **undelayed**.
  - The de-hum's notches at mix 0 hand on its input bit for bit. Its gentle mode at mix 0 does too, **delayed by exactly its look-ahead** (2 400 samples, +0.0 before it).
  - The de-click at mix 0, and on with nothing to find, hands on its input bit for bit, delayed by exactly 767 samples.
  - The de-reverb at mix 0 hands on its input bit for bit, **delayed by exactly the latency stated** (1 023 samples, which are +0.0 before it). This also holds with amount 0.
  - At amount 0 and mix 1, the transform's round trip is within 2·10⁻⁶ of the input.
- **Block-size independence**, at tolerance 0: blocks of 1, 37, 64 and 512, over five settings. These cover the tracked gentle mode with 16 harmonics, Auto with the de-click and the de-reverb estimating, the de-click at full sensitivity and crackle, and the de-reverb alone. Settings of every kind are also changed mid-way, at the same samples whatever the blocks.
- **Delta:**
  - With every module on listening to its delta, the output is `x(t − L) − y(t)` to the bit (at an output level of 0 dB; within 10⁻⁶ with the level moved).
  - The de-hum's delta alone is `x(t − L) − h(t − L)`, the de-reverb's alone `h(t − L) − y(t)`, each to the bit. The de-click's is built the same way, between its own input and output; it is not tested on its own.
- **Rest:**
  - The unit comes to rest after **4.8–5.05 s of silence** with the de-hum on (the high-Q notches and the canceller must decay under 10⁻¹⁵), after **0.04 s** with only the de-reverb, and after **0.05 s** with only the de-click.
  - The sample the drain comes on is silent (< 10⁻⁹).
  - The twins agree to the bit on music afterwards: processing zeros, skipped, reset, and new (new only with the decay set, since reset keeps an estimate).
  - A new unit is at rest, and silence keeps it there.
- **No allocation after `new`**, under a counting allocator. This covers five settings twice, blocks of 1, 37, 64, 512 and 1 024, `set`, `meter`, `at_rest`, `latency`, rest, `skip` and `reset`: 0 allocations.
- **Deterministic maths:** only `crate::math` is used (clippy's `disallowed-methods`), and `sqrt` is IEEE.
- **CPU** (one core, 48 kHz, a minute of program in blocks of 256, best of three, `--release`). The first five rows are from the first delivery, on a quiet machine. The phase A-2 run below is on a busy one.

  | Setting | Share of a core |
  |---|---|
  | Notches, 8 harmonics, fixed | **0.42 %** |
  | Notches, 16, tracked | 0.53 % |
  | Gentle, 16, Auto | 1.93 % |
  | De-reverb | **1.01 %** |
  | Both (gentle 16 Auto, plus the de-reverb estimating) | **2.90 %** |

  The machine was a Ryzen 7 7735HS with no other `rustc` or `cargo` running before or after, at 7–18 % load.

  **Phase A-2**, with the new code:

  | Setting | Share of a core |
  |---|---|
  | Notches, 8 harmonics | 0.44 % |
  | Notches, 16, tracked | 0.55 % |
  | Gentle with look-ahead, 16, Auto | 2.43 % |
  | De-reverb | 1.10 % |
  | Gentle plus de-reverb | 3.48 % |
  | **De-click and de-crackle** (120 clicks and 300 crackles a second) | **1.61 %** |
  | **All three** | **5.53 %** |

  The machine was at 71 % load from other programs at the start, with three other `cargo` or `rustc` processes running by the end, so these numbers are probably high. The notches read 0.44 % against 0.42 % on the quiet run.

  **Phase A-3**, on a quiet machine (no other `rustc` or `cargo`, 17–18 % load):

  | Setting | Share of a core |
  |---|---|
  | Notches, 8 harmonics | 0.43 % |
  | Notches, 16, tracked | 0.54 % |
  | Gentle (look-ahead, phase-locked), 16, Auto | 2.28 % |
  | De-reverb | 1.02 % |
  | Gentle plus de-reverb | 3.24 % |
  | **De-click with the new de-crackle** | **1.27 %** |
  | **All three** | **4.49 %** |

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
  - `purus_gentle_guitar` (`#[ignore]`; needs the CC0 strums on disk). It also reports how far the reference strayed and how much of the time each harmonic was held. Use `PURUS_NODRIFT=1` for a steady hum.
  - `purus_cpu` (`#[ignore]`; run it alone, in release)
- **`purus_tests_click.rs`:** `clicks_and_crackle_come_out_and_the_music_stays`, and `purus_crackle_guitar` (`#[ignore]`; the CC0 strums).
- **`purus_tests_verb.rs`:**
  - `the_late_tail_comes_out_of_a_known_room`
  - `the_decay_is_estimated_from_the_signal`
  - `a_dry_voice_is_nearly_left_alone` (with direct +12 dB)
- **`purus_tests_shared.rs`:** `the_denoisers_outputs_are_bit_identical_to_before_the_sharing`, `the_declickers_outputs_are_bit_identical_to_before_the_sharing`.
- **`tests/purus_alloc.rs`:** `purus_allocates_nothing_after_new`.

**The suite:** `cargo test -p cantus-dsp --no-fail-fast` passes 939 in the library (20 ignored)
plus the integration tests. `cargo clippy -p cantus-dsp --all-targets -- -D warnings` is clean,
and `cargo fmt -p cantus-dsp` is applied.

## Shown to the owner

`C:\projects\daw-fx\compartir\purus\sound\` holds:
- The guitar (Carbonthief, CC0) with synthetic hum and buzz, de-hummed by the notches and by the gentle mode (re-rendered with the look-ahead), each with its delta.
- The guitar with clicks and crackle, through the de-click and through the de-click plus de-crackle, with deltas.
- A synthetic bass on the hum, through both modes.
- A synthetic spoken line in a synthetic room, de-reverberated at 12 and 24 dB with the decay estimated, with deltas.
- Dry twins, a README saying what to listen for, and `render-log.txt` with the numbers.

## Misses against the targets, honestly

- *(A-3: the steady-hum miss on the guitar is closed, with the 3rd harmonic at −75 dBFS against the notches' −58. The crackle miss is closed: 11.8 dB on the guitar, at 83 dB under clean music.)*
- **The bass on the drifting hum** (listening set, file 13) loses up to 0.7 dB on a 100 Hz note. The ≤ 0.5 dB holds on the test's steady 50 Hz hum (0.43 dB).
- **On the guitar, the de-crackle's threshold did not matter.** Crackle at 0.5 and at 1 found the same impulses, every one there was. On sparser, quieter crackle the control will matter. That case is not measured.
- **With the de-crackle on, the clicks on the synthetic test come down 16.3 dB instead of 26.5.** The clicks of 5–8 samples sit under the de-click's raised threshold. On the guitar the clicks still come down 18.8 dB.
- **Sensitivity 1 harms** the de-click alone (the editor's own declick does the same). The panel should cap it near 0.7.
- **The de-click alone (no de-crackle): the clicks on the guitar come down 14.4 dB against the editor's 18.0 dB.** The stream sees a 512-sample block with 1 024 of context, where the editor sees 2 048 with 1 024 before and after. A longer look-ahead would close part of the gap, at the cost of latency.
- **Latency:** the gentle de-hum's look-ahead is 50 ms, and all three modules together reach 87 ms. The de-reverb is 21.3 ms at 48 and 96 kHz and 23.2 ms at 44.1 kHz.
- **On a dry voice, the de-reverb's defaults cost 2.9 dB.** The Direct sound table above is for the owner to choose from.
- **Late reverberation during speech** comes down 6.8 dB at the defaults and 9.5 dB at 24 dB, while the tail between and after words comes down 12–24 dB. The early part pays 2–3 dB.
- **A fast swing of the mains** (±1 Hz every 10 s) leaves the 8th harmonic 25 dB down with the notches.
- **A hum that changes is relearned by the gentle mode in 3.5 s.**
- **Rest takes about 5 s of silence with the de-hum on**, because the 2 Hz notches ring down slowly.

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
   - Harmonics, depth and width are knobs. Gentle is a switch whose help line says it adds 50 ms.
   - The de-click gets sensitivity (perhaps capped near 0.7) and crackle. The starting preset could have de-crackle on at 0.5 when the de-click is on.
   - The de-reverb gets amount, decay, a Set/Estimate switch, Direct sound, smoothing and Early reflections kept.
   - Each module gets mix, output and delta; the unit gets the `out` key.
   - Also: a schema bump and fixture, commands with undo and the parameter menu, the undo fuzz arm, and the starting preset (ADR 0071): the de-hum on at 50 Hz, tracked, 8 harmonics, notches; the defaults stay the null.
2. **The engine unit** (`cantus-engine/src/fx/purus.rs`): an `FxUnit` over `Purus`.
   - `latency` comes from `latency_for`, so it changes when the gentle mode, the de-click or the de-reverb is switched. The engine's delay compensation must follow that, as it does for Sibilus's split modes.
   - `at_rest`, `skip` and `reset` pass straight through; `must_rest` holds.
   - The telemetry: the hum's fundamental, each harmonic's level, depth and held flag, the 48-band reduction, the decay estimated, the clicks rebuilt, and the removed level.
   - Tests: `tests/fx_purus.rs` (the null), the rows in `fx_contract.rs`, `rt_memory.rs`, and the bench kernel.
   - A per-bin reduction readout beyond the 48 bands, if the display wants more.
3. **A panel with its own personality**, unlike every existing unit.
   - The look: fine strokes, a **dark central display in both themes**, and a clinical, precise air, like a repair bench. It must not look like FabFilter's or iZotope's.
   - The display shows the **spectrum**, with **what is removed drawn in its own colour** (a cool cyan-white "residue" falling out of a warm spectrum, say). The hum's harmonics are a comb of fine needles at their true frequencies, each with its depth and a held marker in gentle mode. The de-reverb's reduction is a translucent band under the spectrum.
   - **Drag on the display:** the fundamental (in Fixed mode), the comb's width, and the de-reverb's amount as a level.
   - Three module strips (De-hum, De-click, De-reverb), each with an on switch, mix, output and a **Listen to what is removed** button. The clicks are drawn as fine ticks on a time strip as they are rebuilt.
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
   - A longer optional look-ahead for the de-click (the editor's context).
   - A per-band decay (T60 by octave) and an estimated Direct sound.

## Left (this phase)

- In the DSP: the misses above, which are the owner's to judge. The Direct sound default is the owner's choice.
- The owner's listening.
