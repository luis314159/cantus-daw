# G · Glissando, the figure of light: what the rig draws today, what it would need

**Files:**
- `glissando-sheet.png` / `.html`
- `glissando-moves-night.gif`, `glissando-moves-paper.gif`
- `glissando-window-night.gif`, `glissando-window-paper.gif`
- Design source: `src/glissando.py`

## What it is

The owner: *"haz uno basado en Tron, medio luz neón, acá futurista."*

**Neon-futuristic, our own.**
- **The suit:** near-black (`#0B0E12`), defined by light.
- **Lapis neon** (a periwinkle lapis, `#8FAAF5` at night, never the films' cyan) runs:
  - along the outer edge of each limb;
  - down both sides of the torso;
  - round the collar and across the belt;
  - in short arcs on the elbows and knees, which flash on the beat;
  - along the soles.
- **The chest:** the logo's five vermilion bars, their height the level.
- **The head:** sleek and dark, with two **gold** glowing eyes and gold headphones (the cast's
  motif).
- **Night:** each line is three layers: a translucent halo (3.2× the line, 24 %), the line, and
  a pale core (0.38×).
- **Paper:** the halo goes. The line is `#5B79D6`, sharp on the dark suit (4.7:1) and
  readable off it, on paper (3.8:1).
- **Light trails:** fast hands and feet leave trails over the last 0.21 beat, fading.

**What is deliberately not there:** no identity disc, no vehicles, no helmet or circuit
layout from the films, no names, no cyan-and-orange scheme. The palette is the cast's (lapis,
gold, vermilion); the motifs are ours (edge lines, joint arcs, the logo's five bars, the
headphones).

**Personality:** precise, sleek, quiet confidence. It moves like light.
- **Tutting** (the groove): geometric arm shapes on each half beat, snapped with an
  overshoot, a trail on every snap.
- **A glide** (the step).
- **Signature, Halo:** its hands sweep up both sides and draw a circle of light round its
  head; on beat 2 the circle drops to the floor as a ring and fades. It lands in a crouch,
  one hand to the floor.

**Name:** *Glissando*, the musical slide (Italian, as Fuga and Volta). It is free in the app's
strings, unlike "Eco", "Arpegio" or "Legato", which name existing things.

## What `cantus-dance` draws today

| Glissando needs | Today |
|---|---|
| The suit's pieces | `tube_into`, `blob_into`, flat fills |
| Neon lines | `ribbon_into` (one fill each) |
| Joint arcs, the Halo, the floor ring | `ribbon_into` along arc points |
| Arms by angle and bend (the tutting) | `kit::arm_points` |
| Snapped hits | Volta's phase snapping (generalised as `Style::snap`) |
| Earlier moments for trails | `kit::body_at(pose, style, phase − δ)`: pure, no state |

## What it would need

1. **The neon as three batched layers, never a blur.**
   - All of a colour's halos are one path, all its lines one path, all its cores one path.
     They need no ordering among themselves.
   - So the lapis neon is three fills, the gold (eyes, headphones) three, and the vermilion
     bars three.
   - Only the halo layer is translucent: about 0.2 ms for a body-sized area by ADR 0090's
     measure, much less for thin lines.
   - **A Gaussian blur is out:** it would cost several milliseconds on tiny-skia on the
     processor.
2. **Theme light in the canvas** (shared): the line colour and the halo on or off.
3. **Edge lines:** each limb's outer edge as an offset polyline, computed from the tube the
   rig already builds (its left or right side).
4. **Trails as one tapered ribbon per limb:** from the last six samples of `body_at` at
   phase − k·0.035, fading by width. The design draws them segment by segment; the rig would
   draw two fills per limb, or nothing when the limb is still.
5. **Two-bone IK** for the legs (shared) and `Style::snap` (shared).
6. **Its own moves:**
   - *Bounce* → tut;
   - *Step left/right* → glide;
   - *Jump* → Halo. The Halo is drawn from the move's progress, so it stays a pure function
     of the beat.
7. **Cost, estimated.** It has not been measured (that is the builder's first job,
   `cast_bench`):
   - about 12 suit fills;
   - 9 neon fills (three colours × three layers);
   - up to 8 for trails;
   - about 30 fills in all, one large translucent.

   That is under the cast's 52–113, so well within 1.5 ms if the halo layer stays thin.
8. **Tests:**
   - the chroma key: every colour is ≥ 78 from a key; the nearest is the Paper line, 78
     from blue;
   - the contrasts above;
   - nothing outside `bounds()` (the floor ring and the trails must be inside);
   - the Halo deterministic at any frame rate.

## Risk

It is close in spirit to a famous film. The design holds only if it keeps the cast's colours
and its own motifs; the owner should look for anything that reads as borrowed.
