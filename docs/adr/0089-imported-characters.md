# ADR 0089 — Imported characters: Lottie by our own player, Aseprite and TexturePacker sprite sheets, DragonBones; Rive later; one image rasteriser of our own; data, never code

**Status:** accepted (round 35, import builder; the evaluation confirmed by the integrator
on 2026-10-31) · **Date:** 2026-10-31 ·
**Extends:** ADR 0087 (the dancer, `Character`, the one renderer), ADR 0042 (licences for
content), ADR 0030 (content), ADR 0047 (our licences) · no schema change (§7)

## Context

The owner asked for characters people bring (round 35, item 3; BACKLOG §6i): "Rive,
Lottie, sprite sheets (Aseprite, TexturePacker), DragonBones evaluated first by licence and
capability; glTF/VRM 3D later; Spine and Live2D out unless their licence serves a free GPL
app. A character file is checked and sandboxed (no scripts) and shows its licence." Lottie
and sprite sheets are required this round; DragonBones and Rive go in if the measurement
says they can be done well.

The contract fixed the rest: an import is a `cantus_dance::Character` like an included one
(every `Move`, every `Mood`, drawn by the one tiny-skia renderer at the exact pixel size),
stored as `file:<path relative to the project folder>`; the budget is **one character on a
600×600 stage in ≤ 1.5 ms** (release, this machine).

### How it was measured

Prototypes in `docs/round35/import/prototype/` (a crate of its own, not a workspace
member: `cargo run --release -- velato|lottie|sprite|bones|own …`), on the owner's machine
(AMD Ryzen 7 7735HS, 8 cores, Windows 11, release build, idle machine, mean of 100–1 000
frames after a warm-up). Inputs: the twenty example files of the Lottie specification
(`lottie/lottie-spec`, `docs/static/examples`, downloaded for measurement only, not
committed), files we generate by code (a little character with sixteen moves as markers,
and hostile variants), atlases and skeletons we generate by code.

**Lottie through `velato` 0.12** (linebender, Apache-2.0 OR MIT; with `default-features =
false` it needs only `kurbo`/`peniko`/`serde`, already in our tree, and `serde_repr`; it
has a `RenderSink` trait a tiny-skia sink could implement):

