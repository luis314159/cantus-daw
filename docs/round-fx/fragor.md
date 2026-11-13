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
