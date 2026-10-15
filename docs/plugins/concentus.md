# Concentus — the chorus

*Effect kind `Chorus`. References: the Roland Juno-60's chorus (and TAL-Chorus-LX, its
best-known emulation), Roland's SDD-320 Dimension D and its clones, FL Studio's Fruity
Chorus, the ensembles of Arturia and Waves (ADR 0055).*

Concentus thickens and widens. Its models are the two choruses everyone reaches for — the
**Juno-60's** buttons I, II and I+II, and the **Dimension**'s four — and a **Free** mode of
one to four voices on the family's LFO (free or synced). Every wet voice is a bucket-brigade
style line: a swept delay, darkened by a reconstruction filter (`tone`). The editor draws
every voice's delay over two cycles and checks the **mono** sum as it plays.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/concentus.rs` |
| DSP | `crates/cantus-dsp/src/concentus.rs` (tests in `concentus_tests.rs`); the family's line, clock and LFO |
| Engine glue | `crates/cantus-engine/src/fx/concentus.rs` |
| Editor | `crates/cantus-ui/src/views/fx/concentus.rs`, the family's parts in `modfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/concentus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_concentus.rs`, `crates/cantus-ui/tests/fx_concentus.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Concentus` (CLAP `com.github.lalmazan.cantus.concentus`, VST3 `FD2C6C3440D7C2537A8189D03B59D849`) |

## What the references do well, and what people complain about

- The **Juno chorus** is loved for its width and its slightly dark, noisy wet path; its
  clones disagree on rates and sweeps, and few say where their numbers come from.
- The **Dimension D** is loved because it widens without the "seasick" movement and keeps
  the mono sum; its clones offer four buttons and nothing else.
- **Fruity Chorus** and most multi-voice choruses offer many knobs and no picture; the
  commonest complaint about any wide chorus is **what happens in mono**.

Concentus answers: the Juno's sweeps from the circuit's published measurements, a
Dimension-style crossed pair that loses little in mono, free voices when you want your own,
a picture of every voice's delay, and a mono check (correlation, the mono sum's loss, and a
verdict in words).

## Parameters (keys are forever)

The mix starts at 0, so Concentus inserted and left alone returns its input to the bit; the
model waits at Juno I.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `mode` | 0 … 7 | Juno I | Juno I, Juno II, Juno I+II, Dimension 1 … 4, Free voices |
| `mix` | 0 … 100 % | 0 | equal-gain crossfade: at 50 % dry and wet are both at full level (the classic chorus sum); 0 % dry, 100 % wet |
| `voices` | 1 … 4 | 3 | Free: how many voices, their phases spread evenly round the cycle |
| `sync` | Free / Sync | Free | Free: the rate follows the song or runs free |
| `div` | 8/1 … 1/32 | 1/1 | Free: the synced cycle |
| `rate` | 0.05 … 10 Hz | 0.8 Hz | Free: the free rate |
| `shape` | Sine / Triangle | Triangle | Free: the sweep's shape |
| `delay` | 1 … 30 ms | 7 ms | Free: the centre delay |
| `depth` | 0 … 8 ms | 2 ms | Free: the sweep either side |
| `spread` | 0 … 100 % | 100 % | width: how far the right channel's sweep is from the left's, the voices' places across the field |
| `tone` | 2 … 20 kHz | 9 kHz | the wet path's two-pole low-pass (20 kHz: open) |
| `out` | −24 … +24 dB | 0 | output gain |

## The models

| Model | Rate | Delay | Notes |
|---|---|---|---|
| Juno I | 0.513 Hz | 1.66 … 5.35 ms | triangle; one line per channel, the right swept by the inverted triangle |
| Juno II | 0.863 Hz | 1.66 … 5.35 ms | as I, faster |
| Juno I+II | 9.75 Hz | 3.3 … 3.7 ms | the fast shallow shimmer of both buttons |
| Dimension 1 … 4 | 0.25 / 0.25 / 0.25 / 0.5 Hz | 6 ms ± 0.25 / 0.45 / 0.7 / 0.9 ms | two lines in opposite phase, each fed out of phase into the other channel by 25 / 35 / 45 / 50 % |
| Free | your LFO | `delay` ± `depth` | 1 … 4 voices placed across the field by `spread`, summed at `1/√n` |

The Juno figures are the circuit's as measured and published by the people who modelled it
(the triangle's rates set by its RC network, the MN3009's delay range); the Dimension voicing
is our own in the spirit of the SDD-320's four buttons, not a measurement — the editor and
this page say so.

## How it works

- Lines read with a 4-point Hermite; the LFO is the family's (a function of the clock, so the
  same at every block size, offline and live; Free can sync to the song).
- Model changes glide (centre, sweep, cross-feed: 40 ms), so switching while playing does not
  click.
- **Mono check** (telemetry, never the sound): leaky sums of L², R² and L·R over about
  300 ms give the correlation and the mono sum's level against the stereo's —
  `10·log10((L²+R²+2LR)/(2(L²+R²)))`: 0 dB for identical channels, −3 dB for uncorrelated,
  −∞ for opposite.
- **Exact rest**: both lines empty (counted, not scanned), the tone filter's states flushed
  after a run of silence, the glides landed.

## Factory presets

Juno I · Juno II · Juno I+II shimmer · Dimension, subtle · Dimension, widest · String
ensemble · Vocal double · Lush four voices · Swirl on the bar · Bass, mono-safe.

## What the tests hold

- At its defaults and at mix 0 in every model, the render is the render without it, to the
  bit; working, the same bits at blocks of 1, 7, 64, 1 000 and 4 096; live equal to offline
  (`fx_concentus.rs`).
- The DSP: each Juno model's measured sweep (min and max delay within 0.01 ms, rate within
  1 %), the right channel's sweep opposite the left's; Dimension 4 wider (lower correlation)
  than Juno I while neither loses 3 dB in mono; 2–4 free voices within 2.5 dB of one; the
  mono check's arithmetic; split invariance in every model; rest and `skip` exact.
- The editor: the delays drawn are the delays the running unit plays (within 0.001 ms, every
  model); its cards and words; a model chosen is one undo step; every preset in both
  languages and audible.
- The family's contract and the CLAP/VST3 export, bit-identical, mix automated.
