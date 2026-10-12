# Round 25 — mixer routing drawn as cables, a matrix, pre-fader sends (BACKLOG §6d.3)

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). This round makes the mixer's routing **visible and direct**: FL's cables under the
strips, a Routing matrix, "Route here" on a strip nothing feeds, an Extra large strip
without insert slots, pre-fader sends, and strips and groups that move by dragging.

Read, in order: this file; `docs/round25/mixer-routing-design.md` **completely** — §10,
*Answers from the owner (2026-10-10)*, overrides the proposals above it;
`docs/adr/0041-mixer-routing.md`; `docs/round25/core.md` (what piece A built and what you
get); `CLAUDE.md`; `docs/STATUS.md`'s top.

## The pieces

Piece **A** (core routing + this contract) is merged first: it is the base of every
branch. Then three builders work in parallel:

| Builder | Branch | Is | Delivers (design §) |
|---|---|---|---|
| **desk** | `r25/desk` | pieces B + E | The cable band (§3.1–3.2, §6): ports, arcs, level pills (`PRE −6.0` on a pre-fader send), edge chips, the doubled-signal mark, the *Off · Selected · All* switch (Selected by default), hatching of refused columns; Extra large without insert slots (§4.5): the strip's own send rows with the `POST | PRE` switch, the output chip, the one-line chain summary ("EQ · Comp · +1") that opens the inspector (answer 7), FL's from-selected row kept (answer 6); the new rows and drop order; the frame budget; every picture of §9. Tests 6, 7, 8, 14, 15, 16. |
| **interaction** | `r25/interaction` | piece C + strip/group drag-reorder | Dragging (§3.3): a cable from a port — plain = send, Ctrl = only output, Alt = key (answer 2) — a cable's end moved or dropped on nothing, auto-scroll, the hint; a track or a channel-rack channel dropped on a strip (answer 11); **strips and the selection dragged along the desk, a group dragged by its separator** (answer 12); the Route here plate and menu (§4.2); the one route menu (§3.3) with *New return ▸* (§4.3) and the doubled-signal note; the inspector's *Receives* (§4.4); the route menu's key (answer 9: your choice among free keys, shown in the menus). Tests 9, 10, 11, 12. |
| **matrix** | `r25/matrix` | piece D + piece F's engine and inspector | The Routing tab (§5): strips → strips, tracks → strips (notes into effects in the tracks block, answer 8), the click / Ctrl / Alt grammar, the keys, *All · In use*, **Alt+F9** (answer 9); the **pre-fader engine** (§7.3 as amended by answer 4: mute and solo silence a pre send too) with its null tests; the inspector's live `POST | PRE` switch and the kit's `tap` switch every view uses. Test 13 and §7.3's null tests. |

## Rules (as in rounds 20–24)

- Your worktree and branch only: `git worktree add -b r25/<name> ../daw-r25/<name>
  <contract commit>` with the untracked `.cargo/config.toml` (`[build] jobs = 4`;
  `[profile.dev]` and `[profile.test]`: `debug = 0`, **`incremental = false`** — incremental
  linking fails on this machine). Never the main tree, never push or merge.
- **Commit WIP early and often** (sessions are cut by API errors; resume them). Files in
  pieces under ~300 lines per tool call.
- Shell: Git Bash has no cargo on `PATH` — prefix `export PATH="$HOME/.cargo/bin:$PATH" &&`.
  A heredoc delimited by `EOF` fails: use `XEOF` or the Write tool. UAC refuses test
  binaries named `*setup*` / `*update*` (os error 740): run a copy under a neutral name.
  The Zscaler proxy is never bypassed.
- **Never `cargo fmt --all`** (the base is not fmt-clean): rustfmt only files you create,
  and files you change whose base was formatted.
- **Tests first**, each seen failing. Every mutation is a `Command` (ADR 0004); a new
  batch shape gets an arm in `crates/cantus-core/tests/undo_invariants.rs` (ask
  *interaction*, its owner this round, or add the arm in your own commit and say so — it
  is additive). Null tests before DSP: a post-fader send renders **bit-identically**; no
  golden changes without a `DSP_VERSION` bump in one commit with the measured size.
