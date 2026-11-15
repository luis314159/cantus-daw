# Round 40 phase 3 — builder `scene-px`: the photographic instrument pictures

ADR 0104 (the per-pixel renderer, grown up), ADR 0126 (the scenes; the backend). Branch
`r40/scene-px`.

The owner's decision (2026-11-12): the instruments' pictures are configurable —
*Settings ▸ Appearance ▸ Instrument pictures: Drawn* (the scene module's projected vectors,
the default) or *Photographic* (this renderer) — and the photographic one is polished
"hasta que sea excelente". It draws **the designer's scenes**: the scene module's subject
(Chorda's four bodies in every finish with its lid, Tympanum's kit and its microphones), its
rooms (stage, booth, studio, hall, church) with their floor, walls and rug, its camera
fitted exactly as the vector picture fits it, its moods (a dark theme's evening, a light
one's daylight).

## What it is — `crates/cantus-ui/src/views/inst/`

| File | What |
|---|---|
| `chorda_scene_hybrid.rs` | The renderer's front: the light rig, `render` (a `SceneView`), `render_of` / `render_model` (the scene module's `Model`), `Photographic: PixelBackend`, `register()`, the bake's bounds (`shape`, `MAX_SAMPLES`, `MAX_SIDE`), `FULL` / `PREVIEW` |
| `chorda_scene_px_geo.rs` | Closed meshes in metres with smooth normals and uv: plates (ear-clipped), walls, rounded edges, tubes, lathes, boxes; wires |
| `chorda_scene_px_piano.rs`, `_upright.rs` | Chorda's grands (the designer's outline per body, the rim with rounded edges, a lid with a real thickness, the cast frame with its holes and struts, strings, pins, dampers, keys, desk, bench) and the upright |
| `chorda_scene_px_kit.rs`, `_gear.rs` | Tympanum's kit from the scene module's pieces (`kit-model.json` or the kit's definition): shells, hoops, lugs, coated heads, the kick's port, lathed cymbals, every stand; the microphones and their stands |
| `chorda_scene_px_room.rs` | The rooms from `scene::rooms` and the look (floor, walls, rug), their furniture and lamps |
| `chorda_scene_px_mat.rs`, `_shade.rs` | Materials (clear coat, metal, anisotropic metal, matte, emitter) and per-sample shading: procedural wood, cast iron, spruce, floors, rugs, walls, curtain, heads, lathe grooves |
| `chorda_scene_px_light.rs` | The light: the key softbox, the panels (strips, overhead, bounce, wash), the room's environment, the lamps |
| `chorda_scene_px_trace.rs` | **True reflections**: a bounding-volume hierarchy (binned SAH) over the instrument's triangles (borrowed, not copied) and its wires as ribbons; a gloss sees what stands in its mirror direction, lit once |
| `chorda_scene_px_raster.rs` | The bake: depth-buffered rasteriser, PCSS soft shadow of the key, screen-space occlusion, the floor's mirror, the shading pass, wires, depth of field, bloom, a hue-keeping filmic curve, vignette, dither; bands over a thread crew, the bits independent of the thread count |
| `chorda_scene_px_model.rs` | The `Model`'s captured primitives as triangles (facets fanned and welded smooth, tubes round), for a model without its subject |
| `chorda_scene_mesh.rs` | The shared vector maths and camera (also the old renderer's; mounted by `hybrid` itself) |

`render_of` bakes a `Model`: from its subject with this renderer's own meshes and **the
scene module's fitted camera** (so the framing is the vector picture's, whatever frame the
caller asked for), or from its primitives alone (`render_model`).

The setting: `app.rs` registers the renderer in `CantusApp::assemble`, right after the
scene module applies the setting (`hybrid::register()`, one line marked `r40:scene-px`).
The scene module asks for a `Model` only when the setting is *Photographic* and a renderer
is registered; the bake runs on the scene module's worker, never the UI thread.

## The light, as a photographer lights a black grand

- **Key**: a softbox high on the treble side (for a grand: 3.2 m right, 3 m up, level with
  the keyboard), so its light goes under the open lid — the harp is lit, and the lid's
  underside mirrors it (the frame, the strings, the pins: true reflections). Its soft
  shadow is the only cast shadow (PCSS: contact-hard, wide where the occluder is high).
- **Floor-standing strips** out of frame, placed by their angle round the instrument from
  the camera: the lacquered sides only see what lies between them and the floor, so the
  strips stand close and reach the floor — the long bar down the tail (the owner kept it:
  "se ve muy chido"), the bars on the cheeks, the lines along the lid's and rim's edges.
  Each softbox's face falls off from its middle (`Panel::core`) so a reflection never
  clips into a flat block (measured: the tail bar peaks at 250–251 of 255, no clipped
  pixel, at 1×, 1.5× and 2×, Night and Paper).
- **A big soft panel in front** that the desk and the fallboard show as a gradient; a
  **broad overhead softbox** for the top edges; a **cool rim** from behind (on the
  instrument only — never the floor or walls); a **warm bounce** board; an **unseen warm
  wash** over the stage so the floor reads out of the dark; above the stage a true **void**.
- **Finish**: a filmic curve on the luminance keeping the hue (highlights roll to white),
  a brightness-weighted (Karis) resolve so a one-sample glint is a fine line, not a bead;
  soft bloom only over 2.4× white; the floor mirror without fireflies.

## The upright and the bench

The upright's carcass is open at the top: a dark well behind the front board with the gold
frame and pin block along the back, two rows of tuning pins, the strings running down, the
dampers' white felt and the hammers' heads in a row just over the front board (what the
drawn upright shows). The bench takes the piano's finish (rosewood, walnut, maple, white,
black) with a satin top: it is sat on, not polished.

