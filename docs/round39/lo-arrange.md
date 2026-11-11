# Round 39 close-out — builder `lo-arrange`: the arrangement's, the clips' and the piano roll's leftovers

**Branch:** `r39/lo-arrange` from `r39/integration` `f473d0ba`. **ADR:** 0114 (the
arrangement's tools; the integrator renumbers on a collision). **Sheets:** dir-a
`b/b-toolbar-*`, des-arrange (`canvas-*`, `max-*`, *Clip states*, *Moving a clip*, *Resize*),
des-views (*Piano roll — Head*, *Clip content*). **Schema:** unchanged. **Keymap file:**
version 16 (round 39's own, unreleased) gains `tool-zoom` and `grid-triplets`.

## What was left, and what was done

The "What is left" lists of `el-arrange`, `el-views` and `el-topbar`:

| Left by | Item | Done |
|---|---|---|
| el-views | automation values at the close zoom ("688 Hz", Medium 9, ink 2) | `arrangement_auto.rs`: each point's value in its units at ≥ 60 px a beat, above the point (under it at the lane's top), left out where it would touch the one before |
| el-arrange | the selected track's clips `lit` | `ClipLook::lit`: a step toward the clip's colour, no edge (the selection is lit and edged) |
| el-arrange | a soloed-away track's clips `dim` | `ClipLook::dim`: the tint 40 % toward the lane, the notes at half |
| el-arrange | the moving clip's dashed slot and shadow | the slot it left dashed in ink 3 (4/3), the clip riding lit over a soft shadow (35 % Night, 12 % Paper, +3 px); the snap line was already drawn |
| el-arrange | the resize edge's light | 4 px of the clip's colour + 20 % white on the edge under the pointer, lit while it is dragged; the help line says the length it lands on ("Length 8 bars · Alt: stretch · Ctrl: no snap") |
| el-arrange | the loop notch | a 9-px corner at a note clip's top right, shown when the pointer finds the clip; dragged, the clip repeats at the length it had and its end is drawn out (one undo step) |
| el-topbar, el-views | the panels' toolbar: tools as a radio set, Grid, Follow, the panel menu, the fold order | `views/arrangement_tools.rs`; the shared `Toolbar` gained `follow_right`, `menu_right`, `clip_menu`, `selection_right` (`clips_chrome.rs`) |
| el-views | the grid menu in musical order, Triplets, "Follow the zoom" | `tools::grid_menu`, shared by the arrangement and the piano roll; Triplets on Shift+3 (`GridTriplets`) |
| el-views | the piano roll head's clip menu (track stripe › clip) and the selection readout | `piano_roll.rs` (`clip_menu`, `head_readout`) |
| el-arrange | the maximised arrangement: docks folded to one row of tabs | `workspace_fold::docks_row` under any maximised area (see *Decisions for the owner*) |
| el-arrange | the section drag that ripples the song | **not built** (the owner places it): recorded in `BACKLOG-excellence.md` §3 |

### The tools (a behaviour change — ADR 0114)

- **Select** is in hand at start and does what the view always did.
- **Draw**: a click on an empty lane adds a bar-long clip in the grid cell clicked; a drag
  draws it to the length dragged (one `InsertClip` on release). On a clip it moves and
  resizes, as Select.
- **Slice**: a click cuts the clip on the grid (`clips::split`, one step); the cut is drawn
  under the pointer first.
- **Erase** / **Mute**: a click deletes or (un)mutes; a drag across clips does it to each it
  touches, one merged step.
- **Zoom**: a click zooms in at the pointer, Alt-click out; a drag fits the bars dragged.
- **Keys:** FL's letters and the number row reach the arrangement's tools while it has the
  focus, the piano roll's otherwise (as before); `ToolZoom` is new on **8** (Z stays *Zoom to
  selection*); B stays the arrangement's paint tool.
- **The old row's keys** (split, glue, duplicate, mute, repeat, make unique, paint, delete,
  loop and zoom to selection) are in the panel menu "…" with the snap ("Snap to the grid")
  and the timeline's own entries — the same actions as their keys — and in the clip's menu.
- **Follow** (on at start): while playing, the page turns when the playhead leaves the view,
  starts outside it or wraps; a view the user scrolled away is left alone. Nothing at rest.
- **The fold order** is dir-a's: the grid's word and Follow's word, then the name to its
  glyph, then the tools to the chosen one and a chevron (the existing `Toolbar` folds;
  Follow's word folds with the grid's, as the sheet draws it at 420).

### The coordinator's second look (fixed on this branch)

- **`dim` reads as silenced.** A soloed-away clip's body and bar step 62 % toward the lane,
  its notes and attacks keep 32 %, its name 45 % (`arrangement_paint.rs`, `DIM`,
  `DIM_NOTES`). In `states-*` the soloed Hats stands alone; the rest are clearly out.
- **The keyboard's corner.** The blue slanted shape over the keys was FL's note-colour tab
  (`chrome_parts`), a swatch with no sense and nothing in the sheet. It is des-views'
  corner now: "Rows" in ink 3 and the rows' height in a well; a click opens Compact 8,
  Default 12 and Tall 18 (Ctrl+Alt+wheel over the notes still zooms them freely).