- Real-time rules (ADR 0005): the audio thread never allocates, locks, blocks or does I/O.
- Kit only in views (ADR 0024): no stock egui widgets, no literal colours, no `FontId::new`.
  **Look at what you build** (`cargo run --release -p cantus-ui --example shoot_mixer --
  shots <filter>`), at 75 % and 200 %, English and Spanish.
- Strings: your locale area only, both languages (the i18n test requires every English key
  in Spanish). The areas are registered in `i18n.rs` already; never edit `en.txt`/`es.txt`.
- **No schema change** this round beyond piece A's 23. ADR numbers if you need one: desk
  0043, interaction 0044, matrix 0045 (0042 is taken by the content-attribution ADR; the
  integrator renumbers).
- Finish: `cargo test -p cantus-core -p cantus-project -p cantus-engine -p cantus-ui
  --no-fail-fast` and `cargo clippy --workspace --all-targets -- -D warnings`;
  `docs/round25/<name>.md`; a report (done / measurements / tests / files outside your
  area / left / notes for the integrator).

## File owners (one owner per file; everything else: ask, or keep to an additive hook)

| Owner | Files |
|---|---|
| **desk** | `views/mixer.rs`, `mixer_cables.rs`, `mixer_desk.rs`, `mixer_layout.rs`, `mixer_strip.rs`, `mixer_parts.rs`, `mixer_paint.rs`, `mixer_meters.rs`; `kit/cable.rs`, `kit/glyph.rs` (the `Key` and `Cables` glyphs), `kit/brush.rs` (a dashed polyline, moved from `views/inst/fm_graph.rs`), `kit/gallery.rs`; `tests/frame_budget.rs`, `tests/mixer_desk.rs`, `tests/kit_gallery.rs`; `examples/shoot_mixer.rs`; `locales/areas/cables.*`, `locales/areas/mixer.*` |
| **interaction** | `views/mixer_drag.rs`, `mixer_routehere.rs`, `mixer_menu.rs`, `route_menu.rs`, `inspector_receives.rs`; `src/routing_actions.rs`; `cantus-core/src/routing.rs`, `routing_plan.rs`, `routing_order.rs`, `tests/routing.rs`, `tests/strip_order.rs`, `tests/undo_invariants.rs`; the drag-source lines in `views/arrangement*.rs` and `views/channel_rack*.rs` (the `TrackDrag` payload only); `locales/areas/routing.*`; new `tests/mixer_routing.rs` |
| **matrix** | `views/routing.rs` (and `routing_paint.rs` / `routing_cells.rs` as `#[path]` children of it); the tab registration (`layout.rs`'s pane, `views/workspace*.rs`'s tab line), `keymap.rs` (the *Routing* action, Alt+F9), its one `run_action` arm in `app.rs`; `kit/tap.rs`; `views/inspector_sends.rs`; the engine: `cantus-core/src/snapshot.rs`'s `StripSnapshot::sends`, `cantus-engine/src/graph/strip.rs`, `strip_run.rs`, `ctx.rs` (the PDC feed) and whatever else the pre send needs in the engine; `cantus-engine/tests/null_tests.rs`, `pdc.rs`; `locales/areas/matrix.*` |

`app.rs`, `views/mod.rs`, `kit/mod.rs`, `inspector.rs`, `inspector_channel.rs`, `lib.rs`
and `i18n.rs` already declare and call everything this round needs: edit them only for the
lines named above.

## What piece A gives you (the contract)

Core, `cantus_core::routing` (read `docs/round25/core.md` for the semantics):

