# Slate — direction A (round 39, `dir-a`)

**The idea.** A cool, quiet grey-blue room. Panels are separated by *value*, never by
borders: a dark gutter between panels, a lighter bar for headers and strips, a darker body
for lanes. Keys are recessed wells a shade darker than their bar, with a glyph about a third
of the key's height in a soft light grey and a second ink for edges. There is one accent, a
calm azure, spent only on state: loop on, snap on, the playhead, focus, an open window.
The user's track colours stay a 3-px stripe and a low tint at rest. They light fully on the
selected track (Chords: its header, clips, strip and inspector), so the selection is the
window's focal point. Clips under the playhead brighten a step while playing. Type is one
condensed family with a clear hierarchy, from a large thin clock down to 9-px unit tags.

## Why it beats FL Studio and today's Cantus

- **Against today's Cantus (beta.13):** the warm brown-black and the saturated slabs are
  gone (BACKLOG §6j items 1 and 2). The mixer has no label column; each strip reads from
  top to bottom, and only the master keeps a dB scale (item 3). Idle shows grooves and
  positions: no "−∞", no black meter slabs (item 5). The clock is large and thin, and the
  type has five sizes and four weights where today's has one of each (item 6). Clip content
  is drawn at full lane height: notes in rows ranked by pitch, drums as velocity pills,
  audio as filled envelopes (item 7). The selected track is lit across the whole window,
  which gives it a focal point (item 8). Every glyph is drawn per key size (item 4 and the
  button study). The inspector shows two used insert slots and one "Add an effect" line,
  where today's lists ten slots.
- **Against FL Studio:** it is as calm and as cleanly finished. It keeps what Cantus
  already does better: numbers on every strip, label/value units in the inspector, a full
  EQ curve with its band table, and a hint line that is never empty. FL tints whole strips
  and saturates its track tiles. Slate says the same through a stripe and a tint, so a
  13-strip mixer reads quieter, and the eye goes to the one lit track. The overflow panel
  has no equivalent in FL. It folds three groups into a laid-out sheet: one grid, each
  shortcut under its own key, live graphs with their numbers.
- **The honest risk:** Slate is the calmest of the three hypotheses. Beside FL's
  whole-strip tints it can read as *quiet* rather than *rich*. The relief (lit cap on knobs
  and faders, recessed wells, one lit edge per bar) is there to keep it from reading plain.
  The owner's eyes decide (see `research/ui/round39/dir-a/compare-*.png`).

## The system in short (full page: `system.png`)

| | Night | Paper |
|---|---|---|
| gutter / base / surface / raised | `#14191E` `#1D2329` `#262D34` `#2B3239` | `#C5CDD5` `#F4F6F8` `#E4E9EE` `#FBFCFD` |
| well / well under the pointer / edge | `#1A1F25` `#20262D` `#11151A` | `#D6DDE3` `#DDE3E8` `#B9C3CC` |
| ink 1 / 2 / 3 / 4 (on surface) | `#E3E8ED` 11.3:1 · `#B4BDC6` 7.3 · `#909CA8` 5.0 · `#47515B` 1.7 | `#17202A` 13.5 · `#38444F` 8.2 · `#556170` 5.2 · `#A9B3BD` 1.7 |
| accent | `#5CAFE9` 5.8:1 | `#1B64B2` 4.9:1 |
| play / rec / solo / clip / auto | `#5DCE8A` `#EE5C5A` `#EBB64D` `#FF5D50` `#B48DF4` | `#1F8F55` `#D33B3B` `#A86E06` `#D6322B` `#7A4FD6` |

- **Ink 3 reaches 4.5:1 on every surface it sits on** (phase B): Night `#909CA8` is 4.6 on
  raised, 5.0 on surface, 5.7 on base and 5.9 in a well. Paper `#556170` is 4.6 in a well,
  4.9 in a well under the pointer and 5.2 on surface. Ink 2 moved up a step with it, to keep
  the hierarchy. Ink 4 is for disabled glyphs and idle marks only, never for text someone
  must read. The Night popover (`raised`) went one step darker, and the Paper hover well
  one step lighter, so that ink 3 still reaches 4.5:1 on them.
