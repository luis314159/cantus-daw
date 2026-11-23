# ADR 0126 — The instruments' scenes drawn as projected vectors

**Status:** Proposed for round 40 phase 3 (contract; builder `scene`) · **Date:** 2026-11-12
· **Supersedes:** ADR 0104 §1 (the baked CPU scene renderer); ADR 0104 §2 (the stored look:
piano type, finish, room — stored with the instrument, never heard) **stays** · **Builds
on:** ADR 0024 / 0111 (the hand-drawn kit, the design system), ADR 0107 (memory), ADR 0110
(the frame budget) · **Used by:** ADR 0124 (Tympanum's kit in its room), ADR 0125 (Chorda's
source and mixer) · **Design:** `F:\projects\daw-r40\design-p3\` — the generator `src/`
(`geo.py`, `shade.py`, `piano.py`, `pieces.py`, `hardware.py`, `draw_room.py`, `scene.py`),
`kit-spec.md`, `kit-model.json`, the designer's `port-notes.md` (to follow)

## Context

ADR 0104 drew Chorda's piano in its room with a baked CPU rasteriser
(`views/inst/chorda_scene_{raster,shade,room,gear,geo,mesh,piano,view}.rs`): meshes shaded
and rasterised on a worker into an image, shown and re-baked when the look changes. Its
peak while baking is about 180 MB, and it is a picture, not a place: nothing in it can be
clicked.

For round 40 phase 3 the designer drew Tympanum's kit in its room and Chorda's new panels
from a **3-D model in centimetres, projected and drawn as vector paths** (camera,
projection, facet lighting, depth sort). The owner compared both (2026-11-12) and chose the
designer's: *"vamos frescos con el diseño mejor"* — for Chorda and for Tympanum.

## Decision

1. **A scene module** in `crates/cantus-ui/src/scene/**`, owned by the builder `scene`: a
   port to Rust of the designer's generator, drawing **through the kit's painter** (no
   stock egui widgets, no literal colours — the palette from the theme's tokens, Night and
   Paper, the scene's materials as the kit's named scene colours; no `FontId::new`).
   The pipeline: a 3-D model in cm → the camera's projection → facet lighting (a key light,
   strip lights, a reflected environment) → depth-sorted vector paths.
2. **Live and interactive**: a `pick(point) → Option<Picked>` (a piece, an articulation
   zone, a mic, the lid) from the same projected geometry that is drawn, so what is clicked
   is what is drawn; the lid and the mics animate; the projected paths are **cached per
   frame state** (camera, model, theme, size) so an unchanged scene costs a lookup, inside
   the 60 Hz frame budget (ADR 0110), and **a scene at rest asks for no repaint**.
3. **Both instruments**:
   - **Chorda** — piano types concert / grand / parlour / upright (ADR 0104 §2's `look.body`),
     finishes including maple, the lid, the perspective mics (ADR 0125's *Hammers, Player,
     Audience, Room*), the room pair, rooms booth / studio / hall / church, evening and
     daylight following the theme — ADR 0104's ideas, redrawn. A recorded pack shows its
     piano type (`chorda_source::scene_piano`).
   - **Tympanum** — the kit's pieces, stands, pedals and materials (maple shells, coated
     heads, chrome hardware, lathed bronze cymbals) from `KitDef` (ADR 0124: the pieces'
     `Place`, diameter and depth in cm), the mics where they stand with the selected one's
     pick-up cone, the rooms Small / Studio / Hall.
4. **The switch**: `scene` moves Chorda's view onto the module and **removes the baked
   renderer** (`chorda_scene_{raster,shade,room,gear,geo,mesh,piano}.rs` and what only they
   use) once the module draws every variant the old one did; the bake's ~180 MB peak goes
   with it — measured before and after with the memory map (`tools/scorecard/memmap.ps1`,
   a map, not the scorecard).
5. **One owner**: `tymp-ui` and `chorda-src` call the module and never edit it; what they
   need goes to `scene` through the integrator.

## Consequences

- ADR 0104 §1 is superseded; its stored look (§2) and the commands that set it are kept,
  so no song changes (no schema).
- Chorda's scene goldens change **once, on purpose**, in one commit with the before/after
  shots; every other golden stays.
- The design files are the geometry's spec: a dimension in the Rust model that differs
  from `kit-model.json` is a bug unless the port notes say why.

## What the tests hold

- Every variant drawn (each piano type × finish × lid × room × theme; a modelled and a
  recorded kit in each room), finite and inside its rect, nothing overlapping its labels.
- The frame budget (`chorda_scene_budget.rs`'s successor) at 1× and 1.5×; a scene at rest
  requests no repaint.
- `pick` returns the piece, mic or lid under a point for every one drawn, and nothing in
  empty room.
- Legibility (ADR 0111): what can be clicked ≥ 3:1 against what is behind it, in every
  theme.
- The memory map before/after the baked renderer's removal, in the builder's report.

## Amendment — round 40 phase 3, builder `scene` (2026-11-12)

The owner's and the integrator's decisions of the same day, and what the port found.

1. **How the display list is drawn.** The scene module builds the designer's display list
   (`display.rs`: polygons, lines, polylines, ellipses; flat, linear and radial paints; by
   layer and depth) and **rasterises it with `tiny-skia`** at the screen's pixels (`draw.rs`)
   into one texture per scene state, as ADR 0024's cached layers are drawn — not egui
   meshes. egui's meshes have no per-pixel gradients, no concave fill and no clip, and
   adjacent antialiased facets seam; the soft layers (the far walls, the floor's
   reflection with its fading mask, the shadows' penumbra, core and contacts) need a raster
   anyway. `tiny-skia` is the same antialiasing and gradient model as the browser that drew
   the designer's sheets, so the picture matches them pixel for pixel in structure. The
   soft layers are drawn apart and blurred (three box passes, SVG's `feGaussianBlur`
   recipe); bands of rows are drawn in parallel.
2. **When it is drawn.** A `scene::Scene` (owned by the caller, one per instrument) builds
   on a worker thread whenever what it shows changes — the subject, its controls, the
   room and its look, the theme's mood, the size, the pixels a point — the newest asked
   wins, a stale build is dropped, and the picture lands whole. The live controls (lid,
   Perspective, Width, Room level) glide to where they are set. **At rest** a frame paints
   one cached textured quad (and the selection's cached mesh): no repaint asked for, no
   build, no allocation (`tests/scene_vector.rs`). The pick answers from the build shown.
3. **The backend is configurable** (the owner's decision): `SceneBackend::{Vector,
   PerPixel}` (`scene/backend.rs`). Vector is the default (fast, light). A per-pixel
   renderer (builder `scene-px`, ADR 0104's raster grown up) implements `PixelBackend`
   and registers once with `register_pixel`; it receives the scene's `Model` — the same
   3-D primitives (facets, tubes, flat polygons, each with its material and its part), the
   fitted camera, the light, the room and its look — and returns the pixels. The camera,
   the pick and the cache stay this module's, so what is clicked is what is drawn in
   either backend. The choice is an app setting (*Settings ▸ Appearance ▸ Instrument
   pictures: Drawn · Photographic*), never the project's. ADR 0104's renderer is **not**
   removed: it is `scene-px`'s.
4. **The room look is stored (schema 69).** `look.room` (stage, booth, studio, hall,
   church), `look.floor`, `look.walls`, `look.rug` — text keys beside the piano's look
   (`cantus_core::inst::room_look`), named as the look in the history, held by no frozen
   sound, never heard. A room absent is the room an older song's Room size drew; a new
   Chorda writes the stage (delivery 6's default, the owner's choice). A floor, walls or
   rug absent is the room's own. Maple joins the piano finishes.
5. **Where the port goes past the sheets** (the owner's bar: *the lid a solid glossy black
   panel reflecting the stage, never translucent; long crisp lacquer highlights; a cleaner
   floor reflection*): a lacquer reflects the floor's pool of light where its reflection
   meets the floor and the two tall strip lights as crisp streaks (sharp across, long
   along); the lid is drawn in strips across its length, each shaded at nine points along
   it, so what it reflects moves over it as over a mirror (the plate's tinted reflection
   is gone); the floor's reflection is blurred a third as much. Seen from below, the lid
   mirrors what it opens over: where the eye's ray reflected off it lands inside the case,
   it shows the gold plate as the key lights it there (a soft edge, Fresnel toward
   grazing) — solid black lacquer, never a translucent tint. A broad flat lacquered face
   (a box's side, the key slip, the fallboard, the lid's edge) is shaded without the strip
   lights' streaks and gets them as a band across itself sampled at 25 points, so a
   streak lands as a crisp highlight where the reflection lines up instead of lighting the
   face whole (the key slip had turned white). A floor or a wall that runs past the
   camera is cut at a near plane before it is projected (a floor whose corner came within
   a few centimetres of the eye was thrown off the picture and dropped whole by the
   rasteriser: the Paper stage's boards showed the sky between them).
6. **Framing.** The play view fits the piano into the designer's `chorda_play.stage` box
   (insets 40 / 50 / 40 / 62); the picture-alone sheets' wider box (30 / 40 / 30 / 40)
   is `compose::PIANO_PICTURE_FRAME`, used by the screenshots compared with them.
7. **A per-pixel picture while a control glides** is asked for `quick` (`PixelBackend::
   render(model, ppp, quick)`), and once at rest again in full; the vector picture is the
   same either way, so it is never built twice.

## Amendment — round 42, `arcus-look`: Arcus's cello and double bass (2026-11-22)

The owner found Arcus's first picture ugly (*"Mejóralo, se ve muy feo"*: a flat cello lying
sideways) and asked for Chorda's and Tympanum's class — and for **the same engine**
(*"Usa el mismo motor gráfico de Chorda y Tympanum"*). So the bowed strings are the
engine's third and fourth subjects, added beside the piano and the kit, never a renderer of
their own:

1. **`Subject::Bowed { instrument }`** (`scene/mod.rs`), framed by `BOWED_FRAME`, built by
   `bowed_draw::bowed_scene` as `piano_scene` and `kit_scene` are (the room from `rooms`,
   the floor's mirror, the key's shadows, the falloff, the per-pixel model kept when the
   backend asks), and baked per pixel by the photographic renderer's own arm
   (`chorda_scene_hybrid.rs` → `chorda_scene_px_bowed.rs`). Settings ▸ Appearance ▸
   Instrument pictures switches it with the others.
2. **The geometry is the trade's**, renderer-neutral (`scene/bowed{,_mesh,_head,_fit}.rs`):
   the outline as three curves meeting at the corners from the measured Stradivari
   'Davidov' (757 mm; 340 / 232 / 436 mm), its low arching and purfling 4 mm in (Hargrave,
   *The Strad*, 2001), a 3/4 orchestral bass's measures, the standard set-up (stop, neck,
   overstand, bridge, afterlength, clearances), carved f-holes, the bridge with its heart
   and kidneys, the scroll as a logarithmic volute of two and a half turns, pegs or
   machines, endpin, strings. Built once per instrument (`bowed_mesh::mesh`); both pictures
   draw the same surfaces.
3. **What the engine gained, for any subject**: `Part::Bowed`; the per-pixel texture
   `Tex::Tonewood` (a quartered tonewood's grain and maple's flame); in the Drawn picture a
   cell shaded at both its row edges (a gradient, not a flat facet) and decals drawn into
   the cell they lie on (so a far cell never covers an f-hole or the purfling). Chorda's and
   Tympanum's pictures are untouched to the pixel (`tests/scene_identity.rs`, their hashes
   measured on the tree before).
4. **Live, over the picture**: what Arcus plays (the sounding string lit and swinging, the
   stopping finger with the vibrato, the bow at its contact with its pressure, camber and
   travel, pizzicato's finger, tremolo) is drawn each frame through the kit's brush from the
   build's camera and `bowed::rig` — no rebuild while playing; at rest one textured quad.
