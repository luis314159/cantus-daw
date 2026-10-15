# Impetus — the transient processor

*Effect kind `Transient`. References: SPL Transient Designer, FL Studio's Transient
Processor, Logic's Enveloper (ADR 0056).*

Impetus makes the onsets of a sound louder or softer (**attack**) and its tails longer or
shorter (**sustain**), **from level differences, not a threshold**: it compares a fast
envelope with a slow one, so a quiet hit and a loud one are shaped alike and there is no
level to set. A **split band** shapes only the highs (snap and air, the low end left alone)
or only the lows (a kick's thump), and a **sidechain** lets another strip's hits drive it —
chosen in the editor's *Detect from* field (any strip of the desk; in another DAW, its
sidechain input).

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/impetus.rs` |
| DSP | `crates/cantus-dsp/src/impetus.rs` (tests in `impetus_tests.rs`); the detector in `dynfx_transient.rs`, the follower in `dynfx_follow.rs`, the meter in `dynfx_meter.rs` |
| Engine glue | `crates/cantus-engine/src/fx/impetus.rs` |
| Editor | `crates/cantus-ui/src/views/fx/impetus.rs`, the family's parts in `dynfx.rs` |
| Strings | `crates/cantus-ui/locales/fx/impetus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_impetus.rs`, `crates/cantus-ui/tests/fx_impetus.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Impetus` (CLAP `com.github.lalmazan.cantus.impetus`, VST3 `4A617E6B08572AE48163D60CA326A3E0`) |

## What the references do well, and what people complain about

- **SPL Transient Designer** started it: two knobs, no threshold — loved for exactly that.
  Its limits: full band only, no sidechain, nothing to see.
- **FL's Transient Processor** adds a split frequency and a gain trace, but works on a
  threshold-like *drive* and its trace is hard to read.
- What users ask for: no threshold, a split band so the low end does not pump, a sidechain,
  no distortion of steady bass notes, and a picture of what it does to a hit.

## Parameters (keys are forever)

At 0 % attack and 0 % sustain (the defaults) the gain is exactly 1: the sound passes
untouched, whatever the band, the times or the key.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `attack` | −100 … +100 % | 0 | the onsets' gain: at +100 % their rise is doubled, at −100 % flattened |
| `sustain` | −100 … +100 % | 0 | the tails' gain |
| `speed` | 1 … 50 ms | 15 ms | how long an onset lasts (the slow envelope's attack) |
| `length` | 20 … 1 000 ms | 250 ms | how long a tail is heard as one (the long release) |
| `band` | Full, Highs, Lows | Full | what is detected and shaped |
| `xover` | 40 … 8 000 Hz | 200 Hz | the split band's crossover |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

The rectified level (the louder channel, or the key's) goes through a **running maximum**
over 10 ms — two half-cycles of a 50 Hz tone — so a steady tone is a steady level and the
gain never wobbles at the waveform's rate (a 55 Hz tone reads < 0.05 dB of attack). Two
pairs of one-pole envelopes follow it (`dynfx_follow`): the *attack pair* shares a release
and differs in attack (0.3 ms against `speed`), the *sustain pair* shares the fast attack and
differs in release (`length`/8 against `length`). Their ratios in dB, clamped to 24 dB, are
how much of the sound is an onset and how much a tail; the gain is
`attack × onset + sustain × tail`, clamped to ±24 dB. Everything is a ratio, so the reading
is the same at any level — to the bit for a power-of-two change, which a test holds.

**Split band**: a Butterworth state-variable filter's high- (or low-) pass is the band; the
output is `x + (g − 1)·band`. Band and rest are 90° apart at the crossover, so a boost or a
cut never cancels there. **No latency**; the filters run only in a split mode and are
emptied when it ends; after 64 silent samples small states are flushed, so it rests exactly.

**The editor** shows a synthetic hit run through the real DSP at the current settings (in
and out), and the last three seconds column by column — 512 samples a column, counted from
the stream and carried in the telemetry as a ring of sixteen with a running count, so the
editor misses none however its frames fall.

## Measured

- Null at 0 % in every band, with and without a key, through the engine; block sizes 1 …
  4 096 identical; live = offline.
- Attack +100 %: the first 5 ms of each hit **+14.0 dB** over the body; −100 %: **−12.4 dB**.
  Sustain +100 %: the tail **+12.0 dB**; −100 %: **−9.8 dB**.
- Split band at 800 Hz, attack +100 % over hats and a 55 Hz bass: the bass moves **−22.5 dB**
  in full band, **−43.4 dB** with highs only.

## Presets

More punch, Snare snap, Soften the attack, Tight drums, Bring out the room, Dry it up, Kick
thump, Plucked strings.
