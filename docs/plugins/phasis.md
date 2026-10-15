# Phasis — the phaser

*Effect kind `Phaser`. References: the MXR Phase 90 (script and block versions), the
Electro-Harmonix Small Stone, FL Studio's Fruity Phaser, Soundtoys PhaseMistress,
Kilohearts Phaser (ADR 0055).*

Phasis sends the signal through a cascade of first-order allpass stages tuned together and
mixes it with the dry: wherever the cascade has turned the phase by an odd multiple of 180°
the two cancel, so every two stages make a notch, and the LFO moves the notches between a
low and a high bound on a log scale. **2 … 12 stages**, **feedback** of either polarity
round the cascade (the pedals' resonance), the right channel's sweep apart from the left's,
and the family's LFO with all six shapes (a stepped random sweep included).

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/phasis.rs` |
| DSP | `crates/cantus-dsp/src/phasis.rs` (tests in `phasis_tests.rs`); the TPT allpass in `modfx_line.rs` |
| Engine glue | `crates/cantus-engine/src/fx/phasis.rs` |
| Editor | `crates/cantus-ui/src/views/fx/phasis.rs` |
| Strings | `crates/cantus-ui/locales/fx/phasis.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_phasis.rs`, `crates/cantus-ui/tests/fx_phasis.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Phasis` (CLAP `com.github.lalmazan.cantus.phasis`) |

## What the references do well, and what people complain about

- The **Phase 90** is loved for its simplicity (one knob) and its throaty four-stage sweep;
  the "script" reissue drops the feedback resistor the "block" version keeps.
- The **Small Stone**'s colour switch adds resonance; it can whistle.
- Plug-in phasers offer more stages and sync, but rarely show where the notches are, and
  their resonance at high feedback can run away.

Phasis keeps the pedals as presets (four stages, their sweep ranges, script/block, colour),
goes to twelve stages, syncs to the song, keeps feedback bounded (±95 %), and draws both the
sweep and **the response now** — computed with the unit's own arithmetic, measured against
it in the tests.

## Parameters (keys are forever)

The mix starts at 0, so Phasis inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `stages` | 2 … 12 | 4 | first-order allpass stages: a notch for every two |
| `mix` | 0 … 100 % | 0 | equal-gain crossfade; 50 % is the classic phaser (both full) |
| `lo` | 20 … 2 000 Hz | 200 Hz | the sweep's low bound |
| `hi` | 500 … 16 000 Hz | 2 kHz | the sweep's high bound |
| `fb` | −95 … 95 % | 0 | feedback round the cascade |
| `sync`, `div`, `rate` | | free, 1/2, 0.5 Hz | the LFO (0.02 … 10 Hz free) |
| `shape` | 0 … 5 | Sine | the family's six shapes |
| `edge` | 0 … 100 % | 30 % | the stepped shapes' soft edge |
| `spread` | 0 … 180° | 0 | the right channel's LFO ahead of the left |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- Each stage is Zavalishin's TPT one-pole turned allpass, `y = 2·lp − x`, so its digital
  response is `(z⁻¹ − c)/(1 − c·z⁻¹)` with `c = (1 − g)/(1 + g)`, `g = tan(π·fc/fs)`: the
  analog stage's phase `−2·atan(f/fc)`, prewarped.
- The stages' frequency is `lo·(hi/lo)^((m+1)/2)`; the bounds glide on a log scale, so
  turning them never clicks.
- Feedback takes the cascade's output one sample later back to its input.
- **Exact rest**: the stages' states and the feedback flushed after a silence, the glides
  landed.

## Factory presets

Phase 90, script · Phase 90, block · Small Stone, colour on · Twelve stages, deep · Eighths ·
Stereo swirl · Random steps · Slow wash.

## What the tests hold

- At its defaults and at mix 0 the render is the render without it, to the bit; working,
  the same bits at blocks of 1, 7, 64, 1 000 and 4 096; live equal to offline
  (`fx_phasis.rs`).
- The DSP: for 2, 4, 6 and 12 stages, a notch below −30 dB at every frequency where the
  prewarped phase reaches an odd multiple of 180°, and +6 dB where it makes a whole circle;
  feedback raises the peaks and stays bounded at ±95 %; the sweep reaches its bounds; split
  invariance; rest and `skip` exact.
- The editor: the response drawn is the response played (sines through a held sweep, three
  stage counts with feedback, within 0.3 dB); its cards; every preset in both languages.
- The family's contract and the CLAP/VST3 export, bit-identical, mix automated.
