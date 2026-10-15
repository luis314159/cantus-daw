# Tonsor — the clipper, with a true-peak ceiling

*Effect kind `Clipper`. References: FL Studio's Fruity Soft Clipper, Kazrog's KClip,
StandardCLIP, the clip stage of mastering limiters (ADR 0056).*

Tonsor cuts the peaks off a sound — the cleanest way to win loudness on drums and masters,
because a clipper has no attack and no release to pump. It does it **soft, hard or with an
analog shoulder**, at **1× to 16× oversampling** so the harmonics it makes do not fold back
as aliasing, and — with the **true-peak ceiling** on — it holds the ceiling between the
samples too, where a plain clip leaves inter-sample overs that a converter or a lossy encoder
turns into distortion.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/tonsor.rs` |
| DSP | `crates/cantus-dsp/src/tonsor.rs` (tests in `tonsor_tests.rs`); the oversampler in `dynfx_os.rs` |
| Engine glue | `crates/cantus-engine/src/fx/tonsor.rs` |
| Editor | `crates/cantus-ui/src/views/fx/tonsor.rs`, the family's parts in `dynfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/tonsor.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_tonsor.rs`, `crates/cantus-ui/tests/fx_tonsor.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Tonsor` (CLAP `com.github.lalmazan.cantus.tonsor`, VST3 `496542F994F627269D2426F605EFCA20`) |

## What the references do well, and what people complain about

- **Fruity Soft Clipper** is two knobs (threshold, post gain) and a curve: quick, but no
  oversampling, no meter, no true peak — it aliases audibly on hats and leaves overs.
- **KClip** and **StandardCLIP** add oversampling, a clip meter and "true peak" modes, and
  are the mastering engineers' tools; people ask for a clearer picture of *how much* is
  being clipped and *how often*, and complain when a "0 dB" setting still colours the sound.

Tonsor answers each: a null that holds to the bit (below), oversampling to 16×, a true-peak
ceiling measured against a 32-point reconstruction, and an editor that shows the curve the
DSP runs, the reduction, the share of samples clipped and the overs between the samples.

## Parameters (keys are forever)

Every default is a null: a 0 dBFS ceiling with no knee, at 1× with the guard off, changes no
sample that is not over 0 dBFS.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `drive` | −12 … +24 dB | 0 | input gain into the curve |
| `ceil` | −24 … 0 dB | 0 | the ceiling (dBTP with `tp` on) |
| `knee` | 0 … 12 dB | 0 | how far under the ceiling the curve starts to bend |
| `curve` | Hard, Soft, Analog | Soft | the shoulder's shape |
| `os` | 1×, 2×, 4×, 8×, 16× | 1× | oversampling |
| `tp` | Off / On | Off | the true-peak ceiling |
| `match` | Off / On | Off | gain match: the drive taken back out after the curve |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

**The curves.** From the ceiling `c` and the knee's start `k = c·10^(−knee/20)`, with
`v = (|u| − k)/(c − k)`, below `k` every curve is the straight line (the sample itself, to the
bit): *Hard* is a corner rounded by a parabola, `k + (c−k)(v − v²/4)`, flat from `v = 2` (a knee
of 0 is a plain clip); *Soft* is `k + (c−k)·tanh v`; *Analog* is `k + (c−k)·v/√(1+v²)`, the
slowest shoulder — the rounding of a pair of diodes.

**Oversampling of what changes.** At 2× … 16× the input is interpolated by a polyphase
Kaiser-windowed sinc whose phase 0 is a pure delay; the curve is applied to every point;
only **what it changed** (`shape(u) − u`) is band-limited back to the base rate (a 769-tap
decimator at 16×, about 80 dB down from the base rate's Nyquist frequency) and added to the
input delayed by 40 samples. So a sample the curve leaves alone comes out untouched, to the
bit, at any factor, and the filters' ripple touches only the distortion.

**The true-peak guard.** The waveform between the samples is read as the meters read it (4×
and parabolas); where it would pass the ceiling, the gain each sample needs is held over the
samples around it (±20) and smoothed by a moving mean (±16), so the gain at any sample is no
more than its neighbours need and never steps; it is applied to the signal delayed by the
look-ahead (53 samples). It aims 0.1 dB under the ceiling: the detector reads within that of
the 32-point reconstruction.

**Latency** is a function of the configuration only: 0 at 1× without the guard, 40 samples
oversampled, 53 more with the guard (1.9 ms at 48 kHz at most). When automation switches
the oversampling or the guard, the engine plans delay compensation again before the next
pass (ADR 0056), so the mix stays in line.

**The meter** carries running counts (samples seen, samples clipped, input samples under the
ceiling whose waveform passes it) and peaks held for 50 ms, so the editor — which reads it
once a frame — loses nothing between frames: the share clipped and the overs per second are
over the last second of audio.

## Measured

- A null: at the defaults, any signal under 0 dBFS comes out bit-identical; at every factor
  and with the guard on, a signal with no overs comes out delayed by exactly the reported
  latency, through the whole engine too (the bass waits by the same amount).
- Inter-sample overs: a hot mix with a quarter-rate tone at 45° clipped plainly at −1 dB
  leaves **+1.88 dB** over the ceiling in dBTP; with the guard, **−0.045 dB** at 1× and
  **−0.072 dB** at 4× and 16× (the 32-point reference), −0.10 dB on the meter.
- An oversampled hard clip without the guard holds its ceiling in sample peak within
  **+0.034 dB** at 2× … 16× (review 1: the residual had been added a fraction of a sample
  early, +2.26 dB).
- Switching between two factors above 1× keeps the dry line (same latency, no hole).
- Aliasing: a 7 kHz sine clipped hard folds its 5th and 7th harmonics at **−15.9 dB** at 1×
  and **−84.7 dB** at 16×.
- The curve drawn is the curve played within 0.05 dB from −20 to +8 dB in.

## Presets

Safety ceiling, Streaming −1 dBTP, Soft master, Loud and clean, Drum bus, Snare crack,
Analog warmth, Hard 808.
