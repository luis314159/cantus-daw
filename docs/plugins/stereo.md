# Latitudo — stereo control and mid/side

*Effect kind `Stereo` (ADR 0026). Benchmarks: iZotope Ozone Imager, Waves S1 Stereo
Imager, FabFilter's mid/side modes (Pro-Q, Pro-MB), FL Studio's Patcher mid/side preset.*

Latitudo is the stereo tool for a strip and for the master: width, mid and side gain,
balance, rotation, swap and polarity; **mono below a frequency** through a
phase-coherent crossover; **width in three bands**; a **mid/side tone** section (a low and
a high shelf on the mid and on the side — Patcher's mid/side preset, built in); **Haas
widening**, classic or **mono-safe**; and the meters a stereo decision needs: a
vectorscope, correlation (overall, per band, input against output), balance and the
side's level against the mid. It never delays the signal, and **at its defaults it returns
its input to the bit**.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/stereo.rs` |
| DSP | `crates/cantus-dsp/src/latitudo.rs` (+ `latitudo_run.rs`, `latitudo_view.rs`; tests in `latitudo_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/stereo.rs` |
| Editor | `crates/cantus-ui/src/views/fx/stereo.rs` (+ `stereo_scope.rs`, `stereo_map.rs`) |
| Strings | `crates/cantus-ui/locales/fx/stereo.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_stereo.rs`, `crates/cantus-ui/tests/fx_stereo.rs`, the plug-in suite |
| Plug-ins | CLAP `com.github.lalmazan.cantus.latitudo`, VST3 class `EC13BB3A4A5F71E71468741CD2C29ACE` (feature `stereo`) |

## What the references do, and what people miss in them

- **Ozone Imager**: four bands of width with a linear-phase or "analog" split, a
  vectorscope with *Polar Sample* and *Polar Level*, a correlation meter per band, and
  *Stereoize* (a decorrelated side made from the mid). Users love the scope and the
  per-band width; they complain that the band split is fixed to the plug-in's own
  frequencies on screen, that Stereoize can sound phasey, and that there is no mono-bass
  control of its own (people stack a second plug-in for it).
- **Waves S1**: width, rotation, *asymmetry*, and *shuffling* (more width below a
  frequency). Loved for rotation; its interface is old and has no meter worth reading.
- **FabFilter**: mid/side is a *mode* of the EQ and the multiband, excellent at what it
  does, but there is no width control, no scope, no bass mono — and a side high-pass in
  Pro-Q is a plain filter on the side alone, which shifts the side's phase against the
  mid around the cut-off.
- **FL Patcher's mid/side preset**: a splitter into mid and side, an EQ on each, back to
  left and right. Useful and clumsy: four plug-ins in a patch to do one thing.

Latitudo takes the good parts into one device and fixes the complaints:

1. **Mono below keeps the image above it.** The side's low band is removed at a
   Linkwitz–Riley crossover and the **mid goes through that crossover's all-pass**, so mid
   and side stay in phase at every frequency. A hard-left source above the crossover stays
   hard left: the test measures the right speaker getting −62 dB of a left source at five
   times the cut-off with 24 dB/oct (−34 dB at 12 dB/oct, where a plain 12 dB/oct side
   high-pass would leak about −17 dB). At 6 dB/oct the split is complementary
   (`x − LP(x)`), the mid is not touched at all: a gentle side high-pass.
2. **Width by band without smearing**: the side is split by two Linkwitz–Riley crossovers
   (24 dB/oct) and the mid takes the same all-passes. The crossovers are dragged on the
   map; each band's correlation is printed in its own band, so out-of-phase bass shows
   where it is.
3. **One map for everything that depends on frequency**: the effective width over
   frequency (width × the mid/side gains × the bands × the mono crossover — the DSP's own
   response) above, and the mid's and side's tone below, every handle dragged in place.
4. **Haas, honestly.** Classic Haas (one channel late) is wide and combs in mono; the card
   says from which frequency (`1 / (2·delay)`). The **mono-safe** mode adds the delayed
   mid to the side instead: `L + R = 2M` whatever the side holds, so the mono sum is the
   input's, exactly — wide in stereo, untouched in mono. (Ozone's Stereoize is a
   relative; ours is spelled out and tested.)
5. **The scope tells the truth**: the zoom follows the level so a quiet mix still fills it,
   but rings mark 0, −6, −12 and −24 dB so the zoom never lies about loudness; the polar
   outline shows where the energy points; the correlation meter carries the input's
   value as a mark and the lowest value of the last three seconds as a triangle.

## Parameters (keys are forever)

| Key | Range | Default | Meaning |
|---|---|---|---|
| `width` | 0 … 200 % | 100 % | the side against the mid: 0 is mono, 200 % doubles the side |
| `mid`, `side` | −24 … +12 dB | 0 | the mid's and the side's own gain |
| `bal` | −100 … +100 % | 0 | balance: the far side turned down linearly (+100 % silences the left) |
| `rot` | −45° … +45° | 0 | rotation of the image; +45° sends a centred source to the right speaker, power kept |
| `swap` | off / on | off | left and right exchanged |
| `inv_l`, `inv_r` | off / on | off | polarity of each input |
| `listen` | Stereo, Mid, Side | Stereo | hear the result, the mid alone (the mono check) or the side alone, in both speakers |
| `mono` | 20 … 1000 Hz (log) | 20 = off | mono below this frequency |
| `mono_slope` | 6, 12, 24, 48 dB/oct | 24 | the mono crossover's slope |
| `x_lo`, `x_hi` | 40 … 2000, 500 … 16 000 Hz | 250, 4000 | the bands' crossovers (kept at least 1.5× apart; the higher gives way) |
| `w_lo`, `w_mid`, `w_hi` | 0 … 200 % | 100 % | each band's width, on top of `width` |
| `m_lo_f`, `m_lo` | 20 … 2000 Hz, ±12 dB | 150 Hz, 0 | the mid's low shelf |
| `m_hi_f`, `m_hi` | 1 … 20 kHz, ±12 dB | 8 kHz, 0 | the mid's high shelf |
| `s_lo_f`, `s_lo`, `s_hi_f`, `s_hi` | as the mid's | as the mid's | the side's shelves |
| `tone_slope` | 6, 12 dB/oct | 6 | the shelves' slope (Butterworth: no overshoot either way) |
| `haas` | 0 … 30 ms | 0 = off | the Haas delay |
| `haas_mode` | Delay right, Delay left, Mono-safe | Delay right | how the delay widens |
| `haas_amt` | 0 … 100 % | 50 % | mono-safe: how much delayed mid goes into the side |
| `out` | −24 … +12 dB | 0 | output gain |

The contract's three keys (`width`, `mid`, `side`) kept their ranges and meanings. The kind
is sparse: a project stores only what moved.

## The signal path

1. Polarity, then swap, on left and right (`P`, a 2×2 matrix).
2. Haas, classic: the chosen channel read from a line `haas` ms back (fractional, linear).
3. Into mid and side: `M = (L + R)/2`, `S = (L − R)/2`.
4. Haas, mono-safe: `S += amount · M(t − haas)`.
5. Tone: low then high shelf on M, and on S (Multiplex's Butterworth shelves, TPT
   filters; a low shelf of gain `r` is `r ·` a high shelf to `1/r`).
6. Width by band: `S → lo, mid, hi` at `x_lo`, `x_hi` (LR4), each scaled; `M` through both
   all-passes; the low band through the high crossover's all-pass.
7. Mono below: `S →` the crossover's high half, `M →` its all-pass.
8. Out: `Q` (2×2) — mid and side gains with the width, what is heard, decode to left and
   right, rotation, balance and the output gain.

**When no stage of 2–7 works, the whole effect is one matrix `T = Q·E·P` on left and
right**, computed in `f64` per grid point; when `T` is exactly the identity and nothing
moves, the block is returned untouched. That is how the defaults — and every neutral
setting (shelves at 0 dB wherever they sit, bands at 100 % wherever the crossovers are, a
Haas amount without a delay, any slope) — are bit-exact. A swap or a polarity flip alone
is bit-exact too (`0·x + y`).

## Glides, fades, and the grid

Every value glides: gains in 10 ms, frequencies in 20 ms on a log scale, the Haas delay in
40 ms. The glides step every 16 samples counted from the stream's start (ADR 0020 §4's
grid), and the matrices and the band widths move linearly sample by sample between grid
points, so no step is heard. A glide lands on its target exactly (it snaps when close or
when a step no longer moves the value), which is what lets a control put back become a
wire again.

A stage that starts or stops fades over 10 ms. A **structural change** — another mono
slope, another shelf slope, another Haas mode — waits until its stage has faded out,
switches, and fades back in: nothing clicks (the test throws eight switches at an 80 Hz
tone and finds no step larger than four of the tone's own). While a stage is silent its
frequency jumps to where it goes and its filters are emptied, so it always starts cold
and clean.

Everything runs per sample from the stream's start, so the output does not depend on the
block size, to the bit (and the meters neither, except the per-block peaks).

## Meters (telemetry)

`cantus_dsp::latitudo::view::telemetry` names the 64 slots: the vectorscope's last 48
points of the output (12 bits per coordinate, packed into one exact `f32`), sampled every
`round(sr / 2880)` samples so a frame's worth arrives each frame; correlation and balance
of output and input over 300 ms; correlation in three bands (one-pole splits at the
bands' crossovers); the side's level against the mid, of output and input; levels and the
last block's peaks. The meters never touch the audio.

## Decisions

- **One mono control, not a mono control and a side high-pass.** They are one operation —
  removing the side below a frequency. The slope chooses the character: 6–12 dB/oct a
  gentle side high-pass, 24–48 dB/oct a steep mono-maker. Every slope is phase-coherent.
- **Minimum-phase crossovers, not linear phase.** The all-pass is applied to mid and side
  alike, so the *image* is untouched; what remains is a phase rotation of the whole signal
  at the crossover, which is inaudible on its own and costs no latency. A linear-phase
  option would cost tens of milliseconds at 100 Hz on the master; it is left for when a
  reason shows up.
- **Balance, not pan**: the far side is turned down linearly, the near one never raised
  (no surprise level jumps). To *move* an image keep its power and use rotation.
- **Side listen plays the side in phase in both speakers**, so it can be judged as a sound.
- **The hue is space** (the kit's own words: width belongs to delay, reverb and chorus);
  the tone cards and the mid curve take the tone hue, the output card the output hue.

## The editor

Row one: the **vectorscope** (dots fading with age, the polar outline, level rings, the
correlation and the side level at the top, the correlation and balance meters under it,
input marks and the three-second minimum) and the **map** (width by frequency with the
mono zone hatched and each band's correlation; tone curves of mid and side below, with a
dot per shelf and the slope switch). Row two: **Image** (width, mid, side, rotation,
balance), **Width by band** (three widths, two crossovers), **Output** (output gain, what
is heard, swap and the two polarities). Row three: **Mid tone**, **Side tone**, **Mono
bass** (frequency and slope), **Haas** (delay, amount, mode, and what it does to mono).

Thirteen factory presets cover the usual jobs: master low-end tightening, a wider top with
mono bass, the Patcher-style M/S EQ, the mono check and side solo, narrowing, a wide pad,
the two Haas modes, the two fixes (swapped channels, a polarity flip), a vocal focus and a
steep bass mono.
