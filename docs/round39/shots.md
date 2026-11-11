# Round 39 — `shots`: every interface picture in the new look

The owner's order: once round 39 has landed, every interface picture on the website and in
the README is reshot in the new look (Slate Night, the default, and Slate Paper, its light
twin). He flagged Delay's card (still in round 37's brown Cantus Night), but every picture was
shot in a retired theme. The work is in two phases: the units now (the leftover builders do
not touch the unit windows), everything else once the arrangement, mixer and rack land.

## Phase 1

### The units

- `shoot_skins` applied `theme::slate()` / `theme::paper()`, the retired FL-derived pair.
  It now applies `theme::slate_night()` / `theme::slate_paper()` (shots and `--gallery`).
- Reshot every instrument and effect, EN and ES, at 100 % and 200 %. Then
  `web/tools/units_pictures.py` made from them:
  - `web/assets/units/`: 204 files (34 units × dark, light, thumb × EN, ES), WebP q82 for the
    big pictures and q80 for the thumbs, as before;
  - `docs/images/units/`: 68 PNGs, plus 34 thumbs in `thumbs/` (JPEG q86).
- Every family is smaller than before: the thumbs are 0.84 → 0.75 MB, the big dark ones
  3.54 → 3.32 MB, and the README PNGs 9.58 → 9.25 MB.
- **Delay.** In phase 1 Delay had no window of its own, and its pictures were the
  inspector's view (`shoot_delay`, its crop fixed for round 39's taller insert tile). Since
  `r39/delay` (ADR 0115) Delay has its own window, and its pictures are `shoot_skins`' like
  every other unit's; `shoot_delay` now shoots the inspector's mini view and the whole app
  for that branch's own report, and `reshoot.sh` no longer runs it.
- The units whose own face is wood or a vintage material (Anulus, Phasis, Calor, Organum and
  others) are still brown. That brown is their face (ADR 0074; *Units wear their own faces* is
  on by default), not the theme. Their chrome and the plain units (Aequalis, Spatium, Unda…)
  show Slate.

### The logo

`web/assets/logo.svg`, `favicon.svg` and `press/cantus-logo.svg` are already logo #03,
byte-for-byte: `logos/small/03-48.svg`, `03-32.svg` and `logos/03.svg`. `favicon-32.png`,
`apple-touch-icon.png`, `press/cantus-logo-1024.png`, `og.png` and `og-es.png` were made from
it in `ebfe8553` / `8d6a3499`, and I checked each one by eye. README's line 2 shows
`web/assets/logo.svg`. Nothing to change.

### The dancer

Removed, since nothing in `web/src`, `web/assets/manual`, README.md or `docs/` (outside old
round reports) references them:
- `web/assets/dance/` (six files, 4 MB);
- `docs/images/dance-companion.png`, `dance-import.png`, `dance-stage.png`,
  `dancer-window.png`;
- `docs/images/video-export.png`, the dancer's video dialog (an orphan since the dancer left
  the README).

Also removed: `_headers`' `/dance/*` rule and `site.yml`'s exception for the folder. The
manual's dancer pages (`manual/*/dancer.html`, `stage.html`, `video.html`,
`import-characters.html`) use no picture from these folders.

### One script for every picture

```
CANTUS_SLOT=/f/projects/daw-r39/slot.sh bash tools/shots/reshoot.sh [units|rest|all] [--no-build]
```

- Builds the examples it runs (release only) and runs them headless into `$CANTUS_SHOTS`
  (`target/shots/reshoot` by default), then writes every picture in place.
