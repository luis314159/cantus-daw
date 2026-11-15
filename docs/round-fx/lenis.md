# The effects side round, wave B — `lenis`: Lenis, the resonance suppressor (DSP phase)

Branch `fx/lenis` from `fx/integration` d3b51a09. **DSP only**: no `EffectKind`, no core
parameters, no engine unit, no panel, no ADR number yet. The name is provisional — one
constant, `cantus_dsp::lenis::NAME` (*lenis*: gentle, smooth). The function of the
soothe-class processors (oeksound soothe2, and the dynamic-EQ "resonance" modes of others)
is the reference; the algorithm is our own, from the literature cited below, never from a
product.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/lenis.rs` (the unit: settings, latency, the per-sample path, the frame, the meter, rest), `lenis_stft.rs` (the low-delay window pair, the two-channel packing), `lenis_detect.rs` (the detector, the curve, the focus); tests `lenis_tests.rs`, `lenis_tests_measure.rs`, `cantus-dsp/tests/lenis_alloc.rs`; example `cantus-dsp/examples/lenis_sounds.rs` | `cantus-dsp/src/lib.rs`: one line, `pub mod lenis; // fx-b:lenis` (after `latitudo`) |
| Docs | this file | — |

### The DSP API

```rust
use cantus_dsp::lenis::{Lenis, LenisSettings, LenisMeter, Channels, Mode, Quality, FocusBand,
                        latency_for, display_hz, DISPLAY, FOCUS_BANDS};

let mut u = Lenis::new(sample_rate);       // allocates here only (sized for the largest frame)
u.set(&LenisSettings { depth: 0.4, ..Default::default() }); // taken at the next hop
u.process(&mut left, &mut right);          // in place, any block size
u.latency();                               // samples: 511 at 48 kHz Standard (latency_for(&s, sr))
u.meter();                                 // LenisMeter, refreshed every frame
u.reduction_bins(0);                       // dB per bin, detector 0 (linked/left/mid) or 1
u.bin_hz(k); display_hz(p);                // bin and display-point frequencies
u.at_rest(); u.rest_blocker(); u.skip(n); u.reset();
lenis::detect::focus_weight(&s, hz);       // the focus curve, for drawing it
lenis::detect::reduction_db(excess, threshold, depth, mode); // the curve, for drawing it
```

`LenisSettings` (real units): `depth` 0…1 (shown as %), `sharpness` 0…1, `selectivity` 0…1,
`attack_ms` 0.5…200, `release_ms` 5…2000, `low_hz` / `high_hz` (the focus's limits, a
third-of-an-octave fade outside), `bands: [FocusBand; 6]` (`on`, `hz`, `amount` −1…+1,
`width_oct` 0.1…4 at half height), `channels` (`Linked`, `Independent`, `MidSide`), `mode`
(`Soft`, `Hard`), `quality` (`Fast`, `Standard`, `Precise`), `delta`, `mix` 0…1, `out_db`
±24. **`Default` is the null** (depth 0). There is no factory preset file yet: the
listening example's *gentle* and *strong* settings are the first two (see *Shown*).

