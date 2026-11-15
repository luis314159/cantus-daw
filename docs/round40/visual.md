# Round 40 phase 3 — builder `visual`: the instrument pictures up to the reference

ADR 0126 (the scenes), ADR 0104 (the per-pixel renderer). Branch `r40/visual`.

The owner (2026-11-13): *some pianos don't look the same quality as the first one; some
scenes still need work to match the first photographic one; and the lid sometimes hits the
music desk.* The reference is the concert grand in black lacquer on the stage, Night,
Photographic. **Drawn** (vectors) stays the default; **Photographic** is the premium mode.

## What this branch did first: scene-px merged and registered

`r40/scene-px` merged into `b11315ac` (clean); integration's `KitDef.finish` named in
scene-px's tests and example (e4052fbe). The photographic renderer is registered in
`CantusApp::assemble` (scene-px's `hybrid::register()` beside `backend::set_default`);
`tests/scene_px_e2e.rs` goes through `assemble` for Chorda **and** Tympanum: *Photographic*
→ each picture is the per-pixel renderer's to the bit, *Drawn* → the vectors, and back.

## The audit

`compartir/p3/visual/cp1/AUDIT.txt` — geometry (A1–A4), pianos and finishes (B1–B5), rooms
(C1–C4), Tympanum (D1–D5). What each became is below.

## The pianos' shared geometry — `scene/piano_geo.rs`

Both pictures build from it, so what must clear clears in both:

- **A grand's lid in two leaves.** The main lid runs from the front flap's hinge
  (`LID_FRONT`, z = −34 cm) to the tail; the front flap is folded back over it whenever the
  lid is open or the desk is up — as a real grand is opened. The music rest stands in the
  open strip in front (its back reaches z = −28), so no lid angle can meet it. `lid_point`
  is the lid's top face: closed, its underside lies on the rim (it sank 0.9 cm into it).
- **The prop stick** stands in its cup on the bentside's rim (`rim_x` at `prop_z`) and
  holds the lid's underside (its foot floated outside the case).
- **The upright** is a modern studio upright: side panels stepped as a real one's (the arm
  over a tapered front leg on a toe block and caster, a corbel under it), key blocks, the
  keys and slip, a fallboard in a gentle curve with its nameplate, a music shelf with a
  sheet of music against the upper panel's raised field, a raised field on the bottom board,
  the pedals and plinth, the bench in its finish; the top in two leaves on a piano hinge —
  the front leaf folds back flat (from half the lid on; a standing leaf is only seen on its
  way).

`tests/scene_lid_desk.rs` holds it: every grand body at every half degree from closed to
the full stick — the desk's every corner against both leaves with a 3 cm margin, the closed
lid on the rim, the stick's foot on the rim's band and its head on the lid's underside,
20 cm or more from the desk; the upright's leaf never below its top nor near its front.

## Finishes (both pictures)

- **Drawn's light finishes** carry their finish over the whole case (white, maple and
  rosewood rims read black or grey): a photographer's fill on lacquer
  (`(0.30 − ambient) × 1.8`, hemispherical — black lacquer, all coat, is untouched), and the
  grazing mirror's share capped by the albedo (a white rim seen along its length stays white
  with its mirror over it). `scene/light.rs`.
- **Drawn's black lid** mirrors the harp at the photograph's strength (the lid's mirror at
  0.62, the gold and the strings stronger).
- **The music rest in Photographic** too (the drawn one's: the board in the case's finish,
  fretwork either side over a dark recess, the ledge and lip, a printed sheet of music).
- **Photographic's veneer** quieter (the figure at full contrast read as plywood).
- The drawn bench without its tufts (they read as holes).

## Rooms and Paper

