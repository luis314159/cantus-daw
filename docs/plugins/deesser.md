# Sibilus — the de-esser

`EffectKind::DeEsser`. Parameter model `crates/cantus-core/src/fx/deesser.rs`; DSP
`crates/cantus-dsp/src/sibilus.rs` (+ `sibilus_detect.rs`, `sibilus_split.rs`,
`sibilus_tests.rs`); engine glue `crates/cantus-engine/src/fx/deesser.rs`; editor
`crates/cantus-ui/src/views/fx/deesser.rs`; strings `crates/cantus-ui/locales/fx/deesser.*.txt`;
tests `crates/cantus-engine/tests/fx_deesser.rs`, `crates/cantus-ui/tests/fx_deesser.rs`.
Benchmark: FabFilter Pro-DS 1.32 (`research/fabfilter/pro-ds.md`).

## What it does

A compressor whose detector hears only the band sibilance lives in — and, for a single
voice, only what in that band *is* sibilance. It turns the whole signal down (wide band) or
only part of the spectrum (split), with lookahead, stereo link, mid/side, a key from any
strip, and two ways to hear what it does.

```text
key ─► band (HP lo, LP hi, 24 dB/oct) ─► level (2 ms RMS, sine-peak scale) ──┐
  └──► classifier: share × flatness ─► confidence (Single vocal only) ───────┤
                                                                              ▼
      Comprimo's curve (∞:1, 6 dB knee, capped at Range) × confidence ─► link ─► attack/release ─► g
input ─► delay (lookahead [+ FIR centre]) ─► y = x_d + (g − 1) · band(x_d)
```

## Decisions, and why

**1. The single-vocal detector is two features you can check, not a black box.** Pro-DS's
Single Vocal mode is its best feature and its most opaque one. Ours multiplies two ramps:

- **Share** — the band's energy over the whole key's, in dB (5 ms averages). An "s" puts
  almost all its energy in the band (−0.2 dB on the test hiss); a vowel, however bright,
  puts almost none there (−16.3 dB on a 220 Hz sawtooth with every harmonic to 20 kHz).
  Ramp −15 → −7 dB.
- **Flatness** — the key's spectrum across the band through six sixth-order band-passes;
  geometric over arithmetic mean of their energies. Noise spreads (0.81 on the test hiss),
  a tone does not (0.20 on an 8 kHz sine *at the same band level*). Ramp 0.30 → 0.55.

**Sensitivity** moves both ramps together (±8 dB, ±0.1). The sub-band design was chosen
from the filters' responses before any code (a script over the analog prototypes): with
five fourth-order band-passes a sine midway between two sub-bands reads 0.37 — too close to
sibilance only half inside the band (≈0.55); six sixth-order ones bring the worst sine to
0.20 and leave sibilance inside the band at 0.87–0.98. The confidence is recomputed every
16 samples from the start of the stream, so it is block-size independent.

**2. The curve is Comprimo's.** `comprimo::gain_reduction_db` with ratio "∞" (100), a 6 dB
knee and Range as the cap: the band is held at the threshold, never more than Range down.
In Single vocal the result is scaled by the confidence, so with the threshold low every
sibilant is cut by about Range whatever its level — Pro-DS's "−∞ threshold" behaviour —
and nothing else is cut at all. One dynamics core for the family, as `pro-mb.md` §10 asks.