`LenisMeter`: `level_db[128]` (the input's spectrum as the detectors hear it, dBFS, 128
points log-spaced 20 Hz – 20 kHz), `reduction_db[128]` (the most over the detectors and the
point's bins), `max_reduction_db`, `frames` (the editor sees when it moved).

### How it works (each file's header says it in full)

```text
     ┌──────────────── ring (the dry, delayed by the latency) ────────────────┬─ mix ─┬─ × out
in ──┤                                                                         │       │
     └─ every hop: frame ─ wa ─ FFT (L + jR) ─ detector(s) ─ gains ─ IFFT ─ ws ┘  delta: dry − that
```

- **The transform** (`lenis_stft.rs`): an STFT whose analysis window is long and whose
  synthesis window is short — the low-delay asymmetric pair of Mauler & Martin (EUSIPCO
  2007): √Hann rising over `N − M`, √Hann falling over `M`; the synthesis window is
  `Hann(2M)/wa` over the last `2M` samples, so `wa·ws = Hann(2M)` and the frames sum to a
  constant at a hop of `M/2`. `M = N/4`: a 1024-point analysis (47 Hz bins) with a latency
  of `2M − 1 = 511` samples (10.6 ms) at 48 kHz. Fast 512 (255 samples, 5.3 ms), Precise
  2048 (1023, 21.3 ms); doubled at 88.2/96 kHz and again above, so the times stay. Both
  channels go through one complex FFT (left real, right imaginary) and are taken apart by
  conjugate symmetry. The FFT is Aequalis's (`aequalis::fft`, twiddles in `f64` once).
- **A frame nobody turned down is not transformed back.** It adds its windowed input to
  the overlap straight from the ring (`wa·ws`), and an output sample that no turned-down
  frame covers is taken from the dry ring itself: the input delayed by the latency, to the
  bit. So at depth 0, at mix 0, on silence and wherever nothing sticks out, Lenis is a pure
  delay. All-zero frames skip the FFT too.
- **The detector** (`lenis_detect.rs`), per bin, per frame, for each detector (Linked: one
  on both channels' power; Independent: one each; Mid/Side: one on the mid, one on the
  side):
  1. **Two averages of the power**, 30 ms and 5 ms, flushed to +0.0 under −160 dBFS. The
     slow one catches up at the fast rate while the fast one stands more than 1 dB over it
     *and* the last frame's fast excess stood 3 dB over the threshold (an onset); otherwise
     it keeps its pace — noise's quick flickers never pass the second condition.
  2. **The upper envelope through the prominent peaks** (second pass; the first used a fixed
     ±120 Hz bridge): every local maximum is a candidate; one more than 6 dB under the line
     between its neighbouring candidates is a valley's ripple and is dropped (three passes);
     between two kept peaks no further apart than the lower one's frequency (+4 bins) the
     envelope is the straight line between them, or the spectrum where higher; then
     `[¼ ½ ¼]`. A bridge that adapts to any partial spacing; a resonance is a peak over its
     neighbouring peaks.
  3. **The envelope**: over a box of `W` octaves around the bin (Sharpness: 3 octaves at 0,
     3/8 of one at 1; never narrower than 7 bins a side), the greatest of the least-squares
     line through the box (against octaves: it follows any slope), the mean of the two sides
     beyond a 3-bin guard (each moved along the box's slope to the bin), and the higher raw
     side less an allowance — the "greatest of" cell-averaging detector of radar (GO-CFAR,
     Hansen & Sawyers 1980). The allowance (second pass) is **6 dB where the two sides
     differ as a slope makes them (≤ 12 dB) and 3 dB where they differ as a cliff does
     (≥ 24 dB)**: a broad peak on a steep spectrum is found, a band's edge is not taken for a
     peak. The peaks are clipped out (estimate + 1 dB) and the estimate made again, three
     times; every sum a difference of prefix sums with the constants precomputed — O(1) a bin.
  4. **Harmonic awareness under 800 Hz** (second pass): a candidate peak whose octave and
     twelfth are peaks too (or, for an odd series, its twelfth and seventeenth; within 2.5 %
     and 30 dB) is a partial of a note; it may stand over those partners by what a bright
     tone's partials do (`20·log10 m`: 6 dB over its octave, 9.5 over its twelfth, 14 over
     its seventeenth) before it counts, and its excess (its main lobe, and the line to the
     next partial) is capped at how far it exceeds that. A note's low partials are never
     resonances; a partial a resonance lifts over the series still is; an inharmonic ring (a
     drum shell, a room mode) has no partners and is judged by the envelope alone.
  5. **The excess** = upper envelope − envelope, on each time scale. The slow one says
     whether a resonance is there; once it says so (from the threshold, fully 2 dB over it)
     the fast one says how much (second pass: the onset measured at the fast pace, the noise
     judged at the slow one); the slow one is also raised to the fast one less 4 dB and held
     under the fast one plus 3 dB (it lets go as soon as the ringing stops).
  6. **The curve** (second pass): `depth · gate · excess`, never more than the excess and 30
     dB — **Depth is the share of each resonance's bump taken away**: 100 % brings it down to
     the envelope, 50 % half way, acting over the knob's whole travel (the first pass's
     `4·depth·(excess − threshold)` saturated past ~50 %). The gate opens as the excess
     crosses the threshold (Selectivity: 1.5 dB at 0 … 6.5 dB at 1): Hard over 2 dB, Soft as
     `e/(e + 4 dB)`. × the focus weight (0…2).
  7. **Spread and time**: the most of three neighbouring bins, then `[¼ ½ ¼]` (no bin is cut
     alone: a needle in frequency is a long ring in time, and an isolated bin switching on
     and off is the "musical noise" of spectral subtraction — Berouti et al. 1979, Cappé
     1994); then attack and release, one-pole in dB at the frame rate; released to +0.0
     under 10⁻³ dB.
