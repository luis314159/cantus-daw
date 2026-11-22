# Nitor — the one-knob enhancer

*Effect kind `Enhancer`. References for function: FL Studio's Soundgoodizer (one knob over
curated Maximus settings), Low Lifter (the low end) and the Maximus/Emphasis exciters; Aphex's
Aural Exciter and Big Bottom, Waves MaxxBass and Renaissance Bass, iZotope Ozone's Exciter —
read about and listened to, nothing copied; the design is our own (ADR 0151,
`docs/round41/nitor.md`). DSP: `cantus_dsp::nitor` (MPL-2.0).*

*Nitor* is Latin for brightness, splendour. One knob, **Amount**, turns up a **character**;
the low end and the exciter can be set on their own on top of it; and the output is
**level-matched**, so a change is heard as colour and not as loudness.

## The signal path

```text
       ┌─ × pre ─ Multiplex (the character, dynamic phase) ─ ÷ pre ─ tight ─ lift ─┐
in ─┬──┤                                                                           ├─ + ─ × match ─ mix ─ × out
    │  └─ the exciter by band (body, presence, air: harmonics only, 4× oversampled) ┤
    │  └─ the low end's sub (an octave down, or the song's tonic) and harmonics ───┘
    └─ dry ────────────────────────────────────────────────────────────────── mix ┘
```

- **The characters** (Clear, Punch, Warm, Air, Loud) are recipes for Multiplex's dynamic
  path — four bands (20–150 Hz, 150 Hz–1.5 kHz, 1.5–7 kHz, over 7 kHz) with downward and
  upward compression and a static tilt — plus what each adds to the exciter and the low end.
  The amount takes every ratio from 1:1 to the character's and every gain from 0 dB to its
  own. Dynamic phase: no latency, and while no band moves the path is the input to the bit.
- **Level independence.** A slow K-weighted loudness of the input brings any programme to
  −18 LUFS before the character's dynamics and takes it back after, so a character acts
  alike on a quiet stem and on a loud bus.
- **The exciter by band** adds only harmonics. Each band (a band-pass at 400 Hz and at 2.5
  kHz; a high-pass at 7 kHz) is normalised by its own envelope, driven into `v/√(1+v²)`
  less `v` (odd harmonics) and its square (even), blended by *Colour*, oversampled 4×
  (minimum phase), brought down, high-passed at the band's lower edge and multiplied back
  by the envelope — the same share of harmonics at −40 dBFS as at −6.
- **The low end**: the mid under the crossover (Linkwitz–Riley) feeds the **sub** — an octave
  down from Saltus's divider (phase-locked to the bass's edges, no latency), or a sine on the
  **song's tonic** (octave 1, 32.7–61.7 Hz) opened by each hit; **harmonics** (the bass's
  2nd–5th, 2× oversampled, kept between 1.5 and 6 times the crossover); **tight** (each hit
  opens the low band for 30 ms, then a 120 ms fall toward −12 dB × tightness, through a
  Butterworth low shelf designed each sample); **lift** (bells of Q 2.5 on the tonic, up to
  +6 dB, and its octave, +3 dB).
- **The level match**: the input's and the wet path's BS.1770 K-weighted loudness, each a
  running mean that becomes a 1.5 s exponential one, their ratio glided (150 ms) within ±12 dB
  on a 16-sample grid; under −70 LUFS nothing moves; after 8 s of exact digital silence it
  forgets and starts afresh.

## Parameters

| Key | What | Range | Default |
|---|---|---|---|
| `amount` | the one knob: how much of the character | 0 … 100 % | 0 |
| `char` | Clear, Punch, Warm, Air, Loud | 0 … 4 | Clear |
| `sub` | the sub's level | 0 … 100 % | 0 |
| `submode` | the sub follows the bass an octave down, or the song's tonic | 0, 1 | octave |
| `harm` | the bass's harmonics | 0 … 100 % | 0 |
| `tight` | the low end's decay shortened after each hit | 0 … 100 % | 0 |
| `lift` | the tonic's fundamental and octave lifted | 0 … 100 % | 0 |
| `xover` | the low band's crossover | 40 … 250 Hz | 100 Hz |
| `body`, `pres`, `air` | the exciter's bands | 0 … 100 % | 0 |
| `color` | even (warm) … odd (bright) harmonics | 0 … 100 % | 50 % |
| `match` | the output matched in loudness to the input | off, on | on |
| `mix` | dry/wet | 0 … 100 % | 100 % |
| `out` | output gain | −24 … +24 dB | 0 |
| `key` | the song's key (ADR 0153), or C … B fixed | song, C … B | song |

The keys are forever (the plug-in's ids are blessed in `cantus-plugins/tests/golden/ids.txt`).

## The song's key

The engine hands every effect the project's key (ADR 0153, `FxUnit::set_song_key`); with
`key` at *The song's*, the tonic sub and the lift follow it as soon as the project's key
changes. The exported plug-in has no song: there `key` fixes the tonic (C when left on *The
song's*).

## The null and rest

Every default is a wire to the bit (the amount and every part at 0; only `out` multiplies,
and at 0 dB it is not applied). Each part runs only while its level is above zero; a part's
state is cleared on the grid cell its level landed at zero, so the output does not depend on
the block size (the tests hold 1, 37, 64, 512 and mixed blocks to tolerance 0). Exact rest:
every part's state flushed, the level match forgotten after 8 s of digital silence, the
character's dynamics *parked* after 0.5 s of it (Multiplex's upward compression acts on
silence, so it never rests by itself; parked, it is reset and blended back in over 20 ms from
the first grid cell that carries sound — a new unit starts parked too). No latency.

## Presets

The starting preset on insert is *Gentle shine* (a third of Clear). Thirteen factory
presets (`presets/effects/enhancer.toml`), all level-matched: gentle shine, mix polish,
master glow, beat punch, tight kick with a tuned sub, warm vocal, vocal air, bright
acoustic, loud and dense, an 808 under the kick, bass on small speakers, octave sub, root
bloom.

## Export

*Cantus Nitor*, CLAP `com.github.lalmazan.cantus.nitor`, VST3 class id
`46395C6F-19D2E9B4-7D15652E-590ED7C6`; bit-identical to the DSP (`cantus-plugins/tests`).
