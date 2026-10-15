# Flexus — the through-zero flanger

*Effect kind `Flanger`. References: FL Studio's Fruity Flanger and Flangus, the MXR
Flanger and the Electro-Harmonix Electric Mistress, Soundtoys PhaseMistress, Kilohearts
Flanger, and the tape flanging of two machines run side by side (ADR 0055).*

Flexus mixes the signal with a copy read from a line whose delay sweeps: the comb of notches
that moves is the flange. The sweep follows the family's **LFO** (free or synced), the
input's **envelope** (play louder, it sweeps further), or a **manual** position for
automation. **Feedback** in either polarity sharpens the comb; the wet can be **inverted**.
**Through zero**, the dry path is delayed by a fixed 5 ms and the wet sweeps either side of
it, so the moving copy passes *through* the dry one — the deep cancellation at the crossing
that a plain flanger never reaches.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/flexus.rs` |
| DSP | `crates/cantus-dsp/src/flexus.rs` (tests in `flexus_tests.rs`); the family's line, clock and LFO |
| Engine glue | `crates/cantus-engine/src/fx/flexus.rs` (reports the through-zero latency) |
| Editor | `crates/cantus-ui/src/views/fx/flexus.rs` |
| Strings | `crates/cantus-ui/locales/fx/flexus.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_flexus.rs`, `crates/cantus-ui/tests/fx_flexus.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Flexus` (CLAP `com.github.lalmazan.cantus.flexus`, VST3 `88EF5F4F7F8034B7687D995D3E5912AE`) |

## What the references do well, and what people complain about

- Pedals are loved for their sound and hated for their noise; plug-ins are clean but most
  cannot go **through zero**, and those that do rarely say what latency it costs.
- Users want the sweep **in time** with the song, an **envelope** mode for funk guitar and
  drums, and a way to **park** the comb (manual) for automation.
- Few show anything but an LFO; none shows the comb itself.

Flexus answers each: through zero with its latency reported to delay compensation (5 ms,
shown in the footer), synced and envelope sweeps, a manual position, and two pictures —
the wet's delay against the dry over two cycles (the dry drawn as the line the wet
crosses), and **the comb now**, computed with the unit's own arithmetic (the Hermite read
and the feedback loop), so the notches you see are the notches you hear (the tests measure
sines through the unit against it, within 0.6 dB).

## Parameters (keys are forever)

The mix starts at 0, so Flexus inserted and left alone returns its input to the bit.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `mode` | 0 … 2 | LFO | what moves the sweep: LFO, Envelope, Manual |
| `tz` | off / on | off | through zero: the dry delayed by 5 ms (the latency), the wet crossing it |
| `mix` | 0 … 100 % | 0 | equal-gain crossfade, both full at 50 % |
| `delay` | 0.05 … 5 ms | 0.5 ms | the sweep's shortest delay (plain mode) |
| `depth` | 0 … 10 ms | 4 ms | how far it sweeps (through zero: ± half, up to the 5 ms centre) |
| `fb` | −95 … 95 % | 0 | feedback, either polarity |
| `invert` | off / on | off | the wet upside down |
| `sync`, `div`, `rate` | | free, 2/1, 0.25 Hz | the LFO (0.02 … 10 Hz free) |
| `shape` | Sine / Triangle | Triangle | |
| `spread` | 0 … 180° | 0 | the right channel's LFO ahead of the left |
| `manual` | 0 … 100 % | 50 % | the sweep's position in manual mode |
| `att`, `rel` | 0.5 … 100 ms, 10 … 2 000 ms | 5, 200 ms | the envelope's attack and release |
| `sens` | 0 … 48 dB | 24 dB | the envelope's gain into the sweep |
| `out` | −24 … +24 dB | 0 | output gain |

## How it works

- Plain: wet delay `delay + depth·(m + 1)/2` for the sweep `m` in −1 … 1. Through zero: the
  dry from a line at the fixed centre `C` (5 ms, whole samples), the wet at
  `C + min(depth/2, C − 1)·m`.
- **The centre null**: at `m = 0` the wet is read at exactly `C` samples — the sample the dry
  line holds — so with the wet inverted at 50 % the two cancel to exact zeros. The DSP test,
  the engine test and the editor's comb all hold it.
- Feedback: the last wet sample, times `fb`, joins the input of the swept line; after a
  silence what is left below 10⁻²⁰ is flushed, so heavy feedback still comes to rest.
- The envelope is a peak follower (attack, release) on both channels; it runs in every
  mode so a switch finds it in step.
- **Exact rest**: the lines empty, the feedback and the envelope at zero, the glides
  landed.

## Factory presets

Jet plane · Tape, through zero · Electric mistress · Negative metal · Sweep on the bar ·
Drum bus swirl · Envelope sweep · Hand-set comb.

## What the tests hold

- At its defaults the render is the render without it, to the bit; through zero at mix 0,
  the render later by exactly the 240 samples it reports; the centre point silences the
  strip to the bit across the whole engine; the same bits at blocks of 1, 7, 64, 1 000 and
  4 096 in every mode; live equal to offline (`fx_flexus.rs`).
- The DSP: the centre null to the bit and no null a hair off it; the wet passes ahead of
  and behind the dry; feedback ±95 % bounded and distinct; the envelope sweeps further when
  louder; split invariance in every mode; rest and `skip` exact with 95 % feedback.
- The editor: the comb drawn is the comb played (sines through a held sweep, plain and
  through zero, with feedback); the centre point drawn as silence; its cards; every preset
  in both languages.
- The family's contract and the CLAP/VST3 export, bit-identical, latency reported, mix
  automated.
