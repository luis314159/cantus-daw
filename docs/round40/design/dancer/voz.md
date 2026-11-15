# F · Voz, the microphone: what the rig draws today, what it would need

**Files:**
- `voz-sheet.png` / `.html`
- `voz-moves-night.gif`, `voz-moves-paper.gif`
- `voz-window-night.gif`, `voz-window-paper.gif`
- Design source: `src/voz.py`

## What it is

A crooner, smooth and showy.

- **Head:** a vintage microphone's capsule (a pill of 15 grille bars over a metal body,
  `#C3CBD3` on Night, `#6A7682` on Paper) with a gold ring at its base. The mic's body is
  its neck.
- **Hat:** a pork-pie hat with a gold band.
- **It wears the evening:** a dinner jacket **white at night and black on paper**. It inverts
  with the theme, as the inks do, so it is always the strongest value in the room.
- **Details:** a gold bow tie, a vermilion pocket square, slim trousers, spectator shoes,
  white gloves.
- **Eyes** on the grille; moods as the family's.
- **Moves:**
  - *croon*: on beat 1 a hand to the heart, on beat 2 out to the crowd, palm up, head back;
    the other hand in the pocket;
  - *slide*: a smooth step-touch, snapping fingers at shoulder height;
  - signature, **Hat tip**: the hand to the brim, the hat lifted and turned with a bow, then
    both arms flung out (ta-da), leaning back with a knee popped, held.

## What `cantus-dance` draws today

| Voz needs | Today |
|---|---|
| Capsule, grille bars, ring | `rect_into` → `rrect_into` (new, shared); ribbons for the bars (one batched path) |
| Jacket halves, lapels, shoes | `blob_into`, `polygon_into` with two values (`edge.rs` `hard`) |
| Gloves | ellipses |
| Hat | rounded rects, turned about the brim |

## What it would need

1. **Theme light** for the jacket, trousers, grille and eyes (shared).
2. **Two-bone IK** for its knees and hand targets (shared with Síncopa).
3. **`rrect_into`** (shared).
4. **A part that follows a hand:** the hat, tipped about the brim by the move's progress.
   This is a per-character extra in `Body`, as Tono's pointer.
5. **Its own moves:**
   - *Bounce* → croon;
   - *Step left/right* → slide;
   - *Jump* → Hat tip.
6. **Cost:** about 55 fills (the grille's 15 bars in one path).
7. **Tests:**
   - the inversion: the jacket at 3:1 or more on both rooms (white on Night, near-black on
     Paper);
   - every mood differs at 300 px;
   - the gold never turns with the hue.

## Risk

Object-head characters are a known illustration trope. The hat, the gold ring and the
inverting jacket must keep it ours.
