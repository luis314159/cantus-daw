# Round 39 — builder `lo-rest`: the idle repaint, the rack's and the browser's leftovers

**Branch:** `r39/lo-rest` (worktree `F:\projects\daw-r39\lo-rest`), on `r39/integration`
`f473d0ba`. **Spec:** des-views' sheets and measured README
(`des-views` worktree, `docs/round39/design/des-views/`), and the "What is left" of
`el-views.md` and `el-cables.md`.

## 1. The idle repaint (the priority)

**What el-cables saw was its test rig, not the app.** `cables39_e2e`'s rig sends
`Event::ModifiersChanged` with *every* frame; egui draws again after any input event
(`InputState::wants_repaint_after`: `!self.events.is_empty()` → a repaint at once). A real
window sends that event only when a modifier changes. Through `CantusApp::assemble` on the
demo song, driven as eframe drives it (`eframe::App::ui`) with no events, the window's own
`repaint_delay` — whatever asked, the ticker or anyone calling `request_repaint` — is
`Duration::MAX` frame after frame once the start-up settles.

Checked caller by caller in the integrated tree (`grep request_repaint`, 70 sites): the ones
that run every frame are guarded by a change (the toolbar fold's `clips_chrome` drop, the
dock's device panes, the settings' loopback measure while it runs). One was not at rest but
was still at the screen's rate: **the browser's preview strip asked egui for a frame
directly while a sound played** (`ui.ctx().request_repaint()`) — it now goes through the
ticker (`Reason::Audition`, 60 Hz at most, ADR 0106 §6).

`tests/idle_repaint_e2e.rs` (own binary, through `assemble`, eframe's `App::ui`):
- `an_idle_window_asks_egui_for_no_frame` — after 240 frames of settling, 120 idle frames
  each ask for nothing sooner than a second, at 1920 × 1080 at 100 % and at 125 %; with the
  pointer resting over the arrangement; the mixer in front with **every cable drawn**; the
  piano roll; the channel rack; the browser open; the Sound packs window open. Then Play,
  and the same rig sees the frames a playing window asks for (the test can fail).
- `an_input_event_every_frame_is_what_asked_for_frames` — the same window fed
  `ModifiersChanged` every frame asks for a frame after each (what el-cables measured), and
  is quiet again once the events stop.

## 2. The rack

- **Plates and target marks at des-views' measures** (`channel_rack_paint::row_rects`, `row`):
  the lamp at 16, the knobs at 36 and 58, the mixer track's well 24 × 18 at 72, the plate at
  104, 150 × 20, r 3 — `raised` in Night, `raised` → surface 20 % in Paper, a 1-px foot; the
  selected channel's tinted with its hue (22 % / 20 %) and its name Medium; names left at
  +9; a silent pad's name in ink 3; the target mark at 262, 3 × 14, r 1.5, accent when
  selected, ink 4 at 35 % otherwise. A narrower column shortens the plate. The row is the
  surface with a 1-px base gap and the channel's **3-px stripe** (hue 72 %, full when
  selected), the selected row tinted 10 % / 12 % — across the grid too.
- **A kit's tree:** pads stepped in 14 with a 1-px tree line at 110 (a tick into each pad,
  ending at the last); the kit's plate carries a **chevron** and "3 pads" (ink 3, 10).
  The chevron **folds the kit's pads** under its own row (`ChannelRackView::folded`, a
  view's state; the kit's row keeps summing their hits).
- **The foot:** "Add a channel" in a 150 × 24 well (a plus, ink 2; lifted under the
  pointer) opens the instrument picker for a channel at the end; the **drop zone** across
  the grid, 56 high, dashed 4/3 in ink 4 at 90 %, r 6, "Drop a sound here for a new
  channel" / "or onto a channel to replace its sample" in ink 3 / ink 4, lit in the accent
  while a sound from the browser is held over it (the whole rack already took the drop:
  under the last row it makes a channel at the end). The rows' scroll reaches it.
- **A kit's hats by its pads' names** (`arrangement_paint_clip::Roles`): the clip content's
  hats (sitting back at 62 %) and the far zoom's pulse (kick, snare, clap) are read from the
  kit's pads — the name first ("hat", "hh", "ride", "charles"; "kick", "snare", "clap",
  "bombo", "caja", "palma"), then the voice (Kick, Snare, Clap) — and General MIDI's map
  only for a pitch no named pad plays. A kit that plays its hats on 60/61 sits them back; a
  shaker on 42 is not a hat. No allocation.

## 3. The browser

