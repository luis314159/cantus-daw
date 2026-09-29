<p align="center">
  <img src="crates/cantus-ui/assets/logos/02.svg" width="128" alt="Cantus logo">
</p>

<h1 align="center">Cantus</h1>

<p align="center">
  A free, open-source digital audio workstation, written in Rust and drawn by hand.<br>
  Created by <b>Luis Ángel Almazán</b> · <a href="https://lalmazan.com">lalmazan.com</a>
</p>

<p align="center">
  <img src="docs/images/window-slate.png" alt="Cantus: arrangement, mixer and inspector in the Slate theme">
</p>

> *"Whatever you do, work at it with all your heart."* — Colossians 3:23

Cantus takes its name from the *cantus firmus*: the fixed melody on which polyphony is built.
The engine is that fixed line — deterministic, sample-accurate, allocation-free — and
everything else is built on top of it. The full vision lives in [idea.md](idea.md); the
decisions that shape the code live in [docs/adr/](docs/adr/); where the project stands right
now is in [docs/STATUS.md](docs/STATUS.md).

## Highlights

- **A look drawn by hand, FL Studio first** (ADR 0024). Every knob, fader, meter, slot and
  window frame is painted by Cantus's own vector kit — one "lit from above" recipe, crisp at
  any zoom from 75 % to 200 % and on 4K. No stock widget is visible.
- **As configurable as you want it.** Dark, light or follow the system; hue, saturation,
  brightness, contrast, depth, roundness, density and text size; 61 named colours you can
  search, reset, compare A/B, save and share — or click any element of the interface and
  recolour it on the spot.
- **An FL-style mixer** that fits first and then scrolls, with a legend column, three strip
  widths, docks, sends, groups, submixes and a per-channel sheet in the inspector.
- **Seven effects and eight instruments of its own**, researched against the best on the
  market and measured: EQ, compressor, limiter, reverb, multiband, de-esser, gate; wavetable,
  analog, FM, sampler, bass, organ, electric piano and a drum machine.
- **Works with the market's DAWs.** Hosts **CLAP and VST3** plug-ins; ships its own effects as
  **CLAP/VST3** for any other DAW, bit-identical to the native ones; **MIDI files**, **stems**
  and **DAWproject** in and out.
- **Fast, and tested to stay fast.** A 64-strip mixer frame in about 3 ms, nothing redrawn
  when nothing moves, an audio thread that never allocates, and 1 100+ tests — null tests
  that hold offline renders bit-identical to playback, golden images that hold the look, and
  a frame budget that fails the build when the UI gets slow.

## A tour

### The workspace

Panels dock, split, float and remember their size; every splitter resets on right-click.
F5 / F6 / F7 / F9 bring up the arrangement, channel rack, piano roll and mixer, the hint bar
names whatever is under the pointer, and a value pill follows every drag.

| Slate (dark) | Paper (light) |
|---|---|
| ![Workspace, Slate](docs/images/window-slate-720.png) | ![Workspace, Paper](docs/images/window-paper-720.png) |

### Make it yours

<p>
  <img src="docs/images/appearance.png" width="49%" alt="Appearance settings">
  <img src="docs/images/theme-variations.png" width="49%" alt="The same mixer in four themes">
</p>

Pick an element on screen and recolour it; choose one of ten logos, in its own colours or
tinted by your theme; the splash shows your logo while Cantus loads.

<p>
  <img src="docs/images/pick-element.png" width="49%" alt="Pick an element to recolour it">
  <img src="docs/images/logos.png" width="49%" alt="The ten logos">
</p>

### Mixer and inspector

<p>
  <img src="docs/images/mixer.png" width="70%" alt="The mixer">
  <img src="docs/images/inspector.png" width="26%" alt="The channel inspector">
</p>

The desk follows FL's strip: a colour number, the name, meter, mute and solo lamps, pan,
fader, phase and swap, the FX lamp and sends — rows that line up across the desk, named once
in a legend column. The inspector shows the selected channel as a signal-flow sheet: inserts
as tiles that open a mini view, the channel EQ you drag, sends, routing and delay
compensation, with the channel's own fader pinned at the bottom.

### Arrangement, piano roll, channel rack

