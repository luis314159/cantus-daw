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

## Phase A-3 (2026-11-15): the follower split tried and reverted

`cpu_by_stage` (each circuit's cost in the unit) and `examples/arca_compare.rs` (spectra,
harmonics, transfer for an A/B between builds) added; the follower split measured against
`a7afeffa`, found to change the clipping where a follower's grid conducts, and reverted.
ADR 0137 *Phase A-3* has the table; files 05–20 are `a7afeffa`'s renders again.

## Verified only in emulation

Everything: no guitar has been played through Arca live; the renders are the evidence. CPU
was measured while other builders loaded the machine.

## Next

The engine units and parameter tables, Phasis's two new keys, the editor's design sheets
for the owner, presets, export, docs, the e2e test — and Arca's CPU (ADR 0137, *Left for
the next phases*).

## Phase B (2): the design sheets (2026-11-15) — waiting for the owner

- **Telemetry for the display** (`cantus-dsp/src/arca.rs` `Work`, `arca_net.rs`
  `Preamp::take_peaks`, `arca_power.rs` `Power::take_drive`/`idle_supply`; engine indices
  6–16): each preamp circuit's swing over a third of its supply, the inverter's drive, the
  power valves' approach to grid current, the supply now and at idle, the oversampling.
  Meters only — the output's bits unchanged. `the_valves_work_harder_as_the_gain_goes_up`.
- **Arca's window** (`views/fx/arca*.rs`): the chassis (valves glowing with the telemetry,
  the supply's needle, the amp's character from its models, the oversampling with latencies),
  the control plate, the cabinet (the microphone dragged on the cone and in a side view, the
  microphone's kind, the designed response, the loaded file as a cabinet choice).
- **Rota's window** (`views/fx/rota*.rs`): the cabinet cut away, from above (microphones
  dragged), the ramps with each rotor's dot; Rota's telemetry was already enough.
- **Faces** (`kit/skin/side.rs`): `ARCA` (a new material, `Material::Hammered`, hammered
  enamel — `materials.rs`, `textures.rs` `Tile::Hammered`, the gallery and its goldens) with
  Graphite and Oxblood; `ROTA` (walnut) with Cherry and Black. The objects' palette in
  `kit/skin/cabinets.rs`. Pedals on Tabula: the Chest and Wheel motifs (`arca_art.rs`,
  `rota_art.rs`), their knobs' words.
- **Phasis**: the circuit card (Classic · Lamp vibe, the intensity), the window 1180 wide.
- **Shots**: `examples/shoot_arca.rs` → `compartir\arca\design\v1\`, `compartir\rota\design\v1\`.
  `tests/arca_rota_whole.rs`: no word cut, EN/ES, 75–200 %.
- Found, not ours: Fragor's green overdrive prints "GANANC…" on Tabula in Spanish;
  `learn_everywhere` fails on Saltus's stepped parameters (`ostyle`, `hsrc`, `deg1`, …)
  drawn without their offer; Phasis's "REALIM." is abbreviated in Spanish.