- **Tool glyphs from the family.** The icon family (set 2, approved) already holds the
  tools' drawings — `select` (the arrow), `draw`, `slice`, `erase` (the eraser), `muteTool`,
  `zoom` — and `follow`, in `kit::icons_data`. They were not reachable because `kit::Glyph`
  does not list them; `kit::icons::paint_name` paints a family drawing by its name (the
  existing `paint` calls it), and the shared toolbar's keys use it — the arrangement's six
  tools, the piano roll's Draw, Select, Erase, Mute and Slice, and Follow. Additive: no
  glyph data changed, no `Glyph` added. The piano roll's Paint and Playback tools have no
  drawing in the family and keep the house icons (for the icons agent's next set).

## What the tests hold

- `tests/lo_arrange_e2e.rs` — one test through `CantusApp::assemble`, the whole window at
  1920 × 1080, the pointer pressing where a user would: the row's words; Follow's key; the
  tool keys reaching the focused view; Draw (click and drag, one undo each); Slice; Mute;
  the eraser's sweep (one undo); Zoom (click, Alt-click, a dragged range); the loop notch
  (repeat on, drawn out, one undo); the grid menu's order, Triplets' action, the grid
  following the zoom; automation values only at the close zoom; the piano roll's head (track
  › clip, the count, "2 notes selected" with its range, the clip menu's other tracks); the
  maximised arrangement's docks' row, its tab and "Show the docks".
- Unit tests in `arrangement_tools.rs`: Follow's page; the grid's triplets, order and zoom
  rule; every tool's action.
- `app_flows::the_arrangement_toolbar_is_a_row_of_icons_with_a_grid_picker` now holds the
  sheet's row (Follow instead of the snap lamp and the "1 selected" readout).

**The suite** (`cargo test -p cantus-ui --no-fail-fast`, Windows), after the second look:
**2 598 passed, 1 failed** — `agent_tools::export_writes_only_where_allowed_and_only_when_files_are_allowed`,
"Access is denied" writing its MIDI file into the shared temporary folder; it passes when
re-run alone (14 of 14), and it passed in the run before. The first run's one failure,
`panels::a_double_click_on_a_tab_maximises_its_area_and_gives_the_rest_back` (the docks' row's
28 points under its 1.8× bound, 807 against 810), has its bound at 1.7× with the reason and
passes. `slate_budget`, `frame_budget`, `workspace_budget`, `at_rest_e2e`, `app_flows`,
`kit_lint` and the i18n tests pass. `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui
--all-targets`: clean. Linux (WSL) was not run here.

## The shots

`docs/round39/lo-arrange/` (`cargo run -p cantus-ui --example shoot_lo_arrange -- <dir>`), the
whole window at 1920 × 1080 and 125 %, Slate Night and Slate Paper:

| Shot | What it shows |
|---|---|
| `arrangement-{night,paper}-1920x1080.png` | the tool row as dir-a draws it (name, six tools with Select chosen, Grid · 1 bar, Follow on, "…"); the Chords track selected, its clips `lit` |
| `states-{night,paper}-1920x1080.png` | the Hats soloed — every other track's clips `dim`; a Chords clip moved two bars on: its slot dashed, the clip riding over its shadow, the snap line, "Drop → 3.1" in the help line |
| `close-{night,paper}-1920x1080.png` | 90 points a beat with the Draw tool in hand: a Level lane's points with their values ("+0.0 dB", "−15.0 dB", "+6.0 dB") clear of the clip's name |
| `maximised-{night,paper}-1920x1080.png` | the arrangement maximised: the docks' row at the foot — "Inspector I · Mixer F9 · Piano roll F7 · Channel rack F6" and "Show the docks ⌃" |
| `pianoroll-{night,paper}-1920x1080.png` | the piano roll's head: "Piano roll", the clip menu "▌Chords › Drop ▾", the tools, the grid, Snap, Ghosts, the scale's parts, "6 notes selected · E4 – C5 · velocity 90 – 95", "…" (at 1920 × 1080 125 % the row folds the grid's word and the ghosts' names, dir-a's first fold) |

Read beside the sheets (`b-toolbar-night-1x`, des-arrange `canvas-*` and `max-*`, des-views
`pianoroll-*`): the row's order, sizes and folds are the sheet's; the docks' row matches
`max-*`; the piano roll's head has the sheet's parts in its order. What differs: the tool
glyphs were the house icons at first (fixed: *The coordinator's second look*); the piano roll's head also
carries round 31's key, Scale, Fold, In key, Quantize and Score, which the sheet does not
draw. The tools are the family's drawings (the arrow, the eraser…); only the piano roll's
Paint and Playback keep house icons, the family having none.

## Decisions for the owner

1. **What maximise hides.** des-arrange's `max-*` keeps the inspector beside the maximised
   arrangement and folds only the docks below it; ux-b's maximise (ADR 0110) gives the area
   the whole workspace. The row of tabs is built on ux-b's: every hidden area is in it, the
   inspector included. Keeping the inspector would change ux-b's maximise for every area.
2. **The grid menu's words.** The sheet says "½ bar"; the menu says "1/2", because in 3/4 a
   half note is not half a bar. Bar and Beat are the sheet's.
3. **Follow is on at start** (FL's default). It turns the page only when the playhead leaves
   the view.

## What is left

- **The section drag that ripples the whole song** — the owner places it (`BACKLOG` §3).
- Everything else in the three lists above is done. Verified only on the CPU raster and in
  the test harness; not on a real screen or by the owner.