- **On a grid**: frames every hop (128 samples at 48 kHz Standard) counted from the start
  of the stream; `set` waits for the next hop's first sample (a new unit takes it at once);
  the mix, the output gain and the delta switch glide 10 ms per sample, landing exactly.
- **Delta** = `dry − (the normal output before the output gain)`, computed as that
  subtraction: at 0 dB out it is the input at the latency minus the output, to the bit.
- **Exact rest**: after `N + 2M` exact zeros in, with every average and reduction +0.0, no
  frame still turned down, no pending setting and the glides landed. `skip(n)` advances the
  stream clock; `reset()` clears the audio state **and keeps the clock** (the frame grid),
  so a reset twin and a processed one agree to the bit.

### The design choices

- **STFT against a filterbank.** A filterbank would have no block latency, but the
  detector needs a resolution well under a resonance's width (a 1/12-octave peak at 3 kHz
  is 175 Hz: four bins of 47 Hz) over the whole band — 150+ bands a channel, each a
  dynamic bell whose overlapping phases make ripple when neighbours cut together — and a
  per-band envelope follower can't see the *shape* of the spectrum around a band, which is
  what tells a resonance from a loud region. The STFT gives the whole spectrum each frame
  for O(N log N) and applies any smooth per-bin gain exactly. Not measured against a built
  filterbank (none was built): argued, and the STFT's own costs measured — latency 10.6 ms
  (target 10–20 ms met), pre-echo under −27 dB, CPU below.
- **The window pair, measured** (`the_window_pairs_compared`, a +12 dB Q 8 peak at 3 kHz
  and a +12 dB Q 6 one at 250 Hz on pink noise, Hard 50 %; a struck 2.5 kHz resonance's
  pre-echo in the 20 ms before each strike): at the same 10.6 ms latency the asymmetric
  1024/256 pair cuts the 250 Hz peak by −2.5 dB against −1.5 dB for a symmetric 512, with
  the same pre-echo (−27.7 / −27.5 dB under the strike) and the same 3 kHz cut (−9.4 /
  −9.7); a symmetric 1024 needs twice the latency (21.3 ms) for −2.9 dB and has more
  pre-echo (−25.4). So Standard is 1024/256; Precise (2048/512, 21.3 ms) is for the low
  mids (−4.6 dB at 250 Hz).
- **What a resonance is**: the spectrum against its own smooth envelope, as asked, with two
  refinements found by measuring: the *upper envelope over the partials* (without it a sung
  note's harmonics stood 20–40 dB out of their envelope and the first listening render of the
  voice lost 13 dB), and the *greatest-of* envelope (without it the knee of a roll-off — a
  low-passed source, the top of a band-limited file — stood +38 dB out). Cepstral smoothing
  was considered and not taken: a lifter short enough to bridge harmonics blurs narrow
  resonances, it is pulled up by the peaks it should stand under, and it can't be made
  edge-safe.
  The second pass added two more, also from measurement: the upper envelope drawn through
  the *prominent peaks* instead of a fixed bridge (it adapts to any partial spacing), and
  *harmonic awareness* under 800 Hz (a note's low partials, which no bridge can join to
  a low fundamental, are recognised as a series — van Waterschoot & Moonen's survey of
  howling detection, 2011, lists the same harmonicity test among its features — and judged
  against their partners instead of their envelope).