- **Type:** Barlow Semi Condensed (SIL OFL 1.1, version 1.408). The design source is
  https://github.com/jpt/barlow; the files were taken from https://github.com/google/fonts
  (`ofl/barlowsemicondensed`). They are kept in `F:\projects\daw-r39\fonts\barlow-semi-condensed\`
  with `OFL.txt` and `METADATA.pb`. Tabular figures are on. Scale: clock ExtraLight 30
  (ticks Light 15 in ink 3); values Regular 15; titles Medium 13; names Regular 12;
  labels Regular 11; section titles SemiBold 11 in ink 3, in Title case (never capital
  bars); units Regular 10; tags SemiBold 9.
  IBM Plex Sans Condensed was compared first (`src/specimen.html`) and left for a more
  technical direction.
- **Grid:** a 2-px unit, with layout on even numbers so that 150 % lands on whole pixels.
  Keys are 16, 20 and 24 px high. Rows: track 30, panel head 28, top bar 44, status 20.
  Mixer strips are 60 px wide at a 62-px pitch. Radii: 2, 3, 4, 6, 8. Glyph strokes:
  1, 1, 1.5, 2 and 2 px for keys of 16, 20, 24, 30 and 36 px.
- **States of a key:** at rest, a recessed well with an ink-2 glyph. Hover lifts the well
  a step and turns the glyph ink 1. Pressed darkens the well to its edge colour. On gives
  an accent tint and an accent glyph (green for play, red for record). Disabled is the key
  at 55 %. A radio choice (a tool, a segmented control) is shown as a lifted neutral pill,
  never in colour.

## What it costs to build (for the kit builder)

- Everything is flat fills, 1-px edges and a few gradients: the knob cap (radial), the
  fader cap and the strip's light from above (linear), each with two or three stops.
  egui's painter covers all of it. Popovers and menus need one soft drop shadow (a blurred
  rect; the kit already has shadows).
- Each glyph is drawn per size from a table (`src/glyphs.py`): a box, a stroke and the
  details each size keeps. The gear has 6 teeth below 24 px and 8 from 24 px. The magnet
  loses its pole caps at 16. The play loses its lit facet at 16. The metronome loses its
  weight at 16. Turning these tables into kit code is mechanical. Hinting the diagonals
  (play, loop arrows) to the pixel grid at 16 and 20 px is the part that needs care.
- Clip content at full height needs pitch-rank rows per clip, cached per clip, and audio
  peak envelopes. Both exist in some form today; this design draws them larger.
- The font is a new OFL asset. It needs its notice (`cargo run -p xtask -- notices`) when
  the kit adds it.
- The mark is now drawn for each size (phase B, `src/logo.py`, `b/b-mark-*.png`). The
  `icons` agent's family replaces the glyphs here, which are placeholders. Slate's tokens
  (ink 1/2, ink-on accent) are meant to colour any family.

## Files

| File | What |
|---|---|
| `window-night-1x.png`, `window-night-1.5x.png` | the main window, Cantus Night, idle at bar 41 (the second drop), 1280×720 and 1920×1080 |
| `window-paper-1x.png`, `window-paper-1.5x.png` | the same in Cantus Paper |
| `window-night-playing-1x.png`, `window-paper-playing-1x.png` | the window playing: meters, the play key lit, the clips under the playhead a step brighter |
| `overflow-night-1x.png`, `overflow-night-1.5x.png`, `overflow-paper-*.png` | the window with the top bar's overflow panel open (Metronome, Windows, Performance, Customise top bar…) |
| `parts-night-1x.png`, `parts-paper-1x.png` | the parts sheet at 1:1: transport, tool and window keys at 16, 20 and 24 px and 150 % (24, 30, 36 px), the states, knobs (rest, hover, dragged, automated), fader and meter (silent, playing, clipping), strips, tabs, field, segmented, toggle, scrollbars, a menu with its submenu |
| `parts-*-4x-transport.png`, `-tools.png`, `-windows.png`, `-knobs-fader.png` | the same sheet cut and enlarged ×4 with nearest-neighbour (the real pixels) |
| `system.png` | the system on one page: tokens with hex and contrast, the user's colours at rest and lit, type, spacing, radii, key states, both themes |
| `src/` | the design source: `base.py` (tokens, primitives), `glyphs.py` (each glyph per size), `topbar.py`, `arrange.py`, `song.py`, `mixer.py`, `inspector.py`, `overflow.py`, `parts*.py`, `system.py`, `compare.py`, `zoom.py`; `render_all.sh` rebuilds every PNG with Edge headless (`--disable-lcd-text`, so text is greyscale like the app's) |
| `research/ui/round39/dir-a/compare-playlist.png`, `compare-mixer.png` | private: FL Studio 24.1.2 \| Slate Night at 150 % \| Cantus beta.13, all three 1920 px wide |
| `research/ui/round39/dir-a/compare-overview.png` | private: both rows at half size, with Slate Paper beside FL's mixer and beta.12's Paper |
| `research/ui/round39/dir-a/compare-topbar.png`, `compare-mixer-detail.png` | private: the top bars and the mixers at 1:1 physical, one above or beside the other |

Note: `startup.png` (beta.13) is 1280×720, so the comparison enlarges it ×1.5 (Lanczos). The
sheet says so. Slate's 150 % view is rendered natively.

---

# Phase B — build sheets for the kit (the owner chose Slate, 2026-11-09)

Slate keeps its own values: base `#1D2329`, an azure accent, and no teal in the chrome
(teal appears only as the user's colour on the Chords track). It never moves toward FL's
greys. Every sheet is rendered at 1:1. Where pixels matter there is also a ×4 cut with
nearest-neighbour scaling, and a native 150 % render: glyphs and the mark are drawn at 24,
30 or 36 px through `glyphs.nat`, not scaled from the 1× drawing.

## The top bar (`b/b-topbar-widths-*.png`, `b/b-topbar-groups-*.png`)

**Geometry.** The bar is 44 high on `surface`, with one lit top edge
(`mix(surface, ink1, 7 %)` in Night, `#F7F9FB` in Paper). Every key is 24 high at y 10.
Groups are 10 apart, with 12 at the edges. The left cluster packs from the left and the
right cluster from the right, and there are at least 24 px between the two clusters.
Keys are recessed wells: fill `well`, a 1-px `edge` at the top and a 1-px `lip` under it,
radius 4. Joined keys share one well and are divided by the edge colour, inset 4.

**Groups.** Widths are in logical px. Left to right:

| Group | Full | Compact | Measures and type |
|---|---|---|---|
| Menus and project | 34 + menus (≈ 204) | without the summary | mark 24; menus 12 Regular ink 2, 13 apart, baseline 19; name 13 Medium ink 1, baseline 36; summary 11 ink 3. Minimal: mark + ≡ key 28 + name |
| Pattern / song | 74 | — | segmented 34 + 40; chosen = raised pill inset 2, radius 3, 11 Medium ink 1; other 11 Regular ink 3 |
| Transport and loop | 160 | — | well 122 (30 · 32 · 30 · 30) + 6 + loop 30. Play 9×8 and loop 15×11, hinted. States: stopped, playing (green tint 18 %), count-in (beats in a 13×12 rec pill), recording (rec tint 22 % and a lit dot), loop off |
| Metronome | 46 | — | key 30 + options 16; on = accent tint 20 % and an accent glyph |
| Tempo and metre | 116 | 78 (no metre) | tempo well 78: 15 Regular ink 1 + `.000` 11 ink 3 + `BPM` 9 SemiBold ink 3, tracked 0.6; metre well 34, 12 ink 2. Typing: a 2-px accent ring and an accent caret. Tapping: `TAP` in accent |
| Position | ≈ 124 | ≈ 73 (no time) | bar.beat 30 ExtraLight ink 1, baseline 34; ticks 15 Light ink 3; time 12 ink 2 over “bar · beat” 9 Medium ink 3; no well; recording turns the numbers rec red |
| Grid and snap | 80 | 40 | magnet key 28 + grid 52 (12 Regular, chevron at 70); compact = magnet + chevron |
| Undo and redo | 56 | — | two keys 28; unavailable = glyph in ink 4 |
| Windows | 196 | — | seven keys 28; open = accent tint 16 % and an accent glyph |
| CPU, RAM, voices, dropouts | 142 | — | graph 34×12 + `7 %` 12 Regular; RAM 12 Regular. Busy (≥ 70 %) in solo amber; dropouts as a 14×12 rec pill with the count |
| Inputs | 112 | — | MIDI lamp r 2.5 (halo r 4.5 at 20 %) + “MIDI”; arm ring r 3.5 + “Armed” + count (rec red Medium when above 0) |
| Master meter | 132 | 92 (no label) | mode 10 Medium ink 3; two bars 3 high, 2 apart; ticks at −48 −24 −12 −6 0. Idle shows grooves only, never “−∞”. Clip: a 3×9 clip lamp |
| Audio device | 104 | 30 | speaker at 20 px + name 11 ink 2 + chevron. No device: solo-amber warning and the words |
| Tools and zoom | 84 | — | picker key 28 + zoom well 52 |
| Settings, More | 28, 30 | — | More carries the number of folded groups in a 13×11 raised pill, 8.5 SemiBold ink 2 |

**Fold order** (`topbar2.FOLD`): tools → inputs → CPU/RAM → windows → device compact →
metronome → meter compact → tempo compact → position compact → menus compact →
pattern/song → grid compact → menus minimal → undo → meter. The result at each width:

| Width | Folded into the panel | Compact |
|---|---|---|
| 1920 | nothing | nothing |
| 1600 | CPU/RAM, inputs, tools | — |
| 1440 | + windows | — |
| 1280 | windows, CPU/RAM, inputs, tools | device |
| 1152 | + metronome | tempo, meter, device |
| 1024 | + pattern/song | menus, tempo, position, meter, device |
| 900 | (same) | menus minimal, tempo, position, grid, meter, device |
| 800 | + undo | (same as 900) |

## The overflow panel (`b/b-overflow-*.png`, the window: `overflow-*-1x.png`)

The panel is built from the bar's folds (`overflow2.panel`). Each big group becomes a
section: Metronome, Windows, Performance, Inputs, Tools and zoom. The small folds become
one-line rows under “Also here”: pattern/song, time signature, grid, undo, master meter.
Up to three sections share one column; from four on there are two columns, balanced by
height, so the panel never outgrows the window.

- Columns are 284 wide, with 16 of padding and a 24 gutter that holds a 1-px divider.
  The panel is `raised`, radius 8, with one soft shadow and a nub pointing at the More key.
- Title “Folded here” 13 Medium, with the reason at the right (11 ink 3). Section titles
  are 11 SemiBold ink 3 in Title case. Rows are 26 apart: label 11 ink 2, control at 68 or
  100.
- Shortcuts are keycaps: outline only, 9 Medium ink 3, 13 high. Each window key has its
  shortcut under it. The panel's own help line names the key under the pointer.
- Performance shows four cells (CPU, memory, voices, dropouts), each a small graph with
  its number (15 Regular). A busy CPU turns amber; dropouts show as rec-red ticks with
  “last 0:42 ago”.
- States drawn: metronome off (its rows at 45 %), two tracks armed, MIDI silent, CPU busy,
  three dropouts, and a key with the keyboard focus (a 2-px accent ring outside the key).
- Footer: “Customise top bar…” is a line with the sliders glyph, not a button.

## The toolbar (`b/b-toolbar-*.png`)

A panel's tool row is 28 high, with 20-px keys at y 4, gaps 12 and edges 12. On the left:
the panel's name (13 Medium), the tools (a radio set of six 26-px keys), and the grid
(“Grid” 11 ink 3 + a 58 well). On the right: Follow (a 24 key + “Follow” 11) and the panel
menu (a 24 key).

- The chosen tool is a lifted `raised` pill with a 1-px light top. Hover is `well_hi` with
  an ink-1 glyph. Tools are never coloured.
- Fold order: the grid loses its word → Follow loses its word → the name becomes its glyph
  → the tools become the chosen one plus a chevron. Drawn at 1032, 860, 700, 560, 420 and
  340 px.
- The grid menu lists values in musical order, with a check on the current one, Triplets
  (Shift 3) and “Follow the zoom”.
- Help-line texts are given on the sheet.

## The mark (`b/b-mark-*.png`, ×4: `b/b-mark-*-4x.png`)

`src/logo.py` redraws logo #2's idea for each size: 16, 20, 24, 30, 32, 36 and 48 px.

- At 16 the C is one solid arc. At 20 and 24 it is a thick arc cut into teeth. From 30 up
  it has real spokes of alternating length.
- The playhead and its crossbar sit on whole pixels; the cap appears from 24.
- The tile is the logo's own warm black: the only warm note in the slate bar, and Cantus's
  identity.
- The taskbar-icon tints (Amber, Verdigris, Blue, Silver) recolour only the wave.

## Hinted placeholders

`play_hinted` and `loop_hinted` (`src/glyphs.py`) set play and loop pixel by pixel at 16,
20 and 24 px. Edges are whole pixels or half-covered pixels in a regular rhythm. At 30 and
36 px, `loop_rect` keeps the same rectilinear shape with a 2-px line. These are placeholders
until the `icons` agent's family is approved.

## Settings, first-run setup, export (`b/dlg-*-{night,paper}-{1x,1.5x}.png`)

**Dialog frame** (`src/dlg_kit.py`): a scrim over the window (Night `rgba(6,9,12,.58)`,
Paper `rgba(32,44,56,.34)`). The frame is `surface`, radius 10, with one shadow (dy 14,
blur 22), a lit top edge and a title row 44 high (15 Medium, close key 26×24). A side list
sits on `base`; its chosen row is a raised pill with a 2×14 accent bar. Rows are 32 apart,
with a 150-px label column (11 ink 2). Section titles are 11 SemiBold ink 3, 26 above
their rows. Buttons are 28 high:

- primary: an accent fill with `accent_ink`, 12 Medium;
- secondary: a well with ink 1;
- quiet: words in ink 2.

Fields and drop-downs are 24 or 22 high wells; focus is a 2-px accent ring.

- **Settings** has eight pages: General, Audio, MIDI, Appearance, Shortcuts, Plug-ins,
  Library and files, About. Two are drawn whole:
  - **Audio:** interface with a test tone; driver; rate; buffer, with the latency drawn as
    a green, amber and grey scale and a sentence; inputs and outputs with a live meter;
    measured latency; audio cores; reopen when the device returns.
  - **Appearance:** the three themes as thumbnails (Slate the default; Graphite and gold and
    Studio ink as options); the mode (Night, Paper, follow the system); interface size;
    colours; the taskbar icon in five tints.

  The footer says what is running; there is one primary button, Done.
- **First-run audio setup:** four numbered steps on a thin line — choose (device rows with
  a “Recommended” tag), listen (test tone and meter), play (input meters, and which input
  moves), latency (in words, with Measure). Footer: Skip (quiet), Done (primary).
- **Export:** presets on the left (Streaming, CD, Stems for mastering, Podcast, Custom,
  each with its hint), then What, Format, Loudness and Tags. The footer gives a one-line
  summary (format, loudness, length, size, path) with Cancel and Export…. The exporting
  state shows the phase, a 4-px accent progress bar, what is done, and Stop.

## For the owner to judge

1. The **size at 100 %** (`research/ui/round39/dir-a/compare-b-topbar.png`). On a 1920
   screen, Slate's bar at 100 % is much smaller than FL's. FL's “100 %” matches our 150 %.
   I propose a default interface size of 125–150 % on 1920-wide screens. That is a decision,
   not a drawing.
2. Whether the folds go in the right order for your work (the widths sheet).
3. The mark at 24 px in the top bar and in the ×4 cut.
4. The overflow panel's two columns at 1280.
5. Settings, first run and export, whole, in both themes. Settings is also beside FL's in
   `research/ui/round39/dir-a/compare-b-settings.png`.

## Phase B files

| File | What |
|---|---|
| `b/b-topbar-widths-{night,paper}-1x.png` | the bar at 1920, 1600, 1440, 1280, 1152, 1024, 900 and 800 px, with what folded and what went compact |
| `b/b-topbar-groups-{night,paper}-{1x,1.5x}.png` | every group, its forms and states, with its measures; ×4 cuts `b/b-topbar-groups-*-4x-{brand,transport,tempo,perf,meter,device,more}.png` |
| `b/b-overflow-{night,paper}-1x.png` | the panel at 1600, 1280, 1024 and 800, and two state panels |
| `b/b-toolbar-{night,paper}-{1x,1.5x}.png` | the toolbar at six widths, every tool chosen, hover, Follow, the grid menu; ×4 `b/b-toolbar-*-4x-tools.png` |
| `b/b-mark-{night,paper}-1x.png`, `-4x.png` | the mark at each size, beside the original |
| `b/dlg-settings-audio-*`, `dlg-settings-look-*`, `dlg-firstrun-*`, `dlg-export-*`, `dlg-export-progress-*` | the dialogs, whole, Night and Paper, 1× and 1.5× |
| `research/ui/round39/dir-a/compare-b-topbar.png`, `compare-b-settings.png` | private: beside FL at the same scale |
| `src/topbar2.py`, `overflow2.py`, `toolbar2.py`, `logo.py`, `dlg_kit.py`, `dialogs.py`, `dialogs2.py`, `dmain.py`, `sheets_b.py`, `zoom_b.py`, `compare_b.py` | the phase B sources; `src/render_all.sh` rebuilds everything |