```rust
pub enum RouteKind { Output, Send, Key(Option<EffectId>) }
pub enum RouteRefusal { UnknownStrip(StripId), UnknownEffect(EffectId), FromMaster,
    SendToMaster, ToItself, Loop { through: StripId }, NoRoom, NotKeyable, AlreadyThere }
impl RouteRefusal { pub fn label_key(&self) -> &'static str }
pub struct Route { pub from: StripId, pub to: StripId, pub kind: RouteKind }
impl Route { pub fn output(from, to) -> Route; pub fn send(from, to) -> Route;
    pub fn key(from, host, effect) -> Route }
pub enum End { Source, Destination }
pub struct Feeders { pub tracks: Vec<TrackId>, pub outputs: Vec<StripId>,
    pub sends: Vec<(StripId, Send)>, pub keys: Vec<(StripId, EffectId)>,
    pub notes: Vec<(TrackId, EffectId)> }            // is_empty(), len()
pub fn feeders(project: &Project, id: StripId) -> Feeders;      // also Project::feeders
pub fn is_fed(project: &Project, id: StripId) -> bool;          // no allocation
pub fn output_reaches(project: &Project, strip: StripId, to: StripId) -> bool;
pub fn doubled(project: &Project, strip: StripId, to: StripId) -> bool;
pub fn key_choices(project: &Project, from: StripId, host: StripId) -> Vec<EffectId>;
pub struct Edge { pub from: StripId, pub to: StripId, pub kind: EdgeKind }  // is_master_output()
pub enum EdgeKind { Output, Send { level: f32, enabled: bool, tap: SendTap },
    Key { effect: EffectId } }
pub fn edges(project: &Project) -> Vec<Edge>;
pub fn edges_into(project: &Project, out: &mut Vec<Edge>);      // keeps capacity
impl Project { pub fn route_check(&self, from: StripId, to: StripId, kind: RouteKind)
    -> Result<(), RouteRefusal> }
// routing::plan — each returns one Command::Batch, or None (nothing to do / refused)
add_sends(p, sources: &[StripId], to) / replace_outputs(p, sources, to)
key(p, from, host, effect) / key_with(p, from, host, slot: EffectSlot, label: &str)
move_route(p, route, end: End, to) / remove_route(p, route)
send_to_output(p, from, to) / output_to_send(p, from)
new_return(p, sources, strip: MixerStrip, effect: Option<EffectSlot>)
route_tracks(p, tracks: &[TrackId], to: Option<StripId>) / set_send_tap(p, from, to, tap)
// routing::order
pub struct Spot { pub dock: Dock, pub before: Option<StripId> }  // Spot::before / Spot::end
spot_at(p, dock, index) -> Spot
move_strips(p, strips, spot) / move_group(p, group, spot)
nudge(p, strips, delta: i32) / nudge_group(p, group, delta)     // -> Option<Command>
reorder(p, desired: &[StripId]) -> Vec<Command>
```

The model: `Send::tap: SendTap { Post, Pre }` (`SendTap::ALL`, `is_post()`,
`label_key()` → `routing.tap.post` / `.pre`), schema 23.

UI, `crate::routing_actions` (methods on `CantusApp`; each one undo step):

```rust
pub enum Gesture { Send, Output, Key }   // Gesture::from_modifiers(egui::Modifiers), .kind()
pub struct TrackDrag(pub TrackId);       // the egui::DragAndDrop payload of a dragged track
pub enum Outcome { Done, Refused(RouteRefusal), ChooseKey(Vec<EffectId>), NothingToKey }
pub enum ReturnKind { Empty, Reverb, Delay }
gesture_sources(&self, from) -> Vec<StripId>
route_gesture(&mut self, from, to, gesture) -> Outcome
add_sends(sources, to) / replace_output(sources, to) / key_sidechain(from, host, effect)
gate_strip_from(strip, key) -> Option<EffectId>   // and duck_strip_from (comp_editor.rs)
move_route(route, end, to) / remove_route(route)
send_to_output(from, to) / output_to_send(from)
new_return(sources, kind) -> Option<StripId>      // selected afterwards
route_tracks_here(tracks, to: Option<StripId>) / set_send_tap(from, to, tap)
move_strips_to(strips, spot) / move_group_to(group, spot)
nudge_strips(delta) / nudge_group(group, delta)  // Alt+←/→ already calls nudge_strips
// free functions
strip_name(p, id) -> String; level_text(level) -> String
feeders_text(p, id) -> String          // the name cell's hover (already wired)
refusal_text(p, from, refusal) -> String
doubled_text(p, from, to) -> Option<String>
```