| File | Parse | Result |
|---|---|---|
| the 20 specification examples | 0.05–0.66 ms | all parse |
| ours: a character, 7 layers, 16 markers (29 KB) | 2.5 ms | parses |
| ours, valid: a layer transform **without `r`** (every transform property is optional) | — | **panic** `todo!("split rotation")` |
| ours, valid: a 3D layer with `rx`/`ry`/`rz` (After Effects writes these) | — | **panic** |
| ours, valid: blend mode *Add* (`bm` 16) | — | **panic** `unimplemented!()` |
| ours, valid: a group transform with a split position | — | **panic** `todo!("split position")` |
| hostile: 100 000 nested arrays | 0.03 ms | refused (serde_json's recursion limit) |
| hostile: a precomposition that contains itself | 0.01 ms | refused (by luck: a missing field) |

Four of six *valid* variants of our own file panic; its README lists text, embedded
images, dashes, split rotations and split positions as missing; nothing in it bounds
sizes; its maths is `kurbo`'s `f64` with the platform's `sin`/`cos`.

**Our own Lottie player over tiny-skia** (the prototype: shape layers, null parents,
groups, ellipse/rectangle/path, fill/stroke, transforms with hold, linear and cubic-bezier
keyframes, split positions, markers; parsed through `serde_json::Value`):

| File | Parse | One frame, comp fitted to 600×600 |
|---|---|---|
| 7 layers, 25 paints (29 KB) | 1.8 ms | **0.69 ms** |
| 28 layers, 100 paints (114 KB) | 8.7 ms | 2.6 ms |
| 112 layers, 400 paints (456 KB) | 36 ms | 6.8 ms |

The cost is tiny-skia's, per path and per pixel covered; a light character fits the budget,
a heavy one does not without a cache (§2).

**Sprite sheets** (a PNG atlas decoded, one frame drawn on a 600×600 stage):

| | 1024² atlas | 2048² atlas | 4096² atlas |
|---|---|---|---|
| PNG decode (once, at load) | 5.5 ms | 14.9 ms | 60.6 ms |
| tiny-skia pattern, nearest, frame 540 px | 1.62 ms | 1.72 ms | 1.73 ms |
| tiny-skia pattern, bicubic, frame 540 px | 10.1 ms | 10.1 ms | 9.9 ms |
| **ours**, nearest / bilinear, frame 540 px | 1.15 / 3.7 ms | | |
| **ours**, nearest / bilinear, frame 252 px (size 1 on this stage) | **0.28 / 0.81 ms** | | |

**DragonBones-shaped skeletons and meshes** (bones solved, image regions drawn; a mesh of
a 128-px region warped over ~400×400 px):

| | tiny-skia (pattern per region / per triangle) | ours |
|---|---|---|
| 16 bones, 16 image slots | 1.29–1.50 ms | 0.93 ms |
| 64 bones, 64 image slots | 5.2–6.2 ms | 3.6–4.0 ms |
| bone solve, 64 bones | 0.003 ms | — |
| mesh, 200 triangles | 5.6–7.1 ms, **visible seams** | **2.2–2.4 ms, no seams** |
| mesh, 800 triangles | 9.9–13.2 ms | 2.4 ms |

tiny-skia has no textured-triangle call: a mesh drawn as one anti-aliased path per triangle
shows a light line along every shared edge (the conflation artefact —
`docs/round35/import/mesh-tiny-skia.png` against `mesh-ours.png`), and its pattern shader
runs on its slow (`f32`) pipeline. Our prototype rasteriser — one triangle at a time, the
span of each row solved from the edges, u/v stepped incrementally, a top-left fill rule so
a shared edge is drawn once, nearest or bilinear sampling in fixed point, premultiplied
source-over — draws seamless meshes and frames 1.4–3× faster, and it is the same code for
sprite frames, skeleton images, meshes and Lottie image layers.

**Rive** was measured by what reading and building it would take, not drawn:
`rive-app/rive-runtime` (MIT) is 1 936 C++ source files (12.7 MB) under `src/` and
`include/`, **364 object kinds** (`include/rive/generated/*_base.hpp`: 104 for animation,
50 data binding, 42 view models, 39 shapes, 22 text, 19 constraints, 13 layout, 7 bones,
6 scripted…), changed the day of this measurement, and now carries **scripting** — a Luau
VM and a WebAssembly runtime (WAMR) for files with scripts (`animascript*.riv` in its own
tests). It builds with premake5 and **clang-cl** on Windows (this machine has the Visual
Studio 2022 Build Tools without the clang-cl component); its renderer is GPU-only (Metal,
Vulkan, D3D11/12, GL) behind an abstract `Renderer` a tiny-skia bridge would implement
(paths, gradients, clips, blend modes, image meshes). `rive-rs` (MIT) wraps the same C++
runtime and draws through Vello (GPU). The editor is Rive's own hosted application; third-
party pricing pages (2026) say exporting `.riv` needs a paid plan, Rive's own pages are
unclear (not confirmed). The runtime's own test files (`tests/unit_tests/assets/*.riv`,
579 of them, MIT by the repository) would serve as fixtures.

### Each format, by licence and by what it can do

