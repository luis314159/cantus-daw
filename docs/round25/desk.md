# Round 25 — builder *desk*: cables on the desk (piece B) and Extra large (piece E)

**Branch:** `r25/desk` (base 57f3ccc), worktree `F:/projects/daw-r25/desk` · **ADR:**
`docs/adr/0043-cable-band.md` · design §3.1–3.2, §4.5, §6, §9; the owner's answers 1, 3,
4, 5, 6, 7.

## What was done

### The cable band (`views/mixer_cables.rs`, `kit/cable.rs`)

- A new bottom row, **ROUTING** (`Row::Cables`), in every width mode while the desk's
  cable view is on. Every strip, the master and a collapsed group's separator have a
  **port** at its top: a ring in the strip's colour, filled when the strip goes anywhere
  but the master, larger while chosen or while a drag is on.
- **Cables** are soft U's (ADR 0043): an output 2 pt solid with a dot, a send 1.5 pt with
  a chevron into the port (dashed and faint when off; a gap at its source when
  pre-fader), a key 1.25 pt dashed in the dynamics hue with the key glyph. Lit cables get
  **lanes** so no two share a flat run; nested ones never cross.
- **What shows** — the title row's switch *Off · Selected · All* (**Selected by default**,
  answer 1): the selection's cables (and its route to the master, as FL draws it), or
  every route but the implied master ones with those away from the selection faint.
  Hovering a strip or a cable lights it; the hint bar names it ("3 Snare → 15 Reverb ·
  send · −6.0 dB · pre-fader", plus the doubled-signal sentence when it is one).
- **Level pills** on lit sends (Wide and Extra large; Compact on hover or selection):
  "−6.0 dB", with a **PRE** tag in the cable's colour for a pre-fader send (answer 4);
  dropped when they would cover a port or another pill. Drag = level (house rules,
  double-click 0 dB), click selects the cable, right-click opens the route menu; the
  selected send's pill has the `POST | PRE` switch (`kit::tap::switch`) beside it.
- A **doubled send** (answer 5) carries the warning mark where its flat run starts, in
  the band and in the send row; its hint says the destination hears the strip twice.
- **Edge chips** "‹ 37" / "37 ›" for ends scrolled out of the middle (lit cables; stacked
  when several go one way); a click scrolls the strip into view.
- **Select and act**: a click selects a cable (halo), Delete/Backspace removes it (one
  undo step, `remove_route`), Esc lets it go, right-click opens `route_menu::open` under
  `mixer::route_owner()`.
- **Drag sources** for the interaction builder's machine, as BUILDING.md asks:
  `Source::Port` from every port, the selected strip's ▼ chip and the "+" send row;
  `Source::End { Destination }` from lit end marks and `{ Source }` from the selected
  cable's tail dot. `column_hatch` paints a valid column (selection tint and outline) and
  a refused one (hatched in the danger colour). `drag::active` lights the ports.
- The band's height has a 3-dot handle in the legend (20–64 pt, default 40,
  double-click resets), kept per layout like the width mode.

### Extra large and the rows (`mixer_layout.rs`, `mixer_strip.rs`, `mixer_parts.rs`)

- **No insert slots anywhere** (`Row::Inserts`, `inserts`, `slot_drop`,
  `Desk::most_inserts`, `Drop::FewerSlots` gone). The inserts are the inspector's.
- **FX CHAIN** (Extra large): the chain in one line, "EQ · Comp · +1" (answer 7), dimmed
  while the chain is switched out; a click chooses the strip and opens the inspector
  (`parts::open_chain`); an empty chain shows a "+" that opens the effect picker;
  right-click opens the menu of the insert under the pointer (or the quick add list).
  The master shows its own chain the same way.
- **OUTPUT** (Wide and up): a chip "→ 9" ("→ 9 Drum Bus" at Extra large, "→ M" to the
  master); a click opens the strip's route list. On the master this row carries the pan
  law (it has no output chip).
- **SENDS** (Extra large): the strip's own sends, one row each — lamp (switch), the
  destination's name, the `POST | PRE` switch (`kit::tap::switch`, the matrix builder's),
  the level as a bar in the space hue; drag = level, click on the name selects the
  destination, right-click the route menu; a "+" row after the last send (click: the
  add-send list; drag: a cable); more sends than rows end in "+n" (opens the inspector).
  Rows follow the busiest strip, 2 to 6.
- **FROM SELECTED** (every mode): FL's arrow row kept (answer 6) under its new legend; an
  existing send's knob now opens the route menu on right-click.
- **Drop order** when short: EQ → width → phase/swap → readout → name shrinks → send list
  to two rows → band to 20 pt → output chip. Saved row switches keep their meaning
  (ADR 0043 §5).

### The title row and the furniture (`mixer_paint.rs`)

- The cable view switch (glyph + segmented); folded to one key that steps the view when
  the row has no room. *Solo off* / *Unmute all* now give way before they overlap the left
  side (they overlapped the width modes at 200 % before this round).
