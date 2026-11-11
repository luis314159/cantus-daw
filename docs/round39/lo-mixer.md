# Round 39 close-out — builder `lo-mixer`: the mixer's and the inspector's leftovers

Branch `r39/lo-mixer` (worktree `F:\projects\daw-r39\lo-mixer`), from `r39/integration` at
`f473d0ba`. What it closes: el-mixer's "What is left", the integration's "track icons are
not yet on the mixer's strip heads", el-topbar's item 3 (the two glyphs), and the inspector's
second pass to des-mixer's `inspector-*` sheets
(`des-mixer/docs/round39/design/des-mixer/`).

## What changed

### 1. A track's own icon on the strip head (ADR 0113 §4, the icons sheet's mixer row)

- `Desk::info[i].glyph` (`views/mixer_desk.rs`): the icon of the first track on the strip
  that wears one, worked out with the rest of the desk (per revision; a pick is a command,
  so the desk sees it at once).
- `views/mixer_strip.rs` `head`: **Wide and Large** — 14 pt before the name, the two centred
  as one group (the name at 12, then 11 pt beside it); a name that would not fit beside it
  keeps its whole room and the icon goes beside the number instead; a two-line name takes
  it before its second line. **Compact** — 12 pt under the number and the arm dot, above the
  rotated name (whose room shortens by the icon's). Inks as the header's: the strip's colour
  when selected, toward ink 2 at rest, ink 4 muted or silenced; the second ink half-way to
  the head's ground (the selected tint, or the surface). Drawn from the library's drawing for
  the device size (18 px at 125 %, 15 in Compact), never scaled.
- `MixerView::head_icon(strip)`: where the head drew it last frame (for tests and tools).

### 2. The inspector, second pass (des-mixer's `inspector-*`)

New `views/inspector_look.rs` — the sheet's rows, painted one way everywhere:
- **field** (24 high, r 4, the well with a 1-px edge, a glyph, the value Regular 12, its
  detail Regular 10 ink 3, a chevron) — `Sheet::menu_row`/`menu_field`/`pick_field` and the
  new `menu_row_with` (a glyph, a detail) draw it; rows holding one are 28 high;
- **value** (label · a 3-pt bar up to 96 wide with its handle, from the middle with a centre
  tick when bipolar · the value Regular 11 at the right) — `Sheet::value`, so every parameter
  row (Level, an insert's values, a track's level and pan, the trim, the mix) reads alike;
  a modulator's live position is a tick across the bar;
- **switch** 26 × 14 (`Sheet::switch_row`: Safe from snapshots, Safe from solo, Low
  latency);
- **key** (a well, a lamp or a glyph, the word Medium 11) — `Sheet::led_row` now draws it,
  as wide as its word; the Freeze key is one;
- **text** (no well: a name Regular 12, a figure Regular 11 at the right) — `Sheet::text_row`.

The sections, in des-mixer's order:
- **Source** under the head: a 28-pt well — the track's own icon (else the instrument's or
  audio's glyph), the instrument's name Medium 12, the track's name (and "+ n tracks")
  Regular 10 ink 3, a chevron. A click on a strip chosen on the desk shows its track (as the
  Input row did); the chevron opens the instrument. A bus or a return: the strips routed in.
  The master: "The whole mix".
- **Inserts**: the rows as el-mixer drew them; **the mini view** of the sheet — a 46-pt panel
  under the row, `mix(base, surface, .55)`, three values (label Regular 10 ink 3, figure
  Regular 13 + unit Regular 10 ink 3, a 3-pt bar). A compressor: Threshold, Ratio and the
  live **Reduction** (in the solo colour, its bar from the right; 0.0 in ink 3 at rest; the
  row's own reduction readout hides while the view is open). Each value drags (Shift fine,
  double-click resets) and keeps Learn, Modulate…, automation and Alt-drag depth
  (`modulators::wire_drag`). An effect with no editor (Delay, Reverb): its first three as
  columns, the rest as rows. An EQ insert keeps its curve. The hue rule down the margin is
  gone.
- **The dragged insert row**: its place left as a quiet socket, the row lifted to `raised`,
  4 right and 2 down, over a soft shadow (σ 3); the drop line 2 pt in the accent.
- **Equaliser**: the graph 72 high inside the margins; under it des-mixer's band table — per
  band its colour's mark and name, its frequency (ink 2) and gain (ink 1), its Q under them;
  each figure drags as the wells did (the Q also by right-drag on the graph).
- **Sends**: as el-mixer drew them; an empty section is "+ Add a send" (the sheet's
  "Empty sections").
- **Receives** (Cantus's, not on the sheet — kept): open where other strips feed it; a strip
  only its own tracks feed folds it to one row ("None"). Its rows use the field.
- **Routing**: Output (the out glyph, a list), Input (the in glyph, who plays into it, its
  kind; a click shows the track), **Channel** — Polarity, Swap, Mono as glyph and word, an
  accent pill when on, wrapping under the label when they do not fit (the pinned strip's
  keys, kept) — then the delay compensation and the latency.
- **Sidechain**: Key into (the key glyph, the unit, its place) and From (the source's colour
  square, its name; the list that keys it elsewhere).
- **Recording**: the input field (in glyph, chevron), Monitor segmented, the trim, Armed,
  Low latency as a switch.
- **Automation**: **Mode** (Read · Touch · Latch · Write, the strip's track's
  `SetTrackAutoMode`), then a line per lane — its name, a 56 × 12 sparkline of the lane in
  the automation colour, "n points"; then **Show in the arrangement** (Regular 11 in the
  accent): `CantusApp::show_automation_in_arrangement` opens the track's automation lanes
  (one undo step; nothing when open), asks the arrangement to scroll the track and its
  lanes into view on its next frame (`ArrangementView::reveal`), and brings the arrangement
  forward with the dock's focus and the keys' (`pane_focus_wanted`, applied after the dock:
  unfolded if folded, its tab shown).
- **Level** — for every subject now (a strip, a track's strip, the master; a track straight
  to the master shows the master's), **in place of the pinned strip**, which is gone
  (`inspector_pinned.rs` removed): Volume (the selected strips move together, as the desk's
  and the pinned fader did), Pan, Width, the VCAs it follows, Mute · Solo keys, Safe from
  snapshots, Safe from solo. **A slim meter** under the volume (the pinned strip's meter,
  kept): two 2-pt bars on the volume bar's own travel (so the handle and the level read on
  one scale), green to −12, yellow to −3, amber to 0, the clip colour above, a 1-pt peak
  hold; the pinned strip's ballistics (up at once, down 24 dB/s, the peak held 1.5 s; kept in
  `Chan::level_meter`). Its 8-pt row is always laid out, but it is painted, and asks for
  frames through the ticker's `Reason::Meters`, only while something sounds into it — at
  rest it draws nothing and asks for nothing.
- **Freeze**: the key — the snowflake and "Freeze strip" (frozen: "Unfreeze", the state
  "Frozen") — and what it does, wrapped in Regular 10 ink 3; a bounced track's kept delay
  stays its row above. Frozen, a line of what is knowable: "3 inserts rendered" (a strip's
  playing inserts) and the render's size on disk ("41 MB", from the file the engine plays
  it from, else its first copy), worked out when the project or the render changes. No
  "CPU freed": nothing measures it. Held by `freeze::the_inspector_says_how_many_inserts_
  were_rendered_and_how_big_the_render_is` (a strip with a delay, a compressor and a
  bypassed EQ frozen as Freeze does it: "2 inserts rendered · <its file's size>"), seen in
  `lomix-insp-frozen-open-slate_night.png` ("2 inserts rendered · 12 MB", *Unfreeze*).

