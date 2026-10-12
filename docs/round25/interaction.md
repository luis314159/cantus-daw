# Round 25 — builder *interaction*: drag, route here, the route menu, Receives, strips and groups moving

**Branch:** `r25/interaction` (base `57f3ccc`, the contract) · worktree
`F:/projects/daw-r25/interaction` · **ADR:** `docs/adr/0044-desk-gestures.md` (the
integrator renumbers if needed) · design §3.3, §4.1–4.4, the owner's answers 2, 3, 5, 9,
11, 12.

## What was done

### 1. Dragging on the desk — `views/mixer_drag.rs`

One drag state machine for everything that is carried on the desk (ADR 0044 §1–3):

- **A cable from a port** (`Source::Port`, wired by the desk on its ports and the send
  row's ▼ chip): plain = a unity post-fader **send**, the output untouched; **Ctrl** = the
  destination becomes the **only output**, sends untouched; **Alt** = a **key** — keyed at
  once on a strip with one keyable insert, a menu of them on a strip with several, and on
  a strip with none the offer of a ducking compressor or a gate keyed from the source. A
  drop on the master routes the output there. From a multi-selection every selected strip
  sends or routes, one undo step. The modifiers are read live: the rubber band (output
  solid, send thinner, key dashed in the dynamics hue) and the words change as they do.
- **Click mode** (added at integration, the owner's answer 2 "release *or click* on the
  destination"): a click on a port or the selected strip's ▼ chip arms a route from that
  strip (the port ringed, the chip outlined in the selection's colour); every column then
  answers as for a drag (hatching, outline, rubber band, chip, hint, crosshair); the next
  click on a strip makes the route with the modifiers held at that click (plain = send,
  Ctrl = only output, Alt = key); a refused strip keeps it armed; Esc, a click on empty
  space, on its own strip or outside the desk lets it go. A cover over the desk takes the
  completing click, so no control under it acts. The ▼ chip's list is on its right-click.
  Tests: `tests/mixer_routing.rs` — `a_click_on_a_port_then_on_a_strip_sends`,
  `ctrl_at_the_second_click_routes_the_output`,
  `an_armed_route_is_let_go_by_esc_empty_space_or_its_own_strip`,
  `the_selected_strips_chip_arms_a_route_too` (real ports, real frames).
- **A cable's end** (`Source::End`, wired by the desk on lit end marks): dropped on a strip
  the route moves (a send keeps its place, level, switch and tap); released over nothing it
  is removed — the chip says "Release to remove this route" while it is held.
- **Refused targets show before the drop**: every column answers (`Target::Valid /
  Refused / Idle`) from a cache worked out once per revision, drag, modifier and selection;
  the desk hatches refused columns (`mixer_cables::column_hatch`); the hovered column is
  outlined lime or in the danger colour with a wash; the chip names the reason ("Would
  make a loop: Keys already feeds Bus"); the cursor turns not-allowed. A refused drop issues
  nothing. Itself and "already there" stay quiet (no hatch).
- **Strips and the selection** dragged by the number cell, **a group** by its separator:
  the lifted columns washed, a line in the gap they would land in, the dock named by where
  the pointer is (a strip dropped on the right dock is pinned there), one undo step
  (`move_strips_to` / `move_group_to`, piece A's group rules). A strip outside the
  selection is picked up alone and selected. A click still selects (or folds the group):
  a press is a drag only after 6 pt of travel.
- **A track or a channel** dropped on a strip (`TrackDrag` payload) routes it
  (`route_tracks_here`), the strip selected; the hint and the chip say "Route Kick to 8".
- **Esc cancels** any drag; the middle **auto-scrolls** within 24 pt of its edges; a
  folded group **springs open** after 600 ms under a cable; **Shift+F10** opens the
  selected strip's route menu; *Show the destination* scrolls the strip into view and
  flashes its column without changing the selection.
- The cursor is a grab hand over every drag source, grabbing while carried.

### 2. The route-here plate — `views/mixer_routehere.rs`

On a strip nothing feeds (`Desk::fed`, per revision — sends, keys and notes count, so a
return fed by sends has none): a dashed plate in the silent meter well with a `+`; in Wide
and Extra large it spans the strip so **ROUTE HERE** lies flat under the `+` when it fits
(label role, else micro), runs up the strip when only that fits, and is left out rather
than cut. Quiet at rest (a fresh project has a dozen), lit when pointed at; its hint is
"Route here · Click: route a track or send a strip here". A click opens the route-here
menu and **keeps the selection** the meter's click under it would have changed (ADR 0044
§6), so a return's menu offers "Send ‹the selected strip› here" first. The column under it
is every drag's target.

### 3. The route menu — `views/route_menu.rs`

One module, menus scoped by owner (the desk, the matrix, the inspector each show their
own; ADR 0044 §5):

- **A route's menu** (`open`): a band naming it ("Send · 3 · Keys → 14 · Reverb"),
  *Level ▸* (0, −3, −6, −10, −12, −18, −24 dB, −∞; the current one checked, its value in
  the row), *Send on*, *Post-fader* / *Pre-fader* (radio), the doubled-signal note when
  the destination hears the strip twice, *Make this the output* / *Make this a send*,
  *Open the keyed effect* (keys), *Show the destination / the source*, *Select the strips
  routed here / it feeds*, *Remove* (an output: *Remove (back to the master)*).
- **The add-send list** (`open_add_send`): *New return ▸ Empty · Reverb (Spatium) ·
  Delay* first, then every strip by dock band, checked where the strip (or its whole
  selection) already sends, disabled where it would loop, "heard twice" in the value
  column of a doubled one; a click adds the sends (or removes them when all send already).
- **Route here** (`open_route_here`), by what the strip probably is (design §4.2): a
  channel — *Route "‹selected track›" here*, *Route a track here ▸* (the tracks going to the
  master first), *Route the selected strips here (n)*, *Send a strip here ▸*; a return or
  bus — *Send ‹selected strip› here*, *Send the selected strips here (n)*, *Send a strip
  here ▸*, then the route rows.
- **Which insert to key** and **nothing to key** (the duck and gate offers) after an Alt
  gesture.
- `defer(ctx, owner, Pick)` runs a pick from code that cannot act where it draws (the
  inspector's sheet); `row_rect` / `row_labels` let tests click rows by key or words.
- A submenu that would run off the right opens to its parent's left (also fixed in the
  desk's own menus).

### 4. The desk's menus — `views/mixer_menu.rs`

- The strip menu's *Routing* band starts with **Where it goes… `Shift+F10`** (the route
  menu's key, shown where it is used) and gains an **Its routes** band: the output, every
  send (its level and tap in the value column, its switch as the check) and every key into
  the strip's inserts — a click opens that route's own menu where the strip menu was.
- *Send to ▸* (and the ▼ chip's route menu) start with **New return** (Empty / Reverb /
  Delay), the strip — or the selection it is in — sending into it, one undo step.

### 5. Receives — `views/inspector_receives.rs`

Between *Sends* and *Routing* in the channel sheet: a row per **sender** (its lamp, its
name — a click selects it, a right-click opens the route menu —, a `PRE` chip on a
pre-fader send, a `×2` chip with the doubled-signal hint, the level as the sheet's value
well, dragged as one merging `SetStripSend` gesture), a row per strip **routed into** it
("output", click: its menu — *Make this a send*, *Remove*), a row per **keyed insert**
("2 · Compressor ← key", click: the effect's editor). Fed only by its tracks: "Fed by its
tracks (Input, above)"; fed by nothing: **Nothing feeds this strip — route here**, which
opens the route-here menu. The master stands dimmed. No list is built a frame (the senders
are found by walking the strips; names and doubled flags cached per revision).

### 6. Track and channel payloads — `views/arrangement.rs`, `views/channel_rack.rs`

The arrangement header's reorder drag and the channel rack's channel button (now
click-and-drag; a click still opens the instrument) set `TrackDrag`; the arrangement does
not reorder a track released outside its own view.

### 7. Strings

`locales/areas/routing.{en,es}.txt`: the route menu, the drag words and hint, the plate,
Receives, the strip menu's rows — 71 new keys in both languages.

## Decisions (ADR 0044)

1. Press-then-travel on the raw pointer, egui told something is dragged; no new widget
   over a source. 2. Column answers cached per (revision, drag, modifier, selection); idle
   costs one `Option` check. 3. Feedback before the drop: outline, chip, cursor, hint,
   live rubber band. 4. Tracks travel as an egui payload. 5. One owner-scoped route menu
   with deferred picks (one line added to `inspector.rs`). 6. The plate keeps the
   selection. 7. **Shift+F10** is the route menu's key, read by the desk this round (not a
   keymap action: `keymap.rs` was the matrix builder's). 8. Submenus beside their parent.
   9. *New return ▸* first in add-send lists. 10. The route menu's level is a submenu of
   steps; typing a level is left (below).

## Tests

- `crates/cantus-ui/tests/mixer_routing.rs` — **17**, real frames with pointer and key
  events: a number cell dragged (one step, undo exact), a click still selects, Esc cancels,
  the selection as one block, a strip pinned by a drop on the right dock, a group by its
  separator (one step, undo exact), a track payload dropped on a strip (hint, routed,
  selected, payload taken), the arrangement header and the channel-rack button carrying
  their track, the plate only on an unfed strip (and its hint), the plate's menu routing
  the selected track (and the plate leaving), a return's plate offering the send first,
  New return with a reverb from the add-send list (one step), Shift+F10 and the strip menu
  showing it, Receives (a level drag as one step, the lamp, the name selecting the source,
  the empty well's menu routing a track).
- `views/mixer_drag.rs` tests — **9**, the cable gestures (the press seeded as `source`
  records it): plain / Ctrl / Alt, Alt onto nothing to key (the gate offer), a loop
  refused before the drop with nothing issued, a selection sending as one step, an end
  moved (level kept) and dropped on nothing (removed), each undone exactly; Esc;
  auto-scroll at the middle's edge; the column cache kept across frames; a folded group
  refusing a cable and springing open under it.
- **Seen failing**: the first pass of the gestures was written before these tests, so
  each test was then held against a mutated build (no drop on release, no track drop, no
  spring, no Shift+F10, every column answering *Valid*, the plate's click ignored,
  Receives drawing nothing): 16 of the 26 failed — every test whose gesture was broken;
  the loop test survives "every column Valid" because the drop itself still refuses
  (`route_gesture`), which is the behaviour it pins.
- **Totals**: `cargo test -p cantus-ui -p cantus-core --no-fail-fast` — **931 passed, 0
  failed, 18 ignored** (the pictures' test among the ignored); `cargo clippy --workspace
  --all-targets -- -D warnings` clean; rustfmt on every file touched (all were formatted
  at the base).

## Pictures

`CANTUS_SHOTS=<dir> cargo test --release -p cantus-ui --lib drag::shots -- --ignored`
(`CANTUS_SHOTS_FILTER` to choose) renders the whole window for 14 scenes × English,
Spanish, Paper, 75 %, 200 %, Wide, Extra large: `routing-drag-send`, `-drag-output`,
`-drag-key`, `-drag-loop`, `-drag-remove`, `-reorder`, `-reorder-group`, `-track`,
`-routehere`, `-routehere-menu`, `-route-menu`, `-new-return`, `-receives`, `-strip-menu`.
Looked at, and fixed from what they showed: the chip said its words twice (the house's
value pill — the hint is no longer "live"), the chip elided its own words, a route menu's
band title was cut (bands are measured as painted), a submenu opened over its parent at
the window's edge, the plate's words were cut ("ROU…") on Wide, the plate hint overflowed
the hint bar, clicking a return's plate changed the selection its menu was built from.

## What is left

- **The desk builder wires the cable sources**: `drag::source(v, app, ui, &resp,
  Source::Port(id))` on each port and the send row's ▼ chip, `Source::End { route, end }`
  on lit end marks; `column_hatch` paints `Target::Refused`. The band may ask
  `drag::active` and `drag::dragged_from` to light ports.
- **Typing a level** in the route menu (*Level…* as a field): the menu offers steps; the
  desk's pill, the inspector and Receives drag the exact value.
- ~~**Shift+F10 as a keymap action**~~ — done at integration: `Action::RouteMenu`, keymap
  file version 6, gated on the mixer's edit focus, shown in the strip menu and the Edit
  menu.
- The inspector's own *Add send* list (`inspector_menus.rs`, `inspector_sends.rs` — not
  this builder's) could open `route_menu::open_add_send` to gain *New return ▸*.
- The send row's right-click (`Menu::Arrow`) could open the route menu for an existing
  send (`route_menu::open(ctx, mixer::route_owner(), Route::send(..), at)`).
- No hint on the number cell itself says it can be dragged (the cell's hint is the desk's);
  the grab cursor says it.

## Files

Owned: `views/mixer_drag.rs`, `views/mixer_drag_shots.rs` (new, test-only pictures),
`views/mixer_routehere.rs`, `views/route_menu.rs`, `views/inspector_receives.rs`,
`views/mixer_menu.rs`, `tests/mixer_routing.rs` (new), `locales/areas/routing.*`,
`docs/adr/0044-desk-gestures.md`, this file. Outside the area: `views/inspector.rs` (one
line: the inspector shows its route menus), `views/arrangement.rs` and
`views/channel_rack.rs` (the `TrackDrag` lines, and the arrangement's "no reorder when
released outside" guard).

## Notes for the integrator

- Merges cleanly with the desk if the desk only *adds* calls to `drag::source` and paints
  `column_hatch`; `Source::Port` / `Source::End` carry `#[allow(dead_code)]` until then.
- `mixer_drag.rs` reads `v.desk.{left,middle,right}`, `desk::item_width`, `v.scroll_x`,
  `v.popups` and `Geo` — if the desk renames them, the compiler says where.
- `route_menu::strip_label` is `pub(crate)` ("3 · Keys"): the matrix may want it.
