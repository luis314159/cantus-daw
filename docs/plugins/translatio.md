# Translatio — the frequency shifter

*Effect kind `FreqShift`. References: Harald Bode's frequency shifter and the Moog/Bode
module, the Moogerfooger MF-102's and FL Studio's Frequency Shifter, Eventide's and
Soundtoys' shifters, Kilohearts Frequency Shifter (ADR 0055).*

Translatio moves every partial by the **same number of hertz** — not by a ratio, as a pitch
shifter does — so a harmonic tone stops being harmonic: a few hertz make a slow phasing
(half mixed), tens a detuned shimmer, hundreds a bell or a robot. It is a single-sideband
shifter in Bode's style: the signal's analytic pair from a **Hilbert pair of allpass
chains** times a quadrature carrier, with the **up** and **down** sidebands (or both, one
per side), **feedback** for the barber-pole spirals, and an **LFO** on the shift.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/translatio.rs` |
| DSP | `crates/cantus-dsp/src/translatio.rs` (tests in `translatio_tests.rs`); the Hilbert design in `modfx_hilbert.rs` (tests in `modfx_hilbert_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/translatio.rs` |
| Editor | `crates/cantus-ui/src/views/fx/translatio.rs` |
| Strings | `crates/cantus-ui/locales/fx/translatio.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_translatio.rs`, `crates/cantus-ui/tests/fx_translatio.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Translatio` (CLAP `com.github.lalmazan.cantus.translatio`) |

## The Hilbert pair: how good, measured

The pair is the classic polyphase design (Valenzuela & Constantinides): an elliptic
half-band filter's two allpass branches in `z²`, turned into a 90° splitter by `z → j·z` —
each section `(c − z⁻²)/(1 − c·z⁻²)`, the coefficients alternating between the chains, one
chain a sample late. The coefficients come from closed-form elliptic series of the
transition width (our implementation of the published maths), computed for the sample rate
when the unit is built, with as many sections as it takes for 66 dB over 20 Hz … 20 kHz.

| Sample rate | Sections | Designed (exact response) | Measured (running, f32) |
|---|---|---|---|
| 44.1 kHz | 14 | 69.5 dB | ≥ 69.2 dB (worst of both rates) |
| 48 kHz | 14 | 68.7 dB | |
| 88.2 / 96 kHz | 16 | 69.6 / 68.6 dB | |

The requirement was 60 dB over 20 Hz – 20 kHz at 44.1 and 48 kHz; the test measures it
through the running shifter at ten frequencies from 20 Hz to 19.9 kHz (`translatio_tests.rs`).
The editor prints the design's figure in its corner.

## What the references do well, and what people complain about

- Bode's shifter is the reference for its two outputs and its clean sidebands; digital
  shifters often leak the unwanted sideband at low frequencies (a short Hilbert design)
  and few say how much.
- Feedback makes the famous spirals — and in many shifters runs away near DC, where the
  Hilbert pair is no longer in quadrature.
- People confuse a shifter with a pitch shifter.

Translatio states its rejection (measured, ≥ 60 dB from 20 Hz), keeps its feedback inside
the pair's 90° band (a one-pole high-pass at 25 Hz and a low-pass at 16 kHz in the loop), so
95 % feedback stays bounded at any shift, and draws what a harmonic 220 Hz tone becomes.

## Parameters (keys are forever)

The mix starts at 0, so Translatio inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `shift` | −2 000 … 2 000 Hz | 10 Hz | every partial moves this far (negative: the other way) |
| `side` | 0 … 2 | Up | Up, Down, Both (left up, right down) |
| `fb` | 0 … 95 % | 0 | the shifted output back into the input |
| `lfo` | 0 … 1 000 Hz | 0 | the LFO's swing of the shift either way |
| `sync`, `div`, `rate` | | free, 1/1, 0.2 Hz | the LFO (0.02 … 10 Hz free) |
| `shape` | 0 … 5 | Sine | the LFO's shape |
| `mix` | 0 … 100 % | 0 | equal-gain crossfade; 100 % the shifted signal alone |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- Up `I·cos φ − Q·sin φ`, down `I·cos φ + Q·sin φ`; the carrier's phase runs per sample
  at the shift (with the LFO's swing) in `f64`.
- No latency: the allpasses turn the phase only.
- **Exact rest**: the chains, the feedback and its filters flushed after a silence; the
  carrier's phase is not audio state, and `skip` turns it sample by sample as processing
  would.

## Factory presets

Slow phase · Barber pole, rising · Barber pole, falling · Stereo detune · Bell · Robot ·
Wobble on the quarters · Stereo spin.

## What the tests hold

- At its defaults and at mix 0 the render is the render without it, to the bit; working,
  the same bits at blocks of 1, 7, 64, 1 000 and 4 096 for every sideband; live equal to
  offline (`fx_translatio.rs`).
- The DSP: up and down move a 1 kHz tone 100 Hz at full level, the other sideband and the
  input under −60 dB; both and a negative shift swap sides; the other sideband ≥ 60 dB down
  from 20 Hz to 19.9 kHz at 44.1 and 48 kHz (69.2 dB worst measured); each feedback pass
  shifted again at the feedback's level; 95 % feedback bounded at shifts from −500 to
  2 000 Hz; split invariance; rest and `skip` exact.
- The editor: the partials drawn are the partials played (three settings, feedback
  included, within 1 dB); its cards and the printed rejection; every preset in both
  languages.
- The family's contract and the CLAP/VST3 export, bit-identical, the shift automated.
