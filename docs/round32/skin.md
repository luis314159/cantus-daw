# Round 32 — skin: the kit's skin painters (first delivery), the tone/space units (second)

Builder: **skin** · branch `r32/skin` · ADR 0074. The tone/space units (Aequalis, Fervor,
Spatium, Latitudo, Speculum, Tonus) are the second delivery.

## What was done

- **Every variant drawn** by the kit, in vectors at the style's `k`, both themes:
  - `Material` (10) — `kit/skin/materials.rs`: the device ground, the cards (quieter) and the
    header; fine textures are seamless tiles computed at the screen's own pixels
    (`kit/skin/textures.rs`: brushed, vinyl amp covering, carbon, paper, wood, scan lines, VFD mesh).
  - `Display` (7) — `kit/skin/displays.rs`: `skin::display_ink`, `skin::display_ground`,
    `displays::{trace, graticule, cover, needle_meter, vu_position}`. The effects'
    `canvas::Ink::of` reads `display_ink` and `canvas::{ground, ground_b, edge, edge_b}` draw
    the look, so every effect display takes it with no code of its own; `canvas::trace` gives
    a trace the look's glow. Added: a VU **needle meter** painter for the dynamics family.
  - `KnobShape` (7 + `Custom`) — `kit/skin/knobs.rs`; `knob.rs` now exposes `Geometry`,
    `paint_rings`, `paint_track`, `paint_live`, `value_hue`, `angle` for a unit's own painter.
    `Custom` paints inside the knob's square; the kit keeps the modulation rings and the focus
    ring on every shape.
  - `SwitchShape` (5 + `Custom`) — `kit/skin/switches.rs`: in the lamp's place of
    `led::button` and of a card's/section's power lamp (`band::lamp`); the lamp shape lights
    the whole button and the segmented control's chosen segment. `Custom` is called in the
    lamp's place (the cap and the word stay the kit's).
  - `Lettering` — `text::Face` (+ `text::{paint_face, measure_face, fit_face}`,
    `design::role::Role::{lettered, mono_plain}`): the nameplate, card titles, section bands;
    `mono_values` for readouts and cells. `Lettering::PLAIN` is the kit's own look; any other
    value is literal.
  - `Plate` (5) and `Ornaments` — `kit/skin/plates.rs`: engraved, badge, stripe, legend;
    screws, vents, rack ears in the ground's padding; printed rules after card/section
    titles.
- **The worn skin** (`skin::wear`, `skin::worn`): the shell wears its skin while it paints;
  `Style::current()` returns the worn style, `Style::plain()` the plain one. This is how
  editors that build their own `Style::current()` inside the body keep the face — every one
  of them, without touching their files. The fx frame's popups wear it too.
- **Displays' voices** (the modulation builder's findings): a VFD's light is the accent, its
  second voice the second accent; every look's main and second voices pushed to 3:1 on its
  ground, never the same colour.