| Format | The format's licence; what we would use | What it expresses | Where characters come from (usual licences) | Attack surface | Cost here |
|---|---|---|---|---|---|
| **Lottie** (`.json`, `.lottie`) | Specification under the Community Specification License 1.0, its code MIT (`lottie/lottie-spec`, Linux Foundation's Lottie Animation Community); players MIT (lottie-web, thorvg), Apache/MIT (velato). **We use our own player**, no dependency. | Vector shapes, gradients, trims, repeaters, masks, mattes, precomps, parenting, images, text (as glyphs), markers; **expressions are JavaScript** | After Effects + Bodymovin, **Glaxnimate** (KDE, GPL — free, writes Lottie), Synfig (GPL), Lottie Creator, LottieFiles (the *Lottie Simple License*: use, change, commercial use, credit optional; per-animation limits on premium files) | Large JSON; deep nesting; huge keyframe and vertex counts; self-referencing precomps; expressions; external image paths and URLs; `.lottie` is a zip (bombs, `..` entries) | Large: our player, ~3–4 k lines with tests |
| **Aseprite sprite sheet** (`.json` + `.png`) | Aseprite is source-available (its EULA); the JSON it writes is plain data. We use `png` (MIT/Apache, already in our tree) | Frames, frame durations, **tags** with direction (forward, reverse, ping-pong) and repeat; slices, layers | Aseprite, **LibreSprite** (GPL-2.0), itch.io and OpenGameArt packs (CC0, CC BY, CC BY-SA, GPL — per pack), Kenney (CC0) | PNG dimensions (a decompression bomb), frame counts, rectangles outside the atlas, paths in `meta.image` | Small: ~600 lines |
| **TexturePacker sheet** (`.json` hash or array + `.png`) | TexturePacker is commercial (free basic); the JSON is plain data; **free-tex-packer** (MIT) writes the same | Frames with rotation (90° in the atlas), trimming, pivots; animations as `animations` lists (Pixi's form) or as numbered names (`walk_01`…) | Game-art packs, any packer | As Aseprite | Small: shares Aseprite's reader |
| **DragonBones** (`_ske.json`, `_tex.json`, `_tex.png`) | Runtimes MIT (DragonBonesJS, DragonBonesCPP, active 2025–2026); the format is open JSON. We use our own reader | Bones (a tree), slots, image displays, **meshes with weights**, free-form deformation, IK, z-order, colour, display switching, named animations with tween curves, nested armatures | DragonBones Pro (discontinued); the team now points to **LoongBones**; existing game assets (per pack); Spine-to-DragonBones converters | As sprite sheets, plus bone and vertex counts, cyclic parents, nested armatures | Medium: ~1.5 k lines on the shared rasteriser |
| **Rive** (`.riv`) | Runtimes MIT; the format documented; the editor proprietary and hosted | Vector shapes, bones, meshes, constraints, **state machines with inputs** (the best fit for our numbers), layout, data binding, text, **scripts (Luau, WebAssembly)** | Rive's editor (export reported as a paid feature); Rive's community (per-file licences) | A binary format of 364 kinds; scripts | Very large (§4) |
| glTF 2.0 / VRM | glTF royalty-free (Khronos); VRM open (VRM Consortium); each model's licence is in its own metadata (VRM `meta`) | Skinned 3D characters, morph targets, materials (MToon) | VRoid Studio, Blender, VRoid Hub | Binary buffers, textures | A 3D renderer — not tiny-skia; **later** (BACKLOG) |
| Spine | The Spine Runtimes License: integrating them is allowed only if **each user of the product obtains their own Spine Editor licence** | Like DragonBones, more | Spine (paid) | — | **Out**: a free GPL app cannot require every user to buy a licence |
| Live2D | Cubism SDK under the Live2D Proprietary Software License (Cubism Core closed); an application that loads users' models is an **"Expandable Application"** that needs a special publication agreement reviewed by Live2D | Layered 2D puppets | Live2D Cubism (paid) | — | **Out**: proprietary core, an agreement per release |

## Decision

### 1. This round: Lottie, Aseprite and TexturePacker sheets, and DragonBones; Rive later

- **In:** **Lottie** (`.json`, and `.lottie` — the zip LottieFiles hands out), **sprite
  sheets** from **Aseprite** and **TexturePacker** (hash and array; free-tex-packer and
  LibreSprite write the same), and **DragonBones** (JSON 5.x). The measurement says each
  can be done well here: DragonBones costs a bone solve (microseconds) and the image
  rasteriser that sprite sheets need anyway (§3); with it, skeletal characters made for
  games come in too, and the format's licence and runtimes are MIT.
- **Rive: not this round** — said in the import dialog's help and in the manual, and kept
  in BACKLOG §6i with what it takes: (a) the C++ runtime built in CI on Windows (clang-cl
  and premake added to the toolchain), Linux and macOS, behind an FFI crate (the only
  place for `unsafe`; `cantus-dance` stays `forbid(unsafe_code)`), with its scripting VMs
  (Luau, WAMR) compiled out; (b) a tiny-skia implementation of its `Renderer` — paths,
  gradients, clips, blend modes and image meshes on our rasteriser; (c) its state
  machines' inputs bound to our numbers by name (`move`, `energy`, `mood`…), which is what
  makes Rive worth it; (d) fuzzing a C++ parser from Rust; (e) a moving target — the
  runtime changes weekly and the editor's export is reported to be paid. Measured cost:
  a round of its own. A Rust reader of a subset (shapes, bones, linear animations, state
  machines: well over a hundred of the 364 kinds) costs as much and would lag the format.
- **Later:** glTF 2.0 / VRM (a 3D renderer first; BACKLOG §6i). **Out:** Spine (each
  user would need a Spine Editor licence) and Live2D (proprietary core; an "Expandable
  Application" agreement per release) — said in the manual.

### 2. Lottie: our own player over tiny-skia — no dependency

`velato` is rejected: valid files panic (four of six of our variants), nothing is bounded,
and its maths is the platform's. Its model is a good reference (MIT/Apache); none of its
code is copied. Ours (`cantus-dance/src/import/lottie/`):

- **Reads** `v`, `fr`, `ip`, `op`, `w`, `h`, `meta`, `markers`, `assets` (precomps,
  images), `layers` (shape 4, null 3, precomp 0, image 2, solid 1, text 5).
- **Draws** groups, paths, rectangles (rounded), ellipses, polystars, fills and strokes
  (with dashes, caps, joins, miter), linear and radial gradients (with opacity stops),
  trim paths (simultaneous and individual), repeaters, rounded corners, transforms with
  every keyframe easing (hold, linear, cubic-bezier, spatial tangents on positions),
  split positions and rotations, skew, 3D layers flattened to 2D (`rz` as rotation, `rx`/
  `ry` ignored and said), parenting through null and any layer, time remapping and
  stretch, precomps (nested ≤ 16 deep, a cycle refused), masks (add, subtract, intersect;
  inverted; opacity) and track mattes (alpha, inverted alpha; luma by its alpha-weighted
  luminance), blend modes tiny-skia has (the rest drawn as normal and said), image layers
  (embedded `data:` PNG/JPEG, or a file inside the package), solid layers, and text
  layers drawn **only** from glyph shapes the file carries (`chars`), else skipped and said.
- **Ignores and says**: expressions (`x` on any property — never evaluated; the property's
  own keyframes play), effects other than the few fills/tints it can draw, `.lottie`
  state machines and themes, audio layers.
- **Parsed** by a bounded pre-scan of the bytes (depth, counts, sizes; §5) and then into
  our own model through `serde` — once, at load, off the interface's thread.
- **Its markers are moves** (§4); with none, the whole timeline is one loop.
- **`hue`** turns every colour it draws (`Canvas::color`), gradients and images included
  (images through the same matrix, at load, cached per hue step).
- **Holding the budget**: a heavy file is drawn into a **frame cache** — each of the file's
  own frames (its frame numbers are integers at its `fr`) rasterised once at the pixel
  scale it is shown at, then placed on the stage by our rasteriser (§3); a cache bounded in
  bytes, emptied when the scale or the hue changes. A light file draws directly. The card
  shows the **measured cost** of the character on a 600×600 stage, and says so when it is
  over the budget.

### 3. One image rasteriser of our own (`import/raster.rs`)

Sprite frames, DragonBones images and meshes and Lottie images draw through one
textured-triangle rasteriser on the stage's pixmap (premultiplied RGBA, as tiny-skia's):
affine u/v per triangle, the row's span solved from the three edges, a top-left rule (a
shared edge drawn once — no seams), **nearest** (pixel art: Aseprite's default, a switch on
the card) or **bilinear** sampling in fixed point, source-over with the pose's opacity and
the slot's colour. Only `+ − × ÷` on `f32`/integers — the same pixels on every system
(ADR 0087 §3), no transcendental, no `floor` call. It is benched with the cast's renderer
bench (≤ 1.5 ms per character at 600×600) and improved where the bench says.

### 4. Moves, moods and the beat

- **The mapping.** Each of our sixteen moves is given one of the file's animations — a
  Lottie marker (`cm`, with its `tm`/`dr` span), a sprite sheet's tag (Aseprite
  `frameTags`; TexturePacker `animations`, else names sharing a prefix before a number:
  `walk_01`, `walk_02`…), a DragonBones animation. It is **guessed by name** (lower-cased,
  accents and separators dropped, a whole word matching): `idle stand rest breathe wait /
  reposo quieto parado esperar respirar` → Idle; `bounce dance groove bob nod / rebote
  bailar baile` → Bounce; `step_left left walk_left / paso_izquierda izquierda` →
  StepLeft; `step_right right walk walk_right / paso_derecha derecha caminar` →
  StepRight; `sway rock / balanceo mecer` → Sway; `clap applause / aplauso aplaudir
  palmas` → Clap; `wave hello hi greet / saludo saludar hola` → Wave; `jump hop leap /
  salto saltar brinco` → Jump; `spin turn rotate / giro girar vuelta` → Spin; `kick /
  patada` → Kick; `shuffle run / correr pasito` → Shuffle; `point / señalar apuntar` →
  Point; `arms_up cheer hands_up celebrate victory / brazos_arriba celebrar festejar` →
  ArmsUp; `twist / torsion` → Twist; `bow curtsy / reverencia` → Bow; `freeze pose stop /
  congelar` → Freeze. The card shows every move with its guess, **every one editable**
  (any animation, or *none*), and what a move with none plays (`Move::fallback`, followed
  until a mapped move; with nothing mapped at all, the first animation is every move).
  `has_move` is true exactly for the mapped moves.
