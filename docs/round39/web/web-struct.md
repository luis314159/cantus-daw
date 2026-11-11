# web-struct — the site's frame (round 39)

Builder `web-struct`, branch `r39/web-struct` (from `r39/web`), with `r39/web-extra` and
`r39/web-content` merged in. Plan items: P1-1, P1-2, P1-3, P1-7 and the footer (P2-5) of
`analysis.md` §4; then the owner's two colour changes (gold accents, distinct bands).

## Pre-rendering (P1-1)

- **Approach: full pre-render, then a fresh client render (no hydration).** The site's own
  program runs natively with the `ssr` feature after `trunk build`
  (`cargo run --features ssr -- prerender dist`, `src/prerender.rs`). Yew's `ServerRenderer`
  writes every route in both languages (86 pages) into `dist/<path>/index.html`, each with its
  own head (`src/head.rs`). It also writes `sitemap.xml`.
- **Why not hydration.** Yew 0.21's hydration panics on any tag mismatch, and the components
  read the browser in their first render: the OS, the theme, the release shelf. In the browser,
  `main.rs` clears `#app` and renders afresh. Yew renders in a microtask, so the swap lands
  before the next paint.
- **The app starts after the first picture is painted.** The prerender step rewrites Trunk's
  loader so the WebAssembly loads only after the hero's picture is the largest paint, or after
  something larger has painted, or after 2.5 s. It also drops the wasm preload. Without this,
  the 270 KB of WebAssembly competed with the picture on a phone.
- **Supporting changes for mobile LCP:**
  - the hero gets 960 and 1280 px variants through `srcset`;
  - the four WOFF2 fonts lose their hinting: 97 → 54 KB;
  - the bands below the first screen use `content-visibility: auto` until the app runs.

**Measured** with `tools/web-budget.ps1` (Lighthouse CI 0.15.1, mobile preset, 3 runs, median,
local gzip server):

| | Perf | A11y | BP | SEO | FCP | LCP | First visit |
|---|---|---|---|---|---|---|---|
| r39/web, the review (analysis §1) | 71 | 91 | 100 | 100 | 3.7 s | 5.6 s | 8 195 KiB |
| the merged content, no pre-render (Trunk only) | 83 | 100 | 96 | 100 | 1.36 s | 4.73 s | 798 KiB |
| this branch | **98–99** | **100** | **100** | **100** | 1.2 s | **2.03 s** (1.97–2.11) | 812 KiB |

- The text's observed paint is about 0.45 s. The simulated LCP sits on the 2.0 s line: the
  budget's LCP assertion passes on some runs and not on others.
- **The WebAssembly budget passes now.** Before: 283.7 KB gzipped (opt-level "s").
  - opt-level "z": 251.7 KB.
  - wasm-opt `--converge --strip-debug --strip-producers`: 251.5 KB.
  - twiggy showed two data segments of 88 and 63 KB. They held the text, and the units' long
    text was the largest part (58 KB raw).
  - The units' `what` and `features` are now compiled only natively: the `long!` macro
    (`main.rs`) leaves them out of the WebAssembly.
  - The prerender writes them to `/unit-text.json` (53 KB, 20 KB gzipped). `main.rs` fetches it
    before the app's first render, so that render matches the static page.
  - Result: **231.5 KB gzipped** (620 931 B raw). LCP and the scores did not change.
  - `trunk serve` without the prerender step shows the units without their long text.

## Addresses (P1-2, P1-3)

- **Routes.** `/` and `/es/`, each with `download/`, `instruments/`, `effects/`,
  `instruments/<name>/` (9), `effects/<name>/` (25), `measured/`, `studio/`, `ai/`, `roadmap/`
  and `learn/`.
- **One list for everything.** `route.rs` `Page::all()` feeds the pre-renderer, the sitemap and
  the tests.
- **Moving between pages.** `router.rs` turns a plain left click on a link to one of the site's
  pages into a history entry. The back and forward buttons return where the visitor was
  (`scrollRestoration = manual`). Links stay plain `<a href>`, so they work before the app runs.
- **Language.**
  - `/` sends a visitor to `/es/` before the first paint if they chose Spanish, or if their
    browser speaks Spanish and they never chose.
  - The switch goes to the same page in the other language.
  - Each head has the hreflang alternates en, es and x-default, plus `og:locale` (`en_US` or
    `es_MX`) and its alternate.
  - The Spanish share image is `og-es.png`, from `tools/og-es.html`.
