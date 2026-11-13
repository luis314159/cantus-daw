# The website: light-theme contrast and motion (side round fx, branch `side/web`, 2026-11-13)

The owner (2026-11-13): "Pon un agente a que en la página web agregue animaciones y cosas
interesantes para pulir más el sitio, y que en download en modo claro sea más fácil de leer,
el color de las letras no ayuda."

## 1. The download page in the light theme (fixed first)

**Cause.** The download page's head is a `<section class="dl-hero page-band lapis">`. The bands'
rule (`frame.css`: every `main > section:nth-of-type(odd)` gets the paper band) is more specific
than `.page-band`, so in the light theme the head lost its lapis ground and took paper
(#f5f6f9), while `.lapis` kept its tokens for a dark ground: white title, pale-lavender lede,
white version line, and a dark translucent card that read as a grey slab. In the dark theme the
same rule turned the head Slate instead of lapis. Every other page's head is a `<div>`, so it
kept the lapis, but in the light theme the bar over it reads deep blue (it is dressed for the
white home hero), so the bar's own words (Cantus, Tour, Instruments…) were deep blue on lapis:
1.0–1.3:1 on every instrument, effect and Measured page.

**Fix.**
- The bands' rules skip `.page-band` (the download head is lapis again in the dark theme).
- In the light theme a page's head is dressed like the home's hero (round 39, the owner's own
  choice for the light hero): white with the lapis dot grid, deep-blue words (#1b2c7a 12.6:1,
  #24398f 9.9:1 on white), the eyebrow a darker gold (#7a5410, 6.8:1 on white and 4.8:1 even on
  a dot of the grid). The bar above it now reads on white, as on the home.
- The download card ("Before you open it") in the light theme is a white sheet with a lapis
  hairline and a soft shadow; its keys and the SHA-256 wells are light (#eef1fa) with deep-blue
  text; the quiet button on a light head is white, so the dot grid stays out of "411 MB".
- In the lapis scope, `--ink-3` #93a1cf → #b8c2e6 (3.2 → 4.6:1 on the ground's brightest glow:
  the hero's facts line, the crumbs, the download card's version); the dark theme's
  `--accent-text` #7ea2f6 → #93b2f8 (the Measured cards' "Won" tag, 4.2 → 5.0:1).

## 2. The contrast check (`web/tools/contrast.mjs`)

It opens every page the build pre-renders (86, plus the two manual pages, the press kit and the
404: 90 pages), each in light and dark, 1440 and 390 px wide (360 views), scrolls a screen at a
time and measures every visible text node: the screen is captured once with the glyphs made
transparent (`-webkit-text-fill-color`, which leaves borders, icons and currentColor as they
are), and the text's colour, alpha and opacity are composited over each of 27 pixels sampled
under each line box. A line passes when 90 % of its samples reach WCAG AA (4.5:1; 3:1 at 24 px,
or 18.66 px bold). Text under `aria-hidden`, or in a disabled control, is decorative and counted
apart; text that something else covers (the sticky bar) is skipped. It is measured with reduced
motion, so nothing is mid-animation. No npm package: Node 22's WebSocket speaks the DevTools
protocol to Chrome or Edge (`tools/cdp.mjs`), and the screenshot is decoded by a 40-line PNG
reader.

`site-budget.yml` runs `--quick` (one page of each kind, both languages, without the manual:
the manual's two long generated pages take minutes each and are left to the full run) and
`tools/motion.mjs` on every push that touches `web/`.

**Measured, every page (360 views: 90 pages × light/dark × 1440/390 px; 54 718 text elements).**
Before (master dd4ad68b): **989 texts below AA in 232 of 360 views**, lowest 1.01:1. After:
**0**, lowest 4.58:1. Per kind of page (EN and ES together; `<unit>` is all 10 instruments or
all 25 effects; "lowest" is the worst text's ratio):

| Page | Scheme | Views | Before: lowest · texts below AA | After: lowest · texts below AA |
|---|---|---|---|---|
| `/` | dark | 4 | 3.35 · 29 | 4.58 · 0 |
| `/` | light | 4 | 3.60 · 4 | 4.89 · 0 |
| `/404.html` | dark | 2 | 6.33 · 0 | 6.33 · 0 |
| `/404.html` | light | 2 | 7.93 · 0 | 7.93 · 0 |
| `/ai/` | dark | 4 | 5.93 · 0 | 5.93 · 0 |
| `/ai/` | light | 4 | 4.96 · 0 | 4.96 · 0 |
| `/download/` | dark | 4 | 4.98 · 0 | 4.98 · 0 |
| `/download/` | light | 4 | 1.08 · 68 | 4.96 · 0 |
| `/effects/` | dark | 4 | 4.64 · 0 | 4.64 · 0 |
| `/effects/` | light | 4 | 1.01 · 20 | 4.96 · 0 |
| `/effects/<unit>/` | dark | 100 | 3.27 · 50 | 4.72 · 0 |
| `/effects/<unit>/` | light | 100 | 1.01 · 550 | 4.81 · 0 |
| `/instruments/` | dark | 4 | 4.64 · 0 | 4.64 · 0 |
| `/instruments/` | light | 4 | 1.01 · 20 | 4.96 · 0 |
| `/instruments/<unit>/` | dark | 36 | 3.21 · 18 | 4.63 · 0 |
| `/instruments/<unit>/` | light | 36 | 1.01 · 198 | 4.96 · 0 |
| `/learn/` | dark | 4 | 5.67 · 0 | 5.67 · 0 |
| `/learn/` | light | 4 | 4.96 · 0 | 4.96 · 0 |
| `/manual/en/` | dark | 2 | 5.86 · 0 | 5.86 · 0 |
| `/manual/en/` | light | 2 | 4.96 · 0 | 4.96 · 0 |
| `/manual/es/` | dark | 2 | 5.93 · 0 | 5.93 · 0 |
| `/manual/es/` | light | 2 | 4.95 · 0 | 4.95 · 0 |
| `/measured/` | dark | 4 | 4.18 · 12 | 4.96 · 0 |
| `/measured/` | light | 4 | 1.01 · 20 | 4.90 · 0 |
| `/press/` | dark | 2 | 5.50 · 0 | 5.50 · 0 |
| `/press/` | light | 2 | 5.83 · 0 | 5.83 · 0 |
| `/roadmap/` | dark | 4 | 5.93 · 0 | 5.93 · 0 |
| `/roadmap/` | light | 4 | 4.96 · 0 | 4.96 · 0 |
| `/studio/` | dark | 4 | 5.93 · 0 | 5.93 · 0 |
| `/studio/` | light | 4 | 4.96 · 0 | 4.96 · 0 |

## 3. Motion (`web/style/motion.css`, a few lines of Rust)

Chosen to feel like the program, not a template: the gold rule of logo #03 carries a signal,
the program's glass catches the light, a playing song breathes in time.

| What | How | Where |
|---|---|---|
| Sections and cards rise into place (opacity + 26 px + 1.5 % scale), a row of cards as a wave | CSS scroll-driven animations (`animation-timeline: view()`), no script; a browser without them (Firefox today) shows everything at rest | every page |
| Measured: each bar draws out from zero as its card arrives, the value fades in after | the same, `scaleX` from the left | /measured/, the home's chapter |
| A slow light crosses the program in the hero every 11 s | a pseudo-element's `translateX` | home |
| A spark runs along the gold hairline under the hero and under each page's head, then rests | a pseudo-element's `translateX`, 9 s period | home, every page head |
| Cards lift 5 px; a deeper shadow and a lapis ring fade in under them; the picture leans in 4.5 %; "See it large →" steps forward | `transform`, and the shadow on a pseudo-element whose opacity changes (the shadow itself is never animated) | instruments, effects, studio, download, unit pages |
| The download button: the arrow bobs while pointed at, the key gives under a press, and a click on a file link answers — the arrow drops through and a gold line runs under the label | CSS; one document listener (`main.rs`, `download_answers`) adds `is-sent` for 1.6 s to `a.btn` links to a release file | every download button |
| A playing song's button breathes a ring; the hero's at two beats of Neon Tide (124 BPM); before the first play it invites three times | CSS on the existing `is-playing` / `is-on` classes | hero, Listen, effects' A/B, unit cards |
| A page the visitor goes to settles in (opacity + 10 px, 0.42 s); never the page the address opened, so the first paint and LCP are untouched; a jump within the page does not replay it | `app.rs` marks `<main>` `is-arriving` when it was mounted by a visit | every in-app navigation |
| Day and night cross-fade (0.45 s); the switch's icon turns in | View Transitions API where there is one (`theme.rs`), else at once | the theme switch |

Rules held (`tools/motion.mjs`, in CI, seven pages at 1440 and 390 px): every `@keyframes` on
the site moves only `transform` and `opacity` (15 rules); with `prefers-reduced-motion: reduce`
nothing is left animating after scrolling a page top to bottom, no scroll-driven animation
exists, and every revealed element is opaque and in place; with motion allowed, once the page
has been scrolled past, every reveal has finished (no element left half-shown), and the motion
is really there.

A trap found by Lighthouse's own contrast audit (axe) and now held by that last rule: a
scroll-driven reveal takes its view from the nearest box that scrolls, and on a phone the
effects' list scrolls sideways — its items stayed half-transparent (2.0–2.6:1). Lists that
scroll or clip on their own (the effects' list, the songs) now rise whole. Everything in `motion.css` is inside
`@media (prefers-reduced-motion: no-preference)`, and the view-transition fade is removed under
`reduce`. Nothing hides content until a script runs, so the pre-rendered page, crawlers and a
visitor without JavaScript see it all.

Not done, on purpose:
- **Counters that tick up in Measured.** The values are decimals and ranges (0.76, 1.1–1.5,
  6.6–6.7); a ticking number would show, for half a second, a figure that was never measured,
  on the one page whose point is that every number is measured. The bars draw out instead.
- **A hero entrance.** The app redraws the pre-rendered page when the WebAssembly arrives
  (`main.rs`), which would restart any timed entrance midway; and fading the hero's words would
  delay the mobile LCP (its element is the hero's text). The hero's life is ambient instead.

## 4. Budgets, before and after

Both built from the same sources' data (the latest release, beta.14, fetched with
`tools/release_json.py` into a local copy only — see §7), served by Lighthouse CI on its mobile
preset (slow 4G, 4× CPU) on the owner's PC with the owner's Edge open, **5 runs each, medians**:

| Line | Budget | Before (master dd4ad68b) | After (side/web) |
|---|---|---|---|
| WebAssembly, gzip | ≤ 250 000 B | 231 454 B (620 650 raw) | 232 444 B (623 346 raw), +990 |
| JS glue | — | 53 895 B | 55 162 B |
| Home HTML (CSS inlined), gzip | — | 33 485 B | 36 635 B (+3.1 KB: `motion.css` is 3.1 KB gzip) |
| Download page HTML, gzip | — | 28 244 B | 31 413 B |
| First visit (total transfer) | ≤ 1 MB | 687 096 B | 691 156 B |
| Largest contentful paint | ≤ 2.0 s | 1.975 s | 1.969 s |
| Cumulative layout shift | ≤ 0.02 | 0 | 0 |
| Accessibility | 100 | 100 | 100 |
| Performance score | — | 98 | 95 |
| First contentful paint | — | 1.126 s | 1.116 s |
| Total blocking time | — | 142 ms (102–185) | 201 ms (130–362) |
| Best practices · SEO | — | 100 · 100 | 100 · 100 |

Every budget line holds. The blocking time is the one cost: a build with everything but the
scroll reveals measured 179 ms (170–264), and the main thread's style-and-layout time is 541 ms
before, 538 ms without the reveals and 602 ms with them — so the ~80 reveal timelines cost about
60 ms of style work at start on a 4×-slowed phone CPU (≈ 15 ms on a real mid laptop); the rest of
the spread is this PC's noise (the same build's runs vary by 230 ms). LCP is about 2.0 s on both
sides on this PC (one 3-run set of master measured 2.018 s, just over its budget), so it is near
its line before and after alike.

## 5. Browsers

- **Chrome 140 and Edge 157 (headless, Windows)**: everything above; the full contrast run, the
  motion check, the pictures and Lighthouse in Chrome; the motion check and the download and home
  pages' contrast again in Edge, all passing.
- **Firefox**: no `animation-timeline` yet, so no reveals (content at rest, by `@supports`);
  View Transitions from 144; everything else is plain CSS transitions and animations.
- **Safari**: scroll-driven animations from 26, View Transitions from 18; `inset`, `:is()`,
  `isolation` all supported. Not run here (no Mac): judged by the features' support tables.

## 6. What only the owner can judge

- Whether the light theme's page heads (white, dot grid, deep blue — like the home's hero)
  are the look he wants for the download, unit and Measured pages, or whether he would rather
  keep them lapis in both themes (then the bar over them needs its light words back).
- The motion's amount: the hero's light (11 s), the spark on the rules (9 s), the size of the
  rise (26 px) and of the card lift (5 px) are single numbers in `motion.css`.
- The pictures: `C:\projects\daw-fx\compartir\web\before\`, `after\` and `motion\`.

## 7. The release shown on the download page

The download page's version, size, date and SHA-256 come from `web/assets/release.json`, read
at build time (`build.rs`). The committed file still says beta.13 (it was last refreshed in
round 39); the deployed site is current because `site.yml` runs
`python3 tools/release_json.py assets/release.json` (GitHub's API, with the workflow's token)
before `trunk build`, keeping the committed file only if GitHub cannot be reached. This branch
does not change `release.json`: the pictures in `compartir\web\` were made from a local
build with the current facts written into the working copy (beta.14 · 186 MB · 2026-11-11), and
the file was restored afterwards.

## 8. Files

- `web/style/frame.css`, `content.css`, `base.css` — the contrast fixes.
- `web/style/motion.css` (new, inlined last by `index.html`), `web/src/app.rs` (`is-arriving`),
  `web/src/theme.rs` (the cross-fade), `web/src/main.rs` (`download_answers`),
  `web/Cargo.toml` (web-sys `Event`, `EventTarget`, already in the build through Yew).
- `web/tools/cdp.mjs`, `contrast.mjs`, `motion.mjs`, `shoot.mjs` (new), `web/README.md`,
  `.github/workflows/site-budget.yml` (a "Contrast and motion" step, about 2.5 minutes).
