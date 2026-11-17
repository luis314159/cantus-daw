# Round 40 — `tymp-polish`: Tympanum's Kit view polished (the owner's "púlelo de una")

**Status: delivered on `r40/integration`.** Three faults the coordinator pointed out in the Kit view,
each fixed, looked at in CPU shots (English and Spanish, Slate Night and Slate Paper, 100 % and
150 %), and held by tests. No schema change, no DSP change (`DSP_VERSION` untouched). The kit is
drawn by the **vector** scene (`scene::backend::DEFAULT_PICTURES = Vector`): that is the picture
fixed here. The photographic renderer (`views/inst/chorda_scene_px_kit.rs`, *Settings ▸ Appearance ▸
Instrument pictures ▸ Photographic*) builds its own stands and is unchanged.

## 1. The room card no longer covers the kit

| Before | After |
|---|---|
| The *Room* card (Stage/Booth/Studio/Hall, Floor, Walls, Rug) sat over the picture's lower right, over the floor tom and the kick | A **room chip** left of *Show mics* — the room's own hand-drawn glyph (a spotlight's cone for the stage, a box for the booth, a room seen into for the studio, an arch for the hall), *Room* and the room's name as a label/value unit, a chevron — opens the same card as a **popover** under it, on a soft sheet shadow |

- Closes on a click outside it (which plays nothing), on **Esc** (through `crate::esc`, ADR 0110:
  the keymap's *Clear selection* leaves the Esc to it; a Floor/Walls/Rug list open takes the Esc
  first), or on the chip again. Clicks on the card never fall through to a drum.
- Open/closed is **view state only** (`tympanum::Look::room_open`, per instrument in egui memory, like
  *Show mics*) — not the song's, no schema bump. Default: closed, the kit whole.
- The shared card gained `room_card::show_in` (a caller-given rect) and records its rooms row
  (`Shown::rooms`, probe `Seen::rooms`); Chorda's Play view still calls `room_card::show` unchanged.
- The kit's tag (bottom left) is 50 pt tall now (was 54) and always shown; the tests prove it covers
  no piece.
- Words: `tympanum.room.chip_tip` (the help line) in English and Spanish.

## 2. The stands as real hardware

New `scene/stand.rs` (only the kit's stands; a microphone's tripod keeps `kit::tripod`, so Chorda's
scene is untouched):

- **Double-braced tripods**: each leg a pair of tubes ending in a wide rubber foot with its contact
  shadow; each brace a pair, riveted to the leg; hinge pins; top and lower collars with wing bolts.
- **Height tubes in sections** (25.4 / 22 / 19 mm on the boom stands), each joint a **clamp** with
  its chrome lip, **memory lock** and **wing bolt**.
- **Ratchet tilters** at the boom's root and at the cymbal; the boom through its clamp with its
  **counterweight**; under each cymbal a **cup washer and felt** (over it the felt and wing nut as
  before).
- **Hi-hat**: 29 mm outer tube, inner tube with its clamp, the bottom cymbal's **seat** (cup, felt,
  tilt screw), the **clutch** (felt, body, lock nut, wing screw), the pedal's frame and two **spring
  housings**.
- **Snare stand**: two sections with a clamp, the basket's ratchet tilter, hub and adjusting knob, arms
  in rubber sleeves. **Floor-tom legs**: a black bracket block with its wing bolt, 16 mm legs, rubber
  feet. **Tom holder**: wing bolts on the post clamp and each bracket.
