# Rota, the rotary speaker — the builder's report (fx side round, wave 3)

Built by the arca builder beside Arca, on branch `fx/arca` (ADR 0133; Arca's
report is `arca.md`). Plug-in page `docs/plugins/rota.md`.

## Done

- **DSP** (`cantus-dsp/src/rota.rs`, Phase A): Organum's cabinet split bit-identically into
  `advance` + `tick_moved`; fixed-point angles (an exact closed-form skip); the speed's lag
  landing exactly; the brake; the published speeds and ramps; the cabinet's 12AX7 drive.
- **Model** (`cantus-core/src/fx/rota.rs`): 14 keys, the null at mix 0, `out` kept,
  `TAKES_NOTES`; the starting preset *Chorale* (+3.6 dB, fitted).
- **Engine**: the unit (settings, notes throwing the switch to the sample, telemetry);
  latency 0; at rest only parked.
- **Window**: design sheets v1 and v2 (`compartir\rota\design\`), approved 2026-11-16: the
  cabinet cut away (louvres and cloth at the sides, the horn's blur at speed, the drum's
  baffle over the speaker), from above (microphones dragged), the ramps; the switch along
  the top; walnut with Cherry and Black colourways; the Wheel motif on Tabula.
- **Presets**: thirteen, levels fitted (`preset_tool fit`, −5.3 … +6.7 dB).
- **Export**: the direct DSP in the plug-ins' tests; notes through CLAP and VST3; `rpm`, `cm`
  and `×` read back from text; ids blessed (Phase B 1).
- **Docs**: ADR 0133 (Phase B, Consequences, tests), `docs/plugins/rota.md`, the manual
  EN/ES, the AI's unit and effect words, the site's card.
- **Phasis's lamp vibe** (ADR 0133): its circuit card, three presets.

## Tests and what each holds

See ADR 0133 *What the tests hold*. Files: `cantus-dsp/src/rota_tests.rs`;
`cantus-engine/tests/fx_rota.rs`, `fx_contract`, `rt_memory`; `cantus-ui/tests/rota_e2e.rs`,
`arca_rota_drag.rs`, `arca_rota_whole.rs`; `cantus-plugins/tests/notes.rs`, the CLAP/VST3
suites.

## Verified only in emulation

- No rotary cabinet compared by ear here; the speeds and ramps are the published
  measurements, the renders in `compartir\arca\sound\` (21–23, 31–33) are the evidence.
- The exported plug-in in our own CLAP/VST3 harness only.

## Left

- Owner-only: listening to the presets and files 23 and 33 (re-rendered with the ramps that
  end in finite time).
- At the integration: the site's pictures (`web/assets/units/rota-*.webp`) and the preset
  excerpts, as for the round's other units; renumbering ADR 0133.
