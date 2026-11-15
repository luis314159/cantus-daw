# Round 40 phase 3 — builder `scene`: the instruments' scenes as projected vectors

ADR 0126 (amended), ADR 0104 §2 (the look). Branch `r40/scene`.

## The backend (for `scene-px`) — `crates/cantus-ui/src/scene/backend.rs`

The owner's decision (2026-11-12): *Settings ▸ Appearance ▸ Instrument pictures: Drawn ·
Photographic*, an app setting (`Settings::instrument_pictures`, never the project's).
**Drawn** is this module's vector picture and the default; **Photographic** is the
per-pixel renderer (`scene-px`'s `views/inst/chorda_scene_*`).

```rust
pub enum SceneBackend { Vector /* default */, PerPixel }   // serde; ALL; key()

pub trait PixelBackend: Send + Sync {
    /// The scene's model to its picture, `ppp` pixels a point: an image
    /// `round(model.size * ppp)` pixels large (premultiplied RGBA). Called on the
    /// scene's worker thread (or the painting thread in the CPU screenshots), never
    /// with a lock held; a stale build is dropped by the caller.
    /// `quick` while a live control glides (a fast bake wanted); when it rests the
    /// scene asks again with `quick` false for the whole picture.
    fn render(&self, model: &Model, ppp: f32, quick: bool) -> egui::ColorImage;
}

pub fn register_pixel(r: Arc<dyn PixelBackend>) -> bool;  // once, at start-up
pub fn pixel() -> Option<Arc<dyn PixelBackend>>;
pub fn set_default(b: SceneBackend);                       // the setting (app.rs applies it)
pub fn current() -> SceneBackend;  // PerPixel only when chosen AND a renderer is registered

pub struct Model {
    pub prims: Vec<Prim>,        // the instrument, its stands and mics, in world cm
    pub cam: Camera,             // fitted to the picture in points (scale by ppp)
    pub light: Light,            // the mood, the key's place and focus, the stage's spot
    pub room: Room,              // Stage (default) · Booth · Studio · Hall · Church
    pub look: RoomLook,          // floor, walls, rug (None = the room's own)
    pub gloss: f32,              // the floor's gloss as the vector picture mirrors it
    pub frame: Frame,            // the framing margins the camera was fitted with
    pub size: egui::Vec2,        // the picture in points
    pub subject: Option<Subject>,// always Some from a build: Piano{look,lid,persp,width,
                                 //   room_level,mics} or Kit{kit,def,finish,mics}
    pub evening: bool,           // dark theme's evening / light theme's daylight
}

pub enum Prim {                  // world cm: x right, y up, z toward the viewer, floor y = 0
    Facet { pts: Vec<V3>, n: V3, mat: Mat, grain: f32, part: Part },
    Tube  { a: V3, b: V3, r: f32, mat: Mat, part: Part },
    Flat  { pts: Vec<V3>, col: C3, alpha: f32, part: Part },
}
```

**What stays this module's in either backend:** the camera, the pick (`Scene::pick`,
`scene::pick`), the selection outline, the anchors, the cache and the worker. A
per-pixel picture must be drawn through `model.cam` (multiply its screen transform by
`ppp`) so that what is clicked is what is drawn. A build with `SceneBackend::PerPixel`
still makes the vector display list (the pick's outlines), but `to_image` hands the model
to the registered renderer instead of rasterising the list. The model is captured only
when PerPixel is current (Drawn pays nothing for it).

**Wiring for `scene-px`:** implement `PixelBackend` on a type of yours and call
`crate::scene::backend::register_pixel(Arc::new(..))` once from `CantusApp::assemble`
(one line marked `// r40:scene-px`, beside `scene::backend::set_default` in `app.rs`) —
and in the CPU screenshot examples. Use `subject` to build your own geometry when your
meshes are richer than `prims`; take the camera, the room and the light from the model.
Until a renderer registers, *Photographic* draws vectors (nothing goes blank).

## For `tymp-ui` (its requests)

- **Tags at the piece:** `Scene::anchor(rect, Picked) -> Option<Pos2>` answers from the
  build on screen (nothing built: call it every frame); `scene::anchor(rect, &view, Picked)`
  builds (tests only). A piece's or the lid's anchor is the top middle of its outline; a
  mic's is its head.
- **Floor, walls, rug:** `SceneView::look: RoomLook` (each `None` = the room's own).
  Tympanum keeps them as Chorda does — schema 69's text keys `look.room/floor/walls/rug`
  are accepted for `AcousticDrums` (`cantus_core::inst::room_look`); set them with
  `set_inst_text` (one undo step, named as the look). The kind is not yet shipped, so no
  bump: a Tympanum without `look.room` draws its kit's room.
- **The room drawn:** `Room::of_tympanum(spec, choice)` — the look's room when kept, else
  `Room::of_kit(choice)`, where the kit's **own** room is drawn as the **stage** (delivery
  6). `Room::of_look`, `Room::kind` convert both ways.
- **Picking mics:** with `Subject::Kit { mics: true, .. }`, `Scene::pick` / `scene::pick`
  answer `Picked::Mic(OutputId)` (the definition's `m{n}` output); `Scene::point_on`
  finds a point on any pick.
- **The module draws:** `scene::DRAWS == true` — the plan from above can go.

**ADR 0104's files stay** (the owner's decision: *Photographic* is `scene-px`'s): nothing
of `views/inst/chorda_scene_{raster,shade,room,gear,geo,mesh,piano,hybrid}.rs` is removed
or unmounted by this builder; `chorda_scene.rs` keeps `pub mod hybrid` and the mesh module.

## What changed this session (the builder resumed after an interruption)

- **The backend** above, the setting (*Settings ▸ Appearance ▸ Instrument pictures:
  Drawn · Photographic*, `Settings::instrument_pictures`, applied at boot in `assemble`
  and when changed; words in `areas/scene`, both locales), `quick` for a gliding control.
- **The look past the designer's sheets** (the owner: *espectacular, súper profesional y
  lindo — incluso mejor*): the open lid is solid black lacquer that mirrors the gold plate
  it opens over (soft-edged, Fresnel toward grazing; 72 strips × 13 points), its edge a
  long crisp lacquer line; a broad flat lacquered face (box sides, key slip, fallboard,
  the lid's edge, the bench) is shaded without the strip lights' streak and gets it as a
  crisp band across itself (25 samples, shaped) — no face lit whole, no grey veil; the
  case's strips overlap so no hairline shows between them; floors and walls are cut at a
  near plane before projection (the Paper stage showed the sky between its boards); the
  far walls are drawn sharp under their blurred copy (the sky showed at the left edge).
- **Framing**: the play view uses the designer's `chorda_play.stage` box (40/50/40/62);
  the picture-alone sheets' frame (30/40/30/40, `PIANO_PICTURE_FRAME`) for comparisons.
- **Failures fixed**: `chorda_scene::a_new_chorda_stands_on_the_stage_and_its_lid_opens_
  with_a_click` (the start page lay over the editor in the test's settings);
  `every_factory_preset_shows_a_piano_its_sound_can_stand_for` (maple is the user's
  choice: the test asks the four round-37 finishes, as on this branch);
  `the_scene_card_at_rest_allocates_next_to_nothing` — bisected: the card's widgets
  allocate nothing at rest; each child `Ui` costs egui one allocation a frame and
  `spacing_mut` another (it copies the style): the card now lays out in two children with
  a style made once and its words made once; the baseline is an empty panel with the same
  two child areas (26 against 21 + 6); clippy's `is_multiple_of` and the blur's identical
  branches.
- **Tympanum**: `look.room/floor/walls/rug` accepted for `AcousticDrums` (schema 69's keys;
  the kind is unshipped, no bump), `Room::of_tympanum` (the look's room, else the kit's;
  its own room drawn as the stage), `Room::of_look`/`kind`, `scene::DRAWS`,
  `Scene::anchor`/`scene::anchor`.

## How it is tested

`tests/scene_vector.rs` (every piano × finish × lid × room × mood and the kit in every
room drawn finite and inside its picture, every floor/walls/rug; the pick for pieces,
mics, the lid and nothing in the empty room; at rest no repaint, no build, no more
allocations than an empty frame; a lid drag's build time; the selection ≥ 3:1 on its halo
in all 24 themes), `tests/scene_backend.rs` (new: the setting's default, the fallback,
the model handed over, quick while gliding and whole at rest, nothing at rest; anchors),
`tests/chorda_scene.rs` (e2e through `CantusApp::assemble`: a new Chorda on the stage, a
click on the lid opens it in one undo step, the mics move with Perspective, the room
picked is the room drawn, an older song's room from its Room size),
`tests/chorda_scene_budget.rs` (the card at rest), `cantus-core` `room_look` tests.
All pass on this branch. Clippy `-D warnings` on `cantus-ui --all-targets` is clean for
this builder's files; it stops on files merged from `r40/integration` that are not mine
(`tympanum_load*.rs` type complexity, `sampler_sfz_mics.rs:338` range contains;
`cantus-core` `inst_out.rs:409` collapsible if and `inst/tympanum_hits.rs:217`
`f32::powi`; `cantus-dsp` `tympanum_room.rs` range loops) — `tymp-engine`'s to fix.

## Verified only on the CPU

Every picture here is the CPU rasteriser's (`tiny-skia`, the same path the app uses for
its texture); the frame budget on a real GPU and at 4K is the integrator's to watch in
the app. No per-pixel renderer is registered on this branch (scene-px's).

## What the owner should look at — checkpoint A

`F:\projects\daw-r40\compartir\p3\scene\checkpoint-a\` (designer at the left, Cantus at
the right, the same size): `sbs-chorda-{night,paper}-{1x,1.5x}.png`,
`sbs-crop-chorda-{night,paper}-3x.png` (the case and the lid close: Cantus drawn at 3×
beside the designer's 1.5× enlarged), `sbs-tympanum-{stage,hall}-{night,paper}-1.5x.png`,
`sbs-crop-tympanum-stage-night-3x.png`, and a few variants (`cantus-chorda-*.png`). The
designer's sheets carry their UI cards; the module draws the picture alone.

## After checkpoint A (the coordinator's list)

- **The case's facet lines** (Paper, 3×, toward the tail): gone — each strip of the rim
  reaches 0.35 cm into its neighbours.
- **The designer's contact sheets, tile for tile** (`cargo run -p cantus-ui --example
  shoot_scene_sheets -- <dir>`: each 304 × 171 tile with the designer's framing, pasted
  over a copy of his sheet beside the original): `chorda-variants-*` (every body in three
  finishes, the lid, the rooms, Perspective, the light) and `rooms-*` (every floor under
  the kit, the rugs, every wall behind the grand, the combinations). What they showed,
  fixed: an upright is framed as it stands, its top as open as it is (it was framed for
  the open top, so a closed one stood small); its brass hinges draw over the lid's edge
  they hold; a closed or small lid had a moiré of curved lines (each strip's seam in its
  own gradient) — the board is drawn whole underneath and its strips, about three points
  each on screen, have no seams; the lid's gold reflection is held under half (in a
  bright room the stronger mix read as a tan glass, not black lacquer). Pictures:
  `F:\projects\daw-r40\compartir\p3\scene\variants-rooms\sbs-{variants,rooms}-{night,paper}-1x.png`
  (the designer at the left), the Night pair also in `docs/round40/scene/checkpoint-a/`.
- **The default is one constant**: `scene::backend::DEFAULT_PICTURES` (Drawn today). A fresh
  install and a settings file written before the choice existed read it; the process-wide
  setting starts from it. The owner may make Photographic the default once `scene-px`'s
  renderer is fast and light enough — then only that constant changes (and the assertion
  in `tests/scene_backend.rs` that says Drawn is today's).
- **`PixelBackend` stays as it is** for `scene-px` (`render(model, ppp, quick)`); hooks it
  asks for (a pick buffer for clicks in Photographic, anchors from its own geometry) come
  to this module.

## Toward the photographic picture (the owner: Drawn stays the default and looks as close as it can)

Target: `scene-px`'s renders (`compartir\p3\scene-px\`). By vector means, one cached texture
per scene state, nothing at rest:

- **The lid mirrors the frame and the strings**: the gold plate, its holes and struts and
  the 78 strings are drawn reflected in the lid's plane and cut to its outline (a clipped
  polygon; each string clipped on screen), dim and warm on the black board — the soft
  gold glow it replaces is gone.
- **Long bright strip-light bands** on lacquer: a flat face's streak is sampled sharper
  (cos^1800) and drawn at the clear coat's strength over any colour (a clear coat mirrors
  a light as brightly over maple as over black), and a third tall light behind on the
  right puts the long band on the bentside and on a kick's shell.
- **One band down a curved surface**: the key's tight highlight and the mirrored soft box
  join the strip lights in the band (sampled across the face with the surface's normal
  interpolated from its edges), and a broad flat face's per-strip shade leaves them out —
  shaded once per strip they stepped from strip to strip; the bentside's strips draw their
  bands after all of them (each strip reached into the next and cut the band before it
  with a dark line): one long bright highlight, as in the photograph.
- **The floor's mirror crisp near the contact, softer away**: drawn once, blurred twice
  (0.12 and 0.7 of the base blur) and laid through two row masks (near: whole at the
  contact line, gone a third of the fade down; far: the rest); a stand's tube (r ≥ 0.5 cm)
  now reflects too.
- **Light pools on the curtain**: each uplight's warm pool on the folds and the lamp
  itself lit at its foot.
- Memory: the steady state is one texture per scene (756 × 452 points at the screen's
  pixels); a build's transient peak is the picture, the mirror and its blurred copy, two
  row masks and the blur's transpose — about 14 MB at 1.5×, estimated from the buffers
  (not measured with `memmap.ps1`).

## The whole suite on this branch (r40/integration 637943ca merged)

`cargo test -p cantus-ui -p cantus-core --no-fail-fast`: everything of this builder's
passes — `scene_vector`, `scene_backend`, `chorda_scene`, `chorda_scene_budget`, the
`room_look` and i18n tests. Two cuts in Spanish were this builder's and are fixed (*Show
mics* → *Ver micrófonos* in a key 132 points wide; *Hall* → *Auditorio*). What still fails
belongs to other builders' files, merged from integration:
- `cantus-core` `inst::sampler::tests::indexed_keys_parse_one_spelling_only` (chorda-src);
- `font_coverage` (▸ ▾ at `chorda_source_list.rs:244`, chorda-src);
- `inst_chorda::no_text_is_elided_or_overlaps_in_spanish` and `ux_whole_words`: the
  source list's names and credits, *Obtener · 742 MB*, *Mezclador*, and *Pedal derecho …*
  (chorda-src and the pedal card);
- `ux_help_line` (eight rows of the source list without a hint, chorda-src);
- `tympanum_words` (tymp-ui).

Clippy `-p cantus-ui -p cantus-core --no-deps --all-targets -D warnings`: clean, with
tymp-engine's four lint kinds allowed (`type_complexity`, `manual_range_contains`,
`collapsible_if`, `disallowed_methods` — fixed on `r40/tymp-engine`).

## What is left

- Tympanum's views moving to `Room::of_tympanum` and the text-keyed look (tymp-ui's).
- Drawn stays the default (the owner, 2026-11-12: memory); it is to look as close as it
  can to the photographic picture — the next pass below.