- `units`: `shoot_skins` (EN, ES; Delay's window among them since `r39/delay`), then
  `units_pictures.py`, then
  `tools/shots/check_units.py --fix`. That script holds `units_data.rs`'s `big` / `thumb` to
  the pictures, so a window that changes size cannot go out with a wrong aspect.
- `rest`: `shoot_site`, `shoot_readme`, `shoot_showcase`, `shoot_editor`, `shoot_external`,
  `shoot … ara`, `shoot_session`, `shoot_sync`, `shoot_mpe`, `shoot_ai`, `shoot_mastering`,
  `shoot_stems`, `shoot_transcribe`. Then `tools/shots/place.py` holds the whole recipe:
  - which PNG, which crop, which file;
  - the encoding: lossless WebP for the program picture, then q82 at 960/1280; q82 for
    studio/ and ai/; q84 for external/surfaces; q88 for session/ and score/; PNG + oxipng
    for the README.
  The encodings were checked byte for byte against the committed files' sources.
- The picture examples now apply Slate Night / Slate Paper instead of the retired themes
  (Cantus Night, the FL Slate and Paper).

It ran once end to end (66 min to build on a loaded machine, 8 min to shoot). Its outputs
outside the units were **not** committed: they wait for phase 2.

### What phase 2 must fix (seen in the trial run)

- **Crops.** Round 39 moved things, so several fixed crops in `place.py` now cut the wrong
  area:
  - `midi-ports` (the Settings page);
  - `ai-settings`;
  - `sync-bar`;
  - `external` / `surfaces` (the external-gear page is now a single tall page, so the
    1500 × 412 / 376 crops no longer frame the two halves).
  Each needs new offsets measured on the merged tree. Measure them from the views'
  rectangles if the examples can print them.
- **The program picture** (`shoot_site`) shows the status bar's red *"Dropout: the engine
  took 19.6 ms of 16.7 ms"*. On a loaded machine the manual engine's headless blocks measure
  slow. Shoot on a quiet machine, or have the example clear the dropout line before the
  last frame.
- **No demo device.** `shoot_ai` (ai-agent), `shoot_stems` and `shoot_session` (window, midi)
  draw whole windows on `EngineHandle::dummy()`, with no audio setup. Check them for the
  "No audio" line and give them the demo interface that `shoot_readme` / `shoot_site` use.
  The midi-ports trial shot shows "Now: No audio — click to set up".
- `shoot_readme`'s alt text for `themes.png` still says "The twelve themes"; there are ten.
- `web/assets/studio/ara-menu.webp` is not referenced by `web/src` (README uses
  `ara-menu.png`). Kept, as the order says to keep every name.

## Phase 2

After the leftovers (arrangement, mixer, rack), the design sheets, the docs, Slate Lapis and
Delay's window were merged into `r39/shots`, everything outside the units was reshot from
the merged tree with one command:

```
CANTUS_SLOT=/f/projects/daw-r39/slot.sh bash tools/shots/reshoot.sh all
```

- **The program picture** (`web/assets/program/cantus*.webp`, EN and ES, lossless + 960/1280):
  Slate Night, the demo interface (ASIO4ALL v2) on its chip, *Autosaved* on the status line.
  `web/src/tour.rs`'s mixer crop now starts at the mixer's tab row (y 497, h 493): round 39
  put the desk's toolbar in that row, and y 515 cut its words in half.
- **The README** (`docs/images/*.png`, 34 pictures) and **the site's** `studio/` (26),
  `session/` (4), `score/` (3) and `ai/` (5): Slate Night / Slate Paper, as the README and the
  site name them.
- **The manual's pictures** (`web/assets/manual/img/` and `docs/manual/img/`, 14 each) were
  in round 25's Cantus Night too; they are in the script now (`place.py`'s `manual` family,
  palettised as round 22 made them) and reshot.

### What the reshoot needed

- **A working studio in every whole-window picture.** `examples/shots_common/studio.rs`
  (included by the picture examples): the audio setup's demo interface, and
  `views::transport::perf::hush_dropouts` (doc-hidden, for pictures) so a block a busy
  machine made late never puts *"Dropout: …"* on a picture's status line; it also clears the
  dummy engine's *"Audio device unavailable"* line. Held by `perf_status`'s
  `a_hushed_app_never_says_a_dropout`.
- **Crops measured on the layout.** The examples write the rects they are cut from beside
  their PNG (`<png>.json`): the Settings window (midi-ports, ai-settings), the Sync group with
  its adjacent neighbours (sync-bar; `shoot_sync`'s bar is 2560 px, as at 1920 the demo
  interface's chip folds Sync into More), the external-gear page's two bands
  (`external_bands::last_bands`), the mastering, stems and Convert to MIDI dialogs, the audio
  popover. `place.py` rewrites the sizes `web/src/{ai,session,studio}.rs` declare to match.
- **README:** ten themes and Slate Night / Slate Paper in the text and the alt texts (it said
  twelve and Cantus Night / Paper); the hero's song without its dancer
  (`demos::house_without_dancer`).

### Bugs the pictures showed, fixed

- **The mixer's menus had no sheet.** Since `des-controls` a list in a popup paints its rows
  only, and the mixer's popups (Snapshots, a strip's menu, the cable options) and
  Appearance's base-theme menu open in a bare area: their words floated over the
  arrangement. Both now draw in `kit::automate::frame()`, sized to hold its edge.
  `tests/mixer_menu_sheet.rs` (red without the change).
