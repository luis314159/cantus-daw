# ADR 0044 — Desk gestures: one drag machine, owner-scoped route menus, deferred picks

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 (builder *interaction*) ·
**Extends:** ADR 0041 (routing is drawn), ADR 0024 (the kit, §6 the mixer, §13 the
budget) · **Schema:** none (23, piece A's)

## Context

Round 25 makes routing a matter of gestures (design §3.3, §4.1–4.4; the owner's answers
2, 11 and 12): a cable dragged from a strip's port to another strip (plain = a send,
Ctrl = the only output, Alt = a key), a cable's end moved or dropped on nothing, a track
or a channel-rack channel dropped on a strip, strips and groups dragged along the desk,
the *Route here* plate, one route menu wherever a route shows, and the inspector's
*Receives*. Three things shaped how:

- **The controls a drag starts on belong to other code.** The ports and end marks are
  the desk builder's (the cable band), the number cell and the separator already sense
  clicks (select, fold), and egui's hit test gives a click-only widget on top of a
  drag-only one *no* click (`hit_test.rs`): a second, drag-sensing widget over a number
  cell would have killed its click.
- **The inspector's sheet borrows the application immutably** (`receives(sh, app: &CantusApp, …)`):
  a Receives row cannot select its sender or run a route-menu pick where it draws.
- **The meter is sensed after the Route here plate** (it selects its strip on a click),
  so the plate cannot win egui's click either.

## Decision

1. **One drag state machine** (`views/mixer_drag.rs`, kept in `MixerView::drag`). Any
   control calls `drag::source(.., resp, Source)`; a press is recorded from the response
   (`is_pointer_button_down_on` or `drag_started`) and becomes a drag only once the pointer
   has travelled `THRESHOLD` (6 pt) — read from the raw pointer, so the number cell keeps
   its click and the separator its fold, and a source needs to sense nothing new. When the
   drag starts, egui is told something is dragged (`set_dragged_id("mixer-drag")`), so no
   other control hovers or lights under the pointer. **Esc cancels**; a release does one
   `routing_actions` call — one undo step — or nothing.
2. **The columns answer once, not per frame.** `drag::target` / `target_group` are called
   for every visible column; while a drag is on they read a per-strip cache worked out once
   per (revision, drag, modifier, selection) from `Project::route_check` /
   `routing::plan`, and record the column under the pointer (clipped to its dock, so a
   middle strip under a pinned dock is a target only where it shows). The words beside the
   pointer are rebuilt only when that column or the key changes. With no drag, a target
   costs one `Option` check — the desk's idle frame is untouched.
3. **A drag says what it will do before the drop**: the hovered column outlined (lime when
   a drop works, the danger colour and a wash when refused — the desk builder's hatching
   marks every refused column), a chip beside the pointer ("Send to 6 · Strings · 0 dB,
   post-fader", "Would make a loop: Keys already feeds Bus", "Release to remove this
   route"), the hint bar naming the modifiers, the cursor (grab over a source, grabbing
   while carried, not-allowed over a refused column), a rubber band styled as the route it
   would make (output solid, send thinner, key dashed in the dynamics hue), the modifiers
   read live, the middle scrolling within 24 pt of its edges, a folded group springing open
   after 600 ms under a cable. Strips and groups show the lifted columns washed and a line
   in the gap they would land in.
4. **Tracks and channels travel as an egui `DragAndDrop` payload** (`TrackDrag`, piece A's
   type): the arrangement's header and the channel rack's channel button set it when their
   drag starts; the desk's columns answer it like any drag; the arrangement no longer
   reorders a track released outside its own view (it was routed on the mixer).
5. **One route menu module, menus scoped by owner** (`views/route_menu.rs`): a view opens a
   menu under its own id and calls `show(app, ui, owner)` once a frame; the state lives in
   egui's memory, the rows are built while it shows. A view that borrows the application
   immutably hands over a pick with `route_menu::defer`, run by its owner's `show` the same
   frame. The inspector calls `show` for its owner (`route_menu::inspector_owner`) — one
   line in `inspector.rs`, after the sheet's acts.
