# Round 32 — the instruments' faces (builder: instruments)

Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda and Pulsus, each with a face
of its own (BACKLOG §6h point 1, ADR 0074). The sheets are data in
`crates/cantus-ui/src/kit/skin/instruments.rs`; the characteristic displays live in each
editor's files.

## Design notes

Each: what it is in sound · the reference it is *inspired by* (never a copy of a product's
dress, logo or name) · colour and material · characteristic display · control shapes ·
lettering. Hues are the accent's in a dark theme; every colour is a pair (dark, light).

**Unda** (wavetable flagship). Morphing digital tables, modern and wide. Inspired by the
dark-glass wavetable synths (FL's Harmor and the modern software flagships). Deep indigo
glass (surface tint 224°), accent cyan 190° with a magenta second voice 305°; material
Glass. The table in 3D glowing on a phosphor display. Value rings without bodies (Arc),
pill switches. Wide-tracked medium capitals; a stripe under the name.

**Calor** (analog polysynth, "heat"). Warm subtractive voices. Inspired by 70s/80s
polysynth panels: a brushed panel with screen-printed legends, machined knobs. Warm charcoal
(22°), accent orange 24°, second amber 48°; material Brushed. Its filter curve and envelopes
on a scope's graticule. Knurled caps, rockers, screws, printed rules; tracked capitals in a
legend box.

**Modulus** (FM / additive). Glassy, metallic, precise. Inspired by 80s digital FM synths:
a matte umber panel, membrane keys, a display of operators. Umber (18°), accent teal-green
155°, amber 35°; material Matte. Operators, graph and spectrum on a liquid-crystal display
(the 80s FM synths' own). Membrane keys with a printed colour bar that lights (its own
switch painter), soft knobs; condensed capitals, mono readouts.

**Exemplar** (multisample sampler). Recorded instruments mapped by zone. Inspired by
hardware samplers' anodised cases and liquid-crystal windows. Grey-violet (262°), accent
violet 268°, teal 175°; material Anodised. Its zones and waveform glowing on a phosphor screen,
in its own violet (after the first sampling workstations' CRTs). Arc knobs, pills; sentence-case semibold, mono readouts; an engraved name.

**Gravis** (bass, "heavy"). Strings, synth and 808 bodies. Inspired by bass amplifiers:
black vinyl amp covering, a blue pilot light, a badge. Near-neutral black (220°), accent deep blue 222°,
red 356°; material vinyl amp covering (a red accent turns pink at the lightness a dark theme needs for
4.5:1, so the red is the second voice). The string and the 808 body on a scope. Chicken-head pointers, bat toggles, screws
and vents; condensed, widely tracked capitals.

**Lamina** (electric piano). Tines and reeds through a pickup. Inspired by 70s stage
pianos' lacquered cases and their name rails. Burgundy (340°), accent rose 325°, the tone
bars' brass 42°; material Lacquer. The tine, tone bar and pickup from the side, behind
glass. Skirted knobs, rockers; sentence-case medium, a badge.

**Organum** (tonewheel organ). Drawbars and a rotary speaker. Inspired by console organs'
walnut cheeks, ivory tabs and a magic eye. Walnut (28°), accent magic-eye green 95°, pilot
amber 35° (the sub drawbars' brown); material Wood. The drawbars and the rotor on a printed
chart. Numbered dials, ivory stop tabs that tip down and light
their pilot amber when on (its own switch painter), printed rules; engraved capitals.

**Chorda** (grand piano). A modelled concert grand. Inspired by a grand's black lacquer,
its gold cast frame and red hammer felt. Piano black (white lacquer in a light theme;
230°, almost neutral), accent champagne gold 40°, felt red 5°; material Lacquer. The piano
from above: the spruce **soundboard** with its grain, the frame's rim and portholes, the
strings, the **hammers'** felt row, the shown hammer raised. Black skirted knobs with a gold
line, pills; light sentence case, an engraved name.

**Pulsus** (drum machine). Synthesised and sampled pads. Inspired by classic drum
machines' lamp keys and printed panels. Cool grey (210°), accent 808 yellow 55°, orange
12°; material Carbon. The hit on a yellow vacuum-fluorescent display. Soft knobs, lamp
switches, screws and rules; condensed, widely tracked capitals in a legend box.

No two units share both accent hue (within 20°) and material — a test holds it, and
another holds the kit's contrast promises (text 4.5:1, the unit's hue 4.5:1 on decks and
displays, outlines 3:1) for every sheet in every one of the twelve theme presets.

## The displays

The characteristic displays already existed (Unda's table, Calor's filter and envelopes,
Modulus's graph, envelopes and spectrum, Exemplar's map and wave, Gravis's string, synth and
808 pictures, Lamina's pickup and tine, Organum's drawbars and rotor, Chorda's piano from
above, Pulsus's hit). This round:

- **In the display's look.** The kit lights a card's display ground in the face's look
  (phosphor, LCD, VFD, glass, paper, scope); the pictures inside were written against the
  kit's tokens, so a light label would have landed on a light LCD or a printed chart.
  `views/inst/face.rs::on_display(s)` hands every picture a style whose text, dim text,
  labels, grid, ink, well and hues *are* the display's ink (AA text, 3:1 voices, the kit's
  `display_ink`), and `face::cover` lays the glass's reflection or the CRT's edges over it.
  Wired at every display of the nine editors (Play and Edit), never on the cards' controls.
  Modulus's operator envelopes (drawn in the card's body, not its display) take the LCD too.
- **Two voices.** `face::voice2` (the look's second voice) and `face::accent_on` (the
  face's own colour, toned for the display's ground) give each picture its second colour:
  Unda's frame playing now in magenta; Lamina's tone bar in brass; Organum's sub drawbars
  brown (the pilot amber darkened); Exemplar's loop points in teal; Gravis's pluck marker in
  red; Pulsus's envelope in orange beside its yellow pitch.
- **Chorda's soundboard and hammers**: a spruce soundboard under the strings (the face's
  gold over the display's ground, its grain rising across the board, kept inside the case),
  the cast frame's rim beyond the bridge with its portholes, a row of felt hammer heads
  along the strike, the shown key's hammer raised in felt with a gold edge, the wound
  strings in copper between gold and felt.
- **Two switches of their own** (`SwitchShape::Custom`, in the sheet file): Organum's ivory
  **stop tab** (standing when off; tipped down, its back showing and its pilot amber lit,
  when on) and Modulus's **membrane key** (a flat key with a printed colour bar that lights).

On the kit's plain look (the user's switch off) every one of these is the picture as it
was: `on_display` returns the style unchanged and the voices fall back to the colours they
had (the device hue, the dynamics hue, FL's highlight, the primary text).

## Tests

- `kit::skin::instruments::tests` (lib): every instrument has a face; no two share accent
  hue (within 20°) and material; every accent, *as chosen* (before the kit's floor could
  move it), reads 4.5:1 on its tinted decks, card titles and displays in all twelve theme
  presets; every promise of the kit holds for every sheet in every preset, the card
  materials included (`skin::contrast_failures`).
- The kit's own tests over every unit hold for these sheets: `tests/skins_body.rs` (no
  body paints with the plain tokens), `tests/skins.rs`, the layout, menu and budget suites.
