# C · Síncopa, the figure in values (v2): what the rig draws today, what it would need

**Files:**
- `sincopa-sheet.png` / `.html`
- `sincopa-moves-night.gif`, `sincopa-moves-paper.gif`
- `sincopa-window-night.gif`, `sincopa-window-paper.gif`
- v1 (for comparison): `v1/sincopa-*`
- Design source: `src/sincopa.py`

## v1 → v2 (the owner: "hazme más cool a Síncopa")

| | v1 (friendly) | v2 (swagger) |
|---|---|---|
| Proportions | about 7 heads, a big head | about 8 heads: longer legs, the head ×1.08 |
| Outfit | a boxy hoodie, gold headphones on the head | an **open oversized vermilion bomber** over a cream tee, lapis **cargo** trousers with a pocket flap and a swinging strap, chunky **high-tops**, gold headphones **round the neck** |
| Hair | short curls | **long braids in a high ponytail** with gold beads and a gold cuff, swinging late; a gold hoop |
| Face | round eyes, a smile | **half-lidded** eyes (the upper lid low and level), angled brows, a **smirk** |
| Motion | a polite groove | swagger: the hips hit a side and overshoot (snap 0.4), the chest answers, a shoulder pops on the off-beat, chin up, a finger snap at the chest on beat 2; a smooth step with the shoulder leading |
| Signature | — | **Shades**: the hand passes over the eyes and the sunglasses are on, the hand flicks away with an overshoot, it freezes leaning back (a hand on the hip, the other pointing down and out), then breathes |

## What `cantus-dance` draws today

| Síncopa needs | Today |
|---|---|
| Two-value pieces, no ink | `cast/edge.rs`: `hard`/`hard_each` without rim or ink |
| Sleeves and trousers through three widths | `tube_into` |
| Jacket halves, tee, head, hair, sneakers | `blob_into` |
| Braid marks, strings, brows, mouth | `ribbon_into` |
| Eyes, lids, sunglasses | `rig_face.rs` (Fuga's lids are the closest) |

## What it would need

1. **Two-bone IK** for every limb, with the ground rule. Hang arms at near full reach (a
   target well inside the reach bows the elbows out).
2. **Chest isolation:** a `chest` offset in `Body`.
3. **The room's colour** for the sleeve cut (`StageInk::room`).
4. **`Style::snap`** for the hits.
5. **The braids:** a tapered tube along points that follow `Body::top`/`Style::lag`. They are
   the secondary motion that sells the cool; budget about 8 fills (the tube, the marks, the
   beads).
6. **Its own moves:**
   - *Bounce* → the swagger;
   - *Step left/right* → the smooth step;
   - *Jump* → Shades.

   Shades switches the face to sunglasses from the moment the hand covers the eyes (a
   move-driven mood override).
7. **Cost:** about 75 fills (Fuga's is 90).
8. **Tests:**
   - every mood differs at 300 px;
   - the chroma key: the nearest is the lapis, 78 from blue, against a rule of 60;
   - the hue turns the bomber only.
