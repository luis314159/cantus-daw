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

## Amendment — round 26 (builder *mixer*, 2026-10-13): steady lanes, a key that always shows, a cost put where it is

**Context.** Round 25 left three things about the band (`docs/round25/integration.md`
"What is left", `cables-view.md`): hovering a strip lit its routes and re-shared the
lanes, so an end mark moved under a pointer coming to it; at 1280 × 720 the band sits at
its least (20 pt) and the legend's three sample lines did not show; the Spanish legend
said SALIDA under RUTEO beside Wide's SALIDA row. And the budget test credited "the hint
bar" with the ~17 allocations a hovered control costs.

**Decision.**

2. (amended three times) **Where the cables hang: the selection's lanes reserved, the
   hover's cables fitted around them** (`views/mixer_lanes.rs`). A drawn cable is
   `laned` when the selection (or a selected cable) lights it. *First* the laned cables
   take their lanes among themselves (round 25's rule: shortest first, each below every
   laned cable its span overlaps; the lanes share the band's depth) — at least 0.6 pt
   apart: in a band too short for every lane, the selection's longest cables (the
   highest lanes) yield, drawn faint at their span's depth. A lone laned cable keeps its
   span's depth. Nothing of this depends on the pointer, so an end mark never moves under
   an approaching pointer. *Then* each cable the hover alone lights (shortest first) hangs
   at the depth **nearest the one its span asks for** — where a laned cable alone would
   hang — that keeps 1.5 pt from every lit cable whose span it overlaps, **leaves its end
   mark a place** among the marks already shown (third review: the 4-pt same-port rule
   it replaces held nothing a test could see — a cable fitted 1.5 pt away silently lost
   its mark), and that **nests**: below every lit cable its span
   contains, above every one that contains it, pair by pair where the band has the depth.
   Where the selection leaves no depth to nest at (a strip's port inside a laned cable's
   span, under a lane at the floor) crossing is needed and only the separations hold;
   where not even they can be had, the hover cable **yields**: drawn *related* — in its
   colour, thin, without a mark — so a hovered bus still shows everything feeding it, as
   many with marks as its port's corner holds. Every hover cable that does not yield
   shows its mark (the property asserts it). Measured over 2 000 random desks, a third of
   them with real fan-in (a bus fed by 8–32 strips, hovered):

   | Desks | Band < 40 pt: yield / nesting given up | Band ≥ 40 pt: yield / nesting given up |
   |---|---|---|
   | no bus (1 126 / 1 404 hover cables) | 26 % / 10 % | 0.2 % / 9 % |
   | a bus hovered (6 196 / 7 346) | 87 % / 5 % | 55 % / 26 % |

   A bus's inputs all end on one port: its corner holds a few marks a glyph apart, the
   rest read as the bus's by colour. **Marks** (third review): a mark rides up its
   destination's corner — half its depth up, else the nearest clear height lower on it,
   never past its bottom, off the port's ring — clear of every mark already shown by the
   two glyphs' reach plus 1 pt (`glyph_r`: an output's dot 2.8 pt, a send's chevron 4.4,
   a key's 6.0). In order: the selected cable (always shown), the selection's (shorter
   first), the hover's (placed with their depth, above), the faint where they fit. The
   selection's marks left out where its lanes crowd a corner: 7.7 % (681 of 8 804; the
   property bounds it at 20 %). **Grab areas**: every lit shown mark not at an edge chip,
   and the selected cable's tail, gets a square that holds the mark, keeps clear of every
   other shown mark's glyph and every other area, and stays off every port's hit area (a
   press on a port always starts a new cable) — the selected cable's excepted from the
   port only; a lit mark that cannot have one is left out (never the selected cable's). **The hit test** takes the nearest
   cable; the layer drawn on top (the selection's over the hover's over the faint) breaks
   a tie only (cables within 0.05 pt there), then the shorter — so every lit cable is the
   one picked on its own run, and a faint one wherever no other lies exactly on it.
   **Cost**: O(n·m log m) for the hover's m cables against the n drawn, the marks worked
   out once a frame, every list's capacity kept — no allocation on a still frame; a
   32-input bus hovered on the 64-strip desk costs no allocation over a plain strip and
   0.17–0.19 ms (release, fastest frames; the search for a depth with a place for the
   mark, third review; held under 0.3 ms by its timing test). The first cuts of this round (hover cables at their span depth,
   then fitted from the floor up with the layer always winning) buried marks, collapsed
   long cables onto the ports and took the point from the cable under it: the reviews'
   findings, held now by a property over random desks.
8. **The routing is never out of sight.** While the desk is scrolled away from its foot
   (a short window: 1280 × 720 in Compact, any window at 200 %), the band — its ports, its
   cables, its legend cell — is **pinned to the bottom of the desk's visible rows**, a
   hairline at its top, and the strips' rows scroll under that edge (`pin_band`;
   `Geo::strips` is the strips' clip, one more draw call only while pinned). At the foot
   of the scroll the band is where it would be anyway. The band's legend cell is painted
   with the band, on top. Under ROUTING the full key (three samples with their words)
   needs the band's default 40 pt; below that, down to the least of 20 pt, a **compact
   key**: the three samples side by side after the name and under the band's height
   handle — an output at twice a send's weight, a key's dash and its key mark — never a
   stack that reads as a menu glyph; ROUTING starts where every row's name does. The
   hover still puts the whole sentence in the hint bar. `MixerView::cable_key()` reports
   where each went (`CableKey`), `MixerView::rows_seen()` what of the desk is visible.
   The pinned band is the band everywhere: it stops at the vertical scrollbar (whose
   track stays the bar's), the wheel over it does what it does over the band at the foot
   (sideways; Ctrl steps the width), and nothing hidden under it (the meter/fader ratio
   handle) is grabbed through it. When the desk holds the band at less than asked (too
   short a desk), it shows **the most of the band that fits** (never a jump to the least)
   and the band's height handle is never inert (third review): a drag starts from the
   band as it shows, so a drag down acts at once and a too-tall preference can be lowered
   to one that fits, while a drag up stops at what fits — no preference is written that
   nothing would show. Its dots dim and its hint says the desk is what holds it.
9. **Words in the legend column never read the same, and name a kind as everywhere
   else.** In Spanish the thick line is *salida* — as the cable options (*Salidas*), the
   hover (*salida*) and the route menu call it — and Wide's row, which shows where each
   strip goes ("→ 9"), says *Destino*. A test holds every legend word distinct in both
   languages, the words a strip or the band paints apart from them, and each line's word
   inside its kind's name in the hover and the options.
10. **What a hover costs, measured where it is** (§6, amended). Traced allocation by
    allocation (a backtrace per allocation, a debug-symbol build of `cantus-ui`): the ~16
    allocations a hovered desk control adds are egui's — its hit test's and interaction's
    lists, and in builds with debug assertions its `on_widget_under_pointer` plugin hook
    (one plugin-list clone per widget under the pointer; release builds skip it). The hint
    bar and the desk add none; a hovered cable's "one more" is the band being one more
    widget under the pointer. Measured: the 64-strip desk, Wide, a fader hovered — 20 → 36
    allocations in a debug-assertions build, 20 → 27 in release, exactly a bare egui frame
    of the same shape both times; in release a hovered cable costs what a hovered fader
    does (27 = 27).

**What the tests hold.** `mixer_desk::hovering_a_strip_never_moves_the_selections_lanes`
(Selected and All; the pointer on another strip, on the band under the mark's column, on
the mark: the selection's cables keep depth and ends — seen failing: 5.0 → 10.375; no two
lit cables share a flat run or an end mark — seen failing: 2→15 and 5→9 at 26.5; grabbing
the selection's mark moves the selection's route and no other),
`mixer_lanes::tests::the_placement_keeps_every_promise_on_random_desks` (2 000 desks: 4–40
strips at three pitches, outputs, sends and keys with shared ports and nested spans, a
selection, a hovered strip, All and Selected, bands of 20–64 pt — the selection's lanes
unmoved by the hover, every lit cable picked on itself, marks clear of each other by both glyphs'
reach + 1 pt, each with a grab area of its own off the ports, a lone hover cable at its span's depth, no shared run, no
needless crossing, and the share that yields), `mixer_desk::the_hovers_cables_are_pointable_marked_apart_and_hung_by_their_span`
(seen failing on the previous placement: the marks into strip 10 1.3 pt apart),
`frame_budget::a_hovered_bus_with_many_inputs_costs_no_allocation` (and its timing);
`mixer_routing::the_pinned_bands_legend_cell_grabs_nothing_hidden`,
`the_scrollbar_track_beside_a_pinned_band_pages`, `the_wheel_over_a_pinned_band_acts_as_over_the_band`
(each seen failing: the ratio moved, the click did not page, the wheel scrolled down);
`mixer_routing::the_cable_legend_shows_when_the_band_is_at_its_least` (whole window,
1280 × 720 at 100 % and 200 %, Compact and Wide, English and Spanish: the band's cell
inside the desk's visible rows and the window — seen failing without the pin, the cell at
y 683–703 under a view ending at 687 —, the compact key a row of samples inside the cell,
off the name, each other and the handle, ROUTING aligned and whole);
`mixer_desk::the_legend_words_never_read_the_same` (seen failing on SALIDA, then on
DESTINO against the kind's name); `frame_budget::the_hint_bar_costs_no_allocation` and
`a_hovered_desk_control_costs_only_what_egui_charges_for_a_hover` (the desk's hover delta
no more than a bare egui frame's of the same shape — 16 = 16 in a debug-assertions build;
each seen failing under a one-`format!` mutation).

Third review: the property's generator has real fan-in (a third of the desks: a bus fed by
8–32 strips, hovered) and asserts every non-yielded hover cable shows its mark, every grab
area holds its own mark clear of every other shown glyph, area and the selected cable's
tail, and at most 20 % of the selection's marks left out — failing under the reviewer's
M3 in this design's form (a hover depth taken without a place for its mark: "hover … shows
no mark") and with the glyph reach shrunk to 1 pt (marks meet);
`mixer_desk::two_short_cables_into_one_port_both_show_their_marks_apart` (8 → 10 selected,
9 → 10 hovered, 40 and 20 pt: both marks shown, a glyph apart, or at 20 pt the hover's
yielded related — failing under the same mutation);
`frame_budget::a_hovered_bus_with_many_inputs_costs_no_allocation` (all 32 inputs drawn,
each lit with its mark or related); `mixer_desk::the_band_handle_never_traps_the_band`
(a 64-pt wish on a desk with room for 40 shows the most that fits; a drag down shrinks it
at once; a drag up past the room, released, then down, shrinks it again — seen failing:
the handle inert, the band at 20) and `mixer_layout::tests` (the band shortened by just
what is missing: 47 and 28 pt).