- **Separate stems hid *Accept*.** The kit's foot band was drawn inside the rows' scroll, in
  the middle of the dialog, with the model's card under it; once the card grew, the button
  was below the fold. The actions are now the dialog's foot, outside the scroll, the card
  above them (its note says "above"), and the dialog stays whole on a short screen.
  `tests/stems_dialog_foot.rs` (red without the change: *Accept* at y 1012 on a 720-line
  screen).

### Seen, not changed (for the owner or another round)

- **`Project::strip_is_free`** counts a bus strip that other strips feed (and no track plays)
  as free, so a new track can land on it: in the program's *house* demo the dancer, added
  last, takes the drum bus and renames it "Dancer" (Hats and Percussion show "→ Dancer").
  The README's song is built without the dancer; the core rule is not changed here.
- **The taskbar-icon colours** in `mark.png` are drawn on the round 25 mark (the radial
  ticks), beside logo #03 in its own colours: that is what the app's icon colours draw.
- The **brown units** are their own faces (ADR 0074), not the theme.
- `web/assets/studio/ara-menu.webp` is not referenced by the site (kept by name).

## Tests

**Phase 2.**
- New: `tests/mixer_menu_sheet.rs` and `tests/stems_dialog_foot.rs`, each red without its
  fix and green with it; `perf_status`'s `a_hushed_app_never_says_a_dropout`.
- The stems tests (`stems_feature`, `stems_review2`) and `locale_isolation` pass.
- `cargo test -p cantus-ui --no-fail-fast` on the merged tree: 2 627 passed, 5 failed,
  51 ignored. The 5 fail the same on `r39/integration`'s own sources (1e92c38b; checked by
  putting this branch's five changed source files back to integration's and rerunning):
  - `arrange_e2e::the_arrangements_structure_end_to_end`: the glyph's undo (line 383,
    "undone": the glyph is still "star");
  - `slate_window_golden` × 4 (arrangement and mixer, Night and Paper): 11–26 % of the
    pixels differ. The mixer now folds three strips into its "3 ›" chip where the golden
    folds two, and the strips sit further right. The goldens appear to predate the
    leftovers' mixer and arrangement.
- `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui --all-targets`: clean.
- **The site's budget** on the phase-2 pictures passes every line: WebAssembly 231 370 B
  gzipped; first visit 686 621 B (about 0.69 MB; it was 774 267 B: the new hero is lighter);
  LCP median 1.96 s (budget 2.0 s); accessibility 100.

**Phase 1.** No view code changed: the changes are examples, Python tools and pictures. All 15 changed
examples build in release with no warnings, and `RUSTFLAGS="-D warnings" cargo clippy -p
cantus-ui --all-targets` is clean. `cargo test -p cantus-ui --no-fail-fast`: 2 595 passed, 0 failed,
54 ignored (`stated_preset_counts` reads `units_data.rs`).

**The site's budget** (`tools/web-budget.ps1`, the same checks as `site-budget.yml`, run on
this branch with the new unit pictures) passes every line:
- WebAssembly: 231 727 B gzipped (budget 250 000).
- Lighthouse mobile, three runs:
  - first visit 774 267 B in each run (budget 1 MB; about 0.77 MB);
  - LCP median 1.97 s (budget 2.0 s). The LCP is the hero's program picture, which this phase
    did not change, so the margin is thin;
  - accessibility 100.

## Pictures for the owner

Phase 2 (the merged tree):
- `web/assets/program/cantus.webp` and `cantus-es.webp` (the site's hero, EN and ES)
- `docs/images/window-night.png` (the README's hero)
- `web/assets/units/delay-dark.webp` (Delay's own window, since `r39/delay`)
- `web/assets/studio/mix-snap-night-en.webp` (the Snapshots menu on its sheet)
- `docs/images/stems-panel.png` (Separate stems with its foot)
- `web/assets/session/grid.webp`
- `web/assets/manual/img/startup.png` (the manual)

Phase 1:

- `web/assets/units/delay-dark.webp`, `delay-light-es.webp` (the flagged one; since `r39/delay` its window)
- `web/assets/units/aequalis-dark.webp`
- `web/assets/units/spatium-light.webp`
- `web/assets/units/chorda-dark.webp`
- `web/assets/units/unda-light.webp`
- `web/assets/units/pulsus-dark-es.webp`
- `docs/images/units/tonus-dark.png` (README)
