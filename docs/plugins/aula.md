# Aula — the convolution reverb

*Effect kind `Aula`. References: Logic's Space Designer, Altiverb, Waves IR-1, REAPER's
ReaVerb, FL Studio's Fruity Convolver, Ableton's Hybrid Reverb, LiquidSonics Reverberate
(ADR 0112).*

Aula is the reverb of a real space: the sound convolved with an **impulse response**, a
recording of how a room answers one click. It plays Cantus's own fifteen responses (rooms,
chambers, halls, plates, springs, all true stereo, made deterministically at the engine's
rate), the user's files (WAV of 8/16/24/32-bit integer or 32-bit float, AIFF, FLAC, Ogg;
mono, stereo or true stereo; any rate; up to 20 s), and a free CC BY pack of eight spaces
from the OpenAIR library.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/aula.rs` (and `EffectSpec::ir`, `Command::SetEffectIr`) |
| DSP | `crates/cantus-dsp/src/aula.rs` (the effect), `aula_conv.rs` (the convolver), `aula_kernel.rs` (partitions and schedules), `aula_fft.rs`, `aula_shape.rs` (the response's shaping), `aula_gen.rs` (Cantus's own responses); tests in `aula_tests.rs`, `aula_conv_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/aula.rs` (the unit, its needs), `aula_store.rs` (files read, kernels kept), `aula_load.rs` (reading files) |
| Editor | `crates/cantus-ui/src/views/fx/aula.rs`, `aula_view.rs` |
| Strings | `crates/cantus-ui/locales/areas/aula.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/aula.toml` |
| Agents | `set_effect` (numbers), `set_impulse_response` (a file, files class) — `crates/cantus-ai/src/tools_aula.rs` |
| Tests | `crates/cantus-engine/tests/fx_aula.rs`, `crates/cantus-ui/tests/aula_e2e.rs`, the family's contract (`fx_contract.rs`), `rt_memory.rs`, the plug-in tests |
| Exported | `Cantus Aula` (CLAP `com.github.lalmazan.cantus.aula`): Cantus's own responses only |

## What the references do well, and what people complain about

- Space Designer and Altiverb: the response drawn, shaped by an envelope and EQ, *stretched*
  to a bigger room — and they *add latency* or cost a lot at small buffers.
- REAPER's ReaVerb: zero latency with "ZL" mode at a CPU cost, true stereo by channel count —
  but its picture of the response is minimal.
- Fruity Convolver: an envelope drawn on the response, stereo separation — 16-bit era
  limitations, no true stereo.
- Every one: loading a response restarts the tail (a click, the reverb cut).

## What Aula does

- **Zero latency, constant cost.** The first 64 taps in direct form; then levels of uniform
  partitions of 64, 512, 4 096 and 32 768 samples (15, 14, 14 and as many as needed), each
  level starting at twice its partition so its work — a forward transform, the products, an
  inverse — is cut into pieces and spread over the blocks of its slack. Two inputs share one
  complex transform, two outputs one inverse.
- **Exact.** The grid counts from the stream's start, so any split of the host's blocks gives
  the same bits; within 3.1 × 10⁻⁷ of the peak (−130 dB) of a direct convolution for every
  length and format tested; at mix 0 the dry passes to the bit; it rests exactly in silence.
- **Swaps without a click.** A new response is taken level by level at each level's period
  start; the delay lines hold the inputs, not the response, so the tail of what played
  continues through the new response.
- **The response shaped off the audio thread**: stretch (one windowed-sinc resampling with the
  rate conversion), the gate (the noise floor), length, decay, damping (frames of 512, the
  highs lowered with time), low and high cuts (12 dB/oct), reverse, attack; loudness set
  once from the stretched response.
