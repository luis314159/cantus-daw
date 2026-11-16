# The fx side round, wave 3 — `arca`: Arca (amp and cabinet), Rota (rotary), the Uni-Vibe

**Phase A-1 checkpoint (DSP only, 2026-11-14).** The decisions, values, sources and
measurements are in ADR 0137 (Arca) and ADR 0136 (Rota and the Uni-Vibe), each under
*Phase A-1*. This file says what exists, where, and what is next.

## Done (cantus-dsp only)

| What | Files (owned) | Shared files (additive, marked `fx3:arca` / `fx3:rota`) |
|---|---|---|
| Arca | `arca.rs` (the unit), `arca_net.rs` (blocks → split circuits), `arca_models.rs` (five amps), `arca_tube.rs` (valves), `arca_power.rs` (power amp, sag), `arca_cab.rs` (generated cabinets), `arca_tests.rs`, `arca_spice_tests.rs` | `fragor_dk.rs`: `Nl::Triode`, `Koren`, `fast_ln`, `softplus`, per-port predictor bounds and tolerances (Fragor's elements keep the old constants: its tests unchanged) |
| Rota | `rota.rs`, `rota_tests.rs` | `organum_rotary.rs`: `tick` = `advance` + `tick_moved` (bit-identical; Organum's tests unchanged) |
| Uni-Vibe | `phasis_vibe.rs`, `phasis_vibe_tests.rs` | `phasis.rs` (`PhasisStyle`, `intensity`, the vibe run), `phasis_tests.rs` (`..Default` in one literal), `cantus-engine/src/fx/phasis.rs` (`..PhasisSettings::default()` — not compiled here: DSP-only phase) |
| Sounds | `examples/arca_sounds.rs` → `C:\projects\daw-fx\compartir\arca\sound\` (32 files and a README) | — |

## Phase A-2 (2026-11-15): CPU and the split

`arca_table.rs` (the valves tabulated), `arca_fast.rs` (the lean stepper for all-triode
circuits), the stand-in plate load in `arca_net.rs`; in `fragor_dk.rs` (additive) the
`Koren::table` field and `Circuit::fused`, a few constants made `pub(crate)`. Numbers in
ADR 0137 *Phase A-2*; the sounds re-rendered with them.

## Phase A-3 (2026-11-15): followers as their own circuits

`arca_net.rs`: a follower split from its plate (DC carried, grid-current sink),
`cpu_by_stage`. Numbers in ADR 0137 *Phase A-3*; files 05–20 re-rendered.

## Verified only in emulation

Everything: no guitar has been played through Arca live; the renders are the evidence. CPU
was measured while other builders loaded the machine.

## Next

The engine units and parameter tables, Phasis's two new keys, the editor's design sheets
for the owner, presets, export, docs, the e2e test — and Arca's CPU (ADR 0137, *Left for
the next phases*).
