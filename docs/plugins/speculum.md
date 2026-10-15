# Speculum — the analyser

*Effect kind `Analyzer`. References: FL Studio's Wave Candy, Voxengo SPAN, iZotope Insight,
MiniMeters (ADR 0056).*

Speculum shows what is in a sound — its **spectrum**, a **spectrogram**, the **waveform**
(triggered or synced to the beat), a **vectorscope** with the **phase correlation**, and
**loudness** to EBU R128 (momentary, short-term and integrated LUFS, loudness range, true
peak) with **peak and RMS** per channel — and it never touches it: a pure pass-through,
whatever it is set to. It floats like any editor, **docks** in the workspace as a tab
(resizable, split, torn out as a window), and several can be watched at once.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/speculum.rs` |
| Measurements | `crates/cantus-dsp/src/speculum.rs` (tests in `speculum_tests.rs`); loudness is `terminus::loudness` |
| Engine | `crates/cantus-engine/src/fx/speculum.rs` (the pass-through, the scope rings' types), the copy in `graph/strip_run.rs::scope` (strips and master), `EngineHandle::{watch_scope, drain_scope, release_unwatched_scopes}` |
| Editor | `crates/cantus-ui/src/views/fx/speculum.rs`, `speculum_draw.rs`; `FxHost::take_stereo` |
| Strings | `crates/cantus-ui/locales/fx/speculum.{en,es}.txt` |
| Tests | `crates/cantus-engine/tests/fx_speculum.rs`, `crates/cantus-ui/tests/{fx_speculum,speculum_budget}.rs`, the family's contract and CLAP/VST3 tests |
| Exported | `Cantus Speculum` (CLAP `com.github.lalmazan.cantus.speculum`, VST3 `CBA1953DC34A0CF35419D4DA1196A31A`) |

## What the references do well, and what people complain about

- **Wave Candy** is FL's: an oscilloscope, a spectrum, a spectrogram and a vectorscope in
  one, beat-synced; its spectrum has no averaging or slope worth the name and it has no
  loudness.
- **SPAN** is the mixers' spectrum: averaging, slope (4.5 dB an octave), peak hold, mid/side;
  its loudness is a side panel.
- **Insight** and **MiniMeters** are the loudness and phase people's: R128 readings, a
  loudness history, a correlation meter, resizable panels.
- What users ask for: all of it in one place, readings they can trust (the standards' test
  signals), several analysers at once without one stealing the other's feed, no dropped
  frames, and no cost to the sound.

## Parameters (keys are forever)

None touches a sample: they are how Speculum is looked at, kept with the project.

| Key | Range | Default | Meaning |
|---|---|---|---|
| `view` | Overview, Spectrum, Spectrogram, Scope, Vector, Loudness | Overview | the tab |
| `fft` | 1k … 16k | 8k | FFT size |
| `avg` | 0 … 3 000 ms | 250 ms | the spectrum's averaging time constant (0: none) |
| `slope` | 0 … 6 dB/oct | 4.5 | tilt around 1 kHz |
| `hold` | Off / On | On | the spectrum's peak hold (falling 6 dB a second) |
| `chan` | Mid, Side, Left, Right, L/R | Mid | what the spectrum reads (L/R: left and right as two curves, not their sum) |
| `floor` | −72, −96, −120 dB | −96 | the display's floor |
| `sync` | Free, Trigger, Beat | Beat | the oscilloscope's window |
| `beats` | 1/4 … 8 beats | 1 beat | the synced window |
| `ms` | 1 … 500 ms | 20 ms | the free and triggered window |
| `target` | −30 … −5 LUFS | −14 | the loudness marked on the readings and the history |

## How it works

**Every frame, several at once.** The engine keeps eight **scope rings** (lock-free, 65 536
frames each, 1.4 s at 48 kHz). A Speculum being looked at is given one: after each pass the
strip (or the master, even with its chain off or bypassed) copies the frames at its slot into
the ring in chunks of 64 filled across passes (so the capacity is in frames, however short the
passes), each carrying its owner (a reused ring drops the last one's), the frames a full ring
lost before it (shown as *Gaps* under the loudness), and the song's position at its first
frame as heard at the slot (the transport's less the compensation the slot waits) — eight comparisons for an insert
nobody watches, no allocation, nothing ever waits. The editor asks for its ring every frame
it is shown and drains it; a ring nobody asked for during a frame is let go. In the exported
plug-in the editor's tap carries the frames themselves (left, right) instead of the
input/output pair the other editors read.

**Measured on the editor's thread** (`cantus_dsp::speculum::Analysis`), counted in samples of
the stream — never in screen frames — so the readings do not depend on how often it is
drawn:

- **Loudness** is the house meter (`terminus::loudness`, BS.1770-4 / R128): momentary and
  short-term as they come; integrated and range **worked out every 200 ms of audio** and
  held between; the largest true peak; *Reset* starts them over.
- **Spectrum**: a Hann-windowed FFT every quarter of its length, scaled so a full-scale sine
  reads 0 dB; an exponential average and a peak hold per bin; the slope; a display column
  that spans many bins shows their loudest, a sine between bins reads its level from a
  parabola through the three at the top.
- **Spectrogram**: a row of 128 log-spaced bands (20 Hz–20 kHz) per analysis, 256 rows,
  drawn as one texture.
- **Oscilloscope**: five seconds of the mid signal. *On the beat*, the window is the last
  whole stretch of `beats` beats by the stamped song position (a rising zero crossing while
  the song is stopped); *Trigger*, from the latest rising zero crossing; *Free*, the last
  `ms`.
- **Vectorscope**: the last 4 096 frames as mid (up) and side (across); **correlation**
  over 300 ms; **RMS** over 300 ms; **peak** held for a second.

A sample that is not a number is read as silence: nothing it touches is poisoned.

## Measured

- **EBU Tech 3341** (1 kHz, stereo): cases 1 and 2 read M, S and I at −23.0 and −33.0 LUFS
  (±0.1); cases 3, 4 and 5 read I = −23.0 (±0.1). **EBU Tech 3342**: two levels in turn
  read LRA 10, 5 and 20 LU (±1).
- A −12 dBFS sine at 1 kHz reads −12 dB (±0.6) at every FFT size; an octave away, under −60.
- A pass-through: through the whole engine, Speculum set to the maximum of every parameter
  renders bit-identically to no Speculum.
- **No frame lost**: in the real window at 60 Hz with three Speculums at once (one floating,
  two docked), each measured exactly the 800 frames the engine played per frame; the copy
  on the audio thread allocates nothing. The three add about 13 000 triangles and no draw
  call to the window (65 206 / 91 against 52 038 / 91).
- Timed (release, `speculum_budget -- --ignored`): the whole 1080p window with the three, build + tessellate, **median 1.30 ms, 95th percentile 1.84 ms** (the view budget is 8 ms, a 60-Hz frame 16.7 ms).

## Presets

Mixing: spectrum and phase; Mastering: loudness, −14 LUFS; Broadcast: R128, −23 LUFS;
Low end: 16k FFT, left and right; Drums: scope on the beat; Stereo image.
