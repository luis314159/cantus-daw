# ADR 0104 — Chorda's scene: the piano in its room, and its look

**Status:** Accepted · **Date:** 2026-11-06 · **Extends:** ADR 0024 (the hand-drawn kit),
ADR 0074 (unit faces: the scene is a display), ADR 0098 (Chorda model 2 and its
perspective controls), ADR 0004 (commands) · **Schema:** 58 · **BACKLOG:** "Chorda's look
per preset"

## Context

The owner (2026-11-06): *"imágenes de pianos de los presets de fondo para que se vea
diferente, y la sala y micros … muy pros para que se vean los cambios (tipos de pianos,
colores, formas etc.) … para que se vea un piano súper profesional y la sala de grabación y
todos los elementos."* Chorda's Play view showed the strings from above — an exact technical
picture, but not what a premium sampled-piano library shows: *the* piano, in its room, with
its microphones. Model 2's perspective controls (ADR 0098 §6) — the microphones, the pair's
width, the lid, the room's level and size — had no picture at all beyond a diagram.

What had to hold: hand-drawn vectors, no bitmaps and no literal colours (ADR 0024); both
themes; crisp at 100–200 %; the frame budget, no allocation per frame; the picture must
never claim a piano the sound is not; whatever is stored is a model change done as one
(schema, migration, command, undo fuzz) and is never heard.

## Decision

### 1. The scene is built in 3-D and baked by our own renderer

The first delivery drew the scene as flat vertex-lit vector shapes; the owner, shown it,
asked for it *"más real, que se vea mucho más profesional"* — the bar is the product
pictures of the premium sampled-piano libraries: a photographed piano in a real studio,
not an illustration. Vertex colours on a few thousand polygons cannot carry wood figure,
the room reflected in lacquer, soft shadows or a lamp's bloom inside the frame budget. So
the scene is **baked**: rendered off the UI thread by a small CPU renderer of our own into
an image, shown as one textured quad. Every pixel is computed by this code from geometry
and procedural materials — no photograph, model, texture file or stock asset — which keeps
ADR 0024's rule in its spirit (*ours, drawn by code, crisp at the screen's own pixels*).

- **Geometry** (`chorda_scene_{piano,room,gear}.rs` into `geo.rs`): triangles in metres
  with smooth normals, texture coordinates and a material, and thin lines. A world with
  y up and the floor at 0; the keyboard's front at z = 0, the bass at x = 0 (a grand's
  spine, where its lid is hinged). A grand's outline is one function, `edge_x(z)` (a
  straight run, a smoothstep's convex shoulder and concave waist, a quarter-ellipse tail:
  concert 2.74 m, grand 2.00 m, parlour 1.55 m) raised into the rim's outer and inner skins
  and its top; the soundboard; the cast frame with its holes, rim, struts, pin field and
  144 tuning pins; 144 tenor/treble strings and 20 over-strung copper basses on their
  bridges; dampers on their heads to E6; **88 keys, each a bevelled box** (the gaps are
  real, so the occlusion shades them); key slip, cheeks forward of the keys, fallboard
  with a brass emblem, the red felt strip; the desk leaning back, its panel pierced with
  fretwork; tapered legs with collars, brass ferrules and casters; lyre and pedals; and the
  **lid as a solid slab** — top, underside, edge — at the Lid control's angle (0 … 36°:
  closed, the short stick to 20°, the full stick), its front flap folded on it, brass
  hinges, and the stick leaning from the rim's ledge to the underside a hand inside its
  edge. An upright: the carcass recessed between side cheeks with beaded mouldings, a
  plinth, the raised upper panel and the lower panel's frame, toes and turned columns,
  keybed, key slip and arms, the sloping fallboard with its lip and the music ledge, the
  top lid as a slab overhanging the case (0 … 24°), showing pins and hammers when up.