- **Moods.** An animation named after a mood (`happy excited calm surprised sleepy cool
  shy` and `feliz emocionado tranquilo sorprendido dormido genial timido`) plays in place
  of Idle while that mood is chosen; otherwise a mood changes nothing in an imported
  character (said on the card: "Moods: none in this file").
- **The beat.** A move's frames span its `move_beats` at the song's tempo: the pose's
  phase 0…1 runs through the animation's frames. An animation's beats are its natural
  length at 120 BPM rounded to 1, 2, 4 or 8 (a half-second clap is one beat, a two-second
  wave four), editable per move on the card. *Its own speed* (a switch on the card) plays
  the file's frames at its own frame rate from the song's seconds (`pose.seconds`) instead
  — still a pure function of the song.
- **Placement.** The character's units come from the file's box: the union of every
  mapped animation's bounds (sampled at load), its bottom centre the feet, its height one
  unit; a sprite sheet's `sourceSize` and pivot, a DragonBones armature's `aabb`.

### 5. Data, never code: the sandbox and its bounds

- **No script runs**: Lottie expressions, `.lottie` state machines, anything executable —
  ignored, and the card says what was ignored. Nothing is fetched: a URL anywhere (an
  image `u`/`p`, a `meta.image`) is refused for that item and said.
- **Nothing is read outside the file's folder**: every path a file names is relative,
  without `..`, a drive, a root, a scheme or a NUL, and it resolves inside the folder after
  the operating system's own canonicalisation (a link out of it is refused). Inside a
  project, `file:` paths obey the same rule from the project's folder — a project from
  someone else cannot make Cantus read elsewhere.
