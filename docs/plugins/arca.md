# Arca — amp and cabinet

*Effect kind `Amp`. References: amp simulators of the circuit-modelling kind (Neural DSP,
Line 6 Helix, Kemper's profiles, Positive Grid), impulse-response loaders, FL Studio's
Fruity Fast Dist for the opposite end (ADR 0134).*

*Arca* is Latin for the chest. Each amp is its schematic solved as a circuit, valve by valve
(Koren's triodes and pentodes, grid current, coupling and cathode networks, the tone stack
from its component values, a push-pull power amp with its supply's sag), oversampled; the
cabinet is an impulse response — one of Arca's own five, made from the microphone's kind,
place and distance, or the user's file.

| Amp | Family | Power |
|---|---|---|
| **Clean** | an American blackface-style clean channel | 2 × 6V6GT, 22 W, valve rectifier |
| **Crunch** | a British plexi-style bright channel | 2 × EL34, 50 W, solid state |
| **Chime** | a top-boost class A | 4 × EL84, 30 W, cathode bias, the presence works as a cut |
| **High gain** | a four-stage lead channel | 4 × 6L6GC, 100 W |
| **Bass** | a tweed bass amp's normal channel | 2 × 5881, 45 W |

**The null**: no amp and no cabinet (the defaults) — the signal untouched, to the bit, no
latency. An amp's latency is its oversampler's (2×, 4×, 8×: 1.0, 1.1, 1.2 ms at 48 kHz),
the dry delayed to match; the cabinet alone has none.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/arca.rs` (15 keys; the null: no amp, no cabinet) |
| DSP | `crates/cantus-dsp/src/arca.rs` and `arca_*.rs` (models, circuits, valves, power amp, cabinets); tests `arca_tests.rs`, `arca_spice_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/arca.rs` (settings, the cabinet asked for by value and built off the audio thread, the telemetry: each valve's work, the supply, the oversampling) |
| Editor | `crates/cantus-ui/src/views/fx/arca.rs`, `arca_display.rs` (the chassis), `arca_valve.rs` (valves, transformers, capacitors, back plate), `arca_plate.rs` (the faceplate), `arca_cab.rs` (the cabinet), `arca_words.rs`, `arca_art.rs` (its motif on Tabula) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`ARCA`: hammered green enamel, copper and cream; Graphite and Oxblood colourways); objects' colours `kit/skin/cabinets.rs` |
| Strings | `crates/cantus-ui/locales/fx/arca.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/amp.toml` (sixteen); starts on *Clean combo* |
| Tests | `crates/cantus-engine/tests/fx_arca.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/arca_e2e.rs`, `arca_rota_drag.rs`, `arca_rota_whole.rs`; the CLAP/VST3 suites |
| Exported | `Cantus Arca` (CLAP and VST3) |

## The window

- **The chassis** (dark in every theme): the amps as lit legends; the oversampling with each
  choice's latency; the back plate; the preamp's valves and the inverter at the back, the
  power valves, filter capacitors and transformers at the front — each valve's heater lit
  once an amp is chosen, its plate reddening with the work it does (the telemetry: each
  circuit's swing, the inverter's drive, the power valves' approach to grid current), a bar
  under each; the amp's character, power valves, bias and rectifier; the supply's needle
  sagging below idle.
- **The faceplate**: the jewel pilot lamp, two inputs, Preamp · Tone stack · Power amp ·
  Output, each knob in a ring of ticks; without an amp its knobs stay usable, dimmed.
- **The cabinet**: the list (none, five, the loaded file — choosing it with no file opens
  the file dialog), the microphone's kind, the cabinet's front, the cone with the
  microphone on its boom — **dragged from the cap to the edge** — the side view —
  **dragged back from the grille** (1–30 cm) — and the designed response.

## On Tabula

A wide pedal in its own hammered enamel; the motif a valve before a speaker's cone; knobs
Gain, Bass, Treble, Master.

## Ours, better

- Circuits, not curves: the operating points within 0.05 V of a SPICE solution of each
  whole preamp, its small-signal response within 0.22 dB from 80 Hz to 8 kHz.
- The window shows the amp working — which valve clips, how far the supply sags — where
  most amp simulators show only a photograph.
- The microphone is moved on the cone and back from the grille with the response drawn as
  it moves; your own impulse response is a cabinet like the others.
