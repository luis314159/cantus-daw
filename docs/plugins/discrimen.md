# Discrimen — the transient splitter

*Effect kind `TransientSplit`. References: Eventide SplitEQ (its structural split of
transients and tones), the transient/tonal separators of the restoration tools (ADR 0056).*

Discrimen cuts a sound into its **transient** (the attack of every hit) and its **body**
(everything else), gives each its own **gain**, **compressor** and **tilt**, and sums them
back. Left alone, the two parts add up to the input **to the bit** — so it can sit on any
track and only change what is asked of it. Detection is in **zero latency** or with a
**2 ms look-ahead** that puts every onset in the transient part from its first sample.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/discrimen.rs` |
| DSP | `crates/cantus-dsp/src/discrimen.rs` (tests in `discrimen_tests.rs`); the detector in `dynfx_transient.rs` |
| Engine glue | `crates/cantus-engine/src/fx/discrimen.rs` |
| Editor | `crates/cantus-ui/src/views/fx/discrimen.rs`, the family's parts in `dynfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/discrimen.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_discrimen.rs`, `crates/cantus-ui/tests/fx_discrimen.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Discrimen` (CLAP `com.github.lalmazan.cantus.discrimen`, VST3 `258467C3841A85F4F833DA7137868CD9`) |

## What the references do well, and what people complain about

- **SplitEQ** splits a signal into transient and tonal layers and EQs each: loved for drum
  rooms and for taming a guitar's pick without dulling it. People ask for dynamics per
  layer, a way to hear each layer alone, and a guarantee that the split itself is
  transparent.
- Transient designers (Impetus's family) shape the level only; a splitter lets a part be
  *coloured* differently — a bright attack over a warm body.

## Parameters (keys are forever)

| Key | Range | Default | Meaning |
|---|---|---|---|
| `mode` | Zero latency, Look-ahead | Zero latency | the detection; the look-ahead adds 2 ms of latency |
| `sens` | 0 … 100 % | 50 % | how little of a jump counts as a transient (full at `3 + 21·(1 − sens)` dB) |
| `speed` | 1 … 50 ms | 10 ms | how long a transient lasts |
| `t.gain`, `b.gain` | −24 … +24 dB | 0 | each part's gain |
| `t.comp`, `b.comp` | 0 … 100 % | 0 | each part's one-knob compressor: threshold `−36·amount` dBFS, ratio `1 + 5·amount` (the transient's 0.5/40 ms, the body's 10/150 ms) |
| `t.tilt`, `b.tilt` | −6 … +6 dB | 0 | each part's tilt around 1 kHz |
| `listen` | Both, Transient, Body | Both | what is heard |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

**The exact split.** A mask `m` (0 … 1) says how much of each sample is transient. The
larger part is taken by a product and the other by a subtraction that is exact (Sterbenz:
`x − y` has no rounding when `y` is within a factor of two of `x`): for `m ≥ ½` the transient
is `m·x` and the body `x − m·x`; below, the body is `(1 − m)·x` and the transient the rest.
Their sum is `x` exactly, for every mask and every sample (a test sweeps both); a zero keeps
its sign. With nothing processed the output is the input — through the whole engine, the
render is the render without Discrimen, bit for bit.

**The mask** comes from the family's detector (`dynfx_transient`: a running maximum, a fast
envelope over a slow one, in dB) through a smoothstep. With the **look-ahead**, the audio
waits 2 ms while the mask is a running maximum over the samples ahead, so the mask is up
before the onset arrives. The latency is the mode's, never a level's.

**Each part** runs its compressor and tilt only when they are away from 0; a section
switched off is emptied, so the unit always comes to rest.

## Measured

- The parts summed are the input to the bit, in both modes, at every sensitivity; through
  the engine at its defaults; the look-ahead is the render later by exactly 96 samples
  (2 ms at 48 kHz).
- **Zero latency**: of the first 0.25 ms of a hit **75 %** of the energy is in the transient
  part, of the first 2 ms **96 %**; **look-ahead**: **100 %** and **100 %**. The tail 50 ms on:
  **0.0 %** in the transient part in both.
- Block sizes 1 … 4 096 identical, live = offline, exact rest.

## Presets

More attack, Less attack, Drums: tame the room, Snappy and dry, Bright attack, warm body,
Squash the body, Only the transients, Only the body.