- Shots: `examples/shoot_skins.rs` (the kit's) and `examples/shoot_inst_faces.rs` (each
  instrument's Play *and* Edit view, maximised) in Slate and Paper at 100 % and 200 %; the
  best in `docs/round32/skins/instruments/` (each unit in Slate and Paper at 100 %, Chorda
  and Organum at 200 %, Modulus's and Chorda's Edit views; 22 PNGs, 4.2 MB). Looked at and
  redrawn after looking: light labels on light displays (the reason for `on_display`), a
  pink "red" accent (Gravis turned blue, its red the second voice), pads on an LCD (Pulsus
  moved to a VFD), Modulus's envelopes left on the plain ink, Exemplar's display style
  leaking into its loop fields, the soundboard too faint, the portholes too small.
- Changed by the faces and fixed: `tests/inst_chorda.rs` and `tests/inst_epiano.rs` read
  the nameplate in any case (Chorda and Lamina set their names in sentence case); Exemplar's
  *Drum rows* toggle is as wide as its word needs beside the face's switch (*Filas de
  batería* was cut short beside the pill).

## The review's fix round (f4d0538c..2ab5b36e)

Each with a test that fails before its fix:

1. **A display's fills and darkness follow its own light.** `face::on_display` now maps the
   fills a picture draws on (`deck`, `deck_title`, `panel`, `panel_raised`, `panel_light`,
   `value_pill`) to shades of the display's ground on its side of light, sets `t.dark` to
   the display's, and pushes the words to 4.5:1 on every fill (Gravis's mixed body
   included): Modulus's operator nodes and matrix on its LCD in Slate, Calor's voice cards
   and Unda's routing boxes in Paper, Gravis's body on its scope, Organum's ticks on cream.
   Test `face::tests::a_display_s_fills_and_words_follow_its_own_light_in_every_theme`
   (every instrument sheet × 12 presets). The sheet test checks each display against the
   ground the unit shows (`display_ink(s).ground`), not the plain ink.
