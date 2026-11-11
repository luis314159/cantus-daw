# The website — review and plan (round 39, web-review, 2026-11-09)

The owner (2026-11-09): "Creo que la página web aún puede mejorar mucho, analiza cómo hacerla
más chida, profesional y pulida para nuestro producto que aspira a la excelencia." This
reviews the site as built on r39/web at b9fd3bc5 ("Lapis stage", logo #03) without changing
its code: findings with evidence, a plan by impact and effort, and five page mocks (§5).

**Verdict.** The content is honest and rich: real program pictures, a playable rack in the hero,
A/B audio for seven effects, a measured scorecard. What keeps the site from reading first-rank
is structure, not finish:
- one 26 000-px scroll of 18 sections;
- a hero that sells the logo, not the program;
- all text arriving after a 589 KB WASM;
- 8.2 MB on the first visit, 7.5 MB of it one PNG;
- no menu on phones;
- a download flow that asks for money before giving the download and never mentions SmartScreen.
All fixable while keeping Lapis stage, #03 and the honesty rule.

## 1. Method and evidence

- **Site audit.** web/dist served locally (npx serve, gzip) and Lighthouse 12.8.2 in headless
  Edge, mobile and desktop presets. The raw reports are in
  `F:\projects\daw-r39\downloads\web-review\lh-*.json` (not committed). Cloudflare serves
  Brotli, so transfers would be a little smaller; the weight and the render path are the same.
- **Screenshots.** This round's built-*.jpg, sliced and read section by section.
- **Reference sites.** Fifteen captured with headless Edge at 1440×5200 and kept in
  `downloads\web-review\ref\` (not committed): Ableton, Bitwig, FL Studio, FabFilter, Arturia,
  u-he, NI, REAPER, Vital, Linear, Raycast, Vercel, Splice.
  - Logic renders on scroll and came out blank, so it was read as text.
  - Studio One now redirects to fender.com behind Cloudflare ("Fender Studio Pro").
  - Arc is being sunset.
  - Logic's, Ableton's and Bitwig's page structures were also read as text.

**Lighthouse.**

| Preset | Perf | A11y | BP | SEO | FCP | LCP | TBT | CLS | Weight |
|---|---|---|---|---|---|---|---|---|---|
| Mobile | **71** | 91 | 100 | 100 | 3.7 s | **5.6 s** | 70 ms | 0 | **8 195 KiB** |
| Desktop | 96 | 94 | 100 | 100 | 0.8 s | 1.3 s | 0 | 0.031 | 8 195 KiB |

- The mobile LCP element is the hero's sub-paragraph: text, but it waits for the WASM to be
  fetched, compiled and run.
- SEO 100 is misleading: Lighthouse runs JavaScript. Crawlers and link unfurlers that don't run
  it see only `<noscript>`.

**Sizes.**

| What | Size | Note |
|---|---|---|
| wasm | 589 KB raw, 218 KB gzip | every string in both languages |
| js | 45 KB | |
| CSS | 69 KB in 6 render-blocking files | about 0.99 s to recover on mobile |
| fonts | 426 KB, 4 TTF | a WOFF2 subset is ≈ 4 × 20–25 KB |
| **app-mixer-xl.png** | **7 818 KB** | loaded eagerly as the tour's CSS background; lossless WebP 173 KB (−97.8 %), q90 155 KB |
| assets/units | 27 MB on disk | WebP and lazy: fine |
| assets/dance | 4.0 MB | copied to dist but not referenced by any src/*.rs; check before cutting |

**Accessibility (axe).**
- Rack cells are 18.8×28 px, under the 24×24 minimum.
- The demo songs' `.song-bar` progress bars have no accessible name.
- Light-theme contrast fails: `.window-bar span` 3.09, `.tab-n` 2.62, `.ways span` 4.23.
- The brand's `aria-label` is "Cantus" against the visible "Cantus™".
- "White on #f4f6f8" for the brand is likely a false positive (a transparent bar over lapis);
  verify it.
- Not caught by the tool:
  - below 52 rem the section links are `display:none` and there is no menu, so a 32 553-px
    phone page has no way to jump;
  - the rack's focus ring by keyboard still has to be checked.

## 2. What the best do

| Pattern | Who | Cantus today |
|---|---|---|
| **The product is the hero** | Bitwig (full GUI and play); FL (UI in a device frame); FabFilter (Pro-Q 4); REAPER (the arrange view under the logo); Linear and Raycast (the app is the hero) | the gilded C fills the right half; the program first shows at about 1 300 px on desktop and 1 900 px on a phone |
| **Motion and sound up front** | Ableton (highlights film, labelled looping MP4s); Bitwig (video); Arturia (video, then a preset player under the synth) | a playable rack (unique — keep it), but the program never moves |
| **A short home, deep sub-pages** | Ableton Live (6 sections and a jump menu); Logic (13 gateway chapters); Bitwig (card blocks that link out) | 18 full sections; only the units and the manual have pages |
| **One idea per section, one line** | Linear, Raycast, Logic ("Sound thinking.") | up to 5 columns of small text and asides (about 260 words for the recording mode) |
| **Hear it where you see it** | Arturia, Splice, Ableton | the large view and its presets are in a modal; the effects' A/B is excellent |
| **Social proof** | FabFilter (a Pensado quote, awards); Bitwig (awards, artists); FL (album wall, 4.7★) | none — see P2-4 for honest options |
| **A clear download path** | Bitwig and Ableton ("Try for free", repeated); Raycast (a download and a command) | Download jumps to section 16 of 18, after Support |
| **Trust at download** | Raycast (signed, notarised); REAPER (sizes, versions) | hashes only on GitHub; the Windows installer is unsigned but its card never says so (the macOS card does explain Gatekeeper) |
| **Per-language URLs** | Ableton (/en/, /de/), NI, Apple | a client-side switch on one URL, so Spanish is invisible to search |
| **A real footer** | NI, Splice, Ableton | one line and the motto |

Cantus has three things no one else has: a playable instrument in the hero, A/B audio for its
effects, and a published comparison that includes its losses. No commercial DAW site
publishes a row it loses. That is the differentiator, and it is buried at section 13 of 18.

## 3. Audit by area

- **Information architecture (high).**
  - The page is 26 191 px on desktop (about 29 screens) and 32 553 px on a phone (about 39).
  - The order is Hero, Tour, Recording mode, Units, Effects, Listen, Studio, Features, Session,
    AI, Score, Engine, Measured, Free, **Support**, **Get**, Ahead/FAQ, More: proof and action
    come last, and Support comes before Get.
  - "Features" lists everything again, after six sections that already showed features.
- **Hero (high).**
  - Keep "Finish the song.".
  - The C takes the right half, and the nav already shows it.
  - The licence line (GPL/MPL) sits in the most valuable 100 px.
  - On a phone the primary button says "Download the beta" for a desktop-only program;
    `platform.rs` already detects `Os::Mobile`, but only the download card uses it.
  - On a phone the lapis edge (the gold hairline) cuts through the rack between the hi-hat and
    bass rows and reads as a glitch, and the cells are 19 px.
- **Copy (medium).**
  - It is honest, but written in design-note voice: "baked", "streams", "round trip",
    "faces", "inspector".
  - Some captions defend themselves ("The CPU figures in this picture are staged…"); such
    caveats belong in small print beside the number.
  - The counts disagree: "Nine instruments" in the units against "Ten instruments" in the
    roadmap (which includes Firmus). Choose one convention, or "nine, plus Firmus".
  - Spanish leftovers: "Channel rack — pruébalo", and the tour picture is English UI on the ES
    page (app-mixer-xl.png has no -es twin).
  - Open the first two FAQ answers by default.
- **Imagery and motion (high).**
  - The real program is right, but warm-brown Slate UI on dark Slate sections inside a lapis
    frame makes three dark planes with little contrast.
  - Give the pictures size and air: full-bleed on desktop, and a readable detail on phones
    instead of a whole window in 330 px.
  - The program is never shown moving (playhead, meters, automation).
- **Interaction (medium).**
  - Good: the rack, the A/B player, the units modal, the download card that knows the OS.
  - Missing: a mobile menu, a real download page, deep links per instrument
    (`/instruments/unda`).
- **Performance (high on mobile).**
  - The measures are in §1.
  - `_headers` gives `immutable` to only 3 of the 6 hashed CSS files.
  - `/units/*`, `/studio/*`, `/session/*`, `/ai/*` and `/score/*` have no cache rule.
- **SEO and sharing (high).**
  - There is no content in the HTML, and one URL serves two languages with no hreflang.
  - There is no JSON-LD SoftwareApplication (price 0, OS, version, downloadUrl).
  - The sitemap has no lastmod and no unit pages.
  - There is one English og:image and no og:locale:alternate.
- **Download and trust (high).**
  - The GitHub `releases/latest` link is good, but the page shows no size, date or hash, says
    nothing of SmartScreen, and gives no first steps.
  - The browser asks `api.github.com` directly. The privacy note should name that third-party
    request, and its rate limit (60 an hour per IP) will be hit behind a shared NAT such as a
    school or an office: cache it at build time or in a worker.
- **Footer (low).**
  - Four columns: Product, Learn (manual, design notes, press), Community (GitHub, issues,
    support), Legal (privacy, licence, trademarks, notices).
  - The language and theme switches.

## 4. Plan (impact × effort)

Effort: S ≈ a day, M ≈ 2–4 days, L ≈ a week or more. Every number shown comes from
`docs/scorecard/`.

**P0 — first.**
1. **P0-1 (S).** Re-encode app-mixer-xl.png as lossless WebP, add its ES twin, and lazy-load
   it. The first visit drops by about 7.6 MB (8.2 → ~0.7 MB).
2. **P0-2 (S).** WOFF2 subset fonts, and preload the hero's two weights (about −330 KB).
3. **P0-3 (S).** One CSS file with the critical part inlined; `immutable` for every hashed
   file; one day for each picture folder (about 1 s on mobile).
4. **P0-4 (M).** A product-first hero (Mock 1):
   - the real program full-width on the lapis;
   - "Hear this song" plays Neon Tide;
   - a measured proof strip of three wins and one loss;
   - the C moves to the nav and the favicon, the licence line to the footer and the download
     page.
5. **P0-5 (M).** Phones (Mocks 1 and 5):
   - a burger menu that opens a sheet;
   - "Send the link to my computer" on `Os::Mobile` (Web Share, else copy);
   - rack cells of at least 24 px (8 steps × 2, or a scroller);
   - the lapis edge moved off the rack.
6. **P0-6 (S).** Support after Get, and the SmartScreen truth in the Windows card ("not signed
   yet: More info → Run anyway").
7. **P0-7 (S).** Accessibility, from 91 to 100: name the progress bars, fix the three contrast
   failures, match the brand's name, and give the rack a focus ring.

**P1 — structure.**
1. **P1-1 (L).** Pre-render static HTML with Yew's `ServerRenderer` at build time (an xtask or
   an ssr bin) for `/` and `/es/`, then hydrate (smaller option: the hero and headings only).
   This brings LCP to about 1.5 s on slow 4G and lets crawlers see the content.
2. **P1-2 (M).** `/` and `/es/` with hreflang, og:locale, a Spanish og.png, and a language
   switch that navigates.
3. **P1-3 (L).** A home of at most nine chapters:
   - Hero and proof, Tour (record, mix, perform, write), Instruments & effects, Listen,
     Measured, Free & open, Download, FAQ, Footer;
   - sub-pages: /download, /instruments/<name>, /effects/<name>, /measured, /studio, /ai,
     /roadmap;
   - about half the scroll (26k → ~12k px).
4. **P1-4 (M).** A /download page (Mock 4):
   - the detected OS's build first, with its version, size and date fetched at build time;
   - its SHA-256 with a copy button and the Get-FileHash / sha256sum line;
   - the SmartScreen steps and three first steps into the manual;
   - the other systems and the requirements;
   - a quiet "it stays free" last.
5. **P1-5 (M).** An instrument spotlight (Mock 2): one large picture with its four presets
   playable under it (from `units_sounds.rs`), a shelf of the others, deep links.
6. **P1-6 (M).** The measurements, shown with confidence (Mock 3):
   - paired bars, like against like;
   - the lost row drawn as plainly as the wins;
   - the feature-matrix summary (22 rows where a leader has more, 3 rows no commercial leader
     has) and a link to all 97.
7. **P1-7 (S).** JSON-LD SoftwareApplication, a sitemap with lastmod and the new pages, and a
   title and description per page.

**P2 — polish and proof.**
1. **P2-1 (M).** A 10–12 s muted loop of Cantus playing Neon Tide:
   - AV1/WebM of at most 1.5 MB, with today's PNG as poster;
   - it respects `prefers-reduced-motion`, and sound plays only on "Hear this song";
   - it needs a capture (OBS, or a shoot example and an encoder — ffmpeg is not installed
     here).
2. **P2-2 (M).** An EN/ES copy pass: one idea per section, caveats as small print, one count
   convention, the ES leftovers fixed.
3. **P2-3 (M).** Vary the sections' layouts (full-bleed, split, grid, data); keep the gold
   diamonds for chapter starts.
4. **P2-4 (S–M).** Honest social proof, never invented; an empty slot stays hidden:
   - GitHub stars and release downloads fetched at build time;
   - "made with Cantus" songs from beta users, with their permission;
   - press quotes once they exist.
5. **P2-5 (S).** The real footer.
6. **P2-6 (S).** Phone crops that show a readable detail.

**P3 — later.**
- A performance budget for the site in CI (Lighthouse CI on the trunk output): first visit
  ≤ 1 MB, mobile LCP ≤ 2.0 s, CLS ≤ 0.02, accessibility 100, WASM ≤ 250 KB gzip.
- Video tutorials and a /learn hub.
- A release RSS feed.

## 5. Mocks (docs/round39/web/review/)

How they were made:
- HTML sources in `review/src/` (m*.html, mock.css), built from the site's tokens, Barlow,
  logo #03 and the program's real pictures in web/assets.
- Rendered with headless Edge at 1440 and 390 px, dark and light, as JPEG. The phones are
  framed in an iframe, because headless Edge won't go narrower than about 500 px.
- They are not the site's code. Every number comes from `docs/scorecard/README.md` or
  `features.md`.

| Mock | Files | What it shows |
|---|---|---|
| 1 | m1-hero-{desktop,mobile}-{dark,light}.jpg | the headline; the program full-width with "Hear this song"; the proof strip (3 won, 1 lost); on phones, "Send the link to my computer" and a burger |
| 2 | m2-spotlight-… | Unda large, four presets playable under it, the shelf of nine, "Everything about Unda →" |
| 3 | m3-measured-… | paired bars: window ready, quiet and at rest won; memory lost (vermilion); the feature matrix 97 / 22 / 3 |
| 4 | m4-download-… | Cantus for Windows: the SmartScreen truth, the hash, no telemetry; the first ten minutes; the systems; the requirements; support last |
| 5 | m5-nav-… | desktop: the Tour mega-menu of chapters with real pictures; phone: a sheet with the chapters, language, theme and "send the link" |

**Not verified:** Lighthouse ran against a local server, not Cloudflare (Brotli, HTTP/3, the
real `_headers`); no screen-reader pass and no real phone; Studio One and Logic could not be
captured.