## The hooks between you (declared as stubs that compile and draw nothing)

Each stub lives in the file of the builder who fills it; the calls are already in the
other builders' files, so nobody edits another's file to connect.

| Hook | Filled by | Called by (already wired) |
|---|---|---|
| `mixer_cables::band(v: &mut MixerView, app: &mut CantusApp, ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo)` | desk | `mixer.rs`, after the three docks (pass 5) |
| `mixer_cables::column_hatch(b: &mut Brush, s: &Style, column: Rect, target: drag::Target)` | desk | every strip, the master and each separator, with what `drag::target` said |
| `mixer_drag::source(v: &mut MixerView, app: &mut CantusApp, ui: &Ui, resp: &Response, src: Source)` — `Source::{Port(StripId), End { route: Route, end: End }, Strip(StripId), Group(GroupId)}` | interaction | the number cell (`Strip`), the separator (`Group`) — wired; the band's ports and end marks (`Port`, `End`) and the send row's ▼ chip (`Port`) — **desk wires these** |
| `mixer_drag::target(v, app: &mut CantusApp, ui: &Ui, strip: Option<StripId>, column: Rect) -> Target` (`None` = the master) and `target_group(v, app, ui, group: GroupId, column: Rect) -> Target` — `Target::{Idle, Valid, Refused}` | interaction | each visible column, each separator |
| `mixer_drag::finish(v, app, ui: &mut Ui, b: &mut Brush, s: &Style, geo: &Geo)` | interaction | `mixer.rs`, after the band |
| `mixer_drag::active(v: &MixerView) -> bool` | interaction | the band (desk) |
| `mixer_drag::Drag` (state, `Default`) | interaction | the `MixerView::drag` field |
| `mixer_routehere::plate(v, app, ui: &mut Ui, b: &mut Brush, pass: &Pass, strip: StripId, well: Rect)` | interaction | the meter row of an unfed strip (`Desk::fed`, per revision) |
| `views::route_menu::{open(ctx, owner: Id, route: Route, at: Pos2), open_add_send(ctx, owner, from: StripId, at), open_route_here(ctx, owner, strip: StripId, at), is_open(ctx, owner) -> bool, show(app: &mut CantusApp, ui: &mut Ui, owner: Id)}` | interaction | the desk shows its own under `mixer::route_owner()`; the matrix opens and shows its own under an id of its choosing, once a frame; the desk's pills and send rows and the inspector open it |
| `inspector_receives::receives(sh: &mut Sheet<'_>, app: &CantusApp, ms: Option<&MixerStrip>)` | interaction | `inspector_channel.rs`, between *Sends* and *Routing* |
| `kit::cable::{CableKind, CableLook, depth_for, paint, hit, key_mark, doubled_mark}` | desk | the band; the matrix's key cells (`key_mark`) and doubled cells (`doubled_mark`) |
| `kit::tap::switch(ui: &Ui, b: &mut Brush, s: &Style, rect: Rect, id: Id, tap: SendTap) -> Option<SendTap>` | matrix | the desk's send rows and pills, the inspector, the route menu — each then calls `set_send_tap` |
| `views::routing::{RoutingView, show(app: &mut CantusApp, ui: &mut egui::Ui)}` | matrix | the tab (matrix registers it); state in `CantusApp::routing_view` |
| `crate::routing_actions::TrackDrag` | (A) | set by interaction in the arrangement and the channel rack; received in `drag::target` |

Remove each `#[expect(dead_code, …)]` on a stub item as soon as you construct or call it
(the compiler tells you: an unfulfilled expectation warns).

## The order of merging

The integrator merges A (the contract commit) first, then desk, interaction and matrix
(their hooks are additive), runs the suite and clippy, shoots §9's pictures for the owner,
writes STATUS — and only when the owner asks, merges to `master` and pushes.
