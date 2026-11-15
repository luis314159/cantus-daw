# ADR 0134 — Tabula, the pedalboard

**Status:** accepted (fx side round wave 3, provisional number — renumbered at merge; the design approved by the owner 2026-11-15) ·
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

### 6. What the builder settled (phases A and B)

- **The engine's structure** (§3) holds a running order of `(kid, lane)` and a pending one:
  pedals added or taken out without reordering the others apply at once (fading in where
  they stand, fading out where they stood); a reorder or a lane change waits for the dip's
  bottom. A kid adopted before any spec names it waits (`fresh`) and is never retired by a
  configure that does not know it yet. Pedals fading out sit after the board's own
  (`pedals_mut` hands the processor only the board's), so a new pedal with an id still
  fading out gets its memory, not the old one.
- **The grid.** While anything structural moves, the board runs in pieces that end on a
  16-sample grid counted from the stream's start, and every structural event (a fade ending,
  a retire, the dip's bottom) lands on a grid point: the output is the same at any block
  size (`fx_tabula::structural_changes_do_not_depend_on_the_block_size`).
- **The lines.** One buffer (`MemoryNeed::Tabula(frames)`) carved into owned regions: the
  board's dry line (its latency), the shorter lane's alignment, each latent pedal's dry line
  (owned by the pedal's serial, so a pedal fading out and its successor with the same id
  never share one). A new buffer keeps every region still wanted, with its sound and place;
  regions no longer wanted are cleared, so one wanted again starts silent.
- **`set_position_late`'s argument** is the latency inside the board before the pedal, in
  samples; every insert hears the position through `EffectUnit::hear_position`, which
  picks `set_position_late` for a board.
- **The window** (approved 2026-11-15, its v2 after the owner's verdict): the macro rail over
  a board across the window, over a row of the chosen pedal's controls, its place on the
  board (size, lane, footswitch, remove) and the board's levels. Pedals stand up to 145 % of
  the window's scale to fill the board's height, wrap into two or three rows when that lets
  them stand clearly larger (the cable runs between the rows), shrink to a 60 % floor and
  then scroll. Its own face: **fine black vinyl** (a new material, `FineTolex`: smaller,
  quieter pebbles than the amp vinyl) with brass, no vents. A pedal's knob word takes the
  room its row leaves (centred on its knob, shifted a little where one side has room) and is
  set smaller, to 66 %, before anything is elided.
- **Board presets are flat maps** (`fx::tabula_presets`): the board's own keys, each pedal's
  `p{id}.{key}`, and its structure under `@` keys no parameter can have (`p{id}.@kind` its
  kind's place in `ALL`, `@at`, `@path`, `@on`, `@shape`; a macro's maps `m{n}.@fromJJ.…`
  and `m{n}.@toJJ.…`, its name an index into words the interface translates). So a board
  preset travels every path a preset does — the frame's menu, the browser, A/B, the agents,
  the user's own presets — and `apply_fx_params` loads one on a board as one
  `Command::ReplaceEffect`. The fourteen factory boards are written in code, their styles and
  roles in `board.toml`.
- **Edits outside a project**: `command_board::apply_to_spec` runs the board commands on a
  board that lives in no project (the exported plug-in), with the same checks and refusals;
  `FxHost::board_commands` carries them there.
- **The exported plug-in** (`board_post.rs`): the main thread keeps the board, builds a unit
  for each pedal the processor does not hold, and posts both through a ring; the processor
  adopts them, swaps the board into its spec and hands back the old board and anything
  refused through a second ring, and every faded pedal through a third. Its memory builder
  routes each need to its pedal. A fresh instance starts on the guitar rig.

## Consequences

- A board is an ordinary insert to everything outside it: the strip's compensation sees one
  latency, the rest scheduler one unit, the mix snapshot and the automation its keys (a mix
  snapshot's exact undo puts `p` keys back into their pedals: `put_flat_params`).
- Pedals cannot nest, cannot be hosted plug-ins, Speculum or the old Reverb (§1).
- Removing a latent pedal changes the board's latency (the compensation follows); a
  footswitch never does.
- The engine's processor keeps where its boards run (`Boards`, refreshed when memory is
  asked for) and hands back faded pedals each block. A slot bypassed as a wire is not run,
  so a pedal taken off a bypassed board is handed back when the slot plays again.
- The new units (Saltus, Rota, Arca) appear as pedals as soon as they land (every native
  kind is allowed): their faces are their builders' rows in `tabula_faces.rs`.
- The showcase song does not use Tabula yet (its guitar keeps Fragor, Phasis and Aula's
  spring): putting it there changes the showcase's sound, the owner's call.

## What the tests hold

- **A board of one is the unit**, to the bit, for every kind that may be a pedal, at its
  defaults, heavy and two random settings, keyed and with notes, at blocks of 1, 64 and
  1024, with the same latency (`fx_contract::a_board_of_one_is_the_unit`); and in the app
  through `CantusApp::assemble` (`tabula_e2e::a_board_of_one_in_the_app_is_the_unit`).
- **Nulls:** an empty board, an empty split at 50 % (`fx_tabula`).
- **Latency:** Σ Pre + max(A, B) + Σ Post; an impulse through a latent lane A beside a wire
  on B arrives once; a footswitch on a latent pedal keeps the latency and plays the input
  exactly that late (`fx_tabula`).
- **Structure:** a reorder, a lane, a footswitch, a pedal removed, levels moved, a pedal
  adopted and added, the order taken back — the same bits at blocks of 1, 7, 512, 4096 and
  two random splittings; a pedal removed is handed back, an adopted one waits until named.
- **Song position:** a board of a linear-phase EQ and a synced tremolo renders as the two
  inserts (each pedal hears the song as late as it is).
- **Macros win over lanes**, and the snapshot carries them applied (`fx_tabula`,
  `tabula::tests`).
- **Rest:** the family's twin tests on a default and a heavy board (seven pedals, a split
  of unequal latencies, two linear-phase units) (`fx_contract::at_rest::tabula`), and split
  invariance (`fx_contract::split::tabula`).
- **No allocation on the audio thread:** footswitches on latent and latency-free pedals, a
  reorder, a lane, a pedal added and one taken out, lanes on a pedal's knob and on a macro,
  a seek and a stop (`rt_memory::tabula_never_allocates`); a board loaded into the exported
  plug-in while it plays (`cantus-plugins/tests/tabula_board.rs`).
- **Undo:** every board command and its inverse (`command_board_tests`), the undo fuzz's
  board arm (thirteen kinds, each applied, the refusals refused: an unknown id, a nested
  board, a hosted pedal, the seventeenth, a map to a key that is not there).
- **Presets:** every factory board is sane and survives its flat map; a plain map is no
  board (`tabula_presets::tests`); the audit's floor and tags (`preset_audit`).
- **End to end** (`tabula_e2e`): inserted from the picker's path on a playing track, the
  guitar rig heard in the master; a pedal moved in one step and undone to the bit; a
  footswitch; a pedal added (a fresh id, sounding); a factory board loaded and undone.
- **Export:** a fresh instance's rig, the state's round trip, and the family's CLAP/VST3
  tests over the board's twelve parameters (`clap.rs`, `vst3.rs`, `ids.rs`, `alloc.rs`).
- **CPU:** the board's own cost beside eight pedals as inserts
  (`cantus-engine/examples/tabula_overhead.rs`; measured in `docs/round-fx/tabula.md`).
