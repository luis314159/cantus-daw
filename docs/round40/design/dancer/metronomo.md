# K · Metrónomo, the serious one: what the rig draws today, what it would need

**Files:**
- `metronomo-sheet.png` / `.html`
- `metronomo-moves-night.gif`, `metronomo-moves-paper.gif`
- `metronomo-window-night.gif`, `metronomo-window-paper.gif`
- Design source: `src/metronomo.py`

## What it is

A wooden metronome with a butler's dignity.
- **Body:** a tall pyramid in two values of wood with a lighter grain, on a plinth. A gold
  finial on top; a dark scale panel with gold ticks.
- **The pendulum:** a steel rod with a gold weight, pivoting low on the front. Its swing is
  **30° × cos(π·beat)**, so each extreme lands exactly on a beat: never off it.
- **The butler:**
  - a white wing collar and black bow tie;
  - a gold monocle on a chain, half-lidded eyes, a raised brow, a curled moustache;
  - white gloves, a silver tray with a cup, white spats on short legs.
- **Not Tic** (the old family's robot metronome): wood, manners, a monocle.
- **Moves:** it barely moves, and that is the joke.
  - *keeping time*: a small bow on the one, the free hand conducting a tiny two;
  - exact side steps (snap 1);
  - signature, **Doble tiempo**: the pendulum runs at ×2, then ×4, its extremes still on
    the beats. He loses his composure: eyes wide, the monocle falls on its chain, the arms
    and tray flail. On the beat he straightens his tie and recomposes.

## What `cantus-dance` draws today

| Metrónomo needs | Today |
|---|---|
| Pyramid, plinth, panel | `polygon_into`, `hard` (two values) |
| Pendulum, ticks, moustache, chain | `ribbon_into` |
| Monocle, finial, gloves | `ellipse_into`, even-odd ring |
| The face | the family's face pieces |

## What it would need

1. **The pendulum's angle** as a pure function of the beat (and the move's tempo multiple).
   This is a per-character extra in `Body`.
2. **The monocle's fall** (lerp eye → chain end) from the move's progress.
3. **Wood colours** join the cast's palette: round 39 kept the wooden unit faces. Check the
   chroma-key distance (wood is far from every key).
4. **Its own moves:**
   - *Bounce* → keeping time;
   - *Step left/right* → exact steps;
   - *Jump* → Doble tiempo.

## Cost

About 45 fills, all opaque. Fine.