- **Every size is bounded before memory is given** (one `Limits` table, tested at each
  edge):

| What | Bound |
|---|---|
| a JSON file (Lottie, a sheet, a skeleton, an atlas) | 16 MiB; nesting 96 deep (a pre-scan before parsing) |
| a `.lottie` zip | 32 MiB; 256 entries; 64 MiB uncompressed in all (from the central directory, then counted while inflating) |
| an image | 32 MiB of file; 4 096 × 4 096 pixels (read from the header before decoding); 32 M pixels in all per character |
| Lottie | 2 000 layers (20 000 drawn a frame, precomps counted each time used); 16 precomp levels, no cycle; 50 000 shape items; 200 000 keyframes; 500 000 path vertices; 100 000 frames of timeline; 1 000 repeater copies in all; 256 masks; 256 markers; 64 canvas-sized buffers (mattes, masks, clips) a frame; an id given twice keeps its first |
| sprite sheets | 4 096 frames; 256 tags; every rectangle inside its atlas |
| DragonBones | 1 000 bones (a tree — a cycle refused); 1 000 slots; 65 536 mesh vertices and 131 072 triangles in all; 256 animations; 100 000 keyframes; nested armatures 4 deep and 2 000 drawn a frame (counted once per armature before anything is walked) |
| names and texts | 256 bytes each (longer: cut, said) |

