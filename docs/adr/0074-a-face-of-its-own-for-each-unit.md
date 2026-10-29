# ADR 0074 — A face of its own for each unit: the skin system

**Status:** accepted (round 32) · **Date:** 2026-10-28 · **Extends:** ADR 0024 (the hand-drawn
kit), ADR 0020 (the exported plug-ins' editors), ADR 0046 (themes) · **BACKLOG:** §6h point 1

## Context

The owner's direction for round 32: each of our effects and instruments with a face of its
own — FL's Love Philter, Gross Beat, Vocodex and Harmor; FabFilter's family; Arturia's and
u-he's instruments as the reference — its colour and material, its characteristic display,
its own control shapes and typography accents, in the kit's vector primitives (no bitmaps;
crisp at any scale, both themes), still the family's rules. Until now every unit wore the
kit's one look (ADR 0024): a theme changed every unit together, and Aequalis and Culmen could
only be told apart by their hue and their layout.

What had to hold: label/value units, nothing overlapping, resizable 75–200 %, keyboard focus,
AccessKit and the parameter menu on every control, unchanged; the theme's contrast promises in
every preset; the frame budget; the exported CLAP/VST3 windows the same as the DAW's; a user
who does not want the faces can have the plain look back.

## Decision

1. **A skin is data** (`kit::skin::Skin`): the surface ladder's `Tint`, an `accent` and an
   `accent2` (a `Pair`: one `Hsl` per theme mode), and seven choices — `Material`, `Display`,
   `KnobShape`, `SwitchShape`, `Lettering`, `Plate`, `Ornaments`. One file per family under
   `kit/skin/` holds its units' sheets (`skin::of(Unit)`); a family's builder owns its file
   only. A shape the enums lack is a unit's own painter (`KnobShape::Custom`,
   `SwitchShape::Custom`) in its sheet file, or a new variant the kit draws.
