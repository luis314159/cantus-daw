# ADR 0159 — The figures that play an instrument

**Status:** Accepted (round 42, the `arcus-upper` builder; the owner's direction of
2026-11-24) · **Date:** 2026-11-24 · **Builds on:** ADR 0126 (the scene engine), ADR 0157
(Arcus; its amendment for what holds the instrument) · **Schema:** none (the holder is a
machine preference, `Settings::arcus_holder`)

## Context

Arcus's picture can show who holds the instrument: a stylised musician, a wooden mannequin,
and — the owner's request of 2026-11-24 — a realistic woman in a black velvet concert dress
and a realistic man in a dark suit, built offline from MakeHuman's CC0 assets
(`tools/arcus-figures/`). Their arms are live: the left hand's fingers land on the stop, the
bow hand rides the frog, every frame. The owner wants these figures reusable by the next
instruments that show a player — the guitar family, the winds, Chorda's pianist, Tympanum's
drummer — not rebuilt for each. The code that posed them was written for Arcus: the body was
posed from Arcus's stylised skeleton, the bow hold lived in Arcus's drawing code, the
occluding capsules in its live painter.

## Decision

- **A shared module, `scene/figure/`**:
  - `rig` — turns (`Rot`), a posed bone's transform (`Xf`), how a bone is aimed (`Aim`) and
    the two-bone reach every limb is set by (`two_bone`: an elbow, a knee, a finger's
    middle joint). Moved from `bowed_human`/`bowed_hold`, which re-export them.
  - `play` — **the PlayRig contract** an instrument fills, and the poser for the realistic
    figures:
    - the figure's facing and its right;
    - **the head**: where it goes — a chin rest's point (`HeadAt::Jaw`: the body hangs
      from it) or the eyes' place (`HeadAt::Eyes`) — and how it is turned;
    - **the seat**: hung from the head (a chin rest: leant, turned, the chair follows),
      a chair or a stool (the hips over a point, the seat's top at a height), or standing
      (the hips over the feet at the legs' length);
    - **the legs**: forward from a chair, to given knees and ankles, or straight;
    - **each hand**: a pose already worked out (the live arms'), a **grip** from the
      library with the elbow's pole, or at rest (a wrist and a pole);
    - **the occluders**: capsules that stand in front of the limbs (the live layer paints
      what they hide first, then stamps the picture back over it).
  - `grip` — **the grip library**: `BowFrog` (a bow at its frog, designed against
    photographs of real bow holds: knuckles over the stick, fingers draped to its far side,
    the thumb bent at the frog; the little finger on top for a violin or a viola) and `Rod`
    (a power grip round a stick, a mallet's or a drumstick's handle). A grip is data: the
    hand's joints, which the arm is posed to (`bowed_human::arm_aims`: each phalanx turned
    the shortest way from the hand's turn — no twist —, the non-playing fingers relaxed to
    70–86 % of their own length, the playing one exact). The necks' grips stay Arcus's
    (`arcus_draw_hand::left_pose`): they are worked out from its strings and stops.
  - `live` — the occluding capsules the live limbs are shaded by (the creases between
    fingers, a finger against the stick or the neck).
  - `neck_hand` — **the left hand on a violin's or a viola's neck, posed from data**
    (amendment, 2026-11-24, the owner: "the violin's left hand is a disaster"). The poses
    are authored offline on the figure's own MakeHuman hand (three phalanges a finger; the
    thumb's CMC, MCP and IP) by `tools/arcus-figures/left_hand.py`: each one solved
    (Levenberg–Marquardt, several starts, the best kept) against the rules a violinist's
    hand keeps — the neck in the crook between the thumb and the index finger's base, a gap
    under the palm; the thumb along the G side opposite the first and second fingers, its
    tip about level with the fingerboard; each finger rising from the E side and arching
    high (MCP 45–70°, PIP 70–100°, DIP 20–50°), the playing tip coming down nearly upright,
    the others hovering curved over their notes; the knuckles up beside the fingerboard; the
    palm toward the player and the neck; the wrist straight on a forearm of the figure's
    own length — and rendered from the picture's camera, from photograph 4's angle and from
    the G side beside the owner's photographs. What is stored
    (`assets/figures/left_hand.bin`, 75 KB) is the result: a row per figure × instrument ×
    string × playing finger (or none) × hand position (1, 3, 5, 7, 9, 12, 15 semitones),
    the wrist and every bone's turn as quaternions. At run time the rows either side of the
    hand's position are blended, the bones carried down the chain from the rest skeleton
    (the tool's forward kinematics, the same numbers), and the hand moved by the few
    millimetres that put the playing finger's tip exactly on its stop (the vibrato's rock
    moves it with the hand); the elbow sits on the hand's line (the wrist straight). The
    cello's and the bass's hands and the stylised figures' stay worked out from the stops
    (`arcus_draw_hand::left_pose`).
    **From photographs of professional violinists** (amendment, 2026-11-25, r42:arcus-hand —
    the owner, after three passes: "URGENT LA MANO IZQUIERDA DEL VIOLIN", still "un
    desastre"). Forty Wikimedia Commons photographs of soloists, sections and close-ups,
    plus the owner's five, were measured (`tools/arcus-figures/left_hand_refs.md`; the
    photographs are study material outside the repository, never in it or the app), and
    three causes were found, each fixed and gated:
    - **The rig bent the fingers in the wrong place.** MakeHuman's finger bones begin at
      the webs, about a centimetre and a half past the knuckles' joints (her first
      phalanges 2.2–3.0 cm where a real one is 3.7–4.2). Bent there, a finger folded
      halfway along its first phalanx and the palm ran on into it: a fist, short fingers on
      a broad palm. Each finger's base joint now moves back 12 % of the wrist-to-base
      length along its metacarpal, and the skin between passes smoothly from the hand's
      bone to the finger's (`bowed_human::knuckle_fix`, mirrored in
      `hand_rig.py::knuckle_fix` for the solve). The middle finger over the back of the
      hand goes from 0.87 to about 1.1.
    - **The hand faced the camera broadside.** The rules now hold the back of the hand
      turned 30° toward the scroll and rolled 20–26° down (the palm up toward the neck's
      underside and toward the player), within 5°. The base knuckles sit beside the E side
      at the fingerboard's height, a little under it. The first phalanges rise (within 40°
      of the board's normal), the middle joints stand 3.0–3.3 cm over the board (scaled by
      the hand's length: his are a quarter higher), and the playing tip comes down within
      20° of the normal. Abduction is near zero (the fingers parallel), and the wrist turns
      at most 6° toward the little finger. The forearm's way is now kept per row: the table
      is version 2, 70 `i16` a row.
    - **The camera.** Measured in the instrument's frame, the picture's camera looked from
      59° round the neck toward the scroll. The photographs are taken square to it: photo 5
      at 88°, photo 2 at 90°, both 23–31° over the board's plane. From 59° no reachable pose
      showed the back narrower than 1.4 times its length (photographs: 0.5–0.9), because the
      hand's length foreshortens. The realistic violinists are now seen from azimuth −20°,
      5° up (`bowed_draw::REAL_HELD_VIEW`: 86° to the neck, 31° over the board). The
      stylised figures keep their view.
    - **The hand's bulk** (the owner on v2: "tosca… la parte abultada gorda"): the solve
      had left the thumb's base free and it turned 65°, ballooning the thumb's ball; it is
      capped at 30°. The forearm's wrist end now carries 95 % of the hand's turn (70 %
      left the wrist pinched under the hand). Her palm is drawn in across by 0.86 toward
      the hand's middle line with the fingers' bases (`PALM_SLIM`); his hands are scaled
      0.93 about the wrist (`HAND_SCALE`: wrist to fingertip about his face's length).
      Both are load-time edits mirrored in `hand_rig.py`, so the table is solved on the
      hands the app draws.
    The study tool `hand_study.py` measures any pose as the photographs are measured, and
    `hand_sheet_v10.py` draws the app's own picture from the earlier camera, the new one
    and photo 5's angle. High's hand is drawn as curved patches (`arcus_draw_flesh::fine_tri`:
    PN triangles, Vlachos et al. 2001, cut to about 2.5 px a side) with the skin's own
    texture per point, warmed to the face's (`SKIN_WARM`), the knuckles and the fingertips'
    pads a little redder, the nails paler at their free edge, and each fingertip's soft
    shadow on the fingerboard. The sleeves read as arms in front of the body: a soft cool
    rim and a lifted value (`cloth_rim`), the
    texture seam the thinning had split down each sleeve (a crack the wall showed through)
    is closed when the arm's mesh is made (`close_cloth_seams`); his shirt's cuffs stay
    under the jacket's sleeves (drawn live they broke into specks). From the new camera
    the bake's own defects showed and were fixed at load: the jacket, the shirt and the
    tie told apart with clean edges (`smooth_tie`, a two-neighbour majority), the suit
    1.2 mm further off the skin (`lift_suit`), and the bow arm's shoulder-band triangles
    the live arm covers or the bake's pose stretches left out of the bake (`emit`).
    **The hands** (amendment, 2026-11-25, r42:arcus-hands — the owner: "the musicians can
    still look much, much better… especially the fingers of the hand"; earlier "un
    desastre", "artritis", "tosca/gorda"; the reference is photographs of professional
    violinists):
    - *Proportions.* MakeHuman's index and little fingers were short against the middle
      (7.7 and 6.0 cm against 8.6 on her; a real hand's index ~0.93 of the middle, the
      little finger's tip at the ring's last joint) and the little finger's middle phalanx
      0.49 of its first. Each finger's segments are stretched along their own bones when the
      figure is built (`build_figures.py::lengthen_fingers`); measured, 1 : 0.57–0.60 :
      0.55–0.69 (the last to the skin's tip; the bones' 1 : 0.63 : 0.45 plus the pad), the
      index and the ring 0.90 / 0.94 of the middle, the middle finger 1.13 of the back.
    - *Form.* The joints fuller than the shafts, each phalanx a little slimmer than the one
      before, the pad's side flatter than the nail's (a rounded D), the tip's pad full
      (`slim_fingers`); the nails 0.6–0.75 of the finger's width (`NAIL_ARC`).
    - *Bending.* A finger posed to a tip bends all three joints, the last following the
      middle at 0.7 of it (`figure::fingers::curl`, Kapandji; mirrored in `hand_rig.py`):
      the bow hand's fingers curl round the stick, slanting toward its tip, the knuckles
      higher over it (`grip::bow_frog`, against Commons "Right hand position on bow.jpg").
    - *The left hand's table, re-solved* as open arches (the coordinator on sheet hands-1:
      the fingers read as hooks): the first phalanx leaning over the board toward its string
      (within 50° of the board's normal, was 31–40°), the middle joint carrying the bend
      (55–100°), the last nearly straight onto the string (4–25°, was 15–45), the tip within
      25° of the normal; and the hand and the forearm standing up from the neck as the
      photographs' front views show them (the table had the hand's line 55° to the neck in
      the picture, the photographs 70–89°): `HAND_UP`, `FORE_UP`, the wrist's turn toward
      the little finger 0–12° resting at 8 (the photographs 3–23°, median ~10; the gates
      `arcus_hold` and `arcus_hand_table` hold it within 16°, was 8), the back turned 10°
      toward the scroll (was 30°).
    - *The neck's edge.* A hand patch with some corners behind the neck is drawn in both of
      the left arm's passes, cut four a side, each small triangle in the pass its own middle
      belongs to (`CLS_SPLIT`): painted behind whole, the neck's stamp had cut the fingers
      along their triangles — the dark board-coloured shards and stair-steps.
    - *Creases* as marks on the skinned surface (`views/inst/arcus_draw_marks.rs`): three
      over each middle joint, two over each last joint and the thumb's, anchored at rest to
      the arm's triangles, carried, sorted and hidden with the skin; soft (at most 0.3
      darker), at least 1.6 px wide, gone below 12 points a centimetre. The per-vertex crease
      band (dark 1–3 px dashes at 100 %) is gone.
    - The flat skin round the wrist takes the patches' texture and relief per vertex (a
      lighter band had marked where the patches end); the patches' corners and edge
      midpoints are lit once a frame and shared.
  - `Eyes` — **the realistic figures' eyes, closed or open**, a machine preference
    (`Settings::figure_eyes`, Settings ▸ Appearance under *Figure detail*; amendment,
    2026-11-25, the owner liked them closed: "parecían concentrados o disfrutando de la
    música"). **Closed** is the default. Closing is a morph over the built figure
    (`tools/arcus-figures/lids.py` → `assets/figures/<who>_lids.bin`, 11 KB each). About
    each eye's centre the upper lid turns down by the opening's own angle plus 3°, fully at
    its margin and fading toward the fold, the corners and the brow, so it slides over the
    eyeball and never into it; the lower lid rises 3°; the lashes ride with their lid. The
    eyes are part of the holder the picture is cached by (`Holder::eyes`, never written with
    it), so a change bakes the picture again.
  - `stylised` — the stylised musician's and the mannequin's poser **on the same
    contract** (amendment, 2026-11-24): Arcus fills one PlayRig for them
    (`bowed_figure::arcus_rig`: the jaw on the chinrest or the pelvis on the cello's seat,
    the legs, the left hand's rest) and `stylised::skel` sets their joints from it — the
    numbers their pictures were drawn with, so the pictures are unchanged. What is the
    instrument's (where its neck is, whether a chair stands under the figure) comes beside
    the rig (`stylised::Beside`).
  - `cards` — **hair, brows and lashes as alpha-tested cards** (amendment, 2026-11-25,
    the review: a shell read as a helmet and brows painted on a half-centimetre vertex grid
    vanished): MakeHuman's own CC0 proxies (`ponytail01` for her — its tail left out, the
    chignon is ours —, `short02` for him, `eyebrow001`/`eyebrow010`, `eyelashes01`; each
    asset's licence checked CC0 in `packs/makehuman_system_assets.json` and its `.mhmat`),
    fitted to each body by the converter, their textures kept as grey and alpha
    (`assets/figures/<who>_<hair|brow|lash>.png`, 512/256 px, 370 KB in all). The
    Photographic raster alpha-tests them (a sample the card's alpha leaves bare is not the
    card's — in the picture and in the shadow map) and shades them by the texture's grey;
    a picture with a realistic figure keeps two samples a pixel side at any size so their
    strands are not broken dots. The Drawn picture has no textures: it keeps the hair's
    thick shell and the painted brows (`Stuff::Shell`); the Photographic one draws a thin
    scalp (`Stuff::Scalp`) under the cards instead.
    **One head of hair** (amendment, 2026-11-25, r42:arcus-hair — the owner's review of
    sheet v8: her hair rendered as camouflage, the hairline a hard edge). The patches were
    the scalp — it stood 2.5–3.9 mm over the skin, over a quarter of MakeHuman's ponytail
    cards, which hug the skull — and the texture's own large-scale light. Now:
    - a hair card's texture is stored as its *maps* (`tools/arcus-figures/hair_maps.py`,
      RGBA, 1024 px): the strands' detail (the grey over its own alpha-weighted blur,
      σ 10 texels, so no region is lighter or darker than another), the way the strands
      run (the structure tensor's minor axis, as a doubled angle) and the coverage; the
      bake reads it at the mip level a sample reaches (`cards::Card::levels`);
    - **alpha to coverage** over the picture's samples — a realistic figure's picture is
      baked at three a side while it fits three times the bound, else two
      (`hybrid::figure_samples`, for the app's own path `render_of` too: it had baked a
      200 % picture at one, every card's coverage and every silhouette all or nothing) —
      an ordered pattern turned by the triangle (two layers at half cover three quarters),
      the texture's soft fringe drawn in toward the strands' cores; a pixel a figure's
      silhouette crosses sums its samples evenly (Karis's weighting by brightness gave a
      light jaw or her hair too small a share of its edge);
      brows and lashes read a level and a half softer, their coverage raised (a tone, not
      dots); no card in the shadow map, as before;
    - every card, the scalp and the chignon **lit as one** (`px_shade::strands`): the
      diffuse from the head of hair's smooth normal (the cards' middle, for the scalp too),
      wrapped (0.35); one base colour (her blonde at 0.6 of the Drawn colour: per pixel under
      the key it clipped to white) with each strand's own detail; **Kajiya–Kay** along the
      strands' way from the texture — a narrow near-white highlight and a broad tinted one,
      her sheen band; his hair is dark and his short curls run every way, so his sheen
      stays faint and his detail at a third (`cards::hair_look`); no screen-space
      occlusion on hair (its cards lie in layers a centimetre apart); the scalp deeper
      than the hair over it (0.57 / 0.55 of the Drawn colour: the gaps between cards);
    - the scalp 0.4 mm over the skin and every card at least 1.2 mm over it
      (`build_figures.py::lift_cards`); her front cards drawn 8° down to the hairline
      (`reach_line`: they ended about a centimetre short, and the scalp showed as a band);
    - **the hairline**: each card vertex carries its degrees in from the hairline (the
      `.fig`'s luminance byte, unused on a card); her cards' coverage thins over the last
      eight degrees (`cards::feather`, the faint parts of the strands first, the cores
      last) and the scalp under them thins over six at its edge, unevenly along it
      (`cards::scalp_cover`), so strands, not a line, meet the forehead.
    **The eyes** (same review: open, on the strings): MakeHuman's CC0 eye textures
    (`bluegreen_eye.png` hers, `brownlight_eye.png` his — CC0 in the pack's manifest and
    their `.mhmat` headers — cropped round the iris, `assets/figures/<who>_eye.png`,
    128 px) mapped down each eye's gaze, a natural grey-blue (half the texture's
    saturation, 0.62 of its light) under the upper lid's shadow, wet
    (coat 0.6, rough 0.04: the key's catch-light); the lids raised (upper 5°, lower 2°,
    from 14° and 3°) and the gaze 28° down and 12° to the player's left. Her lips a touch
    more rose.
  - `Quality` — **how finely the live arms are drawn**, a machine preference
    (`Settings::figure_quality`, Settings ▸ Appearance ▸ *Figure detail*; amendment,
    2026-11-24, the owner: "I'd rather it look good than save ms, or make it configurable
    in case someone wants fewer ms"): **High** (the default) the full look; **Light** the
    arm's vertices merged per bone on a grid (1.15 cm on the hand while it is drawn large,
    1.6 cm when small; the sleeve 2.6/3.6 cm — two fingers, a nail and its finger, a cuff
    and its sleeve never merge), no creases' occlusion, the stylised limbs in half the
    strips — the grip, the hand behind the neck
    and the fingertip on its stop exactly High's, since the bones are the same; **Auto**
    High stepping down to Light while the window's frames average over 12 ms and back
    under 7 ms (FL Studio's way under load) — optional, off unless chosen. A file without
    the field reads High.
  - **The polish** (amendment, 2026-11-25, r42:arcus-polish — the owner's lists: "estamos
    cerca pero buscamos la excelencia"):
    - **The left hand's lobe** (the owner on sheet v10: "la mano aún sale algo tosca o
      gorda"). A render of the app's own hand with each corner coloured by its bone showed
      the lobe from under the neck to the wrist was the *hand bone's* skin — the palm's pad
      and heel seen from below (the camera is 5° up) and its ulnar edge — not the thumb,
      which the neck hides, and not High's curved patches (Light's merged mesh had the same
      outline). Offline the palm's own side is drawn in toward the metacarpals
      (`build_figures.py::slim_hand`, `PALM_VOLAR` 0.45, the back 0.9); at load the hand
      bone's skin past the little finger's line is drawn in (`bowed_human::thumb_web`,
      `EDGE_IN` 0.5) and past the index's toward the thumb drawn in and partly handed to the
      thumb's first bone (`WEB_IN` 0.45, `WEB_TO_THUMB` 0.6), mirrored in
      `hand_rig.py::thumb_web`; High's patches tone the back of the hand per point with the
      extensor tendons, the valleys between the metacarpals and the knuckles (`Relief`), and
      a patch edge is worked out the same way from both its triangles and drawn straight
      against a flat neighbour (`curved_edges`: two ways of summing it left pixel cracks —
      the dark dots across the back of his hand). Gated from the app's camera
      (`tests/arcus_hand_shape.rs`, through a test probe of the live arm,
      `views::inst::arcus::probe`).
    - **Her arms and shoulders** (`build_figures.py::shape_arms`): each arm's girth tapered
      along it (the deltoid kept, a narrower elbow, the forearm fuller under the elbow and
      slim at the wrist — `ARM_GIRTH`), her shoulders' top let down 1.5 cm toward the
      acromion and the deltoid's cap drawn in 0.5 cm (a trapezius's slope, not a pad), the
      dress's puff over the shoulder 0.55 → 0.1 cm and the sleeves' ease 0.35 → 0.18 cm; the
      joints never move, so every pose solved on them stands. Measured with
      `measure_arms.py` (her velvet sleeve: upper arm 28.3 → 26.9 cm, elbow 25.6 → 23.2,
      forearm's top 24.6 → 23.4) and gated from the app's camera
      (`tests/arcus_figure_shape.rs`).
    - **Skin**: no softbox in its mirror — the room's tall strip lights, meant for the
      pianos' lacquer, were the pale vertical streak down both faces at 200 % — and a key
      sheen of its own instead (two lobes, Schlick's Fresnel; `px_shade::skin_sheen`).
      Offline, per vertex: the neck and the neckline a little cooler and paler than the
      face, the suprasternal notch and the skin under the jaw shaded, the jaw cooler, the
      ears warmer and redder (MakeHuman's own ear-flap target as the ear's mask) and tilted
      back 15° (`ear-rot-backward` 0.6: they stood upright), his beard's blue-grey shadow on
      the jaw, the chin and over the lip, and the cavities' occlusion traced on the head at
      rest (the conchae, the nostrils, the mouth's and the eyes' corners —
      `build_figures.py::tones`, `cavities`); the sternocleidomastoids, the collarbones and
      the notch in relief on the mesh (`relief`).
    - **Where cloth meets skin** (`figure::cloth`, worked out once when a figure is read):
      each cloth vertex's distance to its cloth's edge (the neckline, a cuff, a lapel) —
      the edge's roll catches the light —; the skin's contact shadow by its nearness to the
      cloth; the skin under a cloth's edge drawn in a few millimetres (`sink`: his collar's
      sawtooth on his neck).
    - **Velvet**: a bright rim where the pile turns fully away, the back light along its
      edge, the bodice's soft folds at the waist (`build_figures.py::cloth_folds`, carried as
      the vertex's texture coordinates and drawn per pixel by `cloth::fold`), the live
      sleeves' folds in the elbow's crook (`arcus_draw_flesh::elbow_folds`, as deep as the
      elbow is bent) and the arm's soft shadow on the trunk it crosses
      (`trunk_shadow`). **His wool** (`Tex::Wool`): a heather where a pixel holds it, a faint
      twill, its edges' roll. The hem's inside seen through a sleeve's open end drawn in the
      sleeve's shade (the grey-blue points by his bow hand).
    - **Light**: a portrait's back light for a realistic figure (the rim ×1.7 by night, ×2.2
      by day); **hair**: tonal streaks along the strands (`HairLook::streaks`), Kajiya–Kay's
      two lobes shifted along the strand, her blonde a little toward ash and deeper (0.52),
      his strands combed one way in the light so a sheen band can cross his head.
    - **Small**: the nails' lunulae, the creases over the middle and last joints, faint
      veins on the back of the hand; the chair's back rail slimmer.
  - **The polish, pass 2** (amendment, 2026-11-25, r42:arcus-polish2 — the owner: "the
    musicians can still look much, much better"; then "no te preocupes por los tiempos,
    luego optimizamos o hacemos configurable": the look first):
    - **The shoulders' band.** Where an arm's part meets the body (the band, in both parts)
      the live arm fades into the baked body. The bake poses the band with the arms at rest;
      the live arm poses it raised, so the faded live triangles stood off the baked ones —
      the dark and grey shards round her bow-side shoulder and the spikes under his upper
      arm by the frog. Now the band's vertices keep the bake's place until the arm's share
      reaches the opaque line (`bowed_human::ARM_COVERS`, 0.38, where the bake leaves the
      bow arm's band out) and turn with the arm over the next 0.4 of share
      (`arcus_draw_flesh::BAND_TURN`): where the two overlap they are the same surface.
    - **Dual-quaternion skinning** for the live arms (Kavan et al. 2007; a bone the pose
      stretches — a finger's phalanx — and the vertices on it keep linear blending): linear
      blending drew the raised shoulder in on top and out under it, broad.
    - **Sleeves follow the forearm**: a sleeve's vertices (only its cloth round them) take
      the hand's bones' share from the forearm's second bone, so the cuff no longer turns
      with the wrist into spikes and a jagged end.
    - **The outline mitred**: each live arm's outline corner takes the mean of its edges'
      ways out, so neighbouring soft strips meet instead of overlapping at a turn (the
      dotted rim lines and the pixel notches the stray-pixel gate found).
    - **Strings drawn even.** The Photographic bake shades a string a pixel or two wide
      (`raster::WIRE_PX` 2.5) at its round's middle with only its coverage falling off
      across (`Geo::wires`: the bowed strings and her loose hairs); the Drawn picture draws a
      string thinner than a point and a half in one colour (`TubeOpts::flat`). Shaded from
      the pixel's place across, a slanted string was bright where it crossed a pixel's
      middle and dark between: a dashed line. Chorda's and Tympanum's rods are untouched.
    - **The trunk's shadow** from the arm's three pieces (upper arm, forearm, hand as soft
      bands cast along the key onto the trunk's capsules, wider and fainter the farther the
      trunk lies, drawn in where their sides find no trunk) instead of every arm triangle:
      softer, as the key is a softbox, and 0.45 ms cheaper.
    - **Hair.** Hers: the underlayer deeper where the hair turns down (the nape, under the
      crown's fall, round the bun), the roots deeper for the first degrees in from the
      hairline (a card's hairline degrees now ride in its shading coordinates), the
      lengths a little lighter toward the crown — all along the strands; broad isotropic
      sections were tried and taken out (camouflage again, the coordinator). His: the
      Kajiya–Kay band tinted warm (`HairLook::warm`), strong enough to read (0.6 / 0.45) and
      broken along the strands into clumps (`broken`), his hairline feathered over a
      narrow ramp. Her loose hairs: baby hairs from the front of her hairline and a few
      flyaways off her crown, rods finer than a pixel (`Role::Wisp`, the Photographic
      picture's only). The Drawn picture: her hair's edge blends into its own deeper roots
      over the band's first third (the pale pink stripe at her temple was the skin blended
      over the whole band), its underlayer deeper, and drawn strand lines along its
      across coordinate (`bowed_draw_tris::strand_lines`).
    - **His collar and tie.** The suit's parts were told apart per triangle, so the lines
      where shirt, lapel and tie meet ran in teeth: those lines are smoothed
      (`smooth_borders`). The shade under a cloth's free edge on the shirt and the tie
      (`figure::cloth::under_edges`: a collar's points, the lapels) gives the collar its
      thickness; the shirt and the tie have their own surface in the bake (`Tex::Shirt`:
      that shade, a rolled edge), the tie's silk softer. A four-in-hand knot is built over
      the tie's top (`tie_knot`: an inverted trapezoid standing off the shirt with the
      wrap's diagonal fold).
    - **The faces.** The lids' crease, the lash line and, with the eyes closed, the lashes'
      soft shadow under the margin, from the lids' own morph (`lid_shade`); her earrings
      polished gold, so the key's glint shows.
    - **The instrument.** Rosin's faint dust on a played top under the strings, from past
      the bridge toward the fingerboard (`with_rosin`, the Photographic picture); a cloth's
      edge roll over the centimetre the mesh can hold (`EDGE_ROLL_CM`: at 0.4 cm the
      lapels' roll was a dotted line).
    - **Gates**: `arcus_figure_shape` measures each arm where its mesh crosses the plane
      across it (the thinned forearm's rings lie farther apart than the old slab) and is
      on; `arcus_figure_specks` is on; `arcus_figure_auto` (new) holds Light's coarse arms
      and Auto's step down on long frames and back.
- **Arcus is the first client**: `bowed_human::arcus_rig` builds its PlayRig from the
  stylised skeleton (the chin rest under a chin; the instrument's seat behind a cello or a
  bass; the stylised figure's knees and ankles; the live arms' poses) and `bowed_human::body`
  is `play::pose(fig, &arcus_rig(...))`. Its pictures and its tests are the same.
- **A second client in a test** (`tests/figure_contract.rs`): a figure standing, both hands
  on a stick held level in front of it, from the contract alone — so the contract cannot
  quietly become Arcus-shaped.
- **Baking stays cached where it is** (`bowed_mesh::mesh_for`: per instrument and holder);
  at rest nothing is built or allocated (`tests/arcus_scene.rs`). The stylised musician and
  the mannequin are posed through the contract too (`stylised`, above): all four figures
  read one PlayRig.

## Consequences

- A new instrument that shows a player fills a PlayRig and, if its hands hold something
  new, adds a grip (designed in `tools/arcus-figures/hand_design.py`, which mirrors the
  posing and renders a grip in seconds against a photograph) — the body, the clothes, the
  hair, the live arms and their light come with it.
- The live arms' cost is the figure's mesh: the arms are thinned offline (short edges
  collapsed inside each arm's own part, `tools/arcus-figures/thin.py`), lit from a normal
  map kept while the light and the view stay within 4 cm (`arcus_draw_flesh::lut_colour`),
  skinned once for both of the left arm's passes. Measured in the release profile beside
  the instrument alone, best of five (`tests/arcus_hold_budget.rs`, 2026-11-25, the
  machine not idle), the arms' extra at the median, 100 % / 200 %:

  | figure | violin Light | violin High | cello Light | cello High |
  |---|---|---|---|---|
  | woman | +0.16 / +0.09 | +1.26 / +1.40 | +0.23 / +0.33 | +1.44 / +1.61 |
  | man | +0.40 / +0.33 | +1.83 / +1.68 | +0.33 / +0.32 | +1.65 / +2.37 |
  | musician | +0.20 / +0.45 | +0.16 / +0.36 | +0.02 / −0.01 | +0.12 / +0.15 |
  | mannequin | +0.10 / +0.38 | +0.30 / +0.51 | +0.02 / +0.16 | +0.09 / +0.07 |

  (The final build, 2026-11-25: the fingertips kept unthinned and the hands' texture
  seams welded; the light maps kept over 8 cm of the arm's motion.)

  Light keeps the views' +1 ms (and its slow frames, p95, within half again plus 1 ms);
  High's ceilings are written into the test (3.0 ms for the realistic figures, 1.6 ms for
  the stylised ones; the owner allowed High up to about 4 ms if it buys the look), so the
  default's cost cannot grow unnoticed.

- **The hand's look costs** (r42:arcus-hand, release, best of five, 2026-11-25): High's
  curved patches (≤ 2 cuts a side at ~4 px), the texture per point, the closed seams and
  the fingertips' shadows put the realistic arms at +1.9 to +3.1 ms at 100 % and +1.9 to
  +4.0 ms at 200 % (the violinists' new camera draws their hands larger); Light unchanged
  (+0.2 to +0.6). High's ceilings are now 4.0 / 4.5 ms (the owner allows about 4 ms for
  the look); the test passes.
- **The hair's cost is the bake's, not the frame's** (r42:arcus-hair): the hair, the
  scalp and the eyes are in the baked picture; the live layer is unchanged, so the frame
  budget's numbers above stand (`arcus_hold_budget` re-run in release on this build,
  2026-11-25: High realistic held p50 1.69–2.23 ms, the arms +1.18 to +1.75 ms; Light +0.07
  to +0.33; the test's ceilings kept, it passes). A
  realistic figure's bake now takes nine samples a pixel while the picture has at most
  about 1.07 M pixels (`3 × MAX_SAMPLES / 9`: a 100 % picture), four above it (a 200 %
  one) — where the app had been baking one at 200 %: the bake's time and its peak (about
  30 bytes a sample, for the seconds it bakes) grow with the samples, up to the bound of
  three times `MAX_SAMPLES` (some 290 MB at the most). Memory kept from the first bake
  with a realistic figure: the hair maps' levels (1024² RGBA and their mips, about 5.6 MB
  a figure) and the eyes' textures (128² linear RGB, 0.2 MB each). The binary grows by the maps (`woman_hair.png`
  1.26 MB, `man_hair.png` 0.99 MB, from 0.58 / 0.43) and the eyes (2 × 28 KB).

- **The polish's cost** (r42:arcus-polish, `arcus_hold_budget`, release, best of five, 
  2026-11-25, the machine loaded by two other builders): the live arms' extra, 100 % / 200 %, 
  High — her violin +3.65 / +5.04 ms, his +4.51 / +5.77, her cello +2.59 / +2.69, his 
  +3.36 / +3.38; Light +0.4 to +1.0. **Over the 4.0 / 4.5 ms ceilings for the violinists at 
  High** (and his cello's Light at 200 % +1.00 against 1.0): the arm's shadow on the trunk 
  (`trunk_shadow`), the elbow's folds, the back of the hand's relief per point and the 
  canonical patch edges were added this session; the shadow's steps were halved (6 → 3) 
  after the measure, not yet measured again. The next session measures on an idle machine 
  and, if still over, draws the trunk's shadow from the coarse Light mesh and only the 
  triangles on the arm's silhouette.

## What the tests hold

- `figure_contract`: standing, from the contract alone: the soles on the floor, the arms'
  lengths kept, both hands at their grips on the stick, the head turned to it, the body
  baked into a picture's mesh — for the woman and the man.
- `arcus_hold`: Arcus through the contract — the fingertip on its stop within a pixel for
  every instrument and figure, the hand behind the neck (the picture's own pixels where the
  neck hides it), the arm's angles from the photographs.
- `arcus_scene`, `scene_identity`: at rest nothing is built; the pictures' identity.