| | |
|---|---|
| ![Arrangement](docs/images/arrangement.png) | ![Piano roll](docs/images/pianoroll.png) |
| ![Channel rack](docs/images/rack.png) | Scroll both ways, zoom by dragging a scrollbar's ends, Shift+wheel for time, Ctrl+wheel to zoom, middle-drag to pan. The piano roll holds 20 000 notes on screen within its frame budget. |

### Effects

| | | |
|---|---|---|
| ![Aequalis](docs/images/fx-eq.png) **Aequalis** — 24-band EQ, zero-latency, natural and linear phase, dynamic bands, EQ match | ![Comprimo](docs/images/fx-comp.png) **Comprimo** — compressor with nine visible detectors, key EQ, auto gain | ![Terminus](docs/images/fx-limiter.png) **Terminus** — true-peak limiter, BS.1770 loudness, delivery profiles |
| ![Spatium](docs/images/fx-space.png) **Spatium** — reverb with decay fitted per frequency and shown in seconds | ![Multiplex](docs/images/fx-multiband.png) **Multiplex** — eight two-sided bands, three crossover modes | ![Sibilus](docs/images/fx-deesser.png) **Sibilus** — a de-esser that tells an *s* from a bright sound |
| ![Porta](docs/images/fx-gate.png) **Porta** — gate and expander with hysteresis, MIDI trigger and hits to MIDI | ![Hosted plug-in](docs/images/fx-hosted.png) **Any CLAP or VST3** — with a generic editor or its own window | |

Every effect is a bit-exact wire at settings that do nothing, and any strip can key any of
them. Design notes and feature matrices: [docs/plugins/](docs/plugins/).

### Instruments

| | | | |
|---|---|---|---|
| ![Unda](docs/images/inst-wavetable.png) **Unda** — wavetable | ![Calor](docs/images/inst-analog.png) **Calor** — analog modelling | ![Modulus](docs/images/inst-fm.png) **Modulus** — 8-operator FM | ![Exemplar](docs/images/inst-sampler.png) **Exemplar** — multisampler |
| ![Gravis](docs/images/inst-bass.png) **Gravis** — 808, synth and string bass | ![Organum](docs/images/inst-organ.png) **Organum** — tonewheel organ | ![Lamina](docs/images/inst-epiano.png) **Lamina** — electric piano | ![Pulsus II](docs/images/inst-pulsus.png) **Pulsus II** — drums |

They share one modulation model — eight macros and a matrix you connect by dragging a
source onto a knob — one frame (Play · Edit · Matrix), and the project's undo.

## Everything it does

<details>
<summary>The full list, area by area</summary>

**Engine.** Real-time audio on WASAPI / CoreAudio / ALSA through `cpal`, with an audio thread
that never allocates, locks or panics (ADR 0005). Offline WAV export bit-identical to
playback. Plug-in delay compensation per routing edge (ADR 0018); the playhead and recording
follow what is heard; a bypassed latent effect keeps its latency (ADR 0022). A recorder on
every strip and the master.

**Instruments** (ADR 0023). Unda (wavetable, additive and granular; imports Serum and Surge
tables), Calor (five self-oscillating zero-delay filters, seeded voice cards), Modulus
(8-operator FM that reads and writes DX7 SysEx), Exemplar (SFZ, Decent Sampler, SF2/SF3,
WAV/AIFF/FLAC/Ogg), Gravis (808, 303-style and string bass), Organum (tonewheels and rotary
speaker), Lamina (tine and reed electric piano), Pulsus II (layers, round robin, 808/909 and
modal voices, 16 choke groups), plus Firmus, the built-in polysynth.

**Editing.** Arrangement with per-track heights, icons, time selection and clip editing
(split, glue, trim, slip, repeat, per-clip mute, colour and name). Piano roll with FL-style
tools, clipboard, quantize, slicing and a velocity lane. A channel rack where a step *is* a
note. MIDI recording with one undo step per take; the computer keyboard as a MIDI
controller; hardware MIDI in.

**Mixer** (ADRs 0014–0016). Strips, buses, sends, solo with connected routes, a three-band
channel EQ, a dry/wet per insert slot, ten slots per strip, multi-selection with ganged
faders, submixes, docks, groups with colours, a pan law per project, width and polarity.

