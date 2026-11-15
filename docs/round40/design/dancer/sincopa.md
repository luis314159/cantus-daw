# C · Síncopa, noir (chosen): what the rig draws today, what it would need

The owner chose S3, noir, as Síncopa (*"Noir y paper se ven cool"*). The other answers are
kept:
- v2 in `v2/`;
- S1 (graphic), S2 (the sidekick), the comparison GIFs and still, and their note
  (`sincopa-variants.md`) in `variants/`.

**Files:**
- `sincopa-sheet.png` / `.html`
- `sincopa-moves-night.gif`, `sincopa-moves-paper.gif`
- `sincopa-window-night.gif`, `sincopa-window-paper.gif`
- Design source: `src/sincopa_s3.py` (it imports nothing from v2)

## What it is

A human drawn as a silhouette.
- **Ink and light:** one dark ink (`#0E1216`) for the whole figure.
  - A **gold rim light** (`#E2B85A` at night, `#C99A3A` on paper) down its lit side.
  - A cool rim (`#8A96A2` / `#7A8692`) down the other.
  - That is what draws it on Night's dark room; on Paper it is a sharp silhouette.
- **Clothes:** a hood with gold headphones over it, a long open coat whose tails swing late,
  slim trousers, sneakers with **gold soles** that flash when it steps.
- **Face:** two gold glints in the hood's dark.
- **Motion:** liquid and behind the beat (lag 0.22).
  - The groove: the hips sway on a sine, the chest answers, a hand draws a slow circle.
  - The step: a glide.
- **Signature, Arm wave:** the arms rise; a wave runs hand → elbow → shoulder → shoulder →
  elbow → hand; then it turns its head and its eyes light.

## What `cantus-dance` draws today

| Síncopa needs | Today |
|---|---|
| The rim light | `cast/edge.rs`: ADR 0105's rim (Fuga's violet back light), here on a one-ink figure with no lit cel |
| Pieces | `tube_into`, `blob_into` |
| Arms for the wave | `kit::arm_points` (angle and bend) |
| Coat tails, following late | `Body::top` with `Style::lag` |

## What it would need

1. **Each piece in three fills:** a gold copy moved toward the light, a cool copy moved
   away, then the ink. The rim survives only at the piece's edges. This is what `edge.rs`'s
   rim does, with the offsets the design uses (0.011, 0.35 of it up; 0.6 of it the other
   way).
2. **Two-bone IK** for the legs (shared).
3. **Its own moves:**
   - *Bounce* → liquid;
   - *Step left/right* → glide;
   - *Jump* → Arm wave: a pulse running along five joints, a pure function of the phase.
4. **Cost:** about 40 fills, all opaque.
5. **Tests:**
   - the rims never turn with the hue;
   - every mood (only the glints and the posture) differs at a 300-pixel stage. This is the
     hard one for a faceless figure.
