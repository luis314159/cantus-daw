# Round 39 — lapis: the Slate Lapis theme

> **Superseded in part by [`lapis2.md`](lapis2.md):** the owner found this first Lapis
> "looks the same" as Slate; it now also lays lapis over the selection, gilds the readouts
> and the loop's edges, and turns Slate's room to night blue (Slate itself unchanged).

An optional theme, approved by the owner to go with the new app logo #03, *the metal* (a gold,
metallic C). **Slate stays the default and does not change.**

## What it is

Slate exactly — surfaces, text, meters, hues, keys, corners, Barlow, `System::SLATE` —
in Night and Paper (`crates/cantus-ui/src/theme_slate_lapis.rs`, built as
`Tokens { …, ..slate_*_tokens() }` so it follows Slate wherever Slate moves). Two
changes:

- **Lapis accent**: `#7ea2f6` (Night), `#2443a8` (Paper), in place of Slate's azure
  `#5cafe9` / `#1f6ec2`. The derived tones use Slate's own ratios, measured from its values:
  Night bright and lit-even-step = 70 % white→accent, highlight band 58.7 %, glyph and
  primary ink = accent × 0.143, menu hover = menu 30 % toward the accent, text selection =
  accent × 0.35. Paper text selection and band = 50 %, bright 84 %, lit even step 78 %;
  white on the accent; menu hover = the accent.
- **Gold details, from the logo's metal**: the playhead (`#e2b85a` / `#9a6e1c`) and the loop
  brace's ends. Slate's tabs have no underline, so there is none to gild.

Contrast (WCAG): accent on the panel 6.3:1 (Night) and 7.9:1 (Paper); text on the accent
7.2:1 and 8.6:1; gold playhead on the rows 8.1:1 and 4.0:1. Every kit promise and round 25's floor
hold (`theme::tests`, `theme_slate_lapis::tests`).

## Where it shows

- `theme::PRESETS` and `kit::appearance::PRESETS` (after Slate); Settings ▸ Appearance's family
  row has a fourth picture (`views/workspace_settings.rs`, `FAMILIES`, tiles sized for *n*).
  Each family's picture now draws its playhead, so Slate Lapis's gold tells it from Slate at a
  glance.
- The loop brace's ends (`views/arrangement_paint.rs`) take the playhead's colour in a
  recessed family. In Slate, Graphite and Studio ink that token equals the accent, so nothing
  moves there; classic themes keep the accent.
- Names: `theme.slate_lapis_{night,paper}` ("Slate Lapis, Night (dark)" / "Pizarra
  lapislázuli, Noche (oscuro)"), `topbar.look.family.slate_lapis` ("Slate Lapis" / "Pizarra
  lapislázuli").

## Tests

- `tests/slate_lapis_e2e.rs`: boots through `CantusApp::assemble` on a fresh data folder,
  closes the start page, opens Settings ▸ Appearance, **clicks Slate Lapis's picture**, checks
  Slate's structure and face with the lapis accent installed, plays the song and finds the
  arrangement's painted shapes in the gold (and none in Slate's azure playhead), then restarts
  on the same folder and finds the choice kept. Slate is still the default.
- `theme_slate_lapis::tests`: only the accent tokens and the playhead differ from Slate; every
  promise holds as written; the theme resolves to its colours and to Slate's shape.
- Every test that iterates the presets (legibility, contrast floor, colour roles, skins,
  `slate_e2e`'s switch-and-back) now covers both Slate Lapis themes.

## Pictures

`cargo run -p cantus-ui --example shoot_lapis -- docs/round39/lapis`, 1920 × 1080 at 125 %:
`arrangement-`, `mixer-`, `pianoroll-`, `look-` × `slate_lapis_night`/`slate_lapis_paper`;
`look1280-*` (the family row at 1280 × 720, 100 %); `arrangement-slate_night`/`-slate_paper`
for side by side. The arrangement is playing in a loop of bars 5–9.

## Open

- The playhead keeps Slate's 1.5-pt line (Slate's geometry). A thinner line would need a
  per-theme width.
- At rest the accent shows in few places (the selected track, a send's dot, the loop bar), so
  Slate Lapis and Slate look close until something is selected or playing. That follows the
  brief, which asked for subtle.
