# E · Onda, the wave: what the rig draws today, what it would need

**Files:**
- `onda-sheet.png` / `.html`
- `onda-moves-night.gif`, `onda-moves-paper.gif`
- `onda-window-night.gif`, `onda-window-paper.gif`
- Design source: `src/onda.py`

## What it is

A living ribbon, the most abstract of the family.

- **Its body:** it rises from a curl on the floor as a travelling wave (60 samples up a
  centreline) to a round head.
- **The twist:** the ribbon twists as it moves, so its front and back take turns.
  - Front: lapis, `#7D97E6` on Night, `#2E4794` on Paper.
  - Back: gold.
  - The twist is strongest at the tail and nil at the head, so the face always shows.
- **No face, only cut eyes:** like Compás, its eyes are negative space in the room's colour.
- **Moves:**
  - *pulse*: the amplitude rises on the beat and it leans side to side;
  - *travel*: the base slides with the step-touch's hips;
  - signature, **Square drop**: it crouches and springs up, and in the air the sine becomes
    a square wave (a gold lightning bolt), then a saw on the way down, then melts back.

## What `cantus-dance` draws today

| Onda needs | Today |
|---|---|
| The ribbon as polygons | `polygon_into`: a run of samples is one polygon, left edge then right edge reversed |
| The head, the curl | `ellipse_into`, `ribbon_into` |
| The cut | the outline (the whole ribbon grown in the room's colour) |
| Squash, lift | the renderer's lift and `Body::squash` |

## What it would need

1. **A character that is not a skeleton.**
   - `Character::draw` already receives the `Pose` (beats, phase, energy, accent): Onda reads
     those, not a `Body`.
   - The wave travels with `pose.beats`, so it is still a pure function of the song.
   - Moves map onto its own numbers: amplitude from energy and accent, lean, base offset,
     shape and saw.
2. **The theme light** (shared) for the front colour.
3. **A move-driven morph** (`shape`, `saw`) for the Square drop; the sixteen moves are re-read
   as waves:

   | Move | Onda |
   |---|---|
   | Spin | a full twist |
   | Freeze | a flat line |
   | Bow | a curl over |
   | Arms up | a tall stretch |
   | Clap | a fold |
4. **Cost:** about 10 fills: two front and back runs (often 2–4 polygons), two shade strips,
   the head, the curl and the cut.
5. **Tests:**
   - bounds while squared (the amplitude grows);
   - nothing outside the bounds when lifted;
   - the chroma key (the nearest is its lapis shade, 83 from blue);
   - the face always front (the twist at the head is nil).

## Risk

Without limbs it cannot do every move literally. It is the hypnotic one, made for streams
and the companion.
