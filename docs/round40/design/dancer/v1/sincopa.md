# Direction C — Síncopa, the figure in values: what the rig draws today, what it would need

**Files:** `sincopa-sheet.png` (and `.html`), `sincopa-moves-night.gif`,
`sincopa-moves-paper.gif`, `sincopa-window-night.gif`, `sincopa-window-paper.gif`. Design
source: `src/sincopa.py`; it dances the shared `groove` and `jump` and its own `steps_human`
(`src/moves.py`).

## What it is

An editorial illustration that dances.

- **No ink line.** Every piece is two flat values: its shade, then its base moved toward the
  key light (upper left) and clipped to the piece. That is ADR 0105's hard cel without the rim.
- **The outfit is the logo's three colours:**
  - a vermilion hoodie (`#E0693B` / shade `#B24E29`) with strings, a pocket and a ribbed hem;
  - lapis wide trousers (`#4664C0` / `#2E4794`);
  - gold headphones;
  - white sneakers with graphite soles and a gold heel tab.
- **Head and face.** Warm skin. Short curls: a crown of 21 bumps and four curl marks that bob
  after the beat. The head is drawn × 1.18 so a face reads at 200 px.
- **Proportions:** about 7 heads. One palette in both themes, like the cast.
- **The groove:** the hips lead, the chest answers the other way (`chest` −0.06), and the head
  and hands follow behind the beat (lag 0.12). On a close the near arm reaches up on the
  diagonal and the far hand comes to the chest.
- **Moods** on a small face:

  | Mood | Face |
  |---|---|
  | Neutral | ovals for eyes |
  | Happy | ^ arcs |
  | Calm, sleepy | closed curves |
  | Surprised | wide eyes |
  | Cool | dark glasses with a gold glint |
  | Excited, singing | an open mouth |

## What `cantus-dance` already draws

| Síncopa needs | Today |
|---|---|
| Two-value pieces, no ink | `cast/edge.rs`: `hard(c, shape, h, rim=false)` and `hard_each` (shaded piece by piece), Fuga's and Volta's look without the rim and without `part`'s ink |
| Tapered sleeves and trousers through three widths | `tube_into` (a tube through points with half-widths, round at both ends) |
| Hoodie, head, hair, sneakers as smooth closed shapes | `blob_into` / `blob_grown_into` (Catmull-Rom, up to 24 points) |
| Strings, curl marks, brows, mouth | `ribbon_into` and the tapered ribbon |
| Face by mood: eyes, lids, sunglasses, mouth | `rig_face.rs` (Fuga's eyes and brows are the closest) |
| The head turning a little before the body | the family's rule (the body follows at 60 %) |
| Contact shadow | `kit::shadow` |

## What it would need (for a builder)

1. **Legs with knees and arms that reach a target: two-bone IK** (`ik2` in `src/rig.py`,
   about 30 lines), with the ground rule: a leg never stretches past its length, so the
   pelvis drops. Today limbs are rubber hoses. A human figure needs a knee and an elbow that
   bend once.
   - Use **near full reach** for arms that hang. A target well inside the reach bows the
     elbows out; that was the "akimbo" defect found and fixed in these renders.
2. **A chest that isolates from the hips:** a `chest` offset in `Body` (or the torso's top
   moved apart from the pelvis). Today `Body::x/y` moves the whole body.
3. **The room's colour for the arm cut:** the same `StageInk::room` as Compás and Tono
   (0.011 round each sleeve where it crosses the hoodie).
4. **Its own Step left/right** (`steps_human`). The groove's open hands and the landing pose
   are style values (`Style::open`, the landing at full reach).
5. **Cost:** about 60–70 fills.
   - The pieces are trousers ×2, sneakers ×2 with soles and tabs, the hoodie, hem, pocket,
     hood and strings, neck, head, curls and marks, face, headphones, sleeves ×2 with cuffs,
     and hands.
   - Each two-value piece is 2 fills. The clip in the SVG becomes the rig's shrink toward
     the light (no clip in tiny-skia), as `edge.rs` does.
   - It is comparable to Fuga's 90, so inside the budget.
6. **Colour rules:**
   - Every colour is ≥ 78 from a chroma key; the lapis trousers are the nearest, 78 from blue.
     ADR 0105 forbids *saturated* blue: lapis passes the measure, but keep the test.
   - `hue` turns the hoodie (and its shade) only.
   - The dark hair is 1.1:1 on Night's room. The head reads by the skin and the gold band, not
     the hair: accept it, or lift the hair's lit value.
7. **Tests to keep:**
   - the bounds;
   - 48 px against 2 160 px;
   - the chroma-key distance;
   - every mood differs at a 300-pixel stage. This one is new for a small face, and the
     risky one.

## Verified only in these renders

Drawn by Edge from the design rig's SVG with clip paths, not by tiny-skia. The face at 48 px
is not readable (it is at 200 px and up).
