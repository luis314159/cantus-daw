# Round 41 — `nitor`: Nitor, the one-knob enhancer (ADR 0151)

Branch `r41/nitor` from `r41/contract`. Status: **stopped for the owner** — the face's design
sheet v1 and the presets' sound (sections at the end).

## Done

| Layer | Files (owned) |
|---|---|
| DSP | `cantus-dsp/src/nitor.rs` (settings, characters, recipes, the meter), `nitor_run.rs` (glides, `set`, `process`, rest), `nitor_chunk.rs` (a grid cell's audio, the dynamics' parking), `nitor_level.rs` (pre-gain and level match), `nitor_excite.rs` (the exciter by band), `nitor_low.rs` (sub, harmonics, tight, lift); tests `nitor_tests.rs`, `nitor_tests_measure.rs`, `nitor_testkit.rs`, `tests/nitor_alloc.rs` |
| Core | `cantus-core/src/fx/nitor.rs` (16 parameters) |
| Engine | `cantus-engine/src/fx/nitor.rs` (the unit, the song's key, telemetry) |
| UI | `views/fx/nitor.rs`, `nitor_corona.rs`; `kit/skin/side_r41.rs`; `presets/effects/enhancer.toml`; locales `fx/nitor.{en,es}.txt`; examples `shoot_nitor.rs`, `nitor_listen.rs`; test `tests/nitor_e2e.rs` |
| Docs | `docs/plugins/nitor.md`, the manual's section EN/ES, ADR 0151, this file |

(The sections below are filled as the measurements come in.)
