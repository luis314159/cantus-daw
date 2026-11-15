# H · Eco, the delay's ghost: what the rig draws today, what it would need

**Files:**
- `eco-sheet.png` / `.html`
- `eco-moves-night.gif`, `eco-moves-paper.gif`
- `eco-window-night.gif`, `eco-window-paper.gif`
- Design source: `src/eco.py`

## What it is

A small floating ghost and its echoes.
- **Body:** a dome, a hem of seven scallops that ripples with the beat, two arm nubs,
  little gold headphones, and the family's cut eyes (negative space).
- **Colour:** it wears the theme's ink, pale at night and graphite on paper.
- **The echoes:** two or three copies repeat each move a quarter beat late (Delay's taps).
  - Each echo is **the same pose function at t − 0.25k**, so it stays a pure function of the
    song.
  - Each sits 0.085k behind and is tinted 28 / 48 / 68 % toward the room: opaque, not
    translucent.
- **Signature, Feedback:** the copies multiply on a widening spiral (3 → 13) until they fill
  the frame, then all go out at once on the beat with a gold flash ring, and the ghost pops.

## What `cantus-dance` draws today

| Eco needs | Today |
|---|---|
| A ghost | `blob_into` (dome and scallops), `hard` (two values), `ribbon_into` (nubs, band) |
| The pose a quarter beat ago | `pose_at` is pure: evaluate it at t − 0.25k (or `kit::body_at` with a phase offset) |
| Cut eyes | the room's colour (`StageInk::room`, shared with Compás and Onda) |

## What it would need

1. **Theme ink and room in the canvas** (shared).
2. **Echoes:** draw the character at earlier poses before itself, tinted toward the room.
   - The tint must be computed per stage colour. On a transparent floating stage the room is
     the theme's base.
   - Cost: about 8 fills an echo (no face).
3. **Its own moves:**
   - *Bounce* → float;
   - *Step left/right* → drift;
   - *Jump* → Feedback.

## Cost, honestly

| | Fills |
|---|---|
| The ghost | about 11 (dome, cel, arms, band, cups, face) |
| Three echoes | about 24 |
| In all | **about 35**: fine |
| **Feedback**, drawn as designed (13 copies of about 8 fills each) | **about 115 fills for those two beats** |

Feedback as designed would likely pass 1.5 ms on a large stage. **In the rig, the Feedback
copies must be one fill each** (the dome-and-hem silhouette only, no arms or headphones):
13 fills, invisible at that tint and size. The bench (`cast_bench`) must include Feedback's
fullest frame.
