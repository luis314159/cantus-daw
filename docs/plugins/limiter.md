# Terminus — the limiter

`EffectKind::Limiter` · parameters `crates/cantus-core/src/fx/limiter.rs` · DSP
`crates/cantus-dsp/src/terminus.rs` (+ `terminus_os.rs`, `terminus_loudness.rs`) · engine
`crates/cantus-engine/src/fx/limiter.rs` · editor `crates/cantus-ui/src/views/fx/limiter.rs` ·
strings `crates/cantus-ui/locales/fx/limiter.{en,es}.txt` · tests
`crates/cantus-engine/tests/fx_limiter.rs`, `crates/cantus-ui/tests/fx_limiter.rs` and the DSP
modules' own. Benchmark: FabFilter Pro-L 2 (`research/fabfilter/pro-l.md`).

## What it does

A mastering limiter in two stages, with true-peak limiting that is measured rather than
claimed, BS.1770-4 / EBU R128 loudness, delivery profiles with a written verdict, dither,
and a history of what it did that is kept **in song time** — the one thing a plug-in
cannot do, because it does not own the timeline.

```text
in ─ DC filter ─ gain ─ clipper (4×) ─┬─ delay ─────────────── × gain ─ true-peak safety ─ clamp ─ 1:1 / match / Δ ─ dither ─ out
                                      └─ detector (4× TP) ─ transient stage ┐
                                                          ─ release stage  ┘ larger of the two
```

- **Transient stage**: a look-ahead brick wall. A running maximum over the attack ramp,
  then one moving average (a straight ramp) or two (an S-curve) whose lengths add up to the
  ramp, all in whole micro-decibels (`i64`). Every value averaged is at least the peak's
  requirement, so the average is too: the sample peak cannot pass the ceiling, and integer
  arithmetic means no rounding drift however long it runs. It lets go with the style's own
  time (2.5–150 ms).
- **Release stage**: an envelope on the same requirement with the Attack and Release knobs.
  A peak shorter than the attack barely moves it; a sustained overload does, and then it
  decides how the reduction recovers. The larger of the two stages is applied.
- **Per-stage stereo link** (Link transients, Link release), as in the benchmark: a
  one-sided transient can be caught on its side alone while the recovery stays centred.
- **True peak**: the detector reads the waveform *between* the samples — a 4× polyphase
  Kaiser-sinc interpolator (33 taps per phase) refined by a parabola through the three
  points around each local maximum. A second, always-linked stage then measures the
  *output* the same way and removes what the gain modulation put back between samples
  (0.15 dB of internal margin, a 1 ms S-curve ramp, 5 ms release).
- **Clipper** (P1): 4× oversampled soft clipper before the limiter. It computes only *what
  clipping changes* at 4× and adds that back, band-limited (below 0.40 of the sample rate,
  round 19), to the delayed input — so with nothing to clip it is an exact delay. The knob moves its threshold from 6 dB over the
  ceiling down to the ceiling: more of each peak clipped, less limited.
- **Knee** (P1): 0–12 dB soft knee *under* the ceiling; the static curve reaches the ceiling
  with slope zero and never passes it.
- **Monitor**: *Out*; *1:1* (the output turned down by the gain, as the benchmark's Unity
  Gain); *Match* (the output levelled to the input's measured 3 s loudness, K-weighted,
  gliding over a second and holding through silence — so styles, A/B and settings compare
  at equal loudness); and **Δ** (what the limiter takes away), at either level.
- **Dither**: TPDF (2 LSB peak to peak) at 24/22/20/18/16 bits, flat, *Basic* (first-order
  error feedback) or *Optimized* (Lipshitz's five-tap curve). Off by default, a wire when
  off, and **silent on digital silence** after 64 zero samples (a request the benchmark's
  users never got).
- **DC filter**: a 5 Hz first-order high-pass before the gain.
- **Trigger** (sidechain): the detector can listen to another strip — stem mastering, every
  stem limited by the reduction the full mix needs. The true-peak safety stage still holds
  each stem's own ceiling.

### Styles are settings you can read

| Style | Attack ramp | Transient stage lets go | Release stage |
|---|---|---|---|
| Clean | S-curve over the whole look-ahead | 40 ms | as set; slows to 2× under sustained reduction |
| Punch | straight, over the last 40 % | 10 ms | as set; quicker (to ½) when light |
| Loud | straight, over the last 15 % | 2.5 ms | attack 2× later; quicker when light |
| Bus | straight, over the whole look-ahead | 30 ms | attack ×¼: engages early and pumps |
| Safe | S-curve over the whole look-ahead | 150 ms | takes every overload at once; slows to 3× |

The editor prints this row for the chosen style, with what it is for. Five clearly
different styles rather than the benchmark's eight overlapping ones (the research's
advice); the test below shows they are ordered as the table says.

