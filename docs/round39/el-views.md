# Round 39 — builder `el-views`: des-views' views in the app

**Branch:** `r39/el-views` (worktree `F:\projects\daw-r39\el-views`), on `r39/integration`
`6e6c9b24` (the Slate kit, ux-a, ux-b, memory). **Spec:** `des-views/README.md` (the measured
spec) and its sheets; icon set 4 (`icons/spec-browser.md`, `glyphs-b-browser*.js`).

## What changed

### The room (`views/room.rs`, new)
The design's names for a theme's tokens — gutter, base, surface, raised, well, well_hi, edge,
lip, ink 1–4, accent — and the keyboard's new materials (`sep`/bed, `ivory`, `ivory_hi`,
`ebony`, `ebony_top`, `ebony_front`, the lit edge, the C inks), **derived from the tokens**,
not spelled out: in Slate they land on the design's values (a test holds them within a few
levels), in Graphite, Studio ink and high contrast they follow those themes' own tokens.
No token was added to the kit.

### The piano roll
- **The keyboard as an instrument** (`views/piano_roll_keys.rs`, new): ivory and ebony with a
  real piano's widths (C–E over five rows, F–B over seven; only E|F and B|C on row lines);
  each white key its own piece with a **1-px gap** where the bed shows, front corners round
  (r 2, 1.5 under 10-px rows), a lit top and a shade at its foot; the depth gradient across
  every key (back in shadow, front lit in Night, its edge falling away); black keys 0.60 deep,
  one row high, their body from ebony to the lit front, **a 1-px lit edge** along the upper
  side and the front end, **two soft shadow steps** onto the whites. Every C named in its key,
  right-aligned 5 from the front — **middle C SemiBold**, other Cs Medium 10, every white key
  from 16-px rows. **States:** the key under the pointer (accent 16 % / 30 %), the key heard by
  a click (accent 55 % / 70 %, from `CantusApp::auditioned`), the keys sounding while the
  song plays (the track's colour 55 % / 75 %). The keyboard is 56 / 72 / 88 wide at 8 / 12 /
  18-px rows (`keys::width_for`).
- **The grid:** white-key rows in the base, black-key rows toward the gutter, the octave line
  under each C, a quieter line at E|F, bars / beats / snap steps (only 6 px apart or more), the
  row under the pointer lit (accent 7 %), a gutter veil outside the clip.
- **Notes:** the row's full height, a 1-px edge, r 2; fill by velocity (Night 50 % + 22 % ×
  v, Paper the hue darkened 38 %, 58 % + 30 % × v); a brighter **attack column**; the name
  inside in SemiBold 9 from 20 px wide; **selected** lifted and ringed in ink 1; **under the
  pointer** a light ring; **muted hollow** (edge in the hue, a 10 % fill, its name in ink 3).
- **Ghosts** (`views/piano_roll_ghosts.rs`, new; on by default, a *Ghosts* lamp in the head):
  the other clips' notes behind this one — the same track's in its hue at 20 % / 22 %, other
  tracks' in slate — inset a pixel, no edge, never hit; **the head names whose they are**
  ("Pluck · Lead · Choir", three and an ellipsis). Strings in `areas/el_views.{en,es}.txt`.
- **The velocity lane:** over the ladder and lines at 127 and 64, a stem a note in its fill
  (2 px selected) and a short cap (ink 1 when selected), muted dotted and hollow; one stem a
  pixel column (a chord one clean cap).
- **The ruler:** on the surface; bar numbers SemiBold in ink 1 with a 9-px tick, beats named
  "41.2" in ink 3 where there is room (`Digits::fmt_pair`, no allocation), sixteenths 3-px.

### The channel rack
- **Steps** (`channel_rack_paint.rs`): a well with a 1-px edge, r 3; beats 2 and 4 a step
  lighter; lit, tinted with the channel's hue (42 %) and **filled from the floor to its
  velocity** (45 % + 55 % × v) at 80 % (86 % for the chosen channel), a lit top; muted
  channel 18 % / 30 %. Beats and bars parted by the gaps round the wells, not lines.
- **A melodic channel shows its pattern as a small roll** in one well over the page (notes 3
  high, ranked by pitch, its hue 78 % / 95 %), beat and bar lines inside.
