# I · Vinilo, the DJ: what the rig draws today, what it would need

**Files:**
- `vinilo-sheet.png` / `.html`
- `vinilo-moves-night.gif`, `vinilo-moves-paper.gif`
- `vinilo-window-night.gif`, `vinilo-window-paper.gif`
- Design source: `src/vinilo.py`

## What it is

A turntable that plays itself.
- **Head:** a record. Black vinyl with a rim that holds 3:1 at night, four groove rings, and
  a light sheen and a gold marker that turn with the spin. The vermilion label carries its
  face upright; the gold spindle is its nose. DJ headphones, one cup on, one pushed back.
- **Body:** a graphite plinth with a lapis light strip (its length follows the level), and a
  gold chain with a tiny record pendant.
- **Right arm, the tonearm:** a gold pivot, a counterweight, a pale rod, and a headshell
  with a gold stylus for a hand.
- **Left arm:** a sleeve and a white glove that scratches the record's edge.
- **Legs and feet:** slim legs, white sneakers on gold soles.
- **No maker's look:** no plinth layout, logo or label design of any brand.
- **Moves:**
  - *scratch*: the record rocks ±0.10 turn twice a beat under the glove while the tonearm
    pumps on the beat;
  - a step;
  - signature, **Backspin**: brake, three turns back in a blur, then on the drop the record
    runs forward and both arms go up.

## What `cantus-dance` draws today

| Vinilo needs | Today |
|---|---|
| Disc, rings, label, spindle, pendant | `ellipse_into`; rings as even-odd rings or ribbons |
| Sheen arcs, chain, tonearm rod | `ribbon_into` |
| Plinth, headshell, cups | `rect_into` → the shared `rrect_into` (new) |
| Face on the label | the family's face pieces (dots, arcs) |

## What it would need

1. **A turn angle as a pure function of the beat:** 0.55 turn a beat, plus the move's own
   offset (`shape`, the scratch and the backspin). A per-character extra in `Body`.
2. **`rrect_into`** (shared); two-bone IK for the legs (shared).
3. **Its own moves:**
   - *Bounce* → scratch;
   - *Step left/right* → step;
   - *Jump* → Backspin.

## Cost

About 35 fills, plus 6 during the backspin blur (extra sheen arcs). All opaque. Fine.
