# The effects side round, wave 3 — the contract

One commit on `fx/integration` (after `d80a2444`): Tabula's board and three new effect kinds,
wired into every list, as the design (`C:\projects\daw-fx\tabula-design.md`) lays out in its
§7. The three units are exact pass-through stubs with one parameter `out` (dB, default 0 — a
null); the board has its real model, its twelve parameters, the routed spec API with the
generic callers switched, its command variants (bodies that refuse), the engine's plumbing
(an empty board — a wire — until its builder writes it); schema 63 (provisional) and
`fixtures/v63.cantus`; the ADR stubs 0134–0137 (0134 holds the design's decisions). No golden
hash moved, `DSP_VERSION` unchanged, every old song renders to the bit. Every added line in a
shared file carries `// fx3:contract` (or `// fx3:<stem>` where it is one builder's line).

| Variant | Stem | Unit | `ALL` | ADR |
|---|---|---|---|---|
| `Board` | `tabula` | Tabula, the pedalboard | 31 | 0134 |
| `PitchShift` | `saltus` | Saltus, pitch | 32 | 0135 |
| `Rotary` | `rota` | Rota, the rotary speaker | 33 | 0136 |
| `Amp` | `arca` | Arca, amp and cabinet | 34 | 0137 |

Never reorder `ALL`. Keep each unit's `out` key (its plug-in ids are blessed; the golden ids
are append-only — add yours with `CANTUS_BLESS_IDS=1 cargo test -p cantus-plugins --test
ids`). The board's twelve keys are its exported plug-in's whole list, forever.

## What the contract already gives every builder

- **Core.** `fx::tabula` (the model: `Board`, `Pedal`, `PedalId`, `Path`, `PedalShape`,
  `Macro`, `MacroMap`, `MAX_PEDALS`, `MACROS`, `MACRO_KEYS`; `board()`, `pedal_allowed`,
  `pedal_key`/`split_key`, `Board::{pedal, pedal_mut, index_of, has_split, macro_at, macro_of,
  sanitize}`, `next_id`, `macro_value`, `ensure_macro_keys`, `apply_macros`,
  `repair_project`, `guitar_rig`). `EffectSpec::board` and the routed API (`param`,
  `resolve`, `get`, `stored`, `set`, `clear`, `value_mut`, `ensure`, `takes_sidechain`,
  `takes_notes`, `param_keys`, `pedals`, `flat_params`). `EffectKind::takes_ir` (Aula; Arca's
  line waits). `starting::Start::Spec(StartingSpec { label_key, spec })` — the board starts
  as `guitar_rig()`.
- **Commands.** `InsertPedal`, `RemovePedal`, `MovePedal`, `SetPedalOn`, `SetPedalShape`,
  `SetMacroMaps`, `SetMacroName` (their `cmd.*` strings in `tabula.{en,es}.txt`, their
  freeze-guard and recording-mode arms); `SetEffectIr` and `SetLapsusPattern` carry
  `pedal: Option<PedalId>` (`None` everywhere today; `Some` goes to
  `command_board::set_pedal_ir`/`set_pedal_lapsus`). Every body in `command_board.rs`
  returns `InvalidValue("board: phase A")`.
- **Engine.** `FxUnit::{adopt, take_retired, pedals_mut, set_position_late,
  pedal_telemetry}` (defaults), `EffectUnit::{adopt, take_retired, pedals_mut, pedal_mut,
  pedal_telemetry}`; `EffectUnit::new` builds a board with `tabula::Unit::build`;
  `EngineMessage::AddPedal` (adopted by the processor, or back as `GarbageEffect`) and
  `SetAnalysedPedal` (→ `Meters::fx_pedal` through `graph::Analysis::fx_pedal` and
  `read_meters`); `EngineEvent::EffectNeed` and `EngineMessage::EffectMemory` carry `pedal`
  (asked for each `pedals_mut()` child, supplied to that child; every pass-through site
  passes it back); `MemoryNeed::Tabula(frames)` → `EffectMemory::Tabula(Box<tabula::Lines>)`;
  `fx::tabula::{Kid, Lines, Settings, settings, Unit}`. The engine's automation stores and
  writes through `ensure`/`value_mut`.
- **Project.** Schema 63 (`migrate_62_to_63` writes nothing), the board repair on load
  (`tabula::repair_project`), `fixtures/v63.cantus`: on the second strip a board — Fragor's
  green overdrive on A (`p0`), Delay at 250 ms on B (`p1`), macro 1 named *Drive* moving
  `od.drive` 20 → 80 %, at 25 % — then one insert each of Saltus, Rota and Arca. Tests
  `schema_63_keeps_a_board_and_wave_3s_kinds`, `a_hand_edited_board_is_repaired_on_load`.
- **UI.** A plain kit editor per unit (`views/fx/<stem>.rs`), the placeholder faces
  (`kit/skin/side.rs`: Tabula wears Fervor's look, Saltus Tonus's, Rota Spatium's, Arca
  Aequalis's — the side round's tests leave a placeholder out), the picker (Tabula and Arca on
  the new **Guitar and amp** shelf and Distortion, Saltus on Pitch, Rota on Modulation),
  `views/fx/tabula_faces.rs` with one `Face` per pedal-allowed kind (its test holds every
  knob to a key of its unit).

## Builders

Never more than two compile at once: **tabula ∥ saltus**, then **arca + rota** once saltus
is merged. Merge first deliveries early; the integrated-tree review runs mid-wave.

### tabula — the board (command bodies, engine, UI board, export)

Files you own: `crates/cantus-core/src/fx/tabula.rs` (and a new `fx/tabula_random.rs`),
`crates/cantus-core/src/command_board.rs`, `crates/cantus-dsp/src/tabula.rs`,
`crates/cantus-engine/src/fx/tabula.rs`, `crates/cantus-ui/src/views/fx/tabula.rs` and any new
`views/fx/tabula_*.rs` (the board view, `PedalHost`), `crates/cantus-ui/src/views/fx/
tabula_faces.rs` (all but the three `// fx3:<stem>` rows),
`crates/cantus-ui/locales/fx/tabula.{en,es}.txt`, `crates/cantus-ui/presets/effects/
board.toml`, `crates/cantus-engine/tests/fx_tabula.rs`, ADR 0134, and any new file under
your stem's name.

Anchors in shared files (find them by `fx3`):

- **Core:** `command.rs` (the seven variants and their dispatch — change only if the design
  needs it); `tests/undo_invariants.rs` (**add** an arm per board command and `p` keys:
  unknown id, a nested board, a hosted pedal, the seventeenth); `snapshot.rs` (≈ l. 464 and
  498, where the slots are cloned: `ensure_macro_keys` there, `apply_macros` at compile — not
  yet wired); `starting.rs` (`K::Board`).
- **Engine:** `backend.rs` (`send_snapshot`: `known_pedals` beside `known_effects`, build and
  send `AddPedal` before the snapshot — not yet wired); `processor_msg.rs` (`adopt_pedal`,
  `supply_effect`'s pedal, `request_memory`'s children — wired; **add** draining
  `take_retired` into `GarbageEffect`); `processor_auto.rs` (≈ l. 807, `apply_macros` before
  a dirty board copy is configured); `automation.rs` (`ensure_macro_keys` where an `FxCopy`
  is made); the place `set_position` is called for an insert (call `set_position_late` for a
  board); `tests/fx_contract.rs` (`must_rest`, **add** `a_board_of_one_is_the_unit`);
  `tests/support/heavy.rs` and `cantus-bench/src/kernels.rs` (`K::Board`: a full board);
  `tests/rt_memory.rs` (adopt, retire, reorder, footswitch, `p`-key and macro automation).
- **Plug-ins:** `src/state.rs` (`save_with_bank` → `save_spec_extras`: `board`, `ir`,
  `lapsus` — today `board: None`); `src/instance.rs` (`spec_of`'s `board: None`; the rings and
  the adopt as `lapsus_bank.rs`); `src/memory.rs` (the pedals' needs); `tests/common/mod.rs`
  (`Board` settings line and its `Direct::Stub` — the board stays the engine's own unit);
  `tests/clap.rs` (the automated key, now `"out"` — a macro once mapped).
- **UI:** `views/fx/mod.rs` (each `Tabula` arm), `views/fx/host.rs` (`set_lapsus_pattern`'s
  and the IR's `pedal: None` — your `PedalHost` fills it), `views/fx/frame.rs`
  (`designed_size`: add your arm; the hue arm), `kit/skin/side.rs` (`TABULA` — replace with
  a face that differs from every other unit's), `agent_units.rs` (`K::Board`),
  `picker_catalog.rs` (your line), `tests/preset_audit.rs` (delete your floor when presets
  land), `tests/demos.rs` (widen past 31 when the board is in the showcase).
- **AI:** `tools_mixer.rs` (the effect words: drop "being built" for tabula).

### saltus — pitch

Files you own: `crates/cantus-core/src/fx/saltus.rs`, `crates/cantus-dsp/src/saltus.rs`,
`crates/cantus-engine/src/fx/saltus.rs`, `crates/cantus-engine/tests/fx_saltus.rs`,
`crates/cantus-ui/src/views/fx/saltus.rs`, `crates/cantus-ui/locales/fx/saltus.{en,es}.txt`,
`crates/cantus-ui/presets/effects/pitchshift.toml`, ADR 0135, any new file under your stem.

Anchors (find them by `fx3`, your lines by `fx3:saltus`): `cantus-core/src/fx/saltus.rs`
(`TAKES_NOTES` — the design's `true`); `starting.rs` (`K::PitchShift`); `fx_contract.rs`
(`must_rest`); `tests/support/heavy.rs` and `cantus-bench/src/kernels.rs`;
`cantus-plugins/tests/common/mod.rs` (settings and the `Direct` arm — name your DSP and drop
the Stub arm); `tests/clap.rs` (automated key); `views/fx/frame.rs` (hue; add your
`designed_size`); `views/inspector_mini.rs` (hue); `views/fx/tabula_faces.rs` (your row);
`kit/skin/side.rs` (`SALTUS`); `agent_units.rs` (`K::PitchShift`); `tools_mixer.rs` (your
words); `tests/preset_audit.rs`; `tests/demos.rs`.

### arca + rota — amp and cabinet, the rotary speaker, the Uni-Vibe

Files you own: `crates/cantus-core/src/fx/{arca,rota}.rs`, `crates/cantus-dsp/src/
{arca,rota}.rs`, `crates/cantus-engine/src/fx/{arca,rota}.rs`, `crates/cantus-engine/tests/
fx_{arca,rota}.rs`, `crates/cantus-ui/src/views/fx/{arca,rota}.rs`,
`crates/cantus-ui/locales/fx/{arca,rota}.{en,es}.txt`, `crates/cantus-ui/presets/effects/
{amp,rotary}.toml`, ADRs 0136 and 0137, any new file under either stem.

Anchors (by `fx3`, your lines by `fx3:arca` / `fx3:rota`): `effect.rs` (`takes_ir`: the
`Amp` line); the same per-unit lines as saltus for each of `Amp` and `Rotary` (starting,
must_rest, heavy, bench, plug-in settings and `Direct`, clap key, frame and inspector hues,
`designed_size`, the faces rows, `ARCA`/`ROTA` in `side.rs`, `agent_units.rs`, the AI words,
preset floors, demos). **Shared with Fragor and Phasis (finished units — additive only):**
`cantus-dsp/src/fragor_dk.rs` (a new `Nl::Triode`, no existing arm changed — Fragor's golden
hashes must not move); Phasis's Uni-Vibe as a style **appended** to its choices
(`cantus-core/src/fx/phasis.rs` `CHOICES`/its stepped range, `cantus-dsp/src/phasis.rs`,
`cantus-engine/src/fx/phasis.rs`, `locales/fx/phasis.{en,es}.txt`, Phasis's presets and
manual) — every existing key, default and render unchanged.

## Notes

- The new units' `TAKES_NOTES` is `false` in the contract (stubs); Saltus and Rota set
  their own.
- `Start::Spec` carries a label key beside the function (`StartingSpec`) — the design named a
  bare `fn() -> EffectSpec`; the label names the rig in the editor and the AI's unit list.
- The engine's `fx::make(Board)` is an empty board; only `EffectUnit::new` builds pedals.
- The MCP tools golden was regenerated on purpose (the four new effect words and aliases).
- `tests/click_paths.rs` `p02b`: the strip menu's *Add effect* list grew by four, so the
  submenu (kept inside the window) opens higher; its travel budget 1 030 → 1 170 pt, the
  same two clicks.
- The website and the manual are untouched; each builder adds its unit's card and section.
- `cargo fmt --all` fails on this machine (os error 206): `cargo fmt -p <crate>`, one crate
  per call.
