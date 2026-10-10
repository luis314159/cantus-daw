# cantusdaw.com

The Cantus website: a Yew (Rust → WebAssembly) single-page app. See
[ADR 0034](../docs/adr/0034-website.md).

```text
rustup target add wasm32-unknown-unknown
# Trunk: https://github.com/trunk-rs/trunk/releases (a prebuilt binary is enough)
cd web
trunk serve --release        # http://127.0.0.1:8080
trunk build --release        # static site in web/dist/
```

If Trunk says `invalid value '1' for '--no-color'`, unset the `NO_COLOR` environment variable.
`src/top.rs` holds the version and the download links; `src/lang.rs` the EN/ES switch.
Fonts are Syne and Instrument Sans (SIL OFL, licences in `assets/fonts/`).

## The demo songs on the page

`assets/demos/*.mp3` are excerpts of the six demo songs, rendered by the program itself
(about 2 MB in all). To re-render them after the songs change:

```text
cargo run --release -p cantus-ui --example render_demos -- web/assets/demos
```

## The manual, the press kit and the rest of the static pages

`powershell -File build-manual.ps1` runs `cargo run -p xtask -- manual`, dresses the two
generated pages (EN/ES) in the site's look and writes them to `assets/manual/`; `trunk build`
copies it into `dist/manual/`. **The generated pages are committed** (the deploy workflow does not
build the manual): run the script and commit `assets/manual/` whenever `docs/manual/` changes.
Run it **before** `trunk build`: the press kit (`assets/press/`) and the 404 page use the `manual/manual.css` it copies.
`tools/og.html` and `tools/logo.html` are the sources of `assets/og.png` (the share image)
and `assets/press/cantus-logo-1024.png`; re-render them with headless Chrome
(`--window-size=1200,760`, then crop to 1200×630) when the tagline or the logo changes.

## The effects' with/without excerpts

`assets/fx/audio/*-{without,with}.mp3` are the same excerpt of a demo song with every slot of
one effect bypassed and as shipped (about 4 MB in all). For each effect the program picks, among
the demos that use it, the one where it changes the sound the most and that stays aligned in time
(a pair shifted by even a few milliseconds would flam when switched). To re-render:

```text
cargo run --release -p cantus-ui --example render_fx_ab -- web/assets/fx/audio          # all seven
cargo run --release -p cantus-ui --example render_fx_ab -- web/assets/fx/audio eq,gate   # some
```

Then write the chosen song's name into `song:` in `src/fx.rs` (the run prints it).

## Publishing

`.github/workflows/site.yml` builds the site and publishes it to Cloudflare Pages (project
`cantus`, production branch `main`) on every push to `master` that touches `web/`. It needs the
repository secrets `CLOUDFLARE_API_TOKEN` (permission *Account > Cloudflare Pages > Edit*) and
`CLOUDFLARE_ACCOUNT_ID`. By hand, upload `web/dist` (or a zip of its contents) from the
dashboard.
