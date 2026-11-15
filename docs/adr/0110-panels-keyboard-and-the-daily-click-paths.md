# ADR 0110 — Panels, the keyboard's reach and the daily click paths

**Status:** accepted (round 39, builder `ux-b`) · **Date:** 2026-11-09 · **Schema:** none
(every shape this ADR keeps lives in the layouts file and `settings.json`, not in the song)
· **Keymap file:** version 16 · **Amends:** ADR 0024 §5 (the workspace: areas fold,
maximise and take the keyboard's focus), ADR 0072 (the palette: menu rows, aliases, the
whole-word match), ADR 0041 (Shift+F10 outside the mixer) · **Relates to:** BACKLOG §6j
("every click path the owner uses daily timed and shortened … every panel resizable and
movable; keyboard reach")

## Context

Round 39 is UX and graphics (BACKLOG §6j). Its UX half asked three things of this builder:
the owner's daily click paths measured and the long ones shortened, every panel resizable,
movable, collapsible and restorable with its shape kept, and every menu action reachable
from the keyboard with Esc closing what opened last. Before this round nothing measured a
path: the round-27 gestures (ADR 0051) had been designed against FL Studio's, but no test
counted what a path costs, so a later change could lengthen one unseen. The dock (ADR 0024
§5) already resized, moved, floated and kept its tree between sessions; it could not fold
an area while keeping its place (hiding forgets where a moved area was) or give one area the
whole window. The top bar's menus mixed rows that show their shortcut with rows that have
none and no palette entry; the context menus could only be used with the mouse; and Esc
was read by each surface on its own, so one press could close a dialog *and* the menu over
it, or clear the selection behind both.

## Decision

1. **Paths are measured through the window.** `tests/paths_rig` drives
   `CantusApp::assemble` at 1600 × 900 with real pointer and key events and counts
   **clicks** (a press of a button: a double-click is two, a drag one), **keys** (a chord or
   a character) and **pointer travel** (points, from the middle of the arrangement). Things
   are found as a person finds them: by the words painted, or — a glyph with no word — by
   pointing at each widget of a region and reading the help line's name. Each daily path
   is a test that holds its count (`tests/click_paths.rs`); the table with FL Studio's and
   Ableton Live's ways is in `docs/round39/ux-b.md`.
2. **The long paths are shortened where FL or Live are shorter.**
   - The add-track key's right-click menu lists **the instruments used last**, and a mixer
     strip's *+ Add effect ▸* — moved from the foot of the strip's menu (below the window at
     1600 × 900) to under Solo / Mute / Mono — opens on **the effects inserted last**
     (`quick_devices`: the picker's own *recently used*, five of each). Two clicks add a
     track with yesterday's instrument (3 with the picker) or put a compressor on a strip
     (4 through the inspector and the picker).
   - **A loop drawn in the loop lane loops**: the switch goes on in the same undo step as
     the brace (FL and Logic draw a loop that plays; it took the drag and Ctrl+L).
   - **Ctrl+F** (`BrowsePresets`) opens the preset browser on the selected track's
     instrument (Live's Ctrl+F); it took F8, the preset well and *Browse presets…*.
   - **Settings opens on the page chosen last**, in the next session too
     (`settings.settings_page`): Ctrl+, is Audio's for whoever goes there.
3. **Every area folds, maximises and takes the keyboard's focus** (`views/workspace_fold`).
   *Collapse area* (a tab's menu, Ctrl+Alt+C, the palette) shrinks the area's side of its split to the
   tab bar — a bar's height in a column, a rail of the same width in a row — keeping the
   split's old position for *Expand area* or a click on the rail; a drag of the folded line
   unfolds it where it is let go. The folds are kept with the layout (`Layout::folds`,
   serde-default: older files load unfolded). The dock's least size (`separator.extra`,
   96 pt) is lowered to the fold's size while one is folded, or `egui_dock` would undo it
   every frame. *Maximise area* (a double-click on a tab, the tab's menu, Ctrl+Alt+Enter)
   gives the area's leaf the whole main surface and the same again — or showing or hiding
   any area — gives the rest back; a layout saved meanwhile is the shape under it.
   **Ctrl+F6 / Ctrl+Shift+F6** move the focus between the areas on screen in reading order
   (left to right, then top to bottom); the focused tab is lit, as it was, and the arrows'
   and M / S's focus follows it to the mixer, the arrangement or the editors.
4. **Every menu row is reachable from the keyboard.** Rows that are keymap actions show
   their shortcut (`action_item`: the File menu's New, Open, Save, Save as, Export too);
   the others are `menu_commands::MenuCommand`s that the menus *and* the palette run, under
   the menu's own words, so a row and its entry cannot differ. Palette entries may carry
   other words (`Entry::also`): the Options menu's *Audio settings…* finds *Settings:
   Audio*. A typed word found whole at the start of a word now scores above the same
   letters scattered (+40), so "chorda" finds *New track with Chorda* before *Check for
   updates*.
5. **Every kit menu answers the keys** (`menu_keys`, in `kit::widgets::menu_list`): ↓ ↑
   Home End move a highlight over the rows that act, Enter or Space runs it, the pointer
   and the keys share it. A popup that draws its rows as several kit lists (the track's
   menu is seven) is walked as one menu, per layer. A menu in front holds the keys it walks
   with — the arrows, Home, End, Enter, Space, Esc — while it is open (the keymap does not
   also nudge notes or launch a slot); every other shortcut still works. **Shift+F10** —
   the keymap's *Route menu*, which acted only in the mixer — opens elsewhere what a
   right-click on the selection opens: while the arrangement is being edited, the selected
   clip's menu, else the selected track's; in an area that is not an editor (the
   inspector, a browser, the history…), its tab's menu. In the mixer it stays the route
   menu (the owner's round-25 answer); the strip's own menu is the palette's *Open the
   menu of the selection*.
6. **Esc closes what opened last** (`esc`): a surface Esc closes says each frame it is open
   and asks whether this Esc is its own; the newest one takes it and the key is taken out of
   the input, so nothing older closes with it, and the keymap's *Clear selection* waits
   while any is open. Registered: the tab menu, the palette, Settings (Esc closes it now),
   the shortcuts, the layout's name, and — said open so nothing under them takes the key —
   the device picker and the preset browser.

### Amended, round 39 after beta.14 (2026-11-11): the bar's menus are kit menus, kept in the window

The owner's screen showed the bar's *View* menu running off the foot of a 1080-px window with
nothing to scroll, its rows egui's own check boxes and buttons (two heights, boxed entries);
File, Edit, Clip and Help were the same mix — a kit menu per row beside stock buttons.

- **One kit menu each.** File, Edit, Clip, View and Help are rows (`Row`: words, check, the
  action whose key shows, enabled, what it does) drawn by one `kit::widgets::menu_list`
  (`transport_menus::bar_menu`): one row height, each action's key at the right, an
  unavailable entry dimmed — never a button. A click closes the menu, then does the row's
  work (`run_action`, a `MenuCommand`, or the menu's own: undo, the layouts, the settings).
  View's *Layout* submenu became a band of its own rows; Clip's colours stay swatches between
  two lists of one width. *Import character…* is always enabled: with no dancer selected it
  says so on the status line, as the Mac's bar does.
- **Kept in the window.** Every bar menu runs inside `kit::menu::fit`. `fit` now sets the
  scroll area's `min_scrolled_height` as well as its `max_height`: egui sizes a popup's
  first frame in `default_area_size` (400 pt tall), and the scroll area took those 400 as
  all its room, so any menu taller than 400 scrolled even with room for all of it. It is
  now as tall as its content up to the window, and scrolls past that.

### Amendment, round 40 (`look`): maximise keeps the inspector — a setting

The owner's answer 2 (2026-11-12): maximise **keeps the inspector beside the area** (des-arrange's
`max-*` sheet), and it is **configurable**. *Settings ▸ Appearance ▸ Layout and controls ▸ Maximise keeps*:
**The inspector beside it** (the default) or **Nothing: the whole workspace** (round 39's). A setting
of this machine (`settings.json` `look.maximise`), never the layout or the project.

- With the inspector kept, the maximised surface is the area's leaf split with an inspector leaf on
  the side it stood, at the share of the main surface it had (`Workspace::toggle_maximise`,
  `workspace_fold.rs`); the inspector is not in the docks' row. When the inspector is a tab of the
  area's own leaf, or floats, or is hidden, nothing is added.
- The inspector's key (`I`) while it is kept hides it alone — out of the maximised surface and of the
  shape under it — and the area stays maximised; any other area's key gives everything back, as before.
- Changing the setting while an area is maximised re-maximises it the new way at once.
- Held by `tests/owner_look_e2e.rs` (`maximise_keeps_the_inspector_unless_the_setting_gives_the_whole_workspace`):
  through `assemble`, Ctrl+Alt+Enter's action keeps the inspector at its side and width; `I` hides it
  alone; Settings' *Nothing* gives the whole workspace; kept across a restart.

## Consequences

- A daily path that gets longer fails a named test; a menu row with no key and no palette
  entry fails `every_row_of_the_menus_is_reachable_from_the_keyboard` by its words; a
  context menu the keys cannot walk fails by name.
- The keymap file is version 16: an older file gains Ctrl+F6, Ctrl+Shift+F6,
  Ctrl+Alt+Enter, Ctrl+Alt+C and Ctrl+F where they are free.
- `menu_keys::holds_keys` makes the keymap leave the walking keys to a kit menu shown in
  front. A list drawn inside a panel never holds them; a future persistent list in a
  foreground area would, and must not be built with `menu_list`.
- What only the owner can judge: whether the fold's rail and the maximise gesture feel
  right, and whether the recent lists' length (five) is right. The look of the rail and the
  menus is today's kit; the new look replaces it through the kit.

## What the tests hold

`tests/menus_in_window.rs` (the bar's View menu, opened by its word: whole at 1600 × 900 —
taller than 600 pt, its last row painted, no word cut — a check switching its area and
closing the menu; in a 520-pt window inside it all the same and scrolling to its last row;
without the `fit` amendment it fails "the menu is 400 pt tall"),
`tests/click_paths.rs` (each path through `assemble`, its count held),
`tests/panels.rs` (a dragged splitter kept across sessions; collapse to a bar and to a rail
and back where it was, kept across sessions; maximise by a double-click and back; float and
dock from the tab's menu, kept; every overflowing area scrolls),
`tests/keyboard_reach.rs` (every top-menu row and every timeline-menu row reachable; the
clip, track, tab and strip menus opened from the keyboard and walked row by row; Esc closes
the newest surface and keeps the selection; Ctrl+F6 walks the areas in reading order),
and unit tests in `esc`, `menu_keys`, `menu_commands`, `views/workspace_fold`.

### Amendment, round 40 (`fix-b`, the mid-round review): *Layout and controls*

Settings ▸ Appearance's band of the owner's look answers was titled *The window* — the only
band named with an article, holding things that are not the window (the mixer's dock, the
inspector's values, Mute's colour). It is **Layout and controls** (*Disposición y controles*),
with a note under the title as its neighbours have ("How the workspace is laid out and how a
few controls are drawn. Choices of this computer, not of the song."), and its five choices
share one width so their right edges line up. *Mute on* reads **Lit mute button** (*Botón M
encendido*).

The inspector's **value bars** draw no edge line under the number and its unit (the fill's own
edge shows the value there; stubs above and below "−3.5 dB" read as a line through it), and
a send's level is a value bar too under the default (it stayed round 39's slim bar beside
Level's value bars); *Inspector values: Slim* keeps round 39's for both.

## Amendment — review 2 of round 40 (builder `fix-d`)

- **The inspector's send bar starts where Level's do** (R2-B4). A send's value column was the
  name's fixed 74 pt after the lamp, 4–5 pt right of Volume, Pan and Width; it is now Level's
  column (the label column's width and the gap after it), the name filling up to it, in both
  *Inspector values* styles (the slim one now Level's slim bar too).
- **A value under its figures keeps a mark** (R2-B4). Where the value's edge falls under the
  number and its unit, the line is left out so the digits read whole (mid-round C-2), which left
  only the fill's edge — faint in Slate Night. Now a small notch at the bar's top and bottom
  edges points at the value, a caret's height clear of the digits (`kit::slider::value_bar`).
- **A field's list stays in the window** (R2-B6). `fx::frame::popup_list` opens under its field
  when it fits there, above it when it fits there instead; a list the window can hold whole is
  raised until it does (over its field); a longer one scrolls, under its field while eight rows
  find room there, else above it. Delay's 27 note values reached y 898 of a 900-pt window.
  Tested: `delay_window_e2e::the_note_list_stays_inside_a_short_window` (720 pt);
  `fx_impetus::the_key_is_chosen_in_the_editor` now looks for the list above its field too.
- **The next strips' run reads apart from the right dock** (R2-B7; the C-1 decision kept their
  true numbers, a click on one making the strips up to it). With the dock at the window's edge
  the run ends 18 pt before the dock's divider, and a ghost's number is fainter than an empty
  strip's (ink 4 at 60 %). The "16" after the returns in the default desk is a real strip — the
  default project's third return, empty — not a ghost.
- **Settings ▸ Appearance's *Top bar extras*** (R2-B5): the Spanish segments' short words had
  lost what they are (the arm, not automation; the computer's keyboard, not a MIDI one), and
  the long ones elide in a four-way choice. The segments keep short words (*Armado*, *Teclado*)
  and a note under the row says what each is, in both languages.
