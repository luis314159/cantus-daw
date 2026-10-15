# Vibrans — the vibrato

*Effect kind `Vibrato`. References: Audio Damage Vibrato, Soundtoys PitchBlender's
vibrato, the vibrato channels of amp simulators and organs, FL Studio's Fruity Chorus at one
voice (ADR 0055).*

Vibrans makes the pitch waver — a singer's or a violinist's vibrato, an organ's, the wow of
a tape — by reading the sound from a delay line whose length moves. The depth is set **in
cents**, so the pitch swings exactly as far as the knob says whatever the rate; a **rise**
lets the vibrato arrive after each note, as a performer's does; the stereo phase widens it;
the mix turns it into a one-voice chorus.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/vibrans.rs` |
| DSP | `crates/cantus-dsp/src/vibrans.rs` (tests in `vibrans_tests.rs`); the family's line, clock and LFO in `modfx_line.rs`, `modfx_clock.rs`, `modfx_lfo.rs` |
| Engine glue | `crates/cantus-engine/src/fx/vibrans.rs` |
| Editor | `crates/cantus-ui/src/views/fx/vibrans.rs`, the family's parts in `modfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/vibrans.{en,es}.txt` (the family's words in Tremor's) |
| Tests | `crates/cantus-engine/tests/fx_vibrans.rs`, `crates/cantus-ui/tests/fx_vibrans.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Vibrans` (CLAP `com.github.lalmazan.cantus.vibrans`) |

## What the references do well, and what people complain about

- Most vibratos set the depth as a time (ms of delay) or a percentage: a faster rate then
  swings the pitch further, and the number on the knob means nothing musical.
- A performer's vibrato **arrives after the note**; plug-ins rarely do that without a MIDI
  input.
- Tape-style wow needs very slow rates with a large sweep — many units stop at 1 Hz or cap
  the sweep without saying so.

Vibrans answers: depth in cents (the sweep follows from rate and shape), a rise after every
silence detected in the audio itself, rates down to 0.2 Hz at the full 100 ct, and a display
that draws the **pitch** — not the delay — over two cycles.

## Parameters (keys are forever)

The depth starts at 0 cents, so Vibrans inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `depth` | 0 … 100 ct | 0 | the pitch's swing either way |
| `sync` | Free / Sync | Free | the rate follows the song (`div`) or runs free (`rate`) |
| `div` | 8/1 … 1/32 | 1/16 | the synced cycle as a note value |
| `rate` | 0.2 … 15 Hz | 5.5 Hz | the free rate |
| `shape` | Sine / Triangle | Sine | a sine swings smoothly; a triangle alternates two pitches |
| `rise` | 0 … 2 000 ms | 0 | the depth fades in over this after each silence |
| `spread` | 0 … 180° | 0 | the right channel's LFO ahead of the left |
| `mix` | 0 … 100 % | 100 % | the moving copy against the dry (lower: a chorus) |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- The line is read at `D₀ + A·m(t)` (4-point Hermite). The pitch is `1 − dD/dt`, so for a
  sine of rate `f` a sweep `A = (2^{c/1200} − 1)/(2πf)` swings it by `c` cents; for a
  triangle `A = (2^{c/1200} − 1)/(4f)`. `A` is capped at 100 ms (the depth is reached down
  to 0.2 Hz for a sine). `D₀ = A + 2` samples; at depth 0 both are 0 and the line returns
  the newest sample — the input, to the bit.
- Rate and depth changes glide (60 / 120 ms), so automating them never clicks; the synced
  rate follows the tempo, and the start of playback lands the glides where the tempo puts
  them (a render from any bar is the full render there).
- **Rise**: 50 ms under −50 dBFS makes the next sound an onset; the depth then rises
  linearly over the rise time. Counted in whole samples, so `skip` reproduces it exactly.
- **Latency**: none reported — the centre delay moves with depth and rate and is part of
  the sound (as in every vibrato); at 100 % mix nothing is heard un-delayed.
- **Exact rest**: the line empties (counted, not scanned), the glides land, and a silence
  long enough to re-arm the rise has passed.

## Factory presets

Singer · Violin · Organ vibrato · Tape wow · Seasick · Two-note trill · Wobble on the
eighths · One-voice chorus.

## What the tests hold

- At its defaults and at depth 0 with everything else moved, the render is the render
  without it, to the bit; working, the same bits at blocks of 1, 7, 64, 1 000 and 4 096;
  live equal to offline, free and synced (`fx_vibrans.rs`).
- The DSP: a 1 kHz tone swings by the cents asked for (±8 %) at 50 ct / 5 Hz sine,
  25 ct / 3 Hz triangle and 100 ct / 0.5 Hz; the rise holds the first 100 ms under 12 ct of
  a 60 ct vibrato and reaches it after; split invariance; a synced render from bar 5 equals
  the full render there; rest and `skip` exact.
- The editor: the pitch drawn is the pitch played (zero-crossing periods of the DSP's output
  against the curve, within 4 ct); its cards; every preset in both languages.
- The family's contract and the CLAP/VST3 export, bit-identical, depth automated.
