# ADR 0134 — Tabula, the pedalboard

**Status:** proposed (fx side round wave 3, provisional number — renumbered at merge) ·
**Date:** 2026-11-14 · **Schema:** 63 (provisional — renumbered when merged after round 40) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0022 (memory built off the audio thread, notes
into effects), ADR 0027 (automation), ADR 0031 §5.1 (at rest), ADR 0063 (isolation), ADR 0071
(effects that sound on insert), ADR 0074 (unit skins), ADR 0101 (presets), ADR 0109 (driven
controls), ADR 0131 (Fragor's pedals and `pedal_draw`), ADRs 0135–0137 (Saltus, Rota, Arca)

## Context

The owner (2026-11-13): *"¿el plugin de efectos podría ser como una pedalera simulation,
donde adentro podamos poner múltiples efectos? … algo gráfico donde quitas, pones, ajustas,
cambias orden"* — and an amp and cabinet inside. BACKLOG §3b's *Distructor-like
multi-effect* and *amp and cabinet with IR loading*. He then asked that no pedal get a new
engine: each pedal is the existing unit, bit-identical to it inserted alone; and that every
new engine wave 3 makes be a full unit of its own (Saltus, Rota, Arca) as well as a pedal.
He chose the names Arca and Saltus, and Tabula's starting preset: a guitar rig.

## Decision

A new effect kind, `EffectKind::Board` (Latin *tabula*, the board), `fx::tabula`: a
container of native units as pedals.

### 1. Model (`cantus_core::fx::tabula`)

- `EffectSpec::board: Option<Box<Board>>` — `None` for every other kind and for an empty
  board, and then not written, so a native effect's JSON is what it always was.
- `Board { pedals: Vec<Pedal>, macros: Vec<Macro> }`, the `Vec`'s order the signal's;
  `Pedal { id: PedalId(u16), spec: EffectSpec, on (default true), path: Path, shape:
  Option<PedalShape> }`; `Path { Pre (default), A, B, Post }` — the flow is
  **Pre → (A ∥ B) → Post**, and a split exists only while some pedal is on A or B;
  `PedalShape { Mini, Standard, Wide, Double, Round }` (`None` = the unit's natural shape).
  `MAX_PEDALS = 16`, `MACROS = 8`. `Macro { name: Option<String>, maps: Vec<MacroMap> }`,
  `MacroMap { pedal, key, from, to }` in the target's own units, along its taper.
- **Addressing.** A pedal's parameter is `p{id}.{key}` (`p3.time_ms`, `p12.b3.gain`);
  `pedal_key` and `split_key` (no allocation) are exact inverses, and the board's own keys
  never start with `p` and a digit. Ids are board-local, so a board preset, a copy and a
  plug-in state are each one self-contained `EffectSpec`. Automation (`AutoTarget::Effect`),
  MIDI learn (`MappingTarget::Param`), modulation and the AI use that key unchanged.
  *Rejected:* project `EffectId`s for pedals — presets and states would need id remapping.
- The board's own twelve parameters: `m1`…`m8` (0…1, 0), `in`, `out` (±24 dB, 0), `mix`
  (0…1, 1; the dry aligned to the latency), `ab` (the A/B balance, 0.5 — an empty split at
  0.5 is exact). Every default is a null.
- **The routed spec API**, in one place (`EffectSpec`): `param`, `resolve`, `get`, `stored`,
  `set`, `clear`, `value_mut` (writes without inserting — the audio thread), `ensure`
  (inserts — off the audio thread), `takes_sidechain` and `takes_notes` (any pedal's),
  `param_keys`, `pedals`, `flat_params`. Routing happens only for `EffectKind::Board`
  (Fluctus's own keys are `p0.on`…). The generic callers were switched in the contract:
  `command.rs` (`SetEffectParam`, `ClearEffectParam`, the note source), `automation/{range,
  set}.rs`, `mix_snap.rs` (a board's pedals snapshot as `p{id}.{key}`), the UI's `host.rs`,
  `controls.rs`, `controllers_words.rs`, `automation.rs`, `swap.rs`, the AI's
  `tools_mixer.rs`/`tools_mod.rs`, and the engine's `automation.rs` (`ensure`/`value_mut`).
- **What may be a pedal** (`pedal_allowed`): every native unit but the board (no nesting),
  a hosted plug-in (delivered by its host, isolated per ADR 0063, and it could not live in an
  exported Tabula), the older Reverb and Speculum (its scope rings are keyed by `EffectId`).
  Enforced by the commands and by `Board::sanitize` on load (`repair_project`).
- **Macros** resolve off the audio thread: `ensure_macro_keys` (allocates) where the
  snapshot clones the slots and where the automation runtime copies a board; `apply_macros`
  (no allocation) at the snapshot's compile and in `processor_auto` before a dirty board copy
  is configured. A mapped target follows its macro (the macro wins over automation of that
  target). One macro per target key.
- **Starting preset** (ADR 0071): `Start::Spec(StartingSpec { label_key, spec })` — the
  owner's guitar rig, `guitar_rig()`: Fragor's green overdrive at a low drive → Concentus →
  Delay → Spatium in its spring style.

### 2. Commands (`cantus_core::command_board`)

| Command | Exact inverse |
|---|---|
| `InsertPedal { strip, effect, index, pedal, maps: Vec<(macro, position, MacroMap)> }` | `RemovePedal` |
| `RemovePedal { strip, effect, pedal }` | `InsertPedal` with its index and the macro maps removed with it |
| `MovePedal { strip, effect, pedal, index, path }` | itself with the old index and path |
| `SetPedalOn { …, on }`, `SetPedalShape { …, shape }` | the old value |
| `SetMacroMaps { …, macro, maps }` | the old list |
| `SetMacroName { …, macro, name }` | the old name |

A pedal's parameters go through `SetEffectParam`/`ClearEffectParam` with `p` keys;
`SetEffectIr` and `SetLapsusPattern` gain `pedal: Option<PedalId>` (`#[serde(default)]`);
a board preset is `ReplaceEffect`. Replacing a pedal (Remove + Insert), randomising
(`cmd.tabula_randomise`, seeded, ranges in `fx/tabula_random.rs`) and each drag are one
`Batch` — one undo step. Refused: a slot that is not a board, a duplicate id, a disallowed
kind, a seventeenth pedal, a map to a missing pedal or key. Fresh ids: no stored `next_id`;
the interface allocates `max(pedal ids, every p{n} referenced by this effect's lanes, maps
and modulators) + 1` (`next_id`), so a leftover lane never attaches to another unit. An id's
kind never changes.

### 3. Engine (`cantus_engine::fx::tabula`)

- Children are `EffectUnit`s (Delay, Reverb, EQ and Comprimo are `Dsp` variants, not
  `make()` units; `EffectUnit` carries the need, supply and mailbox). `EffectUnit::new` has a
  `Board` arm → `tabula::Unit::build(&spec, sr)`, building every child on the UI thread.
- **Add:** `EngineHandle::send_snapshot` keeps `known_pedals: HashSet<(EffectId, PedalId)>`
  beside `known_effects`; a new pedal is built and sent as `EngineMessage::AddPedal { effect,
  pedal, unit }` before the snapshot; the processor calls `FxUnit::adopt` (refused →
  `GarbageEffect`). **Remove:** a pedal missing from the spec fades out (10 ms); the
  processor drains `FxUnit::take_retired` into `GarbageEffect`. **Reorder:** configure
  rewrites the order only, with Fragor's 10 ms dip; nothing is rebuilt.
- **Footswitch:** a latency-free pedal crossfades to a wire and then stops running; a latent
  one is soft-bypassed through a per-pedal dry line — **the board's latency never changes on
  a footswitch.** Latency = Σ Pre + max(A, B) + Σ Post (bypassed pedals included); the
  shorter path is aligned by a line from `MemoryNeed::Tabula(frames)`.
- **Memory:** `EngineEvent::EffectNeed` and `EngineMessage::EffectMemory` carry `pedal:
  Option<PedalId>`; `request_memory` also asks each `FxUnit::pedals_mut()` child, and a
  supply with a pedal goes to that child (the contract wired both; every pass-through site —
  backend, freeze render, listen, modulator, render, the bench rig, the tests — passes the
  pedal back).
- **Fan-out:** the key to every child; `note()` to every child; `follows_song()` = any;
  `FxUnit::set_position_late` gives child k the board's lateness plus the latency before it,
  through `earlier_in` (exact across tempo maps); `fork()` only if every child forks.
- **Rest:** when no fade moves and every running child rests; `skip` to the running ones.
- **Telemetry:** the board's own block holds per-pedal in/out peaks (LEDs, cable energy) and
  latency; the open pedal editor's through `EngineMessage::SetAnalysedPedal` and
  `Meters::fx_pedal` (`FxUnit::pedal_telemetry`; the contract wired the path).
- **Bit identity:** at the defaults, fades settled, the board only calls
  `kid.process_keyed(l, r, key)` — no multiply — so a board of one pedal is that unit alone.
- **CPU:** the board's own cost below 1 % of its children's (bench with 8 pedals).

### 4. Plug-in export

The fixed parameter list is the board's twelve (golden ids appended). No generic slot
parameters — they would silently retarget after a reorder ("a key is forever"). The board
travels as content in the state (`save_spec_extras` writes `board`, `ir`, `lapsus`); the
main thread builds children and posts them through rings (as `lapsus_bank.rs`); the
processor adopts as the engine does; a whole state load swaps a rebuilt unit. The exported
editor is the same board view on `FxHost`; its `set("p…")` goes to the content post and
marks the state dirty. Lost in export: host automation of individual pedal knobs (only the
macros and four globals), host undo of board edits, user IR paths (machine-local).

### 5. The board view (`views/fx/tabula*.rs`)

A top-down 2-D board drawn with `pedal_draw` (enclosure, cable with energy, board, drop
marker, empty slot, jacks). The picker is the existing one filtered by `pedal_allowed`.
Mixed shapes wrap into rows, scale down to a 60 % floor, then scroll — never clip. The macro
rail: eight knobs on the top edge; learn, then touch a knob to map; mapped knobs wear a
driven ring (ADR 0109). The A/B split: a Y box and a mixer box (`ab`); dropping a pedal on a
lane sets its path. Keyboard: Tab between pedals, Space the footswitch, Enter the full
editor, Ctrl+←/→ moves, Delete removes, ↑/↓ the focused knob. AccessKit: the board a list,
each pedal a group of sliders and a toggle. A pedal's full editor is `Editor::of(kind)`
through a `PedalHost` wrapping the outer `FxHost` (it prefixes `p{id}.` and fills the
`pedal` field of IR and Lapsus edits).

### The faces (`views/fx/tabula_faces.rs`)

One row per kind that may be a pedal: up to four knobs, a shape, a motif, a letter, a
texture; the finish from the unit's own skin and colourway. The contract pre-filled every
row (placeholder knobs, below); each unit's builder edits only its own row.

| Unit | Knobs (contract) | Shape | Motif |
|---|---|---|---|
| Aequalis | b0.gain, b1.gain, b2.gain, out | Wide | Curve |
| Comprimo | thr, ratio, att, rel | Standard | Needle |
| Terminus | gain, rel | Standard | Needle |
| Spatium | space, decay, mix | Standard | Rings |
| Multiplex | out | Wide | Needle |
| Sibilus | thr, range | Mini | Needle |
| Porta | thr, rel | Mini | Needle |
| Fervor | drive, tone, mix | Standard | Clip |
| Latitudo | width | Mini | Rings |
| Culmen | gain | Standard | Needle |
| Delay | time_ms, feedback, mix | Standard | Echoes |
| Concentus | rate, depth, mix | Standard | Wave |
| Flexus | rate, fb, mix | Standard | Wave |
| Phasis | rate, fb, mix | Round | Wave |
| Tremor | rate, depth | Standard | Wave |
| Vibrans | rate, depth, mix | Standard | Wave |
| Anulus | rate, mix | Mini | Wave |
| Translatio | fb, mix | Mini | Wave |
| Fluctus | depth, div | Standard | Steps |
| Tonsor | knee, out | Mini | Clip |
| Impetus | attack, sustain | Mini | Needle |
| Discrimen | t.gain, b.gain | Standard | Needle |
| Tonus | key, scale | Standard | Steps |
| Aula | decay, mix | Standard | Rings |
| Cribrum | a.cut, a.res, mix | Standard | Curve |
| Fragor | od.drive, od.tone, od.level | Standard | Clip |
| Lapsus | tslot, vslot, mix | Standard | Steps |
| Vetus | wow, flut, hiss, mix | Standard | Steps |
| Libra | out, pan, width | Mini | Needle |
| Saltus | out (its builder's) | Standard | Steps |
| Rota | out (its builder's) | Double | Spiral |
| Arca | out (its builder's) | Wide | Clip |

**The contract** (wave 3's contract commit): `EffectKind::Board` and the three new kinds
appended (`ALL` 31 → 35) and wired into every list; the whole model, the routed API with its
callers switched, `pedal_allowed`, `sanitize`, `pedal_key`/`split_key`, `next_id`,
`ensure_macro_keys`/`apply_macros`, `guitar_rig` and `Start::Spec`; the command variants,
their dispatch, `cmd.*` strings and guard arms, with bodies that refuse (`"board: phase A"`);
the engine's empty board (`in`/`out` gains — a wire), the `FxUnit` defaults (`adopt`,
`take_retired`, `pedals_mut`, `set_position_late`, `pedal_telemetry`), the `pedal` field in
needs and memory, `AddPedal`, `SetAnalysedPedal`, `Meters::fx_pedal`, `MemoryNeed::Tabula`;
schema 63 and `fixtures/v63.cantus` (a board of Fragor on A and Delay on B, macro 1 named
Drive moving Fragor's drive); a plain kit editor, the placeholder face, the picker on the new
*Guitar and amp* shelf, the exported descriptor and ids.

## Consequences

To be written by the tabula builder.

## What the tests hold

To be written by the tabula builder. The design's risks and their tests: a board of one is
the unit (`fx_contract::a_board_of_one_is_the_unit`, every allowed kind, heavy, blocks
1/64/1024, bit-exact, and end to end through `CantusApp::assemble`); an empty board and an
empty split are nulls (`ab` 0.5); no allocation on the audio thread (`rt_memory.rs`: adopt,
retire, reorder, footswitch, `p`-key and macro automation, the plug-in state post); exact
undo (fuzz arms for each command and `p` keys; unknown id, a nested board, a hosted pedal,
the seventeenth); latency (sum and max; an impulse through A = Saltus ∥ B = wire arrives
once; a footswitch changes no latency); rest twins on boards; the key round trip (held
since the contract); macro over automation; needs routed (an Aula pedal with a user IR
sounds, a Lapsus pedal gets its ring); the export state round trip, `ids.txt` append-only,
no allocation on load; every old fixture and v63 load (held since the contract); the
board's CPU overhead below 1 %.
