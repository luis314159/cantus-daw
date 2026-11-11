# Round 39 — builder `el-mixer`: the mixer and its inspector, as des-mixer drew them

Branch `r39/el-mixer` (worktree `F:\projects\daw-r39\el-mixer`), based on `r39/integration`
(the Slate kit, ux-a, ux-b, memory). The approved sheets:
`des-mixer/docs/round39/design/des-mixer/` (README = the measured spec); the icons: set 3
(`icons/docs/round39/design/icons/glyphs-m-mixer*.js`, `spec-mixer.md`).

## What changed

### The strip (des-mixer's rows, in signal order)

- **Rows** (`views/mixer_layout.rs`, `fit`): the sound — head, inserts, EQ, polarity · swap ·
  width; the mix — pan, M · S · arm, the fader with its meter beside it, the level; the route
  — sends, the output, the socket. Heights are the redline's (Wide: 36 · 68 · 22 · 16 · 28 · 16
  · flex · 16 · 28 · 14 · 18; Compact and Large likewise), gaps 4 (Large 6), the fader's travel
  flexible with a 10-pt overhang each end.
- **Widths** 30 / 60 / 94 (master 40 / 68 / 110), pitch +2: value between strips, not
  borders. "Extra large" is now "Large" (Grande).
- **The short desk** gives way in des-mixer's order: EQ, the polarity row, the inserts fold
  to one summary line, the sends, the socket, the level, the output — then scrolls.
- **No label column, no −∞, no empty slot rows.** FL's "from the selected strip" row is off
  by default (the Rows menu brings it back; old desks keep their switch).
