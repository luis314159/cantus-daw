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
7. **The view has options** (amended 2026-10-11, the owner: "an option to show only the
   current channel's sends, or more view options, to see more or less information as one
   wants"). `DeskPrefs::cable_opts: CableOpts` beside the *Off · Selected · All* switch,
   set from a kit menu behind a funnel key right of the switch (lit while anything differs
   from the default; in the folded title row it follows the single cables key):
   - **Whose cables** (`CableScope`): of the selected strips, what goes *out* (their
     output, sends, keys), what comes *in* (their feeders' outputs, sends and keys
     arriving at them), or **both** — the default, round 25's behaviour. In *Selected*
     the scope decides what is drawn, in *All* what is lit; hovering a strip follows it.
   - **Which kinds**: outputs, sends, sidechain keys, each on or off. A hidden kind is
     filtered before anything else in `collect`, so it is neither drawn nor hit-tested;
     a selected cable whose kind is hidden is let go, and Delete (which only removes a
     cable drawn on the last pass, `deletable_cable`) never removes it.
   - **Detail**: *Send levels* (the pills, their *PRE* tag and the selected send's tap
     chip; the cable keeps its pre-fader gap) and *Outputs to the master* (one per
     selected strip, all converging on the master — noise with a big selection).
   - **Quick views**: *Everything (default)* — the one-click way back —, *Only the
     selection's sends* (the owner's case: out, sends only) and *Only what feeds the
     selection* (in, every kind); the last two also put the view on *Selected*. The
     radio shows which quick view the options are, none when custom.
   - The switches keep the menu open (several set in one visit while the band redraws
     under it); a quick view closes it; a second click on the key closes it too.
   - Under ROUTING, the legend dims a hidden kind's sample and word, and its hover names
     the hidden kinds ("Hidden by the cable options: Outputs, Sidechain keys"), so a
     missing cable is never a mystery.
   - Persistence: `#[serde(default)]` on `DeskPrefs` and on `CableOpts`, so a
     `mixer.json` written before the options (or with only some of them) reads as the
     default. View preferences, not project data: no schema change, no `Command`.

## Consequences

- `kit::cable` has `path` (and `Path { low, run_start, mark, tail }`), `ink` and `width`
  beside the contract's `depth_for`, `paint`, `hit`, `key_mark`, `doubled_mark`; the
  matrix can draw the same marks.
- The design's cubic "arc diagram" wording in ADR 0041 §6 reads as this U from now on.
- The title row's cable switch folds to a single key (a click steps the view) when the
  row has no room; the desk-wide *Solo off* / *Unmute all* keys give way before they
  overlap the left side (both stay in the desk's menu).

- The options cost nothing per frame: one more test per edge in `collect` (a match on
  its kind), no allocation (the legend's hidden-kinds sentence is built only while
  hovered, the menu's rows only while it is open).

## What the tests hold

`kit::cable` unit tests (depth grows and caps; the path runs port to port below them;
nested cables never cross; the hit test), `mixer_layout` tests (rows per mode, the drop
order, saved bits), `mixer_desk` tests (the band's cables port to port in every view, PRE,
the doubled hint, select / Delete / undo, the pill's drag, edge chips, Extra large's send
rows, no overlaps at 75 % and 200 % in both languages), `frame_budget` (no allocation, no
draw call, *Selected* draws only the selection's), the kit gallery's `cable` sheets;
§7: `mixer_cables_view` (the options round trip through `mixer.json` and an old desk
reads as today; only sends shown draws and selects no output or key, in *Selected* and
*All*; *in* draws the feeders and lights only them in *All*; Delete on a hidden kind does
nothing; no pills or *PRE* tag without send levels, no master route without it; the menu
by real clicks — a kind stays switched with the menu open, a quick view resets and
closes, the key toggles it; the legend names hidden kinds; the title row never overlaps
at 1280 px, 75/100/200 %, English and Spanish, with the desk-wide keys showing; every
word in both languages), `mixer_layout` (defaults and quick views).
