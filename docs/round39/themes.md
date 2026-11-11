# Round 39 — themes: the ten earlier themes back, redrawn, and four new ones

The owner, 2026-11-10: *"Recover all of them, and add more if you want."* Round 39 had retired
the ten themes of rounds 24–38 (ADR 0111 §2, `kit::appearance::RETIRED`: a setting naming one
showed Slate in its light). This branch (`r39/themes`) brings every one of them back,
re-expressed in round 39's design system, adds four of its own, and puts all twenty-four on
Settings ▸ Appearance. **Slate and Slate Lapis do not move by a pixel** (their files are
untouched; `slate_window_golden`, `slate_lapis_e2e` and `themes39::slate_and_slate_lapis_are_untouched`).

## What ships

| Group on the page | Themes |
|---|---|
| Round 39's families | Slate Night · Paper (default), Slate Lapis Night · Paper, Graphite Night · Paper, Studio Ink Night · Paper |
| From earlier rounds, redrawn | Cantus Night · Paper, Blue Hour · Daylight, **Pewter · Chalk** (the FL-derived *Slate* and *Paper*, renamed), Ink & Gold · Parchment, Ultraviolet, Lagoon |
| New | Evergreen Night · Paper, Bordeaux, Print |
| For low vision | High contrast dark · light |

Each restored theme keeps its own light and its own character (its old palette), its old preset
id (so `preset:slate` is Pewter, `preset:paper` is Chalk) and takes round 39's structure. The
table of characters and systems is ADR 0111 §2; in short:

- **Cantus Night** — espresso room, ivory inks, copper keys, verdigris selection laid over every
  selected fill, amber clock, rulers, playhead and loop edges. Graphite's system (Sofia).
- **Cantus Paper** — the website's cool stone; copper-deep keys and readouts, verdigris-deep
  selection. Graphite's system.
- **Blue Hour** — deep navy, electric-blue keys, ice-blue readouts. **Daylight** — white with
  blue: pale-blue bars, a vivid blue on keys, readouts and loop. Graphite's system.
- **Pewter** — a lighter, bluer slate-grey room than Slate's over a dark gutter, orange keys,
  lime selection, playhead and meters (FL's look, nobody's name; first drawn mid-grey, darkened
  in the legibility pass below so the user's colour cells read on its bars). **Chalk** — its grey chalk twin, deep-green keys and
  selection. Slate's system (Barlow).
- **Ink & Gold** — blue-black ink, ivory, gold keys, readouts and playhead, soft-blue selection.
  **Parchment** — a warm cream sheet, burnt-umber keys, sepia readouts, gold playhead and loop.
  Studio ink's system (Plex).
- **Ultraviolet** — deep violet, magenta keys and loop, cyan selection, playhead and clock.
  **Lagoon** — teal room and keys, amber selection, readouts and playhead. Slate's system.
- New: **Evergreen Night** (forest green and brass, cream inks) and **Evergreen Paper** (pale
  sage, forest-green keys, brass readouts), Graphite's system with **Jack** cables (real patch
  cords); **Bordeaux** (deep wine, rose-gold keys, champagne readouts), Graphite's system;
  **Print** (black ink on white, one vermilion for playhead, loop, record and clip, grey meters,
  a heavier veil on the track colours), Studio ink's system with **Subway** cables.