- **Errors are words** (`ImportError`, both locales on the card), never a panic. Every
  parser has a **fuzz test**: thousands of seeded mutations (bytes flipped, cut, repeated;
  numbers made huge, negative, NaN; structures nested and duplicated) of a valid file —
  none panics, none exceeds the bounds, each either loads or says why.

### 6. Sprite sheets and DragonBones

- **Aseprite**: `frames` as hash or array, each frame's `duration` (ms) giving the frame's
  share of its move's cycle, `frameTags` with `direction` (`forward`, `reverse`,
  `pingpong`, `pingpong_reverse`) and `repeat`; `slices` and `layers` ignored; trimmed
  frames placed by `spriteSourceSize`/`sourceSize`. Nearest sampling by default.
- **TexturePacker** (hash and array): `rotated` frames (stored 90° clockwise), `trimmed`,
  `pivot`; animations from `animations` or numbered names. Bilinear by default.
- **`hue`** tints images through the same matrix as `Canvas::color`, applied to the decoded
  atlas once per hue step (cached), never per pixel per frame.
- **DragonBones** JSON 5.0–5.5 (`version`, `compatibleVersion` read; older or binary
  `.dbbin` refused in words): armatures (the first, or `defaultActions`), bones with
  their transforms (x, y, skew, scale), inherited or not; slots with display index, colour
  and blend mode; displays: **image**, **mesh** (vertices, uvs, triangles, **weights** to
  bones), nested **armature** (≤ 4 deep); animations with bone, slot (display, colour),
  **deform** (FFD) and **z-order** timelines, tweens with `tweenEasing` and `curve`
  (bezier), `playTimes`; **IK** constraints (one and two bones, bend direction, weight).
  Bounding boxes, events, sounds and actions ignored and said. The atlas (`_tex.json`,
  `SubTexture` with `rotated` and frame offsets) and its PNG beside it.

### 7. The package, the licence and the project — no schema change

- **The copy.** *Use* copies the file and everything it names (atlas, images; a
  `LICENSE`/`license.txt`/`licence.txt`/`COPYING`/`README` beside it) into
  `<project folder>/Characters/<name>/`, adds `character.json` — format, original file
  name, its SHA-256, the licence, the author, the mapping and per-move beats, nearest or
  bilinear, own speed — and sets the dancer's `character` text key to
  `file:Characters/<name>/<file>` in **one undo step**. An untitled project keeps the copy
  in Cantus's own folder for untitled work and the key absolute *there only*; the save's
  collect step (as audio: `collect_audio`) moves it beside the project and makes the key
  relative. *Save As* carries `Characters/` the same way.
