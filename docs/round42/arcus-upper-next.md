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
