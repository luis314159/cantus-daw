# Round 39 — builder `kit`: the kit becomes Slate

**Branch:** `r39/kit` (worktree `F:\projects\daw-r39\kit`), on master beta.13 plus the
round's contract, with `r39/integration` (ux-a) merged in. **ADRs:** 0108 (how the kit draws,
accepted) and 0111 (the design system, a draft for the integrator).

## What changed

### The design system: Slate, and two families beside it (ADR 0111)

- **Six themes**, each written token by token from its design's system: Slate Night (the
  default) and Slate Paper (`dir-a`), Graphite Night and Paper (`dir-b`), Studio Ink Night and
  Paper (`dir-c`) — `src/theme_slate.rs`, `theme_graphite.rs`, `theme_studio_ink.rs`. The
  high-contrast pair stays (low vision). Where a design's colour fell short of a kit promise
  it moved the least that holds it, and the file says so (Paper's record red, Paper's meters
  in a white groove, the function hues no design drew).
- **The ten older themes are retired**; the owner accepted it for now (BACKLOG §6j). A
  setting naming one shows Slate in that theme's own light (`appearance::RETIRED`,
  `Appearance::retire`); a slot's own changes stay; users' own themes are untouched.
- **A theme carries a system** (`kit::system`): its face and how its keys are built, in the
  theme file (`system`, absent = Classic) and in the tokens every painter gets.
- **The faces** (`kit::faces`): Barlow Semi Condensed for Slate, Sofia Sans in three widths for
  Graphite, IBM Plex Sans Condensed + Mono with an ExtraLight clock for Studio ink; each sets
  the ten text roles its own way (Slate: clock ExtraLight 30, names 12, titles Medium 13,
  section titles SemiBold 11 in title case, tags SemiBold 9). Family *names* are stable and a
  face re-points them, so every view follows a theme switch; a switch reinstalls the fonts.
  IBM Plex stays registered for the units' lettering and the score.
- **Fonts bundled** with their OFL notices (`LICENSE-OFL-Barlow.txt`,
  `LICENSE-OFL-Sofia-Sans.txt`, rows in `xtask/notices/head.md`, the README's licence line).
  `tools/fonts/build_faces.py` makes them from the published files: **tabular figures by
  default** (egui applies no OpenType feature), unhinted, Sofia as static instances.
  **Sizes:** Barlow 385 KB (5 files), Sofia 977 KB (6), Plex Condensed ExtraLight + Light
  226 KB — **1.59 MB** added to the program.
- **Keys and wells** (`cap::well_key`, `cap::well_body`, `well.rs`): a recessed family's key
  is a flat well, a 1-px dark edge, a 1-px light lip; hover lifts it, pressed darkens it, on
  tints it with the accent (green for play, red for record); a radio choice is a lifted
  neutral pill. Fields and readouts are the same well, no glass.
- **The icon family's second pass** (`kit::icons`, data in `kit::icons_data`, generated from
  `docs/round39/design/icons/glyphs-n-*.js`): transport, tools and window keys drawn for the
  key's height on the device (16/20/24/30/36), rasterised once per inks into the layer atlas.
- **Track colours** a tint at rest through the veil (each family's lane colour at 59 %).
- **Idle without "−∞"**: a recessed family's strip peak readout and the top bar's meter show
  no number in silence.
- **The splash** and the site's shot scene use Slate Night.

### The interface's size follows the screen (ADR 0111 §7)

`screen_scale`: 1.25 × (physical width ÷ 1 920), 100–200 %, ours the nearest zoom step; until
the user picks one. A settings file written before keeps its scale
(`Settings::ui_scale_follows_screen`, false when absent).

### Smaller fixes on the way

- The top bar re-measures its groups when the face changes (`WantsKey::face`) and sets its
  menus in title case in a recessed family (`transport_menus::menu_caps`).

## View files touched outside the kit (small, listed as the brief asks)

- `views/mixer_strip.rs` — the peak readout writes nothing in silence in a recessed family.
- `views/transport_meter.rs` — the same for the top bar's meter readout.
- `views/transport.rs` — the face in the groups' measuring key.
- `views/transport_menus.rs` — title-case menus in a recessed family.
- `views/appearance_file.rs` (ux-b's) — two test expectations: a theme.json equal to a
  retired preset now opens in Slate.
- `app.rs` — two lines: the scale follows the screen each frame while it should; a user's
  scale stops it. `boot.rs` — the splash in Slate Night and its face.

## How it is tested

- `theme::tests` — the default is Slate Night with the design's values; every shipped theme
  meets the kit's promises and round 25's floor; a recessed key's glyph 4.5:1 at rest and
  under the pointer, 3:1 lit; the well shows against its bar.
- `kit::appearance::tests::every_retired_theme_maps_to_slate_in_its_own_light_keeping_the_slot_s_changes`.
- `kit::system::tests`, `kit::faces::tests` (every role a known family; the new faces'
  figures are tabular, read from the files), `kit::icons::tests` (every family glyph at every
  size parses, all inks named, inked), `screen_scale::tests`.
- `tests/slate_e2e.rs` — boots through `CantusApp::assemble` on a fresh data folder in Slate
  (tokens, system, face, tabular figures), a beta.13 appearance file naming retired themes
  opens in Slate, every theme switches through `apply_appearance` and back exactly.
- `tests/slate_budget.rs` — the whole 1080p window playing: Slate, Graphite and Studio ink
  against beta.13's Cantus Night — no more draw calls, no more triangles, allocations within
  8; timed in release with `--ignored` (ADR 0108's numbers).
- The kit lint holds (no literal colours outside the colour machinery; `FontId::new` only in
  `design.rs`).

## Pictures

`docs/round39/kit/window-song-<theme>-1280x720.png` (100 %) and
`window-song125-<theme>-1920x1080.png` (125 %) — the demo song, every shipped theme; plus
`window-slate_night-*` / `window-slate_paper-*` (the welcome project, zooms, Spanish, the
Appearance page).

## Verified only in emulation

Everything here is drawn by the CPU renderer (`raster.rs`); the screen-following scale was
tested with a synthetic monitor size — the real monitor's reported size (and Windows' own
scaling) needs the owner's machine: `docs/beta-checklist.md`.

## What is left (in this order, each from its approved sheet)

1. dir-a's top bar (44 high, one row, two clusters, the fold order with compact forms, the
   Spanish menus measured), the overflow panel built from what folds, the toolbar, the mark,
   and the dialogs (Settings ▸ Audio and Look, first run, export and its progress).
2. The mixer and inspector (des-mixer) with icon set 3.
3. The control families and the driven signs (des-controls) through `kit::automate::signs`.
4. The piano roll, with the approved keyboard (des-views).
5. The channel rack, the browser with icon set 4, the clips' content.
6. Icon set 5 (the rest of `glyph.rs`), the bulb retired for enable and the insert lamp.
