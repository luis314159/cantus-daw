# r40/visual — where to resume

Branch `r40/visual` (worktree `F:\projects\daw-r40\visual`, untracked `.cargo/config.toml`,
own target). Doc: `docs/round40/visual.md`. Checkpoints: `compartir\p3\visual\cp1`, `cp2`,
`cp3` (sheets Drawn | Photographic | reference; cp3's script is
`F:\projects\daw-r40\tmp\vis3\sheet.py <dir> <out.png> <case>...`, the 3× crops by
`tmp\vis3\crop.py <out> x0 y0 x1 y1 k <png>...`).

Render: `cargo build --release -p cantus-ui --example scene_hybrid` then
`target\release\examples\scene_hybrid.exe <dir> <filter|all> 1.5 [crop]` (prints Drawn's
time, best of five, and the bake).

## Checkpoint 3 — what was done (see the doc)

1. Photographic stands: rods narrower than `THIN_PX` (8 px) on screen are drawn as shaded
   tube strokes (`raster::tubes`), their triangles kept for shadows and reflections; every
   chrome rod's key visibility is a smoothed profile along it (`rod_key`); a metal's mirror
   is spread over the normal's turn under the sample (`normal_turn`, `env_rough`'s `dr`);
   rough chrome's reflections half desaturated; the tube's edges darkened.
2. Cymbals: `scene::kit::prof` shared by both pictures (bell 0.26 of the radius, 3.7 cm
   tall), `settle` eases each cymbal's lean to half (`CYMBAL_TILT`); Drawn: edge band, lit
   lip, bell shoulder and glow, stronger lathe rings; Photographic: a rounded lip, lathe
   bands an octave coarser per pixel size, the env capped and the streak bounded across
   the grain, the grazing Fresnel kept bronze, softer dents.
3. Drums (Drawn): the head's collar band, the hoop's shadow crescent, clouds; a key glint
   on every lug (`glint`); tom brackets on the shell's front-inner quarter (both pictures).
4. Drawn black lacquer's bar at 0.4 of the coat (was 0.15), neutral white on black.
5. The Photographic upright's case edges rounded (`rounded_prism_x`), the fallboard's top a
   bullnose.
6. Paper walls: `pal::daylit` warms the walls in both pictures, Photographic's walls take
   `DAY_WALL` (1.45×) more light by day.

## Checkpoint 4 — done (see the doc's cp4 section; sheets in `compartir/p3/visual/cp4`)

Drawn black lid a black mirror (`piano::lid_mirror` k 0.42, `ink::BLACK_EDGE`), Drawn bronze
fill and bounded streak (`light.rs` Bronze), Drawn chrome silver (`light.rs` Chrome), Drawn
heads off-white (`ink::COATING`, wider collar), the tail bar (`Light::band`, dark lacquer
only), mic stands a size lighter (`gear.rs`, `chorda_scene_px_gear.rs`). Sizes: the kit
definition's, by the coordinator's decision.

## Checkpoint 5 — done (doc's cp5 section; `compartir/p3/visual/cp5`)

Lid band (`piano::lid_mirror`, the `across` mask), heads capped by day (`kit::head`), the
stage a pool (`Light::no_fill`, `rooms::floor` pool by evening, `rooms::stage` curtain).
Drawn timed interleaved against e4052fbe (`cp5/times-drawn-vs-e4052fbe.txt`; the
baseline binary is `F:/projects/daw-r40/tmp/vis3/hyb-e405.exe` — e4052fbe with today's
scene_hybrid example, its target deleted).

## Checkpoint 6 — done (doc's cp6 section; `compartir/p3/visual/cp6`)

Desk (`piano.rs`, the music desk block), light finishes (`light.rs` Lacquer: the fill from
the camera's side, the cap, `band`'s soft coat), the frame's gilt (`piano.rs`, the plate).

## Checkpoint 7 — done (doc's cp7 section; `compartir/p3/visual/cp7`)

Strings and dampers (`piano.rs`, after the struts), `light::light_finish` and `band`'s light
finish rule, the gilt (`piano.rs`, the plate), the keyboard (`piano::keyboard`).

## Checkpoint 8 — done (doc's cp8 section; `compartir/p3/visual/cp8`)

Bass strings (`piano.rs`, the strings loop: every other wound string drawn, `lead` one in
six, 1.0–1.2 wide, desaturated copper — fixed after review), the shells
(`kit::drum` figure ×0.4; `light.rs` Lacquer: `light_finish` fill white and held under
white), the kick's port and ring (`kit::drum`), hoops' rolled top (`kit::hoop_outer`).
Scripts: `tmp/vis3/cp8.sh <exe>`, `crops8.sh`, timing `abtime.sh hyb-e405.exe <exe> <cases>`.

## Next (cp9, for a fresh builder)

Compare every Drawn tile with its Photographic neighbour in `compartir/p3/visual/cp8/`
sheets and pick the weakest; keep the timing interleaved against
`F:/projects/daw-r40/tmp/vis3/hyb-e405.exe` (`abtime.sh`) and the nine tests.

## Open
- The Drawn frame is still more saturated than the photograph's; the maple bentside's
  figure strong toward the tail.
- Chrome made with `g.tube` (lugs, hoops' lathe, the kick's claws) is not a rod: no tube
  stroke and no smoothed key — fine at their size, watch for beads in close crops.
- The cymbals' lathe rings read faintly in Photographic at 1.5×.