- **Search results grouped, with chips** (`samples_browser::items`): the hits grouped by the
  pack or folder they are in, each group titled (SemiBold 11, ink 3) with its count and a
  chevron; five a group until "Show the other N" (a link in the accent); each hit **36
  high**, its place ("Kicks › Hard") under its name in ink 3, 10. A group's title keeps the
  search to that pack or folder; a **chip** (20 high, r 10, accent 10 % / 35 % ring, a ×)
  says so, and another says the family tab chosen; a chip's click lets go. The tab's family
  filters the hits before they are grouped (the counts are true).
- **Section titles** in title case, SemiBold 11, ink 3 ("Packs", "My folders",
  "Favourites"), as the sheet has them (they were small capitals).
- **The preview** (`samples_detail::card`, 88 high): the name (Medium 13) with **its star**
  at the right, where it lives under it in ink 3 (its tags as small chips at that line's
  right where they fit), and the **waveform's well, 38 high**: `well` with a 1-px edge,
  lifted under the pointer; while it plays, the heard part in the accent, the rest
  well → ink 2 at 80 %, a 1-px ink 1 line where it is. The strip under it, with the card
  showing the name, says what the sound is in numbers ("48 kHz · Stereo") beside its
  position.
- **The star** keeps a sound in `Settings::sample_favourites` (this machine's preference,
  `#[serde(default)]`, not the song's — no schema change), listed first in the browser under
  *Favourites*; filled in `solo` when starred.
- **The Packs page's covers** (`samples_content::grid`, `pack_cell`): the Sound packs
  window shows each pack as a **128-px cover** drawn from what it holds (des-views' covers,
  r 8), grouped by category, with its name (Medium 12), what it is · how many files ·
  licence (ink 3, 10), and **its state**: *Installed* with a check in `play` on a disc in
  the cover's corner and *Open* (opens it in the browser's tree); *Installed* + *Update ·
  4 MB*; while fetched a 4-px bar, "42 % of 54 MB" and *Cancel*; or *Get · 140 MB* in an
  accent-tinted well — a click fetches it. A click on a cover (or its name and what it is) shows its details,
  licence and credits beside the grid, as before. The window opens wider (980 × 620) for three
  covers beside the details.

## Strings

`locales/areas/el_views.{en,es}.txt` (the views' area): the kit's pad count, the fold's
tip, "Add a channel" and its tip, the drop zone's two lines, "Favourites", the star's tips,
"Show the other {n}", "Get · {size}", "Update · {size}", "{pct} % of {size}", "Open", the
cover's tip.

## How it is tested

- `tests/idle_repaint_e2e.rs` (above).
- `tests/rack_foot_e2e.rs` — through `assemble` on the demo: a channel's name at 104 + 9 from
  the column, a pad's 14 further in; a kit's row counts its pads; its chevron at 104 folds
  the pads (fewer rows) and opens them; scrolled to the end, the foot under the last row:
  "Add a channel" 150 × 24 with its words inside, the drop zone 56 high with its two lines;
  a click on "Add a channel" opens the picker for `AddInstrument { index: None }`.
- `tests/samples_browser.rs`:
  `a_search_is_grouped_and_a_groups_chip_keeps_it_to_one_place` (groups and counts, five
  then the link, a 36-high hit with its place under its name, the group's title scopes it,
  the chip lets go), `the_preview_s_star_lists_a_sound_under_favourites` (the 38-high well,
  the name and place in the card, the star saved in the settings, *Favourites* first, the
  star taken away), `the_packs_page_shows_covers_and_get_fetches_a_pack` (a 128-px cover,
  "Get · 1 MB", a click on it installs the pack, then "Installed" and "Open");
  `a_chosen_sound_shows_its_card_and_its_waveform_plays_it` now finds the place in the card
  and the numbers in the strip.
- `arrangement_paint_clip::tests::a_kit_s_hats_are_read_from_its_pad_names`.
- `pack_missing::the_card_steps_aside_for_sound_packs_and_its_clicks_reach_the_window`
  clicks a pack's name in the window: a cell's name and line take the click as its cover
  does.

**Runs (Windows, debug info off):** `cargo test -p cantus-ui --no-fail-fast`: 2 601 passed,
54 ignored, 1 failed — `pack_missing`'s card test, which clicked a pack's name where the
list used to be; fixed (the cell's name takes the click), then `pack_missing` (11) and
`samples_browser` (18) pass. `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui
--all-targets`: clean. `cargo fmt -p cantus-ui` applied. The Linux check (WSL) was not
run here.

## Shots

`docs/round39/lo-rest/<view>-<night|paper>-1920x1080.png` at 125 %: `rack` (the channel
rack in front, the arrangement folded: plates, marks, the kits' trees, the foot), `rack-kit`
(the hats' kit chosen), `browser` (a pack open, a sound chosen, one starred), `search`
("kick": groups, counts, places, "Show the other 347 sounds"), `packs` (the Sound packs
window on this machine, every pack installed: the head's counts, filters and Sort, the
foot), `packs-get` (two installed and the packs still to get), `packs-getting` (one of
them shown being fetched, through the test hook).

Read beside des-views' `rack-browser-*` and `browser-pages-*` sheets: the rack's column now
reads as the sheet's (neutral plates, names at the left, a stripe, the tree, the chevron and
"3 pads", the foot); the browser's search and preview as the sheet's.
`cargo run -p cantus-ui --example shoot_lo_rest -- <dir> [case…]` renders them.

## The el-controls questions (the owner's, left as they are)

From `el-controls.md`: the automated knob's violet; the fader cap's weight; Paper's menus
and dialogs; value bars in the inspector; Mute on = grey.

## Session 2: the leftovers closed (the coordinator's list)

- **The Packs page's head and foot** (`samples_content::packs_head`, `packs_foot`): the
  counts ("8 free packs · 3 installed · 471 MB on disk", ink 3); the **filters** as one
  segmented control — *All · Installed* and each category the manifest has, in its order
  (`pack_filters`); **Sort** at the right (*Category* under the category titles, *Name*,
  *Size* largest first; a click takes the next, `PackSort`); the foot's line "CC BY packs
  show their authors on the pack's page and in Help › Credits." and **Where packs are kept
  ›** (a click shows the folder, its path in the hint). `ContentWindow::filter` / `sort`
  are the window's state, not the song's.
- **The preview's keys inside its card** (`samples_browser::card_controls`, the card now 118
  high as the sheet): play (tinted with `play` while it sounds), **loop** (a new
  `Previewer::looping`: a sound that ends starts again, from `samples_browser::frame`), the
  level as a 2-px slider with a 4-r thumb, and at the right "0:00.4 / 0:01.2 · 48 kHz ·
  Stereo" (ink 3, 10). The strip under the card no longer repeats them; it carries the
  hint or a notice. Without a card (a short panel) the strip keeps its keys as before. The
  level's tip no longer says the sound starts again (it has not since round 39's fixes).
- **One mark a row type** (`samples_browser::row_mark`): a pack's row its cover, a folder
  the folder glyph, a sound its waveform — or the audio glyph in the same place while the
  waveform is read; never the pack's cover on a sound (round 37's ADR 0103 §58 drew a
  pack's sounds with its cover until their waveform arrived, which packs-night showed as
  two kinds of row). The row's waveform is drawn to the sound's own loudest column (at most
  16 × up), as the preview is: a click with a quiet tail is a shape, not a row of dots.