**Systems.** Every redrawn and new theme has recessed keys. Classic stays only where a theme is
about it: the high-contrast pair and users' own older themes. No restored theme was about the
raised keys (Pewter's FL character is its mid-grey chrome and lime, not the relief).

## How they are made

`tools/themes/themes.py` (with `build.py`, `check.py`, `colour.py`) writes
`crates/cantus-ui/src/theme_redrawn.rs` and `theme_added.rs`. Each theme is a short spec over
**Slate's own ladder** (read from `theme_slate.rs`): every room token (the 28 surfaces, wells and
lines `lapis2` turned, and the knob body) turned to the theme's hue with its own amount of
colour, at Slate's luminance scaled in WCAG's terms, (L + 0.05) × f − 0.05 — which keeps every
contrast between two surfaces exactly as Slate has it (`lapis2`'s rule is f = 1) —; the metal
(caps, rims, keys) and the inks turned the same way; the accent's tones derived as Slate's; then
the theme's own colours from its old palette. `check.py` measures every pairing of
`Tokens::pairings`, round 25's floor and the recessed keys of `theme::tests`, the old palette's
test, the materials, the piano's names on the ivory, and (new) the meters' amber as the warning's
ink on the bar; anything short is moved the least that holds it (the script prints each move)
and nothing is written while a promise fails. To change a theme, change its spec and run
`python tools/themes/themes.py --write`.

Where a colour moved for a promise: function hues and the automated/modulated inks on the lighter
rooms (Pewter's pastel, the papers' a step deeper); Pewter's record red to a pale coral (4.5:1 on
its mid-grey, as FL's own); Daylight's and Chalk's meters in a white groove with deep colours
(their old dark wells made the amber — also the warning's and the solo's ink — unreadable on the
bars).

## The old settings

`appearance::RETIRED`, `successor` and `Appearance::retire` are gone: every old id names its own
theme again, so beta.13's `appearance.json` opens in the theme it names, its mode's own changes
on top, user themes untouched. The old constructors stay unchanged (Classic, gradients) for the
kit's tests and golden sheets; `appearance::CLASSIC` lists them by id so that a pre-appearance
`theme.json` that is exactly one of them opens in that preset, redrawn, in its own mode (Paper —
written by every release of that era on first run — leaves a fresh install).

## Settings ▸ Appearance

The tiles in four groups, four to a row (fewer on a narrow page), each its picture in its lights
side by side (one picture for a theme in one light), its own playhead, readouts, tint and loop
edge drawn; the name under it, elided if it ever has to be. A family's tile sets both lights; a
one-light theme's (Ultraviolet, Lagoon, Bordeaux, Print) sets its light and switches to that mode
so the click is seen. The page scrolls; the mode, language, size, faces, cables, colours,
shortcuts and the taskbar icon follow below as before. Names in both locales (`look.*.txt`;
`theme.slate` is now "Pewter (dark)" / "Peltre (oscuro)", `theme.paper` "Chalk (light)" / "Tiza
(claro)").

## Tests

- `tests/themes39.rs` — every redrawn and new theme keeps every promise as written and resolves
  to exactly its tokens; is of round 39's system (recessed, a round-39 face, flat, the veil); stands
  on Slate's ladder in its light; the restored keep their light and id, the FL pair is renamed,
  no name collides or carries a product's mark; **no two themes of one light are closer than
  ΔE 14** (base + bars + half the accent, CIELAB; Slate vs Slate Lapis, approved apart, is 16.8 —
  the measure caught a first Daylight that was Studio Ink Paper's twin, and a Parchment with
  Graphite Paper's accent); Slate and Slate Lapis untouched.
- `tests/themes_e2e.rs` (through `CantusApp::assemble`): Cantus Night chosen on its tile —
  both lights set, tokens, Sofia, its room painted, the chosen page's row through the verdigris;
  Print chosen — the light mode set and shown, its room painted, its vermilion playhead in the
  arrangement; a restart keeps both. beta.13's `ultraviolet` / `parchment` and a round-24
  `theme.json` (Ink & Gold) open in their themes. The page has a tile for every preset, none
  overlapping or cut, the last scrolled into view.
- `kit::appearance::tests::every_old_preset_id_names_its_own_theme_again_keeping_the_slot_s_changes`,
  `a_fresh_install_shows_slate_and_an_old_choice_shows_its_theme`; `appearance_file` tests;
  `slate_e2e::a_setting_of_beta_13_opens_in_the_theme_it_names`.
- Every test that iterates the presets (legibility, contrast floor, colour roles, materials,
  skins, faces, the piano's names, `slate_e2e`'s switch-and-back) now covers all twenty-four.

## Every theme easy to read, measured (the owner: "Todos deben ser fáciles de trabajar y de leer")

`views/arrangement_legibility.rs` (a unit test beside the arrangement's painters, so it
measures the colours they really paint: `note_ink` on `body_ground` for every demo track's
tint through the veil — at rest, selected, lit and playing —, the selection outline, the
grid's inks, the ruler's ticks, the fader cap's rim as `fader_slate` draws it, solo's and
record's inks on their keys, the lit key, the meters, the focus ring, the colour cells) holds
every shipped theme to WCAG 1.4.11's 3:1 for what carries meaning — or, where Slate itself
(the approved default) stands under it, to Slate's own measure in the same light (×0.97;
the colour cells ×0.85). Beat lines and the ruler's beat ticks need 1.3:1. The demo's
tracks: their colour cells at least ΔE 12 apart (they are 16.5–17.1 in every theme), their
clip bars (calm by design, ADR 0111 §3) no calmer than Slate's (×0.8).
`tests/themes_whole_words.rs` holds every theme's window whole (below).

What the measure found, and what changed, at the cause:

| Where | Found | Fixed |
|---|---|---|
| Every theme (Slate's family and high contrast too) | the arrangement's bar lines 1.18–1.26:1 on the lanes, beat lines 1.01–1.05:1 — invisible | `arrangement_slate::Ink` holds the bar line at 3:1 and the beat line at 1.35:1 on both lanes, each theme's own line ink moved only as far as it must (once a theme, cached) |
| Chalk, Graphite Paper, High contrast light | solo's amber (the meters') 2.8 / 2.9 / 1.3:1 on its key | solo's ink held at 3:1 on its well and bar (the meters keep their colours) |
| Pewter | the user's colour cells 1.56:1 on its mid-grey bars | the room darkened (f 1.95 → 1.15): 2.65:1, the bars still lighter and bluer than Slate's |
| Pewter, Chalk | the units' accents 3.2–4.4:1 on their cards | cards a step off the desk (round 24's own rule): 4.5:1 |
| Daylight, Chalk | meters in a dark well made their amber — also the warning chip's and solo's ink — 1.6:1 on the bars | meters in a white groove with deep colours, as Slate Paper's |
| Print | a veil of 77 % left two tracks' clip bars 0.7 ΔE apart | Slate's veil (59 %): 2.0, as Slate Paper's 1.4 |
| Bordeaux | the wine veil drew the tracks' bars together (4.0 ΔE) | its veil at 51 % |
| Parchment, Bordeaux | (by eye) a banana-yellow sheet; a bright rose-brown room | calmer cream (gain 3.0 → 1.9); a deeper wine |

The coordinator's suspicions, measured: Parchment's clip content 4.55:1 on its bodies, Daylight's
4.17:1 (both over 3:1: deep tints of the clip's own colour, not tan on tan); Evergreen's and
Lagoon's lanes 5.7:1 content, bar lines now 3:1; Bordeaux's record and solo 5.5:1 on its keys;
Ultraviolet's selected strip 3:1 against its neighbours, its meters 6:1 in their groove. The
100 % crops (`docs/round39/themes/crops/`) show the 9–11 pt words crisp in every theme.

The table the test prints (`cargo test -p cantus-ui --lib legibility -- --nocapture`), after
the fixes, is in `docs/round39/themes/legibility.txt`.

**The three left open, closed** (the owner, 2026-11-10: "do what you think is best, seeking
excellence"), at the cause, in every light theme — Slate Paper included — and held by the
same test:

- **The track's colour marks at 3:1 on light bars.** `kit::color::track_mark` gives the colour
  cell, the routing chips, the mixer strip's colour band, the automation editor's colour and
  `Tints::small` the safe band's colour with, on a light theme, its lightness taken down (hue
  and saturation kept) until it reads at 3:1 on the bar, the strip and the selected strip; the
  arrangement header's stripe (`Ink::stripe`) at 3:1 on the header and the lanes. Every light
  theme's marks now measure 3.00–3.06:1 (they were 1.5–1.84:1). Night's are as they were.
- **Kick and Clap told apart at rest on paper.** A light theme's large fills take a stronger
  band (saturation up to 0.55, from 0.38) and half their theme's veil (`LIGHT_VEIL`): the
  demo's nearest two clip bodies are now ΔE 3.4–4.1 apart (were 1.4–2.0). The floor is **ΔE 3**:
  over the 2.3 of a just-noticeable difference (CIE76), enough to be seen at a glance side by
  side, with the 8-bit steps of a pale fill (≈0.7 each) in hand. Calm is held too: the bodies'
  chroma at rest stays under 20 (Slate Paper 17.9; the night themes' range from Slate Night's
  13.4 to Blue Hour's 19.1, Ultraviolet's 30.8). Daylight's room took a little less blue
  (gain 3.4 → 3.0), its veil 160 and its selection tint 16, to stay calm with its blue outline at
  3:1.
- **The paper fader cap's rim at 3:1.** `kit::fader_slate::paper_rim` takes the rim down until
  the white cap's outline reads at 3:1 on its strip, selected or not (it was 1.8:1).

**What moved in Slate Paper, exactly** (and in every other light theme the same way): the
track colour cells, chips, the mixer strips' colour bands and the arrangement headers' stripes
(a darker shade of the same hue); the clip bodies and title bars at rest (a little more of their
colour, half the veil); the fader cap's rim (darker). Its tokens did not change
(`themes39::slate_and_slate_lapis_are_untouched`). **Slate Night** moved in nothing beyond the
grid lines and solo's ink of the first pass: every change here is gated on a light theme.

## The menu bar's Help and the status line (the coordinator's two defects)

Cause: the top bar's group widths and the status line's readouts are measured once and kept in
caches per thread, keyed by `kit::cache::fonts_seen` — which counted font rebuilds *per
context*. A second window in the same process (every shot after the first; any test or tool
that makes contexts in turn) starts again at 0, takes the first window's widths as its own,
and so a theme set in Sofia or Plex after one in Barlow lost Help off the menus (Graphite,
Studio ink, Ink & Gold, Evergreen…) and cut "Typing keyboard off (Ctrl+T)" in the status line.
Fix: `fonts_seen` is a process-wide number, taken anew by each context's first fonts and by every
rebuild. `tests/themes_whole_words.rs` starts the program (`assemble`) in each of the 24 themes,
one after another in one process, at 1920 × 1080 125 % and 1600 × 900 at 100 % and 125 %: the
six menus all there (or all folded into ≡), no word elided or left out, the status line whole.

## What was run (Windows)

- `cargo test -p cantus-ui --no-fail-fast`: the lib's 534 and every integration binary green
  but two, neither of this branch's making:
  - `arrange_e2e::the_arrangements_structure_end_to_end` fails the same at the base `1e92c38b`
    (built from the base's sources here): its "click away" at (1000, 700) lands on the mixer's
    empty-slot key, inserts a strip, and the undo that should take the track's icon back takes
    the strip instead (`cmd.insert_strip`). For its owner (lo-arrange / el-mixer).
  - `slate_window_golden`: at the base, its arrangement and mixer cases already fail
    (105 688 / 172 656 / 238 682 pixels, the same counts with and without this branch: the
    desk draws its empty strips 17–36 since the goldens were blessed); with this branch the
    piano roll's cases differ too, **on purpose** — the grid's bar and beat lines (above).
    Re-bless with `CANTUS_BLESS=1` once the integrator has looked at the difference; Slate's
    colours themselves are untouched (`themes39::slate_and_slate_lapis_are_untouched`).
- `cables39_e2e::the_style_is_a_setting_kept_across_a_restart` failed once with this branch
  (Settings ▸ Appearance grew, the cables' row went below the fold and the click landed while
  the page still glided): its `click_word` now lets the page settle after each wheel. Green.
- `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui --all-targets`: clean. `cargo fmt -p
  cantus-ui`: done (the generator's output is rustfmt'd after it writes).
- Not run: the Linux check in WSL.

## Shots (`docs/round39/themes/`)

`cargo run --release -p cantus-ui --example shoot_themes -- docs/round39/themes [--look]`, then
`python tools/themes/contact_sheet.py docs/round39/themes`. 1920 × 1080 at 125 %, the song the
program opens on, the Clap clip selected, playing in a loop of bars 5–9, the arrangement with the
mixer under it.

- `contact-sheet.png` — all twenty-four, reduced, in the page's order.
- `full/<id>.png` — one full-size shot per theme.
- `crops/<id>.png` — at the shot's own pixels: the track headers and clip lanes, the mixer's
  strips, the inspector (`python tools/themes/crops.py docs/round39/themes`).
- `look-slate_night-top.png`, `look-slate_night-foot.png` (wheeled down to the new themes and high
  contrast), and the same in Slate Paper and Print.

## Open

- Judged only on the CPU renderer's shots; the owner's eye on a real screen decides each theme's
  strength (Parchment's cream, Bordeaux's wine, Ultraviolet's violet, Pewter's mid-grey).
- The arrangement's playhead *line* is each theme's accent and only its cap the playhead token
  (as in Slate Lapis); Print's line is therefore ink, its cap vermilion.
- The user's track colours are spent through each theme's veil; Print's heavier veil keeps them
  as tints, not grey (a monochrome mode for clips would be a feature of its own).