- **The parts** (`views/mixer_look.rs`): Slate's palette and des-mixer's named mixes
  (`strip.panel`, `fader.groove`, `meter.groove`, `bar.track`…), **the fader's law** (+6 →
  0 … −48 → .88, −∞ → 1) shared by the meter so one tick reads both, the groove, ticks, cap,
  the two meter bars with peak holds and the clip cell, the keys (recessed wells; lit in the
  state's colour), the insert panel, the EQ curve, the socket, the frozen hatch.
- **The rows** (`views/mixer_rows.rs`): the insert list (dot + kind; Large the unit's name;
  the first free row a faint `+`; "+n"; a keyed insert's key glyph), the EQ thumbnail, the
  bare polarity/swap/mono glyphs (a tinted well when on) and the width knob, pan (Large with
  its value), M · S · arm (Compact: the arm a dot alone at rest), the fader and meter, the
  level (the clip peak on a pill until clicked), the sends (name + level bar; Large with dB),
  a return's "5 sends in" / "← Clap", a VCA's "4 strips", the output ("Master" quietly, or
  "→ Drum bus"; Compact the bus's colour as a dot; the master "→ Out 1–2", Large the
  short-term LUFS).
- **The head** (`views/mixer_strip.rs`): number over name, Medium when selected, 11 pt then
  **two lines** (the number leading the first) before the ellipsis; Compact the name up the
  strip; a return's hook, a frozen strip's snowflake, a VCA's tag, a follower's "2 VCA".
- **States**: playing, clipping, armed, muted (stripe 32 %, name ink 3, meter 38 %), soloed,
  silenced by a solo (M ringed in solo at 45 %), selected (body tinted 13 %, stripe 100 %,
  cap's centre line in its colour), bypassed inserts, sends off, sidechain key, VCA, frozen
  (the sound block hatched), empty (number, groove, a ghost cap, socket; the Route-here plate
  under the pointer).

### The desk

- **Header** (`views/mixer_paint.rs`): the density segmented control, **Rows ▾** (the desk's
  menu), the solo chip "1 soloed · Clear", takes; at the right the cable view's key (set 3's
  routing glyph) and its options, *Snapshots ▾*, `+`.
- **Ground** `base`, the header `surface` with its lit edge; master at x 8, docks 8 apart, the
  right dock's 1-px seam; the slim scrollbar (3 pt, only when there is something to scroll);
  the **overflow** fades and pills ("‹ 6", "6 ›", a click pages).
- **Cables** (`views/mixer_cables.rs`): the sockets' row (18); the selected strip's routes in
  the accent (a key in ink 2, dashed), sockets it reaches lit; the level pills are now an
  option of the cable view (off by default).

### Icons: set 3 (`kit/glyph_mixer.rs`, `kit/glyph_mixer_data.rs`)

All 23 glyphs of `spec-mixer.md` at every drawn size (phase, swap, mono, link, arm, enable,
fx, send, out, into, ret, key, routing, freeze, vca, lamp, lampOff, grip, miniView, more,
add, foldOpen, foldShut), generated from the design source by `tools/icons/glyph_mixer.js`,
rasterised once per (glyph, size, inks) into the layer atlas, placed on the device grid.
At 125 % the largest drawing that fits is used (never a scaled one). `kit/mod.rs` gains one
line (`pub mod glyph_mixer;`).

### The inspector (first pass)

- **Head**: `surface`, a 3-pt stripe in the subject's colour, number, name Medium 13, "Mixer
  strip" at the right, the more key (the panel's menu; *Collapse* is there).
- **Sections** (`inspector_sheet.rs`): a hairline, a small chevron and the title SemiBold 11
  in ink 3, the state at the right; labels Regular 11 ink 3 at the left of a 64-pt column.
- **Insert rows** (`inspector_channel.rs`): 26 high, `surface`, number (a grip under the
  pointer), unit name and kind, the lamp with its halo (bypassed: a ring, "· Bypassed"), the
  mini view's key under the pointer, a keyed insert's key and source; one "+ Add an effect"
  row instead of ten empty slots.

## Pictures (`docs/round39/el-mixer/`)

`shoot_mixer39` (`cargo run -p cantus-ui --example shoot_mixer39 -- <dir> [filter]`): the whole
window on the demo song (Neon Tide), Chords selected, playing, the arrangement folded so the
desk and the inspector take the window — `mixer39-<case>-slate_<night|paper>-<1280x720|1920x1080>.png`
for `wide`, `idle` (stopped), `compact`, `large`, `es` (Spanish, Lead soloed) and `docked`
(the arrangement open above: the short desk's rows leave, the inserts fold). 1920 × 1080 is
at 125 %. Side by side with des-mixer's sheets: `compare-wide-{night,paper}.png`,
`compare-compact-night.png`, `compare-large-night.png`, `compare-es-night.png`
(`tools/look/mixer39_compare.py`).

## Budget (deterministic counts, `tests/frame_budget.rs`, debug build)

| | before (round 37's table) | now |
|---|---|---|
| mixer64, desk alone — triangles | 100 787 | **38 866** (22 allocations, 9 draw calls) |
| mixer64 Wide, desk alone | — | 35 604 triangles, 22 allocations, 9 draw calls |
| mixer16, desk alone | — | 11 760 triangles, 9 draw calls |
| mixer64, whole window — triangles | 96 584 | **59 567** (87 draw calls, ceiling 96) |

`slate_budget` (Slate against beta.13's Cantus Night on the same code), `workspace_budget`,
`frame_budget` (every deterministic test) pass. The timing tests (`--ignored`, release) were
not run here (slots); the counts above are what they rest on.

## Tests

- `mixer_layout` tests rewritten to des-mixer's redline (heights, order, drop order at 440
  and 288, FL's row off by default).
- `mixer_look` (the law and its inverse, the widths, silence writes no −∞),
  `kit::glyph_mixer` (every set-3 glyph at its sizes parses and inks; the 125 % pick).
- `tests/mixer_desk.rs`: tests of the removed legend column, band handle and send-row tap
  chips rewritten or removed (see below); the rest unchanged.

## Changed or removed tests (the approved design supersedes them)

- `the_legend_names_each_row_once…` → `the_desk_has_no_label_column_and_no_minus_infinity`.
- `the_routing_legend_names_each_kind_of_cable`, `the_band_handle_never_traps_the_band`:
  removed (no legend column, the band is the 18-pt socket row).
- `wider_modes_show_more`, `the_master_carries_the_pan_law` (in the master's menu now),
  `a_short_desk_fits_first_then_scrolls`, `a_tall_desk_shows_every_row` (Wide):
  expectations of the new rows.
- FL's row tests turn the row on through the Rows menu's switch (`set_row`).
- `mixer_targets`: a target is its share of the strip (≥ 15 pt) and ≥ 20 pt tall — des-mixer's
  30/60-pt strips cannot hold three 24-pt keys across (owner to judge, below).

## Second pass (the coordinator's asks and the owner's four choices of 2026-11-09)

### The strip, the owner's choices (`options-keys-cables-*`)

1. **Keys, option C.** M|S one segmented pill (Wide 27 × 24 a key, Large 43 × 24, Compact the
   two stacked 26 × 24); the arm a 24 × 24 dot in the head beside the number (grey; the
   record red armed); polarity and swap one pill (Large adds mono; the master polarity and
   mono); the width knob in the pan row. Row heights: polarity 24, M|S 24 (Compact 48).
2. **Cables, option C with B on demand.** Nothing at rest (`CableView::Off` the default; a
   desk saved before is read so once, `DeskPrefs::look39`). Under a strip "→ Drum bus" or
   "3 routes"; the strip under the pointer (else the selected one) lights its destinations —
   an accent outline and a tag of the kind in their output row ("> out", "> send −12",
   "> key"). The header's **Cables** key shows B's curves for the selection; "Show all" in
   its options menu. The band, when asked for, no longer gives way on a short desk.
3. **Send level, a small knob.** Wide and Large: name + knob, the value in dB in the name's
   place on hover and while dragging, driven-aware (`DeskMod`: the automation's sign, the
   modulators' rings). Compact: a dot per send; a click opens its knob above the dots.
4. **Sidechain.** The key glyph on a keyed insert says "← Kick" on hover and outlines the
   source strip; Large names the source beside the key.

`tests/mixer39_e2e.rs` holds all four through `CantusApp::assemble` on the demo song.

**Budget after the second pass** (`frame_budget`, deterministic): mixer64 desk alone 27 814
triangles, 19 allocations, 9 draw calls; Wide 35 879; mixer16 8 540; mixer64 whole window
46 855 triangles, 86 draw calls (before round 39: 100 787 and 96 584 triangles).

**For integration:** el-controls' `kit::automate::vca_ghost` is not on this branch; its call
site is marked `INTEGRATION (r39:el-controls)` in `views/mixer_rows.rs` (`fader`), where a
VCA follower's tick is drawn today. The send knobs use the kit's knob as it is (el-controls
restyles it to des-controls' small knob).
The cable band's older tests start with the band shown (`views::mixer::with_cables`);
`click_paths::p07` takes the Cables key first (3 clicks).

### The strips' EQ curve

`Desk::eq` (`EqThumb`): each strip's own three bands and its playing Aequalis inserts,
sampled at 64 points, worked out again only when their settings hash changes; drawn as the
thumbnail in Wide and Large (Large with band dots).

### The inspector, second pass (des-mixer's `inspector-*`)

Order: head, source, Inserts, Equaliser, Sends (switch, name, a 64-pt level bar with its
handle, the value), Receives; then folded rows with their state — Routing, Sidechain,
Recording, Automation, Level, Freeze, the Modulators (open when there are some) — each a
click from open (`Sheet::fold`, its title folds it again). Level (volume, pan, width as
parameter rows) replaces the pinned strip for a strip or the master. A compressor's insert
row shows its live reduction (the value in the solo colour over a short bar). The send's
tap moved from the row to the send's menu ("Before the fader").

## Verified only in emulation

Everything is drawn by the CPU renderer (`raster.rs`) through `shoot_mixer39` (the whole
window, the demo song, meters fed by the real engine running offline). The glyphs' pixel
placement at 125 % and 150 % on a real screen, and the meter's look while music plays at
60 Hz, need the owner's eyes on the machine.

## Test status on this branch

Green: the lib's 493 tests, `mixer_desk`, `mixer_routing`, `mixer_targets`, `mixer_vca`,
`mixer_cables_view`, `route_entry_points`, `modulators`, `frame_budget`, `slate_budget`,
`slate_e2e`, `workspace_budget`, `panels`, `ux_help_line`, `ux_signs_e2e`, `ux_whole_words`,
`kit_lint`. Two failures that are not the mixer's and come from the base (the kit's title-case
menus and the top bar): `keyboard_reach::every_row_of_the_menus_is_reachable_from_the_keyboard`
(expects "FILE"; the menus read "File" since `r39/kit`) and
`click_paths::p09_change_the_tempo` (no "Tempo" control where it looks in the top bar).

## For the owner to judge

1. The click targets: des-mixer's strips (30 / 60 pt) hold three keys of 17 pt across;
   each key's target is its share of the strip (Wide 20 × 24, Compact 15 × 20), not round
   37's 24 × 24.
2. The cables keep the kit's curve (the design draws square lanes with r 3 corners); the
   kit's cable is `kit/cable.rs` (not this builder's).
3. The knobs and the segmented control are the kit's (el-controls restyles them).

## What is left

- **The inspector, second pass** (des-mixer's `inspector-*`): the section order (head,
  source, inserts, EQ, sends, then routing · sidechain · recording · automation · level ·
  freeze, folded to one line each with its state), the mini view of an insert as the sheet
  draws it, the Level section in place of the pinned strip at the panel's foot, the Freeze
  key. This pass restyled the head, every section title, the labels, the insert rows, the EQ
  display and the send switch; the other rows are still the kit's wells and fields.
- The header shares the window with the workspace's tab row (ux-b's): des-mixer draws the
  tabs and the density in one 28-pt row.
- The empty strips 11–13 and the right dock pinned at the window's edge as the sheet shows
  them: the dock keeps round 37's place right after the middle (the judge's "dead space").
- Not drawn yet: the dragged insert row's lift and drop line in the inspector's new style,
  and the Large EQ's band dots on real data (the demo's strip EQs are flat).
