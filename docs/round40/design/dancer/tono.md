# B · Tono, the knob (v2): what the rig draws today, what it would need

**Files:**
- `tono-sheet.png` / `.html`
- `tono-moves-night.gif`, `tono-moves-paper.gif`
- `tono-window-night.gif`, `tono-window-paper.gif`
- v1 (for comparison): `v1/tono-*`
- Design source: `src/tono.py` (its moves and drawing).

## v1 → v2 (the owner: "hazme más cool a the knob")

| | v1 (cute) | v2 (cool) |
|---|---|---|
| Proportions | a big head (r 0.205) on a short body | a smaller head (r 0.165) on a lean robot body, longer legs |
| Eyes | pill eyes, ^ arcs, a mouth | an **LED visor**: gold segments in a dark slot. No mouth: the five bars are its voice |
| Limbs | floppy cables, jack-tip hands, fader-cap feet | straight segments with joint discs, fists, high-top fader-cap sneakers with a gold stripe |
| Detail | — | a **vermilion patch cable** slung round the neck, its gold plug swinging late |
| Pointer | a notch on the rim | a gold **fin** through the rim, swinging late |
| Motion | bouncy squash and stretch | control: a hard head nod with fist hits (hits with an overshoot, snap 0.45), a mechanical glide with robot arms |
| Signature | — | **Turn it up**: it reaches up, grips its own rim and turns the knob in three clicks (each with an overshoot) to 5 o'clock; the dots fill, the visor scans, it leans back |

## What `cantus-dance` draws today

| Tono needs | Today |
|---|---|
| Knob, rim, lip, dots | `ellipse_into`, flat fills; the lip is a value step (`bands_of`) |
| Straight limbs with joints | `ribbon_into` for each segment; circles for the joints |
| Pointer swinging late | `Body::top` with `Style::lag` |
| Squash and stretch | `Body::squash` |
| Contact shadow | `kit::shadow` |

## What it would need

1. **Theme light in the canvas** (shared with Compás, Onda and Voz). The knob, key, limbs and
   slot take Night or Paper values.
2. **`rrect_into`**, a rounded rectangle: the key, the well, the bars, the visor slot, the
   sneakers, the plug.
3. **Two-bone IK** for its knees, and hand targets for the fist hits and the rim grip (shared
   with Síncopa and Voz).
4. **`Style::snap`** (the `snap_time` hit-then-settle) for the nod.
5. **Its own moves** (`has_move` true):
   - *Bounce* → the nod;
   - *Step left/right* → the glide;
   - *Jump* → Turn it up.

   Turn it up needs the pointer's angle driven by the move (a per-character extra in
   `Body`, e.g. `extra[0]`), and the dots driven by `max(energy, progress)`.
6. **The visor's eyes by mood:**

   | Mood | Visor |
   |---|---|
   | Neutral | keen bars, the inner end higher |
   | Happy | chevrons |
   | Calm, sleepy | thin bars (sleepy: dim gold) |
   | Surprised | squares |
   | Cool | one bar with a scan (its position from the beat) |
   | Wink | one bar off |

7. **Cost:** about 50 fills.
   - 11 dots batched in two paths;
   - 5 bars in one path;
   - the visor;
   - the body;
   - limbs with joints;
   - the cable and plug.
8. **Tests:**
   - every mood differs at a 300-pixel stage;
   - gold, vermilion and the LED colour never turn with the hue;
   - the rim at 3:1 on both rooms;
   - the chroma-key distance (all ≥ 119 except the vermilion, far from every key).
