# Síncopa: three new answers (S1, S2, S3) beside v2: what `cantus-dance` would need

**Update: the owner chose S3 (noir).** It is now the cast's Síncopa (`../sincopa-*`,
`../sincopa.md`); S1, S2 and this comparison stay here.

The owner on v2: *"Síncopa aún no me gusta."* He gave no reason, so this is exploration, not
polish. There are three clearly different answers to *a cool human dancer in the Slate look*,
each with the name and the role kept: the cast's human dancer.

v2 is kept in `../v2/` (sheet, GIFs, note, `sincopa_v2.py`). Its source `src/sincopa.py` still
draws v2 for the comparison.

**Open first:**
- `sincopa-variants-night.gif` / `-paper.gif`: v2, S1, S2 and S3 side by side on the same
  12-beat routine, each doing its signature on the drop.
- `sincopa-variants.png`: the still.

**Then, for each variant:**
- `sincopa-s1-…`, `sincopa-s2-…`, `sincopa-s3-…`: the sheet (`-sheet.png` / `.html`), the
  moves GIFs (`-moves-night/paper.gif`) and the window GIFs (`-window-night/paper.gif`).
- Design sources: `src/sincopa_s1.py`, `src/sincopa_s2.py`, `src/sincopa_s3.py`.

## S1: graphic

**What it is:** Compás's clarity with clothes, hair and a face.
- **Shading:** one flat value per piece, no shading.
- **Body:** limbs are bars of one width; the head is a disc.
- **Hair:** a sharp vermilion A-line bob.
- **Face:** dots and a line.
- **Colours:** it wears the theme as Compás does: a lapis jacket, trousers in the theme's ink 1.
- **Signature, Spin and point:** a paper-cut spin, landing on the beat into a disco point.

**`cantus-dance` today:** `ribbon_into` (bars), `ellipse_into` (head, hands, hoops),
`polygon_into` (jacket, bob), and the room-coloured cut (the outline).

**It would need:**
- the theme's inks in the canvas (as Compás);
- two-bone IK;
- `Style::snap`;
- **the spin as a horizontal scale** of the drawing about the pelvis, through cos(turn). The
  renderer's placement already applies a transform: add a per-pose x scale, and a back view
  (no face) when the scale is negative.

**Cost:** about 25 fills.

**Risk:** flat human figures are common in product illustration.

## S2: the sidekick

**What it is:** feature-animation proportions.
- **Proportions:** a big head (about 3.5 heads tall), big hands and shoes, long thin legs.
- **Face:** huge eyes with pupils and a highlight, thick brows, a mouth that grins with teeth
  and shouts.
- **Hair:** a curly puff with a gold headband.
- **Clothes:** an oversized tee, lapis shorts, striped tube socks.
- **Shading:** two values per piece.
- **Signature, Hype jump:** a deep crouch, a stretched launch, a star with a shout, a squashed
  landing with dust.

**`cantus-dance` today:**
- `edge.rs` `hard` (two values), `tube_into`, `blob_into`;
- `rig_face.rs` (eyes with lids and pupils, a cartoon mouth: the closest to what S2 needs);
- `Body::squash`, `rig_fx::dust`.

**It would need:**
- two-bone IK;
- a mouth with teeth and a tongue, sized by `mouth`;
- pupils that look where the move points;
- the puff bouncing on the accent.

**Cost:** about 60 fills.

**Risk:** fun can tip into childish; it is the opposite answer to "cool".

## S3: noir

**What it is:** a silhouette.
- **Ink and light:** one dark ink for the whole figure; a gold rim light on the lit side and a
  cool rim on the other. That is what draws it on Night; on Paper it is a sharp silhouette.
- **Clothes:** a hood with gold headphones, a long open coat whose tails swing late, sneakers
  with gold soles.
- **Face:** two gold glints.
- **Signature, Arm wave:** a wave runs hand → elbow → shoulder → shoulder → elbow → hand;
  then a look, eyes lit.

**`cantus-dance` today:** `edge.rs` already paints a **rim** (ADR 0105: Fuga's violet back
light, Volta's coral). S3 is that rim on a one-ink figure, with no lit cel.

**It would need:**
- the rim offset per piece. Each piece is three fills: a gold copy toward the light, a cool
  copy away from it, then the ink.
- two-bone IK;
- arms by angle and bend (`kit::arm_points` exists) for the wave;
- coat tails that follow `Body::top`/lag.

**Cost:** about 40 fills.

**Risk:** a faceless figure keeps people at a distance; its moods are glints and posture.

## My reading (the owner chooses)

**S3 (noir)** is the strongest answer to *cool*. It is Slate's own idea (light as value, one
ink, one gold) on a body, and it is cheap.

**S1 (graphic)** belongs most clearly to Compás's family.

**S2** is here as the honest opposite: if what the owner missed in v2 was *fun* rather than
*cool*, it is S2.
