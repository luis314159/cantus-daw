# Fervor — saturation and colour

*Effect kind `Saturator`. Benchmarks: FabFilter Saturn 2, Soundtoys Decapitator, FL Studio's
Blood Overdrive and Fruity WaveShaper; Ableton's Saturator and iZotope Trash in mind
(ADR 0026).*

Fervor bends a signal through a curve: seven models — **tape, tube, overdrive, fuzz, clip,
fold** and a **curve you draw** — oversampled up to 8× with linear- or minimum-phase filters,
with a bias for even harmonics, tone before and after the curve, bit and rate reduction,
mid/side, dry/wet, and an output **matched in level** so a louder sound never wins the
comparison. The editor shows the curve as heard with the signal over it, and measures the
harmonics a test tone comes out with — even and odd told apart, aliasing in red.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/saturator.rs` |
| DSP | `crates/cantus-dsp/src/fervor.rs`, `fervor_shape.rs` (curves), `fervor_os.rs` (oversampling); tests in `fervor_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/saturator.rs` (telemetry layout documented there) |
| Editor | `crates/cantus-ui/src/views/fx/saturator.rs`, `saturator_curve.rs`, `saturator_harm.rs` |
| Strings | `crates/cantus-ui/locales/fx/saturator.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_saturator.rs`, `crates/cantus-ui/tests/fx_saturator.rs`, the family's CLAP/VST3 tests |
| Exported | `Cantus Fervor` (CLAP `com.github.lalmazan.cantus.fervor`, VST3 `C1072D15F4D9B4EBD8CBE3228AD22E23`) |

## What the references do well, and what people complain about

- **Saturn** has the widest palette (28 styles, multiband, modulation) and a good HQ mode —
  but draws no transfer curve and no spectrum, its styles are names without a description,
  and its output is not level-matched, so every comparison is won by the louder side.
- **Decapitator** is loved for its five hardware models, *Thump*, *Punish* and *Auto* gain;
  it shows nothing, and at high drive the older versions alias audibly.
- **Blood Overdrive** is FL's quick distortion: pre-band, colour, pre-amp, ×100, post-filter,
  post-gain. Six knobs, no oversampling, harsh at high gain.
- **Fruity WaveShaper** lets you draw the curve (points with tension, symmetric or not,
  presets) — and stops there: no oversampling, no level match, nothing about what the
  curve does to a sound.
- What users ask of all of them: **no aliasing** at high drive, **honest loudness**, models
  that say what they are, **M/S** for mastering, no **DC** from asymmetric curves, no
  **zipper** when automating, and a cheap mode for tracking (no linear-phase latency).

Fervor answers each: oversampling with both filter kinds, a static level match on by default,
every model described in words in the editor and here, M/S, a DC blocker, glides on a grid,
and a measured spectrum that makes aliasing visible.

## Parameters (keys are forever)

Every default is a null: Fervor inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `model` | 0 … 6 | Tape | Tape, Tube, Overdrive, Fuzz, Clip, Fold, Curve |
| `drive` | 0 … 36 dB | 0 | the gain into the curve (see *drive* below); at 0 every analytic model is a wire |
| `boost` | off / on | off | ×100: 40 dB more drive (Blood Overdrive's switch) |
| `char` | 0 … 100 % | 50 % | the model's character, named for what it does (below) |
| `bias` | −100 … 100 % | 0 | an offset into the curve: even harmonics |
| `lo_cut` | 20 … 2 000 Hz (log) | off (20) | a 12 dB/oct high-pass **before** the curve: tightens the low end so it does not drive the curve |
| `hi_cut` | 1 … 20 kHz (log) | off (20 k) | a 12 dB/oct low-pass **after** the curve: tames the fizz |
| `tone` | −12 … 12 dB | 0 | a tilt around 1 kHz after the curve (Aequalis's matched tilt shelf) |
| `os` | 0 … 3 | 1× | oversampling 1×, 2×, 4×, 8× |
| `phase` | 0 / 1 | linear | the oversampling filters: linear phase (latency 47–57 samples) or minimum phase (5, 7 or 8 samples) |
| `bits` | 1 … 24 | off (24) | bit depth after the curve, continuous (12.5 is between 12 and 13 bits) |
| `rate` | 100 … 50 000 Hz (log) | off (50 k) | sample-and-hold rate after the curve; at or above the sample rate it is off |
| `chan` | 0 … 2 | stereo | colour both channels, only the mid, or only the side (the other passes, delayed to match) |
| `mix` | 0 … 100 % | 100 % | dry/wet; the dry path is delayed by the latency, so a blend never combs |
| `auto` | off / on | on | level match (below) |
| `out` | −24 … 24 dB | 0 | output gain, after everything |
| `c.sym` | off / on | on | the drawn curve is odd-symmetric: only x ≥ 0 is drawn, from the origin, and mirrored |
| `c.lo`, `c.hi` | −1 … 1 | −1, 1 | the drawn curve's value at x = −1 and x = 1 |
| `c.t` | −1 … 1 | 0 | the bend of the drawn curve's last segment |
| `p{n}.on`, `.x`, `.y`, `.t` | n = 0 … 15 | unused | the drawn curve's points: used, where (x, y in −1 … 1), and the bend of the segment that ends there |

A point's number never changes once it exists (Aequalis's band rule): removing one frees its
number and renumbers nothing, so automation of `p3.y` always means the same point. The kind
is sparse: a project stores only what moved.

### The character, per model

| Model | `char` is | What the model is |
|---|---|---|
| Tape | **Emphasis**: a high shelf at 3 kHz, 0 … +12 dB, before the curve, and its exact inverse after | tanh; with emphasis the highs reach the curve first, so driving it loses top end the way tape does |
| Tube | **Asymmetry**: the negative side bends at 1/k and stops at −1/k, k = 1 + 2·char | tanh on the positive side; even harmonics from the asymmetry alone |
| Overdrive | **Focus**: a low shelf at 700 Hz, 0 … −18 dB, before the curve, and its inverse after | x / (1 + x⁴)^¼, a harder knee; the low end is spared, as an overdrive pedal's mid hump spares it |
| Fuzz | **Rectify**: the negative half scaled from 1 down to −0.25 | x / (1 + x⁸)^⅛, near-square; past 80 % the negative half flips, toward octave fuzz |
| Clip | **Knee**: the width of a quadratic knee, 0 = hard | exactly linear up to the knee, flat above the ceiling |
| Fold | **Shape**: sine fold (0) to triangle fold (1) | the signal folds back from the ceiling, again and again with drive |
| Curve | — (the knob rests) | the drawn curve |

## The DSP, and the decisions

### Drive is a gain into a curve whose knee comes down from infinity

Each analytic model is a normalised sigmoid `S` (slope 1 at the origin, bounded by 1 except
the fold) and the output is

```text
y = G · (S(a·x + β) − S(β)) / (a · S'(β)),     a = G − 1,  G the drive as a gain
```

The slope at the origin is `G` — drive is an input gain, as on hardware — and the ceiling is
`G / a`: far above full scale at small drive, coming down to 0 dBFS as the drive rises. At
0 dB (`a = 0`) the formula's limit is `y = x`, and the code takes the limit exactly: **the
default is a wire to the bit, and the first tenth of a dB has no step**. This is why a drive
of 0 dB is clean in every model while tanh at "0 dB" is not — the usual design either
colours at its default or switches the effect in with a jump. `S(β)` is taken off so silence
stays silent at any bias, and dividing by `S'(β)` keeps the small-signal gain where it was,
so the bias changes the colour, not the level. The bias moves the curve by up to 0.8 of its
knee.

The drawn curve is used as drawn: `y = C(G·x + β) − C(β)`, its domain ±1 (the ends hold
beyond it), read from a table of 2048 segments made once per change of the curve.

### Emphasis: where a model bites

Tape and overdrive put a shelf before the curve and its **exact inverse** after it
(numerator and denominator swapped, so the pair is the identity wherever the curve is
linear). Tape lifts the highs, so they saturate first and come back down — the
level-dependent high-end loss of tape, made by the mechanism tape uses (pre-emphasis), not by
a low-pass. Overdrive lowers the lows, so the bass stays clean while the mids drive.

### Oversampling: two kinds of filter

- **Linear phase**: a cascade of Kaiser-windowed half-bands (95, 31 and 15 taps; β 9, 8, 8).
  Every other tap is zero and the centre is ½, so one phase of the up-sampler is a pure
  delay and each stage costs about a quarter of a plain FIR. A few samples of padding at the
  top rate make the total whole at the base rate: **47, 55 and 57 samples** at 2×, 4×, 8×.
  The pass band is flat to 20 kHz within 0.1 dB (measured).
- **Minimum phase**: polyphase IIR all-pass half-bands (12, 6 and 4 coefficients), designed
  by the elliptic method of Valenzuela and Constantinides as in de Soras's HIIR. Up and down
  together are an all-pass — the magnitude is flat everywhere — with more than 120 dB of
  rejection from 25.9 kHz at 48 kHz in the first stage; the latency reported is the group
  delay at low frequencies, rounded: 5, 7 and 8 samples at 2×, 4×, 8×.

Measured (`oversampling_keeps_the_aliasing_out`): a 3.2 kHz sine into the clip model at 18 dB
of drive — what folds back drops by more than 10 dB at 2× and more than 30 dB at 8×, to under
−60 dB of the harmonics' energy.

### The level match: static, so it never pumps

With `auto` on, the wet path is multiplied by the gain that brings a reference sine — **−18
dBFS RMS**, a mix's level — back to its own RMS after the curve (its DC removed, as the DC
blocker removes it). It is computed from the curve on the grid (64 points), not measured
from the audio, so it cannot pump; the editor prints it. Material much louder or quieter than
the reference is matched less exactly — the price of not pumping, and the usual choice
(Decapitator's *Auto* works the same way).

### On a grid, gliding; crossfades for what cannot glide

Every setting is read on a grid of 16 samples counted from the start of the stream (the
family's rule, ADR 0017/0020), so the output does not depend on the block size and a
plug-in host's automation lands on the same sample whatever blocks it cuts. Drive,
character, bias, tone, the cuts, mix and output glide (a 20 ms one-pole that lands on its
target exactly); the gains are interpolated sample by sample inside each grid step. A change
of model or of the drawn curve crossfades over 10 ms between the old curve and the new.

### The wire path

When nothing would colour the signal — no drive on an analytic model, no filter, no tilt, no
crush, 1× — the unit copies its input to its output (times the output gain, exactly 1 at
0 dB). Entering the wet path clears its state, so nothing stale comes out.

### DC, denormals

The wet path ends in a 5 Hz DC blocker (asymmetric curves and the bias make DC). The
blocker and every biquad (cuts, tilt, emphasis) keep their state in f64: in f32 a 30 Hz
high-pass and a 5 Hz blocker lift their own rounding noise to about −100 dB at low
frequencies — inaudible, but the harmonics display measures it, and a mastering tool should
not make it. Every filter and all-pass state below 10⁻²⁰ is flushed, so silence stays exactly
silent.

### Order of the path

```text
in → low cut → emphasis → ↑os → curve → ↓os → de-emphasis → DC block → rate → bits → tone → high cut → × match → mix (with the delayed dry) → × out
```

Bit and rate reduction come after the down-sampler on purpose: their aliasing is the sound.

## The editor

- **Model list** at the left: seven keys, each with a small picture of its curve; under them,
  what the chosen model is in two lines.
- **Curve**: the transfer curve as heard (drive and bias in it), filled to the axis; the line
  of no change dashed; the signal as a cloud of (in, out) points — the output lined up with
  the input it came from by the latency — and a ball where the input's peak sits on the curve
  now. Dragging up or down on an analytic model's curve drives it. With *Curve*: drag a
  point; double-click to add one (or on a point to remove it; right-click removes too); drag
  a segment's diamond to bend it (up bows it up, whichever way the segment runs); drag an end
  up or down; *Symmetric*; eight ready-made **Shapes** (straight line, soft knee, hard clip,
  crossover, fold back, stairs, rectify, asymmetric). Every drag is one undo step.
- **Harmonics**: a 720.7 Hz test tone (123 cycles in 8192 samples, an odd count, so what
  folds back past Nyquist lands *between* the harmonics) at −18, −12, −6 or 0 dBFS through the
  real DSP — every filter, the oversampling, the crush — Blackman-Harris windowed. Harmonics
  are bars, even and odd in two hues; everything between them is red. Readouts: THD, the
  even/odd balance, the loudest inharmonic component. The oversampling and its filter kind
  are chosen right under it.
- **Cells**: Drive (the device's one XL knob), the character under its model's name, bias,
  ×100; Tone (low cut, tilt, high cut); Crush (bits, rate); Output (mix, output, match with
  the gain it applies, L+R / Mid / Side).
- **Footer**: latency and oversampling chips, the output meter.
- **Factory presets** (13): tape on the bus, tape-crushed drums, tube vocal, tube glue on the
  mid, Blood overdrive, bass growl, fuzz face, master clipper, folded synth, lo-fi sampler, air
  on the sides, waveshaper soft knee, waveshaper rectifier.

## Telemetry

See `crates/cantus-engine/src/fx/saturator.rs`: input and output peak and RMS, the match's
gain, latency, sample rate, the running factor, whether the wet path runs, the gliding drive,
and the input peak as a linear value for the ball.

## Not yet

- **Bands** (Saturn's multiband): two or three bands with their own model and drive. The
  model is per unit today; bands would add `b{n}.*` keys, which the sparse model allows
  without touching what is stored.
- **Dynamics on the drive** (Saturn's *Dynamics*, an envelope follower pushing the drive) and
  **feedback**.
- A **tape hysteresis** model (Jiles–Atherton); tape here is a memoryless curve with emphasis.
