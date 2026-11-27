# arcus-upper — handoff (what holds the instrument, the live hands)

Branch `r42/arcus-upper` (worktree `F:\projects\daw-r42\arcus-upper`). Build only through
`tools/build/slot.sh`, `CARGO_PROFILE_DEV_DEBUG=0`, on the F: target. Never push.

## Done

- **Pose B** (owner-approved composition): violin/viola held at the playing angle with the
  bow, amber varnish, no moiré (299eee0bb; `docs/round42/arcus-upper/sheet-v3/`).
- **Holders** (owner's direction 2026-11-24), a machine preference `Settings::arcus_holder`
  (`scene::bowed_hold::{Hold, Holder}`): *Air* (wall shadow; cello/bass as before), *Stand*
  (violin stand / cello floor stand; no bow, no finger), *Player* (stylised musician, eyes
  closed), *Mannequin* (wooden). `Subject::Bowed { instrument, holder }`; meshes cached per
  (instrument, holder) in `bowed_mesh::mesh_for`; the camera/fit per holder.
- **The figure's hands follow the playing**: both arms live (`views/inst/arcus_draw_arm.rs`,
  `arcus_draw_hand.rs`), shaded as the body (`arcus_draw_lit.rs`); hand positions keep / shift
  with a glide (`hand::target`, `hand::glide`, state `app.inst.arcus_hands`).
- **How the stop is shown**: `Settings::arcus_stop` (`StopShow::{Finger, Marker, None}`), the
  old `arcus_finger` migrated in `Settings::migrated` (called by `Settings::load`). Marker:
  the theme's focus colour on `scene::halo_for`.
- Tests: `tests/arcus_hold.rs` (migration through the file, positions/glide, the fingertip on
  the stop within 1 px for 4 instruments × 2 figures, elbow lengths, marker 3:1 in every
  preset); `tests/arcus_scene.rs` at rest × every holder × cello/bass/violin.
- Shots: `shoot_arcus --hold=all|air,stand,player,mannequin --stop=marker --only=a+b`; sheet in
  `docs/round42/arcus-upper/sheet-v4-holders/`.

## Now (owner, 2026-11-24, before the queue below)

1. **The left hand passes behind the neck.** Design: split the live left hand into the part
   behind the neck (sleeve, palm, thumb, finger bases knuckle→knee) and the part in front
   (finger arch and tips knee→tip). Paint order in `arcus_draw::picture`:
   `with_brush(live_behind)` → re-stamp the neck from the baked texture (a textured
   `egui::Mesh` over the neck + fingerboard silhouette between the body's edge and the nut,
   UV = (p − shown.min) / shown.size; needs `Scene::texture_id()` in `scene/live.rs`) →
   `with_brush(live)` (string, finger tips, marker, bow, bow arm). Violin/viola: palm and
   thumb behind/under the neck, fingers curve over and down onto the strings. Cello/bass:
   thumb behind the neck (behind the second finger), fingers wrap from the neck's side.
   Test: pixels on the neck's centre line at the hand, between fingertips, are the neck's or
   fingerboard's (not the figure's skin/wood), both looks, musician and mannequin, violin
   and cello; the fingertip still lands within 1 px.
2. **Natural arms** (cellist read odd at 200 %). Left arm: upper arm hangs down and out
   30–45° from the body, elbow out to the side at about chest height, forearm rising to the
   neck, wrist in line with the forearm, no detached cuff, hand about the face's length.
   Bow arm: upper arm hangs, elbow a little below the bow's line on low strings, level on
   the top string; forearm forward to the frog; knuckles on top, thumb bent under. Same
   thickness both arms; shoulders slope, the left not raised. Method: look at free
   photographs (Wikimedia Commons: "cellist playing", "violinist playing"), note shoulder/
   elbow/wrist angles for first and a higher position here, constrain the pole (elbow out and
   down, never up or forward) and clamp the wrist; compare silhouettes at the same camera.
   Apply to the violin musician and both mannequins.
3. Shots to send: cello and violin, musician and mannequin, 100 % and 200 %.

## Done since (a27ccff1d)

- The left hand behind the neck: `live_behind` (sleeve, palm, thumb, finger bases) →
  the body's two outline fans and the neck's hull stamped back from the picture's texture
  (`hand::stamp_body`, `hand::stamp`; meshes kept in `app.inst.arcus_stamps`, so nothing is
  allocated at rest) → `live` (finger arches, string, bow, bow arm). Test
  `the_left_hand_passes_behind_the_neck` (both looks, both figures, violin and cello, at 2×).
- Elbows by a desired direction from the shoulder (the pole): cellist left arm out to the
  side at chest height (down 30, out 12, forward 4 cm), bow arm hanging down and out (down 22,
  out 14, forward 6); violinist left elbow under the violin (down 24, in 10, forward 12), bow
  arm out to the right (down 12, out 18, forward 8). Wrist kept straight (the wrist on the
  elbow–palm line). A cello's hand spans a minor third (3 semitones), a violin's a fourth.
- Reference numbers (Hopper et al. 2017, Med. Probl. Perform. Art., 31 cellists, bowing
  arm at the tip on the A string, loud): shoulder flexion ~107°, internal rotation ~59°,
  abduction ~77°; trunk left rotation 5–16°. Turner-Stokes & Reid 1999: shoulder elevation
  greater on the cello, elbow flexion greater on the violin. Left-arm angles: pedagogy
  (elbow out at about chest height on the cello; under the violin for the violin), not yet
  measured from photographs side by side.

## The hands rebuilt from the owner's photographs (2026-11-24, second pass)

The owner (a guitarist) on sheet v4: "La mano está mal en el violín, así no se toma un
violín." His five photographs are in `docs/round42/arcus-upper/ref-owner/` (1–5); two free
cello photographs from Wikimedia Commons were used for the cello (credited on the cello
side-by-sides: "Woman playing cello during a concert in Moscow", Tatiana El-Bakri, CC BY-SA
2.0; "Cellist cello", T. Voekler, CC BY-SA 3.0).

**What was wrong**: the violin hand was mirrored — palm and knuckles on the G side, the thumb
on the E side — so from the picture's camera (in front, 4° to the player's left, 15° above,
which sees the E side of the neck) the fingers lay as a flat block on top and the thumb never
showed; the elbow was pulled out to the right, so the forearm came up from far below.

**What it is now** (`views/inst/arcus_draw_hand.rs::left_pose`, `left_pieces`,
`hidden_by_neck`, `left_paint`; poles in `scene/bowed_hold.rs::left_arm`):
- `Left::side` is the knuckles' side: `+x` of the instrument's frame for all four (the E
  string's under a chin, the C/E string's — the player's left — behind a cello or a bass).
  `Left` also carries the instrument's `Dims`.
- **Violin / viola**: the index knuckle against the neck's E side level with the
  fingerboard's top (1.5 cm beyond its edge), 1.6 cm toward the scroll from the first
  finger's tip (in first position by the nut); the knuckles step 1.6 cm toward the bridge,
  0.45 cm away from the neck and 0.2 cm up each (the little finger's side a little off the
  neck). The thumb up the G side opposite the first and second fingers, its tip level with
  the fingerboard (0.2 cm over its top, 0.6 cm beyond its edge), its joint at 70 % of the
  neck's depth, its knuckle under the neck. Fingers: phalanges 4.0/2.4/1.9 (index),
  4.5/2.8/2.0, 4.2/2.7/1.9, 3.4/2.0/1.7 cm; each finger a two-bone arch (proximal; middle +
  distal) bent toward up 1.0 + out 0.7 (arched knuckles, the tip landing from above), the
  last joint bowed 0.3 cm. Fingers sit a third of the span apart (`span / 3`: four fingers,
  three gaps), the playing one is the nearest by `round`.
- **Cello / bass**: the knuckles centred on the four notes, 2.0 cm beyond the C side, 0.6 cm
  over the fingerboard's top, 2.0 cm apart; the thumb behind the neck behind the second
  finger, its pad 0.7 cm under the neck's back.
- **Palm**: a fan of five strips from the wrist (±0.3 cm) to the knuckles, 7.2 cm long; the
  thumb painted first, under it. The wrist straight: on the line from the elbow to the
  knuckles' middle (iterated with the two-bone elbow).
- **Elbow poles**: violinist `down + 0.45·right + 0.15·forward` (under the violin, toward the
  chest's middle — photos 2 and 5); cellist's left `−right + 1.3·down` (out to the side, a
  little under the hand); cellist's bow arm `right + 1.6·down + 0.1·forward` (hanging, the
  elbow near the bow's line).
- **What is behind**: each piece of the arm is split where the neck stops hiding it from the
  eye (`hidden_by_neck`, an exact ray–neck test: the neck's width and top are linear along
  it, so it is a handful of linear bounds); hidden parts paint before the neck and the body
  are stamped back, the rest after. The upper arm always paints behind (the body's stamp
  covers what the body hides). From the violin's camera the hand's back covers the thumb;
  behind a cello the neck hides the thumb and the body the upper arm.
- **Frame**: the violinist's fit now holds the bow hand at the frog (a little past mid-bow
  on the A string, computed as the live bow places it) above the caption's band.

**The angles** (`the_left_arm_keeps_the_photographs_angles`, `--nocapture` prints them;
note A5 = 7 semitones on the A string, then an octave):

| | elbow (interior) | forearm–neck V | forearm rise | upper arm from vertical | elbow under the knuckles | wrist bend |
|---|---|---|---|---|---|---|
| violin, 1st pos. | 84° | 44° | 59° | 69° | 29 cm | 0° |
| violin, octave | 69° | 50° | 64° | 63° | 30 cm | 0° |
| viola, 1st pos. | 99° | 37° | 54° | 75° | 27 cm | 0° |
| cello, 4th pos. | 36° | 55° | 52° | 56° (17 cm out) | 26 cm | 0.1° |
| cello, octave | 47° | 75° | 43° | 40° (15 cm out) | 23 cm | 0° |
| bass | 63–68° | 47–58° | 47–54° | 53–88° (14–18 cm out) | 24–27 cm | 0.1° |

The violinist's elbow in the instrument's frame: (20, 19, −16) cm — under the body's middle
(the body runs 0–35.6 along y), 16 cm under its back. The cellist's elbow folds tightly
(36°) because the neck passes close to the left shoulder in our skeleton; a real cellist's
is nearer 50–70° in that position — the next pass on the cellist should move the cello a few
cm forward of the chest or the shoulder back.

**Frame cost** (`tests/arcus_hold_budget.rs`, ignored; optimised dev profile, the instrument
alone timed beside, best of three): violin +0.46/+0.80 ms at 100/200 % (p50 0.94/1.44 ms),
mannequin +0.52/+0.84; cello +0.35/+0.41. Budget: +1 ms at the median.

**Shots**: `docs/round42/arcus-upper/sheet-v5-hands/` — every instrument × musician and
mannequin × Photographic and Drawn × Night and Paper × 100 and 200 %, a second position for
the violin and the cello (Night), and the side-by-sides with the photographs
(`side-violin-*-vs-photos.png`, `side-violin-hand-*-vs-photos.png`,
`side-cello-*-vs-photos.png`).

**Still open (seen at 200 %)**: in Drawn the musician's face and neck show flat facets
(vertical bands) and a pale overexposed face in Paper; the hair is a hard bowl. The
cellist's bow hand hangs under the stick from the camera's side (its knuckles should read
on top). The palm's strips are visible as ridges.

## Realistic figures (owner, 2026-11-24): the faces are original

The owner asked for a realistic woman like his photograph `ref-owner/6-cellist-woman.png` and
a man in a suit. **That photograph shows a real, identifiable person** (Wikimedia Commons,
"Woman playing cello during a concert in Moscow", CC BY-SA 2.0): her likeness needs her
consent and a derivative brings ShareAlike, which ADR 0042 excludes. So **no face is copied**:
both faces are original (MakeHuman's generic CC0 adult averages, nothing fitted to a
photograph). From the photograph only the posture, the clothing (the black velvet
long-sleeved concert dress), the lighting mood and the general styling are taken. What the
owner liked is kept by design: a serene, calm expression, the eyes lowered toward the
instrument, soft refined features, fair skin with warm soft shading, light hair pulled back.
The same rule for the man. Later sessions keep it.

### The realistic figures: how they are made (decision, 2026-11-24)

- **Source: MakeHuman 1.x's CC0 assets**, converted offline (`tools/arcus-figures/`,
  DEVELOPING §4): the base mesh, the macro targets, the default skeleton and its weights
  (github.com/makehumancommunity/makehuman @ a8bc2d54, `makehuman/data`; LICENSE.md §C:
  "the base mesh and proxies, targets and modifiers, textures, clothes, poses — CC0 1.0"),
  the suit `male_elegantsuit01`, the shoes `shoes04` and the skins
  `young_caucasian_{female,male}` from `makehuman_system_assets_cc0.zip` (every asset
  "license": "CC0" in its `packs/makehuman_system_assets.json`). No MakeHuman code
  (AGPL) is copied: the converter reads the plain-text formats itself. CC0 needs no
  credit; the in-app credits will thank MakeHuman anyway (phase 2).
- **Why this route**: a real mesh is what makes a body read as a person; MakeHuman's is
  the one CC0 human with a rig, weights, clothes and skins together. Modelling a human in
  code would cost weeks and look worse; CC BY figures would add credits for nothing more.
- **Shape**: young adult, average muscle and weight, ideal proportions 0.6; the woman a
  mix 0.5/0.3/0.2 of MakeHuman's caucasian/asian/african averages, the man 0.45/0.3/0.25
  — generic, original faces. The eyelids are lowered over the eyes (the upper lid's bone
  weights turned 38° about the eye's centre, the lower 9°: a player's eyes on the
  instrument); the lashes' line darkened on the rims; the brows painted on the skin as a
  soft arc (no card); the skin's albedo detail (lips, cheeks) sampled per vertex from the
  skin texture as a luminance factor and a red shift, so the tone stays the user's.
- **Clothes**: her black velvet dress is her body's own surface cut along a scoop neckline
  and at the wrists (the cuts follow a smooth curve: `cut()` splits the triangles along the
  edge), the velvet 2 mm proud; the long skirt is draped in Rust round the seated legs
  (`scene/bowed_human_skirt.rs`). His suit is MakeHuman's, its jacket, shirt, tie and shoes
  told apart by its texture (the tie by its blue over a wide box, so its stripes stay tie),
  the jacket 1.8 mm proud of the shirt.
- **Hair**: a shell over the scalp cut along a hairline drawn round the head (elevation by
  azimuth tables `HER_LINE`/`HIS_LINE`), thickening from nothing at the hairline; her low
  bun an ellipsoid at the nape. Strand coordinates per vertex give the streaks.