## Materials

Clear-coat lacquer (Fresnel over the colour, the finish's roughness), wood figure (sliced
veneer, pores only where a pixel is finer than they are), the harp's gold paint on cast
iron, spruce, chrome (mirror, a touch soft), nickel, brass, lathed and hammered bronze
(anisotropic), coated heads (cloudy coating, collar), the kick's front head in dark satin
with its ring and port, felt, leather, floors (boards with joints filtered to the pixel,
slabs, terrazzo, flagstones), walls (plaster, panels, acoustic tiles, brick, limestone,
velvet curtain), rugs (border, motif, weave).

## The audio first: the bake's pool, its priority, its cache

- **One pool for the app's life** (`chorda_scene_px_pool.rs`): made at the first bake,
  sized to the physical cores less two (one for the audio, one for the interface), at
  most six; let go 30 s after its last bake by a watcher that then ends — nothing of it is
  left at rest. A bake runs wholly on it; the scene's worker only waits.
- **Below normal priority**: the pool's threads call
  `cantus_rt::thread::lower_background_thread()` as they start (new in `cantus-rt`, marked
  `r40:scene-px`): Windows `THREAD_PRIORITY_BELOW_NORMAL`, Linux a nice of +5, macOS the
  *utility* QoS class; never MMCSS. Tested: `cantus-rt/tests/machine.rs`
  (`a_background_thread_is_lowered_below_normal`) and `scene_px.rs`
  (`the_bake_pool_is_shared_low_priority_and_lets_go`: the pool's threads report -1 on
  Windows).
- **Recent bakes**: the last three full pictures are kept (keyed by the subject, room,
  look, mood, camera, size and pixels a point), so toggling a theme, a finish or a room
  back shows the picture again without a bake (≈3–5.5 MB each).
- **Quick while gliding**: the scene module asks `render(model, ppp, quick)`; quick bakes
  `PREVIEW` (one sample a pixel; no occlusion, mirror, traced reflection or bloom).

## Cost and bounds

Measured with `scene_hybrid <dir> mem 1.5` (the workspace's release profile, thin LTO; the
bake pool's 6 threads below normal priority). **The machine was never idle**: three other
builders' cargo builds ran throughout, and below-normal threads yield to them, so these are
upper bounds — the same bake measured 0.99 s with one build running and 2.4 s with three.

| Picture (1134 × 678) | Quick | Full | Full, no traced reflections | Bake peak |
|---|---|---|---|---|
| Chorda, stage, Night | 0.27–0.37 s | 0.99–1.7 s | 0.44–1.1 s | 25 / 66 MB |
| Tympanum, stage, Night | 0.26–0.44 s | 0.69–1.3 s | 0.48–0.87 s | 38 / 82 MB |

Per pass (Chorda, full, best of three, one other build running): shadow map 10 ms, floor
mirror 28, reflections' hierarchy 6, raster 11, **shading 905** (of which traced reflections
≈ 540: the same bake without them shades in 366), wires 3, depth of field + bloom + tone 22.
Tympanum: shadow map 23, mirror 36, hierarchy 62, raster 27, shading 516 (≈150 of it
reflections), wires 2, post 23.