- **Two bars a page** when fitted (`channel_rack.rs` `navigate`): 128 steps no longer pack into
  stripes; the scroll bar pages through the rest.

### The browser
- **Rows** (`samples_paint.rs`): the selection a lifted well tinted with the accent and a 2-px
  accent stripe; the pointer's row the lifted well; **tree guides** as hairlines; a sound's
  **own waveform** (8 columns every other pixel, 10 high; accent when chosen or sounding) as
  its icon; names in ink 1 (folders, packs) and ink 2 (sounds); the sounding speaker.
- **Pack covers drawn from their content:** drum rings, a kick's sweep, vowels, a keyboard, a
  square wave, contour lines, an impulse response, a groove — on a gradient of the pack's hue
  (darkened 30 → 62 %), the motif in the hue lifted 30 %, a faint light ring.
- **Icon set 4** (`kit/glyph_browser.rs`, new): folder / open, file, audio, MIDI, preset,
  instrument, effect, pack, star / favourite, search, history, clear, speaker, check, download,
  **each at every size its designer drew** (12, 14, 16 → 18, 21, 24), rasterised once per
  (glyph, size, inks) into the layer atlas, one quad on the pixel grid (ADR 0108 §3). The
  table is generated from the design source with the sheets' own helpers
  (`tmp/elv/gen_glyphs.js`, evaluated in node in the sheet's load order).

### The arrangement's clip content at four zooms
`views/arrangement_paint_clip.rs` (new), chosen by pixels a beat (`Zoom::of`):
- **MIDI:** far (< 6) a **pitch contour** per pixel column (lowest to highest note sounding,
  into a scratch buffer, runs merged into one rect); default (6–20) rows ranked by pitch, a
  monophonic melody **linear in pitch** (rows 3–6); near (20–60) true length, **brightness by
  velocity**, melody rows 4–7; close (≥ 60) plus a brighter **attack column**, rows 5–8.
- **Drums (Pulsus):** far the **pulse** (a 1-px tick a kick or clap, as tall as its
  velocity; every hit for a clip of hats alone); default one row a piece with 2-px ticks,
  **hats at 62 %**; near and close hits at their length by velocity.
- **Audio** (`arrangement_audio.rs`): far the loudness alone (the envelope's smoothed mean);
  default the peak envelope at 62 % with the loudness inside it as its texture and a centre
  line; near and close **peak and loudness in two tones** (55 %, +25 % white), stereo apart in
  lanes ≥ 44, **"L" / "R"** when close.
- **Automation** (`arrangement_auto.rs`): far a 1.2-px line over its fill, no points; default
  2-r points; near 3-r points; close the curve handles always.

### Session 2 (`el-views2`, resumed): the budget, the warm frame, the leftovers
- **The dancer's budget test** (`frame_budget::stage_budget`) failed only in a whole-suite run
  and only some of the time: the kit's tokens are **process-wide** (`kit::tokens::install`),
  and the other tests in the same binary build applications (whose first frame installs the
  saved look, Slate Night) or apply the old Slate on other threads. A dump of every frame's
  triangles by clip rect and of the installed tokens showed the whole window — top bar,
  status line, inspector — re-themed in the middle of the dancer's runs (panel `#4E585E`
  against `#1D2329`, +3 100 triangles), not a painter growing with the dancer. Fixed in the
  harness: every test in `frame_budget` holds one look at a time (`one_look()`, re-entrant on
  a thread, held by every `Rig`); the bounds are untouched. Four suite runs out of four: 47 491
  hidden / 47 499 shown. (Round 37's "a whole inspector section came and went" was the same.)
- **The arrangement's warm frame:** the rows' summary of a clip's notes (the pitches in use,
  whether two sound at once) is cached per clip, keyed on the project's revision and the note
  count (`clip_content::Summary`, the arrangement's `Scratch::summaries`): the stress song's
  eight 20 000-note clips are no longer walked every frame.
- **The toolbar folds** as dir-a's sheet says (`clips_chrome::fold`): each part says what it
  takes at every fold, the row remembers those widths (egui's memory, by the row's id;
  written only when one changes) and the next frame starts at the first fold that fits —
  the grid's word first, then the panel's name becomes its glyph (`Toolbar::named`, the
  name in the tooltip), then the tools become the chosen one and a chevron that lists them
  all (`Toolbar::tools`, the piano roll's seven). Edges are 12 px. `PianoRollView::bar_fold`
  says how far it folded.
- **The rack's pattern map** at the head's right: eight bars of 22, the page in view a raised
  pill, each beat's hits as a 2-px column (counted once per edit), a click pages there.
- **The browser's search:** the match is lit in the accent and a heavier weight inside the
  row's name (`samples_paint::find_ci`, no allocation; the head, the match and the tail
  painted as one run).
- **Clip states** (`arrangement_paint::clip_chrome`): a 13-px name strip in lanes of 40 and
  taller (des-views says 44; this branch's default lane is 40, the merged tree's 44 — both
  keep it); a shorter lane names a clip over its content only when it is selected or 80 px
  wide at the near and close zooms; **playing** (under the playhead while the song plays) a
  step brighter with a lit top; **muted** a 1-px hatch in ink 4 at 35 % every 6 px, drawn
  only where the clip is on screen.
- **The rack's audio rows:** an audio channel's pattern is one well over the page with its
  sound's peaks in it, a column a physical pixel, in the channel's hue at 72 % (through the
  tempo map, the clip's offset and slip). The first cut never drew: it asked the rack's
  `clip_on`, which takes note clips only — the end-to-end test found it; the row now finds
  the audio clip under the pattern itself.
- **The browser's preview numbers:** where the sound is and how long it is ("0:01.2 /
  0:03.4", ink 3) between its name and the level.

### Small additions outside my files (additive, marked `r39:el-views`)
- `kit/brush.rs` — `Brush::hrect`, a rect whose colour runs left to right through stops (the
  keys' depth and the black keys' bodies). No other painter changes.
- `kit/mod.rs` — `pub mod glyph_browser`.
- `app.rs` — `CantusApp::auditioned(track)`: the pitches a click is sounding (read only).
- `i18n.rs` — the `el_views` locale area.

## How it is tested

- `views::room::tests` — Slate's derived keyboard and inks are the design's (within a few
  levels); every C name reads on the ivory in every shipped theme (C4 7:1, the others 4.5:1).
- `kit::glyph_browser::tests` — every glyph of set 4 has a 100 % and a 150 % drawing, every
  drawing parses and inks pixels; the drawing picked is the one made for the size.
- `views::samples_paint::cover_tests` — a pack's motif says what it holds.
- `clips_chrome::digits_tests` — "41.2" without allocating.
- `clips_chrome::fold_tests` — dir-a's arrangement row at ten widths from 1032 to 200 px:
  nothing folds at 560 and wider, the folds only grow as it narrows, every fold is reached and
  the narrowest is the tools'; the first fold that fits is the least folded.
- `samples_paint::cover_tests::a_search_match_is_found_in_any_case`, `samples_browser::clock_tests`.
- `frame_budget` — every test holds one look at a time (see session 2): the dancer's test
  agrees with itself in whole-suite runs.
- `tests/el_views_e2e.rs` — **through `CantusApp::assemble`** on the demo song: the piano
  roll's keyboard in Slate Night and Paper read from the rendered pixels (ivory, ebony, the
  1-px gaps between white keys); the rack's lit steps in their channel's hue; the browser and
  the arrangement at the four zooms render; the piano roll's toolbar folds in a 620-px window
  (to its name's glyph at least) and unfolds when it widens again. Session 2's end, through the
  same boot (the start page off): the rack's map shows one 22-px cell a bar and a click on the
  third bar pages there; an imported tone's channel draws its sound in the rack
  (`ChannelRackView::audio_columns`); a search for "kick" paints "Kick" in the accent beside
  "Deep" in the row's ink, and the chosen sound's preview reads "0:00.5"; a 44-px lane names
  its clip in the 13-px strip and a 30-px lane at the far zoom does not; playing lights the
  clip's top (read from the pixels against the stopped frame); muted, a line under the notes
  crosses the hatch.
- Kept green: `panels`, `ux_help_line`, `ux_whole_words`, `samples_browser`, `views_budget`,
  `slate_budget`, `workspace_budget`, the lib's 495 tests.

## The frame budget

`tests/views_budget.rs` in release, the stress project (eight 20 000-note clips), the base's
(`6e6c9b24`) and this branch's test executables run alternately three times on the same busy
machine — best of three (session 2, after the rows' summary cache):

| Warm frame | base | el-views | allocations (base → here) |
|---|---|---|---|
| arrangement | 0.86 ms | **0.34 ms** | 75 → 75 (21.9 → 21.0 KB) |
| piano roll | 0.52 ms | 0.36 ms | 34 → 36 (the ghost names' galley) |
| piano roll, whole piece (20 000 notes) | 1.02 ms | 1.06 ms | 34 → 36 |
| channel rack | 1.99 ms | 1.36 ms | 65 → 65 (21.3 → 19.1 KB) |

The arrangement had been 1.06 ms against 0.51 in session 1 (before the visible-range skip came
back), then 0.56 against 0.56 with it; the per-clip summary (the pitches in use and whether
the clip is a melody, worked out once per edit instead of walking every note of every clip a
frame) took it to a third of the base's best here. The first measure of the whole piece had
been 8.37 ms (the ghosts walked every other clip's every note); bounded to the view, as plain
rects, and left out past a sixteenth a pixel, it is the base's. `slate_budget` (whole 1080p
window, release, noisy machine, session 1): Slate Night p50 1.36–1.53 ms, under beta.13's
Cantus Night in the same runs. `frame_budget` (every deterministic budget, the dancer's
included) and `workspace_budget` pass.

How the base was measured: a detached worktree of `6e6c9b24` built into this branch's target
folder. Its release artifacts take the same names as this branch's (the path is not in their
hash), so after it cargo took the base's binary for fresh in this tree — `cargo clean
--release -p cantus-ui` and a rebuild, then the two executables copied apart and run in
turn (the allocation counts, 34 against 36, say which is which).

## Pictures

`docs/round39/el-views/<view>-<night|paper>-<1280x720|1920x1080>.png` — the whole window
with each view in front: `song`, `pianoroll`, `rack`, `browser`, `clips-far`,
`clips-default`, `clips-near`, `clips-close`; 1280 × 720 at 100 % and 1920 × 1080 at 125 %;
the piano roll and the rack also at 760 × 560, where the toolbars fold (session 2: the
arrangement's and the piano roll's names are their glyphs there, the roll's tools the pencil
and a chevron, the grid without its word).
`cargo run -p cantus-ui --example shoot_el_views -- <dir> [case…]` renders them.

## Failing on the base too (not this branch)

On `6e6c9b24` itself: `app_flows` (2), `click_paths::p09_change_the_tempo`, `font_coverage`,
`keyboard_reach::every_row_of_the_menus…`, `kit_gallery` (Paper glass, 3), `topbar` (2),
`about::help_credits…` — the kit's title-case menus and the top bar's changes, not views.
`memory_e2e::what_a_start_holds` measured 12.9 MB on the base as here.

Session 2's whole `cantus-ui` run (debug, Windows): the lib's 502, `el_views_e2e` (3) and
every other target pass but those, all on the base's list. `pitch_editor`'s narrow-bar test
compared a band title ("Tonic") with its case: the 12-px edges fold the key into ⋯ at 480 px
now, which brought that check into play; it compares without case, as the others do since
session 1.

## Verified only in emulation

Everything is drawn by the CPU renderer (`raster.rs`). The sounding and heard key states and
the ghosts were checked in code and tests, not on a playing machine.

## What is left

- **Automation values** at the close zoom ("688 Hz", Medium 9, ink 2) are not drawn:
  `arrangement_auto.rs` is being rewritten by `el-arrange` (277 lines), so they belong in that
  file after the merge — the integrator's or `el-arrange`'s, not done here to spare a
  conflict.
- **The toolbar's other folds:** Follow losing its word applies to a lamp with a glyph; the
  piano roll's lamps (Snap, Ghosts, Score) are words only and keep them (three bare lamps
  could not be told apart). The grid menu's musical order with Triplets and "Follow the
  zoom" is `arrangement::grid_menu`, `el-arrange`'s file.
- The rack: the foot's "Add a channel" well and drop zone, the name plates' and target marks'
  last measures; a kit's pad names are not read for the drums' hats (found by GM pitch).
- The browser: result chips and groups, the Packs page's 128-px covers with their state
  (installed, get, getting), the preview's waveform well (38 high) and star.
- The piano roll head's clip menu (track stripe › clip) and the selection readout.
- Every shot was read beside des-views' and dir-a's sheets by me only; the owner approves.