- **Daylight's haze** warm, not grey, and a thin veil in Drawn (a grey wash over every
  Paper room); **the stage by day** lit as with the house lights up — warm and low ambient,
  its fly tower dark (Photographic's Paper stage was a grey studio backdrop); **daylit rooms**
  in Photographic get a stronger soft fill and exposure (their walls were dull grey).
- **Drawn rugs**: a pile with its edge where it faces us, a fringe at both ends, a
  medallion, the field's sheen falling off toward the border.
- **The microphones**: the stereo pair hangs on a short black clip, a black bar across and
  the pencils from its ends along the aim (a chrome drop rod and splayed pencils drew a
  zig-zag "lightning bolt"); pencils satin, not glowing; stand chrome a touch brushed in
  Photographic (a sharp mirror on a thin mast beaded with the rug's pattern).

## Tympanum

- The kick's resonant head: a printed band (two rings), its wooden hoops in the shells'
  finish with an inlay line (both pictures) — it read as a black hole.
- Photographic bloom only from real lights (threshold 3.2, each sample's part capped): no
  white haloes round the chrome hoops.

## Checkpoint 3 — stands, cymbals, drums, the bar, the upright's edges, Paper walls

**The striped stands, found.** Three causes, each proved on a 3× crop
(`compartir/p3/visual/cp3/crop-stand-*.png`, before | after):

1. *Coverage and the round under two samples a pixel.* A mast 2–6 px wide holds its whole
   half-round in four to twelve samples; a slanted one crosses the sample grid in steps, so
   which normals (and how many samples) it gets repeats down its length — the twist. Rods
   narrower than 8 px on screen now leave the samples and are drawn as **tube strokes**
   (`raster::tubes`): anti-aliased against the pixels' depth, every pixel shaded at its
   place across the tube, its footprint's spread given to the mirror; the triangles stay
   for the key's shadow and the reflections. The pixel takes the tube's depth where it
   covers most of it (depth of field and bloom see the tube where it stands).
2. *The key's penumbra grain on the key's mirror.* `Shadow::light` turns its filter per
   sample; a rod in a pool's edge got 0 or 1 per pixel and its mirror of the key's very
   bright face beaded with it. Each chrome rod's key visibility is now **one profile along
   it** (`rod_key`, looked up toward the key every 1.5 cm and smoothed), for the strokes
   and for the rods still in the samples.
3. *The softboxes as ray hits.* A filtered mirror's eighteen rays caught a small bright
   face or missed it from pixel to pixel (salt). In a filtered mirror the room is still
   sampled by rays but the **softboxes are integrated in closed form** (`panels_filtered`:
   each face a Gaussian of its solid angle against the filter's own) — smooth however few
   pixels the tube has. Rough chrome's reflections are half desaturated (a red rug made
   every mast a red-and-white pole) and a tube's edges darken — one long highlight and a
   dark edge.

Also: a metal's mirror is spread over the normal's turn under the sample (`normal_turn`,
the reflected ray's change across one sample), so the rods still in the samples (wider
than 8 px) and the hoops filter their mirror too; the metal's key look-up moved toward
the key (off along the normal it fell in the rod's own shadow).

**Cymbals** (both pictures from `scene::kit::prof`, now shared): a bell a quarter of the
radius standing 2.1 cm off a 1.6 cm bow at a clear shoulder; each cymbal's lean toward
the player eased to half the definition's (`CYMBAL_TILT`: at 14–20° every cymbal stood
edge on, its underside to the camera); the felt and wing nut on the bell's top and the
hats' clutch over the top bell. Drawn: an edge band under the rim, the lip lit where it
faces us, the bell's shoulder line and lit ring, a glow on the dome toward the key,
lathe rings twice as strong. Photographic: a rounded 3 mm lip, the lathe's grooves drawn
an octave coarser per pixel size (they were flattened away at every size the picture is
seen at), the room's light in the bronze capped and the key's streak bounded across the
grain (a cymbal facing the key half-way to the eye flashed white), the grazing Fresnel
kept bronze, the hammering's dents broader and shallower (they were a grain).

The sizes stay the kit definition's (crash 16″ and 18″, ride 20″, hats 14″, splash 10″,
china 18″: `cantus-core` `tympanum_studio.rs`, which the voicing reads and the size label
shows) — the picture draws what the definition says.

**Drums.** Drawn heads: the collar band where the head turns over the bearing edge, its
lit crest, the hoop's shadow a crescent on the side toward the key, the coating's broad
clouds; every lug catches the key in a fine line (`glint`); the rack toms' brackets on
the shell's front-inner quarter in both pictures, so the arms show between the toms
(on the back the toms seemed to float).

**The Drawn bar** on black lacquer at 0.4 of the coat (a light finish keeps 0.15), its
colour a neutral white over black (the warm tint read gold).

**The Photographic upright**: the side panels, legs, toe blocks and key blocks are
extruded with every edge rounded (`rounded_prism_x`, 0.6–0.9 cm), the fallboard's top a
bullnose.

**Paper walls**: `pal::daylit` warms and lifts the walls by day in both pictures, and the
photographic walls take 1.45× more of the day's light (`DAY_WALL`): the church's wall
108 → 150-odd in sRGB, the booth's 96 → 115, no grey cast.

## Checkpoint 4 — Drawn up to Photographic

The coordinator's cp3 review: the sizes stay the kit definition's (a real 16/18/20 studio
setup, which the voicing reads). `compartir/p3/visual/cp4/` (crops cp3 | cp4):

- **Drawn black lid** read as a tinted glass: its mirror of the frame at 0.42 (was 0.62),
  dimming toward the tail where it looks into the dark, the holes and strings fainter, and
  the board's dark thickness drawn round its outline — a black mirror with the frame on it.
- **Drawn cymbals**: a photographer's fill keeps the bronze's colour on a dark stage, the
  key's streak bounded across the lathe's grain (it lit the whole top white), the lip in
  the bronze's own colour (a white rim read as a silver plate).
- **Drawn chrome** (every stand, boom, lug and hoop): the room's colours half washed out, a
  silver fill from the photographer's cards, the key's one long highlight, the edges dark
  where the tube turns away — the Photographic treatment; on the dark stage the booms were
  black lines.
- **Drawn heads**: off-white (`ink::COATING`), a stronger mottle and clouds, the collar band
  twice as wide and darker.
- **The second bar** on Drawn black lacquer: a tall light straight behind the instrument,
  seen only in black lacquer's faces (in the shared streak it washed the stage floor grey),
  lands where the bentside turns toward its tail.
- **Busy rooms**: the Kit view shows no mics by default (`show_mics: false`); with them
  shown, the overhead and room stands' masts and booms are a size lighter in both pictures
  (0.75 / 0.55 cm, were 0.9–1.0 / 0.7).

## Checkpoint 5 — the lid's band, daylit heads, the Drawn stage a pool

`compartir/p3/visual/cp5/` (crops cp4 | cp5 | Photographic):

- **Drawn black lid**: the frame's reflection only in a band from the edge it lies along,
  fading out by a third of the board (`lid_mirror`'s mask), the holes and strings fading
  with it, the rest of the board black; the rim's bevel the lid's brightest line.
- **Drawn heads by day**: capped below paper white (a warm off-white a little darker than
  the walls), so the collar and the mottle show.
- **Drawn's cost, re-timed against e4052fbe** (e4052fbe built with today's example, the two
  binaries interleaved twice per case, best of five each): **1.06× over all 33 cases**,
  median 1.03×, 0.80–1.55× per case (the highest the lid cases, the noisiest under the
  other builders' load) — `cp5/times.txt`, `cp5/times-drawn-vs-e4052fbe.txt`.
- **The Drawn stage, my pick of the weakest** (every stage tile): its floor was lit edge to
  edge (the photographer's fill meant for the instrument lit the floor too) and its
  curtain stood in bright bars. The floor now takes the room's light only (`no_fill`) and by
  evening falls off past the key's pool to a fraction of it; the curtain is dark velvet
  with soft crests, the uplights' warmth lower.

## Checkpoint 6 — the music desk, light finishes, the frame's gilt

`compartir/p3/visual/cp6/` (crops cp5 | cp6 | Photographic). The owner on cp5: *¡Van
increíbles los pianos, excelente!!!* — the same approach.

- **Drawn music desk**: the board in the case's finish shaded down its height (black
  lacquer a deep board), the fretwork cut through to the dark behind it — a recess, the
  slats left standing with a fine light edge and a rail across — the moulding a fine line,
  the sheet a warm page (`ink::PAPER_WARM`). It was a mid-grey board with white bars.
- **White and maple grands**: lit only by the ambient their rims read grey and chrome-like.
  A light finish now takes the photographer's fill from the camera's side (bright where a
  face looks at us, turning softly darker as it turns away), its mirror only adds light
  (never darkens it toward the dark room), and its bands are soft.
- **My pick — the frame in Drawn grands**: one saturated orange-yellow sheet beside the
  photograph's gilt. It is a little less saturated, bright near the keys where the key's
  pool falls and falling off toward the tail, with a dark gap round it at the inner rim; the
  treble strings show a little more.
- **Cost**: Drawn against e4052fbe interleaved: 1.08× over 33 cases (median 1.03×); the
  highest re-timed alone 0.83–1.52× — `cp6/times-drawn-vs-e4052fbe.txt`. `scene_vector`'s
  lid-drag budget (2 s, debug) misses only while other builds load the machine (alone:
  845 ms at 1×, 1078 ms at 1.5×).

## Checkpoint 7 — strings, the white bend, the gilt, the keyboard

`compartir/p3/visual/cp7/` (crops cp6 | cp7 | Photographic).

- **Drawn strings**: 110 over the frame (were 78 faint lines), each with its shadow on the
  gilt and lit by the key's pool — the wound bass in copper crossing over the steel treble —
  fine lines over the gilt, never a sheet of steel over it; the dampers' row 60 felted
  heads, each with its lit top edge.
- **The white grand's bend**: a light case finish shows no strip-light bars (the bar at the
  bend read as a chrome streak), only the key's soft sheen, and takes a stronger cream
  fill from the camera's side. `light::light_finish` says which finishes: bright, and a wood
  or near neutral — the gilt frame and coloured trims keep the ordinary lacquer (with the
  bright rule alone the frame went a greenish pale yellow).
- **The frame's gilt** a third of the way toward its grey and a little darker: nearer the
  photograph's.
- **My pick — the Drawn keyboard**: its white keys read as a grey hatch (the keybed showed
  through every gap) and its black keys as stubs, the key slip a silver band. Now an ivory
  run with fine soft gaps, black keys two thirds the white's length with a lit front edge,
  the slip in its finish with one lit line along its top.
- **Cost**: Drawn against e4052fbe interleaved: 1.05× over 33 cases (median 1.04×, max
  1.34×) — `cp7/times-drawn-vs-e4052fbe.txt`; the nine tests pass with the lid-drag budget.

## Checkpoint 8 — the bass strings, Tympanum's three weakest in Drawn

`compartir/p3/visual/cp8/` (crops cp7 | cp8 | Photographic).

- **Bass strings**: thin golden-copper lines with the frame between them — every other
  wound string drawn, 1.0–1.2 wide, the copper a little desaturated and darker, the key's
  glint along the wider ones (drawn 1.7 wide and saturated they read as an orange band,
  the coordinator's review; `cp8/crop-strings-drawn-fix.png`); the treble fine bright steel.
- **Tympanum, Drawn beside Photographic** on the stage and in the daylit booth — the three
  weakest, fixed:
  1. *The shells* read as painted stripes, and yellow: the figure at a piano case's
     strength, and the light finish's fill clipped channel by channel (red first, then
     green). The figure is 0.4 of it on a drum, the grain lines fainter, a light finish's
     fill the softbox's white, held under white with its hue — the photograph's satin peach.
  2. *The kick's front head*: its port and printed ring where the photograph puts them
     (the two pictures had them in different places; one ring at 0.71, not two).
  3. *The hoops* were dark bands on the stage: a chrome hoop's rolled top is now its
     brightest line.
- **Cost**: Drawn against e4052fbe interleaved: 1.05× over 33 cases (median 1.04×, max
  1.42×) — `cp8/times-drawn-vs-e4052fbe.txt`; the nine tests pass, clippy clean.

## Cost

`scene_hybrid` times Drawn (best of five) beside each bake: over all 33 cases Drawn takes
0.94× of e4052fbe's (the machine busy with other builders; per case 0.5–2.2×, the noise of a
busy machine). The photographic bake's peak is unchanged (63–66 MB at 1.5×).

Checkpoint 3 (`compartir/p3/visual/cp3/times*.txt`): the machine ran four other builders'
builds and tests throughout, so Drawn was timed with cp2's binary and cp3's interleaved,
case by case (`times-drawn-interleaved.txt`: 0.57–1.44× per case, 0.76× over eight cases —
the noise; the work added is a few hundred lines per kit: the heads' crescents, the lug
glints, the cymbals' edges and bell). The photographic kit's peak 82–84 → 86–87 MB (the
rods' key profiles and their triangles' owner, 4 bytes a triangle).

## How it is tested

`tests/scene_lid_desk.rs` (new, above); `scene_px_e2e` (Chorda and Tympanum through
`assemble`); `scene_vector`, `scene_px`, `scene_backend`, `chorda_scene`,
`room_card_layout`, `inst_faces`, `tympanum_views_e2e` pass; clippy `-D warnings` on
`cantus-ui --all-targets` clean.

## Verified only in pictures

Every picture here is the CPU's (the examples' PNGs), compared by eye with the reference
beside it; the owner's eye is the judge. Checkpoints: `compartir/p3/visual/cp1/` (the
audit, the lid and the upright), `cp2/` (finishes, Paper, rugs, the kick), `cp3/`.

## What is left

- Chrome made with `g.tube`/`g.lathe` (lugs, hoops, the kick's claws) is not a rod: no
  tube stroke, no smoothed key — fine at their size; watch close crops.
- Photographic cymbals' lathe rings are faint at 1.5×; a fine speckle on the floor toms'
  shells (the shadow's or the half-res reflections' noise).
- The upright's open top shows only the frame's bar and pins (no hammers) — what a look
  over a folded leaf sees from the scene's camera.