- **The download in a shot:** `ContentWindow::shown_busy` (a `#[doc(hidden)]` test hook,
  never set by the program) draws one pack as being fetched; `shoot_lo_rest` case
  `packs-getting` shows the 4-px bar, "42 % of … MB" and *Cancel*.
- **Two BACKLOG entries** (`docs/BACKLOG-excellence.md` §6j's end): one browser for every
  kind (*All · Sounds · Plug-ins · Presets*, with the chips that depend on it), and a pad's
  own mixer track in the rack (a model change). **The owner decides both.**

**Runs, session 2 (Windows, debug info off):** `cargo test -p cantus-ui --no-fail-fast`:
2 602 passed, 54 ignored, 1 failed — `el_views_e2e`'s preview test read the length as the
whole text "0:00.5", now the start of the card's numbers line ("0:00.5 · 44.1 kHz ·
Mono"); fixed, then `el_views_e2e` (6) and `samples_browser` (18) pass. `RUSTFLAGS="-D
warnings" cargo clippy -p cantus-ui --all-targets`: clean. The Linux check (WSL) was not
run.

Tests added: `samples_content::pack_page_tests` (the filters and the three orders on the
bundled manifest); `samples_browser`: `every_row_type_has_one_mark`, the packs test now reads
the head and the foot, the fetching state through the hook ("42 % of 1 MB", *Cancel*) and
the Installed and category filters; the star test finds play, loop and the level inside the
card under the well, and the loop key toggles; the chosen-sound test reads its numbers in
the card.

## What is left

- **The browser's kinds** (*All · Sounds · Plug-ins · Presets*, with the chips that depend
  on it: grouping by kind, the length chip, "+ Filter"): a BACKLOG entry (§6j). The owner
  decides.
- **A pad's own mixer track** in the rack (the sheet's well on each pad): a model change, a
  BACKLOG entry (§6j). The owner decides.
- The Packs page's search field on the sheet ("Search sounds, presets, plug-ins") belongs
  with the kinds above: the page searches nothing of its own yet.
- Covers' hues come from the pack's id (el-views' rule), not hand-picked per pack.
- Everything here was read by me beside the sheets; the owner approves. Nothing was checked
  on a real screen or with a real download (the CPU renderer only).
