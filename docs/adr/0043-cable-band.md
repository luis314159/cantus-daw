# ADR 0043 — The cable band: soft U cables in lanes, a desk preference, rows that keep their bits

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 (builder *desk*) · **Extends:**
ADR 0041 (routing is drawn), ADR 0024 (the kit, §6 the mixer, §13 the budget)

## Context

ADR 0041 puts the routes in a band under the strips "as arcs whose depth follows their
span". Built that way (a cubic per cable, depth ∝ span, capped by the band), two problems
showed at once in the pictures: in a 32–40-pt band every long cable reaches the cap, so
all long cables share one flat line and their pills and marks belong to nobody; and an
output and a send between the same two strips (a doubled signal, answer 5) were the same
curve. A hard cap also breaks the arc diagram's promise that nested cables never cross.

## Decision

1. **A cable is a soft U**, not a cubic arc (`kit::cable::path`): straight down from the
   source port through a quarter curve as wide as the cable is deep, a flat run, and a
   quarter curve back up into the destination port. Depth grows with span (0.35 pt per
   pt, 5 pt least, capped by the band). With corners as wide as the depth, **two cables
   from one port never cross when their spans nest**, cap or no cap (property-tested over
   spans, bands and both directions).
2. **Lit cables get lanes** (`mixer_cables::lanes`): shortest first, each takes the lane
   below every lit cable its span overlaps; the lanes then share the band's depth. No two
   lit cables share a flat run; an output and a send to the same strip are two lines; a
   nested cable hangs above its outer one. Faint cables (*All* mode, away from the
   selection) keep their span depth with a kind share (output 1, send 0.78, key 0.6).
3. **Marks ride on the path**: the end mark 30° up the destination's corner (a dot, a
   chevron pointing into the port — hollow when off — or the key glyph), the doubled
   warning where the flat run starts, the level pill at the run's middle (dropped when it
   would cover a port or another pill), a pre-fader send's gap at its source and its pill's
   *PRE* tag in the cable's colour.
4. **The view is a desk preference** (`DeskPrefs::cables: CableView { Off, Selected
   (default), All }`, `DeskPrefs::band` 20–64 pt, default 40), kept per window layout
   like the width mode; never project data. The band row exists while the view is not
   *Off*; it shrinks (send list first, then the band to 20 pt, then the output chip) but
   never drops while on.
5. **Row bits are fixed, not positional** (`Row::bit`): the round-25 rows (chain, output,
   send list, band) took the insert slots' old bit 12 and the free bits after it, so a
   desk saved before the round reads the same — a desk that had hidden the insert slots
   hides the chain line that replaced them.
6. **Cost**: the band reads the desk's per-revision cache (edges with their project
   indices, each strip's place in its dock, the doubled flags), strokes into the desk's
   mesh in the right dock's flush (no draw call of its own) and keeps every list's
   capacity. Measured: 64 strips, 129 sends and 16 keys in *All* — 146 cables, 0 extra
   allocations, 0 extra draw calls, +0.2 ms build + tessellate (1.48 → 1.68 ms p50,
   release).

## Consequences

- `kit::cable` has `path` (and `Path { low, run_start, mark, tail }`), `ink` and `width`
  beside the contract's `depth_for`, `paint`, `hit`, `key_mark`, `doubled_mark`; the
  matrix can draw the same marks.
- The design's cubic "arc diagram" wording in ADR 0041 §6 reads as this U from now on.
- The title row's cable switch folds to a single key (a click steps the view) when the
  row has no room; the desk-wide *Solo off* / *Unmute all* keys give way before they
  overlap the left side (both stay in the desk's menu).

## What the tests hold

`kit::cable` unit tests (depth grows and caps; the path runs port to port below them;
nested cables never cross; the hit test), `mixer_layout` tests (rows per mode, the drop
order, saved bits), `mixer_desk` tests (the band's cables port to port in every view, PRE,
the doubled hint, select / Delete / undo, the pill's drag, edge chips, Extra large's send
rows, no overlaps at 75 % and 200 % in both languages), `frame_budget` (no allocation, no
draw call, *Selected* draws only the selection's), the kit gallery's `cable` sheets.
