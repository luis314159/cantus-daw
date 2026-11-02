# Round 35 — import: characters people bring (ADR 0089)

## Part 1 — the evaluation (2026-10-31)

Measured before building, as the brief asks; the decision and every number are in
[ADR 0089](../adr/0089-imported-characters.md). The prototypes are kept in
[`import/prototype/`](import/prototype/) (a crate of its own, outside the workspace):

```
cd docs/round35/import/prototype
cargo run --release -- velato <folder of Lottie files>   # velato 0.12: parse, panics
cargo run --release -- lottie - <out>                     # our Lottie player: parse, frame
cargo run --release -- sprite - <out>                     # PNG atlases: decode, tiny-skia frame
cargo run --release -- bones - <out>                      # skeleton and mesh through tiny-skia
cargo run --release -- own - <out>                        # the same through our rasteriser
```

**The decision (proposed, for the integrator to confirm):**

- **In this round:** Lottie (`.json`, `.lottie`) by **our own player** over tiny-skia;
  Aseprite and TexturePacker **sprite sheets**; **DragonBones** JSON 5.x (bones, slots,
  images, weighted meshes, deform, IK, z-order).
- **Not this round:** Rive (a 12.7 MB C++ runtime with 364 object kinds and two script
  VMs, a clang-cl/premake build in CI on three systems, an FFI crate, a tiny-skia bridge
  for its renderer; BACKLOG §6i). Later: glTF/VRM. Out: Spine, Live2D (their licences).
- **Why not velato:** four of six valid variants of our own Lottie file panic (`todo!` on a
  transform without `r`, split rotations, split positions; `unimplemented!` on blend
  *Add*); no bounds.
- **Why our own image rasteriser:** tiny-skia draws a mesh with seams along every shared
  edge (`import/mesh-tiny-skia.png` against `import/mesh-ours.png`) and its pattern
  shader is slow; ours is seamless and 1.4–3× faster (a mesh of 200 triangles 2.3 ms
  against 5.6–7.1 ms), shared by sheets, skeletons, meshes and Lottie images.
- **No schema change**: the mapping and the licence live in `character.json` beside the
  copy in `Characters/<name>/`; the dancer's `character` key (schema 49) points there.

## Part 2 — the importers, the card, the project (2026-10-31)

Confirmed by the integrator as proposed (formats, no schema 50, marked lines in
`save_job.rs`). ADR 0089 is accepted, with a *What landed* section.

### What was built