2. **The kit draws every variant**, in vectors at the style's `k`, through the lighting
   recipe, in both themes:
   - **Materials** (`skin::materials::finish`) on the device ground, the cards (quieter, so
     the controls stay the eye's first stop) and the header: matte, brushed, anodised,
     lacquer, wood, vinyl amp covering (`Tolex` in the code), glass, paper, carbon. Light and shade only (the theme's `light`
     and `dark` tokens, by its depth): the colour is the tint's. Fine textures are tiles
     computed procedurally once at the screen's own pixels (`skin/textures.rs`) and laid as
     quads — one texel per pixel at every scale, seamless (a test).
   - **Displays** (`skin::display_ink`, `display_ground`, `displays::{trace, graticule,
     cover, needle_meter}`): one way to light a display — plain (the theme's ink well),
     phosphor (the accent as the phosphor, P31 green without one), LCD, VFD, glass over the
     face, ink on chart paper, a scope's graticule; a VU needle meter for the dynamics. The
     effects' `canvas::Ink::of` reads `display_ink`, so every display of a unit takes the
     look without code of its own; `canvas::edge_b` paints the look's cover (its glass).
     Words on a display are pushed to 4.5:1 on its ground, dim words to 3:1.
   - **Knobs** (`skin::knobs`): a value ring on a flat disc (FabFilter's), a fluted skirt
     under a cap, a chicken head over a printed scale, soft rubber with a dot, a machined cap
     with knurling, a graduated dial (long even marks on L and XL; no numbers, which would not be the parameter's units). The modulation rings,
     the value arc from the default, the default's notch, the live modulation dot and the
     focus ring stay on every shape. Static bodies are rasterised once per size, colours and
     shape (the cache key holds all three, so no unit's colours show in another's window).
   - **Switches** (`skin::switches`): bat toggle, rocker, sliding pill in the lamp's place
     of an LED button and a card's power lamp; the lamp shape lights the whole button and the
     segmented control's chosen segment.
   - **Lettering** (`text::Face`, `design::role::Role::lettered`): the nameplate, card titles
     and section bands in another weight or width of the bundled IBM Plex faces, a case and
     extra tracking; readouts and cell values in the plain mono face (`mono_values`).
     `Lettering::PLAIN` is the kit's own setting; any other is taken literally.
   - **Plates** (`skin::plates`): engraved letters, a raised badge in the accent, a painted
     stripe across the header's foot, a screen-printed legend box. **Ornaments**: screws,
     vents, rack ears in the ground's padding (no card ever covers them); printed rules after
     card and section titles.
3. **The skin is worn while a unit's window paints** (`skin::wear`). The shell
   (`kit::shell::Shell::show`) wears its skin around the header, the body and the footer;
   `Style::current()` returns the installed tokens in the worn skin. So every widget and
   helper inside a body that asks for "the current style" is in the unit's face — an editor
   cannot lose its skin by building its own style. `Style::plain()` is the plain look on
   purpose (the shell's base). Wired through `fx::frame::shell_of` (so the exported windows
   wear it), the instruments' frame and Pulsus.
4. **Colours keep the theme's promises.** `Skin::tokens` moves the surface ladder toward the
   tint's hue **keeping each surface's relative luminance** (a yellow at a grey's HSL
   lightness is far brighter than the grey), lays the accent over every function hue but
   modulation's (ADR 0057's colour stays the cue for what moves — modulation marks, drop
   targets, the knob's live dot, Latch — turned away from the accent, on its own side of the
   wheel, when the two are within `MOD_GAP` = 50°), then runs the theme's contrast floor
   again (`Tokens::floor`) and a **material's floor** (`materials::floor`): text, dim text,
   labels and the function hues reach 4.5:1 on the darkest and the lightest colour the
   material makes of a card and of the header (`materials::extremes`, from each recipe's
   peaks) as well as on the plain surfaces. A style's hue comes from these floored tokens
   (`Style::skinned`), so an arc, a lamp or a badge paints the hue the header and the worn
   style paint. In a face with an accent, every function's section hue is the accent
   (`Style::function`, `Style::function_hue`), a modulation unit's included. Only the
   modulation cues read `t.hue_mod`. `skin::contrast_failures(skin)` lists where a sheet breaks either floor in any
   of the twelve presets; each family's sheet test calls it.
5. **The user's switch**: *Settings ▸ Appearance ▸ Units wear their own faces* — *On* (the
   default) or *Off* (Spanish: *Cada unidad con su propia cara* — *Sí* / *No*) — kept in
   `settings.json` (`unit_faces`), applied in `CantusApp::assemble` (the boot path) and when
   changed (`apply_unit_faces`); off, `skin::of` returns `PLAIN` for every unit. Per thread
   (the thread that draws), so tests never touch each other. The picker's thumbnails are
   keyed by the switch and a digest of every sheet (`skin::sheets_digest`), so a picture made
   plain — or before a sheet changed — is never shown for a face.
6. **What keeps the plain look**: hosted plug-ins' Cantus view (it is not one of our units),
   the legacy delay and reverb, everything outside a unit's window, and every unit when the
   switch is off. **The exported CLAP/VST3 plug-ins** have no settings file: they always wear
   their unit's face (their window's set-up, `cantus_plugins::editor::prepare`, sets it on,
   as its thread may be the host's).
7. **Inside a display** a family draws with `skin::display_style(s)` (the instruments'
   `face::on_display` delegates to it, adding only that words also read on Gravis's mixed
   body fill; the dynamics' `dynfx::on_display` is not a style but a plate's colour, one
   step from the ground away from the words — the one exception). Its fills are shades
   of the display's ground and its `dark` is the display's, so a control on a CRT in Paper
   or on an LCD in Slate keeps the floor. A display's two voices come from
   `canvas::Ink::voice`; its grid colours are opaque, as the theme's are.
8. **Inspect mode** (the theme's eyedropper) registers, for every element, the skin it was
   painted in (`inspect::Item::skin`): a skin's colours are its sheet's data, named by the
   unit's key, beside the tokens.

## Consequences

- A unit's face is a reviewed sheet of data, not painting code; the families add their units'
  faces in parallel (`kit/skin/{dynamics,modulation,tone,instruments}.rs`).
- The kit's gallery grows by thirty sheets (every material, display, knob and switch shape,
  plate, and the ornaments), with goldens in Slate and Paper at 1×, 1.5× and 2×.
- `examples/shoot_skins.rs` shoots every unit in Slate and Paper at 100 % and 200 % (and with
  the switch off, and the kit's sheets, and four sample sheets to try on any editor).
- Wood, vinyl amp covering and carbon cost a few hundred textured quads a window (tiles are laid from
  256-texel patches in the layer atlas; wood's grain and fibre are one tile); a display's
  glow is one strip of six rings round its trace. The three-editor budget test measures it
  (round 32's numbers are in `docs/round32/skin.md`).
- `every_unit_has_a_face_of_its_own` is turned on by the integrator when the last family has
  landed.

## What the tests hold

- `tests/skins.rs`: keys unique; the plain skin changes nothing; off, every unit is plain;
  one cached knob body per face (`cached_bodies_are_one_per_face`).
- `tests/skins_body.rs`: every unit's body, worn by a probe sheet over sentinel tokens, has no
  vertex painted from the plain tokens and every part registered under the probe — with a
  control (faces off) that must be caught.
- `tests/skins_boot.rs`: the switch is on by default, applied by `assemble` from the settings
  and when it changes.
- `tests/skins_budget.rs`: Culmen, Spatium and Unda in one frame, each in a face of its own
  (their sheets, and three distinct rich sheets), docked at 100 % and floating at 200 %, at 1
  and 2 physical pixels per point; the timing in release (best of three runs of 200 frames,
  p95 against the 4 ms budget), with no atlas reset and no skin's tokens, worn style or
  display colours made again in steady state.
- `cantus-plugins/tests/editor.rs::every_exported_editor_wears_its_units_face`: every
  exported editor, in English and Spanish, paints under its unit's key, and a sheet's tint
  reaches its pixels — after its window's own set-up (`editor::prepare`) on a thread where
  faces were turned off and another sheet was worn.
- `kit::skin::tests`: every tint of hue, saturation and amount, with and without an accent,
  keeps the contrast floor in all twelve presets; a style's hue is the floored accent the
  worn style paints (a pale yellow accent, 4.5:1 on the deck); modulation keeps its own
  colour, `MOD_GAP` from any accent; every material, alone and tinted, keeps text, dim text,
  labels and hues at 4.5:1 over its darkest and lightest; the worn skin comes off.
- `views::picker_thumbs::tests`: a picture's look changes with the switch and with every
  sheet.
- `kit/skin/textures.rs` tests: every tile repeats without a seam.
- `tests/kit_tokens.rs` and `tests/kit_gallery.rs` over the new sheets.
