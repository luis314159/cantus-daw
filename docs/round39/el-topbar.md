# Round 39 — builder `el-topbar`: dir-a's phase B, the top bar and its dialogs

**Branch:** `r39/el-topbar` (worktree `F:\projects\daw-r39\el-topbar`), on `r39/integration`
(the Slate kit, ux-a, ux-b, memory). **Sheets built:** `dir-a`'s phase B
(`F:\projects\daw-r39\dir-a\docs\round39\design\dir-a\b\`, README *Phase B*), approved by the
owner on 2026-11-09.

## What changed

### The top bar (`views/transport*.rs`)

- **One row, 44 high** (`transport::HEIGHT`, was 66 with two rows), on the bar's surface with
  one lit top edge and the gutter's line under it. Every key is a recessed well 24 high on the
  centre line (`transport_paint.rs`: wells, keys joined in one well and divided by its dark
  edge inset 4, a key's state as a tint inside it — green for play, red for record, the accent
  else — words at the sheet's exact sizes from the kit's roles).
- **Two clusters** (`transport_layout.rs`): the left packs from the left edge (the mark and the
  menus over the name, Pat / Song, the transport and loop, the metronome, the tempo and metre,
  the position, the grid, undo, the windows), the right from the right edge (the system
  meters, the inputs, the master meter, the device, the tools, **Settings**, **More**); groups
  10 apart, 12 from the edges, at least 24 between the clusters. The right cluster starts at
  the first group of the owner's order that belongs there (`Group::right`), so a customised
  order is kept as it is.
- **Forms and the fold order** (`layout::STEPS`, the sheet's `topbar2.FOLD`): tools → inputs →
  CPU/RAM → windows → device compact → metronome → meter compact → tempo compact → position
  compact → menus compact → Pat/Song → grid compact → menus minimal → undo → meter; then, past
  the sheet (a narrow window at a large size), the agent chip, the device, the grid, the menus
  bare, the tempo, the position, the transport last. Sync, shown only when asked for or in use,
  folds first. A unit test reproduces the widths sheet exactly at 1920 … 800 with the sheet's
  measures.
- **Every group drawn from the sheet:** the menus 12 Regular ink 2 over the name 13 Medium and
  the summary (`9 tracks · 16 bars · 0:30`); minimal, the mark, a ≡ key holding every menu as
  a submenu, the name. Pat / Song a segmented well with the chosen raised pill. The transport
  well (30·32·30·30) and loop. The metronome key and its ▾ in one well (count-in, level, sound,
  accent and when are in its options). The tempo `124.000 BPM` (a click on the unit taps; it
  reads `TAP` in the accent while tapping) and the metre well. The position in the clock face
  (ExtraLight 30) with its ticks, the time over `bar · beat`; recording turns it red and the
  take's time takes the time's place. The magnet and the grid in one well. Undo and redo. The
  seven windows (browser, arrangement, rack, piano roll, **the plug-in window (F8)**, mixer,
  inspector). CPU as a graph with its number, the memory, dropouts as a red pill. The MIDI lamp
  and the armed ring with its count. The master meter as two 3-px lanes, the scale's ticks,
  grooves at rest (never "−∞"), a clip lamp; its figures are on the help line. The device: its
  name, or the amber warning with "No device". The picker and the zoom. The gear opens
  Settings. More carries the number of folded groups in a raised pill.
- **The More panel** (`transport_panel.rs`): built from the folds — sections for Metronome
  (its key, switch and shortcut; count-in, level, sound, accent, quiet while it is off),
  Windows (each key with its shortcut under it, the hovered key's words below), Performance
  (CPU, memory, voices, dropouts as graphs with numbers over the last 30 s, from a new
  `perf::History`), Inputs (the ports, *Settings ›*, the armed tracks by name) and Tools and
  zoom (− 100 % +); the small folds and the compact forms' pieces (Pat / Song, the time
  signature, the grid, undo, the meter, and any group a very narrow bar folded) under *Also
  here*. One column up to three parts, two from four, balanced by height. Keycaps are outline
  pills. The foot: *Customise top bar…*, a line.
- **Spanish** folds rather than collides: every group's width is measured in the language
  (menus, Pat / Song, the inputs' words, the meter's mode, the device's words); the property
  test sweeps both languages at seven zooms.
- **The help line**: the old bar's hint panel is gone (the sheet has none); what the pointer
  rests on, its value and what drives it are written in the status line's place
  (`workspace_status.rs`, a small change) while something is hovered.

### The mark (`kit/logo_small.rs`)

`dir-a`'s `logo.py` ported: #2's idea drawn for 16, 20, 24, 30, 32, 36 and 48 px — a solid
arc at 16, a toothed arc at 20 and 24, real spokes from 30, the playhead and crossbar on whole
pixels, the cap and the tile's lit edge from 24, the logo's warm black tile. Other sizes scale
the nearest drawing. Roles stay marked, so the taskbar tints recolour the wave.

### The dialogs

- **The dialog frame** (`workspace_settings::frame`, `title`, new `foot`, `scrim`,
  `side_ground`): the bar's surface, corner 10, one shadow 14 down, a lit top edge; the title
  15 Medium with a 26 × 24 close key; the scrim (Night toward its gutter at 58 %, Paper toward
  its ink at 34 %). Every floating window that uses the frame follows.
- **Settings:** 888 × 590, the side list on the window's base with the chosen page a raised
  pill and a 2 × 14 accent bar, section titles 11 SemiBold ink 3 in title case, labels left in
  a 150 column, and a foot: "Now: …" what the sound runs through, and **Done**.
  **Appearance** gains the three families as pictures (Slate the default, Graphite and gold,
  Studio ink — a click sets both its Night and its Paper), the mode (Night, Paper, follow the
  system) and the taskbar icon's tints.
- **First-run audio setup** and **export**: the new frame, title row and scrim; the steps'
  heads a number in a ring and a 13 Medium title; export's presets on the window's base.
- Dialog buttons (`audio_parts::paint_button`): primary an accent fill, secondary a well, 28
  high.

## How it is tested

- `transport_layout.rs` unit tests: the widths sheet reproduced at eight widths (what folded,
  what went compact, the menus' form), the clusters packing from their edges with 24 between,
  a very narrow bar keeping the transport last.
- `tests/topbar_layout.rs`: the property sweep (widths × seven zooms × six group sets × two
  languages) — nothing overlaps, nothing leaves the bar, every group is on the bar in one of
  its forms or in the panel, in the owner's order; every group drawn alone keeps its words
  whole inside its width; at 1920 the English bar shows everything (Spanish at most folds the
  tools); narrower, the steps are taken in order; the clock reads whole in every metre and
  while recording; the owner's order allocates nothing more.
- `tests/topbar.rs` (through the whole window): tap on the tempo's unit, pattern mode, the
  title through the boot path, the narrow window's panel (a window key, the grid list, the
  metronome's sound from the panel), customising, the meter's mode and its figures on the
  help line, an over held while the meter is folded, the audio popover under the bar.

## Pictures

`docs/round39/el-topbar/window-<case>-<theme>-<size>.png` — Slate Night and Paper, 1280 × 720
at 100 % and 1920 × 1080 at 125 %: `window` (as it opens), `overflow` (the More panel),
`playing`, `es`, `settings`, `look`, `firstrun`, `export`; `bar-<width>-<theme>.png`, the bar
alone at the widths sheet's eight widths. Made by `cargo run -p cantus-ui --example
shoot_topbar -- docs/round39/el-topbar`.

## Verified only in emulation

Everything is drawn by the CPU renderer (`raster.rs`). The MIDI lamp, the device's names and
the dropout graph were seen with the dummy engine and the demo device, never a real one.

## The budget

- `tests/slate_budget.rs` (the whole 1080p window playing, counted): **Slate Night 38 293
  triangles, 90 draw calls, 311 allocations** — against ADR 0108's 41 134 triangles before this
  branch, and beta.13's Cantus Night 40 641 / 90 / 307 in the same run. The test holds (no more
  draw calls or triangles than beta.13, allocations within 8).
- `tests/topbar_layout.rs bar_triangles`: the bar alone at 1920 px, **2 684 triangles**.
- `tests/workspace_budget.rs`: the bar **23** allocations a warm frame (egui's per-widget
  bookkeeping; the ceiling is 400), the status line 2; `frame_budget.rs` green.
- The timed release runs (`--ignored`) were not run here: the integrator measures on a quiet
  machine.

## What is left

1. **The panels' toolbar** (`b-toolbar-*.png`: the arrangement's tools as a radio set, Grid,
   Follow, the panel menu, its fold order) is not built. Its code is the shared `Toolbar` in
   `views/clips_chrome.rs` and the arrangement's tool set in `views/arrangement.rs`, both
   `el-views`' (clips) — to coordinate: the arrangement has no tool radio set today (its keys
   are clip actions), so the sheet is a behaviour change too. Passed to `el-views`.
2. *Done since the first report:* Settings ▸ Audio's driver as a segment (ASIO | WASAPI
   with a word on each), the buffer's latency scale (green to 6 ms, amber to 12, grey past,
   the round trip and a sentence), the inputs as slim lanes; the first run's interfaces as
   rows with the "Recommended" tag and a check; export's progress (the phase, the
   percentage, a 4-px accent bar, Stop a secondary key). Shots: `window-settings-*`,
   `window-inputs-*`, `window-firstrun-*`, `window-exporting-*`.
3. **Glyphs:** the device group shows `Glyph::Device` (the family's "plugins" drawing) where
   the sheet has a speaker, and the More panel's foot shows the gear where the sheet has the
   sliders glyph — both set-5 drawings `el-controls` owns; the code names them through
   `kit::Glyph`, so they change with the family.
4. **Decisions for the owner:** the bar no longer carries the automation arm and its modes
   (round 31) nor the typing-keyboard key: the sheet has neither (their keymap actions, the
   palette and the status line's "Typing keyboard" keep them). The windows' seventh key is
   the plug-in window (F8), as the sheet draws it; the session view is on Tab and in View.
   The record options open by a right-click on the record key (the ▾ beside it is gone).
   At 1920 × 1080 at 100 % the built bar folds the tools (English) or the tools and the
   inputs (Spanish): its tabular clock and the take's time cost ~30 px more than the sheet's
   proportional digits.
5. **Small changes outside the owned files**, for the merge: `views/arrangement.rs` (the
   empty rows below the last track drawn only whole — the 44-high bar left one cut at 1600 ×
   900), `views/workspace_status.rs` (the help line), `views/workspace_widgets.rs` (`band` and
   `row_label` in Slate), `views/audio_parts.rs` (dialog buttons), `views/record_options.rs`
   (`popover`, and the level's slow drag summed), `views/audio_chip.rs` (the bar's device
   group), tests `about.rs`, `click_paths.rs`, `keyboard_reach.rs` (title-case menus, the
   tempo's new place).
