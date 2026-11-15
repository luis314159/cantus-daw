# Onda: three new answers (O1, O2, O3): what `cantus-dance` would need

**Update: the owner chose O1, the oscilloscope.** It is the cast's Onda (`../../onda-*`, `../../onda.md`). O2 and O3 stay here; to rebuild them, copy `onda_o2.py`/`onda_o3.py` back into `src/`.

The owner, after correcting himself: *"Metrónomo está chido, Onda es el que no me gusta."*

The ribbon Onda is kept in `variants/onda-ribbon/` (sheet, GIFs, note, `onda_ribbon.py`).
These are three clearly different answers to *a character that is a waveform* (sine,
square, saw), each with its own silhouette and a signature where the wave changes shape.

**Open first:**
- `onda-variants-night.gif` / `-paper.gif`: the ribbon, O1, O2 and O3 side by side, on the
  same routine.
- `onda-variants.png`: the still.

**Then, for each variant (`o1`, `o2`, `o3`):**
- `onda-<v>-sheet.png` (and `.html`)
- `onda-<v>-moves-night.gif` / `-paper.gif`
- `onda-<v>-window-night.gif` / `-paper.gif`

Design sources: `src/onda_o1.py`, `src/onda_o2.py`, `src/onda_o3.py`.

## O1: the oscilloscope

**What it is:**
- **Body:** none. The figure is a phosphor trace: nine lines (legs, spine, shoulders, arms,
  hips), each rippled by the waveform, its amplitude the level.
- **Head:** a Lissajous loop that turns in depth; two bright dwell points for eyes.
- **Colour:** amber phosphor in the cast's gold. At night a halo, a line and a core; on
  paper a dark-amber line alone.
- **The beam:** a bright dot runs along the trace, drawing it; a faint afterglow stays
  behind.
- **Signature, Square:** the ripples snap into a square wave and the poses go angular; then
  a saw (a lightning-bolt figure); then a sine again.

**`cantus-dance` would need:**
- polylines offset by a waveform: `ribbon_into` along computed points;
- the glow as three batched layers (as Glissando), the halo the only translucent fill;
- the Lissajous as a ribbon of 140 points.

**Cost:** about 10 ribbons (each 200–400 points) × 3 layers, batched by layer into 3 fills,
plus the dot and the afterglow. The point count, not the fill count, is the cost: about 3 000
points a frame. That should be fine, but bench it.

**Risk:** thin at 48 px; its charm is the trace moving.

## O2: the wave-jelly

**What it is:**
- **Body:** chunky and wobbly, its top edge the live waveform travelling across it. A gold
  trace runs just inside that edge.
- **Face and limbs:** two values of lapis, big eyes, a wide mouth, stubby arms and feet.
- **Signature, Shape shift:**
  - a **square** top (a crown of crenellations, eyes wide, arms up);
  - then a **saw** (a grin of shark teeth);
  - then it melts back into a sine with a jiggle.

**`cantus-dance` would need:**
- the outline as one polygon (49 top points plus the rounded sides), two values (`hard`),
  the room-coloured cut;
- the face pieces;
- the arm stubs as ribbons.

**Cost:** about 25 fills, all opaque. Fine.

**Risk:** the friendly answer, not the cool one.

## O3: the surfer

**What it is:**
- **The wave:** the floor is a lapis swell with a gold waveform crest.
- **The surfer:** rides it on a board that follows the wave's height and slope. Sunglasses,
  messy hair with gold tips, a cream tank, vermilion board shorts, a gold pendant.
- **Signature, Square set:** the wave turns square and the surfer hops each step; then a
  saw: a slide down the ramp and an aerial with the board turning under its feet, landing
  on the drop.

**`cantus-dance` would need:**
- the human rig (two-bone IK, shared);
- the sea as one polygon (61 points) with a crest band and a foam ribbon;
- the board placed by the wave's height and slope at x = 0 (a pure function of the beat);
- the surfer drawn in the board's frame;
- the aerial's board flip as a scale of its width.

**Cost:** about 45 fills, all opaque. Fine.

**Risk:**
- The sea takes the stage's floor: on a narrow floating window it must be clipped to the box.
- In the companion it is a scene more than a figure.

## My reading (the owner chooses)

**O3 (the surfer)** is the coolest: a human with a reason to move, where every change of
wave is a trick.

**O2** is the most fun: the crown and the shark teeth are real jokes, and it reads at 48 px.

**O1** is the most beautiful idea, and the closest to the instrument (an oscilloscope), but
it is a line figure like Glissando's neon. Pick it if the cast should keep one "light" figure.
