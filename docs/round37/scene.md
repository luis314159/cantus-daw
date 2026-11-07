# Round 37 — scene: Chorda's piano in its recording room (ADR 0104, schema 58)

**The owner's ask (2026-11-06):** *"imágenes de pianos de los presets de fondo para que se vea
diferente, y la sala y micros … muy pros para que se vean los cambios (tipos de pianos,
colores, formas etc.) … un piano súper profesional y la sala de grabación y todos los
elementos."*

## What was done

- **The scene** (`crates/cantus-ui/src/views/inst/chorda_scene*.rs`): Chorda's Play view's
  hero card, *The piano and its room*. A first delivery drew it as vertex-lit vector
  shapes; the owner asked for it *"más real, que se vea mucho más profesional"* (the bar:
  the product pictures of the premium sampled-piano libraries), and the integrator listed
  five faults (a lid that read as glass, a rod through the lid, uprights like wardrobes,
  cables ending in the air, keys overhanging the cheek). So it is now **built in 3-D and
  baked by our own CPU renderer** on a worker thread (ADR 0104 §1): a depth-buffered
  rasterizer, per-pixel procedural materials (wood figure and pores, staggered floor boards,
  the rug's weave and border, fabric, limestone, cast gold, brushed metal, grille mesh), a
  key spot with a soft shadow map, ambient occlusion, a rim light, reflections traced
  against the room (softbox, floor, walls, and the frame under the lid), the strings as
  anti-aliased lines, a polished floor's mirror image, depth of field on the far room,
  bloom, a filmic curve. The grand: rim skins and top, soundboard, frame with holes,
  struts and 144 pins, strings and bridges, dampers, 88 bevelled keys, cheeks forward of
  them, fallboard, fretwork desk, legs with ferrules and casters, lyre, and the lid as a
  solid slab on a stick that leans from the rim to its underside. The upright: carcass
  between beaded side cheeks, plinth, panels, turned columns on toes, key slip and arms,
  sloping fallboard with its lip, a top lid slab. Pencil pair on a stereo bar (tally ring),
  room pair of large-diaphragm mics in shock mounts; cables off the frame. Booth, studio
  live room, hall, church. Evening light in Slate, daylight in Paper (the scene is a
  display and keeps its own ground — ADR 0074 §7). Previews follow a gliding control; the
  full bake replaces them when it rests.
- **The look** (`cantus-core/src/inst/chorda_look.rs`): two text keys, `look.body` and
  `look.finish`; **schema 58** writes a black concert grand into every older Chorda;
  `fixtures/v58.cantus`; set through `SetInstrumentText` (named *Change the piano's look*);
  the undo fuzz's own stream; pickers *Piano* and *Finish* over the scene.
- **Honesty**: `chorda_look::fits` — which bodies a sound can stand for (ADR 0104 §3); each
  of the 24 factory presets declares its look in its text (all four bodies and finishes
  used) and a test holds them to it.
- The strings from above (*Strings and hammers*) stay in Edit.
- The palette is data in `kit/skin/scene.rs`; the scene's files name no colour.
- Manual EN/ES (Chorda's section), BACKLOG entry marked done, ADR 0104.

## After the owner's look at the bake (integrator's notes, 2026-11-06)

- The rug's concentric rings were the medallion's own pattern: now an all-over lattice of
  small lozenges with a soft abrash. The uprights' softness was the depth of field focused
  on the piano's centre: the focus now reaches the instrument's farthest corner, so only
  the room behind it softens. The grand's lid read as smoky glass (its underside mirrored
  a flat wash of the frame): it is now a dark lacquered board that mirrors the floor only
  dimly, with its lit edge. The Play view's tall Pedals card is now the pedals with model
  2's *Microphones and lid* under them, beside the scene.
- Two robustness fixes found on the way: a line's bounds near the lens overflowed in a
  debug build (clamped before the cast; a failed bake now returns an empty picture instead
  of leaving the scene waiting), and a window's passing frame at another size asked for a
  bake (a new size is baked once it has held for a frame).
- Merged r37/integration (c721f672): Chorda's Play and Edit views checked on finish's
  seventh delivery (the Edit view scrolls).

## How it was tested

| Test | Where |
|---|---|
| The look is text, never a parameter; defaults; no upright for a default sound | `cantus-core` `inst::chorda_look::tests` |
| Older fixtures open as a black concert grand; v58 survives save/load; a look there is kept | `cantus-project/tests/chorda_look.rs` |
| Undo/redo among the other commands, refusals | `cantus-core/tests/undo_invariants.rs::a_chordas_look_undoes_and_redoes_among_the_others` |
| Every factory preset's look fits its sound; all bodies and finishes used | `cantus-ui/tests/chorda_scene.rs` |
| Through `CantusApp::assemble` and the app's own frames: Lid, Mics, Width, Size and Room move the scene; a preset changes the body; a picked look is one undo step; no rebuild at rest | `cantus-ui/tests/chorda_scene.rs` |
| Null test: the look changed twice while the piano plays live = the offline render without it, to the bit; other looks render the same | `cantus-ui/tests/chorda_scene.rs` |
| Budget: no allocation beyond an empty frame at rest and while gliding (uploads aside); triangles bounded; the bake's time (release, ignored) | `cantus-ui/tests/chorda_scene_budget.rs` |
| Strings in both locales | the i18n test (`locales/areas/scene.{en,es}.txt`) |

## The numbers (this machine, release)

- A full bake (`measure_the_bake`, release, seven threads): the hero at 1 040 × 600 px
  (2 080 × 1 200 samples) in 537 ms; a preview (520 × 300 px, one sample, no extras) in
  69 ms. The 800 × 450 sheet tiles: 290–550 ms each. 12 600 – 28 800 triangles a scene
  (felt upright … the hall's concert grand with its room pair), plus the strings' lines.
  A 200 % hero (2 080 × 1 200 px, one sample): about 0.8 s. A debug build bakes some twenty
  times slower (the shots are made in release).
- Steady state: no allocation beyond an empty frame, no bake asked for
  (`chorda_scene_budget.rs`); a glide's frames allocate nothing but the texture uploads of
  the bakes that land.

## The pictures

`docs/round37/scene/` (1.4 MB, palette-quantised): `sheet-presets-slate.png` and
`sheet-presets-paper.png` (all 24 presets, tiles at half size), `chorda-play-slate-100.png`
(the Play view in the app) and `hero-200-slate.png` (a crop at 200 %). Everything else —
each preset at full size, the controls at 0 / 50 / 100 % in both themes, 100–200 %, the
Edit view — is in `F:\projects\daw-r37\tmp\scene9\`, not in git. Regenerate with
`cargo run --release -p cantus-ui --example shoot_chorda_scene -- <dir>` and
`cargo run --release -p cantus-ui --example shoot_inst_faces -- <dir> chorda --view=play`
(release: a debug build bakes some twenty times slower).

## Marked lines (outside my files)

`// r37:scene` in: `cantus-core/src/inst/mod.rs` (the module), `cantus-core/src/inst/chorda.rs`
(`is_text_key`), `cantus-core/src/command.rs` (the history's name), `cantus-core/tests/
undo_invariants.rs` (a test of its own at the end), `cantus-project/src/format.rs` (schema 58,
its migration and helper), `cantus-project/examples/write_fixture.rs`, `cantus-ui/src/i18n.rs`
(the area), `cantus-ui/src/kit/skin.rs` (the palette's module — finish's file, one line),
`cantus-ui/examples/shoot_inst_faces.rs` (screenshots bake on the painting thread),
`cantus-ui/tests/fx_lint.rs` (the scene's files under the views' lint),
`cantus-ui/src/views/inst/chorda.rs` (the scene's modules, its cache in `State`, the Play
view's hero card, the presets' `text_with_look!`).

## What needs the owner

- **His eye**: whether the scene reads as first-rank artwork beside the premium sampled
  pianos' GUIs — the sheets above are what to judge. Everything was judged here only by
  looking at CPU renders.
- Schema 58 may collide with another builder's number at integration (library said 57 only
  if it must; drums took 57): renumber freely, the migration is self-contained.
- The branch's history still holds the first ~13 MB of pictures (commits e0d901db and
  before); a squash at merge keeps them out of master.