- **Contrast**: `Skin::tokens` tints keeping each surface's luminance, then runs
  `Tokens::floor` (made `pub(crate)`, the integrator's request) — cached per skin and theme.
  `skin::contrast_failures(skin)` for the families' sheet tests.
- **Caches**: knob-shape bodies keyed by shape, size and colours; tiles by kind only (they
  are light and shade, tinted where drawn); displays' inks cached per look and colours.
- **Inspect**: `inspect::Item::skin` (the unit's key), shown on the pick card.
- **The switch**: `Settings::unit_faces` (default on), *Settings ▸ Appearance ▸ Units wear
  their own faces* (Spanish: *Cada unidad con su propia cara*, the dynamics builder's wording;
  a `.short` form for a narrow column), On/Off,
  `CantusApp::apply_unit_faces` called in `assemble` and on change; strings in
  `locales/areas/skin.{en,es}.txt` (the skin builder's block).
- **Exported plug-ins**: wear their face always (`cantus-plugins/src/editor.rs` sets it on at
  the window's build).
- **`examples/shoot_skins.rs`**: `<unit>-<theme>-<scale>.png` for every unit, a unit filter,
  `--plain`, `--gallery` (the kit's skin sheets), `--quick`, and `--try
  studio|vintage|machine|lab` to try a sample sheet on any editor.
- **Gallery**: 30 new sheets (`kit/gallery_skin.rs`), goldens blessed (180 PNGs).
- ADR 0074; manual EN/ES (*Each unit's face*, the switch in Settings).

## How it was tested

Windows, `RUSTFLAGS=-D warnings`: `cargo clippy --workspace --all-targets` clean; `cargo test
-p cantus-ui` — 173 binaries, 1 871 passed, 0 failed (31 ignored: timings and the integrator's
`every_unit_has_a_face_of_its_own`); `cargo test -p cantus-plugins` — 76 passed.
Linux (WSL, `tools/wsl/check.sh r32/skin clippy`, own clone `~/ci-r32-skin`): clippy clean,
the plug-ins without the editor clean. `check.sh … test` did not finish: drive C: (which
holds the WSL disk image) ran out of space during the workspace build and WSL stopped; no
result was written. To run once C: has room (the integrator's Linux run covers it).

New tests:

- `tests/skins_body.rs::every_units_body_paints_in_its_face_never_with_the_plain_tokens` —
  all 32 units (23 effects, 8 instruments, Pulsus) under a probe sheet over sentinel tokens:
  no vertex of a body from the plain tokens, every part registered under the unit's key; a
  control run (faces off) must be caught, and is.
- `tests/skins_boot.rs` — the switch on by default, applied by `assemble`, kept, applied on
  change.
- `tests/skins.rs::cached_bodies_are_one_per_face`.
- `tests/skins_budget.rs` — three editors in one frame, each body in its face.
- `cantus-plugins/tests/editor.rs::every_exported_editor_wears_its_units_face` (EN and ES).
- `kit::skin::tests::a_tinted_skin_keeps_the_kit_s_contrast_floor_in_every_preset` (12 hues ×
  5 saturations/amounts × with/without accent × 12 presets), `the_worn_skin_…`.
- `kit::skin::displays::tests::every_look_keeps_its_voices_readable_and_apart_in_every_preset`
  (7 looks × 4 accent cases × 12 presets: text 4.5, dim, trace and second voice 3:1, two
  distinct voices) — the modulation builder's two findings.
- `kit/skin/textures.rs`: every tile repeats without a seam; `knobs.rs`: shape names unique.
- `kit_tokens` and `kit_gallery` over the 30 new sheets (180 goldens blessed; the kit's own
  goldens unchanged).

**Frame budget, three editors** (Culmen + Spatium + Unda, `cargo test --release -p cantus-ui
--test skins_budget -- --ignored --nocapture`, this machine, build + tessellate over 200
frames):

| Faces | Scale | p50 | p95 |
|---|---|---|---|
| their sheets (plain today) | 100 %, docked | 2.32 ms | 3.18 ms |
| richest sheet (wood, phosphor, skirted, every ornament) | 100 %, docked | 2.34 ms | 3.08 ms |
| their sheets | 200 %, floating | 2.48 ms | 3.27 ms |
| richest sheet | 200 %, floating | 3.96 ms | 5.99 ms |

(The window budget is 4 ms; asserted at 8 ms, as the mixer's timings are.)

Looked at: every gallery sheet in Slate and Paper; Unda, Comprimo and Spatium in the four
sample sheets (`docs/round32/skins/kit/`). Redrawn after looking: the chicken head's outline,
vinyl amp covering (finer, quieter), carbon on light themes, card textures quieter than the ground, the
paper display's fibre (a printed grid instead), vents that fit narrow bodies, a lamp button's
word centred and its glow on light themes.

## Fix round (the integrator's review of the first delivery)

Each with a test that fails before its fix:

1. **A skinned style's hue is the floored accent** (`Style::skinned` takes it from the floored
   tokens; a style asking for modulation's hue keeps it). Test:
   `kit::skin::tests::a_skinned_style_s_hue_is_the_floored_accent_the_worn_style_paints` (a
   pale yellow accent, Hsl 50/.9/.75: 4.5:1 on the deck in all 12 presets, equal to the worn
   style's hue).
2. **Thumbnails keyed by the faces**: `look_key` hashes `skin::enabled()` and
   `skin::sheets_digest()` (every unit's sheet as worn, cached per switch/override); `REV` 2.
   Test: `views::picker_thumbs::tests::a_picture_s_look_changes_with_the_switch_and_with_every_sheet`.
3. **Modulation keeps ADR 0057's colour**: the accent no longer lands on `hue_mod`; when the
   theme's modulation hue is within `MOD_GAP` (50°) of the accent it is turned away on its own
   side. Test: `kit::skin::tests::modulation_keeps_its_own_colour_apart_from_any_accent`
   (36 accent hues × 12 presets).
4. **A material's floor**: `materials::extremes` (the darkest and lightest a recipe makes of a
   card and of the header, from each recipe's peaks and each tile's measured reach,
   `textures::reach`) and `materials::floor` (text, dim text and labels — and the hues on
   cards — lifted to 4.5:1 over them, held on their usual surfaces too, only when the material
   breaks the floor); `skin::material_failures`, and `contrast_failures` checks it. Test:
   `kit::skin::tests::every_material_keeps_text_and_hues_legible_on_its_darkest_and_lightest`
   (10 materials × alone/tinted with a pale accent × 12 presets). The token caches are keyed
   by a sheet's colours (`Skin::colour_key`), not only its key.
5. **The budget measured properly** (`tests/skins_budget.rs`): three *distinct* rich faces
   (wood/phosphor/skirted, vinyl amp covering/VFD/chicken-head, carbon/scope/dial), at 1 and 2 px/pt,
   docked 100 % and floating 200 %; best of three runs of 200 frames; asserts p95 ≤ 4 ms, no
   atlas reset and no skin tokens, worn style or display colours made again in steady state.
   Optimised: a display's glow is one six-ring strip (`Brush::glow_polyline`) instead of
   three polylines; tiles are laid from 256-texel patches and wood's two tiles are one
   (`Tile::Varnished`) — the material's vertices fell from 18 k to 7 k at 200 %/2 px/pt; the
   display inks and the worn style keep a few entries (three editors no longer remake each
   other's every frame).
6. **ADR 0074 §5** names the switch as the code and manuals do (*Units wear their own faces*,
   On/Off; *Cada unidad con su propia cara*, Sí/No).
7. **The plug-in window's set-up is testable**: `cantus_plugins::editor::prepare(ctx)` (what
   `App::build` runs); `every_exported_editor_wears_its_units_face` now turns faces off and
   wears another sheet first, then calls it — deleting its two lines fails the test.

Also: **`Glyph::Score`** (a five-line staff with a note) for the score pane.

**Frame budget after the fix round** (release, best of 3 × 200 frames, this machine with
other builders compiling):

| Faces | Scale | px/pt | p50 | p95 | vertices |
|---|---|---|---|---|---|
| their sheets | 100 %, docked | 1 | 1.58 ms | 2.17 ms | 74 767 |
| three rich sheets | 100 %, docked | 1 | 1.85 ms | 2.65 ms | 86 633 |
| three rich sheets | 100 %, docked | 2 | 2.02 ms | 2.85 ms | 116 552 |
| their sheets | 200 %, floating | 1 | 1.73 ms | 2.24 ms | 101 412 |
| three rich sheets | 200 %, floating | 1 | 2.01 ms | 2.68 ms | 116 552 |
| three rich sheets | 200 %, floating | 2 | 2.51 ms | 3.68 ms | 169 126 |

No atlas reset, no layer rasterised, no skin colours remade in steady state.
(Under heavier load from four builds the same cases measured up to p95 4.9 ms before the
optimisations: the p95 is sensitive to the machine; the integrator's idle run is the
reference.)

At the end of delivery 2, with every family merged (Culmen, Spatium and Unda each in its
own face), measured the same way:

| Faces | Scale | px/pt | p50 | p95 | vertices |
|---|---|---|---|---|---|
| their sheets | 100 %, docked | 1 | 1.77 ms | 2.43 ms | 88 097 |
| three rich sheets | 100 %, docked | 1 | 1.66 ms | 2.08 ms | 86 649 |
| three rich sheets | 100 %, docked | 2 | 1.90 ms | 2.64 ms | 116 568 |
| their sheets | 200 %, floating | 1 | 1.93 ms | 2.80 ms | 122 121 |
| three rich sheets | 200 %, floating | 1 | 1.92 ms | 2.62 ms | 116 568 |
| three rich sheets | 200 %, floating | 2 | 2.55 ms | 3.81 ms | 169 142 |

## Second delivery — the tone, space and analysis faces (`kit/skin/tone.rs`)

Design notes (*inspired by*, never a product's trade dress; no two siblings share a hue
within 20° and a material or a display — `tone::tests`):

- **Aequalis** (parametric EQ) — a precision instrument, after FabFilter-class EQs and
  console EQ modules. Blue anodised aluminium (tint 205°), accent cyan-blue (196°/199°),
  second voice amber for the analyser. The curve behind **smoked glass** (`Display::Glass`).
  Small **capped** knobs, pills, an **engraved** name, printed rules, mono readouts.
- **Fervor** (saturation) — a valve amplifier, after tube preamps and guitar amps (FL's
  Blood Overdrive's warmth). Warm **vinyl amp covering** (16°), accent amber-orange (26°/18°), second voice
  filament yellow. The transfer curve and harmonics glow in an **amber VFD**.
  **Chicken-head** knobs, **bat** toggles, a raised **badge**, screws and vents, uppercase
  condensed lettering.
- **Spatium** (reverb, its room) — a hall at night, after classic digital reverbs' remote
  panels. Deep blue **lacquer** (240°), accent indigo (238°/242°; 26° from Culmen's
  lacquer sapphire), second voice cold cyan
  for the early reflections. Decay curve and tail in a **blue phosphor** glow. **Soft** knobs,
  **lamp** buttons, a **stripe** under the name.
- **Latitudo** (stereo) — a broadcast goniometer, after RTW/TC-style phase scopes. **Brushed**
  steel in a rack (160°), accent green (150°/154°) for the mid, orange for the side. The field
  and the tone on a **scope's graticule**. **Dial** knobs, **rockers**, a printed **legend**,
  screws and **rack ears**, uppercase lettering, mono readouts.
- **Speculum** (analyser) — a bench instrument, after lab spectrum analysers. Grey-green
  **matte** enamel (95°), accent olive (78°/84°), held peak red. Spectrum, scope, vector and
  loudness on a backlit **LCD** (dark segments on olive glass in both themes). **Value rings**,
  pills, condensed uppercase mono lettering, a legend, printed rules.
- **Tonus** (auto-tune) — a vocal booth's glass, after pitch editors' note lanes. Magenta
  (326°/330°) behind **smoked glass** (318° tint), second voice teal. The pitch drawn on
  **chart paper**. **Soft** knobs, **lamps**, a stripe.

Display work, beyond the look the kit gives every display:

- `canvas::Ink::voice(plain, second)`: a display's two voices — a card's hue in the plain
  look, the look's main trace or second voice in a face. Latitudo's mid and side tone lines
  use it; they were the cards' hues, dark on the scope in Paper.
- A look's `grid`, `grid_strong` and `faint` are opaque, as the theme's tokens are. Tonus's
  black-key lanes thin the grid with an alpha, and on chart paper that was a heavy blue band.
- Fervor's numbers under the curve sit on the card in the card's text. They were in the
  display's text, which is pale on a light face.

The integrator's review of the instruments, applied:

- **`kit::skin::display_style(s)`**: the style for drawing *inside* a display. Every fill is a
  shade of the display's ground: cards, title bands, panels, the toolbar, value pills,
  wells, knob bodies and rims. Its `dark` is the display's, its words are the display's and
  its hue is the main trace (modulation's colour kept apart). The theme's sheen and glows
  are off, and the contrast floor is run over all of it, cached per look and theme. In the
  plain look it returns the style unchanged. Test:
  `kit::skin::displays::tests::a_display_s_style_keeps_every_promise_in_every_look_and_preset`
  (7 looks × with/without a pale accent × 12 presets: every pairing the kit promises, plus
  text, labels and the hue on cards, text on value pills and on the ground).
  Used by Spatium's two heads (*Decay curve / Post EQ* and its switches; *Envelope /
  Spectrum*) and Latitudo's slope switch.
- The tone sheets' tests use `contrast_failures`, which checks the sheet's tokens. No test
  of mine checks an accent against `t.ink`; the display tests use the look's ground.
- *Off* is as before: everything new is behind the face (the material, display, plate and
  ornament painters do nothing in `PLAIN`; `display_style` and `Ink::voice` return the plain
  style and colours).
- Title case is consistent: Aequalis, Spatium and Tonus (`Case::AsIs`) have sentence-case
  card titles throughout; Fervor, Latitudo and Speculum are uppercase. The kit's LED
  buttons and micro labels are caps in every unit, as before.

- **A unit's device hue is its accent, whatever its function** (the integrator's note on the
  modulation units). The fix is in the kit, not per family. `Style::function(f)` and the new
  `Style::function_hue(f)` give the accent in a face with an accent, for every function,
  and the shell's header uses `function_hue`. `Style::skinned` gives the accent for any
  function hue. The modulation *cues* keep ADR 0057's colour, apart from the accent: the
  knob's live dot, `automate::mod_mark`, drop targets and Latch read `t.hue_mod`; the
  modulation rings and the source chips go through `skin::cue_hue` (the instruments'
  rings and chips only since the review of delivery 2, below). Plain, nothing changes. Test:
  `kit::skin::tests::a_modulation_unit_wears_its_accent_and_its_cues_keep_modulation_s_colour`.
  After merging integration, the modulation builder's `face_function` is removed:
  `views/fx/frame.rs` asks for `hue_of(kind)` again, and `modfx_face.rs`'s tests do too.
  `every_face_s_header_and_body_paint_in_its_accent` stays green. `modfx::{own, body}`
  are now no-ops in a face and harmless; the modulation family can drop them later. Of my units, only Fervor's *Crush*
  card asked for `Function::Mod`, and it now wears the accent with no change to its file.

Kit findings from the modulation and dynamics reviews (each test fails before its fix):

- **A look's voices follow the display's polarity, not the theme's.** A CRT, a VFD and a
  scope take the dark-theme accents, and chart paper the light ones, in both themes; an LCD
  takes the light second accent. Test:
  `displays::tests::a_look_s_voices_follow_the_display_s_polarity_not_the_theme_s`. It
  covers every unit with a look of its own and a second accent, plus a probe in every
  look: the second voice is the display's accent at 3:1, the same in Slate and Paper on a
  CRT, VFD or scope, and at least 20° of hue from the main trace.
- **Danger and ok are floored on a look's ground.** `DisplayInk::{danger, ok}` are 4.5:1
  and 3:1 on the ground and on a label's plate; `canvas::Ink::of` reads them. Plain, they
  are the theme's. Test: `displays::tests::danger_and_ok_read_on_every_look_s_ground`
  (7 looks × 12 presets).
- **Modulation rings never wear the accent.** `skin::cue_hue(s, f)` is the theme's hue for
  the source's function, turned at least `MOD_GAP` away from a face's accent and kept at
  3:1 on the deck. `views/modulator.rs` uses it for every ring. Test:
  `kit::skin::tests::modulation_rings_never_wear_the_accent`.
- **"Vinyl amp covering"** (Spanish: *vinilo de amplificador*) replaces the brand name
  wherever users read it: the manual (EN/ES, Gravis's row too), ADR 0074, the round reports,
  `Material::name()` (`vinyl`; gallery goldens renamed) and the doc comments. The enum
  variant stays `Material::Tolex`, so the other families' sheets keep compiling.

- **The thumbnails' digest is the same in every process.** `skin::stable(sheet)` is the
  sheet's text with a custom knob or switch painter named by its kind, never by its address.
  `Debug` of a `fn` printed the address, which ASLR moves every launch, so every thumbnail
  missed. The thumbnail folder is also pruned now: the first picture written in a look
  keeps that look and the three looks used most recently before it
  (`picker_thumbs::prune`, `KEEP_LOOKS`). Other files are left alone. Tests:
  `kit::skin::tests::no_sheet_s_digest_text_holds_an_address`;
  `kit::skin::tests::the_sheets_digest_is_the_same_in_every_process` (two child processes
  of the test binary and this one agree);
  `views::picker_thumbs::tests::the_folder_keeps_the_current_look_and_the_last_few`.

- **The user's Text size reaches the words a face paints** (the instruments builder's
  finding). Card titles (`band::card`) and the nameplate (every `Plate`) were painted at
  `s.k`, while the nameplate was measured at `s.tk()`; both are now at `s.tk()`. Fixed the
  same way: the card's selector name, the footer's *OUT* and its chips (measured at `tk`),
  and the menu's shortcuts (measured at `tk`). The digit in a modulation chip's 10-pt dot
  stays at `k`, to fit its dot. No golden changed (the goldens are at 100 % text). Test:
  `tests/skins_text.rs` (at 140 % the title and the name are 1.4× their size at 100 %,
  faces on and off).

**Checks at the end of delivery 2** (`a3fd27f4`, every family merged):
- Windows, `RUSTFLAGS=-D warnings`: workspace clippy clean; `cargo test -p cantus-ui -p
  cantus-plugins`: 190 binaries, 2 013 passed, 0 failed, 32 ignored.
- Linux (WSL, own clone, `tools/wsl/check.sh r32/skin clippy`, then the same two crates'
  tests headless): clippy and the editor-less plug-ins clean; 2 019 passed, 0 failed, 32
  ignored. The clone's `target/` was deleted afterwards.

## Review of delivery 2 (fix round; each test fails before its fix)

1. **The instruments' modulation sources are cues.** The rings (`views/inst/frame.rs`
   `rings_for`; `analog_parts`, `organ_parts`, `wavetable_parts`), the chip bar and drag
   colour (`source_color`) and `shell::mod_card`'s chip bars now use `skin::cue_hue`.
   Before, in a face every function hue was the accent, so envelope, MPE, wheel,
   aftertouch, expression and velocity rings matched the value arc. The changed instrument
   lines are marked *Round 32, skin builder*. Test:
   `views::inst::frame::tests::every_instrument_s_modulation_sources_stay_apart_from_its_accent`
   (every instrument sheet × preset × source family: at least `MOD_GAP − 6`° from the
   accent).
2. **The header registers the token it paints.** It used to register `h.hue.tok()` and
   now registers `s.function(h.hue).hue_tok`. Test:
   `tests/skins_inspect.rs::the_header_registers_the_token_it_is_painted_in` (every
   function, in a face and plain: the registered token's value is the painted hue).
3. **The second voices are painted.** Each is drawn with `ink.voice(plain, true)` (faces
   off: the plain colour as before):
   - Aequalis's analyser (fill, line, legend) in amber;
   - Fervor's odd harmonics in filament yellow (in a face they had matched the even ones);
   - Spatium's early reflections in cyan;
   - Speculum's held peak (line and legend) in red;
   - Tonus's sung pitch in teal.

   Test: `tests/skins_voices.rs::every_tone_unit_paints_its_second_voice`, which checks
   for a vertex in each unit's display in `display_ink(s).trace2`, in Slate and in Paper.
4. **The manual** (EN, ES) has a faces table for the six units (*Each tone, space and
   analysis unit's face* / *La cara de cada efecto de tono, espacio y análisis*), written
   from the sheets.
5. **Text size, every case tested.** `tests/skins_text.rs` covers the card title and its
   selector (*Bell*), the nameplate, the footer's *OUT* and a chip with the meter, and a
   menu shortcut, faces on and off, all at a ratio of 1.3–1.5 at 140 %. Reverting the
   menu's shortcut to `k` fails it.
6. **Cues carry the style's own theme hues.** `Style::cues` holds the theme's function
   hues before a face's accent (set by `Style::new`, kept by `skinned`, the worn style and
   `display_style`). `cue_hue` no longer reads the installed tokens. Test:
   `kit::skin::tests::cues_are_the_style_s_own_theme_s_hues_in_every_preset`.

Also:
- **A dial prints no numbers.** The "0–10" round Latitudo's width (0–200 %) read as a
  second, wrong value. `KnobShape::Dial` is now a graduated scale (long even marks on L
  and XL), for every unit that uses it (Latitudo, Organum, Tremor, Discrimen); the value
  pill says the value in the parameter's units. The manuals and ADR 0074 now say
  "graduated dials". Test: `tests/skins_text.rs::a_dial_prints_no_numbers_of_its_own`.
- The note on measurement noise sits under the first budget table again.

From the integrated-tree review (each test fails before its fix):

- **A. Colours with a meaning stay apart in a face.**
  - `skin::distinct_hue` (`canvas::Ink::third` on a display) gives a third colour. Plain,
    it is the theme's function hue. In a face, it is the style's own theme hue turned at
    least 24° away from the main trace, the second voice and danger, at 3:1 on the ground.
  - Terminus's loudness (trace, target dash, meter; `limiter::loudness_colour`) is no
    longer the ceiling's red. Spatium's post EQ (`curve::post_colour`) is no longer the
    decay curve's colour. Fervor's odd harmonics (`harm::odd_colour`) use the second voice.
  - Tests: `face_colour_tests::colours_with_a_meaning_stay_apart_in_the_face` in
    `limiter.rs`, `space_curve.rs` and `saturator_harm.rs` (every preset: at least 20°
    apart, 3:1 on the ground).
  - The other `s.t.hue(Function::…)` uses inside displays already have their own voice
    (Translatio, Organum's drawbars) or are outside a face (mixer, routing).
- **B. The instruments' and Pulsus's header menus wear the face.** Both `popups` wear
  `shell.skin`. Test: `tests/skins_body.rs::every_unit_s_header_menu_wears_its_face`
  (Unda, Calor, Pulsus).
- **C. One way to draw inside a display.** `views/inst/face::on_display` now delegates to
  `skin::display_style`. It keeps its recall cache and returns a style already on the
  display as it is. The only instrument addition: words also read on Gravis's mixed body
  fill. `dynfx::on_display` is a plate colour, not a style; it is written into ADR 0074 §7
  as the exception. Test:
  `views::inst::face::tests::the_instruments_display_style_is_the_kit_s`. The families'
  tests stay green.
- **D. Manual (EN, ES).** All 25 effects are listed. The faces sentence now says Delay,
  Reverb and Firmus keep the plain look.

Shots: `docs/round32/skins/tone/` (each unit in Slate and Paper at 100 %; 200 % shot and
looked at too). Tests: `kit::skin::tone::tests` (faces of their own, unlike their siblings;
the contrast floor in every preset, on the tint, under the accent, over the material);
`tests/skins_body.rs` over all units; the existing editor layout, menu and budget tests.

## Shared-file lines touched

- `kit/*.rs` (owned): `mod.rs` (`Style::current` worn, `Style::plain`), `knob.rs`, `led.rs`,
  `band.rs`, `well.rs`, `cell.rs`, `segmented.rs`, `shell.rs`, `text.rs`, `inspect.rs`,
  `brush.rs` (one line: the item's skin), `tokens.rs` (`floor` → `pub(crate)`),
  `gallery.rs`.
- `views/fx/canvas.rs` (owned): `Ink::look`, the look's ground/cover, `trace`.
- `views/fx/frame.rs` (owned): `header_name_w(ctx, skin, …)`, the popups' style skinned.
- `design.rs`: `Role::lettered`, `Role::mono_plain` (+ `pub use Weight`).
- `settings.rs`: `unit_faces`. `app.rs`: `apply_unit_faces` and its call in `assemble`.
- `views/workspace_settings.rs`: the *Unit faces* row on the Appearance page.
- `cantus-plugins/src/editor.rs`: two lines in `App::build`; `cantus-plugins/tests/editor.rs`:
  one test.
- Fix round: `kit/{skin.rs, skin/*.rs, brush.rs (glow_polyline), glyph.rs (Score)}`;
  `views/picker_thumbs.rs` (`look_key`, `REV` 2); `cantus-plugins/src/editor.rs`
  (`prepare`, called by `App::build`) and its test; ADR 0074.
- Second delivery: `kit/skin/tone.rs` (the family's sheet); `views/fx/canvas.rs`
  (`Ink::voice`); the tone units' editors: `views/fx/stereo_map.rs` (two lines),
  `views/fx/saturator_curve.rs` (`numbers`); `views/fx/space_curve.rs` and
  `space_tail.rs` (one line each: the heads in `display_style`); `kit/mod.rs`
  (`Style::function`, `function_hue`).
- No other family's sheet or editor file was touched.

## For the family builders

- Your sheet's colours show at once (`Skin::tokens`); your displays take the look through
  `canvas::Ink` (effects) — call `canvas::trace` for a main curve to get the look's glow;
  instruments' displays not on `canvas::Ink` call `skin::display_ink(s)` /
  `skin::display_ground(b, rect, s)` / `skin::displays::cover`.
- Check your sheets with `skin::contrast_failures(&MY_SKIN)` in a test.
- Look: `cargo run -p cantus-ui --example shoot_skins -- <dir> <unit…>`.

## Needs the owner

Nothing.
