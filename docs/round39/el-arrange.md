# Round 39 — builder `el-arrange`: the arrangement in Slate (ADR 0113, schema 61)

**Branch:** `r39/el-arrange` (worktree `F:\projects\daw-r39\el-arrange`), on `r39/integration`.
**Build sheets:** `F:\projects\daw-r39\des-arrange\docs\round39\design\des-arrange\` (approved by
the owner 2026-11-09). **ADR:** 0113. **Schema:** 61 (the migration writes nothing;
`fixtures/v61.cantus`). The owner's addition of 2026-11-09 — a track's own icon — is in.

## What changed

### The model (schema 61, ADR 0113; `cantus-core/src/arrange.rs`)

- **Sections** (`Project::sections`): named spans with their length, never overlapping.
- **Folders** (`Project::folders`, `Track::folder`): a row of the view, open or closed; its M
  and S are its members' own (one batch); *Ungroup* empties and removes it in one step; a
  header dropped between two members joins their folder, anywhere else leaves it.
- **A mode per automation lane** (`Track::lane_modes`): a lane's own mode over its track's.
  The write pass asks it (`auto_write::own_auto_mode`), a track "wrote" when it or one of its
  lanes is in Write, and a lane in Write turns to Touch after a pass as its track does.
- **A track's own icon** (`Track::glyph`): an id of the track icon library, before the name.
  `views::track_glyphs::track_icon(app, track)` is the helper for the mixer's strip head and
  the clip browser. The library holds 16 placeholder ids drawn with today's pictures (the
  type icons and the kit's glyphs) until the icons agent's set 6 lands; an id this build does
  not know is kept and drawn as none. The agent's `set_track` sets it (`icon`), and
  `track_summary` reads it.
- `Command::Arrange(ArrangeCommand)` — nine commands, each with its exact inverse; an undo
  fuzz arm of its own; the freeze signature ignores the new fields.

### The view, built to the sheets (`crates/cantus-ui/src/views/arrangement_*.rs`)

- **Inks and type** (`arrangement_slate.rs`): the sheets' roles (gutter, base, surface, ink
  1–4, well, edge, accent, rec, solo, auto…) read from the installed tokens, so every family
  and light draws them; the sheets' sizes (Regular 10, Medium 11, SemiBold 9…) set through the
  kit's roles at a scale.
- **Track headers** (`arrangement_head.rs`, `_head_paint.rs`, `_headers.rs`,
  `_headers_input.rs`): a quiet `surface` body, the user's colour a 3-px stripe (whole when
  selected, `ink4` when muted), the number right-aligned at x 24, the name (Medium when
  selected, ink 3 when silenced), line 2 with the type icon ("Calor", "Audio · Perc loop
  124", the armed input with its meter, a *Frozen* tag); M S ● A as letters at rest, gathered
  into one well with "more" under the pointer, only the engaged key lit (M an ink-2 pill, S
  solo amber, ● record red, A violet), M outlined amber when silenced by a solo; a slim meter
  at the right; on a 64-px row the level as a slider and its value, on a 96-px row the
  inserts in words; on a wide column (≥ 260) the level on line 2. Widths 120–360 (default
  200; narrow headers carry M S only); heights 24/32/44/64/96 (default 44) from the
  *Height* page of the track's menu, any height by dragging a row's foot (Alt: every track),
  with the height on the help line. The selected track's header and lane are tinted.
- **Folders** (`arrangement_folders.rs`): one layout for headers and lanes alike (folders'
  rows among the tracks, a closed folder's members at no height, `TimelineGeometry::rows`
  still by track). Open: a 30-px row, chevron, Medium name, M S; its lane the group's span as
  one quiet body with each member's clips as 5-px bars. Closed: a 44-px row with "Folder ·
  4 tracks → Drum bus" and 8-px bars. Children indented 12, a step back, the folder's stripe
  continuing and their own colour a 2-px bar.
- **The ruler and top lanes** (`arrangement_ruler.rs`, `_sections.rs`, `_markers.rs`,
  `_tempo.rs`): ruler 24 — the loop as a band in its top 5 px (accent on, ink 4 off, its ends
  as 2 × 7 bars, the looped bars tinted under the numbers), bar numbers from 26 px a bar then
  every 2/4/8/16, the playhead's bar in ink 1, beat ticks from 6 px a beat, beats named
  "41.2" from 40, the time selection as a neutral veil, the playhead's cap and — while
  playing — where play started as a hollow cap. Under it the **sections and markers** lane
  (20): sections as named spans with their length, the playing one Medium ink 1, every other
  one a step lighter, the name kept at the left edge; hover lifts a section and shows its end
  grip; click selects, double-click renames, a drag moves it along the bars (one undo step),
  its end resizes it, its menu selects or loops its time and removes it; markers as a line, a
  pennant and their name (the pennant only below 20 px a bar). The lane's menu and the
  corner's lanes menu add a section (of the time selection, or four bars). The tempo lane is
  40 high, its header "Tempo 124.00 BPM · Metre 4/4".
- **The corner** (`arrangement.rs`): a plus in a well (click: an instrument from the picker;
  right-click: every kind of track and the instruments chosen last), "N tracks", the lanes
  key (sections and markers, tempo, add a marker, add a section); "Sections" beside its lane.
- **The canvas** (`arrangement_grid.rs`): 4-bar groups by value (16 below 8 px a bar), bars,
  beats and sixteenths by zoom, a section's start a step stronger, each row's foot, the
  selected lane at 5 % of its colour and an armed one at 4 % red, after the song's end a veil
  and a line, below the last track the grid veiled and "Drop a sound, a loop or a MIDI file
  here for a new track"; over the clips the time range, the rubber band, the snap line of a
  clip being moved or resized, the playhead. The help line reads "43.1 – 46.1 · 3.0 bars"
  while a range is drawn and "Stabs → 44.1" while a clip moves.
- **Automation lanes** (`arrangement_auto.rs`): 36 high under their track; the header a step
  back, the stripe continuing, a violet hanger, the envelope, the parameter and its owner,
  the value at the playhead and the lane's **mode chip** (Read ink 2, Touch and Latch violet,
  Write red; a click picks the lane's own mode), a close × under the pointer.
- **The song map** as the horizontal scroll (`arrangement_songmap.rs`): the panel's 14-px
  foot, every track a line where it has clips, the view a frame (drag it; its ends zoom;
  press elsewhere to go there), the playhead and the loop in the accent; "16 bars · 0:30"
  under the headers. The vertical scroll is a 3-px thumb at the right, only when the rows
  overflow.
- **The empty song** (`arrangement_empty.rs`): "Start your song", one sentence, three
  starters (Instrument track ringed as the default, Drum kit, Audio track with its key), a
  drop zone when there is room, "Or open a demo song"; "No tracks yet" in the header column.
- **Menus** (`arrangement_menus.rs`): the add-track menu (Instrument track, the recent
  instruments, Audio track, Drum kit, Folder, From a preset…, From a file…, Return,
  Automation lane); the track's menu (Rename, the colour row, Instrument ▸, Height ▸,
  Automation ▸, Icon…, Insert above/below, Duplicate, Put in a new folder / Take out of its
  folder, Freeze, Show in the mixer, Recording and more ▸, Delete track) — a submenu opens
  in the same place with *‹ Back*.
- **A track's own icon** (`track_glyphs.rs`): the picker (search, *None*, a grid) from the
  menu's *Icon…* or a click on the icon.

### Files touched outside what I own (small, said here)

- `arrangement_paint.rs` (el-views'): the old header, ruler and lane-ground painters
  removed (dead now) — deletions only, the clip painters untouched.
- `views/warp.rs`: `..Default::default()` in its `TimelineGeometry` literal (new fields).
- `auto_write.rs`: the lane's own mode in `own_auto_mode`, `any_writing_mode`, the tracks
  that wrote.
- `views/mod.rs`: `pub mod track_glyphs`; `i18n.rs`: the `arrange` area.
- Tests updated for the new header (each marked `r39:el-arrange`): `quick_gestures.rs`
  (`header_map` aims at line 2's words: no song in miniature now, and the type icon is the
  instrument chip); `views_nav.rs` (the scroll bar's end is the song map's frame).

## How it is tested

- `cantus-core`: `arrange_tests.rs` (6 tests); the undo fuzz arm
  `the_arrangements_structure_undoes_and_redoes_among_the_others` (every kind applied, the
  refusals refused).
- `cantus-project`: all 61 format tests and every fixture through `v61.cantus`.
- `cantus-ui/tests/arrange_e2e.rs`, through `CantusApp::assemble` on the whole window: a folder
  made, its members indented, closed by its chevron (members hidden, no header), undone; a
  section added, dragged two bars along by its body (one undo step); a lane's own mode
  honoured by the write pass and its neighbour reading; the track's icon picked from its
  menu's *Icon…* in the picker, shown before the name, reopened by a click on it, undone;
  each saved and loaded back.
- Kept green: `ux_help_line` (0 controls without a sentence), `ux_whole_words`, `ux_signs_e2e`,
  `panels`, `kit_lint`, `rec_flows`, `quick_gestures`, `views_nav`, `arrangement_header_width`,
  `automation`, `auto_write*`, `tempo`, `mixer_routing`, `device_ux`, the lib's 491.
- Budgets (counted in every run): `frame_budget`, `workspace_budget`, `slate_budget`,
  `views_budget`, `night_budget` pass — no more draw calls, triangles or allocations than
  their bounds. **Timed** (`slate_budget`/`workspace_budget --ignored`, release, on a machine
  shared with three other builds — the rows are interleaved in one run, so they compare):
  the whole 1920 × 1080 window playing, build + tessellate: beta.13's Cantus Night p50
  1.73 ms / p95 2.86 ms, **Slate Night with this arrangement p50 1.49 ms / p95 2.54 ms**
  (14 % under beta.13; the kit's own run without it measured 18 % on a quiet machine);
  `workspace_budget` layout p50 0.70 ms at 1920 × 1080, 0.67 ms at 1280 × 720.

## Pictures

`docs/round39/el-arrange/window-{song,closed,detail,empty}-{night,paper}-{100-1280x720,
125-1920x1080}.png` — `cargo run -p cantus-ui --example shoot_arrange -- <dir>`: the demo song
with three sections, a marker, its drums in a folder (open; closed), Chords selected with its
own icon, Lead muted; *detail*: a wide column, a 64-px row with its level, two lanes of
Chords (Touch, Read), an armed audio track; the empty song.

## Verified only in emulation

Everything is drawn by the CPU renderer (`raster.rs`) and driven by synthetic pointer
events; nothing was seen on the owner's screen.

## What is left

- **A section drag moves the section, not the song's bars.** The sheets say a section drag
  "moves its bars on every track" (Ctrl copies, Shift inserts): ripple editing of the whole
  song (clips split at the edges, the rest shifted, automation, tempo and markers with them).
  A feature of its own — for the owner to place (ADR 0113).
- **Clips lit/dim** (the selected track's clips `lit`, a soloed-away track's `dim`), the
  moving clip's dashed slot and shadow, the resize edge's light and the loop notch are clip
  painting — `el-views`' files; the view passes them what they need (`selected_track`).
- **The maximised arrangement** (docks folded to one row of tabs) is the layout's (ux-b).
- **The icons**: set 6 (track icons) and the kind marks, envelope, folder and snowflake
  glyphs are placeholders until the icons agent's drawings land (`track_glyphs::LIBRARY`,
  `head::envelope`, `head::chevron`).
- `app_flows` (`FILE`/`CLIP` in capitals), `keyboard_reach` (`FILE`) and `click_paths` p09
  (the top bar's tempo) fail on this branch from the top bar's title-case menus and its new
  layout, not from the arrangement.
- **Shared target dir hazard (for the integrator):** worktrees sharing
  `integration/target` build the same crate into the same artifact, so a build can link
  another worktree's `cantus-core`. My jobs `touch` my changed sources first
  (`F:\projects\daw-r39\tmp\ea_touch.sh`); a test run that sees a model without schema 61's
  fields is that, not this branch.