- **Smoothing in time and frequency, against musical noise and zipper**: the 30 ms power
  average (measured: noise's excess 99.9th percentile +3.4 dB against +8.9 for a 12 dB
  resonance; in the first detector 15 ms gave +5.7 against +9.2 and 30 ms +4.4 against
  +9.1), the spread over three bins, attack and
  release in dB, the gains applied per bin in the frequency domain with a window pair that
  sums exactly (no zipper by construction: a gain changes once a hop, cross-faded by the
  overlap).

## The second pass: before → after

The integrator's review asked for five things before Phase B. Every null and bit-exact test
is unchanged and still passes.

| Asked | Before (first pass) | After (second pass) | Target |
|---|---|---|---|
| 1. A Q 4, +6 dB peak on a 12 dB/octave slope | +3.0 dB of excess (the 2.7 kHz cabinet peak barely touched: guitar Strong −0.9 dB at 2 kHz, −0.5 at 4 kHz) | **+5.1 dB of excess**, cut −3.6 dB at Hard 100 %, the slope around it ≤ 0.01 dB; guitar Strong now **−2.8 dB at 2 kHz, −3.8 at 4 kHz**, −1.7 at 500 Hz, with the focus open from 150 Hz | ≥ 4 dB ✔ |
| 2. A steady note's partials under 800 Hz at a strong setting (Hard 80 %, selectivity 25 %) | −1.2 / 0 / −3.4 / −4.6 dB (110 Std / 110 Precise / 220 / 330 Hz, 400–800 Hz) | **0.00 dB** in 150–800 Hz at all four; the detector's excess there p99 +1.0 dB (was +12.5); an inharmonic ring at 550 Hz among a 220 Hz note's partials still cut −12.7 dB (Precise; the partials ≤ 0.7 dB) and −2.7 dB (Standard: 2.3 bins from the 440 Hz partial, which takes −3.1 — the resolution's limit) | ≤ 0.5 dB ✔ |
| 3. 90 % of the reduction after an onset (attack 1 ms) | 45 ms (half at 13 ms) | **21 ms** Standard, 20 ms Fast, 35 ms Precise (half at 7–8 ms); noise before the tone < 0.5 dB | ~15 ms ✘ (partial) |
| 4. Hard saturating past 50 % | −9.6 / −9.7 / −9.7 dB at 50 / 75 / 100 % | **−5.7 / −8.6 / −11.4 dB**, Soft −3.8 / −5.6 / −7.4 (25 %: −2.9 / −1.9): linear to 100 % | ✔ |
| 5. A Quality change | resets the unit (a gap of a latency) | unchanged — **left for the engine**, see below | — |

- **Why 21 ms and not 15.** The tone has to fill the 1024-point analysis frame (21 ms; its
  window weighs the newest samples most) and the 5 ms fast average. Tried and measured: a
  3 ms fast average gives 19 ms but raises noise's 99.9th-percentile excess from +4.0 to
  +5.2 dB and the removed part of pink noise at depth 100 % from −44 to −40 dB; the slow
  average catching up ungated (6 dB) gave 21 ms with flickers of 2–3 dB on noise before the
  tone. The gated catch-up keeps 21 ms with the noise calm. Fast quality's frame is half
  as long but its bins twice as wide, so the excess settles no sooner (20 ms). Below 15 ms
  needs a shorter frame than the resolution allows; I'd not trade the noise for 6 ms.
- **Why a Quality change can't crossfade.** The three qualities have different latencies
  (255 / 511 / 1023 samples); a crossfade between two outputs delayed differently is a
  comb filter and an audible slip, and the host must re-align the track anyway. The engine
  should treat Quality like Sibilus's look-ahead: report the new latency, let delay
  compensation re-align, and accept the one-latency gap (5–21 ms) the reset leaves — or
  ramp the output down and up over it. Noted in Phase B.