6. **The plate reads the pointer itself** and, when clicked, puts back the selection the
   meter's click under it changed (after the docks), so its menu acts on the musician's
   selection ("Send Keys here"), not on the plate's strip.
7. **The route menu's key is Shift+F10** — the platform's context-menu key, free in every
   preset, never a letter (the typing piano keeps those). At integration it became the
   keymap's `Action::RouteMenu` (`route-menu`, category *Mixer*; keymap file version 6,
   `NEW_IN_V6` fills it into older files): rebindable in the shortcuts dialog, acting only
   while the mixer is what is being edited (`EditFocus::Mixer`), the desk opening the menu
   on its next frame where the ▼ chip is. The strip menu's *Where it goes…* and the Edit
   menu's *Route menu of the selected strip* show the bound key.
8. **A submenu never lies over its parent**: one that would run off the right opens to the
   parent's left (route menu and the desk's menus) — over it, a click meant for the
   parent's row picked the submenu's.
9. **New return ▸ comes first** in every add-send list (the design put it last): on a
   64-strip desk the last row is a scroll away.
10. **Click mode** (integration, the owner's answer 2: "release **or click** on the
    destination"). A click — a press with no travel — on a strip's port or the selected
    strip's ▼ chip **arms** a route from that strip (`Drag::armed`): the port gets a ring
    and a lit core in the selection's colour, the ▼ chip a lit outline. While armed the
    drag machine behaves as for a carried cable with no button held: every column answers
    (`Drag::what` yields the armed port), refused columns are hatched, the hovered column
    outlined, the rubber band follows the pointer, the chip says what a click does, the
    hint bar names the modifiers ("Click: send · Ctrl: output · Alt: key · Esc: cancel", short enough to read whole in the hint bar; the drag's is "Drop: …" likewise), the cursor is a crosshair (not-allowed
    over a refused column). The **second click** on a strip that takes the route makes it,
    with the modifiers read at that click — the same `route_gesture` call, one undo step;
    on a refused strip it stays armed (the chip says why); on empty space, its own strip,
    outside the desk, a right-click, or Esc, it is let go. A **cover** (`ui.interact` over
    the whole desk, registered after every control while armed) takes the completing
    click, so the fader, mute or name under it never acts. The ▼ chip's destination list
    moved to its right-click (the output chip's click still opens it). A lit cable's end
    mark is kept off the port beside it, so a press on a port always starts a new cable.
    The matrix needs no click mode: a cell already is one click.

## Consequences

- No new `Command`, no schema change: every gesture is a `routing_actions` batch from piece
  A. The cable gestures' sources (ports, end marks, the send row's ▼ chip) are wired by the
  desk builder with `drag::source(.., Source::Port | Source::End)`; until then the cable
  gestures are driven in `mixer_drag`'s tests by seeding the press `source` would record.
- `Drag`, `Source`, `Target`, `active` and `route_menu::{open, open_add_send,
  open_route_here, is_open, show}` keep the contract's signatures; added public helpers:
  `route_menu::{inspector_owner, plate_id, close, row_rect, row_labels}` (tests and tools
  click rows by key or words).
- Shift+F10 is a keymap action (rebindable), acting while the mixer is being edited.

## What the tests hold

`crates/cantus-ui/tests/mixer_routing.rs` (17) and `views/mixer_drag.rs`'s tests (9),
listed in `docs/round25/interaction.md`: plain / Ctrl / Alt cables, Alt's two menus, a
loop refused before the drop with nothing issued, a selection sending as one step, an end
moved (level kept) and dropped on nothing (removed), each undone exactly; Esc; auto-scroll;
the column cache; strips, the selection and a group dragged (one step, docks respected, a
click still selects, Esc cancels); a track and a channel dropped on a strip; the plate (on
an unfed strip only, its hint, its menu routing the selected track, a return offering the
sends first); New return with a reverb; Shift+F10 and the strip menu showing it; Receives
(levels dragged as one step, the lamp, the name selecting the source, the empty well's
menu).

## Amendment — round 26 (builder *mixer*, 2026-10-13): click mode and the hidden desk, a typed level, one add-send list

**Context.** Round 25 left: an armed route survived the desk being hidden and was still
armed when it came back (letting it go on hide was tried and dropped — it also dropped a
selected cable the musician expected to keep); the route menu's level was steps only
(§10 of the decisions list: "typing a level is left"); the inspector's *Add send* was a
list of its own without *New return ▸*.

**Decision.**

11. **The desk was hidden — a signal of its own.** The desk notes the egui pass it was
    drawn on (`MixerView::shown_pass`); a gap when it is next drawn means it left the
    screen in between (its pane closed, the Routing tab brought over it, any tab). Then
    what it was in the middle of lets go — an armed route, a press not yet a drag, and
    anything carried (`drag::desk_was_hidden`); a button still held through the hide is
    stale, and no source takes it up again until every button is up (egui still reports
    it down on the control it started on) — and nothing else: the selected cable is the
    band's and stays, so Delete removes it once the desk shows it again (round 25's guard: Delete
    only removes a cable drawn on the last pass).
