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

## What the tests hold

- `figure_contract`: standing, from the contract alone: the soles on the floor, the arms'
  lengths kept, both hands at their grips on the stick, the head turned to it, the body
  baked into a picture's mesh — for the woman and the man.
- `arcus_hold`: Arcus through the contract — the fingertip on its stop within a pixel for
  every instrument and figure, the hand behind the neck (the picture's own pixels where the
  neck hides it), the arm's angles from the photographs.
- `arcus_scene`, `scene_identity`: at rest nothing is built; the pictures' identity.