**Half-resolution reflections** (checkpoint 3): the traced reflection is found once every
other pixel each way and shared out to the samples round it that see the same material,
facing the same way, at the same depth — only where the four cells agree (within 12 %):
a fine reflection (the strings in the lid, an edge's line) is still traced at full
resolution. Traced reflections dropped from ≈540 ms to ≈16 ms of Chorda's bake when
interpolated everywhere (blocky on the strings), and cost ≈100–400 ms with the agreement
test that keeps the strings sharp (measured with two other builds running; a quiet machine
would show the target).

What made it cheaper this round: a pixel inside one triangle shaded once (≈40 % of the
shading); reflections on a fixed five-ray cross instead of seven (no grain, so no samples
needed to average it), one shadow look-up where a reflection lands instead of the 32-tap
penumbra, the strings as 12 cm pieces in the hierarchy (a long diagonal wire's box overlapped
half the frame's), and no room view for a coat seen square on. Next: reflections at half
resolution with an edge-aware upsample, and the shading's panels pre-culled per bake.

Memory (working set of the example process, which holds nothing else): 4 MB at start, 8 MB
after Chorda's quick and full bakes, 11 MB after Tympanum's, 14 MB with both pictures kept —
each bake's scratch (65–82 MB at its peak) is released when it returns; what stays is the
picture (3 MB at 1.5×) and the last three in the recent-bake cache.

**Audio at 64 frames during bakes** (`cantus-bench paced --frames 64 --seconds 40 --songs
startup`, callback in MMCSS, 6 promoted workers, 30 000 blocks; the machine busy with other
builders throughout): misses **99 and 52** with no bake; **40 and 73** with Chorda and
Tympanum baked back to back for the whole run (23–37 full bakes). The bakes add no
measurable misses; the misses come from the other builds. To repeat on a quiet machine:
`scene_hybrid <dir> loop 1.5 45` beside the paced run.

Memory: a bake's peak is its buffers (depth, triangle ids, colour, the mirror, the shadow
map) plus the geometry and the hierarchy, which borrows the instrument's triangles instead
of copying them (Tympanum 131 → 82 MB at 1.5×); the cymbals' meshes were halved (288 k →
155 k triangles). A bake is bounded to 3.2 M samples (a larger picture drops to one sample
a pixel, then bakes smaller and is drawn stretched) and 8 192 px a side.

Speed: a pixel whose samples all see one triangle of a lacquer or a matte is shaded once at
its middle (its textures filtered to the pixel) unless the key's shadow edge crosses it;
edges, metals and the lid's mirror take every sample. Reflection rays are traced only on
surfaces smoother than 0.12.

## How it is tested

- `tests/scene_px.rs`: every piano body × finish × lid with and without mics; every room ×
  theme for both instruments; every floor, wall and rug with every effect; the bits never
  depend on the thread count; the bake's bounds; the time and peak at the card's size
  (release, ignored by default).
- `tests/scene_px_e2e.rs`: *Photographic* in `settings.json` → `CantusApp::assemble` →
  the renderer registered, `backend::current() == PerPixel`, the scene module's picture of
  Chorda on the stage is the photographic renderer's to the bit; *Drawn* gives the vectors
  back; *Photographic* again gives the same picture.
- `chorda_scene_px_model.rs` (unit): a `Model` with no subject is baked from its primitives.

Run: `cargo test -p cantus-ui --lib --test scene_px --test scene_px_e2e --test scene_backend
--test chorda_scene` (547 + 6 + 1 + 2 + 3 pass) and `cargo test -p cantus-rt --test machine
a_background` (passes); clippy `-D warnings` on `cantus-ui` and `cantus-rt` clean in this
builder's files (`--no-deps`; other builders' files carry Rust 1.97's new lints, allowed in
that run).

## What the owner should look at

`docs/round40/scene-px/` and `compartir\p3\scene-px\`: `sbs-*-1.5x.png` (vector | photographic)
and `crop-*-3x.png` for Chorda on the stage (Night, Paper) and Tympanum (stage, hall). In
Chorda's crop: the lid's underside mirroring the harp and strings, the bar down the tail and
the cheeks' bars, the gold interior, the desk's sheen, the floor's reflection.

## Verified only in pictures / left

- Verified by eye in the example's PNGs and by the tests; not yet seen in the running app
  by the owner (the setting's switch is tested through `assemble`).
- While a control glides the scene module asks for a quick picture
  (`PixelBackend::render(model, ppp, quick)`): `PREVIEW` (a sample a pixel, no occlusion,
  mirror, traced reflection or bloom); at rest the full bake replaces it.
- At very large pictures the bake is smaller than `size × ppp` and drawn stretched (the
  bound above); the backend's contract says the image is `round(size × ppp)`.
- The old ADR 0104 renderer files (`chorda_scene_{raster,shade,room,gear,geo,piano}.rs`)
  are unused by the photographic renderer; `chorda_scene_mesh.rs` and `pub mod hybrid` in
  `chorda_scene.rs` must stay mounted.
- Chrome stands still show a little sparkle at 1.5× where they mirror a patterned rug.
