# The fx side round — `fragor`: Fragor, the pedals (ADR 0131, provisional)

**Phase A (checkpoint, 2026-11-13):** the DSP and its tests, the parameter table, the engine
unit, the design sheet and the listening examples. Phase B (after the owner's verdict on the
design): the full editor, presets, the end-to-end test, the plug-in export, docs, manual,
site card, AI description, demos, the finishing suite.

## Done in Phase A

| Layer | Files (owned) | Shared files (lines marked `// fx:fragor`) |
|---|---|---|
| DSP | `cantus-dsp/src/fragor.rs` (the board), `fragor_dk.rs` (the DK circuit solver), `fragor_circuits.rs` (the five pedals), `fragor_gate.rs`, tests `fragor_tests.rs`, `fragor_spice_tests.rs`, example `examples/fragor_sounds.rs` | — |
| Model | `cantus-core/src/fx/fragor.rs` (the table) | — |
| Engine | `cantus-engine/src/fx/fragor.rs`, `tests/fx_fragor.rs` | `tests/support/heavy.rs` (Fragor's heavy settings) |
| UI | `views/fx/fragor.rs`, `fragor_view.rs`, `pedal_draw.rs` (reusable), `kit/skin/pedals.rs` (finishes), locales `fx/fragor.{en,es}.txt`, `examples/shoot_fragor.rs` | `views/fx/mod.rs` (`pub mod pedal_draw`), `views/fx/frame.rs` (designed size; hue Tone), `views/inspector_mini.rs` (hue Tone), `kit/skin.rs` (`pub mod pedals`), `kit/skin/side.rs` (Fragor's own face) |
| Plug-ins | — | `tests/golden/ids.txt` (Fragor's new ids appended, `CANTUS_BLESS_IDS=1`) |
| Docs | `docs/adr/0131-fragor-the-pedals.md`, this file | — |

The measurements and what each test holds are in the ADR.

## Measured in Phase A

- **Against the schematics** (an independent SPICE-class analysis of each whole circuit,
  `fragor_spice_tests`): TS808, RAT, DS-1, Fuzz Face within 0.1 dB from 80 Hz to 3 kHz
  and 0.25 dB at 8 kHz; Big Muff within 0.08 dB to 1 kHz, 0.5 dB at 8 kHz.
- **Published operating points:** Fuzz Face Q2 collector 4.7 V, Q1 0.7 V (ElectroSmash's
  targets 4.5 V and 0.5 – 0.7 V); Big Muff input booster collector 7.0 V (ElectroSmash 7 V).
- **Aliasing** (2.5 kHz at 300 mV, op-amp distortion at 80 %, 48 kHz): −30.0 dB at 2×,
  −53.6 dB at 4× (default), −63.1 dB at 8×.
- **The dying battery:** fresh, a decaying note's output holds within 1 dB while its input
  falls 24 dB; at Bias 10 % it collapses 20 dB faster than the input (the sweep in
  `compartir/fragor/sound/07-…`: the note's end 12 dB under its start when fresh, 45 – 70 dB
  under it at Bias 35 % and below).
- **CPU per pedal**, mono, 48 kHz, release build, a plucked note (`fragor_tests::cpu_per_pedal`,
  `--ignored`), measured **while other builders kept this machine at 100 % load** — read
  them as upper bounds; to be measured again on a quiet machine:

  | Pedal | 2× | 4× (default) | 8× |
  |---|---|---|---|
  | Green overdrive (1 port) | 2.8 % | 4.5 % | 8.1 % |
  | Op-amp distortion (2) | 4.0 % | 6.5 % | 11.5 % |
  | Silicon distortion (2 ‖ 2) | 9.7 % | 13.7 % | 21.6 % |
  | Germanium fuzz (4) | 5.6 % | 9.7 % | 16.4 % |
  | Triangle muff (2 ‖ 3 ‖ 5) | 12.2 % | 22 % | 41 % |

  Too much for the Silicon and the Muff: Phase B makes Newton a chord method (the
  factorisation kept while the ports barely move), linearises the Muff's output booster
  (3.3 kΩ of emitter degeneration: linear to within the model's own error) and measures
  again.