**Effects** (ADRs 0017–0018). The seven above, each with presets, A/B, a display that is the
control, and design notes in `docs/plugins/`.

**Standards** (ADRs 0019–0021). CLAP and VST3 hosting with a crash-proof scanner; our effects
as CLAP/VST3 (`crates/cantus-plugins`); MIDI files, stems and DAWproject.

**Project.** One command bus for every change, with unlimited undo/redo and an audit log
(ADR 0004). Versioned JSON projects with SHA-256 integrity, schema migrations, atomic saves
and autosave (ADR 0006).

**Interface** (ADRs 0007, 0009, 0010, 0024). English and Spanish; every shortcut rebindable
in `keymap.toml`, with presets modelled on FL Studio, Ableton Live and Bitwig; named layouts;
the appearance settings above.

</details>

## Build and run

```
cargo run --release -p cantus                     # build and open Cantus
cargo run --release -p cantus -- --locale es      # in Spanish
cargo run --release -p cantus -- song.cantus      # open a project
cargo run --release -p cantus -- --stress         # 160 000-note stress project
```

**A standalone `.exe`:**

```
cargo build --release -p cantus
```

It is written to `target\release\cantus.exe`, with the Cantus icon; fonts, logos and themes
are inside it, so the file can be copied anywhere and run.

**Our effects as plug-ins for other DAWs:**

```
cargo run -p xtask -- bundle          # target/bundled/Cantus <Name>.clap and .vst3
cargo run -p xtask -- bundle --v3     # a build for modern CPUs (AVX2), 15–25 % faster
```

Copy the `.clap` files to `%LOCALAPPDATA%\Programs\Common\CLAP` and the `.vst3` bundles to
`%LOCALAPPDATA%\Programs\Common\VST3`, then rescan in your DAW.

Requires stable Rust 1.95 or newer. No system libraries are needed on Windows or macOS; on
Linux install `libasound2-dev` (and `libxkbcommon-dev`, `libwayland-dev` for the window).

## Tests

```
cargo test --workspace                          # unit, property, null, golden-image and budget tests
cargo test -p cantus-engine -- --ignored        # real-device smoke test
cargo clippy --workspace --all-targets
```

The null tests in `crates/cantus-engine/tests/null_tests.rs` are the project's most important
guardrail: they fail if the engine ever produces different output for different block sizes,
or if a render stops matching playback, and they hold the line that a control at its default
must not touch a sample. The kit's golden images (`crates/cantus-ui/tests/golden/`) hold the
look, and `tests/frame_budget.rs` holds the speed.

## Layout

| Crate | License | Purpose |
|---|---|---|
| `cantus-core` | MIT OR Apache-2.0 | musical time, project model, commands, undo/redo, snapshots |
| `cantus-dsp` | MIT OR Apache-2.0 | every instrument's and effect's DSP, null-test harness |
| `cantus-engine` | MIT OR Apache-2.0 | audio thread, transport, mixer graph, delay compensation, offline render |
| `cantus-project` | MIT OR Apache-2.0 | project file format, migrations, MIDI files, DAWproject |
| `cantus-host` | GPL-3.0-or-later | hosting CLAP and VST3 plug-ins |
| `cantus-plugins` | GPL-3.0-or-later | Cantus's effects as CLAP and VST3 |
| `cantus-ai` | GPL-3.0-or-later | `AiProvider` trait, tool specs, command bridge |
| `cantus-ui` | GPL-3.0-or-later | the interface, its design system and the hand-drawn kit |
| `cantus` | GPL-3.0-or-later | the binary |

See [ADR 0001](docs/adr/0001-license.md) for why the license is split. The bundled fonts
(IBM Plex Sans, Sans Condensed and Mono, in `crates/cantus-ui/assets/fonts/`) are licensed
under the [SIL Open Font License 1.1](crates/cantus-ui/assets/fonts/LICENSE-OFL.txt) and
shipped unmodified — "IBM Plex" is a Reserved Font Name.

## Author

Cantus is designed and built by **Luis Ángel Almazán** — [lalmazan.com](https://lalmazan.com).
The logos are his own designs.
