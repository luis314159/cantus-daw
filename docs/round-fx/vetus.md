# The effects side round — `vetus`: Vetus (lo-fi) and Libra (utility) (ADR 0133, provisional)

Branch `fx/vetus` from `fx/integration` 3b6f2eba. Vetus first — Phase A (DSP, engine, the
design sheet, sounds), the owner's verdict, then Phase B; Libra after it, its design sheet
approved before its editor is finished.

## Status: Vetus Phase A done — waiting for the owner's verdict

| Layer | Files (owned) | Shared files (lines marked `// fx:vetus`) |
|---|---|---|
| Model | `cantus-core/src/fx/vetus.rs` (33 keys, `out` kept) | — |
| DSP | `cantus-dsp/src/vetus.rs`, `vetus_unit.rs`, `vetus_run.rs`, `vetus_digital.rs`, `vetus_noise.rs`, `vetus_age.rs`, `vetus_filter.rs`, tests `vetus_tests.rs`, `vetus_tests2.rs` | — |
| Engine | `cantus-engine/src/fx/vetus.rs`, `tests/fx_vetus.rs` | `tests/support/heavy.rs`, `tests/rt_memory.rs` (`vetus_never_allocates`), `cantus-bench/src/kernels.rs` (heavy) |
| UI | `views/fx/vetus.rs`, `vetus_record.rs`, `vetus_spectrum.rs`, `vetus_modules.rs`, `kit/skin/records.rs`, locales, examples `shoot_vetus.rs`, `render_vetus.rs`, test `vetus_face.rs` | `kit/skin.rs` (`pub mod records`), `kit/skin/side.rs` (the face and two colourways), `views/fx/frame.rs` (1120 × 800) |
| Docs | `docs/adr/0133-…` (Vetus's decision drafted), this file | — |

The decision and what the tests hold: ADR 0133.

## Measured (Phase A)

- Raw, a 9 kHz sine held at 8 kHz folds to 1 kHz at −0.2 dB; behind the anti-alias filter at
  −53.6 dB. The hold's image of 1 kHz at 7 kHz: −22.8 dB held, −47.9 dB smoothed.
- The wow's swing measured from zero crossings: ±20 ct within 2 for a 20 ct setting.
- The tone band drawn = heard within 0.1 dB (four media, 40 Hz – 16 kHz).
- Hiss at 100 %: −24 dBFS RMS (tape and vinyl, within 1 dB); hum −30 dBFS.
- Rest: heavy settings at rest 2.4 – 3.0 s into a silence (`fx_contract`).

## Shown to the owner

`C:\projects\daw-fx\compartir\vetus\design\` (rest and playing, Slate EN and Paper ES at
100 %, 75/150/200 % Slate EN, the telephone, the preview, `colour\` the three colourways,
`compare-references.png`) and `compartir\vetus\sound\` (guitar, drums, keys; README).

## Left (Phase B and Libra)

Phase B: the owner's notes; presets (≥ 12) and the starting preset; the end-to-end test
through `CantusApp::assemble`; the CLAP/VST3 export bit-identical (its ids blessed, the
plug-in tests' `Direct` arm); the AI description, docs page, manual EN/ES, site card, demos
line; the frame budget measured; the finishing suite. Then Libra.