- **The costs of the second pass, measured**: noise's excess 99.9th percentile +3.4 → +4.0 dB
  (pink noise at the defaults with depth 50 %: removed part −53 → −50 dB, worst
  third-octave 0.01 dB either way); a pink noise low-passed with eight poles at 5 kHz:
  99.9th +2.3 → +5.3 dB at its knee (the slope allowance; a synthetic brick-wall is the
  worst case — the 220 Hz note's partials stopping dead at 16 kHz read +6.9 at its p99
  there, which the threshold mostly covers); CPU ≈ +0.2 % of a core.

## Measured (48 kHz; `cargo test -p cantus-dsp --release --lib lenis -- --nocapture --include-ignored`)

- **The null**: depth 0, and mix 0, at every quality × channel mode × curve (38 cases, the
  focus's limits above the band among them): the output **is** the input delayed by
  `latency()` (511 / 255 / 1023 samples), bit for bit, on both channels. The dry path is
  the ring itself, so the latency compensation is exact; a host aligns Lenis by
  `latency()` like any other unit.
- **Flat noise barely touched** (8 s; the removed part's level under the input, and the
  worst third-octave band's change): at the defaults with depth 50 % — pink **−49.7 dB**,
  0.01 dB; white −47.5 dB, 0.01 dB. Depth 100 %: pink −44.1 dB / 0.02 dB. Only at
  selectivity 0 (a threshold of 1.5 dB) does noise get worked: pink −26.0 dB / 0.66 dB.
- **A resonance cut, its neighbours kept** (pink noise with +12 dB at 3 kHz, Q 8; the peak's
  sixth of an octave, and 200 Hz–1 kHz, 1–2 kHz, 5–10 kHz, 10–16 kHz): Soft at depth
  25/50/75/100 % **−1.9 / −3.8 / −5.6 / −7.4 dB**; Hard −2.9 / −5.7 / −8.6 / −11.4 dB;
  every neighbouring region within **0.02 dB**.
- **Against sharpness** (Hard 50 %; a narrow Q 20 peak at 4 kHz and a broad Q 1.5 bump at
  1.5 kHz, both +10 dB): sharpness 0 / 0.25 / 0.5 / 0.75 / 1 — narrow −5.2 / −5.1 / −4.9 /
  −4.6 / −4.1 dB; broad −4.8 / −4.1 / −2.8 / −1.0 / −0.7 dB.
- **A broad peak on a steep slope** (Q 4, +6 dB at 3 kHz on 12 dB/octave from 1 kHz):
  excess +5.1 dB; Hard 100 % −3.6 dB at the peak, 1–1.8 kHz and 5.5–9 kHz ≤ 0.01 dB; the
  slope alone ≤ 0.03 dB everywhere.
- **Notes are not resonances** (a steady note, harmonics at 1/k with a slow vibrato): Soft
  50 % — the note alone ≤ 0.10 dB everywhere at 110 (Standard and Precise), 220 and 330 Hz;
  with a +10 dB Q 8 peak at 3 kHz the peak −1.3 / −1.3 / −0.9 / −1.3 dB. Strong (Hard 80 %,
  selectivity 25 %): the peak −4.1 / −3.8 / −3.4 / −3.7 dB, 150–800 Hz **0.00**, 800 Hz–2 kHz
  0.00, 5–15 kHz ≤ 0.21 dB.
- **An inharmonic ring among low partials** (a 220 Hz note and a ring at 550 Hz ~11 dB over
  its neighbouring partials; Strong): Precise −12.7 dB, the partials at 220 / 440 / 660 Hz
  0.0 / −0.1 / −0.7; Standard −2.7 dB, the partials −0.1 / −3.1 / −0.1 (the ring is 2.3 bins
  from 440 Hz).
- **The detector's statistics** (`diagnose_the_excess`, the excess over 800 Hz–16 kHz, and
  150–800 Hz): pink noise median 0.0 dB, 99th percentile +2.8, 99.9th +4.0 (150–800 Hz p99
  +0.7); a +12 dB Q 8 resonance on it +10.6; a 220 Hz note p99 +6.9 (its band-limited top),
  under 800 Hz p99 +1.0; its +10 dB 3 kHz peak +6.4; pink low-passed at 5 kHz p99 +3.2,
  99.9th +5.3; the Q 4 peak on the slope +5.1. With the first pass's fixed bridge: the note
  under 800 Hz +12.6, the slope peak +2.0.
- **Time response** (a 3 kHz tone, −20 dBFS, over pink noise from 1 s to 2 s, Hard 60 %; it
  is held at −12.3 dB): Standard, attack 1 / 5 / 20 / 50 ms — half the reduction **8 / 13 /
  24 / 45 ms** after the onset, 90 % **21** / 32 / 67 / 136 ms; release 50 / 80 / 200 / 500
  ms — half let go 59 / 80 / 173 / 381 ms after the end, 90 % 179 / 227 / 496 / 1 187 ms.
  Fast (attack 1 ms) 90 % in 20 ms, Precise 35 ms. Nothing before the tone (< 0.5 dB). The
  release is now gradual (the cut is a share of the excess, which fades as the ring does):
  90 % ≈ 2.3 × release + 60 ms.
- **The window pairs** (Hard 50 %): at 10.6 ms the asymmetric 1024/256 pair cuts the 3 kHz
  peak −5.7 dB and a 250 Hz peak −0.9 dB, against −5.2 / −0.4 for a symmetric 512; pre-echo
  −29.9 dB (512: −31.5; symmetric 1024 at twice the latency −27.8). Precise (2048/512)
  −1.3 dB at 250 Hz.
- **The latency, checked on the signal**: processed hard, the output's cross-correlation
  with the input peaks exactly at `latency()`; 255 / 511 / 1023 at 48 kHz, 511 at 44.1,
  1023 at 96, 2047 at 192 kHz (Standard).
- **Delta** = input at the latency − output, **bit for bit**, at mix 100 % and 60 %,
  Linked, Independent and Mid/Side.
- **Block size**: 1 / 37 / 64 / 512, a setting changed mid-stream (depth and curve; the
  channel mode with mix, output and delta; Fast to Precise; the defaults to heavy):
  identical to the bit. A setting given anywhere in a hop is taken at the next hop.
- **Rest**: at rest 1.2 s after the music stops (Standard and Precise, release 60 ms);
  19.8 s with a 2 s release. Processing zeros, skipped, reset-and-skipped, and a new unit
  that heard only silence for as long: the same bits on music afterwards.
- **No allocation after `new`** (`tests/lenis_alloc.rs`): at 44.1, 48, 96 and 192 kHz,
  every quality × channel mode × curve with the delta and a focus band, settings changed
  mid-stream, blocks of 1 to 1024, the meter, `reset`, `at_rest`, `skip`: **0**.
- **Deterministic maths**: `cantus_dsp::math::*` only (clippy's `disallowed-methods` clean
  in the library, the tests and the example); the detector's `log10` is our own plain
  arithmetic (within 2·10⁻⁶ of `math::log10`, tested).
- **CPU** (one core, 48 kHz, a minute of a harsh stereo signal in blocks of 256, release,
  best of three; **two other builders' `rustc` running at the start, none at the end**):
  Standard **Linked 3.2 %**, Independent 5.7 %, Mid/Side 5.6 %; Fast 3.2 / 5.5 / 5.5 %;
  Precise 3.3 / 5.9 / 5.8 %; at depth 0 1.0–1.6 %. (First pass, quieter machine: 3.0 / 5.0 /
  5.0 % Standard.)
- **The suite**: `cargo test -p cantus-dsp --no-fail-fast` — 940 passed in the library
  (Lenis's 20, two more ignored: the CPU and the detector's statistics), every integration
  test and `lenis_alloc` green; `cargo clippy -p cantus-dsp --all-targets -- -D warnings`
  clean; `cargo fmt -p cantus-dsp` applied.

### Tests and what each holds

`lenis_stft.rs`: `the_window_pair_reconstructs` (five shapes, the overlap sums to 1 within
10⁻⁶, the FFT path's window equals the direct one), `two_channels_share_one_transform`.
`lenis_tests.rs`: `depth_zero_and_mix_zero_are_the_input_delayed_to_the_bit`,
`the_latency_is_fixed_and_reported`, `the_output_does_not_depend_on_the_block_size`,
`a_setting_given_anywhere_in_a_hop_is_taken_at_the_next_hop`,
`delta_is_the_input_minus_the_output_to_the_bit`,
`silence_in_is_silence_out_and_a_new_unit_is_at_rest`,
`it_comes_to_rest_exactly_and_its_twins_agree`, `the_meter_shows_the_resonance_where_it_is`,
`the_focus_curve_is_one_where_nothing_acts_and_follows_its_bands` (and the curve's shape),
`the_detector_s_log_is_close_to_the_library_s`. `lenis_tests_measure.rs`: the measurements
above (`a_resonance_on_pink_noise_is_cut_and_its_neighbours_kept`,
`flat_noise_is_barely_touched`, `the_reduction_follows_depth_and_sharpness`,
`a_note_s_harmonics_are_not_resonances_but_a_peak_over_them_is`,
`a_broad_peak_on_a_steep_slope_is_found`, `an_inharmonic_ring_among_low_partials_is_still_cut`,
`the_time_response_follows_attack_and_release`, `the_window_pairs_compared`;
`cpu_at_48k` and `diagnose_the_excess` ignored). `tests/lenis_alloc.rs`.

## Shown to the owner

`C:\projects\daw-fx\compartir\lenis\sound\` (replaced by the second pass) — the guitar
(Carbonthief, CC0) through two of Fragor's pedals and a honky cabinet, a synthetic harsh
sung line with whistling "s" sounds, a synthetic boxy kit; each dry, *gentle* (Soft 50 %),
*strong* (Hard 90 %, selectivity 30 %) and the strong *delta*; level-matched, aligned;
`README.md` (what to listen for, what changed) and `render-log.txt` (settings, the meter,
the change per octave band). The guitar's focus now opens from 150 Hz (700 Hz in the first
set); the voice's from 1 kHz (its synthetic formants are narrow enough to ring by design),
the drums' from 300 Hz (their fundamentals are their body).

## Verified only in emulation / needs the owner

- Everything is DSP-only and measured on synthetic signals and one recorded guitar; **the
  sound is for the owner's ears**: whether the deltas sound like rings and whistles (right)
  or like a thin copy of the instrument (too much); any watery or chirping artefact; any
  smear of the attacks; whether the guitar sounds less harsh (the second pass now acts
  where its cabinet rings, 2–4 kHz) and not thinner; whether the drums, their ping and
  shell ring cut −20 dB in the 1 kHz octave by Strong, sound too dry of their ring.
- No real music beyond the guitar DI: a mix, a real vocal and real drums would say more.

## What the later phases need (Phase B)

1. **Core**: an `EffectKind` (the contract's `ALL` index), `PARAMS` for every field — depth,
   sharpness, selectivity (%), attack, release (ms), low and high limits (Hz), six focus
   bands × (on, frequency, amount %, width in octaves), channels (Linked / Independent /
   Mid-Side), curve (Soft / Hard), quality (Fast / Standard / Precise), delta, mix, output —
   plus the `out` key; schema bump and fixture; commands with undo and the parameter menu
   (automate, modulate, MIDI learn); the undo fuzz arm; the starting preset in
   `starting.rs` (ADR 0071: the defaults stay the null).
2. **Engine unit** (`cantus-engine/src/fx/lenis.rs`): `FxUnit` over `Lenis` — `latency`
   from `latency_for` (**it changes with Quality**: the engine must re-report the delay
   compensation, as for Sibilus's look-ahead; a quality change resets the unit — a
   5–21 ms gap, documented — a crossfade cannot bridge two latencies, see the second pass); `at_rest`/`skip` straight through; `reset` keeps the stream
   clock (say so to the engine's reset semantics); the meter (`LenisMeter`, 2 × 128 floats a
   frame) to the editor's telemetry; `tests/fx_lenis.rs` (the null — delayed by the
   latency), its rows in `fx_contract.rs` and `must_rest`, `rt_memory.rs`, the bench kernel.
3. **A panel with its own personality — the design sheet first**, for the owner's
   approval. A proposal to draw: *Lenis* is calm water over rough stones. The **centre is a
   dark display** in both themes (Slate and Paper keep it dark): the live input spectrum as
   one fine bright line, and the **reduction hanging under it per frequency** as a
   translucent veil whose depth is the cut (dB scale on the right), so the eye sees *where*
   it smooths and *how much*, moving at 60 Hz and settling to a still line at rest (no dead
   slab, no −∞); over it the **focus curve** as a thin band with **draggable handles** — two
   vertical gates for the low and high limits, a node per band (drag in frequency and
   amount, a wheel or side handles for the width), double-click to add, right-click for
   the band's menu. Under the display, controls grouped by function with label/value units,
   nothing abbreviated in English or Spanish: *Depth / Profundidad* (the large one),
   *Sharpness / Nitidez*, *Selectivity / Selectividad*; *Attack / Ataque*, *Release /
   Liberación*; *Curve: Soft · Hard / Curva: Suave · Dura*; *Channels: Linked · Independent ·
   Mid/Side / Canales: Enlazados · Independientes · Medio/Lados*; *Quality: Fast · Standard ·
   Precise / Calidad: Rápida · Estándar · Precisa* with its latency in ms beside it; *Listen
   to what is removed / Escuchar lo que se quita* (the delta); *Mix / Mezcla*, *Output /
   Salida*. A material of its own (e.g. a pale stone-grey plate with fine engraved strokes
   and a single cool accent), unlike any existing unit's; Aequalis's curve and Delay's
   window as the quality bar; FabFilter-class and oeksound-class windows as references for
   function, never copied. At 75–200 %, EN and ES, Slate and Paper, at rest and playing;
   the mini view under the insert (the veil alone); the exported CLAP/VST3 window the same.
4. **Presets**: ten or more, named and made by ear — the starting preset (Soft, ~50 %,
   from ~200 Hz), *Gentle polish*, *Harsh vocal* (from 1 kHz, focus on 2–5 kHz), *De-ess
   wide* (focus 5–10 kHz), *Boxy drums* (from 250 Hz, focus 400–900 Hz), *Fizz tamer*
   (guitar, 2–8 kHz), *Room ring* (Precise, low mids), *Mix bus air* (Linked, light, high
   shelf focus), *Strings and brass*, *Mid only* / *Side only* (Mid/Side with the side or the
   mid focus), *Surgical* (Hard, sharp) — each a `LenisSettings`, fitted to level.
5. **Export**: the CLAP/VST3 descriptor, bit-identical to the engine (the latency reported
   to the host, and re-reported on a quality change); golden ids appended.
6. **Docs**: the ADR (the low-delay asymmetric window, the upper envelope over partials,
   the greatest-of envelope with moved sides, the two time scales, the "untouched frames are
   the dry" null), `docs/plugins/lenis.md`, the manual EN/ES, the AI tools' description, the
   site's unit card.
7. **End to end**: insert it from the picker on a playing track through
   `CantusApp::assemble`, move Depth, hear the master change (and the delta switch), with
   the delay compensation holding the track in time.

## Left (this phase)

- **The attack's last step**: 90 % of the reduction in 21 ms (Standard), not the ~15 ms
  asked — bounded by the analysis frame; going lower costs noise robustness (measured
  above). The owner's ears decide whether it matters.
- **The resolution's limit under 800 Hz at Standard**: a ring within ~2 bins (100 Hz) of a
  note's partial takes the partial with it (−3.1 dB beside a −2.7 dB cut); Precise resolves
  it. Phase B could suggest Precise when the focus reaches below ~300 Hz.
- **A brick-wall band edge** (a synthetic tone stopping dead, an 8-pole low-pass) reads up
  to +5–7 dB at its knee since the slope allowance widened; real program rarely has one,
  and the threshold covers most of it.
- A Quality change stays a reset (for the engine, above).
- The owner's listening; then the phases above.
