# web-extra — social proof, release feed, /learn, the site budget (round 39)

Builder `web-extra`, branch `r39/web-extra` (from `r39/web`). Only new files; `web-struct`
wires them. Plan items: P2-4, P3 (feed, /learn, budget) of `analysis.md` §4.

## What was built

| Piece | Files | Wiring (for web-struct) |
|---|---|---|
| GitHub stars + release downloads, and the Atom feed | `web/tools/site_data.py` | site.yml, after `trunk build`: `python3 web/tools/site_data.py --out web/dist` with `env: GITHUB_TOKEN: ${{ github.token }}`. Writes `stats.json` and `releases.xml`; when GitHub is unreachable it writes neither and exits 0. |
| Proof strip | `web/src/proof.rs` (`proof::Proof`, `proof::MadeWith`) | `mod proof;` in main.rs, `<proof::Proof />` where the home wants it. Fetches `/stats.json`; renders nothing until it holds a dated count at or above `STARS_FROM` (50) / `DOWNLOADS_FROM` (500). The "Made with Cantus" row renders only when `MADE_WITH` has songs. |
| Feed link | — | in index.html `<head>`: `<link rel="alternate" type="application/atom+xml" title="Cantus releases" href="/releases.xml" />` |
| /learn hub | `web/src/pages/mod.rs`, `web/src/pages/learn.rs` (`pages::learn::Learn`) | `mod pages;` in main.rs and the `/learn` (and `/es/learn`) route. Links are absolute (`/manual/en/…`). Groups: Start here, Record, Mix, Instruments, Effects, AI, The dancer, Reference. |
| Styles | `web/style/extra.css` | `<link data-trunk rel="css" href="style/extra.css" />`, or merged into the single CSS of P0-3 |
| Site budget | `.github/workflows/site-budget.yml`, `web/lighthouserc.json`, `web/tools/wasm_budget.py`, `tools/web-budget.ps1` | nothing: the workflow runs on its own (public repo only), on push/PR touching web/ |

"The dancer" is an eighth group beyond the seven in the brief: four manual pages (Saltator,
the stage, the video, importing a character) belong to none of the seven.

## Tests and what was run

- `cargo test` in web/ (with the modules declared temporarily): 11 pass. `learn`'s tests read
  the manual: every linked page and anchor exists in both languages, every manual page is
  reached, every docs/ folder exists. `proof`'s tests hold that nothing shows without a dated
  number over the floor.
- `cargo check --target wasm32-unknown-unknown` with both components rendered in `App`: no
  warnings. Unwired, they warn only as unused.
- `site_data.py` run once against GitHub on 2026-11-10 (UTC): 4 stars, 109 downloads over 14
  releases, 14 feed entries, and the XML parses. At those numbers the strip stays hidden.
- `tools/web-budget.ps1` run once on the current site. WASM: 218 839 B gzip (passes). CLS
  passes. Over budget, as the analysis measured:
  - first visit 8 277 452 B against 1 048 576;
  - LCP 5.4 s against 2.0 s;
  - accessibility 0.91 against 1.
  The workflow will fail until P0-1/P0-7/P1-1 land. That is intended.

Not verified: the workflow itself on GitHub's runner (it needs the public repo).