### 3. The Large EQ's band dots on real data

`EqThumb::dots`: the strip's own three bands where they shape it (by octaves over 20 Hz …
20 kHz), else the first EQ insert's first three playing bands; Large puts each dot on the
curve where its band is (`look::eq_curve`), coloured solo / play / accent.

### 4. The header in the workspace's tab row

The workspace records where each bar's tabs end (`Workspace::tab_ends`, from
`on_tab_button`); before the mixer is drawn docked under a bar with 420 pt or more to the
right of its tabs, it hands the mixer that room (`MixerView::bar_room`). The desk then has no
header row of its own: the density, Rows, Cables and its options, the solo chip, Snapshots
and `+` sit on the tab row, over the dock's own ground, in their own clip. Floating, too
narrow, or drawn bare (tests, tools): the header keeps its 28-pt row. Tabs drag, double-click
and open their menu as before (the header's widgets are only right of the tabs).

### 4b. The overflow pill in a slot of its own, and FL's next inserts

- **"5 ›"** (the coordinator's look at `lomix-large-slate_paper`): the middle's overflow pill
  — how many strips are scrolled out that way, a click pages to them — sat at the middle's
  edge, vertically centred, over the last visible strip's meter. Where the middle scrolls,
  it now has a 36-pt slot each side (`size::OVERFLOW`, always both, so nothing moves when
  one first scrolls), the middle still a whole number of strips; the pill sits centred in
  its slot (`Geo::slots`). Held by `mixer_desk::the_overflow_pill_never_covers_a_strip`
  (64 strips at 1400 pt, each density, at both ends: no strip's column under either pill).
- **The desk fills its width** (FL's mixer, the coordinator's look at the integrated
  Compact shot: half the desk was dead space): where the desk does not scroll, the room up
  to its edge holds the next strips' places, numbered on from the last, in the empty
  strip's look (the quiet body, the number in ink 4, a groove, the cap's ghost)
  — after the right dock, or after the add column with none (`paint::ghosts`). They are
  only painted: nothing in the project, nothing in the engine, nothing that repaints (the
  desk's frame as before). A click on one makes the strips up to it in one undo step
  (`CantusApp::add_strips_up_to`, a `Batch` of `InsertStrip`) and selects it, ready to be
  routed to. Held by `mixer_desk::the_desk_fills_its_width_with_the_next_empty_strips` (1920
  and 1280 wide, every density; Compact at least three; filled to under two pitches from
  the edge; the project's strips unchanged; a click on the third makes three, one undo
  step, the last selected; undone).

### 5. The glyphs el-topbar named

- The device group: the family's **speaker** (set 4, `glyph_browser::Glyph::Speaker`) in the
  key's inks, in place of `Glyph::Device` (`views/audio_chip.rs`).
- The More panel's foot: a new **sliders** glyph (`kit::Glyph::Sliders`, family name
  `sliders`) — drawn in set 5's grammar for 16, 20, 24 (1-px tracks, each cut by a pixel
  round its knob) and 30, 36 (2-px round strokes), in the design source
  `docs/round39/design/icons/glyphs-r-rest2.js`, generated into `kit/glyph_rest_data.rs` by
  `tools/icons/glyph_rest.js` (now listing it); the kit's own drawing is the fallback.

## Tests

- `tests/mixer_icon_e2e.rs` (new, through `CantusApp::assemble`, 1920 × 1080):
  `an_icon_chosen_in_the_picker_is_painted_on_the_strip_head` — the track's menu ▸ Icon…,
  "piano" typed into the picker, its cell clicked; then in Compact, Wide and Large the strip
  head paints it (a textured quad in its place, inside the head, 12 / 14 pt, the name whole
  after it, no other strip wearing it); undone, it leaves the head.
  `the_header_shares_the_tab_row` — the density on the Mixer tab's line, right of the tabs;
  a click on Large there sets the density.
  `show_in_the_arrangement_reveals_the_tracks_lanes` — the last track (out of sight below
  the docked arrangement) with a closed automation lane, its strip chosen on the desk;
  Automation ▸ *Show in the arrangement* clicked: its lanes open (one undo step), its row
  scrolled into view with room for them, the arrangement holding the dock's focus and the
  keys'; undone, the lanes close.
- `views/mixer_desk.rs` `the_band_dots_follow_the_strips_real_bands` — a strip whose EQ is
  not flat (+2 / −2.5 / +1.6, the bell moved to 632 Hz) puts its dots at its bands'
  frequencies, the bell's at the curve's dip; flat, none; an EQ insert's playing bands take
  their place.
- Rewritten for the Level section (the pinned strip is gone) and the folded sections:
  `learn_everywhere::sends_and_the_level_section_offer_learn`,
  `modulate_everywhere::sends_and_the_level_section_offer_modulate`,
  `modulators::a_handle_dropped_on_the_level_volume_connects_the_volume`,
  `modulators::the_level_section_shows_its_modulation_on_its_bars` (the probe's live mark
  is now "bar"), `final_look::the_level_section_never_runs_a_caption_into_its_value`
  (a strip chosen, Level open, 1160–1420 pt, both languages: no two words on its rows
  touch), `panels::every_overflowing_area_scrolls` (the sections opened and a track chosen;
  where the inspector's last row ends whole at the edge, a wheel must still move it).
- `mixer_routing::a_receives_key_row_names_the_insert_as_the_chain_line_does`: unchanged;
  the row's field leaves the words the room the key glyph takes.

**Results.** The whole `cargo test -p cantus-ui --no-fail-fast` ran at `ada90504`: 2 589
passed, 8 failed, all in tests this branch then fixed (the band table's figures cut at
220 pt and in Spanish at 150 %, the receives key row, the "Add a send" row's hint, the
inspector's overflow guard, the two new e2e tests' own searches, the Level caption test);
the 20 affected binaries ran again green at the head (`inspector`, `final_look`,
`ux_whole_words`, `ux_help_line`, `mixer_routing`, `panels`, `mixer_icon_e2e`,
`learn_everywhere`, `modulate_everywhere`, `modulators`, `frame_budget`,
`workspace_budget`, `slate_e2e`, `id_clash`, `mixer39_e2e`, `mixer_desk`, `at_rest_e2e`,
`kit_glyphs`, `kit_lint`, `topbar`). The lib's 520 (the band dots and set 5's sliders
among them) green. `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui --all-targets`
clean. The frame budget and "at rest costs nothing" tests (`frame_budget`,
`workspace_budget`, `at_rest_e2e`, `slate_budget` in the full run) are green: nothing new
repaints at rest; the icons are layer quads cached per (icon, size, inks), the dots read the
desk's per-revision thumbnail.

**Follow-up (the coordinator's five asks), at the head:** the lib's 520 and `at_rest_e2e`,
`final_look`, `frame_budget`, **`slate_budget`**, `workspace_budget`, `freeze` (92),
`id_clash`, `inspector`, `kit_lint`, `learn_everywhere`, `mixer39_e2e`, `mixer_desk`,
`mixer_icon_e2e` (3), `modulate_everywhere`, `modulators`, `panels`, `ux_help_line`,
`ux_whole_words` — all green; clippy with `-D warnings` clean. The Level meter adds no frame
at rest (`at_rest_e2e`, `workspace_budget`, `frame_budget` unchanged and green).

## Shots (`docs/round39/lo-mixer/`)

`cargo run -p cantus-ui --example shoot_lo_mixer -- <dir> [filter]` — the whole window on the
demo song, playing, 1920 × 1080 at 125 %, Slate Night and Slate Paper; the tracks wear their
icons, Chords' strip EQ is shaped: `lomix-{wide,compact,large,docked}-*` (the mixer),
`lomix-insp-{strip,track,bus,master}[-open]-*` (the inspector on Chords' strip, Chords'
track, the Drum bus and the master — as they start, and every section open, the
compressor's mini view open; these are shot 1920 × 2700 so the whole inspector shows, with
Automation's *Show in the arrangement* and Level's meter live while the song plays),
`lomix-insp-drag-*` (the first insert lifted, its drop line under the second).
`compare-inspector-{night,paper}.png` (`tools/look/lomixer_compare.py`): des-mixer's three
inspector columns at ×1.25 beside the built one.

**Compared with the sheets.** The mixer shots read as `mixer-*` and `strips-*` did after
el-mixer, now with the icons on the heads (Compact under the arm dot; Percussion's,
whose name needs the head's width, beside its number), the header on the tab row as
`mixer-wide-*` draws it, Large's dots on the shaped curves (Kick, Chords, Lead). The
inspector follows `inspector-*` section by section; what differs, on purpose:
- Cantus keeps what the sheet does not draw: the inserts' power and the EQ's (*Flat* is
  its lamp), an insert's Mix and Sidechain rows under its mini view, Receives, the delay
  compensation and latency in Routing, the trim and the Armed key in Recording, the Q under
  each band, Mute · Solo in Level, the Modulators fold.
- Labels sit in an 80–120 pt column (the sheet's 64 cut the Spanish labels), so fields
  start further right than the sheet's.
- Fixed after looking: the EQ graph ran past the margins, the band table cut "100 Hz" on a
  narrow panel, the mini view kept round 37's hue rule, the Freeze key's word was cut, a
  bus's source said "(none)", the empty Sends section was a well, the Receives rows and
  the sidechain's two lists shared an id (egui flagged it in red).

## What is left

- **Engine features, in BACKLOG:** the sheet's Sidechain *Hear the key* switch and
  Recording's *Record: Before FX · After FX* — the engine has neither (no key audition, no
  recording tap after the inserts); both are entered in `docs/BACKLOG-excellence.md` (end of
  §6j) with what the engine needs and their null tests.
- **Freeze's "CPU freed"**: nothing measures what a freeze saves; the inserts rendered and
  the render's size are shown.
- **The right dock's place** (el-mixer's list): it keeps following the strips (round 37's
  judge), and the room after it is now filled with the next empty strips (§4b), so there
  is no dead space either way; the next strips carry no socket ring (they have no route
  until they are made).
- **Verified only in emulation** (the CPU renderer and headless egui): the header on the
  tab row with real tab drags and floating windows on a real screen (`panels` holds them
  headless), the icons and the sliders/speaker glyphs at 125 % and 150 % on a real screen.

