# Round 42 — `arcus-look`: Arcus's picture on the scene engine

**Branch:** `r42/arcus-look` (from `r42/arcus` 58b7a0006) · **Builder:** design agent for
Arcus's picture · **ADR:** 0126's amendment (round 42, `arcus-look`) · **Sheet:**
`docs/round42/arcus/sheet-v3/`

## Why

The owner looked at Arcus's Play view (sheet v2: a flat cello lying sideways, a plain
gradient, S-strokes for f-holes) and said *"Mejóralo, se ve muy feo"*. His bar is Chorda's
and Tympanum's pictures: a real instrument in a real room, in two looks. He also said
*"Usa el mismo motor gráfico de Chorda y Tympanum"*. So the cello and the double bass are
meshes and materials for that engine, not a new renderer.

## What was built

- **The instruments, from luthiers' geometry** (`scene/bowed.rs`, `bowed_mesh.rs`,
  `bowed_head.rs`, `bowed_fit.rs`). The meshes don't depend on either renderer and are built
  once per instrument.
  - The outline is three Catmull–Rom curves (the lower bout, the C-bout, the upper bout) that
    meet at the corners with the corners' own angle.
    - The cello is the Stradivari 'Davidov' (1712, B form): 757 mm long; 340 / 232 / 436 mm
      across the upper, middle and lower bouts; a 690 mm string. Its low, full arching
      (about 25 mm) and its purfling 4 mm in are from R. Hargrave, *The Strad*, Dec. 2001.
    - The bass is a 3/4 orchestral bass as makers publish it: 1 100 mm long, about
      490 / 370 / 650 mm across, ribs about 200 mm, its shoulders sloping into the neck, a
      flat back bent toward the neck.
    - Measures only, no drawing copied. The sources are cited in `bowed.rs`.
  - The arching rises from the edge's thickness through the channel to its plateau.
  - The ribs are set in under the overhang, and both plates have their edges.
  - The f-holes are carved: eyes, an S stem, pointed wings, the nicks at the bridge's feet,
    the pale cut round the dark inside.
  - The bridge has its feet, legs, kidneys, heart and the arc that carries the strings.
  - The neck carries the board's overstand, so the nut sits behind the top's plane. Its heel
    goes down to the button.
  - The ebony fingerboard has an arched top, sides, end and underside. There is a nut.
  - The pegbox is open, with cheeks.
  - **The scroll is a logarithmic volute** of two and a half turns. Its back is fluted beside
    the spine, its sides step in turn by turn, and it ends in an eye.
  - The cello has ebony pegs with collars. The bass has brass machine plates and keys.
  - The tailpiece has a fret and four fine tuners (the cello's).
  - There is a saddle and a tailgut, and an endpin with its collar and a rubber rest on the
    floor.
  - The strings are steel or tungsten in their silks.
- **Both looks.**
  - *Drawn*: each cell is a facet lit by the engine's one shading function. The varnish has
    a clear coat, the strip lights' streaks and the flame's figure, plus the spruce's fine
    grain lines.
  - *Photographic*: the per-pixel renderer bakes the same cells as smooth triangles with
    `Tex::Tonewood`. That is a quartered grain, plus maple's flame on the back, ribs, neck
    and scroll, under a saturated orange-brown varnish. The room's light, the soft shadows,
    the traced reflections and the floor's mirror are Chorda's.
  - The setting is *Settings ▸ Appearance ▸ Instrument pictures*, as for the other
    instruments.
- **Posed as played.** The instrument stands on its endpin in the studio's live room
  (Chorda's and Tympanum's room, rug and walls). It is leant back and its scroll tipped
  toward the viewer's right, seen three-quarter from the audience's left and a little above.
- **Live, over the baked picture** (`views/inst/arcus_draw_body.rs`, from the build's camera
  and `bowed::rig`):
  - the sounding string, lit, with its swing as loud as the level;
  - the stopping finger where the note stops the string, rocking with the vibrato;
  - the bow at its real contact (β of the stopped length):
    - tilted over the bridge's arch to reach its string alone;
    - its hair a ribbon bent at the string and darker where it grips, both as hard as it
      presses;
    - the stick's camber, the frog with its ferrule, slide and pearl eye, the head with its
      tip plate;
    - sliding along its hair as it moves, with a blur behind it as fast;
  - pizzicato's finger pulling the string where the settings pluck;
  - tremolo's short strokes.

  Dragging still moves the contact (across) and the force (up and down), one undo step per
  drag.
- **The Play layout** (`arcus.rs` `play()`, with the integrator's agreement):
  - the picture takes the whole left column;
  - the read-out (note, string, dynamic, contact, force, bow speed) is a two-line caption
    on a plate under the endpin, in a band the frame leaves free at every scale;
  - the Instrument, Vibrato, Body and Output cards run down the right;
  - Edit keeps its read-out beside the Schelleng diagram.

## What the engine gained (additive)

- `Subject::Bowed`, `Part::Bowed`, `bowed_draw::frame_for`.
- The per-pixel `Tex::Tonewood`.
- In the Drawn path for this subject:
  - cells shaded at both row edges as a gradient;
  - decals sorted just nearer than the nearest cell of the top they cover.
- The photographic arm `chorda_scene_px_bowed.rs`.
- The palette `kit/skin/scene_bowed.rs`.

**Chorda's and Tympanum's pictures are unchanged to the pixel.** `tests/scene_identity.rs`
hashes five pictures in each look (concert, upright and grand pianos; the kit on the stage
and in the studio; Night and Paper). The hashes were measured on 58b7a0006 with the same
file. They hold on Windows, where they were measured; elsewhere the test only checks that
each picture is drawn.

## Measured

| What | Value |
|---|---|
| A frame, Arcus live (string, finger, bow) ÷ Chorda's picture at rest, timed beside it, best of 5 | 2.98 – 3.3 × (the test holds it under 6 ×) |
| A frame, best of 30 (debug build) | Arcus live 0.089 ms · Chorda at rest 0.025 ms |
| Builds while a note plays | 0 (only what it plays is drawn) |
| At rest | no repaint, no rebuild, no allocation beyond an empty frame (19 = 19) |
| A picture built (on the worker, debug build, 590 × 300 at 1×) | Drawn ≈ 0.3 – 0.5 s, Photographic ≈ 0.2 – 0.3 s; at 3× ≈ 0.8 / 1.3 s |

## Tests

- `tests/arcus_scene.rs`:
  - every room × theme × instrument draws finite and inside its picture, and leaves the
    caption's band free;
  - the pick finds the instrument and nothing in the empty room;
  - at rest there is no repaint and no rebuild;
  - the frame-cost ratio.
- `tests/scene_identity.rs`: Chorda's and Tympanum's pixels.
- `tests/fx_lint.rs` now walks the new files.

## Verified only on screen

The pictures were looked at in `docs/round42/arcus/sheet-v3/` (100 % and 200 %, Night and
Paper, English and Spanish, and the frame strips). The owner's verdict is pending.

## For the integrator

- The merge with `r42/arcus` touches `arcus.rs` (`play()`, the caption, `readout_rows`, and
  `instrument_card`'s `with_readout` flag), `arcus_draw.rs` (`picture`, `scene_view`,
  `picture_for_tests`) and `arcus_draw_body.rs` (rewritten).
- The picture's API is the same `draw::picture(app, ui, track, &fs, &tele, d, s)`. It uses
  only the telemetry the builder already sends (`KEY`, `STRING`, `BETA`, `FORCE`, `SPEED`,
  `LEVEL`, `FMIN`/`FMAX`, `VIBRATO`, `ALIVE`) and `fs.pizz_pos` / `fs.trem_rate`. No new
  telemetry is needed.
- `InstEditors` gains `arcus_scenes`, pruned as the other scenes are.
