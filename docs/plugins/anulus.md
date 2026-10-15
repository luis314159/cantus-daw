# Anulus — the ring modulator

*Effect kind `RingMod`. References: the diode ring modulators of the Moog and EMS modules
and the Moogerfooger MF-102, FL Studio's Fruity Ringmod and Frequency Shifter's ring
mode, Soundtoys FilterFreak's and Kilohearts' ring modulators (ADR 0055).*

Anulus multiplies the sound by a carrier: every partial `f` becomes the pair `f ± f_c` — the
inharmonic, bell-like and metallic sound of the ring of diodes it is named for; at a few
hertz, a tremolo that runs through zero. The carrier is a **sine, triangle or square**,
band-limited; an **LFO** swings its frequency by up to two octaves either way, synced or
free; the right carrier's phase can be apart from the left's; and the product's DC is
taken away, so nothing leaks.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/anulus.rs` |
| DSP | `crates/cantus-dsp/src/anulus.rs` (tests in `anulus_tests.rs`) |
| Engine glue | `crates/cantus-engine/src/fx/anulus.rs` |
| Editor | `crates/cantus-ui/src/views/fx/anulus.rs` |
| Strings | `crates/cantus-ui/locales/fx/anulus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_anulus.rs`, `crates/cantus-ui/tests/fx_anulus.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Anulus` (CLAP `com.github.lalmazan.cantus.anulus`) |

## What the references do well, and what people complain about

- Analog ring modulators leak the carrier and the input (imperfect diodes) — loved as
  character, hated when a quiet passage whistles.
- Digital ones are clean but their square and triangle carriers alias at high frequencies.
- Nobody can predict the result from the knobs: the partials are sums and differences.

Anulus is clean (no carrier or input bleed: a tone through it leaves nothing at its own
frequency, below −80 dB), band-limits its carriers (polyBLEP steps and polyBLAMP corners), keeps
DC out, shows the carrier as a note with its cents, and draws **what a 1 kHz tone becomes** —
every partial the carrier makes of it, at its level (measured against the DSP within 1 dB).

The carrier is set by its frequency, not by played notes: following a track's notes (as
the brief allows, *only if complete with the export*) waits for the exported plug-ins' note
input, which belongs to the voice builder's Tonus this round.

## Parameters (keys are forever)

The mix starts at 0, so Anulus inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `freq` | 1 … 5 000 Hz | 440 Hz | the carrier's frequency (shown as a note too) |
| `wave` | 0 … 2 | Sine | Sine, Triangle, Square — band-limited |
| `lfo` | 0 … 24 st | 0 | the LFO's swing of the frequency either way |
| `sync`, `div`, `rate` | | free, 1/1, 0.5 Hz | the LFO (0.02 … 20 Hz free) |
| `shape` | 0 … 5 | Sine | the LFO's shape |
| `spread` | 0 … 180° | 0 | the right carrier's phase ahead of the left |
| `mix` | 0 … 100 % | 0 | equal-gain crossfade; 100 % the product alone |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- The carrier's phase runs per sample at `f·2^(st·m/12)/fs`; the square adds `±2·r(τ)` at
  its steps and the triangle `∓8·dt·R(τ)` at its corners, `r` the polyBLEP residual of a
  unit step and `R` its integral, within a sample of each (`τ` in samples).
- The product goes through a one-pole DC blocker at 5 Hz: a partial on the carrier's own
  frequency (or an offset in the input) makes DC, which never reaches the output.
- **Exact rest**: silence times any carrier is silence, so the carrier's phase is not audio
  state; `skip` advances it sample by sample exactly as processing silence would. The DC
  blocker is flushed after a silence; the glides land. (A product of +0 by a negative
  carrier is −0, normalised to +0 so a resting state's bits are a new unit's.)

## Factory presets

Bell · Robot · Metal voice · Tremolo through zero · Siren · Metal drums · Stereo shimmer ·
Random steps.

## What the tests hold

- At its defaults and at mix 0 the render is the render without it, to the bit; working,
  the same bits at blocks of 1, 7, 64, 1 000 and 4 096 for every wave; live equal to offline
  (`fx_anulus.rs`).
- The DSP: a 1 kHz tone with a 300 Hz sine carrier becomes 700 and 1 300 Hz at half level
  each, its own frequency below −80 dB; a 5 kHz square or triangle carrier's folded ninth
  harmonic at least 10 dB under a naive one's, the fundamental kept; no DC (< 10⁻³) from a
  partial on the carrier plus an input offset; the LFO swings ±7 st exactly; split
  invariance; rest and `skip` exact.
- The editor: the partials drawn are the partials played (sine, triangle and square, the
  first six partials within 1 dB); notes named with their cents; every preset in both
  languages.
- The family's contract and the CLAP/VST3 export, bit-identical, the frequency automated.