- The wheel: Ctrl over a send row moves its level; the plain wheel over the band scrolls
  sideways like the head rows.

### Kit

- `kit::cable` (contract stub filled): `depth_for`, `path`, `paint`, `hit`, `ink`,
  `width`, `key_mark`, `doubled_mark`; registers its tokens for inspect mode.
- `Brush::dashed(pts, w, color, dash, gap)` (moved from `fm_graph.rs`, which now calls
  it); glyphs `Key` and `Cables`; a `cable` sheet in the gallery.

### The desk's cache (`mixer_desk.rs`)

Per revision and language: `edges` (`routing::edges_into`), `ends` (each edge's project
indices), `first_edge`, `doubled`, each strip's `place` in its dock (a folded member at its
separator), `Info::goes`, the chain lines, `most_sends`. A frame visits the edges once.

## Measurements

- 64 strips, 129 sends, 16 keys, *All* (146 cables): 19 allocations a frame either way
  (band off or all), 9 draw calls either way (the band shares the right dock's flush — the
  desk has one draw call fewer than before), 97 k triangles; release build + tessellate
  p50 **1.68 ms** against 1.48 ms for the plain 64-strip desk (+0.2 ms, design target
  ≤ 0.3 ms; budget 4 ms). Idle with cables on: no repaint.

## Tests (16 new, 1 new ignored timing)

`kit::cable` 4 · `mixer_layout` 3 new (Extra large rows, its drop order, saved bits) and 4
rewritten · `mixer_desk` (lib) 1 · `tests/mixer_desk.rs` 6 (port-to-port cables in every
view, PRE and the doubled hint, select/Delete/undo and the pill's drag, edge chips, Extra
large's send rows, no overlaps at 75 %/200 % EN/ES) and 3 updated · `tests/frame_budget.rs`
2 (+1 ignored timing) · the gallery's `cable` sheets (6 golden images). Seen failing first:
the chain line elided at 76 pt (fixed with thin separators), a pill's text elided, the band
costing one draw call, the cable sheet's unregistered tokens; the geometry tests were
written with the painter.

`cargo test -p cantus-core -p cantus-project -p cantus-engine -p cantus-ui --no-fail-fast`:
**1 367 passed, 0 failed, 25 ignored**. `cargo clippy --workspace --all-targets -- -D
warnings`: clean.

## Pictures

`cargo run --release -p cantus-ui --example shoot_mixer -- <dir> <filter>` (a filter ending
in `$` matches that variant only). New variants: `-cables` (Wide, the grown desk with a
bus, a pre-fader send, a switched-off send, a doubled send and a key), `-cables-compact`,
`-cables-all` and `-cables-all-compact` (the house demo), `-cables-allgrown`,
`-cables-scrolled` (64 strips: the edge chip), `-cables-es`, `-cables-75`, `-cables-200`,
`-cables-off`; `-xl` and `-xl-es` now on the routed desk. Looked at in both themes, English
and Spanish, 75 % and 200 %.

## Files outside my area

- `views/mixer_drag.rs` (interaction's): removed the two `#[expect(dead_code)]` the
  contract said to remove once used (`Source`, `active`). Interaction's branch replaced
  them with `#[allow(dead_code)]`: **take interaction's version** at the merge.
- `views/inst/fm_graph.rs`: its dashed polyline now calls `Brush::dashed`.
- `tests/device_ux.rs` (the slots test became the chain line's), `tests/master_chain.rs`
  (the master's chain line), `tests/app_flows.rs` (legend words), `examples/shoot_devices.rs`
  (its `slot` case is `chain`): consequences of the slots leaving the desk.

## Notes for the integrator

- The hooks were used as declared, with one change inside my own files: `band()` takes the
  frame's `Pass` instead of `Style` (it needs the rows); only `mixer.rs` calls it.
- **Delete on a selected cable**: the keymap's Delete runs before the panels and removes
  selected notes; the band removes the cable only when the mixer has focus and no notes
  are selected. Routing Delete to `MixerView::selected_cable()` in `app.rs`'s Delete arm
  would make it exact (three lines).
- `kit::tap::switch` is still the matrix builder's stub on this branch, so the send rows
  and the selected pill reserve its place but show nothing until the merge; after it,
  look at `-xl` for the switch's fit in 20 × 11 pt.
- `Control` lost `Slot(n)` and gained `Chain`, `Output`, `SendRow(n)`, `SendAdd`, `Port`.
- `mixer_menu.rs`'s *Rows* submenu lists `Row::TOGGLES`, which now has the chain, output
  and send list; the band follows the cable view (not a row switch).

## What is left

- The design's "×n" chip on a collapsed group's separator (its members' cables do gather
  at its port).
- Key cables carry no effect name label ("Comp 2"); the hint bar says "sidechain key".
- At 1280 × 720 the desk is short enough that the band sits at its 20-pt least, with the
  lanes close together; a taller mixer pane reads much better.
