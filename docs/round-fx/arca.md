# The fx side round, wave 3 — `arca`: Arca (amp and cabinet), Rota (rotary), the Uni-Vibe

**Phase A-1 checkpoint (DSP only, 2026-11-14).** The decisions, values, sources and
measurements are in ADR 0134 (Arca) and ADR 0133 (Rota and the Uni-Vibe), each under
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
ADR 0134 *Phase A-2*; the sounds re-rendered with them.

## Phase A-3 (2026-11-15): the follower split tried and reverted

`cpu_by_stage` (each circuit's cost in the unit) and `examples/arca_compare.rs` (spectra,
harmonics, transfer for an A/B between builds) added; the follower split measured against
`a7afeffa`, found to change the clipping where a follower's grid conducts, and reverted.
ADR 0134 *Phase A-3* has the table; files 05–20 are `a7afeffa`'s renders again.

## Verified only in emulation

Everything: no guitar has been played through Arca live; the renders are the evidence. CPU
was measured while other builders loaded the machine.

## Next

The engine units and parameter tables, Phasis's two new keys, the editor's design sheets
for the owner, presets, export, docs, the e2e test — and Arca's CPU (ADR 0134, *Left for
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

## Phase B (2), v2 (2026-11-16): the owner's verdicts ("more elaborate", "richer")

- Arca: `arca_valve.rs` (valves with heater and plate apart, the getter's flash, shielded
  sockets; transformers, filter capacitors, the back plate), the chassis in two rows,
  the faceplate (`arca_plate.rs`: chrome trim, jewel, two jacks, engraved legends, tick
  rings), the cabinet's woven cloth, piping and corners, the cone's fibre and frame, the
  microphone on its boom with its cable; the window 740 tall.
- Rota: the front's louvres and cloth left at the sides, the horn's and scoop's blur at
  speed, the drum's baffle and the speaker firing up into it, the moulded top, a finer
  grain, the amp's valves (Arca's painter), the instruments in walnut bezels.
- The integrator's findings fixed: Tabula's pedal print scales with its enclosure and a
  tight row staggers; the empty slot on two lines; the chosen pedal's cells as wide as their
  words; Phasis's feedback and LFO cells whole (`modfx::lfo_card_as`); the probe notes short
  forms apart (`probe::take_shorts`); `learn_everywhere` excuses Saltus's mode-only keys and
  a new test draws its other modes.

## Phase B (3), to the finish (2026-11-16): v2 approved ("Aprobado, sigue")

- **Editors**: the full windows as approved; the microphone's drag on the cone and in the
  side view one undo step each; Rota's microphones' angle and distance one step; Rota's horn
  turning clear of the louvres, its blur with streaks.
- **Presets**: Arca sixteen, Rota thirteen, Phasis three lamp-vibe ones, levels fitted by
  `preset_tool fit` (Arca −15.9 … +12.5 dB, Rota −5.3 … +6.7, Phasis −3.3 … −0.9); the
  starting presets with them (Clean combo +5.4 dB, Chorale +3.6); roles an effect's.
- **Export**: the plug-ins' direct DSP for both (`Direct::Arca` with its cabinet,
  `Direct::Rota`), Rota's notes in `notes.rs`, `rpm`/`cm`/`×` read back (`readout`).
- **AI**: the unit words (`agent_units.rs`) and the effect words (`tools_mixer.rs`); the
  MCP tools golden regenerated on purpose.
- **Site**: the two cards (`web/src/units_data.rs`, `EFFECTS` 34); Phasis's count 15.
- **Docs**: ADRs 0133 and 0134 (phase B, consequences, tests), `docs/plugins/arca.md`,
  `docs/plugins/rota.md`, the manual EN/ES, `docs/round-fx/rota.md`.
- **Tests**: `arca_e2e.rs`, `rota_e2e.rs` (through `CantusApp::assemble`),
  `arca_rota_drag.rs` (pointer events), `arca_rota_whole.rs`, the engine's and plug-ins'.

## Shared files touched (marked `fx3:arca` / `fx3:rota`)

Core: `starting.rs`, `fx/readout.rs`. DSP: nothing shared beyond phase A. Engine: none new.
UI: `kit/skin.rs`, `kit/skin/materials.rs`, `textures.rs`, `side.rs`, `gallery_skin.rs`,
`kit/text.rs` (`probe::take_shorts`), `views/fx/frame.rs`, `phasis.rs`, `modfx.rs`
(`lfo_card_as`), `pedal_art.rs`, `pedal_draw.rs` (print scales with the enclosure, staggered
words, the empty slot on two lines), `tabula.rs`, `tabula_board.rs`, `tabula_faces.rs`,
`agent_units.rs`; tests `learn_everywhere.rs` (Saltus's modes), gallery goldens. AI:
`tools_mixer.rs`. Plug-ins: `tests/common/mod.rs`, `tests/notes.rs`, `tests/clap.rs`. Web:
`units_data.rs`. README (Phasis's count).

## Verified only in emulation

- No guitar or organ played live through Arca or Rota; the renders are the evidence.
- The exported plug-ins in our CLAP/VST3 harness only; the file dialog not driven (the IR
  load path is tested from `aula::set_file` on).
- CPU measured while other builders loaded the machine (Arca's high gain ≈ 25 % at 4×).

## Left

- Owner-only: listening to the presets and to files 23 and 33 (re-rendered).
- At the integration: the site's pictures (`web/assets/units/{arca,rota}-*.webp`) and the
  preset excerpts; renumbering ADRs 0133/0134; Arca's high-gain CPU on a quiet machine.