2. **Off is as before.** Chorda's soundboard, rim, portholes and felt row draw only in a
   face with a display look; the shown hammer is the device hue on the plain look. Test
   `face::tests::the_plain_look_draws_the_piano_as_before_and_the_face_adds_its_wood_and_felt`.
3. **Titles one way.** Chorda's, Lamina's and Exemplar's capitalised title keys are in
   sentence case (EN and ES; the plain look capitalises through its lettering), and
   Organum's tail titles (`organ_parts::card`) take the sheet's lettering. Test
   `tests/inst_faces.rs::every_card_title_of_a_unit_is_set_one_way` (faces on and off).
4. **Organum's white drawbars show on the chart**: groove, shaft and handle outlined in the
   display's rule pushed to 3:1 on the paper and on the white bar. Test
   `drawbars::tests::a_white_drawbar_shows_on_the_chart_in_every_theme`.
5. **Cost.** The soundboard allocates nothing (an array rim, one segment per grain line
   instead of a polyline; the bridge and strike rows are arrays too); a display's colours are
   made once per unit and theme, on the unit's tone hue, and kept beside the mapped style
   (`face::ink`), so `voice2`, `accent_on` and `cover` never make them again. Tests
   `face::tests::a_unit_s_displays_make_their_colours_once` and
   `tests/inst_faces.rs::four_instrument_faces_in_one_frame_settle_their_caches` (Chorda
   Play, Modulus Edit, Organum, Unda in one frame: `skin::counts()` still over 32 frames).
   (Tests 1–4 fail on the code before their fixes by what they check — the old style kept
   the theme's `dark`, the plain hammer was the dynamics hue, the titles were capitals,
   the outline did not exist; 5's could not be run against the old `face.rs`, whose API
   the fixed pictures no longer match.)
6. **Chorda reads at a glance**: piano black (the ladder's blue cast taken out), a richer
   gold (43°, 75 %) on the rings and a gold **badge** for its name, a warmer soundboard.

## Numbers

- Windows (`RUSTFLAGS=-D warnings`): `cargo clippy --workspace --all-targets` clean;
  `cargo test -p cantus-ui` before the fix round 1 873 passed, 3 failed (the three above, then
  fixed and re-run green), 31 ignored; after merging the fix round (f4d0538c) the sheet tests
  (12 in `kit::skin`) and every `inst_*`, `skins`, `skins_body` test green.
- Linux (WSL, `tools/wsl/check.sh r32/instruments`, own clone `~/ci-r32-instruments`, at
  8651db59): clippy clean (and the plug-ins without their editor); `cargo test --workspace`
  4 799 passed, 59 ignored, 1 failed — `cantus-sandbox/tests/session.rs::
  a_hang_never_blocks_the_interfaces_frames_and_is_ended`, a timing test of a crate this
  round does not touch, run while three other builds compiled; re-run alone three times, 9/9
  each time. After the review's fix round (153a804e): clippy clean, `cargo test
  --workspace` 4 806 passed, 0 failed, 59 ignored. The clone's `target/` deleted after
  each run.

## Shared-file lines

- `views/inst/mod.rs`: one line, `pub(crate) mod face;` (the new `views/inst/face.rs`).
- Nothing in `kit/*.rs`, `views/fx/*`, `views/inst/frame.rs` or `kit/shell.rs`.

## Needs the owner

Nothing.