- **The home's nine chapters:**
  1. hero and proof, with the rack;
  2. tour;
  3. instruments and effects;
  4. listen;
  5. measured;
  6. free and open;
  7. download;
  8. FAQ (the first two answers open);
  9. the footer.
- **What moved off the home:**
  - recording mode, the studio, the session view, the score, the engine and the features list
    → `/studio/`;
  - AI → `/ai/`;
  - roadmap → `/roadmap/`;
  - support → `/download/#support`.
- **404.** The static `assets/404.html` stays (Cloudflare `404-page`).

## Head, footer, wiring (P1-7, P2-5)

- **JSON-LD.** Every page carries a `SoftwareApplication`: price 0, the systems, and the version
  from the workspace. It has no rating. Each page has its own title and description. The head
  links the releases feed (`application/atom+xml`).
- **Sitemap.** It lists all 86 pages with `lastmod` (the commit's date in site.yml) and their
  xhtml alternates, plus the manuals and the press kit.
- **Footer.** Product, Learn, Community and Legal columns, then the language links, a theme
  switch shared with the nav (`theme::ThemeCtx`), the marks and the creed.
- **What the other branches provide:**
  - web-content: nav, hero, Groove, Spotlight, `DownloadPage`, `MeasuredPage` and
    `MeasuredSection`, `UnitPage` and `UnitIndex`;
  - web-extra: `proof::Proof` (stays hidden below its floor) and `pages::learn::Learn`.
- **Changes in their files:**
  - `nav::href` now adds the trailing slash;
  - the nav uses the shared theme;
  - dead code was cleared: `all_paths`, `release::has`, an unused field;
  - the /download page links its own `#support`.
- **site.yml** adds a "Pre-render every page" step and web-extra's data step, and checks that
  the pages exist. `site-budget.yml` and `web-budget.ps1` pre-render too.

## Colours (the owner)

- **Gold accents.** The rack's lit cells, Play, "Hear this song", the console meters, the song
  and A/B bars and the AI glow are now #03's metal (`--metal`, `--metal-key`, `--metal-bar`).
  The text on gold is `#1a1406`, which passes AA on every stop.
- **What did not change:**
  - Lapis stays for links and the primary button.
  - The scorecard's "Lost" keeps its own vermilion (`--lose`): a loss is not gilded, and the
    coordinator confirmed it. The memory card's numbers are untouched while they are
    re-measured.
- **The hero in the light theme** (the owner) is white with a lapis dot grid. Its words are
  deep blue: the headline is `#1b2c7a` (12.6:1), the lede `#24398f` and the platform line
  `#3a4c9a`. The eyebrow and the headline's dot use the darker gold `#8a6212` (5.1:1). The
  primary button is lapis with white text; the secondary is outlined in deep blue. The picture
  sits in its frame with a soft shadow. The bar over it reads the same until the page scrolls.
  The dark theme keeps the lapis night. Lighthouse accessibility: 100.
- **Bands.**
  - Paper `#f5f6f9` alternates with ivory `#f4eedf`.
  - The proof, listen, engine and AI bands are deep navy `#0e1630`.
  - The dark theme uses Slate `#1d2329`, warm `#29241b` and navy `#0b1226`.
  - A gold hairline marks each edge.
  - Accessibility stays 100 (Lighthouse runs it on the light theme).

## Phones

The header was checked with phone emulation at 360, 390 and 414 px: dark and light, EN and ES,
on the home and on /es/studio/. The bar takes its own row (0–73 px), with the logo at the left
and the menu at the right edge. The eyebrow starts at 105 px. The overlap seen in the first
`home-mobile-light.jpg` came from the screenshot script, which had set the bar to
`position: absolute` to stitch the tiles. The script now keeps the bar in its row.

## Screenshots

`docs/round39/web/struct/`:
- `home-desktop-light.jpg` (1440 px, full page);
- `home-mobile-light.jpg` (390 px, emulated phone with DPR 2, stitched from what a visitor
  scrolls through);
- `home-es-desktop-dark.jpg`;
- `home-es-mobile-light.jpg` (390 px, full page) and `phone-414-en-light.jpg`;
- `phone-360-en-dark.jpg`, `phone-390-en-dark.jpg`, `phone-414-en-dark.jpg`,
  `phone-360-es-light.jpg`, `phone-390-es-studio-light.jpg`: the first screen on a phone.

## Not verified

- Cloudflare itself (Brotli, the trailing-slash redirects, `404-page` under `/es/`).
- site.yml on the runner: the native build of the `ssr` feature on Ubuntu has not run there.
- A real phone and a screen reader.
- Lighthouse on the dark theme.
