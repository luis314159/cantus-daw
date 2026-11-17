# Fragor — the pedals

*Effect kind `Pedal`. References: the Ibanez TS808 Tube Screamer, the ProCo RAT, the Boss
DS-1, the Dallas Arbiter Fuzz Face, the Electro-Harmonix Big Muff π (their published
schematics); among plug-ins, Logic's Pedalboard, FL Studio's Hardcore, and circuit-modelled
pedal plug-ins (ADR 0128).*

Fragor (Latin, *a crash, a roar*) is a pedalboard: up to three distortion and fuzz pedals in
any order, each modelled from its circuit, a noise gate before them, an input, a mix and an
output, mono or stereo, oversampled 2×, 4× or 8×.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/fragor.rs` |
| DSP | `crates/cantus-dsp/src/fragor.rs` (the board), `fragor_dk.rs` (the circuit solver), `fragor_circuits.rs` (the five pedals, every component value), `fragor_gate.rs`; tests in `fragor_tests.rs`, `fragor_spice_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/fragor.rs` |
| Editor | `crates/cantus-ui/src/views/fx/fragor.rs`, `fragor_view.rs`; the reusable drawings `pedal_draw.rs` (pedals, cables, slots) and `valve_draw.rs` (tubes, grille, jewel lights); finishes in `kit/skin/pedals.rs` |
| Strings | `crates/cantus-ui/locales/fx/fragor.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/pedal.toml` (fifteen) |
| Tests | `crates/cantus-engine/tests/fx_fragor.rs`, `crates/cantus-ui/tests/fragor_e2e.rs`, the family's contract (`fx_contract.rs`), the plug-in tests |
| Exported | `Cantus Fragor` (CLAP/VST3), bit-identical to the engine |

## What the references do well, and what people complain about

- The best circuit-modelled pedals sound like the pedal at every knob position because the
  knobs move the circuit; curve-based "drive" plug-ins sound right at one setting only.
- Board plug-ins (Logic's Pedalboard, FL's Hardcore) make chains easy to see and reorder —
  but often hide their models' character behind a generic waveshaper, and few show what the
  chain does to the sound.
- People complain about aliasing (fizz on high notes), about latency that jumps when a pedal
  is switched, and about CPU.

## What Fragor does

- **Circuits, solved.** Each pedal is its schematic: resistors, capacitors, op-amps (with the
  RAT's LM308 slew rate), diodes and transistors (Ebers–Moll), discretised by the
  trapezoidal rule and solved each oversampled sample by the DK method with Newton's
  iterations on the nonlinear ports only. Every small-signal response matches an independent
  SPICE-class analysis of the whole schematic within 0.1 dB (the Big Muff within 0.08 dB
  below 1 kHz).
- **The five pedals:** *Green overdrive* (TS808: diodes in the op-amp's loop, the mid hump),
  *Op-amp distortion* (RAT: a slow op-amp into diodes to ground, the Filter), *Silicon
  distortion* (DS-1: a transistor boost, hard clipping, the scooped tone), *Germanium fuzz*
  (Fuzz Face: two AC128s; its **Bias** is the battery — turned down, the transistors starve
  and a fading note sputters and gates), *Triangle muff* (Big Muff π V3: two clipping
  stages, the mid scoop).
- **The board.** Three slots, any order; drag a pedal to another slot (they swap), click an
  empty one to add, right-click to replace or remove. A footswitch crossfades in 10 ms and
  never changes the latency. The gate closes on what falls below its threshold before the
  pedals hear it.
- **Oversampling** 2×, 4× (default) or 8×: what folds back is −30, −54 and −63 dB under the
  harmonics for a 2.5 kHz note through the op-amp distortion at 80 %.
- **The display:** one cycle of the note going in and coming out at the level playing, and
  its harmonics — odd solid, even hatched, the dominant ones marked — with the total
  distortion.
- **Nulls and rest.** An empty board is a wire to the bit; pedals all switched off are the
  input delayed by the latency the host compensates; a second of silence returns every
  circuit exactly to its operating point and costs nothing after.

## Parameters

| Key | What | Range | Default |
|---|---|---|---|
| `s1`, `s2`, `s3` | the slots, in signal order: 0 empty, 1 green overdrive, 2 op-amp distortion, 3 silicon distortion, 4 germanium fuzz, 5 triangle muff | 0 … 5 | empty |
| `od.on` … `mf.on` | each pedal's footswitch | 0 / 1 | on |
| `od.drive`, `od.tone`, `od.level` | Green overdrive | 0 … 100 % | 50 % |
| `op.dist`, `op.filter`, `op.vol` | Op-amp distortion | 0 … 100 % | 50 % |
| `si.dist`, `si.tone`, `si.level` | Silicon distortion | 0 … 100 % | 50 % |
| `ge.fuzz`, `ge.bias`, `ge.vol` | Germanium fuzz (Bias 100 % a fresh battery) | 0 … 100 % | 50, 100, 50 % |
| `mf.sustain`, `mf.tone`, `mf.vol` | Triangle muff | 0 … 100 % | 50 % |
| `gate` | gate threshold; −90 is off | −90 … −20 dB | off |
| `in` | level into the pedals | −24 … 24 dB | 0 |
| `mix` | dry/wet | 0 … 100 % | 100 % |
| `mono` | one circuit for both channels | 0 / 1 | stereo |
| `os` | oversampling 2×, 4×, 8× | 0 … 2 | 4× |
| `out` | output | −24 … 24 dB | 0 |

Inserted from the picker it starts with the green overdrive on the board (*Green push*).
