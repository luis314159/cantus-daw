# Round 25 — piece A: core routing and the round's contract

**Builder:** core (piece A) · **Branch:** `r25/integration` (on `master` a89edf4), worktree
`F:/projects/daw-r23/beta-hardening` · **Uncommitted**: the integrator commits it as the
round's contract.

Read with `docs/round25/mixer-routing-design.md` (§10: the owner's answers),
`docs/adr/0041-mixer-routing.md` and `docs/round25/BUILDING.md`.

## What was done

### 1. `cantus_core::routing` — the graph read as a whole (`crates/cantus-core/src/routing.rs`)

- **`Project::route_check(from, to, kind) -> Result<(), RouteRefusal>`** — kinds
  `Output` (to a strip or `StripId::MASTER`), `Send`, `Key(Option<EffectId>)`. Checks, in
  order: the source (`FromMaster`, `UnknownStrip`), the destination, `ToItself`, then per
  kind. **`AlreadyThere` is the one refusal under which the command still applies** (as a
  change: a send's level, the same output again, the same key) — a property test holds
  `Ok ⇔ InsertStripSend applies` and `Ok ∨ AlreadyThere ⇔ SetStripOutput / SetStripSend /
  SetEffectSidechain applies` for every (from, to, kind) on twelve random graphs built by
  commands (stale ids and the master included); a mutation dropping the send's loop check
  fails it.
  - `Loop { through }`: the destination already reaches the source; `through` is the
    first strip found (breadth-first from the destination over outputs, sends and keys)
    that feeds the source — the hint says "Would make a loop: {through} already feeds
    {from}".
  - `Key(Some(e))`: `UnknownEffect`, `NotKeyable`, `AlreadyThere` (keyed from that source
    already). `Key(None)` ("can this strip be keyed at all?"): `Ok` when an insert takes a
    key and is not keyed from the source yet, **or** there is room for a new insert (the
    duck / gate offers); else `AlreadyThere` / `NoRoom`.
  - A key **into** a master insert is allowed (the master runs last); the master is never
    a source; a send to the master is `SendToMaster`.
- **`feeders(project, id)` / `Project::feeders`** → `Feeders { tracks, outputs, sends:
  (StripId, Send) — level, switch and tap, switched-off sends included (still a route),
  keys: (source, insert), notes: (track, insert) }`; works for `StripId::MASTER` too
  (strips whose output is the master, keys and notes into master inserts). **`is_fed`**
  answers without allocating. The name cell's hover now says who feeds a strip
  (`routing_actions::feeders_text`) — the owner's bug (a return fed only by sends said
  nothing fed it) is pinned by a test.
- **`output_reaches(project, strip, to)`** (through outputs and *sounding* sends
  downstream) and **`doubled(project, strip, to)`** = a sounding send to where the output
  already reaches — allowed, flagged (answer 5).
- **`key_choices(project, from, host)`** — the inserts `from` could key now, in chain order.
- **`edges(project)` / `edges_into`** — per strip in project order its output (the
  master's too, `Edge::is_master_output()`) then its sends in list order; then every key per
  keyed host in project order, the master last. `EdgeKind::Send` carries level, switch and
  tap.
- `Route { from, to, kind }` (`Route::output/send/key`) and `End { Source, Destination }`
  name a route for the cables, the cells and the route menu.

### 2. `routing::plan` — the batches (`routing_plan.rs`)

Each returns **one** `Command::Batch` with its label, or `None` (nothing to do, or
refused — ask `route_check` why): `add_sends` (`cmd.add_send`, unity post-fader, sources
that cannot send skipped), `replace_outputs` (`cmd.replace_output`, sends untouched),
`key` (`cmd.key_sidechain`), `key_with` (a caller-made slot inserted at the chain's end and
keyed; the caller's label — `cmd.comp_duck`, `cmd.gate_key`), `move_route`
(`cmd.move_route`: a send's destination end keeps **its place in the list**, level, switch
and tap via `RemoveStripSend` + `InsertStripSend`, and a full list is no reason to refuse;
a send's source end moves the same send to another strip; an output's either end; a key
only by its source — a key belongs to its insert), `remove_route` (`cmd.remove_route`; the
master route cannot be removed), `send_to_output` (`cmd.send_to_output`), `output_to_send`
(`cmd.output_to_send`: back to the master plus a unity send, so the destination hears what
it heard), `new_return` (`cmd.new_return`: the strip appended in the right dock, its effect,
a send from every source that can), `route_tracks` (`cmd.route_tracks`), `set_send_tap`
(`cmd.set_send_tap`).

### 3. `routing::order` — strips and groups moving (`routing_order.rs`, answer 12)

`Spot { dock, before: Option<StripId> }` (`Spot::before`, `Spot::end`, `spot_at(p, dock,
index)`); `move_strips` (`cmd.move_strips`), `move_group` (`cmd.move_group`), `nudge`
(Alt+←/→; a selection that is exactly a group's members moves the group), `nudge_group`,
`reorder` (the `MoveStrip`s that reach an order). Rules: a drag names its dock (docked
there); a nudge never leaves its dock; a strip dropped **between two members** joins the
group, a member dragged away from every other member leaves it (kept when it lands
beside a member, or when the whole group moves with it); a nudge hops a loose strip over
a whole group run, swaps a member inside its run, and steps a member at its run's edge
out of the group where it stands; a group dropped inside another group's run lands
before that run; a group hops over a whole other group per step.

### 4. Pre/post-fader sends — the model (answer 4)

`SendTap { Post (default), Pre }` (`ALL`, `is_post`, `label_key`), `Send::tap` with
`#[serde(default, skip_serializing_if = "SendTap::is_post")]`; the extension rule is on
the enum's doc (a new variant: its own name, handled in every `match` — no wildcard arms —
and a schema bump). `Send::new` = unity, post, on. **Schema 23** (`migrate_22_to_23`
writes nothing; `fixtures/v23.cantus` has a pre-fader send from Bass into the return at
0.5). DAWproject: import maps `type="pre"` to `Pre` (no longer reported as a loss;
`interop.report.d_pre_sends` retired from the report list and both locales), export writes
`pre`/`post`. **The engine ignores the tap**: `StripSnapshot::sends` stays `(usize, f32)`,
documented as rendering every send post-fader until piece F changes that field; every
existing null test and golden holds unchanged.

### 5. `cantus-ui/src/routing_actions.rs` — what every view calls

`Gesture { Send, Output, Key }` (`from_modifiers`: Alt → Key, Ctrl/⌘ → Output, else Send),
`TrackDrag(TrackId)` (the drag payload of a track or channel), `Outcome { Done,
Refused(RouteRefusal), ChooseKey(Vec<EffectId>), NothingToKey }`, `ReturnKind { Empty,
Reverb, Delay }`, and `CantusApp` methods: `gesture_sources`, `route_gesture` (a drop on
the master always routes the output; from a multi-selection every selected strip sends or
routes; a key is the dragged strip's alone), `add_sends`, `replace_output`,
`key_sidechain`, `gate_strip_from`, `move_route`, `remove_route`, `send_to_output`,
`output_to_send`, `new_return` (Spatium or the delay at full wet, named after the effect,
else "Return n"; the returns' colour; selected afterwards), `route_tracks_here`,
`set_send_tap`, `move_strips_to`, `move_group_to`, `nudge_strips`, `nudge_group`; free
functions `strip_name`, `level_text`, `feeders_text`, `refusal_text`, `doubled_text`.

Changed in the app to go through them: `send_strips_to` (was mislabelled
`cmd.route_strips`; now `cmd.add_send`) and `route_strips_to` (`cmd.replace_output`);
`duck_strip_from` builds its batch with `plan::key_with` (its preset moved to
`ducking_compressor()`); **Alt+←/→ moves the selection** (or the group) by
`nudge_strips`; `move_strip` (the strip menu's *Move*, Shift+wheel) follows the nudge's
rules — docks respected (it used to move across docks invisibly in project order).

### 6. The contract

- `docs/round25/BUILDING.md` — the three builders (desk = B+E, interaction = C + strip and
  group drag-reorder, matrix = D + F's engine and inspector), rules, file owners, the API
  above and every hook's exact signature.
- Stubs that compile and draw nothing, in the files their builders own, already called from
  the other builders' files: `views/mixer_cables.rs` (`band`, `column_hatch`),
  `views/mixer_drag.rs` (`Drag`, `Source`, `Target`, `source`, `target`, `target_group`,
  `finish`, `active`), `views/mixer_routehere.rs` (`plate`), `views/route_menu.rs`
  (`open`, `open_add_send`, `open_route_here`, `is_open`, `show`), `views/routing.rs`
  (`RoutingView`, `show`; `CantusApp::routing_view`), `views/inspector_receives.rs`
  (`receives`, called between Sends and Routing), `kit/cable.rs` (`CableKind`,
  `CableLook`, `depth_for`, `paint`, `hit`, `key_mark`, `doubled_mark`), `kit/tap.rs`
  (`switch`). `Desk::fed` (per revision) decides the plate; `mixer::route_owner()` is the
  desk's route-menu owner id. Stub items nobody constructs yet carry `#[expect(dead_code,
  reason = …)]`, which warns once they are used — remove it then.
- Locale areas `routing` (piece A's strings: the new `cmd.*`, `routing.tap.*`,
  `routing.feeders.*`, `routing.doubled`, `routing.refusal.*`, `routing.return_name`),
  `cables` and `matrix` (empty), registered in `i18n.rs`. `mixer.fed_by` and
  `mixer.fed_by_none` retired from `en.txt` / `es.txt`.
- `docs/adr/0041-mixer-routing.md` — the design's §11 draft, updated with the owner's
  answers (gestures, pre-fader sends and their growth rule, doubled sends allowed and
  flagged, Alt+F9, the chain summary, strips and groups moving).

## Tests

New: `cantus-core/tests/routing.rs` (16), `cantus-core/tests/strip_order.rs` (8),
`cantus-project` `format::send_tap_tests` (2), `cantus-ui` `routing_actions::tests` (7).
Changed: `undo_invariants.rs` (the send arms randomise the tap; `routing_batch` and
`order_batch` arms for every `plan` and `order` batch, a `fuzz.group` batch so groups
exist to move, a coverage check that all 14 routing labels really apply — all 40 seeds ×
120 commands undo and redo exactly), `dawproject.rs` (a pre-fader send round-trips; the
Bitwig file's pre send arrives `Pre` and is no loss). Seen failing first: the DAWproject
pair and the schema-23 pair (assertions), the routing and order files (no module), two
`routing_actions` expectations.

**Totals:** `cargo test -p cantus-core -p cantus-project -p cantus-engine -p cantus-ui
--no-fail-fast` — **1 351 passed, 0 failed, 24 ignored**; `cargo clippy --workspace
--all-targets -- -D warnings` clean; rustfmt run on every file piece A created and on the
changed files whose base was formatted (one file at a time, children untouched).

## What the next builders get, and what to watch

- **Desk**: derive `edges_into` and a per-strip edge index in `Desk::rebuild` (the fed
  flags are already there); the pill reads `EdgeKind::Send { level, enabled, tap }`; the
  doubled mark asks `routing::doubled` per send edge **per revision**, not per frame;
  call `drag::source(.., Source::Port(id))` from ports and the send row's ▼ chip,
  `Source::End { route, end }` from lit end marks; paint `column_hatch` from `Target`.
- **Interaction**: everything a drop does is a `routing_actions` call; `route_gesture`
  returns what to show (`ChooseKey` → a menu of `key_choices`, `NothingToKey` → the duck
  and gate offers, `Refused(r)` → `refusal_text(p, from, r)`). Drag-reorder computes a
  `Spot` from the drop x (`spot_at` takes an index in the dock's strip order); groups
  follow the rules in §3 above. `TrackDrag` is the payload the arrangement and the channel
  rack set.
- **Matrix**: a cell is `route_check(row, column, kind)`; a key cell needs
  `RouteKind::Key(Some(e))` per keyable insert or `Key(None)`; the pre-fader engine changes
  `StripSnapshot::sends` to carry the tap (`(usize, f32, SendTap)` as the design says) and
  the doc line there; `kit::tap::switch` is yours to fill; strings for the inspector go in
  the `matrix` area (`routing.tap.*` already exist).

## Uncertain / left for the integrator

- `Desk::fed` calls `is_fed` per strip per revision: O(strips × (tracks + strips)) — at
  256 strips well under a millisecond, but the desk builder may fold it into the edges
  pass.
- A key gesture from a multi-selection keys from the dragged strip only (an insert has
  one key); the design's "every selected strip keys" is not possible as written.
- `cmd.route_strips` was dropped from `en.txt` / `es.txt` (no user left).
- The interface still has its old per-selection loop walk (`Desk::routes`) for the send
  row; the desk builder may switch it to `route_check` or keep it (it is cheaper per frame).