- **Size**: `woman.fig` 0.50 MB, `man.fig` 0.63 MB (14 k / 17.5 k vertices; int16 at
  0.01 cm, four weights, the detail, half-float strand coordinates): embedded with
  `include_bytes!` (1.1 MB in the binary) — under what ADR 0030's on-demand download is
  for (sample packs of hundreds of MB).
- **Posing** (`scene/bowed_human.rs`): MakeHuman's 163 bones mapped onto 53; the body
  posed from the stylised skeleton (`bowed_figure::skeleton`): the jaw's left side on the
  chinrest (held; the chair's height follows) or the hips on the cello's seat with the
  trunk leaning to the head; legs by two-bone IK to the floor; skinned (4 bones a vertex).
  The arms are **live** as the stylised figure's are: each triangle has its part (body,
  left arm, right arm, a band over each shoulder in both); `views/inst/arcus_draw_flesh.rs`
  skins an arm per frame to the left hand's pose (`hand::left_pose`: fingers, thumb, the
  wrist's twist shared along the forearm) or the bow hand (`arm::bow_hand`), sorts its
  triangles, paints them smooth (`Brush::shaded_tri`), feathers its outline
  (`Brush::edge`) and fades it into the baked body at the band's cut.
- **Both pictures**: `Mesh::tris` (smooth triangles with a detail per corner);
  Photographic: `Tex::Skin` (detail, a wrapped terminator, a warm share, less rim),
  `Tex::Velvet` (pile sheen), `Tex::Hair` (streaks); Drawn: `scene/bowed_draw_tris.rs`
  (three shaded corners, the gradient along the brightness's change; cloth two-sided).
- **Fixed for every figure**: the bow hand sits **over** the stick (over = up in the room
  made square to the bow, not the string's normal: a cellist's hand hung under the stick
  from the camera's side); the cellist's shoulders 4 cm behind the chest's front (the
  elbow folded to 36° in fourth position).

### Next: the shared figure module (owner, 2026-11-24) — after the sheet's approval

The owner wants the musicians and the mannequins reusable by other instruments (the guitar
family, the winds, Chorda's pianist, Tympanum's drummer). **Order decided**: the realistic
figures' design sheet first (the owner is waiting for it), then this refactor — the sheet's
code is already split along the lines the module needs (`bowed_human` is instrument-agnostic
but for `skeleton()`'s input), so the refactor moves code rather than rewriting it.

Plan (ADR 0159 unless the integrator renumbers):
- `crates/cantus-ui/src/scene/figure/`: `rig.rs` (named joints: pelvis, spine, chest, neck,
  head, shoulders, elbows, wrists, the fingers' joints, hips, knees, ankles; `Rot`, `Xf`,
  `Aim`, `Posed`, two-bone IK with a pole, the wrist clamp, the photo-derived limits:
  elbows out and down), `who.rs` (the four figures: stylised musician, realistic woman,
  realistic man, mannequin; their tones), `real.rs` (today's `bowed_human`: the `.fig`
  reader, skinning, emit), `stylised.rs` / `mannequin.rs` (today's `bowed_figure_parts`),
  `pose.rs` (seated with the chair's height a parameter, standing; the instrument's layer),
  `grip.rs` (the grip library: violin-neck, cello-neck, bow-frog now — data: finger curls and
  spreads, plus a rule placing the knuckles/thumb from the target), `live.rs` (today's
  `arcus_draw_flesh` + the stylised tubes: the live limbs, the behind/in-front split against
  any occluder), `bake.rs` (meshes cached per (figure, pose, look), nothing at rest).
- **The contract** an instrument fills (`PlayRig`): the seat (chair at a height, a stool,
  standing) and where; each hand's target (a point, a facing, a grip name: "violin-neck",
  "cello-neck", "bow-frog", later "pick", "fret", "keys", "stick"); the head's attachment
  (a chin rest's point, or a look-at point); the occluders (a neck's hull, a body's outline,
  a keyboard's lid, a drum) and which limb parts each hides. Arcus becomes its first client;
  its tests and pictures stay identical.
- A test with a dummy client (a "held stick": a standing figure, both hands on a rod from the
  contract alone) proves it is not Arcus-shaped.

## How to use the realistic figures (today)

`Hold::Woman` / `Hold::Man` in `Holder` (key "woman" / "man"; `Holder::skin` the tone,
`hp::REAL_SKINS`). `bowed_hold::build` bakes the body (`bowed_human::build`), `left_arm` /
`arm` give the live arms their shoulders and lengths from the posed body, and
`arcus_draw_body::live`/`live_behind` draw them through `arcus_draw_flesh::arm`. Shots:
`shoot_arcus <dir> --hold=woman,man --only=cello-play+woman`. Rebuild the meshes:
`python tools/arcus-figures/build_figures.py <makehuman/data> <system assets>
crates/cantus-ui/assets/figures --preview <dir>` (sources in the script's header).

### Sheet v6 (the realistic figures) — sent for approval, 2026-11-24

`docs/round42/arcus-upper/sheet-v6-realistic/`: the woman with the cello and the man with the
violin, Photographic and Drawn, Night and Paper, 100 and 200 %; `side-cello-woman-*-vs-
reference.png` beside the owner's reference photograph (credited; posture, dress and light
only). **Waiting for the owner's verdict** before the rest (the other instruments' shots, the
Settings rows, the module refactor above).

Known, to do after approval: `arcus_hold` with the realistic figures — the woman's viola
elbow opens to 132° (her shoulder far from a viola's neck: lean her in or seat her closer),
and the behind-the-neck pixel test's cello man (the flesh arm's behind pass not yet seen by
the test's sampling); the frame budget for the flesh arms not measured yet; the man's hair
reads a little as a cap at 200 % and his face is flat in the Photographic key; the hands are
MakeHuman's fingers bent to the stylised poses (readable, not yet refined); tiny cuff shards
where a shirt's cuff meets a sleeve; the skirt's sides show the knees' velvet at the hem.

## Sheet v7 — the owner's verdict on v6 (2026-11-24) and what it changed

The owner on v6: "Más realistas, por favor, más parecido a la foto original (sin ser la
misma cara), y ten más cuidado con las manos". **Hard rule kept: neither face is the
photographed woman's** (a real person, CC BY-SA 2.0): from the photo only posture, dress,
light, hair styling.

Done (commit 9be4ec6ea and after):
- **Skin grain** gone: `raster::Shadow::soft` (the disc unrotated, twice, bilinear compares)
  for `Tex::Skin`/`Tex::Hair` through `Shadow::light_for`; the figures' silhouette normals
  bent to graze the eye instead of flipped (`shade`: a dark outline round faces/sleeves).
- **Faces**: MakeHuman's CC0 face modifiers (`targets/{head,cheek,chin,nose,mouth,
  eyebrows,neck}`, sparse-checked out beside the rest) chosen by eye on the generic average
  (`FACE` in `build_figures.py`): hers oval, high cheekbones, narrow nose, fuller lips,
  long neck; his squarer jaw, firm chin. Warm cheeks/lips/nose/ears (`warmth`). Lids
  lowered 27°/6° over **eyeballs** looking down 32° (`Stuff::Eye`/`Iris`), **lashes** along
  each upper lid's edge (`lid_margin`, `lashes`; the base mesh is closed round the eyes, so
  the old open-edge lash line found nothing).
- **Nails** (`Stuff::Nail`, `Role::Nail(tone)`): the body cut along each nail's outline
  (`nail_field`: on each last phalanx's back, the hand's back from its knuckles' plane).
- **Hair**: strand coordinates are now (across = the angle round the head's front-back axis,
  folded under the middle; hairline blend) — the swap is read by `px_shade::Tex::Hair`,
  `px_shade::hair` (grooves + Kajiya–Kay sheen along the strands) and
  `bowed_draw_tris::hair_at`; a degenerate-uv triangle's tangent is now the in-plane
  gradient of `u` (`raster::place`). Ash-blonde `HAIR_FAIR`.
- **Dress**: a V neckline, shoulders a little gathered, soft velvet folds (`Tex::Velvet`'s
  normal swung by slow noise).
- **The live arms** are lit by the picture's own light (`flesh::Shading`: the Drawn `Light`
  + `skin_tone`; the Photographic key/fill/ambient fitted to the bake, its `raster::tone`),
  with **capsule occlusion** between the fingers, the palm and what the hand holds (the
  stick, the neck: `flesh::Capsule`).
- **Fingers**: each phalanx turned from the hand's turn by the shortest way (no twist —
  the roll left to the parent crumpled the knuckles); non-playing fingers reach 70–86 % of
  their own length (`HandAim::exact` = the playing finger, exact).
- **Bow hold** designed in `tools/arcus-figures/hand_design.py` (+ `hand_rig.py`, a mirror
  of `arm_aims`) against the Commons photographs "Bow hand Violoncello.jpg" (CC BY-SA 2.5)
  and "Right hand position on bow.jpg" (Martin Morgenstern, CC BY-SA 3.0): knuckles over the
  stick, fingers draped to its far side, thumb bent at the frog; the hand's size from the
  figure (`Arm::palm` = forearm / 21.4 cm).
- **Bow arm** (cellist, every figure): pole `r·0.6 + down·2 + f·0.6` (forward, low); a figure
  starts with the bow at 0.3 of its hair (the hand in front of the body); the cello's sheet
  shot plays the G string (the bow tilts down toward its tip as the reference's does).
- **Violin/viola**: the realistic trunk turns 0.2 rad to the left (`TRUNK_TURN`).

### Sheet v7 — delivered (2026-11-24), waiting for the owner

`docs/round42/arcus-upper/sheet-v7-realistic/` (made by `tools/arcus-figures/sheet_v7.py
<shoot_arcus.exe> <dir> <refs> [--no-shoot]`): the woman and the man with the cello, the
bass, the violin and the viola, Photographic and Drawn, Night and Paper, 100 and 200 %
(64 shots); `side-cello-woman-{photo,drawn}-{night,paper}-vs-reference.png` (labelled
"posture, dress and light only"); `hand-{bow-cello-woman,bow-violin-man,left-violin-woman,
left-cello-woman}-200-vs-photo.png` (200 % crops beside Commons photographs of real bow holds
— "Bow hand Violoncello.jpg" CC BY-SA 2.5, "Right hand position on bow.jpg" M. Morgenstern
CC BY-SA 3.0 — and the owner's references). Best first: the cello woman photo/night side by
side; the violin man's bow hand; the violin woman's left hand; the photo/night violin pair.

Also in v7 after the list above: the live arms **thinned** offline (`thin.py`: short edges
collapsed in each arm's own part to 0.62 cm, ~6.5k → 3.6k triangles an arm for her), lit
from a **kept normal map** (`lut_colour`, an octahedral 40² map per stuff, remade when the
light or the view moves), skinned **once for both of the left arm's passes**; each hand
placed by **its own length** (`Arm::reach`, `bowed_human::hand_reach`: the man's knuckles had
landed 4 cm past the neck); behind a cello whatever lies deeper than the neck's middle paints
before the stamps; the realistic cellist sits 4 cm further back (`SIT_BACK`: the man's elbow
folded under 30°); the hair's sheen soft (Kajiya–Kay at a low level; per-triangle tangents
showed facets when strong), its hairline broken up by noise; a narrower V (8.4 cm), the
velvet lifted along the uncut body's normals (no fins).

**Gates (this build):** `arcus_hold` 7/7 (the angles and the behind-the-neck test pass for
every figure and instrument, the realistic arm's own skinned vertices sampled too),
`arcus_scene`, `scene_identity`, `kit_lint` pass. **The frame budget does not:** the
realistic arms cost +2.6 to +3.7 ms a frame at the optimised dev profile (the stylised +0.5
to +1.1, the machine busy with other builds) — next: a level of detail for the hands at
100 % (a second thinning at ~1 cm), and a quiet release-profile measurement.

## The figure module (ADR 0159) — done

`scene/figure/`: `rig` (Rot, Xf, Aim, `two_bone`; re-exported by `bowed_human`/`bowed_hold`),
`play` (the **PlayRig** contract and the poser: head at a jaw/chin rest or the eyes; seat
hung from the head, a chair, or standing; legs forward/to points/straight; each hand a pose,
a grip or at rest; occluders), `grip` (`BowFrog` — the sheet's bow hold, moved from
`arcus_draw_arm` —, `Rod`), `live` (`Capsule`, `shade`). Arcus is the first client:
`bowed_human::arcus_rig` + `body() = play::pose(...)`. The dummy client:
`tests/figure_contract.rs` (standing, both hands on a stick, from the contract alone).
Settings ▸ Appearance: **Held by** and **Stopped note** rows (both locales; the hint
`arcus_upper.hold.hint`); the old `arcus_finger` migration was already in
`Settings::migrated`. ADR 0157 amended ("who holds it"). MakeHuman credited in About ▸
Credits (`about.credits.figures`, both locales).

### Gates at b8f362e4a and what is still open (2026-11-24, end of the v7 session)

Green: `arcus_hold` 7/7, `figure_contract`, `arcus_scene` (nothing built or allocated at
rest — the light maps are kept per arm), `scene_identity`, `kit_lint`, the i18n lib tests,
`cargo clippy -p cantus-ui --all-targets -- -D warnings`; `THIRD-PARTY-NOTICES.md`
regenerated (MakeHuman's section in `xtask/notices/head.md`).

Open:
1. **The frame budget** (`arcus_hold_budget`, ignored): the realistic arms +2.6 to +3.7 ms
   (dev profile, busy machine) against +1 ms. Next: a hands' level of detail at 100 % (thin
   again to ~1 cm for small views), measure in release on a quiet machine.
2. The WSL Linux check (`tools/wsl/check.sh`) not run this session.
3. The Settings rows are not shot yet; the room card's holder rows (the earlier plan) are
   not done — Settings ▸ Appearance holds them.
4. The violin's left hand reads as a hand but its arches are low from the camera's side;
   the man's hairline still shows the shell's triangles faintly at 200 % (Photographic).
5. The stylised musician and the mannequin still pose from `bowed_figure` (the next to move
   behind the PlayRig).
6. The owner's verdict on sheet v7.

## Next (coordinator's order, 2026-11-24)

0. **Waiting**: the owner's verdict on sheet v5's hands. Nothing below starts before he
   approves them. Done meanwhile from this list: the frame (bow hand over the caption), the
   mannequin's wrist joint, the gates and the frame time (above).

1. Violin musician: softer hairline, jaw/cheek form, neck in shade; frame higher/wider (bow
   hand clear of the caption); thinner bow arm on the violinist.
2. Mannequin's wrist ball never drawn (`arcus_draw_arm.rs` / `arcus_draw_hand.rs`: draw the
   wrist joint after the hand).
3. 200 % artefacts: strip banding on live limbs (`lit::tube`, `STRIPS`), facet grid on hair/
   shirt in Drawn (`bowed_figure::shell` resolution / flat shading).
4. Gates: arcus_hold, arcus_scene, scene_identity, kit_lint, clippy `-D warnings` on cantus-ui;
   time the frame with the live arms beside a reference.
5. Double stop (needs the second voice's telemetry), then C: bow follows playing (direction,
   speed, β, tilt per string, crossings), articulations (pizz hand, tremolo, staccato/
   spiccato), vibrato rocking, string blur/decay, breathing/head sway (eyes closed, smooth),
   reduce-motion / dancer on-off precedent, zero CPU at rest, frame budget measured.
6. UI rows (holder, tones, stop) on the card and in Settings ▸ Appearance; ADR 0157 amendment;
   `docs/round42/arcus-upper.md` section 3 update.

## File map (all lines marked `// r42:arcus-upper`)

- `src/scene/bowed_hold.rs` — Hold/Holder/StopShow, `pose`, stands, `fit`, `arm`/`left_arm`
  (cached skeleton), `elbow` (two-bone IK).
- `src/scene/bowed_figure.rs` — skeleton (violinist / cellist), `shell`/`ball`/`taper`, fit.
- `src/scene/bowed_figure_parts.rs` — musician (head, face strokes, hair, torso, legs) and
  mannequin; arms are live.
- `src/kit/skin/scene_hold.rs` — skins, cloths, woods, stand, pads.
- `src/scene/bowed_draw.rs` — camera/points per holder, wall shadow, held ebony bias.
- `src/views/inst/chorda_scene_hybrid.rs`, `chorda_scene_px_bowed.rs` — Photographic path.
- `src/views/inst/arcus_draw.rs` (hand glide, Play), `arcus_draw_body.rs` (live layer),
  `arcus_draw_arm.rs`, `arcus_draw_hand.rs`, `arcus_draw_lit.rs`.
- `src/settings.rs` — `arcus_holder`, `arcus_stop`, `arcus_finger_was`, `migrated`.
- `tests/arcus_hold.rs` (grip, angles, behind the neck), `tests/arcus_hold_budget.rs`
  (ignored timing: the live arms beside the instrument alone).

## Open bugs

- Mannequin wrist ball: now drawn (under the palm, the size of the wrist).
- Photographic (a) wall shadow not confirmed visible (key moved beside the camera).
- Violin figure: the bow hand clears the caption and the torso no longer crowds the left (v5); the face in Drawn is faceted.
- Drawn: facet grid on dark hair/shirt at 200 %; live limbs show strip banding.

## Session of 2026-11-24 (evening): the violin's hand from data, realism, Figure detail

### The owner's words this session (record them; later sessions keep them)

- On v7's frame cost: "I'd rather it look good than save ms, or make it configurable in
  case someone wants fewer ms."
- On v7's hands: "The violin's left hand is a disaster. I think you can make the characters
  more realistic, and the girl even more like the photo."
- **Her face** (settled by the owner through the coordinator): *change the features enough
  that she is another person.* Keep the type — young, fair skin, light brows, a soft oval
  face, serene — but never trace, fit or morph toward the photographed face; her eye shape,
  nose, mouth and proportions clearly different from the photo's. The photo is a quality
  bar and a styling reference only (lids with depth and lashes, soft cheekbones, a delicate
  nose, natural lips, warm cheeks, a clean hairline, soft flattering light). Styling taken:
  light blonde hair swept back with soft volume at the crown, tied low; black velvet, long
  sleeves, a V neck, a fitted bodice; small drop earrings; upright, chin a little lifted,
  leaning into the cello; soft high-key light on the face.

### The violin's left hand, posed from data (`scene/figure/neck_hand.rs`)

- Authored offline by `tools/arcus-figures/left_hand.py` (DEVELOPING §4): the inputs come
  from the app (`CANTUS_HAND_DUMP=<dump.json> cargo test -p cantus-ui --test
  arcus_hand_table -- --ignored dump`), the solve runs a process per figure and instrument
  (`solve <dump> --only=woman-violin` × 4, ~30 min side by side), `pack` writes
  `assets/figures/left_hand.bin`, `look <dump> <dir> who inst string finger at` renders a
  row from the picture's camera, photograph 4's angle, photograph 3's and photograph 2's.
- The rules (each a weighted residual, names in `Pose.residuals`): playing tip on its stop
  (exact after the runtime shift), its last phalanx nearly upright, the others hovering
  within 0.8 cm of their notes; MCP/PIP/DIP within 45–70/70–100/20–50° (hovering 35–70/
  60–100/15–50); each middle joint 2.4 cm over the fingerboard; nothing inside the neck
  but the playing tip; the index finger's base against the E side (0.85 cm out, −0.7…+0.3
  cm from the top); the knuckles at least level with the top (photograph 2); the thumb's
  tip on the G side, level with the top, opposite fingers 1–2; the palm 0.4 cm clear of the
  neck, toward the player and the neck (`PALM_WAY`); the forearm straight on the hand's
  line at its own length, the upper arm its own length from the shoulder, the elbow under
  the violin.