12. **A level of one's own** (§10 amended). *Level ▸* opens with **Type a level…**: the
    menu becomes a field where it was — the route's band, the kit's numeric entry (a lit
    well, the value face) prefilled with the level and selected, so what is typed replaces
    it, and a caption with the range and the keys. The field holds the keyboard for as
    long as it is open — a click on the panel around the text box does not hand the keys
    back to the app, whose shortcuts (Ctrl+Z) would act under it. Enter sets the level in
    one undo step (`SetStripSend` + end of gesture) and, on the prefill untouched, nothing
    at all; only decibels read — a number, "dB" after it or nothing, a decimal comma or
    point, a plain or typographic minus; any other unit ("50%", "-6k", "-6 Hz") is no
    level; above 0 dB clamps to unity (a send carries at most the whole signal); −∞, or
    anything at or below the faders' floor (−96 dB), is silence; words that are not a
    level keep the field, selected again, the caption saying what to type; Esc cancels and
    a click outside sets what was typed, as Enter does (third review: the kit field's one
    rule, ADR 0024 §15); that Esc is the field's alone — the cable the right-click selected
    stays selected (the band lets a cable go on Esc only when no field or route menu took
    the key). Selected text is drawn on the theme's selection fill (ADR 0046). The field
    belongs to the route menu, so it is the same from the desk, the matrix and the
    inspector (`route_menu::open_level` opens it directly for tools).
13. **One add-send list.** The inspector's *Add send* (the empty Sends row and the band's
    +) opens `route_menu::open_add_send` under the inspector's owner — *New return ▸*
    first, then every strip by dock, the selection's sends when the strip is in it, a
    doubled signal said — the desk's list through the same code; the inspector's own list
    (`MenuId::AddSend`) is gone.

**What the tests hold.** `mixer_routing::an_armed_route_lets_go_when_the_desk_is_hidden_and_a_selected_cable_stays`
(whole window; the mixer pane closed, then the Routing tab over it; seen failing: still
armed), `what_the_desk_carried_lets_go_when_it_is_hidden` (a cable carried, a press not
yet travelled, a strip by its number cell: hidden with the button held, released after the
desk is back — no command; seen failing on the press: `cmd.add_send`),
`the_level_field_keeps_the_keyboard_while_it_is_open` (seen failing: Ctrl+Z removed the
send), `esc_on_the_level_field_keeps_the_selected_cable` (seen failing: the cable let go), the prefill case in the typed-level test (seen failing at a level of 0.3), `a_level_typed_in_the_route_menu_sets_the_send_in_one_step` (prefilled and
selected, −4,5 read, one undo step, +6 clamps, −inf / −∞ / −120 silence, words keep the
field, Esc cancels; seen failing: no such row), `route_menu::tests::a_typed_level_reads_as_decibels_clamped_to_unity`,
`the_inspectors_add_send_offers_a_new_return` (the empty row and the band's +, New return
first, a reverb return in one undo step, a strip from the list; seen failing: no route
menu opened).
