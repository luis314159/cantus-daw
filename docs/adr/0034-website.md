# ADR 0034 — The website: Yew, a static single-page app, outside the DAW's workspace

**Status:** accepted (2026-10-10) · **Owner's request:** a landing page for cantusdaw.com
(and cantusdaw.studio) with manuals, information and downloads, "in Yew"
(`docs/BACKLOG-excellence.md` §6a, `docs/monetize.md` §7).

## Context
The beta needs a front door: what Cantus is, a download, the manual, the values (free, no
telemetry, GPL), and later sound packs and donations. The owner asked for Yew, so the site is
Rust like the rest, and its interactive piece (a playable step rack) shares our language.

## Decision
- `web/` is a **separate Cargo workspace** (`[workspace]` in `web/Cargo.toml`): the DAW's
  `cargo build`/`test` never builds it, `Cargo.lock` and `THIRD-PARTY-NOTICES.md` of the DAW
  are untouched, and the site's dependencies (yew, wasm-bindgen, web-sys) stay out of the
  application.
- **Yew 0.21, client-side rendered**, built with **Trunk** to a static `web/dist/` (about
  270 KB of WebAssembly): any static host or CDN serves it; no server, no cookies, no
  analytics (PRIVACY.md holds).
- **No external requests**: fonts (Syne and Instrument Sans, SIL OFL; licences beside them in
  `web/assets/fonts/`) and the logo are self-hosted; the rack's sound is synthesised with Web
  Audio in the browser (`web/src/audio.rs`), nothing is downloaded.
- **English and Spanish** switch live (`web/src/lang.rs`), remembered in `localStorage`
  (a language choice only), defaulting to the browser's language.
- The palette is the logo's own (copper, amber, verdigris on espresso), the look is the
  page's, not the DAW's; the product screenshots are the real program.

## Consequences
- Build: `rustup target add wasm32-unknown-unknown`, Trunk (a prebuilt binary is enough;
  note Trunk refuses `NO_COLOR=1` — unset it), then `trunk build --release` in `web/`, or
  `trunk serve` to develop. `docs/DEVELOPING.md` lists it.
- Content that can drift from the product (feature claims, the download link and version in
  `web/src/top.rs`) is reviewed at each release; claims must be ones the tests or STATUS hold.
- Open: the download links point to GitHub Releases until `cantusdaw.com/downloads` serves
  them with their hashes (§6a); the manual page, sound-pack pages and donations come later.