- `tests/arcus_hand_table.rs`: every figure × violin/viola × string × eight notes — the
  skinned hand's own fingertip within a pixel of the stop at 100 %, the playing finger's
  angles, its last phalanx upright, the index base on the E side, the thumb's tip on the G
  side level with the fingerboard.
- The 4-up for the owner: `tools/arcus-figures/hand_sheet.py <shoot_arcus.exe> <dump>
  <dir> [woman|man] [photo|drawn] [night|paper] [--quality=light]` (photo 4 | the app from
  photo 4's angle through `shoot_arcus --cam=` | the app's own camera at 200 % | photo 3).

### Figure detail (`scene::figure::Quality`, `Settings::figure_quality`)

- Settings ▸ Appearance, under *Held by* / *Stopped note*: **Figure detail** — High (the
  default), Light, Auto; a hint line under it (`arcus_upper.quality*`, both locales).
- Light: `arcus_draw_flesh::lod` merges the arm's vertices per bone on a grid
  (`LOD_CELL`: 1.15 cm on the hand while it is drawn over `LOD_FINE_PX` px a cm, 1.6 cm
  when smaller; the sleeve 2.6/3.6 cm), keyed by the main bone and the stuffs a vertex
  touches (fingers, nails and cuffs never merge), each cell's vertex nearest its middle
  standing for it; no creases' occlusion; the stylised limbs in 8 strips instead of 16
  (`Lit::strips`). The meshes are made once on first use and kept. The bones are High's,
  so the grip, the hand behind the neck and the fingertip are too.
- **Measured** (release, beside the instrument alone, best of five, the final build of
  2026-11-25; the table in ADR 0159; the test passes): Light −0.01 to +0.45 ms; High: the
  woman +1.26 to +1.61, the man +1.65 to +2.37, the stylised +0.07 to +0.51 (an earlier
  build: up to +2.20 / +1.13). The test's ceilings: Light +1 ms (p95 ≤ 1.5 × alone + 1 ms), High 3.0 ms
  realistic, 1.6 ms stylised (`HIGH_100`, `HIGH_200`, `HIGH_STYLISED_MS`). The slow frames
  were the light maps remade whenever the arm's middle crossed a centimetre (vibrato):
  kept now over 4 cm.
- Auto (`figure::frames_long`): Light while `CantusApp::average_frame_ms` (the last 120
  frames) is over 12 ms, back under 7 ms; off unless chosen (FL Studio lowers its visuals
  under load the same way, but never by itself here: the default stays High).

### Realism (Photographic) and her styling