**3. Split band is a linear-phase FIR used as a difference, so it is still a wire.** The
output is `x_d + (g − 1)·h∗x`. With `h` linear-phase and `x_d` delayed by its centre, the two
terms are in phase at every frequency: the response is exactly `1 + (g − 1)·A(f)`, a cut of
`g` in the band, none outside, no bump between; and at `g = 1` the sum is not computed, so
the output is the delayed input to the bit. An IIR crossover cannot do both (it sums to an
all-pass, or used the same way limits the depth of cut near its corner — measured on paper
before choosing: a 2nd-order "1 − HP" shelf reaches only −2.8 dB an octave above its
corner). The FIR is a Kaiser-windowed sinc, β = 6, 255 taps at 44.1/48 kHz (511 / 1023 at
higher rates): about −60 dB of stop band, a 0.7 kHz transition, **127 samples (2.6 ms) of
latency**. Two split shapes: *Split high* (everything above the low edge — Pro-DS's) and
*Split band* (only low to high edge — what fixes the "lispy split band" complaint, since the
air above the band is left alone).

**4. Lookahead is true latency, in whole milliseconds.** 0–15 ms, stepped (a latency is a
setting the host compensates per value, not something to sweep). The gain computed from the
key at time *n* is applied to the audio of time *n* — the audio is delayed by lookahead +
FIR centre, and the gain by the FIR centre alone, so the detector is exactly `lookahead`
ahead. `latency()` = lookahead + (split ? 127 : 0); `sibilus::latency_for(settings, sr)`
gives the same number without a unit, for the host.

**5. The history is measured by the DSP, not sampled by the frame rate.** Columns of 512
samples (≈11 ms) counted from the start of the stream: input peak, output peak, detector
level, cut, confidence, and the trigger's zero-crossing centre frequency — all measured
where the audio leaves (detector and confidence delayed by the latency), so the lanes line
up. The telemetry carries the last nine and a count; the editor appends what it has not
seen and draws a gap for what it missed. Layout in `crates/cantus-engine/src/fx/deesser.rs`.

**6. Keys stored as choices by index.** `mode` (single vocal, allround), `proc` (wide, split
high, split band), `stereo` (L/R, M/S, mid only, side only), `listen` (output, trigger,
removed). Continuous: `thr` −60…0 dB, `range` 0…30 dB, `lo`/`hi` 1–20 kHz (either order;
the DSP orders them and keeps a sixth of an octave), `att` 0.1–50 ms, `rel` 5–500 ms,
`look` 0–15 ms, `link` 0–100 %, `sens` 0–100 %.

**7. Learn (auto threshold) is taught, not guessed.** Click *Learn threshold*, play a
passage, click again: the classifier's events (Single vocal) or the loudest quarter of the
band (Allround) give a peak per event; the threshold becomes `median(peaks) − Range/2`, so
the median sibilant is cut by half the Range and the loudest by all of it. One undo step,
and the line is marked "learned" while it stands. The histogram of event peaks along the
detector lane's dB axis shows what it learned from, at all times.

## The editor

Three lanes on one time axis, then the band on a spectrum and the knobs:

- **Signal** — the input mirrored, on the square root of its amplitude (a dB scale flattened
  every vowel into a slab in the first screenshots; a linear one loses a quiet "s"); de-essed
  columns turn green: solid for what is left, lighter *and hatched* for what was taken (a
  second cue besides colour), with a legend on the lane. Every lane is a smooth strip
  through the columns, not a staircase, and its scale sits in a gutter at the right, so no
  number is printed over the data.
- **Cut** — the reduction in dB hanging from the top on the Range's scale, a dashed line at
  the Range, "now / peak" in numbers.
- **Detector** — the band level; **the threshold is a dashed line you drag**; the part over
  it filled (in Single vocal, filled as strongly as the classifier is sure); the classifier's
  confidence as a bar per column along the bottom; the event-peak histogram at the right.
  Hover any column: time ago, input, band level, cut, sibilance, centre frequency.
- **Spectrum** — the input (tilted 4.5 dB/oct) glowing inside the band, the output as a
  line, **the band's edges dragged on it** (the middle drags both), the processing's cut
  drawn over it — dashed at the full Range, solid live — and the last event's centre marked.
- **Knobs** — Threshold and Range large; the edges, attack, release, lookahead, link and
  sensitivity small; every one keyboard-reachable with the house gestures.
- **At rest, always visible**: mode, processing, stereo, listen, key, latency ("Latency
  7.6 ms" / "No latency"), events, events in the last 10 s, average and deepest cut per
  event, a warning past 6 dB average, Live or Paused (the wheel scrolls 30 s back; a
  double-click returns to live).

## What the screenshots found

`cargo run -p cantus-ui --example shoot -- target/shots 1.0 deesser` writes
`deesser-light.png` and `deesser-dark.png`: the real DSP run over a synthetic phrase
(`demo_voice`: formant vowels, "s", "sh" and "t"), split band, threshold −38 dB. Three
rounds:

1. Lane labels and the Live note were drawn under the data and overprinted; axis numbers
   sat on the waveform; per-column rectangles looked like a bar chart next to Pro-DS's
   envelope; the live cut on the spectrum was a 6 px dip on a 30 dB scale; the status row
   said "8 events · 9 in the last 10 s" (an open event counted by one and not the other);
   the 20 kHz label was clipped. — Plates under every label, a right gutter for scales,
   smooth strips, the cut on its own scale (1.5 × Range) with 0 and −Range marked, closed
   events only, the last label right-aligned. The same pass found the **window crept taller
   every frame** (the body asked for the available height without the item spacing), which
   the threshold-drag test caught; a test now holds the geometry still.
2. The dB scale flattened every vowel into a slab: the signal lane went to the square root of
   the amplitude. "Learn threshold" read as a mode, not an action: it is a button now.
3. The event centre label was green on the gold band glow in the dark theme: on a plate.

## What the tests hold (numbers)

DSP (`sibilus_tests.rs`, 16 tests):

| Claim | Measured |
|---|---|
| Wire below the threshold and at Range 0, every mode (2 × 3 × 4), lookahead 0 and 5 ms | output = input delayed by `latency()`, **bit-exact**; latency 0 in wide band without lookahead |
| Latency is the delay there is | impulse out at exactly `look·48 + (split ? 127 : 0)` samples, unchanged |
| Block-size independence, all 24 modes + lookahead/link/sens variants, own input and key | **identical to the bit** at blocks 1, 7, 64, 1000, 4096 (output, columns, event stats) |
| A sibilant burst 28 dB over is cut by the Range | wide 6.00 / 12.00 dB; split high 5.98 / 11.91; split band 5.95 / 11.77 (tolerance ±1 dB) |
| Single vocal vs a bright tone at the **same band level** | burst 6.00 dB, 220 Hz sawtooth **0.00 dB**, 8 kHz sine **0.00 dB**; Allround cuts all three 6.00 dB |
| What the classifier reads | hiss: share −0.2 dB, flatness 0.81, confidence 1.00; sawtooth −16.3 dB / 0.95 / 0.00; whistle 0.0 dB / 0.20 / 0.00 |
| A phrase: vowel, "s", vowel (threshold −50 dB, lookahead 5 ms) | vowels 0.00 and 0.04 dB, the "s" 6.00 dB |
| Split leaves the rest alone | 200 Hz tone under the burst: 1.2·10⁻⁴ (split high), 1.5·10⁻⁴ (split band) of it in the removed signal; FIR amplitude 1.00 at 7 kHz, < 0.01 at 1 kHz and 18 kHz |
| Output + Removed = input | within 1·10⁻⁶ in every processing mode |
| Trigger is the band | 8 kHz passes within 0.5 dB, 500 Hz more than 60 dB down |
| Lookahead catches the onset (hard onset, attack 0.5 ms) | first millisecond cut 6.00 dB with 5 ms lookahead, 2.39 dB without |
| External key; stereo modes; link | keyed cut 6 ± 1 dB, silent key = wire; mid-only leaves a side-only hiss bit-exact, side-only cuts it 6 dB; link 1 cuts the quiet side 6 dB, link 0 leaves it |
| Silence and extremes | exact zeros after a long silence (no denormal tail) in every mode; 40 random extreme settings on 8× full-scale noise, no NaN |
| Events | three bursts → 3 events, mean and max 6.0 dB, centre 7.43 kHz for a 5–10 kHz hiss |

Engine (`tests/fx_deesser.rs`): bit-identical render to no de-esser at settings that do
nothing (both modes, every stereo mode); with 5 ms lookahead and the split, the render is
the same render exactly 240 / 367 samples later; block sizes 1–4096 identical while it
works hard; a key from another strip changes the render and stays block-size independent;
the unit reports its latency. The shared contract test and the undo fuzz cover the rest.

UI (`crates/cantus-ui/tests/fx_deesser.rs`, 7 tests): the editor paints its state in real
units; the window settles and stops growing; the threshold drag lands within 0.6 dB of the
pointer and is one undo step; an edge drag and a
middle drag on the spectrum each move what they should and are one undo step; Learn sets
−19.0 dB from events at −20/−16/−12 with a 6 dB Range, marks it, undoes in one step, and
refuses two events; every preset's label exists in both languages and sets what it names;
the preview says what it is set to; the editor speaks Spanish.

## Feature matrix against Pro-DS 1.32

| Feature | Pro-DS | Sibilus | |
|---|---|---|---|
| Single Vocal / Allround detection | yes (black box) | yes; the classifier's two features documented, tested and drawn per column | **better** |
| Sensitivity of the classifier | no | yes | **better** |
| Threshold, Range | −∞…0, 0–24 dB | −60…0, 0–30 dB | have |
| Detection band | 2–20 kHz slider | 1–20 kHz, dragged on a spectrum | **better** |
| Wide / Split band | wide, split above HP (linear phase) | wide, split high, **split band** (linear phase; leaves the air alone) | **better** |
| Attack / Release | hidden, programme-dependent | exposed, real ms | **better** |
| Lookahead | 0–15 ms | 0–15 ms, true latency reported for compensation | have |
| Stereo link, Mid only, Side only | yes | yes, plus independent M/S | **better** |
| External side chain | host routing | any strip, chosen in the editor | **better** |
| Audition trigger / removed | yes | yes (Trigger, Removed) | have |
| Scrolling display with de-essed parts green | read-only | lanes for signal, cut, detector; hatched removal; hover per column; scroll back 30 s | **better** |
| Threshold on the display | no | dragged on the detector lane | **better** |
| SC meter | ring around the knob | detector lane with the threshold on the same scale | have |
| Statistics / over-de-essing warning | no | events, per 10 s, mean and deepest cut, warning past 6 dB | **better** |
| Auto threshold | no | Learn from a passage, visible, one undo step | **better** |
| Presets | yes | six factory (male, female, voice-over, backing side-only, cymbals, bus HF limiter) + user | have |
| Undo, A/B | own | host's (one step per gesture) | **better** |
| Oversampling 2×/4× | yes | no — the gain moves smoothly in dB and there is no nonlinearity to alias; not needed for a clean result | missing (deliberate) |
| Input / output gain, pan | yes | no — the strip has them | missing (host does it) |
| Loudness-matched bypass | no | no | missing (planned: needs a host bypass hook) |
| Second target band ("sh"), cut shapes (bell), spectral mode | no | no | missing (P1/P2 of the report) |
| Print de-essing to clip gain; events on the timeline | no | no | missing (needs the arrangement's clip-gain automation) |
| Oversampled / surround | stereo only | stereo only | same |