- **The mapping lives with the copy** (`character.json`), shared by every dancer that
  uses it; editing it later (*Edit mapping…* from the dancer's window) rewrites that file —
  an asset's file, like an audio file's peaks, not the project's model. So **no schema
  bump**: `character` exists since schema 49.
- **The loader**: `import::load(project_dir, rel) -> Result<Arc<dyn Character>,
  ImportError>`, cached by path and content hash; a missing or broken file draws the
  default character and the dancer's window says why — never an error dialog at load.
- **The licence** (`Licence`): **stated** — a field of the file where the format has one
  (Lottie `meta`, a `.lottie` manifest), or a licence file beside it (its first line, and
  the file kept) — or **chosen** by the person on the card from a short list: *my own
  work*, CC0 1.0, CC BY 4.0, CC BY-SA 4.0, the Lottie Simple License, *other* (their
  words), or **unknown** ("make sure you may use it"). The card adds one plain line when
  the licence asks something: credit (CC BY), share-alike (SA), non-commercial (NC — music
  videos made with it may not be sold). Shown in the picker's licence line and kept in
  `character.json`.

### 8. The flow

*Import character…* (the dancer's picker) → a file dialog (`.json`, `.lottie`) → the file
is checked off the interface's thread → **the card**: the preview dancing to the song's
tempo (the routine, or the move under the pointer), the name, the format, the author, the
licence (stated, or the list), what was found (moves, moods, layers or frames or bones,
size, the measured cost on a 600×600 stage) and what was ignored (in words); **the
mapping** — sixteen rows, our move ↔ the file's animation (a guess shown as such), its
beats, a preview on hover; *Use* / *Cancel*. All in the kit (ADR 0024), both locales
(`dance_import` area), both themes, keyboard and screen reader.

## Consequences

- **No new crate in the tree**: `png` 0.18, `zip` 9 (deflate only), `sha2` 0.10 and
  `zune-jpeg` 0.5 are already in `Cargo.lock` with their notices (MIT/Apache-2.0/Zlib);
  `cantus-dance` names them as direct dependencies. `velato` is not added.
- **Ours to maintain**: a Lottie player (the largest piece), sheet readers, a DragonBones
  reader and an image rasteriser — every line tested, with the specification's examples
  as a measurement corpus kept outside the repository (their licence for redistribution is
  not stated per file) and our generated files as the committed fixtures.
- **What a Lottie file cannot do in Cantus**: run its expressions (the keyframes play
  instead — most character files animate without them), use fonts it does not carry, or
  apply After Effects effects beyond fills and tints. Each is said on the card, not
  silently dropped.
- **Rive waits** for a round that can take a C++ runtime into CI on three systems;
  BACKLOG §6i says what it takes (§1).
- **The budget**: light characters draw within 1.5 ms directly; heavy Lottie files through
  the frame cache; the card shows each import's measured cost, so a person sees a heavy
  file's price before using it.

## What the tests hold

- Each format loads our generated character (sixteen moves as markers, tags or
  animations) and dances: the routine to the beat, an automated `move`, a note — the same
  poses and the same pixels at 30 and 144 frames a second (`pose_at` is pure).
- The mapping guesses every English and Spanish name of §4; a move with no mapping plays
  its fallback; a mood animation plays in place of Idle.
- A move's frames start on its bar or note and end on its last beat at 90, 120 and 174 BPM.
- Hostile files: every bound of §5 at its edge loads and one past it is refused in words;
  `..`, absolute paths, drives, URLs, links out of the folder and zip entries escaping it
  are refused; a self-containing precomp, a cyclic skeleton and nested armatures past 4
  are refused; expressions are never evaluated (a file whose expression would change a
  pixel draws its keyframes); the fuzz tests (a seeded stream per parser) never panic.
- The rasteriser: a mesh draws without seams (no pixel along a shared edge lighter than
  both neighbours); a frame of a pixel-art sheet at 4× is exact blocks; the same input
  gives the same bytes; its bench and the cast's renderer bench hold ≤ 1.5 ms for our
  generated characters at size 1 on 600×600.
- *Use* is one undo step (the key, and the copy kept); the licence is in `character.json`
  and on the card; an untitled project's character moves beside the project on save and
  *Save As* carries it; a missing character draws the default and says so.

## Sources

- Lottie specification and its licences: <https://github.com/lottie/lottie-spec>
  (`License.md`: Community Specification License 1.0; code MIT), examples
  `docs/static/examples/*.json`.
- velato: <https://github.com/linebender/velato>, crate 0.12.0 (Apache-2.0 OR MIT), source
  read at `src/import/converters.rs` (the `todo!`/`unimplemented!` arms).
- Rive: <https://github.com/rive-app/rive-runtime> (MIT; build, renderers, scripting),
  <https://github.com/rive-app/rive-rs> (MIT; Vello), pricing:
  <https://rive.app/docs/account-admin/pricing>, <https://vijaytalksai.com/rive-pricing-explained/>.
- DragonBones: <https://github.com/DragonBones/DragonBonesJS> (MIT, points to LoongBones),
  <https://github.com/DragonBones/DragonBonesCPP> (MIT).
- Spine: <https://esotericsoftware.com/spine-runtimes-license>.
- Live2D: <https://www.live2d.com/en/sdk/license/>.
- LottieFiles' licence: <https://help.lottiefiles.com/animation-licensing-basics->, the text
  as reproduced by Qt: <https://doc.qt.io/qt-6/qtlottieanimation-attribution-happy-star.html>.
- Editors: LibreSprite (GPL-2.0) <https://github.com/LibreSprite/LibreSprite>, Piskel
  (Apache-2.0) <https://github.com/piskelapp/piskel>, free-tex-packer (MIT)
  <https://github.com/odrick/free-tex-packer>, Glaxnimate (KDE)
  <https://invent.kde.org/graphics/glaxnimate>, Synfig (GPL-3.0)
  <https://github.com/synfig/synfig>.

## What landed (round 35, import builder)

The decision above, built in `cantus-dance/src/import/` (GPL) and `cantus-ui`'s
`dance_import.rs` and `views/dance_import.rs`; `docs/round35/import.md` has the detail.

- **Formats**: Lottie (`lottie/`: `model.rs` reads, `prop.rs` animates, `geom.rs` makes
  outlines, trims and rounded corners on the curves themselves, `draw.rs` paints) and
  `.lottie` packages; Aseprite and TexturePacker sheets (`sheet.rs`); DragonBones 5.x
  (`dragonbones.rs`). The words that name moves grew four Spanish ones to match the
  dancer's own names (*vaivén*, *palmada*, *pirueta*, *congelado*).
- **One rasteriser** (`raster.rs`): within one step of tiny-skia's blending (a test), a
  warped mesh with not one pixel skipped or drawn twice (a test), pixel art at 4× in
  exact blocks (a test).
- **Measured** (`examples/import_bench.rs`, release, this machine, our generated figure at
  size 1 on 600×600): Aseprite 0.42 ms a frame, TexturePacker 0.54, Lottie 0.95, `.lottie`
  0.93, DragonBones 0.69 — all within the 1.5 ms budget. Heavy Lottie files (more than
  120 paints, or masks and mattes) draw through the frame cache.
- **Hostile files**: every parser fuzzed with seeded mutations (30 000 files in a long run,
  `CANTUS_FUZZ_ROUNDS=6000`; 150 per format in the suite) — no panic; precomps and
  armatures that loop or nest past their bound, bone loops, rectangles outside their
  atlas, paths out of the folder and URLs are refused in words. velato's four panics play.
- **The card** opens from `cantus_ui::dance_import::open(app, track)` (the dancer's
  picker), checks the file on a thread, and *Use* copies it and sets `character` in one
  undo step (`cmd.use_character`); *Edit moves…* (`edit_moves`) rewrites `character.json`
  for every dancer using it. `collect_on_save` (a marked line in `save_job.rs`) carries the
  characters of an untitled project, and of *Save As* into another folder.
- **Reachable today** from *File ▸ Import character…* (the selected dancer track) as well
  as from the dancer's picker; `import::import(path)` of a copy reads the `character.json`
  beside it, so a caller that loads by path still gets the card's choices.
- **Review 1** (`docs/round35/import.md`, part 3): duplicate ids, nested-armature
  instances, markers and canvas buffers bounded as in the table; the hue turned per pixel
  in the rasteriser (no turned copies of an image or an atlas); *Use* and the cost measured
  on threads; a save beside another project's character of the same name carries ours to
  `<name>-2` and rekeys its dancers; Windows's device names never become folders; every
  read behind a `catch_unwind` boundary (`import::guarded`) and a drawing that panics left
  out, never the app.
- **Review 2**: copies live in `Characters/<name>-<8 hex of the file's hash>/`; a save
  beside another version of one of them is refused in words (no key ever renamed, nothing
  to undo); DragonBones weighed per frame (slots, corners, vertices, triangles of every
  instance) and one depth limit; Lottie buffers only for mattes and masks; a *Use* copy
  tied to its project and dancer, removed when it can no longer be used; heavy frames
  cached at hue 0.
- **Review 3 — content-addressed, immutable folders** (supersedes §7's "editing it later
  rewrites that file" and review 2's refusal): a copy lives in
  `Characters/<name>-<8 hex>/`, the hash over every file of the package (path, length,
  bytes, in path order) and its choices (mapping, sampling, pace, licence); a folder is
  never edited in place. *Edit moves…* writes a new package (or reuses an identical one)
  and re-keys this project's dancers in one undo step (`cmd.edit_character_moves`, an exact
  inverse: the old folder is still there). Two projects in one folder share only identical
  content; a carry finding the name verifies the content and, if a hand changed it, copies
  ours to a fresh name and re-keys (`cmd.carry_characters`); a missing source is said.
  Folders this project no longer uses are left alone (they may be another project's). A
  *Use* that finishes after the project moved folders is carried there first; DragonBones
  posing (bones, IK solves per instance) is weighed; the box's sampling stays within its
  bound.