## The editor

- **Two views of the history.** *Live* scrolls (6, 20 or 60 s) and can be frozen. *Song*
  stores every 10 ms column at its bar and beat while the transport plays: playing bars
  33–48 again **replaces** bars 33–48. It scrolls (drag, wheel) and zooms (Ctrl+wheel) like
  the arrangement, has a ruler in bars, shows the arrangement's time selection and the
  playhead, and follows or stays put. Hover reads input, output, reduction and short-term
  loudness at that moment, with its bar and beat.
- **Peak labels** on the largest reductions, spaced apart; in the Song view each is a
  button that moves the playhead there.
- **Loudness measured three ways at once**: since the last reset (pause, reset, reset on
  play); over **the song as last heard** (each bar at its latest take); and over **the
  arrangement's time selection**. The last two come from the history in song time, gated
  as BS.1770 gates (momentary blocks every 100 ms for the integrated value, short-term ones
  for the range).
- **The display is the control**: drag the dashed ceiling, the target line (on the display
  or on the loudness meters), or the gain strip on the left edge; Alt on the gain strip
  moves the ceiling the other way, for level-matched listening. Every drag is one undo
  step. Reduction has its own lane and scale (0–12 dB), so it is never lost behind the
  waveform; loudness is drawn on the same dB scale as the levels, so no connector lines.
- **Meters**: reduction and output true peak per channel with held peaks (click to reset),
  momentary / short-term / integrated bars against the target — what is over the target is
  hatched, not only coloured.
- **Delivery profiles**: Spotify (−14 LUFS, −1 dBTP), Apple Music (−16, −1), YouTube (−14,
  −1), EBU R128 (−23 ±0.5 LU, −1), ATSC A/85 (−24 ±2, −2), or a custom target. Choosing one
  sets the target, the ceiling and true peak in **one undo step** and prints its rule in a
  sentence. The **verdict** is written ("2.8 LU loud: it will be turned down by 2.8 dB",
  "true peak 0.6 dB over") next to a shape — a filled disc for a pass, a hollow triangle
  for a miss — so colour is never the only cue.
- **Latency is always on screen** ("latency 3.88 ms (186)"), and a warning appears when the
  attack ramp is under 0.1 ms, where the limiter behaves like a clipper.
- **Presets**: Transparent master, Streaming master (−14), Loud and dense, Drum bus punch,
  Mix bus glue, Broadcast safe, CD master with 16-bit dither, Tracking (no latency).
- Every string in English and Spanish.

## Decisions, and why

1. **Detection-only oversampling; the gain is applied at the base rate.** The benchmark
   runs its whole limiter oversampled (linear phase, with pre-ringing its manual admits).
   Here only the detectors run at 4×; the audio is only ever delayed and multiplied by a
   smooth gain. That keeps the wire exact, the latency small, and the audio free of
   resampling filters. The gain curve is band-limited by construction (moving averages over
   ≥ 1 ms), so what a limiter's gain modulation adds above the band is small; the clipper,
   which is a hard nonlinearity, is the one stage that runs at 4×.
2. **Micro-decibel integers in the transient stage.** A running maximum and moving averages
   in floating point drift (a running sum subtracts what it added, never quite exactly),
   and a limiter's promise is exact. Integers make "the average is at least every value in
   it" hold forever.
