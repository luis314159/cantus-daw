# Round 25 — builder *night*: Cantus Night for long sessions, and the website's light

Branch `r25/night` (base 22f69c8). Amends ADR 0046 (§8). The owner, 2026-10-12: *"Make the
default theme look better and more pleasant to the eye for working for hours; if you like,
add gradients like the website's style."*

## What tired the eye (the first Cantus Night, looked at for hours)

Measured on `shoot_window`, `shoot_mixer` and `shoot_site` pictures (before/after in
`night-shots/`):

1. **Maximum-glare text.** Ivory #f6ece2 on the panel at **15.5:1**, 16.6:1 on the canvas —
   every label, menu and value at full brightness. Halation on near-black; nothing ranks.
2. **Large saturated fills.** Track headers and clip bars are FL's large band (L 0.36–0.38,
   S up to 0.40): eight headers and a full arrangement read as a wall of orange, blue,
   purple, green.
3. **Bright objects in rows.** Sixteen ivory fader caps (#dccbbb, the lightest thing on the
   desk) and five or six full-amber toolbar keys (#f5ad3d) side by side.
4. **Pure-dark wells beside bright meters** (#0b0806) and bar lines brighter than needed.
5. **Flat and dead.** One brown-black plane; nothing of the website's light.

## What the guidance says

- **Material dark theme**: avoid pure black (#121212-class surfaces); primary text ≈ 87 %
  white, secondary ≈ 60 %; elevation by *lighter* surfaces; desaturate colours used on large
  dark areas; keep saturated colour for small, meaningful things.
- **Pro DAWs** (Bitwig, Ableton's dark themes, FL's default, Logic): mid-dark, low-chroma
  neutrals; one accent for what is active; clip/track colours muted when large (Ableton and
  Bitwig both damp clip colour on dark themes), vivid where small (chips, meters); body text
  well below pure white.
- **WCAG**: 4.5:1 for text (AA), 7:1 (AAA) is generous; beyond ~13:1 on a dark ground only
  adds glare.

## Decisions

- **Keep the identity**: warm espresso, ivory, copper, amber, verdigris — the website's
  colours, the canvas still #120d0a.
- **Surfaces a step up and calmer**: panel #1d1410 → #1f1712, toolbar #231913 → #251b15,
  tile #2a1d15 → #2b2019, strip #191210 → #1b1411, wells #0d0907 → #110c09, meter well
  #0b0806 → #0f0b09; beat/bar lines #3b2a1f/#5a4433 → #382a20/#4f3d30.
- **Comfortable text**: primary #f6ece2 → **#e6d9ca** (12.7:1 on the panel, 13.9:1 on the
  canvas, ≥ 10:1 under every gradient); secondary #c1a995 → **#b09a86** (6.6:1); labels
  #dccbbb → #cbbaa8 (9.6:1 on a strip).
- **Bright things softer, not weaker**: fader caps #dccbbb → #b9a997; white keys #efe4d8 →
  #ddd0c2; active keys #f5ad3d → #e09b48 (glyph 6.7:1); focus #ffdf9e → #f0cf98; playhead
  #ffd25e → #f5c860 (11:1 on the lanes); selection #5fc2ac → #58b8a3 (7.6:1 on a strip);
  record #ff6450 → #f0604c. Meter clip stays #ff5a3c: it must shout.
- **Track colours**: a new token, `editor.track_veil` (#2b221c at 43 %), lays an espresso
  veil over a track's *large* fills (headers, clip bars and, through them, clip bodies) —
  lower chroma and lightness, the hue kept (≤ 12°); the **colour cell keeps the bright
  hue**, so a track is still found by its colour.
- **The website's light, as theme tokens** (clear = flat; every other theme stays flat):
  - `depth.sheen` (#ffd9b0 at 7 %): a top-light fading to clear down two thirds of a bar —
    the transport, every view toolbar, the mixer's and inspector's title rows, section
    bands. The active key takes the site's button gradient (pale top, highlight, deeper
    bottom) instead of FL's mid band when a theme has a sheen.
  - `depth.glow` (copper #d4622a at 12 %) near the top right and `depth.glow_cool`
    (verdigris #3e8e7e at 8 %) at the bottom left of the arrangement's lanes, the mixer
    desk and the piano roll's grid — the hero's two radial gradients, under the lines
    and the clips.
- **Cheap by construction**: the sheen is one gradient rect (≤ 16 vertices); a glow is
  12 rings × 48 segments of geometry, clipped exactly to its ground (no texture: the GPU
  and the CPU renderer draw it alike, and a stretched texture showed steps in the CPU
  renderer; a vertex grid showed diagonal seams). No allocation once the brush's buffers
  have grown; no extra draw call (it is in each view's own mesh).

## Measured

| | before | after |
|---|---|---|
| primary text on panel / canvas | 15.5 / 16.6 : 1 | 12.7 / 13.9 : 1 |
| primary text, worst gradient (both glows on the panel) | — | ≥ 10 : 1 |
| secondary text on panel | 8.1 : 1 | 6.6 : 1 |
| selection on a strip | 8.6 : 1 | 7.6 : 1 |
| playhead on the lanes | 13.2 : 1 | 11.9 : 1 |
| fader cap L | 0.80 | 0.66 |

Frame cost (`tests/night_budget.rs`, whole window at 1080p, playing): see the report and
the test's output — no allocation and no draw call added; the glow triangles are a few
thousand at most.

## Tests

- `tests/theme_gradients.rs` — every theme defines the four tokens; only Cantus Night is
  lit, every other theme flat and unveiled, an old theme file derives flat; a flat theme
  paints **no vertex** of gradient; the glows stay inside their ground at any shape and
  under 4 000 vertices; the text over every gradient ≥ 7:1 and Cantus Night's primary text
  10–13.5:1, secondary 4.5–8:1; meters/selection/playhead ≥ 3:1 over the glows; the veil
  calms every track colour's large fill, keeps its hue and the colour cell, its name ≥ 4.5:1.
- `Tokens::pairings` measures the gradients for any theme that paints them (text, labels,
  selection and focus over both glows and the sheen), so the contrast floor, the knobs'
  easing and the family test hold them too.
- `tests/night_budget.rs` — Cantus Night against itself with the gradients cleared: no more
  allocations, the same draw calls, ≤ 6 000 more triangles; timing with `--ignored`.

## Pictures

`F:/projects/daw-r25/night-shots/` — `before/`, the iterations, and `after/`. The website
hero for `web/assets/app-mixer-xl.png` is `after/app-mixer-xl.png` (not committed — the
integrator updates the site).