- **Shaded as chrome cylinders**: `TubeOpts::bands` (only the stands' tubes) gives each tube a bright
  core, the dark room mirrored either side, a lighter rim and dark silhouette edges, and the key's
  streak (`kit::glint`) along it. Drawn `stand::THICK` = 1.45× their real radius (a 25 mm tube was
  four pixels across at 150 % — too thin to read as round beside the drums).

**Untangled legs** (the coordinator's review of the first delivery: on the right the hi-hat's, the
snare's and the crash's tripods piled into a cluster of crossing lines; the studio kit's crash stand
stood 5 cm from the hi-hat's centre). `stand::untangle` places every stand's legs in turn — the hi-hat
first (its pedal kept clear as a fixed obstacle, with the kick's spurs, pedal and footprint), then the
snare, the floor toms and the boom stands — choosing for each the turn (±60° in 6° steps), the reach
(100 … 55 %) and, for a boom stand only, a shift of its centre up to 22 cm (its boom still reaches
the cymbal) that scores best: a leg through another stand's on the floor costs 60, a foot within
4 cm of another's leg or 10 cm of another's foot 8–60, a crossing in the picture (another's leg or
upright) 3, against small costs for turning, drawing in and moving. Planned with the picture's own
camera (never the mirror's: the reflection matches). `kit::tests::no_stand_s_legs_run_through_another_s`:
both modelled kits, crossings on the floor 1 → **0**, crossings in the picture 14 → **2**.
Before/after: `cmp-untangle-night-150.png`.

**Cost** (`scene_vector::the_kit_builds_within_its_budget`, new; test profile, best of 5, the kit
on the stage at 756 × 452 pt):

| | build (display list) | ops | picture 1× | picture 1.5× |
|---|---|---|---|---|
| before (night) | 20.8 ms | 55 259 | 385 ms | 456 ms |
| after (night) | 22.8 ms (+10 %) | 56 181 (+922) | 412 ms (+7 %) | 490 ms (+7 %) |
| before (day) | 20.0 ms | 55 252 | 384 ms | 450 ms |
| after (day) | 23.0 ms | 56 174 | 395 ms | 465 ms |
| + untangling (night) | 39.8 ms | 56 181 | 510 ms | 597 ms |
| + untangling (day) | 33.6 ms | 56 174 | 441 ms | 520 ms |

The last two rows were measured while other test binaries ran beside it (noisier); the planner's
search (≈ 2.5 M segment tests, run for the picture and again for its mirror) adds about 10–15 ms to a
build in the test profile — off the UI thread, once per change of what is shown, well inside the
2 s budget the tests hold. It could be halved by planning once for both passes (not done).

The scene is built on its worker thread, never the UI's; at rest nothing is built
(`at_rest_no_repaint_no_build_and_no_allocation` still passes). There are no golden hashes for the
vector kit (the bake hashes in `chorda_scene_budget.rs` are Chorda's old raster, untouched).

## 3. The picture in light themes

Tried and kept, looked at side by side with the ship shots:

- **The picture is a framed window**: its corners rounded (the scene's texture drawn as a rounded,
  textured `RectShape`: `Scene::set_corner`, cached, no allocation at rest), a hairline edge in the
  kit's `edge` token, and in a light theme a soft sheet shadow under it (`kit::dialog::sheet_shadow`)
  and a light inner line — it reads as a deliberate inset in the page, not a hole cut in it.
- **The house lights up by day**: on the stage in a light theme the velvet is lifted to a warm umber
  (×2.6) and `MOOD_STAGE_DAYLIGHT` is a step brighter (ambient 0.22 → 0.38) with the corners'
  falloff halved (0.10 → 0.05). Night is untouched (`MOOD_STAGE_EVENING`, the evening curtain).
- Not chosen: a fully light "daylight studio" stage — the Walls field would still say *Stage black*
  while showing something else, and Booth/Studio/Hall are already the daylit rooms one click away.

Legibility: `tympanum_stage::tests::the_overlays_on_the_picture_read_in_every_theme` measures, in all
24 themes, the chip's words at rest, hovered and open (≥ 4.5:1), its glyph and chevron (≥ 3:1) and
the kit tag's words over its plate on the darkest and the lightest picture (≥ 4.5:1). It caught two:
open, *Room* in ink 2 on the accent's tint was 3.94:1 in Slate Night — it is ink 1 when open now;
and the shared lit button's words (*Show mics* lit, the chip open — `iu::paint_button`) were 4.37:1 in
Ultraviolet — `iu::lit_ink` now falls back to ink 1 wherever the accent's mix would read under 4.5:1
(a change only in the themes where it failed; every lit button of the instrument views gains it).
`scene_vector::the_selection_reads_against_the_scene_in_every_theme` still passes.

## Tests

- `tympanum_views_e2e::the_kit_shows_whole_with_the_room_card_closed` — through
  `CantusApp::assemble`, Studio Maple, Jazz Birch and a recorded kit, on 1280 × 720, 1600 × 900 and
  1920 × 1080 screens: no point of any piece (drum, cymbal or its stand — a 160 × 96 pick grid) lies
  under *Show mics*, the chip or the kit's tag; the card is closed by default.
- `tympanum_views_e2e::the_room_chip_opens_the_card_and_the_room_changes_through_it` — the chip opens
  the card under it, inside the picture; *Studio* clicked in it sets `look.room = studio` and acoustic
  room 2 (one undo step takes both back); Esc closes it; a click outside closes it and plays nothing;
  the chip opens and closes it.
- `room_card_layout` — Tympanum's card measured as the popover.
- `scene_vector::the_kit_builds_within_its_budget` — the numbers above.
- `picker::a_scene_s_thumbnail_waits_for_its_picture` passes; the picker's Tympanum thumbnail shows
  the kit (`after/picker/`).

## Shots (`F:\projects\daw-r40\compartir\p3\polish\`)

- `before/` — the ship shots; `after/` — the same cases now (kit and mixer, night and paper, 100 and
  150 %, `-es`), `after/room-open/` (the popover open), `after/jazz/`, `after/recorded/`,
  `after/picker/`.
- Side by side: `cmp-kit-night-150.png`, `cmp-kit-paper-150.png`, `cmp-kit-paper-100-es.png`,
  `cmp-stands-night-150.png`, `cmp-untangle-night-150.png` (first delivery → untangled). Iterations: `it1/`…`it4/`, crops in `crops/`.
- The shoot gained `TYMP_SHOT_ROOM=<room>`, `TYMP_SHOT_ROOM_OPEN=1` and
  `TYMP_SHOT_THEMES=preset:dark,…`.

## Suite

`cargo test -p cantus-ui` (Windows, `-j 2`): **2 762 passed, 0 failed, 56 ignored**. Clippy and the
WSL Linux check: see the report.

Housekeeping: the first full run failed to link (`LNK1180`, F: full at 44 MB free); 828 superseded test
executables and PDBs (an older hash of a test name, each with a build at least an hour newer) were
deleted from the shared `memory/target/debug/deps` — 16.2 GiB — and the suite rerun.

## Not done / for later

- The photographic renderer's stands (`chorda_scene_px_kit.rs`) are still the simpler ones.
- Chorda's room card still sits over its picture (it was not part of this ask; `room_card::show_in`
  makes the same chip possible there).
- Verified only in CPU shots and tests, not on the owner's screen.

---

# Chorda: the same polish (2026-11-17, the owner: "its room card still covers the piano")

**What changed** (`views/inst/chorda_scene_view.rs`, `room_card.rs`, `scene/mod.rs`):

- **The room chip.** The room card no longer sits over the picture's lower right. A chip beside
  *Show mics* — Chorda's own menu field holding the room's hand-drawn glyph (now
  `room_card::room_glyph`, shared with Tympanum), *Room* and the room's name, the field's chevron —
  opens the card (`room_card::show_in`) as a popover under it on a soft sheet shadow; open, the chip
  wears the focus ring. Where the pickers' row leaves no room beside *Show mics* (a small editor
  window), the chip stands under it. It closes on Esc (`crate::esc`), on a click outside it (a
  click on the lid still opens or closes the lid as ever; elsewhere it changes nothing) and on the
  chip. A Floor/Walls/Rug list open takes Esc and the outside click first. When the picture is too
  small for the open card, the card is left out (as before).
- **Still the picture only.** Chorda's room is the look (`look.room`), as it was: what is heard is
  the Room in the mixer. A room chosen in the card is one undo step and leaves `room`/`roomsize`
  untouched (the e2e test checks it).
- **View state only**: `Words::room_open` (per track, in the editor's state), no schema change.
- **Framed window**, as Tympanum's: the picture's corners rounded (`Scene::set_corner`), the
  hairline frame (`tympanum::stage::frame`, now `pub(crate)`), a soft shadow under it in light
  themes; the stage's daylight lift is the scene's, shared since the Tympanum polish.
- **At rest it allocates nothing more**: the chip's words are measured once per words and scale
  (`Words::chip_w`), what the tests read is written to egui's memory only when it changes, the
  overlays kept in a fixed array — `chorda_scene_budget::the_scene_card_at_rest_allocates_next_to_nothing`
  caught a first version at 33 allocations a frame against an empty panel's 25; now 26 (budget +6).
- Words: `scene.room.chip_tip` (the help line) in English and Spanish; the rest reuse `scene.room`
  and the card's existing keys.

**Tests**

- `chorda_room_e2e::the_piano_shows_whole_with_the_room_card_closed` — through
  `CantusApp::assemble`; Grand and Upright, the model and a recorded source (`pack:…`), on 1280 ×
  720, 1600 × 900 and 1920 × 1080; mics hidden and shown: no point of the piano (inside the
  outlines of its body, lid, keys, bench and, when shown, its mics — new
  `scene::instrument_points`) lies under the pickers, *Show mics*, the chip or the tuning's plate;
  the card is closed by default.
- `chorda_room_e2e::the_room_chip_opens_the_card_and_the_room_changes_through_it` — the chip opens
  the card under it, inside the picture; *Studio* (third of five) sets `look.room = studio`, the
  sound's `room`/`roomsize` unchanged; Esc closes it; a click on the empty floor closes it and
  changes nothing; a click on the lid (clear of the card) closes it and moves the lid; the chip
  opens and closes it; undo takes the room back. (In a small editor window the open card covers
  the whole lid, so this test maximises the editor.)
- `chorda_scene_view::tests::the_room_chip_reads_in_every_theme` — in all 24 themes, at the
  picture's style: *Room* in ink 2 on the field's well at rest, ink 1 hovered and open, the room's
  name in ink 1 — 4.5:1; the glyph 3:1.
- `room_card_layout` opens both cards from their chips for its sweep (English and Spanish, 100 %
  and 150 %, two screens): each card inside its picture, its fields inside it.
  `picker::a_scene_s_thumbnail_waits_for_its_picture` passes.

**Shots** (`F:\projects\daw-r40\compartir\p3\polish-chorda\`): `before\{grand,upright}\` and
`after\{grand,upright,grand-open,upright-open}\` — Night and Paper, 1× and 1.5×, English and
Spanish (`chorda-play-*`).