3. **A second true-peak stage after the first**, rather than a larger internal margin. The
   first stage already holds the reconstructed peaks of its *input*; the second measures
   the *output* and removes only what the gain changes put back between samples. The
   margin was first set at 0.05 dB from the measurement below (at 0.02 dB the worst case
   was +0.002 dB over: Loud, zero look-ahead, +18 dB, tones at 11–19 kHz). **Round 19
   raised it to 0.15 dB**, on a measurement that corpus missed: drums and hats split into
   three bands by Culmen, squeezed and driven 18 dB into Terminus (Loud at 0.5 ms, Safe at
   5 ms) read **+0.032 dB** over the ceiling by the 32× reference with the clipper off and
   **+0.244 dB** with it at 100 %, while the 4× detector read exactly the −0.05 it aimed
   for. The detector's reconstruction is flat to 0.44 of the sample rate and rolls off to
   −6 dB at Nyquist, so energy in that last tenth — what a fast gain puts around bright
   hats, and above all what a clipper makes — is read 0.08–0.29 dB low. Two changes,
   measured by `terminus::tests_tp`: the margin is 0.15 dB (worst over that grid with the
   clipper off: −0.068 dB; on the corpus below: −0.100 dB), and the clipper's residual
   comes back through its own low-pass at 0.40 of the sample rate (Kaiser, the
   prototype's length and centre, so the latency is unchanged), which leaves nothing in
   the band the detector reads low (clipper at 100 %: +0.244 → −0.016 dB). Where the
   safety stage does nothing — a signal under the ceiling, or held by the first stage with
   room to spare — nothing changed; where it works, a true-peak-limited peak sits 0.1 dB
   lower than before. Culmen's own 0.05 dB (`TP_MARGIN_DB`) now adds to this one.
4. **Telemetry as a ring of 10 ms columns with a counter**, not a last-block snapshot: the
   engine keeps only the latest `Meters`, so a snapshot would lose the columns between two
   frames. Eleven columns (110 ms) cover a late frame; the editor reads the counter, takes
   only what it has not seen, and treats a counter that went back as a new stream.
5. **Loudness is integrated in the editor, from the same columns**, by
   `ColumnLoudness` — the code the EBU conformance tests check. That makes reset, pause,
   reset-on-play and *measuring any stretch of the song afterwards* possible without a
   message channel to the audio thread (there is none for plug-ins, by design). The price:
   the integrated reading covers what played while the editor was open. The history in
   song time makes that a smaller price than it sounds — open the editor, play the song
   once, and every bar is measured.
6. **Song time is stamped in the editor** from the transport position at each frame, minus
   the limiter's latency, one column back per 10 ms. Buckets are 1/32 of a quarter note; a
   new pass (play pressed, or the playhead jumping back) replaces a bucket instead of
   merging into it.
7. **Five styles, each a row of numbers.** The benchmark's eight styles are opaque and its
   users cannot tell several apart; the research recommended four or five clearly
   different ones. The zero-latency "tracking" use is a factory preset (Safe, no
   look-ahead, true peak off), not a style, because it is a set of knob positions.
8. **Target is a parameter** (`target`, LUFS), so it is saved with the project, undoable,
   automatable and read by the AI bridge like any other; profiles are pairs of (target,
   ceiling) recognised back from the parameters, so there is no separate profile state to
   drift out of step.
9. **M/S linking is not offered.** Limiting mid and side separately cannot hold a ceiling on
   left and right (L = M + S); every design that tries leaves the last stage doing most of
   the work. A width-safety readout is the better answer and is listed below.

## What the tests hold

DSP (`cargo test -p cantus-dsp terminus`; the heavy ones in `--release`):

| Test | Holds |
|---|---|
| `a_limiter_with_nothing_to_do_is_an_exact_delay` | every style, true peak on and off: output = input delayed by `latency()`, to the bit |
| `a_clipper_that_clips_nothing_is_an_exact_delay_too` | clipper on, nothing over its threshold: exact delay |
| `the_result_does_not_depend_on_the_block_size` | every style with everything on (clipper, knee, DC, dither, Match): blocks 1, 7, 64, 1000 equal 4096, tolerance 0 |
| `it_is_deterministic_and_finite_on_nasty_input` | +30 dB into ×40 noise and 1e-30 sections: identical twice, all finite |
| `no_sample_passes_the_ceiling` | sample-peak mode, every style, look-ahead 0 / 0.3 / 2 / 5 ms, +20 dB: no sample over −3 dBFS |
| `true_peak_never_passes_the_ceiling_by_a_32x_reference` | four inter-sample-peak signals (quarter-rate sine at 45°, swelling tones at 11–19 kHz, a band-limited square, band-limited noise bursts), five style/look-ahead/gain cases, both channels: the output's true peak by an independent 32× Kaiser-sinc reconstruction in f64 is **at most −0.100 dB re the ceiling** (round 19, margin 0.15 dB; worst: the quarter-rate sine at 45°, Bus, 1 ms, +9 dB; it was −0.019 dB at the first margin of 0.05) |
| `tests_tp::band_processed_drums_driven_into_the_clipper_stay_under_the_ceiling` | (round 19) drums and hats split into three bands by Culmen in every crossover mode, two seeds, driven +18 dB; clipper off, 20, 40, 70, 100 %; Punch, Loud, Clean, Safe: true peak by the 32× reference **at most −0.016 dB re the ceiling** (was +0.244 dB) |
| `the_latency_reported_is_the_latency_measured` | an impulse comes out exactly `latency()` samples later, four configurations |
| `a_steady_overload_settles_at_the_ceiling` | a tone 10 dB over settles within 0.3 dB under the ceiling |
| `linking_is_per_stage` | unlinked, a left-only peak leaves the right untouched (1e-5); linked both move; transients linked + release unlinked: the right is back 100 ms after the peak while the left still recovers |
| `styles_differ_in_the_order_the_table_gives` | 100 Hz tone, +8 dB: residual after the fundamental Safe −65.3, Clean −48.4, Bus −47.4, Punch −32.0, Loud −26.8 dB — ordered, and Loud vs Safe > 10 dB apart |
| `the_knee_starts_under_the_ceiling_and_never_passes_it` | static curve |
| `unity_gain_hears_the_limiting_at_the_level_it_came_in` | +12 dB in, 1:1 out: peak 0.224 (= 0.891 / 3.98) ±0.01 |
| `match_levels_the_output_to_the_input_loudness` | +12 dB of limiting on drums, Match: integrated in and out within 0.5 LU |
| `delta_is_what_the_limiter_took_away` | out + Δ = gained, delayed input (1e-5) |
| `the_dc_filter_removes_an_offset` | a 0.3 offset: mean under 1e-3 after a second |
| `dither_quantises_with_the_noise_it_promises` | on the 16-bit grid; flat TPDF error 0.5 LSB RMS (±0.05); noise under 3 kHz: Basic ≥ 5 dB and Optimized ≥ 9 dB below flat; silence stays exactly silent |
| `the_clipper_takes_the_peaks_it_is_given` | clipper at 100 %: under the ceiling, and the telemetry reports it clipping |
| `telemetry_counts_columns_from_the_start_of_the_stream` | ten columns in 100 ms, latency in the header |

Oversampling (`terminus::os`): phase 0 is a pure delay (bit-exact); pass band flat within
0.02 dB to 0.44·fs, images more than 60 dB down; a sine's true peak reads **−0.002 …
+0.009 dB** of its amplitude from 200 Hz to 0.44·fs at any phase (a plain 4× meter reads up
to 0.69 dB low near Nyquist); the quarter-rate 45° sine reads **−0.003 dBTP** for 0 dB
(3.01 dB over its samples); the decimator round-trips the interpolator within 0.4e-3.

Loudness (`terminus::loudness`), against published reference values:

| Reference | Expected | Measured |
|---|---|---|
| BS.1770-4 Tables 1–2, K-weighting at 48 kHz | the published coefficients | equal within 1e-7 |
| EBU Tech 3341 case 1: stereo 1 kHz sine at −23 dBFS | M, S, I = −23.0 ±0.1 | −22.993 (all three) |
| case 2: −33 dBFS | −33.0 ±0.1 | −32.993 |
| one channel alone (stereo counts both) | −26.0 | −26.004 |
| case 3: −36 / −23 / −36 dBFS (relative gate) | −23.0 ±0.1 | −23.014 |
| case 4: −72 / −36 / −23 / −36 / −72 (both gates) | −23.0 ±0.1 | −23.014 |
| case 5: −26 / −20 / −26 dBFS | −23.0 ±0.1 | −22.979 |
| EBU Tech 3342 cases 1–4 (LRA) | 10, 5, 20, 15 LU ±1 | 10.000, 5.000, 20.000, 15.000 |
| true peak, 0 dBFS quarter-rate sine at 45° (3341 tolerance +0.2 / −0.4) | 0.0 dBTP | −0.003 |

Engine (`crates/cantus-engine/tests/fx_limiter.rs`): the null test through the whole
engine (the render with the limiter is the render without it, delayed by the reported
latency, to the bit — or not delayed, once delay compensation lands); block-size
independence of a full render with everything on (tolerance 0); a mix pushed 18 dB into a
−1 dBTP ceiling leaves the engine under −1 dBTP by the 32× reference, and under −1 dBFS in
sample-peak mode; the parameters read as the model says.

Editor (`crates/cantus-ui/tests/fx_limiter.rs`, headless frames): it paints every part at
rest; a −23 dBFS stereo sine delivered as telemetry reads −23.0 LUFS (±0.1) on screen;
columns arrive exactly once whatever the frame rate (block lengths 0 to 5 000, each
telemetry block read twice); a second take of bars 2–3 replaces them (the history still
holds four bars, bar 2 at its new reduction); the song as heard measures within 0.2 LU of
the meter that heard it; dragging the ceiling line moves the ceiling and one undo restores
it; dragging the gain strip raises the gain in one undo step; clicking a peak label in the
Song view puts the playhead on that peak (within one bucket); a profile is one undo step
and prints its name and rule; the verdict's words; every factory preset names real
parameters in range; the demo paints labels and loudness in both themes.

Cost: about 1.1 µs per stereo frame with true peak on and nothing to limit, 1.2 µs
limiting, 1.4 µs with everything on (clipper, dither, Match) — measured in release on a
machine running seven other builds, so an upper bound; three 4× true-peak detectors per
channel (input, output safety, output meter) are most of it. Allocation-free after `new`.

## Feature matrix against Pro-L 2

| Pro-L 2 | Terminus | |
|---|---|---|
| Two-stage limiter, Lookahead 0–5 ms, Attack / Release 0–10 s | same ranges (attack from 0.1 ms, release from 1 ms, log knobs) | have |
| Channel link per stage (transients, release) | same, tested per stage | have |
| Eight styles, opaque | five styles, each printed as the numbers it sets, ordered by distortion in a test | **better** |
| True Peak Limiting (~5 ms latency) | 4× detection refined between points + an output safety stage; **verified ≤ −0.1 dB re ceiling on the corpus, ≤ −0.016 dB on clipped band-processed drums, by a 32× reference**; 90 samples (1.9 ms) | **better** (verified, lower latency) |
| Oversampling 2–32× of the whole limiter | detection-only 4×; clipper at 4× | different — missing as an option; see decision 1 |
| Gain 0–30 dB, Output −30…0 dB(TP); Alt-drag gain against output | same; the gain strip and the ceiling line are dragged on the display, Alt on the strip moves the ceiling inversely | have / **better** (on the display) |
| Unity Gain (1:1) | 1:1, plus **Match** by measured K-weighted loudness | **better** |
| Audition limiting (delta) | Δ, at either monitor level | have |
| DC offset filter | 5 Hz high-pass | have |
| Dither 16–24 bit, three noise shapes | 16–24 bit TPDF, flat / Basic / Optimized, **auto-blank on digital silence** | have (two shapes, not three) / better (blanking) |
| External side-chain trigger | trigger from any strip, chosen in the editor | have |
| Loudness: M, S, I, LRA, TP, max M / max S | all at once, always visible, conformance-tested (Tech 3341 / 3342 tables above) | **better** (tested, nothing hidden by mode) |
| Targets (−9, −14, −23, −24, custom) | profiles Spotify, Apple Music, YouTube, EBU R128, ATSC A/85, custom — target **and ceiling** together, rule in a sentence, written pass/fail with a shape | **better** |
| Scrolling display, peak labels, display modes, freeze: no | Live (6 / 20 / 60 s, freeze) and **Song** (bars, scroll, zoom, follow); peak labels that **seek**; hover values | **better** |
| — | loudness of **the song as last heard** and of **the time selection** | **better** (host-only) |
| Latency not shown | always on screen, with a clipping warning under 0.1 ms of attack | **better** |
| No knee | soft knee 0–12 dB under the ceiling | **better** |
| No clipper | 4× soft clipper, exact wire when idle, clip activity drawn | **better** |
| A/B, undo in the plug-in | host A/B and undo, one step per gesture | have (shared frame) |
| Lock Output on preset load | — | missing: presets that should not touch delivery simply do not name `ceiling`/`target` (the factory ones), but user presets do |
| Meter scales (K-12/14/20, +9/+18) | one fixed scale, expanded in the top 12 dB | missing |
| Surround / Atmos linking | — | missing (Cantus buses are stereo) |
| — | M/S linking | missing, deliberately (decision 9) |
| — | style shoot-out table (every style over the last bars, loudness-matched) | missing — Match + host A/B give the matched listening; the table needs audio the editor does not keep |
| — | width-safety readout, spectrum of where reduction happens | missing (P1 of the research) |

## At merge

- Delay compensation: `latency()` is the true delay (look-ahead + 17 samples for the
  detector + 73 for the true-peak stage + 32 for the clipper). The engine null test accepts
  both "delayed by latency" (today) and "not delayed" (after PDC), so it needs no edit.
- No files outside the owned set were edited; no schema bump (the parameters are new keys
  of an existing kind, and missing keys read as defaults).
