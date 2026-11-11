# Round 39 — `ux-b`: panels, the keyboard's reach, the daily click paths (ADR 0110)

Branch `r39/ux-b`. Behaviour and structure only, in today's kit tokens: the new look comes
later through the kit. No schema change (every shape kept here lives in the layouts file or
`settings.json`); keymap file version 16.

## 1. The daily click paths, counted

Every path is driven through `CantusApp::assemble` at 1600 × 900, the welcome song open, the
pointer starting in the middle of the arrangement, by `tests/paths_rig` — real pointer and
key events; things found by the words on screen or, for a glyph, by pointing at each widget
of a region and reading the help line. **Clicks** count every press (a double-click is two,
a drag one), **keys** every chord or character, **travel** the pointer's distance in points.
Each path is a test in `tests/click_paths.rs` that holds its count.

| # | Path | Before | After (held) | FL Studio 24 | Ableton Live 12 |
|---|---|---|---|---|---|
| 1 | Add an instrument | + key, double-click the tile: **3 clicks**, 1 776 pt | right-click + ▸ *New track with Chorda* (recent): **2 clicks**, 831 pt · keyboard: Ctrl+Shift+P, "chorda", Enter: 8 keys | channel rack +, the plug-in in its list: 2 | double-click in the browser: 2 (a drag: 1) |
| 2 | Add an effect to a strip | select the strip, an empty insert row, double-click the tile: **4 clicks**, 2 919 pt | right-click the strip, *+ Add effect ▸* (now near the menu's top), *Insert Comprimo* (recent): **2 clicks**, 885 pt | select the track, the slot ▸, the plug-in: 3 | select the track, double-click in the browser: 3 (a drag: 1) |
| 3 | Open the piano roll on a clip | double-click: 2 clicks, 440 pt | same | double-click: 2 | double-click: 2 |
| 4 | Mute and solo a track | 1 click each: 2, 539 pt | same | 1 each | 1 each |
| 5 | Set the loop | drag in the loop lane, then Ctrl+L: **1 click, 1 key**, 614 pt | the drawn loop loops: **1 drag**, 614 pt (one undo step) | drag a time selection on the ruler: 1 | select, Ctrl+L: 2 |
| 6 | Arm and record | the arm lamp, R: 1 click, 1 key, 547 pt | same | arm, record, play: 3 | arm, F9: 2 |
| 7 | Route a send | the routing dot, the destination: 2 clicks, 1 196 pt | same | select the source, the destination's arrow: 2 | the send's menu, a return: 2 |
| 8 | Open an insert's editor | select the strip, its insert: 2 clicks, 1 829 pt | same (1 in the Wide desk, whose slots show) | select the track, its slot: 2 | 1 (the device view shows it) |
| 9 | Change the tempo | drag the well: 1 click, 289 pt | same | drag: 1 | click, type, Enter: 1 + keys |
| 10 | Save | Ctrl+S | same | Ctrl+S | Ctrl+S |
| 11 | Export | Ctrl+Shift+E, Enter, the system's dialog Enter: 3 keys | same | Ctrl+R, the dialog, *Start*: 2 keys + 1 | Ctrl+Shift+R, Enter, Enter: 3 |
| 12 | Find a preset and hear it | F8, the preset well, *Browse presets…*, the words, ↓: **2 clicks, 5 keys** | **Ctrl+F**, the words, ↓: **0 clicks, 5 keys** | the browser's plug-in presets: ≈ 3 clicks | Ctrl+F, the words, ↓: same |
| 13 | Settings ▸ Audio | Ctrl+, then *Audio*: **1 key, 1 click**, 294 pt | Ctrl+, opens where it was left, next session too: **1 key** | F10 / Options: 1–2 | Ctrl+, (the last tab): 1 |

FL's and Live's columns are their documented and usual ways (FL 24's channel-rack *+* list
and mixer slot menu; Live 12's browser, Ctrl+F and Ctrl+L), counted the same way; they were
not driven by a program.

The four shortened: **recent devices one click away** (`quick_devices`: the picker's own
*recently used*, five instruments in the add-track key's right-click menu, five effects at
the head of a strip's *+ Add effect ▸*, which moves from the foot of the strip's menu — below
the window's bottom at 1600 × 900, reachable only by scrolling the menu — to just under
Solo / Mute / Mono), **a drawn loop loops** (the switch goes on in the brace's undo step), **Ctrl+F**
(`BrowsePresets`), **Settings remembers its page** (`settings.settings_page`). The old
ways stay and are measured too (`p01a`, `p02a`, `p12a`).

## 2. Every area resizable, movable, collapsible, restorable, kept and scrollable

What the dock (ADR 0024 §5) already did: every splitter drags (right-click resets,
double-click toggles), every tab drags to another leaf or out to float, floating windows
snap, F5–F9 / I / Alt+F8… show and hide, *Reset layout*, named layouts, the tree kept
between sessions. What it lacked, and this round adds (`views/workspace_fold.rs`):

- **Collapse** (tab menu *Collapse area*; Ctrl+Alt+C; palette): the area folds to its tab bar — a bar in
  a column, a rail of the same width in a row (its glyph; a click opens it) — and *Expand*
  puts the split back exactly; a drag of the folded line unfolds it there. Unlike hiding,
  the area keeps its place, so a moved area comes back where it was. Kept with the layout.
- **Maximise** (double-click a tab, tab menu, Ctrl+Alt+Enter): the area takes the whole
  workspace; again, or showing/hiding any area, gives the others back. Floating windows
  stay. A layout saved meanwhile is the shape under it.
- The **tab menu** ignores the click that opened it (`TabMenu::fresh`): under the tests'
  input it closed in the frame it opened, the opening click counted as a click outside. On
  a real screen it probably opened (the area's first rect contains the pointer); not
  verified on one.

Tests through `assemble` (`tests/panels.rs`): a splitter dragged and the window closed —
the next session has it; the mixer collapsed to its tab bar (under 40 pt), the arrangement taking the room, kept
across sessions, expanded back to its height; the inspector to a rail and back by a click;
a double-click maximises and Ctrl+Alt+Enter restores; float and dock from the tab menu,
floating kept across sessions; **every area that overflows scrolls** (each of ten areas
shown in the default window; where words are cut at its bottom or right, a wheel — Shift
for the right — must move what is seen; the inspector and the Routing tab overflowed and
scrolled, and the test fails if the inspector's overflow is not seen, so it cannot pass on
nothing).

## 3. Keyboard reach

- **Every top-menu row** shows its shortcut or has a palette entry under the same words
  (`menu_commands`: 30 rows that are not keymap actions — imports, exports, help pages, the
  clip's colour and deletion, the device picker, the statistics, the timeline's switches,
  the area being worked in — run by the menus *and* the palette). The File menu shows
  New, Open, Save, Save as, Export with their keys. Palette entries may carry the menus'
  other words (`Entry::also`). A word typed whole now outranks scattered letters.
- **Every kit menu walks with the keys** (`menu_keys`, inside `kit::widgets::menu_list`):
  ↓ ↑ Home End, Enter or Space; the pointer and the keys share the highlight; a popup
  made of several lists (the track's menu) walks as one; a menu in front holds the walking
  keys only. **Shift+F10** opens what a right-click on the selection opens — the clip's
  menu, the track's, or the focused area's tab menu — outside the mixer, where it stays the
  route menu (the owner's round-25 answer); the strip's menu is the palette's *Open the
  menu of the selection*.
- **Focus between areas**: Ctrl+F6 / Ctrl+Shift+F6 in reading order; the focused tab's
  glyph is lit; the arrows' and M / S's focus follows.
- **Esc closes what opened last** (`esc`): the palette over Settings closes first, then
  Settings (which Esc did not close before); a tab menu closes and the selection stays.

Tests (`tests/keyboard_reach.rs`): the six top menus opened with the pointer and every row
read off the screen — each must show a key or match a palette entry; the timeline menu the
same; the clip, track, tab and mixer-strip menus opened **from the keyboard** and walked
row by row with ↓, Enter running *Maximise area*; Esc order; Ctrl+F6 order.

Pictures (`docs/round39/ux-b/`): `recent-instruments.png`, `strip-menu-effects.png`,
`ctrl-f-presets.png`, `mixer-collapsed.png`, `inspector-rail.png`, `mixer-maximised.png`,
`tab-menu.png`, `clip-menu-by-keys.png`, `track-menu-by-keys.png` — each the last frame of
its test (today's kit, Night, 1600 × 900).

## Files

New: `src/esc.rs`, `src/menu_keys.rs`, `src/menu_commands.rs`, `src/quick_devices.rs`,
`src/views/workspace_fold.rs`, `locales/areas/ux_panels.{en,es}.txt`,
`tests/paths_rig/mod.rs`, `tests/click_paths.rs`, `tests/panels.rs`,
`tests/keyboard_reach.rs`, ADR 0110.
Touched outside the files this builder owns (one-line or small, all marked `r39:ux-b`):
`kit/widgets.rs` (`menu_list` calls `menu_keys::nav`), `views/arrangement.rs` (the add-track
key's recent instruments; the drawn loop; Shift+F10's clip and track menus),
`views/mixer_menu.rs` + `views/mixer_strip.rs` (*Add effect ▸* moved up, its recent
effects; the desk's menus in the Esc order), `views/route_menu.rs` (the Esc order),
`tests/mixer_routing.rs` (one Esc added: Shift+F10 in the arrangement now opens the
selection's menu, which the test then closes),
`views/transport_menus.rs` (rows run `MenuCommand`s and show keys), `views/interop.rs` (its
File rows as functions), `views/picker.rs`, `views/preset_browser.rs`,
`views/palette_paint.rs`, `views/workspace_settings.rs` (Esc), `settings.rs` (one field),
`i18n.rs` (the area), `keymap.rs` (five actions, v16).

## Tests and checks

`cargo test -p cantus-ui` on Windows: **2 529 passed, 0 failed** (51 ignored), the 29 new
tests among them (`click_paths` 17, `keyboard_reach` 6, `panels` 6) and the unit tests of
`esc`, `menu_keys`, `menu_commands`, `workspace_fold`. `cargo clippy -p cantus-ui
--all-targets` with `-D warnings`: clean. Linux not checked by this builder (the round's
rule); nothing here is behind `cfg`.

## Verified only in emulation, and what is left

- Everything above was verified through `assemble` with synthetic input; nothing on a real
  screen with a real mouse. The OS's save dialog (export) was not driven. The owner should
  try: right-click the **+** above the tracks and a mixer strip after using a device once;
  draw a loop in the thin lane at the top of the ruler; Ctrl+F on a track; double-click a
  tab; *Collapse area* on the mixer; Ctrl+F6; Shift+F10 on a clip.
- The owner's judgement: the rail's look (today's tokens), whether five recent devices is
  right, Ctrl+Alt+Enter for maximise.
- Elided menu rows seen while reading the menus (*Create automation clip (last t…*, *Route
  menu of the select…*, *Link the last tweaked p…*) are ux-a's (truncation); reported, not
  changed here.
- The track header's menu is drawn as several boxed lists (boxes inside boxes, the look
  §6j names) and runs past the window's bottom at 1600 × 900 (`ux-b/track-menu-by-keys.png`):
  for the kit's new menu; the keys now walk it as one menu.
- *Add an effect on —* : the picker's subtitle names no strip when opened from the
  inspector's empty insert row (seen in `p02a`'s picture); not fixed here (the picker's
  title is outside this brief) — reported.