- **Materials and light, per pixel** (`shade.rs`): procedural textures — wood with its
  rings, figure and pores (the finishes, the floor's staggered boards with their joints),
  the rug's border chain, medallion and weave, fabric, plaster, limestone courses and
  flagstones, the frame's cast gold, the microphones' brushed bodies and grille mesh; a
  key **spot** light with a soft **shadow map** (PCF), a sky-and-floor fill darkened by
  **ambient occlusion** from the depth buffer, a rim light, Blinn highlights, and
  **reflections traced against the room itself** — the softbox, the floor and rug, the
  walls — so the lacquer shows the room it stands in (a lid's underside, facing the floor,
  mirrors it only dimly: a lacquered board, never smoked glass); metals tint what they
  reflect. Lamps, windows and LEDs are light.
- **The renderer** (`raster.rs`): a depth-buffered rasterizer (rows shared among worker
  threads) writing which triangle each sample sees and where, two samples a pixel side;
  every sample shaded; thin lines (the strings) drawn anti-aliased against the depth; a
  **polished floor's mirror image** (a second pass, blurred); **depth of field** behind the
  instrument (focus to its farthest corner: the piano is sharp in every preset); **bloom** round what is brighter than white; a filmic tone curve (ACES fit), a
  vignette and a dither; averaged down to the image's pixels.
- **The microphones** (`gear.rs`): two small-diaphragm pencils on a stereo bar (brushed
  body, grille, clip; a red tally ring on the pair the mix hears) on a stand with tripod,
  mast, boom and counterweight — placed by `persp`: over the hammers (0, spaced A–B across
  bass and treble), at the player behind the bench (0.35), out where the audience sits (1),
  gliding between; `width` spaces and splays them. A **room pair** of large-diaphragm
  condensers in shock mounts on tall stands stands far back when `room` is above 3 %.
  Every cable runs down its stand, across the floor and **out of the frame**.
- **The room** (`room.rs`): two walls meeting behind the piano, `roomsize` setting their
  distance (0.9 … 8.9 m behind the spine, 0.9 … 10.9 m behind the tail) and height; a
  **booth** (plaster, fabric absorbers, a quadratic-residue diffuser, a glowing wall lamp,
  a rug), a **studio's live room** (wood slats, the control room's window with its
  console's lights, absorbers, a rug), a **hall** (tall wooden fins, uplights, a polished
  stage that mirrors the piano), a **church** at size 0.95+ and more than 85 % live
  (limestone, lancet windows of light, flagstones). The room's level thins a small room's
  absorbers and polishes the floor. A bigger room is seen from lower down, so its walls
  rise into the frame.
- **The themes**: the scene is a display (ADR 0074 §7) and keeps its own ground in both;
  the theme chooses its light — **evening** in a dark theme, **daylight** in a light one.
  The palette (finishes, parts, gear, rooms, the two lights) is data in
  `kit/skin/scene.rs`; the scene's files name no colour (they are in the views' lint).
- **Live, and the frame budget**: each Chorda has its own scene (by track: one piano is
  never shown on another), its worker baking one request at a time, the newest. A picture
  where there is none comes as a **preview** first (half the pixels, one sample, no
  occlusion/mirror/depth of field/bloom), as do a gliding control's (time constant 70 ms);
  once it rests, the **full bake** replaces them. The worker **wakes the UI** when a bake
  lands (`request_repaint`), so the UI never repaints to poll; a bake that lands for a look
  no longer wanted (a change undone while it baked) is dropped, never shown. A new size is
  baked once it has held for a frame, and that frame is asked for. The UI thread never
  renders: a frame at rest paints the cached textured quad (an `Arc` clone), its card's
  words cached by what they say, and allocates next to nothing (tested over the whole
  card). **Bounds**: no side past the GPU's largest texture (`max_texture_side`), and at
  most **3 000 000 samples a bake** (`scene::MAX_SAMPLES`; a bake's peak is about 45–60
  bytes a sample, so about 180 MB — a larger picture, a multi-4K window, is baked smaller and
  drawn stretched; two samples a pixel side drop to one first). Times and sizes are in
  `docs/round37/scene.md`. Screenshots bake on the painting thread
  (`scene::set_blocking`), as they cannot wait.

### 2. The look is stored with the instrument, as text, and is never heard

`cantus_core::inst::chorda_look`: `look.body` (`concert`, `grand`, `parlour`, `upright`)
and `look.finish` (`black`, `white`, `walnut`, `rosewood`) — two **text** keys of Chorda.
Text keys are not parameters: no knob, no automation lane, no modulation destination (only
numeric keys are), and the engine's `configure` reads only the parameters and the
modulation routes. A key absent, or a word this build does not know, reads the default (a
black concert grand). **Schema 58**'s migration writes `concert`/`black` into every Chorda
saved before (a look already there is kept), so a later default never changes an old song's
picture; `fixtures/v58.cantus` holds a walnut upright. Setting it is the existing
`SetInstrumentText` command (exact inverse: the previous text), named *Change the piano's
look* in the history (`cmd.chorda_look`); the undo fuzz has a stream of its own for it. The
look is **no part of a frozen sound**: a freeze's content key leaves Chorda's `look.*` text
out (`freeze::track_part`), so schema 58's migration keeps every older frozen key, and the
edit lock lets the pickers change the look of a Chorda inside a frozen region (a change to
its sound is still refused — `cantus-core/tests/chorda_look_freeze.rs`).

