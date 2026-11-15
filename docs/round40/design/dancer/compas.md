# Direction A — Compás, the pictogram: what the rig draws today, what it would need

**Files:** `compas-sheet.png` (and `.html`), `compas-moves-night.gif`, `compas-moves-paper.gif`,
`compas-window-night.gif`, `compas-window-paper.gif`. Design source: `src/compas.py` (drawing),
`src/moves.py` (`pump`, `steps_open`, `star`), `src/rig.py` (`snap_time`), `src/body.py` (IK).

## What it is

- **One ink.** The figure is the theme's ink 1: `#E3E8ED` on Night, `#17202A` on Paper.
- **Pieces of one weight with round ends,** in units of a figure 1 tall:
  - head: a disc, r 0.092;
  - torso: a capsule 0.162 → 0.120 wide;
  - legs 0.078 wide, arms 0.066 wide.
- **The cut.** Each piece in front is cut from the one behind it by 0.017 of the room's colour.
  The legs and torso are one silhouette, with no cut between them. The arms join at the
  shoulder and are cut where they cross the body.
- **Gold.** Theme gold: `#E2B85A` on Night, `#9A6E1C` on Paper.
  - The headphones: a band 0.017 wide and two cups 0.040 × 0.066.
  - A **beat ring** at its feet: an opaque ellipse that lands on the beat and shrinks after it.
- **Motion marks.** Two short arcs in ink 3 along the path a hand or foot just travelled,
  drawn only during a snap.
- **No face.** A mood is a posture. *Cool* cuts a visor band into the disc.
- **Pose to pose.** It snaps into a key and holds it, using `snap_time`: the hit crosses
  the key on the beat and the overshoot settles just after. Its moves keep every limb clear
  of the body, in straight bars at clean angles (arms by angle and bend).

## What `cantus-dance` already draws (no new primitive needed)

| Compás needs | Today |
|---|---|
| Head disc, cups, ring | `rig_draw::ellipse_into`, `fixed`/`flat` fills; the ring is an even-odd fill (`fixed_even_odd`) |
| Limbs as round-ended bars | `ribbon_into` (one fill, round at both ends), as Tic's and the family's lines |
| Tapered torso capsule | `tube_into` / `limb_into` (w0 → w1) |
| The cut | the rig's outline: `outline(c, shape, ink)` grows a shape and fills it first. Compás uses the same mechanism with the room's colour as the ink, and without the nudge to the lower right (an even cut) |
| Arms by angle and bend | `kit::arm_points(shoulder, angle, bend, upper, fore, i)` (the design's `fk` is the same convention: 0 hangs, 90 out, 180 up) |
| Contact shadow | `glow` (three translucent ovals) |
| Pop and lock | Volta's `mecha/moves.rs` already snaps the phase to half and quarter beats and locks it |
| Motion marks from earlier moments | `kit::body_at(pose, style, phase, …)` gives the body at any phase, so a draw can sample `phase − δ` itself: still a pure function, no state |

## What it would need (for a builder)

1. **The theme's inks in the canvas.** Today every character paints its own palette, turned
   by `hue` (ADR 0090 §3). Compás takes four colours from the theme: ink 1, ink 3, gold, and
   the room (for the cut and the visor).
   - Add a small `StageInk { ink, ink_dim, accent, room }` to `render::Stage`/`Canvas`, set
     by the interface from the theme's tokens.
   - For a transparent floating stage, the room is the theme's base. For the companion and
     the docked stage, it is the stage's colour. For video, it is the export's background
     (or the base when transparent).
   - `hue` turns only the gold.
   - Imported characters ignore it.
2. **Legs with knees: two-bone IK** in `rig.rs` (`ik2`, about 30 lines; `src/rig.py` has it,
   with the bend side chosen outward). Add the **ground rule** from `src/body.py`: a leg never
   stretches past its length, so the pelvis drops instead. Today legs are rubber hoses
   between hip and foot, with no knee.
3. **`Style::snap`** (0 fluid … 1 pose to pose) applied to a move's phase through
   `snap_time`. This generalises Volta's snapping so that the hit lands on the beat (see
   the doc comment in `src/rig.py`).
4. **Its own moves** (`has_move` true):
   - *Bounce* → `pump`: one arm up on the diagonal and one down, swapping each beat, a W
     flex between them.
   - *Step left/right* → `steps_open`: a T on the step, a disco point with a hand on the
     hip on the close.
   - *Jump* → `star`: straight arms, low behind in the crouch, a V in the air.

   The other thirteen moves take the family's choreography with a pictogram review of each
   pose: limbs clear of the body, straight where extended.
5. **The beat ring** at the shadow, from `Pose::accent`: an opaque ring, its width and size
   from the accent, nothing when it is under 0.04.
6. **Cost:** about 20 fills a frame (2 leg cuts, a torso cut, 2 legs, the torso, 2 arm cuts,
   2 arms, the head cut and head, the band cut and band, 4 for the cups, the shadow and the
   ring) against 52–113 for today's cast. It is well inside the 1.5 ms budget. The motion
   marks add 2–8 ribbons during snaps.
7. **Tests to keep** (`tests/cast.rs` already holds them for every character):
   - the bounds;
   - 48 px against 2 160 px;
   - the chroma-key distance (ink and gold are ≥ 119 from every key);
   - the cut colour never hue-turned;
   - a new test: a pose held between snaps is identical frame to frame.

## Verified only in these renders

The sheets and GIFs are the design rig drawn by Edge from SVG, not by tiny-skia. The shapes
are the rig's: fills, ribbons and grown outlines. Anti-aliasing and the exact cut width at
48 px will differ slightly in the app.
