# Libra — the utility

*Effect kind `Utility`. References: Ableton Live's Utility and Tuner, FL Studio's Fruity
Balance and Stereo Shaper, Waves InPhase (ADR 0130).*

*Libra* is Latin for the balance — the scales. Libra holds a channel's everyday
corrections, in this order: **DC removal**, the **channel operation** (swap, mono, one side
to both, mid, side), **polarity** per channel, a **delay** per channel in whole samples (two
microphones aligned to the sample), **width**, the **balance** (the master's balance law,
ADR 0012) and the **gain** (±36 dB), **mute** last. It inserts as a wire.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/libra.rs` (12 keys) |
| DSP | `crates/cantus-dsp/src/libra.rs` (the chain, rest, the meters' telemetry, the tuner's reading); tests in `libra_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/libra.rs` |
| Editor | `crates/cantus-ui/src/views/fx/libra.rs` (the cards), `libra_display.rs` (the tuner, the goniometer, the layout and its drags), `libra_balance.rs` (the brass balance) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`LIBRA`: blue-grey anodised, signal green on a dark screen); the balance's brass in `kit/skin/scales.rs` |
| Strings | `crates/cantus-ui/locales/fx/libra.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/utility.toml` (fourteen; no starting preset — it inserts as a wire) |
| Tests | `crates/cantus-engine/tests/fx_libra.rs`, `rt_memory::libra_never_allocates`, the family's contract; `crates/cantus-ui/tests/libra_e2e.rs`; the CLAP/VST3 tests over every descriptor |
| Exported | `Cantus Libra` (CLAP `com.github.lalmazan.cantus.libra`) |
| Example | `shoot_libra` (the design sheet) |

## The window

At the centre **a brass balance**: its beam tilts with the left/right balance and reads it on
a fine scale; the arms reach as wide as the image (the width); each pan holds its channel —
its level, its delay and, when inverted, its polarity, each in words; the correlation runs
along the stand's foot. Drag it across for the balance, up and down for the width;
double-click to centre it. Around it, the **tuner** (a needle on a scale of ±50 cents, the
note, its frequency, its reference A from 415 to 466 Hz dragged up and down beside it) and
the **goniometer** (the output's mid up, its side across).

## Ours, better

- Every stage at its identity is skipped, not multiplied by one: the defaults are a wire to
  the bit, 0 dB is exact.
- The delay is whole samples, exact (the alignment of two microphones), shown in ms or in
  samples.
- The tuner reads within ±1 cent on a sine and ±3 on a sung vowel (measured on the pitch
  tests' material).