- Skin: the key's terminator wrapped wider (0.2), a stronger warm share where the key falls
  off and a little red in the shade, a soft fill from the camera's side on what faces it
  (`px_shade::shade`, the live arms' `flesh::colour` the same); figures' creases take the
  screen-space occlusion in their direct light too (`crease`).
- The room behind a realistic figure goes soft (`Rig::soft_bg`: a wider blur reached
  sooner, only the background's own samples blurred together — a plain blur drew a halo
  of hair round the head).
- The soft shadow's bias 1.8 cm + 0.6 cm/m (fine diagonal stripes on a face at 200 %).
- Her face (`FACE["female"]`, an original person — see the owner's words above); her
  hairline rounded over the brow (`HER_LINE`), the hair 1.4 mm proud inside it (no scalp
  through it), soft volume at the crown, the bun low at the nape; small gold drop
  earrings found on each lobe (`earrings`, `Stuff::Jewel` → `Role::Brass`); the blonde
  lighter (`HAIR_FAIR`). A realistic cellist leans 2.5 cm further into the cello, the chin
  lifted 0.1 rad (`LEAN_IN`, `CHIN_UP`).

### The coordinator's review of the first previews (2026-11-24, late) and what it changed

The review: the left hand "a real step forward" (it wraps the neck, the thumb under it,
the tips on the strings); then, in order: hands like clay (short thick fingers, no nails,
creases or tendons, flat pale colour; the man's fingers a fist on the board); ragged cuffs
(a white/black sawtooth); her face porcelain (brows, lashes, lid lines, rose lips, warmth,
uneven skin, a sheen; eyes half-open looking down; he too pale, brows); the hair a
striped helmet (locks, a soft hairline without a band or sparks, loose strands at the
temples, a real bun); **modesty — her dress read as body paint: a draped concert dress,
a looser bodice, soft folds, no anatomical contour; long sleeves with some bulk; the
figures clearly modest and elegant** (the owner's Christian values); the cellist's face
and hair the same.

Done (`build_figures.py` unless named):
- **Bodice** (`dress`): the trunk's front half blurred in depth (a 5 cm Gaussian, two
  passes, weighted in softly) so the cloth lies over the forms, a welded Laplacian
  smoothing (160 passes; never across the armpit), an ease off the body (0.7 cm + 0.6 in
  front), the sleeves 0.35 cm fuller. Checked from the front, three-quarter and side.
- **Cuffs** (`arcus_draw_flesh`): where two stuffs meet on a live arm the upper one's edge
  is feathered over the other (`rank`: sleeve over shirt over skin); the shirt's cuff sorted
  3 cm behind the sleeve.
- **Face**: brows darker (hers light brown), a rose lip spot (`lip_spot`), cheeks/nose
  warmer, a soft uneven redness, his skin warmer; the lids 14°/3° (half-open over eyes
  looking down at the strings); the skin's sheen up (`chorda_scene_px_bowed`, lacquer
  0.09/0.38).
- **Hair**: locks across the head (`thick`'s `locks`), a chignon (`torus`) over a smaller
  core; the bright strands at the hairline gone. Loose strands at the temples were tried
  (thin ribbons) and taken out: at 200 % in the Photographic look they read as bright
  scratches on the skin (the per-pixel bake has no coverage for a strand thinner than a
  sample) — open: real fine strands need an alpha-tested hair card in the bake.
- **Hands**: the fingers drawn in toward their phalanges' axes (`slim_fingers`, 0.86, the
  thumb 0.92); the joints' creases darker, the fingertips and the palm's edge warmer
  (`finger_tint`); the nails lighter and glossier (`flesh::colour`); the table's fingers
  spread (`SPREAD`), held near the middle of their ranges (`MID`), each hovering toward its
  own string.

### The review of prev4/prev5 (2026-11-25): cards, skin texture, fingertips

- **Cards** (`scene/figure/cards.rs`, ADR 0159 amended): MakeHuman's CC0 `ponytail01`
  (hers; the tail cut, the chignon is ours) and `short02` (his) hair, `eyebrow001` /
  `eyebrow010` brows and `eyelashes01` lashes, fitted by `build_figures.py::card_proxy`,
  textures as grey + alpha (`assets/figures/<who>_<hair|brow|lash>.png`, `cards()`). The
  Photographic raster alpha-tests them per sample (`raster::cuts`, `raster_rows`), a
  realistic figure's picture keeps 2×2 samples at any size (`hybrid::render`); a card casts
  no shadow (its strands' shadows on the scalp read as dark patches); a hair card is lit
  by its head of hair's radial normal (one smooth light over every card), its texture's
  grey held near its mean (`Card::shade`, 0.85…1.18); a thin scalp in the cards' finish
  under them, set back 5° from the hairline so the strands make the hairline
  (`Stuff::Scalp`); the thick shell stays the Drawn picture's (`Stuff::Shell`).
- **Skin texture** (face and hands in the bake): the body's vertices split at the skin
  texture's seams (`split_seams`: copies appended, so every proxy's references still name
  the base mesh), each vertex's texture coordinates in the `.fig` (hers u 0…1, his 2…3);
  the skin's fine detail — MakeHuman's own diffuse, its luminance over a 6 px blur of it
  (`skin_texture`, `assets/figures/<who>_skin.png`, 1024 px) — sampled per sample in the
  bake (`raster::place` → `cards::skin_detail`) on the vertex detail's luminance. The
  copies share one normal (`Figure::weld`: `bowed_human::skin`, the live arms'
  `weld_of`), and an edge between copies is not an outline. The live arms (the hands in
  the live layer) keep the per-vertex colour: egui draws them untextured — open.
- **Fingertips**: the last phalanx's last half never slimmed, and never thinned
  (`main`'s `tipv`), so the tips keep MakeHuman's round pads.

### Sheet v8 and where this session leaves it (2026-11-25)

`docs/round42/arcus-upper/sheet-v8/` (`tools/arcus-figures/sheet_v8.py <shoot_arcus.exe>
<dump.json> <dir>`, the dump's solved caches beside it): the woman and the man with the
violin and the cello, High and Light, Photographic and Drawn, Night and Paper, 100 % and
200 % (the Light shots themselves are left out of the tree: they are in the
`side-*-high-vs-light.png` side-by-sides); `face-woman-*` (her face at 200 %, ×2);
`hand-left-violin-*-4up.png` (photograph 4 | the app from its angle | the app's camera at
200 % | photograph 3, both looks, High and Light); `settings-appearance-*` (Settings ▸
Appearance whole, the figures' rows: *Held by*, *Stopped note*, *Figure detail*).

Gates (this build): `arcus_hand_table` (the table's hand: the skinned fingertip within a
pixel at 100 % in every case tested, the playing finger's angles, upright tip, the crook;
Figure detail's preference), `arcus_hold` 7/7 (the behind-the-neck test at Light in Drawn
and High in Photographic), `arcus_scene`, `scene_identity`, `figure_contract`, `kit_lint`,
`ux_whole_words`, `instruments_scale_e2e` (`instruments_narrow_e2e` lives on
`r42/fix4-narrow`, not this branch), the lib's i18n and `figure::` tests,
`cargo clippy -p cantus-ui --all-targets -- -D warnings`.

Open:
1. Her hair: the ponytail cards still show faint lighter and darker patches at 200 %
   (the texture's own regions; the shade is held near its mean, the scalp under them in
   their finish) — next: per-card colour from the texture's average and a strand-direction
   (Kajiya–Kay) sheen shared across cards from the cards' texture tangents.
2. The skin texture is the bake's (face, neck, the baked hands); the live arms' hands keep
   per-vertex colour (egui draws them untextured) — next: the live arm as a textured egui
   mesh (the same high-pass texture, its UVs in `ArmMesh`).
3. The eyes' own textures (MakeHuman's `brownlight_eye.png`, CC0) are not used yet: the
   eyes are half-open and small at 200 %; worth it at 400 %.
4. The room card: Arcus has none (its picture is always the studio), so the holder rows
   live in Settings ▸ Appearance only; a room card for Arcus would be its own design.
5. Auto (Figure detail) is decided by the window's average frame time; it is not tested
   end to end with a real slow frame (its state machine is, `figures_long`).
6. The WSL Linux check (`tools/wsl/check.sh`) not run this session.

## arcus-hair (2026-11-25): one head of hair, the hairline, the eyes

Branch `r42/arcus-hair` (worktree `F:\projects\daw-r42\arcus-hair`), from r42/integration
e3396db94. The owner's most visible defect on sheet v8: her ponytail cards rendered as
camouflage at 200 % (large lighter and darker patches across the crown) and the hairline a
hard edge against the forehead. ADR 0159's `cards` amended ("One head of hair").

**What the patches were**: (1) the scalp under the cards stood 2.5–3.9 mm over the skin
while a quarter of MakeHuman's ponytail cards lie within 2.4 mm of the skull — wherever a
card dipped under it the scalp (its own normal, its own finish) showed instead; (2) the
ponytail's texture carries large baked light and dark regions (its butterfly layout: dark
round the crown, light patches over the sides) and the bake read its grey as the hair's
light. Found with a debug render that painted the scalp dark (`CANTUS_HAIR_TUNE` during the
session, taken out before the commit).

**What changed** (`build_figures.py`, `hair_maps.py`, `scene/figure/cards.rs`,
`px_raster.rs`, `px_shade.rs::strands`, `px_bowed.rs`):
- the hair textures stored as maps (detail without large-scale light, the strands' way,
  coverage), read at the mip level a sample reaches;
- alpha to coverage over the 2 × 2 samples (ordered, turned per triangle), the strands'
  soft fringe drawn in toward their cores;
- Kajiya–Kay from the texture's strand way (dP/du·du + dP/dv·dv), one diffuse from the
  hair's middle for the cards, the scalp and the chignon; her blonde at 0.6 of the Drawn
  colour (it clipped to white per pixel); his sheen faint and his detail at a third (his
  short curls run every way: a strong sheen read as white blotches);
- the scalp 0.4 mm over the skin, every card ≥ 1.2 mm over it (`lift_cards`), her front
  cards drawn 8° down to the hairline (`reach_line`);
- the hairline: degrees-in-from-the-hairline per card vertex (the `.fig`'s luminance byte;
  `Figure::line`), her cards thinning over the last 5° (`cards::feather`, `cards::thin`),
  her scalp thinning at its edge with a waviness along it (`cards::scalp_cover`);
- brows and lashes a level and a half softer with their coverage raised (no dotted lines);
  lashes 1.8 × lighter in the bake (black blobs over the eye);
- the eyes: MakeHuman's CC0 `bluegreen_eye.png` (hers) and `brownlight_eye.png` (his),
  cropped round the iris (`assets/figures/<who>_eye.png`), mapped down the gaze
  (`build_figures.py::eye_uv`, `Tex::Eye`, `cards::eye`), wet (coat 0.6, rough 0.04); lids
  5°/2° (from 14°/3°), the gaze 28° down and 12° to the player's left (`GAZE_DOWN`,
  `GAZE_LEFT`). Notices: `xtask/notices/head.md` names the eye textures.

**The coordinator's review of the first faces** (same day) and what it changed:
- the silhouette (hair, jaw) stepped against the soft room and the hairline was a jagged
  staircase: **the app's own pictures never had the realistic figure's extra samples** —
  `hybrid::render` forced 2 × 2 for a realistic figure, but the app (and `shoot_arcus`)
  bakes through `render_of`, which did not; at 200 % the picture was baked at one sample a
  pixel, so every card's coverage was all or nothing (the hair's dots) and no edge was
  smoothed. Now `hybrid::figure_samples` serves both: three samples a side while the
  picture fits three times the bound (`MAX_SAMPLES`: a 100 % picture), else two (200 %). A pixel a figure's
  silhouette crosses sums its samples evenly (`raster::figure_mat`; Karis's brightness
  weighting gave a light jaw or her hair too small a share of its edge). Her hairline's
  ramps widened (cards over 8°, the scalp's edge over 6°);
- the irises a natural grey-blue (half their saturation, 0.62 of their light) with the
  upper lid's shadow over the top of the iris (`cards::eye`);
- her lips a touch more rose (`lip_spot`: red +0.45, luminance −14 %);
- his scalp darker than his hair (0.55: his short cards leave wide gaps, and a scalp as
  light as the hair read as pale patches over his crown — `HairLook::scalp`).

**Shots**: `tools/arcus-figures/hair_sheet.py <shoot_arcus.exe> <dir> <sheet-v8 dir>` —
the woman with the violin and the cello, the man with both, Photographic, Night and Paper,
200 %: `face-*` (×2), `head-*` and `before-after-<who>-<inst>-<theme>.png` (sheet v8's own
shot cropped the same way beside the new one). Sheet: `docs/round42/arcus-upper/sheet-v9-hair/`.

**Gates (this build, 528de5064 and after)**: `arcus_hold` 7/7, `arcus_hand_table`,
`arcus_scene`, `scene_identity` (Chorda's and Tympanum's pictures unchanged: the hair's
paths are a realistic figure's only), `figure_contract`, `kit_lint`; the lib's `figure::`
tests (6, among them the maps' no-large-scale-light test: every covered 64-texel block's
mean within 16 % of 1, sheet v8's texture 0.27 … 1.97); `cargo clippy -p cantus-ui
--all-targets -- -D warnings`; `arcus_hold_budget` in release (High realistic held p50
1.69–2.23 ms, within 3.0); rustfmt on every touched file; `THIRD-PARTY-NOTICES.md`
regenerated.

**Open**:
1. The hairline is soft at 100 % and 200 % but still shows a faint lighter band at her
   temples on the violin (Night): the cards there end a little short even after
   `reach_line`; a longer reach at the sides (|θ| 40–90°) or a few strands' cards of our own
   along the hairline would close it.
2. The Drawn look's hair is unchanged (the thick shell): the cards are the Photographic
   bake's only.
3. The live arms' hands still use per-vertex colour (egui draws them untextured).
4. Fine loose strands (flyaways) at the temples need cards of their own; not done.
5. The man's hair keeps a faint sheen only: his short02 texture's curls give the strands'
   way no coherent direction. A groomed short-hair texture would let him have a sheen.
6. The WSL Linux check (`tools/wsl/check.sh`) not run this session.


## arcus-hand (2026-11-25): the violin's left hand from photographs, the eyes' setting

Branch `r42/arcus-hand` (worktree `F:\projects\daw-r42\arcus-hand`), from r42/integration
f42f88109. The owner: "URGENT LA MANO IZQUIERDA DEL VIOLIN, ESFUÉRZATE BIEN EN ESO" — after
three passes still "un desastre"; more realistic musicians; the eyes closed or open (he
liked them closed: "parecían concentrados o disfrutando de la música"); more photographs of
professional violinists, in orchestras too. ADR 0159 amended (`neck_hand`, `Eyes`).

**References** (study only, never in the tree): `F:\projects\daw-r42\refs-violin\` — 40
Wikimedia Commons photographs (soloists from the front and front-left, close-ups, sections,
the player's side), `sources.md` (page, author, licence, view, position), a contact sheet,
eight crops, `annotated/` (the landmarks drawn). Measured in
`tools/arcus-figures/left_hand_refs.md` (forearm–neck 50–85°, the hand steeper than the
forearm by 3–23°, the knuckles from level with the fingerboard's top to one neck thickness
under it, the arches 1–2 neck thicknesses over it, the back at a slant, 0.5–0.9 as wide as
long, the thumb hidden from the front).

**What was wrong, and the fixes** (each with its number in `left_hand_refs.md` / the ADR):
1. MakeHuman's finger bones begin at the webs, 1.2–1.5 cm past the real knuckle joints: the
   fingers folded mid-phalanx, the palm ran on into them (the fist). `knuckle_fix` moves each
   base joint back 12 % along its metacarpal and hands the skin between to the finger
   (Rust `bowed_human::knuckle_fix` and Python `hand_rig.knuckle_fix`, alike).
2. The hand was broadside to the camera: the solver now holds the back turned 30° toward the
   scroll and rolled 20–26° down (within 5°), the knuckles beside the E side at the board's
   height, the first phalanges rising, high arches (scaled by the hand's length), the tip
   within 20° of the board's normal, the fingers parallel, the wrist's turn 0–6° (kept per
   row: the table's version 2).
3. The camera saw the neck from 59° toward the scroll; the photographs are square to it
   (88–90°). The realistic violinists' camera is now azimuth −20°, 5° up
   (`bowed_draw::REAL_HELD_VIEW`; `shoot_arcus --view=4,7` shows the earlier one).

**High's hand** (`arcus_draw_flesh`): curved PN patches (`fine_tri`, ≤ 4 cuts a side, ~2.5 px)
lit per point with the skin texture's detail (`cards::skin_detail`), the knuckles redder,
the nails pale at their free edge (`hand_detail`), the patches' outline along their curved
edge; each fingertip's soft shadow on the fingerboard (`arcus_draw_body::contact_shadows`).

**Eyes** (`scene::figure::Eyes`, `Settings::figure_eyes`, Settings ▸ Appearance under Figure
detail, both locales `arcus_upper.eyes*`): Closed by default; the lid morph from
`tools/arcus-figures/lids.py` (`assets/figures/<who>_lids.bin`); `shoot_arcus --eyes=open`.

**Tools**: `hand_study.py` (a pose from the app's camera, photo 5's, photo 2's and photo 4's,
with the photographs' measures; `HAND_AZ`/`HAND_EL` turn the app's camera), `lids.py`,
`hand_sheet_v10.py <shoot_arcus.exe> <dump.json> <out> [--old-dump=<dump>] [--refs=<dir>]`
(the app's own picture round the hand from the earlier camera, the new one and photo 5's
angle; the whole picture from both cameras; `numbers.txt`).

**Later the same day (the owner's and the coordinator's reviews of the app sheets v1–v3)**:
the thumb's base capped at 30° (it had turned 65°: the "mitten", the owner's "parte abultada
gorda"); the wrist end of the forearm carrying 95 % of the hand's turn (was 70 %); her palm
drawn in across by 0.86 (`PALM_SLIM`), his hands 0.93 (`HAND_SCALE`) — both mirrored in
`hand_rig.py`; gentler joints (playing DIP 15–45°, hovering 5–35°), a stronger upright tip
(within 24° in the solve, gated at 30°), more hovering slack for the little finger; the
hand's skin warmed to the face's (`SKIN_WARM`); the sleeves' seam crack closed
(`close_cloth_seams` — it was the "tan stripe", not skin, not a stamp: shown with the stamps
off); a soft rim and lifted value on the live sleeves (`cloth_rim`); his shirt cuffs kept
under the sleeves; the bake's jacket shards (the bow arm's stretched shoulder band) and the
tie's sawtooth (`smooth_tie`) fixed, the suit 1.2 mm off the skin (`lift_suit`).
Sheets (outside the tree: they hold the photographs): `F:\projects\daw-r42\tmp\hand-app-v3\`
(`hand-*-vs-refs.png`, `full-*.png`), the offline previews `F:\projects\daw-r42\tmp\hand-preview\`.

**Gates**: `arcus_hand_table` — the photographs' gates per figure × instrument × string ×
note: the knuckle line on the E side (the index's base −0.5 … +2.2 cm from the edge, −1.0
high up the G/D strings where the hand comes round), the tip within 30° of the board's
normal, the back of the hand toward the camera (≥ 0.2) at a slant (width over length
0.45–0.95, 1.2 high up), the wrist within 8°, no finger bent sideways over 5°, no last joint
over its middle by more than 5°, the middle finger 0.95–1.35 of the back of the hand, the
palm's outline at its middle ≤ 1.35 × the wrist's crease (index/middle in low positions;
1.7 elsewhere); the eyes' setting test; `arcus_hold` (its V bound 85/88° and wrist 8° from
the photographs).

**Frame budget** (`arcus_hold_budget`, release, best of five): High realistic +1.9 to +3.1 ms
at 100 %, +1.9 to +4.0 at 200 %; Light +0.2 to +0.6; ceilings 4.0 / 4.5 ms (passes). The
patches were capped at 2 cuts a side (4 px) after a first measure of +6.7 ms at 200 %.
Clippy (`-p cantus-ui --all-targets -D warnings`) clean; `arcus_hand_table`, `arcus_hold`,
`arcus_scene`, `scene_identity`, `figure_contract`, `kit_lint` green.

**Open** (honest list for the next session):
1. The palm's outline is 1.4–1.63 × the wrist where the little or ring finger reaches or
   the hand is high (the photographs 1.1–1.3): gated at 1.7, not yet within the photos.
2. Tips meet the string at up to ~29° from the board's normal (the coordinator's ~25°).
3. From the new camera, under his bow hand, small grey-blue pointed shapes by the frog
   (not his elbow: the trunk-occlusion cull was tried and changed nothing, so it was taken
   out) — most likely the bow's frog/slide drawn for the earlier view's angles, or the
   inside of his sleeve's open end now that the cuff is hidden (darkening the "inside"
   faces by the forearm's axis darkened outside faces too and was reverted).
4. The coordinator's overall polish list (2026-11-25, the owner: "Agrega más detalles y
   pule en general todo más"): velvet sheen and folds on her dress with a visible elbow
   and contact shadows where an arm crosses the body; her shoulder-outline specks; the
   hair's strand variation, specular band, a twisted bun, value against both themes; the
   face's local variation, the closed lids' crease and lashes, chin/chinrest contact and
   the violin's shadow on the neck; key/fill/rim light and crease occlusion; the chair's
   grain or framing; the same for his suit, hair and skin — not started this session.
5. A pixel gate for silhouette strays on the full pictures at both cameras: not written
   (the full tiles were checked by eye; the causes found were fixed in the geometry).
6. The WSL Linux check (`tools/wsl/check.sh`) not run this session.


## arcus-polish (2026-11-25): the musicians' polish toward 100 %

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from r42/integration
a61af28dc. The owner's three polish lists of 2026-11-25 (above, arcus-hand's Open 4) and,
mid-session, the coordinator's reorder: **the left hand first** (the owner on sheet v10: "la
mano aún sale algo tosca o gorda"). ADR 0159 amended ("The polish").

**How to look at a figure while building it** (new): `build_figures.py <mh data> <assets>
<folder>` then `lids.py --dir=<folder>`, and run `shoot_arcus` with
`CANTUS_FIGURE_DIR=<folder>` — the app reads the `.fig` and the lids from there (no compile:
45 s a figure instead of ~10 min). The MakeHuman sources on this machine:
`C:\Users\luis1\AppData\Local\Temp\claude\mh-src\mh\makehuman\data` and
`C:\Users\luis1\AppData\Local\Temp\claude\mh-dl\sys` (a temp folder — copy them somewhere
durable before a clean-up; the converter reproduced the committed figures byte for byte).

**Done** (what changed and where; the measures):
1. *The left hand.* A render with each corner coloured by its bone (a debug switch during
   the session, taken out) showed the lobe from under the neck to the wrist is the **hand
   bone's skin** — the palm's pad and heel seen from below (the camera is 5° up) and its
   ulnar edge —, not the thumb (hidden by the neck, as in the photographs) and not High's
   PN patches (Light, no patches, the same outline). Fixed: the palm's own side drawn in
   toward the metacarpals offline (`build_figures.py::slim_hand`, `PALM_VOLAR` 0.45, the back
   0.9); at load `bowed_human::thumb_web` (mirrored `hand_rig.py::thumb_web`): the ulnar
   edge drawn in (`EDGE_IN` 0.5), the web past the index drawn in and 60 % handed to the
   thumb's first bone; the middle and the little fingers slimmer (`FINGER_EXTRA`); High's
   patches tone the back of the hand with tendons, metacarpal valleys and knuckles per point
   (`Relief`); the live skin's wrap narrower (0.45 → 0.25) and the camera-side fill halved,
   so the back of the hand is modelled, not flat; patch edges canonical (`curved_edges`:
   the dark crack dots on his hand). Gate `tests/arcus_hand_shape.rs` (the app's own arm
   through `views::inst::arcus::probe`): the lobe's share of the back of the hand (before
   0.21/0.23 her violin/viola, 0.21/0.18 him; after 0.17/0.17, 0.18/0.16; gate ≤ 0.19),
   the edge's sagitta ≤ 4 mm (the photographs ≤ ~3 mm against the fingerboard), breadth and
   thickness at the MCPs within an adult's, fingers under a ceiling.
2. *Her arms and shoulders* (`shape_arms`, `ARM_GIRTH`, `SHOULDER_SLOPE` 1.5, `SHOULDER_IN`
   0.5; the dress's puff 0.1, the sleeves' ease 0.18). `measure_arms.py`: her sleeve's
   upper arm 28.3 → 26.9 cm, elbow 25.6 → 23.2, forearm top 24.6 → 23.4; joints unchanged.
   Gate `tests/arcus_figure_shape.rs` (silhouette widths across each arm from the app's
   camera, cm at depth, a woman's and a man's in a jacket, and the taper).
3. *Neck and neckline*: tones (cooler, paler, the notch, under the jaw), relief (the
   sternocleidomastoids, collarbones, notch), the V edge's contact shadow and the velvet's
   edge roll (`figure::cloth`).
4. *The dress*: velvet rim at grazing angles and back light, waist folds
   (`cloth_folds` + `cloth::fold`), the live sleeves' folds in the elbow's crook, the arm's
   shadow on the trunk it crosses (`trunk_shadow`).
5. *Light*: a portrait's back light for a realistic figure (rim ×1.7 night, ×2.2 day).
6. *Ears*: tilted back 15°, warmer and redder (MakeHuman's ear-flap target as the mask),
   the conchae's occlusion traced at rest (`cavities`).
7. *Hair*: tonal streaks, shifted Kajiya–Kay lobes, her blonde toward ash and deeper (0.52),
   his combed sheen band.
8. *Faces*: **the pale vertical streak was the room's tall strip softboxes in the skin's
   mirror** (meant for the pianos' lacquer) — skin now mirrors no softbox and has a key
   sheen of its own (`px_shade::skin_sheen`); jaw cooler; his beard's shadow; cavities.
9. *His suit*: `Tex::Wool` (heather, twill, edge roll), the skin under a cloth's edge
   sunk (`cloth::sink`: the collar's sawtooth), the contact shadows; the sleeve hem's inside
   drawn in shade (the grey-blue points by his bow hand were the hem's inward faces lit by
   the sleeve's rim).
10. *Small*: lunulae, joint creases, faint veins; the chair's back rail slimmer.
11. *The stray-pixel gate* `tests/arcus_figure_specks.rs`: both figures, violin and viola,
    both cameras, both themes, both looks (32 shots); the subject's mask rasterised from the
    baked mesh, the live arms (probe) and the bow; strays (groups darker or lighter than
    every pixel touching them, on smooth surroundings) outside the grown mask (≤ 30 px) or on
    the figure's skin and clothes shrunk by 3 px (≤ 3 px) fail.

**Gates** (this session, release): `arcus_hold` 7/7, `arcus_scene`, `scene_identity`
(Chorda's and Tympanum's pictures unchanged), `figure_contract`, `kit_lint`,
`arcus_hand_shape` (new), `arcus_hand_table` (after keeping the wrist's crease out of the
palm's thinning — the first build pinched her wrist under the palm, the table's
palm-to-wrist rule caught it); `figure::cloth`'s unit tests. Two new gates are written
and **ignored with their reason** until the next session finishes them:
`arcus_figure_shape` (her left forearm finds no points in its slab from the probe's joints
— a measuring bug to find) and `arcus_figure_specks` (40 strays at this state: e.g. her
violin at (399, 324) in every look, (151, 287) and (418, 312) from the earlier camera, his
(319, 325) and (253, 334) — `CANTUS_SPECKS_DIR=<folder>` saves every shot to look; each is
a real speck to fix or a feature to tell apart). The frame budget and clippy: see the end of
this section.

**Open** (honest list for the next session, in the order the owner will see them):
1. *The hand's proportion* (the coordinator, after the first sheet): the lobe is smaller but
   the change reads modest at 200 %; what still reads "gordo" is the back of the hand facing
   the camera with fingers curled tight and low. Raise the MCPs (knuckles higher over the
   board), flex the PIPs less, arches higher and longer within the rules (tips ≤ 30°,
   parallel), and **re-solve the table rows for first position** (`left_hand.py solve`, its
   rules in `left_hand.py`; not done this session). Her middle and little fingers measured
   1.9 / 1.8 cm at the PIP before `FINGER_EXTRA`; tighten `arcus_hand_shape`'s finger
   ceilings to her ≤ 1.8 / 1.5, him ≤ 2.1 / 1.7 once re-measured on the new figures.
2. *His sleeve at the bow hand*: the grey-blue points by the frog persist from the new
   camera (the hem's inward faces rule did not catch them — `fig.edge` near his jacket's hem
   may not reach them; colour them per triangle by `fig.edge` in a debug render), and the
   room shows through the gap between his wrist and sleeve (the shirt's cuff is hidden;
   drawing it again broke into specks in arcus-hand). The coordinator saw a black ring and
   spike at his wrist on the first `polish-hand1` sheet: that was the first "inside" version
   (back faces near the edge), replaced by the inward-faces rule — re-check his viola too.
3. *Aliasing at 200 %*: the strings drawn as dashed white lines along the fingerboard and
   hard stair-steps at the fingertips and the strings' crossings — the live layer at 200 %
   (`Brush::segment` is coverage-correct for thin lines; look at how the strings and the
   neck's stamp are drawn), not investigated.
4. *The raking key on the back of the hand*: the live skin's wrap and camera fill were
   reduced; a key that rakes across the back so the tendon relief reads is not done.
5. *Hair*: the bun as twisted strands, flyaways and baby hairs at the hairline — not done.
6. *Faces*: the closed lids' crease and lash shadows not specifically drawn (the cavities'
   occlusion darkens the eyes' corners); the violin's shadow on the neck is the bake's own.
7. *His collar and tie*: thickness and the knot's folds not modelled (the collar's skin is
   sunk under it and shaded).
8. *Small*: the bow hand's fingers touching the stick, rosin dust, fabric seams — not done.
9. *Hand table item 11* (palm outline 1.4–1.63 × the wrist high up; tips ~29°) — not done.
10. *Arm widths*: `arcus_figure_shape` measures from the posed joints (the probe's); his
    bounds allow a suit's sleeve. The woman's deltoid/upper arm read 9–10 cm sleeved; the
    bow arm's shoulder still reads broad from the new camera (linear skinning at the raised
    shoulder) — a corrective shape or dual-quaternion skinning for the shoulder is the
    next lever.
11. The WSL Linux check (`tools/wsl/check.sh`) not run this session.

**The owner's review of sheet v11 (2026-11-25, through the coordinator) — the next
iteration, in this order:**
1. *His hair*: the sheen read as a grey, helmet-like sheet over his crown. After the sheet
   his lobes were lowered (`hair_look(1)`: shine 0.012, sheen 0.018) and his primary tinted
   by his hair (`px_shade::strands`) — **not yet looked at**; the band should be narrow,
   broken along the strands, warm; keep the dark roots and the breakup.
2. *Her shoulder*: jagged dark shards at the top of the bow-side shoulder (round the seam),
   and the velvet's sheen there faceted (polygonal) — smooth its normals or raise its
   tessellation (the live shoulder band and the bake's). The stray-pixel gate is the check.
3. *His bow arm*: the dark shard under the elbow beside the frog (before and after) and his
   left sleeve's jagged end at the wrist. Gate: the stray-pixel gate.
4. *Her hair* still yellow-flat at 100 %: roots and underlayer, tonal bands, the twisted bun
   (item 6) — the streaks and the ash tint alone did not carry it.
5. *The hands at 100 %*: dark speckles and jaggies on both left hands in the full tiles, the
   bow hands small and unclear — the antialiasing at 100 % as well as 200 %.

**Frame budget and clippy at the end of the session**: `arcus_hold_budget` (release, 
loaded machine) **failed**: High violinists +3.65 / +5.04 ms (her, 100 / 200 %), +4.51 / +5.77 
(him), over the 4.0 / 4.5 ceilings; his cello Light 200 % +1.00 at its 1.0 ceiling (numbers in 
ADR 0159). The trunk shadow's steps were halved afterwards, unmeasured. `arcus_hand_table` 
fails one rule ("Woman Viola string 3 +1: the palm 4.3 cm wide at its middle, the wrist 2.9"): 
the offline palm thinning (`build_figures.py::slim_hand`, `PALM_VOLAR`) narrows the wrist from 
the camera — handed to the arcus-hands builder, who now owns the hands. Clippy 
(`-p cantus-ui --all-targets -D warnings`): one finding fixed (`cloth::contact`'s loop), 
re-run at the session's end (see the final commit).

## arcus-polish2 (2026-11-25): the musicians' polish, pass 2

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from bffb9f682. The
owner: "the musicians can still look much, much better"; then "no te preocupes por los
tiempos, luego optimizamos o hacemos configurable" (the look first); on sheet v12 "ese brazo
aún se ve raro"; beside ref-04/06/10: a thinner neck, a camera "like the girl's", a concert
light. ADR 0159 amended ("The polish, pass 2", "Optimise later"). Sheets: v13
(`docs/round42/arcus-upper/sheet-v13-polish2/`, before the hold/neck/camera/light), and
beside photographs in `F:\projects\daw-r42\tmp\polish2-arms\arms1.png` (the hold) and
`F:\projects\daw-r42\tmp\polish2-light\concert-vs-studio.png` (all of it) — never committed.

**Commits for the hands' re-solve**: the hold 03888c909, the neck 01c32794e, the camera
d630eb474 (each its own commit); the Concert light 02e2400cb.

**How to tune without rebuilding** (a release build of cantus-ui took 20–37 minutes with three
builders on the machine): `CANTUS_HELD="tx,ty,tz,ux,uy,uz,dx,dy,dz"` (the realistic
violinists' hold), `CANTUS_ARM_POLE_BOW` / `CANTUS_ARM_POLE_LEFT="down,right,forward"` (the
left one is ignored for the realistic figures: their left elbow is the table's), read once a
process by `shoot_arcus` and the tests; `arcus_arm_pose -- --ignored --nocapture` prints the
arms' screen angles along the stroke.

**Done**:
1. *Frame budget*: profiled, then stopped by the owner's decision (BACKLOG §4b "The realistic
   figures' frame cost": the profile and the levers; the fine_tri cache is arcus-hands').
   Kept: the trunk's shadow from the arm's three pieces, the lighting table borrowed once a
   loop, the left arm's passes sharing their classes. `arcus_hold_budget`'s ceilings are the
   last measure plus a quarter. `arcus_figure_auto` (new): Light draws the coarse arms, Auto
   steps down on long frames and back.
2. *Shoulders*: the band keeps the bake's surface until the live arm is opaque
   (`BAND_TURN`, `ARM_COVERS` 0.38) — her bow-side shards gone in both looks; dual-quaternion
   live arms; the bow band's turned-away faces as the cloth's inside.
3. *Sleeves*: they follow the forearm (no twist with the wrist); the cuff's inside lit as the
   sleeve's own (no black ring); `cuff_shadow` on the wrist; over-stretched sleeve triangles
   outside the band left out (the left elbow's points by the frog: not yet confirmed gone).
4. *Antialiasing*: the live outline mitred (miters capped) and its strip softened toward the
   inner corner; strings even in both looks (`Geo::wires`, `WIRE_PX`, `TubeOpts::flat`).
5. *Hair*: hers along the strands (roots, underlayer, lighter lengths; the isotropic sections
   were tried and taken out), her loose hairs (`Role::Wisp`); his warm broken band and
   feathered hairline; Drawn: no pink hairline stripe, a deeper underlayer, strand lines.
6. *Details*: the tie knot, the suit's part lines smoothed, the shade under cloth edges and
   `Tex::Shirt`, the matte tie; the lids' crease, lash line and lash shadow; gold earrings;
   rosin dust; cloth edge rolls over a centimetre.
7. *The hold, the neck, the camera, the light* (ADR 0159): the scroll 3° under level with the
   end button pinned; the luthier's neck; the camera from chest height; Concert (the stage)
   the realistic figures' default, Studio an option (Settings ▸ Appearance ▸ Light).
8. *Gates*: `arcus_figure_shape` measures by the mesh's crossings and is on (passes);
   `arcus_figure_specks` is on (red, below); `arcus_arm_pose` (new) is written and ignored
   with its reason (Open 1); `figure_contract`, `arcus_figure_auto` pass.

**Gates at the end** (build 11 = 02e2400cb; the final cleanup commit only removes the timing
probes and debug tints and sets the budget's ceilings): `figure_contract` ✓,
`arcus_figure_auto` ✓, `arcus_figure_shape` ✓; `arcus_hold` 5/7 — "Violin Man 12 st: elbow
49°" (the left-hand table, to be re-solved on the new hold) and "Vector Cello Woman: the
figure shows over the instrument" (a live outline strip past the cello's neck stamp: the
miters are now capped — re-run); `arcus_hand_shape` "Woman violin: finger 3 0.28 cm" (the
hands' re-solve on the new neck and camera); `arcus_figure_specks` 47 strays with the new
composition — the dotted sleeve outlines (about 5), the Drawn stage's spot reflections on
the violin's top inside the arms' mask (about 4: the mask counts arm triangles the violin
hides — mask them by depth), the hands' creases (about 7, arcus-hands'), grey one-pixel dots
on her neck in Drawn; `arcus_hold_budget` not re-run after the last changes (the ceilings
were raised by decision); `arcus_scene`, `scene_identity`, `kit_lint` not run this session
(Chorda's and Tympanum's pictures should be untouched: every change is gated to the bowed
subject or the realistic figures); clippy: see the final commit.

**Open** (in the order the owner will see them):
1. *The bow hand's height*: at 0.3 of the bow the hand sits 1.6–2.5 upper arms over its elbow
   on screen (the photographs: level), over the shoulder nearer the frog — the strings stand
   higher over the bow shoulder than a player's. Next lever: the skeleton's shoulder and
   collarbone heights (`bowed_figure::skeleton`), then `arcus_arm_pose` on.
2. *The left elbow*: the left-hand table's (arcus-hands' re-solve on 03888c909/01c32794e/
   d630eb474).
3. *Concert's tuning*: Drawn's spot reflections on the violin read as blotches; Paper is a dim
   stage; the chair stool on a stage. Compare with ref-04/06/10 again after the re-solve.
4. *The stray-pixel gate* (above). His left cuff's jagged end may remain (hem ring geometry:
   a smoothed hem ring at load is the next lever).
5. *Collar thickness* (the collar is painted on the suit's surface: needs a modelled band)
   and *fabric seams* (`cloth::fold`'s seam code has no data) — not done.
6. The WSL Linux check (`tools/wsl/check.sh`) not run.

**The light against ref-04/06/10 — my view for the next builder** (the owner on the Concert
sheet: "aún le falta lo de la luz, compara con las imágenes reales"). Concert is `Room::Stage`
as it was built for the pianos; nothing in it was tuned for the figures. What it lacks:
1. *A directional key*: the photographs' key comes high and from the side (stage spots), so
   faces, necks and arms have a lit side and a shadow side with a soft terminator; ours lights
   the face nearly frontally — flat, pale. Move the key higher and to the figure's right
   (camera left), narrower, warmer; drop the fill to about a quarter.
2. *A rim/back light*: in every reference the hair, the shoulders and the violin's edge glow
   against the dark; ours barely separates them (the realistic rim ×1.7/2.2 is tuned for
   Studio). A back light from above-behind, strong on hair and velvet.
3. *Real cast shadows*: the violin's shadow on her neck and chest, the bow arm's on the body,
   the chin on the rest — the bake's shadow map should carry them under a hard-ish key; the
   live arms' trunk shadow is a soft band only.
4. *The hall*: the references show a dark warm hall with soft bokeh (other players, lights);
   ours is a flat dark curtain with one brown band and a stool — replace the stool and give
   the background a little warm, out-of-focus structure.
5. *The varnish*: deep red-amber with sharp specular streaks from the spots; ours reads
   orange and flat, and in Drawn the spots make white blotches on the top.
6. *Contrast*: deeper blacks with the velvet's and the wool's sheen kept; warmer skin
   (ours pale); Paper's Concert currently a dim grey stage.

## arcus-polish3 (2026-11-25): the concert light, his face, the clothes

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from 4ac9aa779. The
owner beside three concert photographs: "Aún le falta lo de la luz, compara con las imágenes
reales"; on iteration 1: "Va mejor, sigue mejorando la luz y la ropa; el hombre puede mejorar
mucho sus rasgos, hazlos más masculinos, y toma nota de los detalles de la ropa". ADR 0159
amended ("The polish, pass 3: the concert light").

**Sheets** (never committed, the photographs beside): iteration 1
`F:\projects\daw-r42\tmp\polish3\a10\sheet.png`, iteration 2 `…\polish3\m4\sheet.png` and his
face `…\polish3\man_face_200.png`; the final ones are listed in the session's report.
`tools/arcus-figures/light_sheet.py <before> <after> <refs> <out>` makes them (both figures,
Night Photographic and Drawn, Paper as the whole window, the photographs beside, the
measures on each tile); `light_measure.py refs <dir>` / `shot <png> <who>` the numbers.

**How to tune without rebuilding**: `CANTUS_CONCERT="key_az=-40,exposure=1.2,…"` — every
number of `figure::concert::Concert` (the key's place, size, light, warmth and pool; the fill;
the two back lights; skin, hair and cloth; the curve, grade and vignette; the hall, its
orchestra's wash, its bokeh and blur; the varnish; the Drawn stage's `d_*`). Arrays as `a/b/c`.
`CANTUS_SHRUG="left,right"` the shoulders' raise (cm). His face: `build_figures.py … --only=man`
into a folder and `CANTUS_FIGURE_DIR` (the skin texture, `man_skin.png`, is compiled in: the
stubble needs a build).

**Done**:
1. *The concert light* (`scene/figure/concert.rs`, `chorda_scene_hybrid::concert_rig`, the
   shade's `cc` branches, `raster`'s concert curve and blur, `arcus_draw_flesh::concert_colour`
   for the live arms): a small high stage spot from the audience's side, a weak fill, two
   back lights (rims, and a line along black cloth's edge, `cloth_edge`), the cloth's sheen
   along its folds (`cloth_spec`), skin's narrower wrap and warm grade, a red-amber varnish,
   its own filmic S-curve. Measured against ref-04/06/10 (below).
2. *The hall* (`chorda_scene_px_hall.rs`, `scene/hall.rs`): the wood wall in pools, the
   orchestra on four risers (half in white shirts), stands and their lamps, the stage floor's
   warm pools off to the sides, the hall's lights as bokeh; Drawn in soft shapes. Chorda's and
   Tympanum's stage untouched. The black concert chair (`bowed_props::concert_chair`).
3. *Drawn*: `Light::concert` (a softer terminator, the back lights' line `Light::rim`, one soft
   varnish highlight and a deeper amber, no strip-light blotches), `stage_skin`.
4. *The bow arm*: `PlayRig::shrug` / `bowed_human::SHRUG` (6 cm, the bow side): the hand at the
   frog 0.63–0.78 upper arms over the elbow (was 1.65–2.46); `arcus_arm_pose` split — the bow
   arm's test on, the left arm's ignored for arcus-hands' table re-solve.
5. *The neck heel* (da0ad2fc2, its own commit for arcus-hands): the depth kept to 1.8 cm from
   the body.
6. *His face* (`build_figures.py` FACE["male"], `beard_field`, `uv_field`, the stubble in
   `skin_texture`, the Adam's apple in `relief`; his hair shorter at the sides; his hairline
   broken by fine tufts, `cards::scalp_cover`/`feather`); `man_skin.png` now 2048² (0.8 MB).
7. *The sleeve as cloth* (`arcus_draw_flesh::sleeve_drape`): the shoulder's pull lines as the
   arm rises, the cuff's slack — driven by the pose.
8. *The light-measure gate* (`tests/arcus_light_measure.rs`, the bands from the photographs,
   the dark clothes' silhouette below the face included).

**Concert clothing, studied** (the owner, 2026-11-25: "toma nota de los detalles de la
ropa"; ref-04, ref-06, ref-10 and the rest of `refs-violin`, study only). What real concert
clothing shows, against what the figures have:

| detail | the photographs | ours | |
|---|---|---|---|
| The elbow crook's diagonal folds and the inner elbow's compression folds | every bent sleeve (ref-06 both arms) | `elbow_folds`: normals swung and creases shaded round the crook, as deep as the elbow bends | have |
| The shoulder's pull lines when the bow arm is raised | ref-06, ref-10: long diagonals from the shoulder's top toward the armpit | `sleeve_drape` (polish3): a few diagonal folds on the upper arm's first third, as deep as the arm is raised | new |
| Sleeves as fabric with slack, not tubes | the cuff end gathers over the wrist (ref-06's shirt, ref-10's jacket) | `sleeve_drape`: the cuff's slack, soft ripples over the last 7 cm | new (normals only: the silhouette is still the mesh's) |
| Sheen along the folds under a spot | ref-06's black shirt: a pale line on every crest | `cloth_spec` (wool and velvet, the key's broad lobe) and `cloth_edge` (the back lights' line along the edge), Photographic and the live arms | new |
| Seams: the shoulder seam, the sleeve's set-in seam, the side seams | ref-10's jacket: the set-in seam reads as a soft ridge | none (`cloth::fold` has no seam data) | lack |
| A shirt's button line and placket; the jacket's buttons, lapel roll and pocket flaps | ref-06 (placket and buttons), ref-10 (lapel roll, one button) | the lapel's rolled edge (`EDGE_ROLL_CM`), the shade under the lapels; no buttons, no placket, no pockets | lack |
| The collar's thickness and roll; the tie's knot dimple and drape | ref-10 (open collar), formal photographs (knot dimple) | the collar painted on the suit's surface with the shade under its points; a four-in-hand knot without a dimple | partial |
| Cuffs with an edge and a thickness | ref-06's rolled cuffs, ref-10's shirt cuff past the jacket's | the sleeve's end shading the wrist (`cuff_shadow`), the shirt's cuff a band of `Shirt`; no thickness | partial |
| Fabric texture and sheen: wool's weave, the shirt's sateen along folds, velvet's pile | everywhere | wool's heather and twill (`Tex::Wool`), velvet's pile and back light (`Tex::Velvet`), poplin's unevenness (`Tex::Shirt`); no sateen sheen on the shirt | mostly |
| Her dress: the neckline's edge, its waist, the skirt's fall | concert gowns: a finished neckline edge, a fitted waist, the skirt's long folds | the neckline's edge roll, the waist's gathered folds (`WAIST_FOLDS`), the long skirt (`bowed_human_skirt`) | have |

Next, by impact: (1) the jacket's buttons and the shirt's placket and buttons as small modelled
parts on the figure (the man's chest reads flat without them); (2) the collar as a modelled band
with a thickness and the tie's dimple; (3) the cuffs as short tubes with an edge; (4) the
set-in sleeve seam and the shoulder seam as a soft ridge in the figure's mesh.

**Gates at the end** (build 6 = 9f0eaafcd's tree, release): `arcus_light_measure` ✓ (her face
0.36, his 0.25; R/B 3.6 / 4.1; sides 2.5 / 2.3; face over the hall 20 / 14; contrast 2054 /
2425; the silhouette and the edges far over their floors), `kit_lint` ✓, `figure_contract` ✓,
`arcus_scene` ✓, `scene_identity` ✓, `arcus_figure_auto` ✓, `arcus_arm_pose` ✓ (the bow arm;
the left arm's test ignored for arcus-hands), clippy `-D warnings` on cantus-ui ✓.
`arcus_figure_shape` ✗ — her bow arm's deltoid 11.4 cm against 7.5–10.5; not pass 3's: the
polish2 binary of 19:54 and `CANTUS_SHRUG=0,0` give the same 11.5 (decide on realism: if the
photographs say a velvet sleeve's cap is 11.4 cm across there, re-baseline with the measure in
the commit, else fix the band's shape). `arcus_hold` ✗ on arcus-hands' two (the left table's
"Violin Man 12 st: elbow 49°", the Vector Cello Woman strip). `arcus_figure_specks` ✗ 62, all
on the figure, none in the room (the hall is clean): most round the bow hand in the earlier
camera's Photographic shots (arcus-hands'); mine: light dots at her right shoulder's edge
(Woman Viola camera (115, 273)), a speck on the velvet ((335, 355)), the Drawn neck dot
((192, 277)). `arcus_hold_budget` ✗ by p95 on Light (+1.4–1.75 ms, the man and her cello) with
three builds on the machine; High's violinists +9–10 ms at 200 % (ceiling 11): re-measure idle
(the owner: cost does not gate the look). The neck heel (da0ad2fc2) is not yet seen from the
side against a luthier's profile.

**Open — the coordinator's list after iteration 3** (in this order):
1. *The Drawn violin*: the deeper amber reads as flat saturated orange; the photographs'
   varnish is red-brown with darker flames and a specular ridge along the arching — less
   saturation, value variation across the plates (`Light::shade`'s concert lacquer branch
   multiplies the colour by `[1.0, 0.8, 0.62]`: start there).
2. *His hair*: still a helmet with a hard hairline (and in Drawn a bright rim and white flecks
   on the crown). A new hair proxy with volume and strand direction (the top swept back,
   longer on top than the sides as ref-06), a soft broken edge at the temples and the nape
   (MakeHuman's CC0 `short01/03/04` are in the system assets pack; only `short02` is
   extracted on this machine).
3. *Her skin*: the face, neck and neckline too uniform and pale-pink beside ref-04's warm skin
   — warmth and the redness under the skin at the ears, the nose and the knuckles, the
   clavicles' and the sternum's form (`relief`'s `RELIEF["female"]`), less matte uniformity;
   her bun reads as a carved shell — strands and flyaways at its outline.
4. *The clothes* (the owner asked twice): the study's lacking items — real folds at the elbow
   crook and the armpit in the geometry, his lapel's edge, buttons and placket, a modelled
   collar, the tie's knot and dimple, the cuffs' thickness, the seams; her sleeve's end must
   not lose itself into the body.
5. *Specks*: the three above that are mine and the flecks in his hair.
6. `arcus_figure_shape`'s deltoid (above), the budget's idle re-measure, the WSL Linux check.

## arcus-polish4 (2026-11-26): toward concert photographs

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from 4f9e99efe. The
integrator's review of pass 3's sheets, in order: the Drawn varnish, his hair, her skin, the
clothing, the specks, the deltoid gate. ADR 0159 amended ("The polish, pass 4"). The
left hand and `arcus_hold`'s left-arm items are arcus-hands' (not touched here; the hand
geometry code in `build_figures.py` is unchanged).

**How to look without rebuilding** (new): `CANTUS_FIGURE_DIR` now serves the cards' and the
skin's textures too (`<who>_hair.png`, `_brow`, `_lash`, `_skin`), not only the `.fig` and
the lids — a new hair proxy is looked at in `shoot_arcus` without a compile. The Drawn
varnish and skin are `Concert` numbers (`d_varnish`, `d_flame`, `d_edge`, `d_ridge`,
`d_ridge_pow`, `d_plate`, `d_mottle`, `d_skin`), tunable by `CANTUS_CONCERT`. The MakeHuman
system assets on this machine now also hold `hair/short01`, `short03`, `short04`
(`C:\Users\luis1\AppData\Local\Temp\claude\mh-dl\sys`, extracted from `sys.zip`).

**Done** (where; the measures):
1. *The Drawn varnish* (`Light::shade`'s concert branch; `bowed_draw::plate_rows`): red-brown
   (`d_varnish` 0.92/0.42/0.32 over the wood), the flames twice as strong (`d_flame` 2.0, and
   0.2 of the flat colour, was 0.45), deeper where the arching turns from the eye (`d_edge`
   0.45), a specular ridge (`d_ridge` 0.4, `d_ridge_pow` 28), the plates darker toward
   their edges (`d_plate` 0.35) with a slow mottle of wear (`d_mottle` 0.12).
2. *His hair*: MakeHuman's CC0 `short04` (`HIS_HAIR`; `short02` before), its cards lifted into
   a combed-back top (`his_volume`, `HIS_VOLUME` 1.7 cm at the front of the crown) and
   feathered away below `HIS_TOP_LINE` (the sides and the nape short: the scalp there, its
   colour drawn toward the skin's near the hairline, `cards::side_cover`); `HAIR_DARK`
   (46, 35, 29) for his cards, scalp and Drawn shell, `HairLook` detail 0.4 (higher, the
   crossing card layers read as a weave at 200 %); loose tips at the top's outline
   (`his_tips`, `Role::WispDark`, Photographic). Drawn: the shell's top with the volume
   (`thick`), the sides short and deeper (`blend`, `hair_at(.., short)`: roots with more of
   the skin, broader value bands along the combing), its temples' edge cut into tufts
   (`hair_shell(.., wave)`), sorted 0.6 cm nearer than the skin (`SHELL_BIAS`: the skin's
   triangles had painted the light flecks through it), its rim 0.25.
3. *Her skin*: `skin_gain` 1.0/0.92/1.04, `skin_sss` 0.5, no wrap or camera fill (her face
   R/B 4.2 → 3.8, its sides 2.0 → 2.2; ref-04 2.9 and 3.3; his R/B 4.4, sides 2.8); the
   Drawn stage skin's hue `d_skin` 1.0/0.92/0.88 (its value kept); `tones`: her neck no
   longer cooled, a flush over the breastbone, a mottle over the neck and chest, the ears
   redder (0.48); the nose's tip warmer; the knuckles' and joints' red ×2 (colour only);
   `RELIEF` collarbones 0.4/0.3, notch 0.6, `STERNUM` (0.22 cm deep, 1.3 wide, 7 long).
   Her bun a rope of two twisted locks (`torus(.., rope=0.12, turns=3, dense=9)`) and up to
   12 loose hairs at its outline (`BUN_WISPS`).
4. *Clothing*: the sleeves subdivided once (`subdivide`, a red-green split so nothing
   cracks; cloth is no longer thinned) and creased (`sleeve_wrinkles`: slanted rings cut in
   round the elbow's crook, 0.45 cm deep, 2.6 cm apart, deepest on the crook's side; drag
   folds from the armpit) — the creases cut in only, so the silhouette stays the arm's;
   finer, his sleeves keep the suit's own three cuff buttons (the thinning had flattened
   them; the front's two buttons and the pocket flaps were always the suit's); edges with a
   thickness (`hem`): her cuffs (0.3 cm + 0.12 flare: her sleeve's end no longer thins onto
   the skin), his jacket's edge over the shirt (the lapels, the sleeves' hems over the
   shirt's cuffs, `HEM_JACKET`) and the shirt collar's edge over the jacket (`HEM_COLLAR`);
   the tie's blade under the knot with its dimple (`tie_knot`, `DIMPLE`) and a neater knot
   (the wrap's fold faint, the upper corners going in under the collar's points); the
   cloth's sheen duller (`velvet_spec` 0.06, `wool_spec` 0.035, `cloth_rim` 0.28); in Drawn
   his shirt and tie sorted 0.8 cm farther (the collar's far point drew white shards over
   the violin). The bodice smoothed further over the bust (`BUST_SMOOTH`: a small crescent
   of its form showed in the Drawn sheen — modesty).
5. *Specks*: on cloth the live outline strip takes its triangle's inner corner's colour
   (was halfway: the dotted light line on her bow shoulder) and none is drawn on a cloth
   contour edge under 1.6 px (`SMALL_EDGE_PX`: the tops of his cuff buttons each drew a
   lone light dot); his sleeves smoothed after they are made finer (`soften`, `SOFTEN` 10:
   the suit's cuff buttons rounder — their sharp tops each caught the back light on one
   pixel).
6. *The tie, modelled whole* (`bowed_human::tie_knot`; the coordinator twice: a navy blob
   with white flecks): the knot a smooth tapered trapezoid (1.9 → 3.0 cm, 3 cm tall, its
   wrap a soft crest, not a crease) and the blade following the shirt down from the knot's
   foot (the suit's tie vertices' middle a centimetre at a time), widening 3.6 → 7.6 cm,
   with one dimple under the knot and a pointed end, never narrower than the suit's painted
   tie under it (up to 1.2 × its own shape, `REACH_MOST`); the painted tie region becomes
   jacket cloth under it (its staircase edges and the navy fragments at the collar are gone).
   The shirt's points inside a held violin's or viola's body (and the jacket's cloth over
   its top: the painted tie flared there too) are pressed flat and tucked under its back
   (`clear_of_the_body`, `TUCK` 0.4 cm): the collar's far point stood through the lower
   bout — the white wedge in Photographic, the shards in Drawn; culled (an earlier build)
   the collar was half a band. The knot by value (the coordinator, at 400 % a navy smear):
   `hp::TIE` (40, 50, 96) with silk's faint sheen; the knot's top face lit, its underside
   in shade, a darker rim, its cast shadow and a crease on the blade under it.
7. *The deltoid, decided by realism* — the geometry was wrong, not the band: her bow-arm
   cap measured 11.4/11.6 cm (violin/viola) against her left arm's 9.5 (the same cap at
   rest) and ref-04's raised bare arm, 8–9 cm against her bow hand's breadth (~7.5 cm): the
   rigid turn of skinning bulged the lifted cap. `arcus_draw_flesh::Deltoid` (a pose-space
   corrective, the bow arm only: up to 0.3 of the radial distance over the upper arm's first
   quarter, as the arm lifts 25° → 65° from hanging): 10.3 / 10.4. `Elbow` (both arms: the
   bent joint's dual-quaternion swelling, which the finer sleeves revealed — 9.5–9.8 cm —
   drawn in up to 0.16 as it bends 30° → 100°). `CANTUS_DELTOID` / `CANTUS_ELBOW` tune both
   while measuring.
8. *Her V neckline's point higher* (`V_DEPTH` 11 cm, was 15): the bow forearm crosses her
   chest, and the V's point showed under it as a skin diamond that read as the arm through
   the sleeve (the coordinator, on the hands builder's bow-hand sheet); modesty too. The new
   test `arcus_figure_specks::no_skin_shows_through_or_under_her_bow_sleeve` (her bow arm at
   the frog, the middle and the tip, violin and viola, both looks): no skin-coloured pixel
   inside the live sleeve's outline (drawn in by 2 px; the hand and the bow left out) and at
   most two in a 12 px band under it, shot at 200 % (`SLEEVE_PPP`).

**Gates at the end** (build 14 of this session, release, the committed figures):
`arcus_light_measure` ✓, `kit_lint` ✓, `figure_contract` ✓, `arcus_scene` ✓,
`scene_identity` ✓ (Chorda's and Tympanum's pictures unchanged), `arcus_figure_auto` ✓,
`arcus_arm_pose` ✓ (the bow arm; the left arm's ignored for arcus-hands), the new
`no_skin_shows_through_or_under_her_bow_sleeve` ✓; clippy `-D warnings` on cantus-ui ✓
(on 271600266). Build 15 (271600266): the same, `arcus_figure_specks` 200 strays. `arcus_figure_shape` ✗ on its taper rule only — "Woman violin/viola arm
1: no elbow": her bent bow elbow 9.2 cm across the crook against the upper arm's middle
8.3/8.6 (the deltoid 10.3/10.4 and every width in its band; the viola's failed the same rule
before this pass, 9.8 against 8.7). `arcus_figure_specks`' stray count ~250: the largest
group is a row of single light pixels along the tops of his three cuff buttons (now kept,
see Done 4) in every shot of his bow forearm — not the outline strip (left out on tiny
edges, no change) but the buttons' own shading; then the bow hand's (arcus-hands') and his
bow shoulder's edge; her Drawn neck dot (192, 277) is not the earrings (tested without them)
— most likely a decal of the violin (a grain line or the purfling) sorted over the skin
beside the lower bout. `arcus_hold_budget` not re-measured: the machine was never idle
(three builders' builds the whole session; a build took 35–70 minutes).

**Open** (in the order the owner will see them):
1. *The collar band on the violin's side* (the coordinator's review of iteration 4, first):
   the far collar must stay a continuous band round the neck, pressed flat and its point
   tucked under the lower bout / chin rest. `clear_of_the_body` tucks every shirt point
   inside the body's outline up to 4 cm over its arching — and that slab still takes the
   visible part of the far band beside the knot (the neck's skin meets the tie on the
   violin's side in `sheet-it5`). Next: tuck only the point (the shirt points farther than
   ~2 cm from the neck's surface, or under the arching's top + 1 cm), keep the band, and
   drape the tucked part along the back rather than moving it straight under (a fold at the
   edge, not a stretched wall). The knot: by value now (its top lit, the underside and the
   blade under it in its shade, a darker rim, the crease and dimple; `hp::TIE` a richer
   navy with silk's sheen) — the coordinator asked to see it at 400 % under the Concert key
   once the collar reads; the near collar point lying over the knot's corner is not
   modelled.
2. *His Photographic hair*: the crossing card layers still read as a fine weave at 200 %
   (detail 0.3); the top's outline still nearly a smooth cap — the tips (`his_tips`) barely
   read against the dark room. A groomed hair texture of our own (strands one way, no
   crossing layers) is the next lever.
3. *The specks gate* (above): give the cuff buttons a broader, softer highlight (their
   tops' normals bent toward the sleeve's) or tell the gate a button from a stray; find the
   Drawn neck dot (capture the Drawn primitives at that pixel).
4. *The taper rule on a bent bow elbow* (above): decide by the photographs whether a cut
   square to the upper arm through a bent crook may be wider than the upper arm's middle
   (it is in ref-06's bent sleeves), or draw the crook in further (`ELBOW` up to 0.45 makes
   it pass, but pinches it).
5. *The new sleeve test*: it holds "no skin inside the bow sleeve" at the frog, middle and
   tip; run against the old figure (V 15 cm) its 12 px band under the sleeve did not flag
   the neckline's point at the editor's 100 % — widen the band or shoot at 200 %.
6. *Seams*: the set-in sleeve seam and the shoulder seam (`cloth::fold`'s seam code reads
   `[20 + depth, across]` on velvet; nothing writes it yet, and wool does not read it).
7. `arcus_hold_budget` on an idle machine; the WSL Linux check (`tools/wsl/check.sh`).

**Sheets** (never committed; the photographs beside): `F:\projects\daw-r42\tmp\polish4\`
`sheet-it1\`, `sheet-it2\`, `sheet-it3\`, `sheet-it4\` (the last: `sheet-100.png`, the 200 %
crops, `knot-collar-400-base-vs-now.png`, `her-bow-forearm-v-neck-200.png`). The crop and
sheet scripts were session tools (the scratchpad), not committed: `light_sheet.py` makes the
whole-picture sheet.

## arcus-polish5 (2026-11-26): the collar, the specks, the measures

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from a1ecf5033. The
integrator's list after sheet-it5, in order. ADR 0159 amended ("The polish, pass 5"). The
left hand and the hand geometry code in `build_figures.py` are arcus-hands' (untouched).

**How to look without rebuilding** (new): `CANTUS_TUNE="name=value,…"` (`scene::figure::tune`)
sets any number under study for a process — `collar_keep`, `collar_ramp`, `collar_top`,
`collar_tuck`, `collar_drape`, `collar_rib`, `collar_log` (prints the neck's radius and the
corners tucked); `knot_w1`, `knot_lap`, `knot_lap_u0/u1/v0`, `knot_lap_gap`; `despike_g0/g1`,
`despike_passes`; `small_part`, `strip_small_px`; `tips_n/p/len/lift/r`; `his_albedo`,
`his_shine`, `his_sheen`, `his_detail`, `his_streaks`, `his_sections`, `his_under`,
`his_rim`, `his_broken`; `drawn_skip` (a bit each: 1 the Drawn decals, 2 patches, 4 the
figures' triangles, 8 polygons, 16 rods — to find what drew a stray pixel). `shoot_arcus
--scales=2 --ppp=2` shoots the window at 400 %. `build_figures.py`: `CANTUS_SEAM=h,half,lean,
groove` (the jacket's seams).

**Done** (where; the measures):
1. *The far collar band* (`bowed_human::clear_of_the_body`): the band round the neck stays
   whole; only the point goes under a held violin's or viola's back. A shirt corner inside
   the body's outline is tucked by how far it lies past the neck's surface (the neck's axis
   posed, its radius the middle distance of its skin — 5.7 cm on him): within `COLLAR_KEEP`
   (3 cm) kept, tucked fully `COLLAR_RAMP` (0.5) further; the instrument claims corners
   only up to `COLLAR_TOP` (2 cm, was 4) over its top's highest point. Studied with
   `CANTUS_TUNE` (keep 2.2 → still half a collar; 4–7 → the point lay over the varnish as a
   wedge; top 0.5–2 against 4). In Drawn the band's corners over the outline are marked
   (`BESIDE_BODY` in their `uv`) and sorted 1.5 cm farther (`bowed_draw_tris`,
   `BESIDE_BIAS`) — sorted by their own middles they drew white stripes between the chin
   rest's cells, sorted farther they uncovered dark shards by the knot — and by default
   (`drawn_beside_mode` 0) left out: the Drawn knot is clean, but its far band goes under
   the chin rest as before (the neck meets the tie on that side in Drawn; Open 0b). The
   knot's top is 3.6 cm (was 3.0). The collar's points lifted onto the knot (`KNOT_LAP`) are
   off by default (`KNOT_LAP_ON` 0, `knot_lap=1` to study): single corners lifted left a
   white curl over the knot's top-left at 400 % (the coordinator on sheet6).
2. *The sleeve test* (`no_skin_shows_through_or_under_her_bow_sleeve`): it could never fail
   — each shot after the first drew the live arms over an empty picture (a run of frames
   keeps only the textures it uploaded; the baked body is uploaded once per scene), so no
   neckline was ever under the sleeve. Now a new scene each shot, three frames, the bow
   also at the figure's start (0.2, 0.3, 0.4), the band under the forearm's sleeve only,
   and only skin the forearm cuts off counts (a patch under `SLEEVE_PATCH`, 2500 px at
   100 %: the old V's point ~955; her neckline running on up past the arm ~15 000).
   `CANTUS_SLEEVE_DIR` saves the shots. **It fails on the old figure (V 15 cm) — and on the
   new one too**: at the figure's start the new V's point still shows under the forearm,
   smaller (a patch of ~240–750 px at 100 %, 0.2–0.3 of the bow). `V_DEPTH` below 11 does
   not move it (8, 6.5 and 4 gave the same patch: the point there is set by something else
   in `dress` — the bodice's smoothing or the cut's line). Not ignored: the figure needs the
   fix (the neckline's point higher or covered at the start), not the test.
3. *Specks*: his cuff buttons' rings are not the live outline's strips (`strip_small_px`
   and a no-outline flag for small separate pieces changed nothing), not the shirt's cuff
   through a crack (the cuff made wool: the same), not the corners' normals (`despike` at any
   strength: the same) — a pixel-wide light ring at 400 %; still open. `despike` kept (a
   sleeve corner grazing the eye more than its neighbourhood takes its broad normal). Her
   Drawn neck dot (192, 277) is the figure's own triangles in the Drawn picture
   (`drawn_skip=4` removes it; the violin's decals, patches, polygons and rods do not; not
   the tailgut either — tried and reverted): still open.
4. *The taper rule on a bent elbow* (`tests/arcus_figure_shape.rs`): decided by ref-04 —
   her bare bow arm bent about 75°, cut square to the upper arm at the elbow, runs along the
   forearm's start out to the measure's 6 cm: 1.07–1.12 × the upper arm's middle (the
   figure's bow elbow 1.07–1.11, bent 75–85°). Past 45° of bend the elbow may be up to 1.2 ×
   the middle (`BENT`, `BENT_MOST`), straighter it must stay under it; the report prints
   each arm's bend. Re-baselined, no geometry pinched.
5. *His Photographic hair*: the weave was the tonal streaks' fine noise over the crossing
   cards, not the maps (the maps' detail flattened, coverage hardened, the detail smoothed
   along the strands: the weave stayed) — `his_streaks` 0.9 → 0.15, `his_broken` 0.7 →
   0.2, `his_detail` 0.3 → 0.2; the top's outline broken by more, longer, thicker tips
   (`HIS_TIPS` 90 → 250, `TIPS`: 0.35 of the crown's corners, × 1.2 long, × 2 thick).
6. *Seams* (`build_figures.py::seams`, his jacket only): a soft ridge round each armhole
   (where the arm's share of the weights crosses one half) and along each shoulder's top,
   0.22 cm (`SEAM`; 0.4 a lump in the outline, 0.13 invisible). Her velvet has none: on her
   raised shoulder the ridge rippled the outline. `man.fig` regenerated (0aa2d8f1e).

**Open** (in the order the owner will see them):
0b. *The far collar band in Drawn* (the coordinator on sheet6, next): stop sorting it among
   the chin rest's cells — draw its visible part (from the knot to where the chin rest
   covers it) as its own layer after the chin rest's cells, clipped to the instrument's
   silhouette, or shorten the chin rest's claim to its own footprint. The marked triangles
   (`BESIDE_BODY` in a shirt corner's `uv`) are the band's; `drawn_beside_mode=1` sorts them
   instead of leaving them out. Send the next 400 % pair when both styles read as a whole
   collar with a clean knot.
0c. *His hair*: tone down the few bright single-strand streaks on the crown (the tips lying
   on the hair catch the back light — they read as scratches); `tips_*` and `his_*` tune
   them without a build.
0. *Her V's point under the bow forearm at the figure's start* (the sleeve test, above,
   fails on it): find what sets the point's height in `dress` now that `V_DEPTH` no longer
   moves it, then raise it clear of the forearm at 0.2–0.3 of the bow (or close the V there).
1. *His cuff buttons' light rings* (Photographic only, the specks gate's largest group):
   `CANTUS_CONCERT=cloth_rim=0` removes them — they are the concert's back-light line along
   black cloth's edge (`cloth_edge`, the live arm's `concert_colour` and the bake's
   `px_shade::cloth_edge`) on each button's steep rim; ruled out: the live outline's strips
   (`strip_small_px`), the shirt's cuff through a crack, the corners' normals (`despike`).
   Next: weaken the line where the normal turns fast over a pixel (a button's rim, a
   crease's lip — his bow shoulder's dots are the same line on a fold), keeping it on the
   arm's true silhouette.
2. *Her Drawn neck dot* (192, 277): the figure's own triangles in the Drawn picture
   (`CANTUS_TUNE=drawn_skip=4` removes it; the violin's decals, patches, polygons and rods
   do not). A sliver triangle's gradient on her neck, most likely; capture the triangles
   covering that pixel next.
3. *His hair* at 400 %: the tips read as fine straight strokes over the crown where they lie
   on the hair (not only at the outline); a groomed hair texture of our own is still the
   larger lever.
4. *Seams*: his jacket's only (a soft ridge); her velvet none.
5. `arcus_hold_budget` on an idle machine (never idle this session: three to four builds
   at once, a release build 31–55 minutes); the WSL Linux check.

**Gates** (build 5, release, the committed figures; build 6 = the same plus Drawn's leave-out, the sleeve test's patch rule; the last commit's two defaults — the lap off, the ramp 0.5 — looked at through `CANTUS_TUNE` on build 6, not rebuilt):
`arcus_light_measure` ✓, `kit_lint` ✓, `figure_contract` ✓, `arcus_scene` ✓,
`scene_identity` ✓ (Chorda's and Tympanum's pictures unchanged), `arcus_figure_auto` ✓,
`arcus_arm_pose` ✓ (the bow arm; the left arm's ignored for arcus-hands),
`arcus_figure_shape` ✓ (the bend-aware taper rule; it failed before this pass). The sleeve
test ✗ on purpose (above: the new figure's V point at the start). `arcus_figure_specks` ✗
185 strays (was 200), none in the room, 115 of them from the earlier camera (the bow hand's,
arcus-hands', and the varnish's rosin dots); mine: the cuff buttons' rings, his bow
shoulder's fold-lip dots (both the concert's `cloth_rim` line, Open 1), her Drawn neck dot
(Open 2).

**Sheets** (never committed; the photographs not beside — study only):
`F:\projects\daw-r42\tmp\polish5\sheet7\` (the committed defaults; `sheet6\` the lap on; `sheet5\` the hair) — `collar-100.png` (before | after, Photographic
over Drawn, the picture at 100 %), `collar-400.png` (the knot and both collar points at
400 %), `his-hair-200.png` (before | after at 200 %). Before = this branch's binary with
`CANTUS_TUNE` set to pass 4's numbers. The sheet script was a session tool (the
scratchpad): `shoot_arcus --hold=man --only=violin-play-+-night --scales=1` and
`--scales=2 --ppp=2`, the picture cropped at (8, 60)–(600, 490) and the knot at
(158, 192)–(262, 282) of the 100 % window.

## arcus-polish6 (2026-11-26/27): the neckline, the edge line, the far band

Branch `r42/arcus-polish` (worktree `F:\projects\daw-r42\arcus-polish`), from 89243a276. The
coordinator's list after sheet7, in order, with the coordinator's rulings mid-pass. ADR 0159
amended ("The polish, pass 6"). The left hand and the hand geometry code in
`build_figures.py` are arcus-hands' (untouched).

**The MakeHuman inputs** (new, and why pass 5's V trials were void): the system assets in
`C:\Users\luis1\AppData\Local\Temp\claude\mh-dl\sys` had lost their `skins` and `clothes`
(a temp folder); `build_figures.py` failed halfway, left its out folder empty, and
`CANTUS_FIGURE_DIR` read the embedded figure in silence — so `V_DEPTH` 8, 6.5 and 4 "gave
the same patch". The assets the converter reads are now extracted durably in
`F:\projects\daw-r42\tmp\mh-sys` (from `mh-dl\sys.zip`: `skins/young_caucasian_*`,
`clothes/shoes04`, `clothes/male_elegantsuit01`, `eyes`, `eyebrows`, `eyelashes`, `hair`,
`packs`); the data folder is still `C:\Users\luis1\AppData\Local\Temp\claude\mh-src\mh\makehuman\data`
(copy it somewhere durable before a clean-up). `build_figures.py` checks every input before
any work and exits naming what is missing (`NEEDED`, `check_inputs`); `CANTUS_FIGURE_DIR`
says on stderr when a file is not in its folder and the embedded one is read. With the
committed inputs the converter reproduces `woman.fig` and her lids byte for byte (V 11).
`CANTUS_V_DEPTH` sets `V_DEPTH` for a run.

**Done** (where; the measures):
1. *Her V neckline* (`build_figures.py::V_DEPTH` 11 → 7; `woman.fig`, `woman_lids.bin`
   regenerated): with the real figure, `V_DEPTH` moves the point — the sleeve test's patch at
   the frog 1096 px at 200 % (V 11), 631 (6), 324 (4), 25/0 (2). The coordinator: keep a
   moderate V (the dress's design; ref-04), seen under the forearm it reads as the neckline,
   not as a hole. Sheets `tmp\polish6\sheet-v\neckline-{photo,drawn}-{100,200}.png` (old V 15
   over new V 7 at the frog, 0.2, the middle and the tip).
2. *The sleeve test* (`no_skin_shows_through_or_under_her_bow_sleeve`) fails only on islands:
   a patch of skin under the forearm that is not the chest region (the picture's largest
   patch of skin), that dark cloth frames (`SLEEVE_RING` 0.6 of the pixels round it darker
   than `SLEEVE_DARK`), at least `SLEEVE_THIN` 4 px across (the viola's lit lower edge line
   was a 56 × 6 strip) and at least `SLEEVE_ISLAND` 500 px at 100 % (the whole patch). It
   reports every island's size. V 7: islands of 119–410 px (the sliver at the frog and 0.2)
   — passes; the old V 15 (`CANTUS_FIGURE_DIR` = `tmp\polish5\oldfig`): 638–2121 px at
   0.05–0.3, both styles, both instruments — fails.
3. *The back lights' edge line on small steep forms* (the specks gate's largest group): its
   share is the cloth's form's size — on the live arm per vertex (`figure::cloth::broad`, the
   radius of curvature at rest from the normals' turn along the cloth's edges, eased from 0 at
   0.8 cm to 1 at 2.0, `cloth::EDGE_SMALL`; the concert's colour from a second map without the
   line, `LutAt::colour_sized`, slots 8–9), in the bake per sample (`px_shade::edge_size`: the
   footprint over the normal's turn and its slope, `normal_turn` now taken for velvet and
   wool). His cuff buttons' rings and the shoulder's fold-lip dots are gone; the gate 185 →
   147 strays on the final build (`rim_r0`, `rim_r1` tune it).
4. *His crown's bright single strands* (`bowed_human::TIPS` thickness 2.0 → 1.2,
   `px_bowed::TIP_ALBEDO` 2.4 → 1.6, `tip_albedo` tunable): the tips over the hair are faint
   strands; the outline stays broken.
5. *Tunables added*: `knot_lap_ru`, `_rv`, `_h0`, `_hr` (the lift's ramps, soft now);
   `drawn_beside_mode` 2 (below); `probe_x`, `probe_y` (every Drawn figure triangle over a
   point, printed); `strip_probe_x`, `_y` and `strip_pass_edges` (the live outline strips);
   `arm_shadow_reach`, `arm_shadow_depth` (the forearm's shadow on the trunk);
   `contact_depth`, `contact_sigma` (the skin's shade at a cloth's edge).

**Open** (in the order the owner will see them):
0. *The far collar band in Drawn* (`drawn_beside_mode`, default 0 = pass 5: the neck meets
   the tie on the violin's side). Tried this pass: (a) the band sorted by its middles and
   kept out of the instrument's faces on screen (`Shape::Poly`'s `out`, a mask): the band
   then lost to the stretched jacket triangles the tuck leaves (`Wool`, nearer by ~1.5 cm at
   the knot's right) — nothing showed; (b) nearer by 0.5–4 cm and kept out of the chin rest's
   ebony only: it showed, striped by the top's grain decals (sorted just nearer than the
   top's nearest cell) and in shards; (c) a layer of its own (`Layer::Front`) kept out of the
   chin rest and the figure's nearer skin, tie and hair (grown 0.6 pt): with every marked
   triangle, white shards ran down past the knot over the jacket and the varnish (a triangle
   with a tucked corner is a sliver); with only the band kept where it is (every corner
   untucked, `uv[1]` now carries the tuck), too few triangles are left to make a band.
   Next: draw the band from the Photographic picture's own pixels for that patch, or tuck by
   whole triangles (a triangle tucked when its middle is), so the kept band has clean edges.
0b. *The collar's points over the knot* (`knot_lap=1`): with the softer ramps the white curl
   is still there (the collar point's own jagged tip lifted onto the knot); off.
1. *The thin white diagonal line on her dress under the neckline* (every picture, both
   styles, the same place at every bow position, about (257, 315) of the 100 % window): a
   live arm's cloth outline strip — `strip_small_px=1000` removes it; `drawn_skip` of every
   kind does not (not the bake); not a strip between the two passes (`strip_pass_edges=0`
   leaves it). `strip_probe_x/_y` did not catch it at the window's or the picture's
   coordinates — find the space `sc.sp` is in, then which arm and edge.
2. *Her Drawn neck dot* (192, 277): not a sort (a velvet bias of 0.4–2 cm leaves it): no
   figure triangle covers that point but the dress's (`probe_x/_y`) — a hole between skin
   triangles (a T-junction of the dress's cut, `build_figures.py::cut`) through which the
   velvet behind shows. Next: weld the cut's T-junctions, or a wider seam on skin.
3. *The neckline's edge and the forearm's shadow* (the coordinator: hem thickness and outline
   where the dress meets the skin, the forearm's cast shadow on the chest): the tunables
   above change nothing on her — her bake takes the skin from its texture, not from the
   contact detail, and the trunk shadow does not reach there. Bake work; not started.
4. `arcus_figure_specks` 147 strays (185 before; none in the room); most are the earlier camera's
   bow hand (arcus-hands').
5. `arcus_hold_budget` on an idle machine; the WSL Linux check.

**Gates** (the final build, release, the committed figures — `woman.fig` at V 7):
`arcus_light_measure` ✓, `kit_lint` ✓, `figure_contract` ✓, `arcus_scene` ✓,
`scene_identity` ✓, `arcus_figure_auto` ✓, `arcus_arm_pose` ✓ (the left arm's ignored for
arcus-hands), `arcus_figure_shape` ✓, the sleeve test ✓ on V 7 and ✗ on the old V 15 (as it
must), `arcus_figure_specks` ✗ 147 (Open 4), clippy `-D warnings` on cantus-ui ✓.

**Sheets** (never committed; `F:\projects\daw-r42\tmp\polish6\`): `sheet-v\` (the
neckline), `sheet6\` (before = 89243a276's binary, after = this pass's final build:
`man-100.png`, `man-400.png` — the collar and the knot; `woman-100.png`, `woman-400.png` —
the neckline; `his-hair-200.png`). The sheet script: the scratchpad's `sheet6.py <before>
<after> <out>` (shots from `shoot_arcus --hold=man|woman --only=violin-play-+-night
--scales=1` and `--scales=2 --ppp=2`).