### 3. Honest: each factory preset declares a piano its sound can stand for

Chorda models one grand's strings (Bensa et al.'s measured C2/C4/C7, about a two-metre
grand's scale) voiced on a recorded Yamaha C5 (ADR 0098); it has no upright's soundboard.
So the body is not derived from the sound — it is declared per preset (in its text, beside
its macros), and `chorda_look::fits(body, params)` says which bodies a sound can be shown
as, from what the model really uses:

| Body | Its sound must have |
|---|---|
| concert grand | a long scale (stiffness ≤ 1.05) and a grand's unisons (≤ 4 ct) |
| grand | stiffness ≤ 1.15, unisons ≤ 4 ct |
| parlour grand | stiffness ≤ 1.25 (a shorter scale), unisons ≤ 4 ct |
| upright | the case shut over the board (lid ≤ 0.5 — the closed lid's shelf) **and** an upright's mark: short-scale stiffness (≥ 1.25), a honky-tonk's detuned unisons (> 4 ct), or the felt rail's soft, noisy action (hardness ≤ −0.6, noise ≥ 1.2) |

The 24 factory presets use all four bodies and all four finishes (Felt: a black upright;
Honky-tonk: a walnut upright; Old upright: rosewood; Baroque: a rosewood parlour grand;
Ambient: a white concert grand …); a test holds every one to `fits`. The user may pick any
look from the scene's two pickers (*Piano*, *Finish*) — it is their picture; the scene's
hint says the look changes the picture only, never the sound.

### 4. Where it is

The Play view's hero card, *The piano and its room* (70 % of the top row; beside it the
pedals and, under them, model 2's *Microphones and lid* — the controls that move the
picture most); the pickers over its top-left corner; the screen reader reads what it shows in
words (*Concert grand, Black lacquer — the lid on the full stick, the microphones at the
player, in a studio's live room*); the tuning's progress, while one is designed, on a small
plate at its foot. The strings from above stay in Edit (*Strings and hammers*).

## Consequences

- Every preset now looks different, and every perspective control has a visible meaning.
- The picture is a drawing of *a* concert grand, grand, parlour grand or upright, not of a
  maker's model; no brand, no trace of anyone's artwork.
- The look is a model change for a picture: one schema number, one migration, two keys.
- What only the owner can judge: whether it reads as first-rank artwork beside the premium
  libraries' GUIs (`docs/round37/scene/` has every preset in both themes, the controls at
  their ends and middle, and 100/150/200 %).

## What the tests hold

| What | Test |
|---|---|
| The look is text, never a parameter; unknown words read the default; a default Chorda is no upright | `cantus-core` `inst::chorda_look::tests` |
| Old songs open as a black concert grand, said explicitly; v58's look survives save and load; a look there is kept | `cantus-project/tests/chorda_look.rs` |
| Look commands undo and redo among the others; refused on an instrument without it | `undo_invariants::a_chordas_look_undoes_and_redoes_among_the_others` |
| Every factory preset declares a look that fits its sound; all bodies and finishes used | `cantus-ui/tests/chorda_scene.rs::every_factory_preset_shows_a_piano_its_sound_can_stand_for` |
| Through `CantusApp::assemble`: Lid, Mics, Width, Room size and level move the scene; a preset changes the body; a picked look is one undo step; no rebuild in steady state | `chorda_scene.rs::turning_lid_mics_and_room_moves_the_scene_and_a_preset_changes_the_piano` |
| The null test: the look changed twice while the piano sounds live equals the offline render with no look, to the bit, and other looks render the same | `chorda_scene.rs::the_look_touches_no_sample_live_or_offline` |
| The frame budget: no allocation beyond an empty frame at rest and while gliding (bake uploads aside), no request at rest, the whole card at rest; triangle bound; the bake's bounds (texture side, samples); a late bake for an undone look never shown, the worker waking the UI; a new size baked with no event; the bake's time (release, ignored) | `cantus-ui/tests/chorda_scene_budget.rs` |
| The look is no part of a frozen sound: allowed on a frozen Chorda, the key kept; its sound still refused | `cantus-core/tests/chorda_look_freeze.rs` |