| Piece | Where | Tests |
|---|---|---|
| Limits and the JSON pre-scan (bytes, depth, values before parsing) | `cantus-dance/src/import/limits.rs` | edges of every bound |
| The folder sandbox (no `..`, root, drive, scheme, stream, NUL; links out refused) | `import/paths.rs` | every escape refused |
| Our image rasteriser (textured triangles, top-left rule, nearest/bilinear in fixed point, source-over, tint) | `import/raster.rs` | blending within 1/255 of tiny-skia; a warped mesh with no gap and no double blend; 4× pixel art exact; nonsense vertices draw nothing |
| PNG/JPEG decoding, sizes from the header first; `data:` URIs; hue turn | `import/image.rs` | |
| Moves and moods by English and Spanish names; beats; a hostile mapping made whole | `import/mapping.rs` | every word of every move |
| An import as a `Character` (clip and time from the pose; routine from what it has; the frame cache for heavy files) | `import/imported.rs` | |
| Aseprite and TexturePacker sheets (tags with direction and repeat, durations, rotated and trimmed frames, `animations` or numbered names; the box from the art's own pixels) | `import/sheet.rs` | `tests/import_sheets.rs` (6) |
| Lottie: our player (layers, parents, precomps with time remap and stretch, masks, track mattes, shapes, gradients, dashes, trims, repeaters, rounded corners, stars, images, solids, text from glyphs, blend modes; expressions never run) and `.lottie` packages | `import/lottie/` | `tests/import_lottie.rs` (10 + the specification's examples, ignored, `CANTUS_LOTTIE_CORPUS`) |
| DragonBones 5.x (bones, slots, images, weighted meshes, deform, display and colour timelines, z-order, one- and two-bone IK, nested armatures) | `import/dragonbones.rs` | `tests/import_dragonbones.rs` (5) |
| `character.json`, the copy, the loader with its cache, carrying, licences | `import/package.rs` | `tests/import_package.rs` (8) |
| The fuzz test per parser | `tests/import_fuzz.rs` | 5 |
| Our test figures in every format | `import/samples.rs`; `examples/import_sheet.rs --samples <dir>` writes them | |
| The card, the flow, *Edit moves…*, collecting on save | `cantus-ui/src/dance_import.rs`, `views/dance_import.rs` | `cantus-ui/tests/dance_import.rs` |
| Strings | `locales/areas/dance_import.{en,es}.txt` | the i18n test |
| Manual | `docs/manual/{en,es}/import-characters.md` | |

### The numbers

- **Cost a frame** (`cargo run --release -p cantus-dance --example import_bench`, our
  figure at size 1 on a 600×600 stage, this machine — Ryzen 7 7735HS): Aseprite 0.42 ms,
  TexturePacker 0.54, Lottie 0.95, `.lottie` 0.93, DragonBones 0.69. Budget 1.5 ms.
- **Load**: 6–25 ms for our files (parse, decode, the sampled box).
- **Fuzz**: 30 000 mutated files (6 000 per format) in a long run, none panicked; the suite
  runs 150 per format (1.8 s).
- **The specification's 20 example files** load and play every sampled frame.

### Tests

79 of mine: 35 unit tests in `cantus-dance/src/import/`, 36 in `cantus-dance/tests/import_*.rs`
(sheets 6, Lottie 10 + 1 ignored corpus run, DragonBones 5, package 9, fuzz 5), 8 in
`cantus-ui/tests/dance_import*.rs` (the card 6; end to end 2: import into an untitled
project → *Use* is one undo step → the first save and *Save As* carry the copy → the project
opened again draws it; *Edit moves…* reaches two dancers sharing one file). Windows: all
pass; clippy `-D warnings` and fmt clean on `cantus-dance` and `cantus-ui`. Linux (WSL,
`F:\projects\daw-r35\import-wsl.sh`, after merging the dancer's, cast's, stage's and
video's first deliveries): workspace clippy and fmt clean; the card, end-to-end, kit lint
and i18n tests pass; every `cantus-dance` import test passed on Linux before the dancer's
merge. After it, `cantus-dance`'s `choreo_song::notes_play_their_moves_from_their_starts`
(the dancer's test with the cast's songbird — the integration's, not this branch's) fails
on Windows and Linux alike and stops that binary's run.

### Shots

`docs/round35/import/`: the card for a `.lottie` package (English, Spanish, Paper), an
Aseprite sheet without a licence (the choices; Spanish), a DragonBones skeleton (and at
150 %), *Edit moves…*, a file that could not be read; the contact sheets of our figures
(`sheet-*.png`, every clip at four moments); the mesh seam comparison.

### Marked lines (`// r35:import`)

- `crates/cantus-ui/src/lib.rs`: `pub mod dance_import;`
- `crates/cantus-ui/src/app.rs`: the field `pub dance_import: crate::dance_import::DanceImport`,
  its `Default` in `CantusApp`'s literal, and `crate::dance_import::frame(self, ctx);` beside
  `workflow::frame`.
- `crates/cantus-ui/src/save_job.rs`: first in `write_save`, a four-line block (review 2)
  `if !crate::dance_import::collect_on_save(self, path) { // r35:import … return; }` — a
  carry refused or failed stops the save, said (the `if` line's marker sits inside the
  block, where `cargo fmt` keeps it).
- `crates/cantus-ui/src/views/transport_menus.rs`: `crate::dance_import::file_menu_item(app, ui);`
  after *Import audio…* — **File ▸ Import character…** for the selected dancer track
  (dimmed, with a word why, for any other), so the flow is reachable from `cantus_ui::run`
  before the dancer's picker lands.
- `crates/cantus-ui/src/macos/menu.rs` (3 lines): `Pick::ImportCharacter`, its File-menu
  entry after *Import audio…*, and its arm in `run` (`crate::dance_import::menu_pick`) — the
  Mac's menu bar has it too. AppKit's items are not dimmed per frame, so with no dancer
  selected the status line says *Select a dancer track first* (a test, on every system:
  the model is plain Rust).

### For the dancer builder (calls, not files of mine)

- The picker's *Import character…* calls `cantus_ui::dance_import::open(app, track)`; the
  imported characters of the project are `cantus_ui::dance_import::imported_in_project(app)`
  (`Listed { rel, name, format, licence }`; the key is `file:` + `rel`).
- The window's *Edit moves…* (shown for a `file:` character) calls
  `cantus_ui::dance_import::edit_moves(app, track)`.
- **Found after the dancer's first delivery was merged** (`dance_live.rs`, the dancer's
  file): its `load` uses `cantus_dance::import::import(&dir.join(rel))` with the project
  file's folder, so (1) in an **untitled** project an imported character draws the default
  until the first save, (2) the key's path is joined without the sandbox's check, and (3)
  its cache by id never sees *Edit moves…* until Cantus restarts. `import()` now reads the
  `character.json` beside a copy (the mapping, licence and name), so the choices made on the
  card do apply. The fix is the dancer's, three lines:
  `let dir = cantus_ui::dance_import::project_dir(app);` and
  `cantus_dance::import::load(&dir, rel)`, with `file:` ids not cached forever (`load`
  keeps its own cache, checked by the files' stamps).
- `DanceLive` resolves `file:<rel>` with `cantus_dance::import::load(&cantus_ui::dance_import::project_dir(app), rel)`
  — sandboxed, reading `character.json` (the mapping, the licence) and seeing *Edit
  moves…* (its cache checks the files' stamps; a cache by id that never checks would not).
  `project_dir` is the untitled folder for a project not saved yet.

### What needs the owner

Nothing to build. To look at: the card and the figures (the shots above). Our test figure
is a test, not a character of the cast.

## Part 3 — review 1 (2026-11-01)

Each finding fixed with a test that fails without the fix.

| # | Finding | Fix | Test |
|---|---|---|---|
| 1 | Two Lottie assets of one id mapped past the precomps (a panic) | The first of an id is kept, said on the card (`note.duplicate_ids`); the fuzz repeats assets, armatures, layers and markers whole, and fuzzes a figure inside a precomp | `duplicate_asset_ids_keep_the_first_and_say_so`, `lottie_precomps_never_panic` |
| 2 | Nested DragonBones armatures multiplied without limit (200⁵) | Instances counted per armature once (memoised, with the depth and loop checks) before anything is walked: at most 2 000 a frame | `nested_armatures_cannot_multiply_without_limit` (refused in < 2 s) |
| 3 | Lottie markers had no limit | `Limits::markers` = 256 | `markers_are_bounded` |
| 4 | A save beside another project's `Characters/<name>` took that one | `carry` compares hash, choices and licence; another character there → ours goes to `<name>-2`, and the save rekeys every dancer wearing it in one step (`cmd.carry_characters`) | `carrying_onto_another_projects_character_of_the_same_name_renames`, `saving_beside_another_projects_character_of_the_same_name_rekeys` |
| 5 | A Lottie's unreadable external image opened but *Use* failed | Files the card already said could not be read are left out of the copy; what draws was read when the file was checked | `a_missing_external_image_is_said_and_the_rest_is_copied` |
| 7 | Hue automation: up to 64 turned copies per Lottie image, a whole atlas re-hued per degree | No turned copy anywhere: the rasteriser turns each pixel as it draws it (an 8.8 matrix on premultiplied colour, whole degrees); the caches are gone | `a_hue_turns_pixels_as_they_are_drawn_like_a_turned_copy` (within 3 steps of a turned copy); the bench's hue-sweep column |
| 8 | Interface-thread work | *Use* reads and copies on a thread (the card says *Copying…*, *Use* waits); the cost is measured on a thread once per card; a Lottie frame makes at most 64 canvas-sized buffers (mattes, masks, clips), each counted as 100 paints | `import_use_save_reopen_and_it_draws` (busy, then done), `masks_and_mattes_are_bounded_per_frame` |
| 9 | Windows's device names as folders | `con`, `prn`, `aux`, `nul`, `com1…9`, `lpt1…9` → `<name>-character` | `reserved_windows_names_are_mapped` |
| — | A hostile file must never take the app down | `import::guarded` (a `catch_unwind` boundary) around every read — `inspect`, `import`, `load` — turning a panic into `ImportError::Invalid`; a panic while drawing marks that character broken and the frame goes on without it | `import_guard.rs` (2) |

(6, the dancer's live loader and picker, went to the dancer builder.)

Measured after the review (release, this machine, 600×600, size 1; `import_bench`'s new
*hue sweep* column turns the hue one degree a frame): Aseprite 0.34 ms a frame / 0.38 in a
sweep, TexturePacker 0.43 / 0.45, Lottie 0.54 / 0.47, `.lottie` 0.48 / 0.47, DragonBones
0.44 / 0.54 — a sweep costs what a frame costs, with no copy of any image kept.

Checked after the review on Windows (clippy `-D warnings`, fmt, every import test) and on
Linux (WSL at `5db5c84a`: workspace clippy and fmt clean; `cantus-dance` import tests 71 +
44, the card 7, end to end 3, kit lint 4, i18n 2 — all pass).

## Part 4 — review 2 (2026-11-01)

| # | Finding | Fix | Test |
|---|---|---|---|
| 1 | Nested DragonBones armatures were counted but not weighed; a memoised armature skipped the depth check; `walk` cut silently at 4 | Each armature weighed once (memoised): its instances, its **work** (a slot, an image's 4 corners, a mesh's vertices + triangles — every instance its own) and its **height**; refused before walking past `Limits::frame_work` (2 000 000 a frame), 2 000 instances or the one depth limit, wherever an armature is reached from; meshes' uvs and triangles shared (`Arc`), never cloned per instance; the box's sampling bounded by work (`MEASURE_WORK`); `walk` uses the same limit | `nested_armatures_are_weighed_by_what_they_draw`, `the_depth_limit_holds_through_a_shared_armature` |
| 2 | Every visible Lottie layer took a canvas buffer (+100 paints): mattes dropped after 64 layers, drawing stopped near 500 | A buffer only for a layer with a matte (or masks), only when drawing pixels | `plain_layers_take_no_buffer_and_a_late_matte_still_cuts` (600 layers, a matte drawn last) |
| 4 | A renaming save left an undo that pointed at the other project's character; a part-way failure lost renames | No renaming, so no step to undo: folders are named by the character and its file's hash (`Characters/<name>-<8 hex>/`), so two projects in one folder meet only for the very same file; then the same version is shared, another version (its moves edited) **refuses the save in words** — all or nothing, nothing half copied, the errand waiting on it dropped (`save_job.rs`: three marked lines) | `carrying_onto_another_version_of_the_character_is_refused_whole`, `saving_beside_another_version_of_the_character_is_refused` |
| 5 | The *Use* copy was not tied to its project or dancer | The copy carries its project's epoch and folder: another project coming in closes the card; *Cancel* while copying, a project change or a track no longer a dancer removes the folder the copy made (never one it reused) and sets nothing | `a_copy_that_can_no_longer_be_used_is_removed` (project, cancel, track) |
| 6 | The heavy-frame cache kept a frame per hue | Frames cached at hue 0, turned as they are placed | the heavy-file test: six hues, one cached frame, the same shape |
| 7 | `COM¹`…`LPT³` passed (5 bytes) | Compared by characters, the superscripts included | `reserved_windows_names_are_mapped` |

(3, DanceLive caching a miss, went to the dancer builder.)

## Part 5 — review 3 (2026-11-01)

**Content-addressed, immutable character folders.** `Characters/<name>-<8 hex>/`, the hash
over every file of the package and its choices (`package::content_hash`); `place` reuses a
folder only when its own files hash the same (`folder_hash`), else writes a fresh one.
*Edit moves…* → `package::with_settings` makes a new package and `dance_import::rekey`
points this project's dancers at it in one undo step. Carrying shares identical content,
moves ours to a fresh name beside a hand-edited folder (the keys follow), and reports a
missing source. No save is refused for "another version" any more.

| # | Fix | Test |
|---|---|---|
| design | the above | `folders_are_named_by_all_their_content_and_carried_by_it`, `two_dancers_sharing_a_file_both_follow_edit_moves` (immutable, same edit → same folder), `edit_moves_reaches_every_dancer_using_the_character` (re-key + one undo), `saving_beside_another_version_of_the_character_keeps_both` |
| 1 | A *Use* finishing after *Save As* / the first save is carried to the project's folder before its key is set; a missing source is said (`status.missing`) | `a_copy_finished_after_the_first_save_is_carried_beside_the_project` |
| 4 | DragonBones weighing counts every bone of every instance, once per IK solve; `measure_plan` keeps the box's sampling within `MEASURE_WORK` (no floor) | `bones_and_ik_are_weighed_per_instance`, `the_box_is_sampled_within_its_bound` |
| 5 | A carry that still fails names the queued save it dropped (`status.carry_failed_queued`) | (the message; carries now fail only on I/O) |
| 6 | `begin` closes the card properly (a copy in flight abandoned, its folder removed) | `a_new_import_abandons_a_copy_in_flight` |

## Part 6 — the integrated-tree review (2026-11-01)

- The heavy-frame cache no longer holds its lock while a frame is rasterised: look up →
  miss → unlock → draw → lock → insert (a duplicate render is harmless; the first kept is
  used). Frames are shared (`Arc`), and each size shown has its own budget
  (see the next item for the budget as it is now), so a video export at another size never
  evicts the screen's frames. Tests (`tests/import_cache.rs`): two threads whose draws must
  meet inside the rasteriser (each waits for the other to have started — impossible under
  the old lock) draw the same pixels as alone; sixty frames at an export's size leave the
  screen's frame cached.
- The manual page links the dancer, stage and video pages (EN/ES).
- Review of the fix rounds: one size per budget was evicted whole when a fourth size came
  (four are often live: a docked or floating dancer, the companion, the window's card, an
  export) and a resize made a new size at every step. Now every frame is its own entry,
  evicted by recency within one budget (`CACHE_BYTES` = 64 MB); sizes are quantised to an
  eighth of an octave (`scale_key`, the blit fits the frame to the size shown); a size drawn
  in the last second is protected — an export evicts its own frames before the screen's.
  Tests: `five_consumers_drawing_in_turn_each_keep_their_frames` (each frame rasterised
  once over four rounds of five sizes), `a_live_resize_reuses_frames` (16 sizes, at most
  2 rasterisations), `another_size_never_evicts_the_screens_frames`.
