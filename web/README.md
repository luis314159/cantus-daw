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
The type is Barlow Semi Condensed (SIL OFL, licence in `assets/fonts/`), the app's own (round 39).
The look follows logo #03 (round 39): Slate surfaces, lapis for what you press, gold only in
fine marks and rules; the tokens are at the top of `style/base.css`, and `style/lapis.css` and
`style/stage.css` dress the sections, the lapis tiles and the footer.

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
and `assets/press/cantus-logo-1024.png`; `tools/touch.html` and `tools/fav32.html` of
`assets/apple-touch-icon.png` and `assets/favicon-32.png`. Re-render them with headless Edge or
Chrome (`--window-size=1200,760`, then crop to 1200×630; the icons with
`--default-background-color=00000000`) when the tagline or the logo changes. The logo is #03:
`assets/press/cantus-logo.svg` is `crates/cantus-ui/assets/logos/03.svg`; `assets/logo.svg` (the
bars' mark) and `assets/favicon.svg` are its 48- and 32-px small masters.

## The effects' with/without excerpts

`assets/fx/audio/*-{without,with}.mp3` are the same excerpt of a demo song with every slot of
one effect bypassed and as shipped (about 4 MB in all). For each effect the program picks, among
the demos that use it, the one where it changes the sound the most and that stays aligned in time
(a pair shifted by even a few milliseconds would flam when switched). To re-render:

```text
cargo run --release -p cantus-ui --example render_fx_ab -- web/assets/fx/audio          # all seven
cargo run --release -p cantus-ui --example render_fx_ab -- web/assets/fx/audio eq,gate   # some
```

Then write the chosen song's name into the effect's `ab:` in `src/units_data.rs` (the run
prints it); an effect with an `ab:` gets the player on its page and in *Hear what they do*.

## The instruments' and effects' pages

`src/units_data.rs` holds every native unit's name, one-line pitch, what it does, its standout
features, presets and pictures (written from the design notes in `docs/plugins/` and
`docs/instruments/`: only what is built, numbers only as measured there); `src/units.rs` draws
the cards and the page a click opens. The pictures are the program's own windows:

```text
cargo run --release -p cantus-ui --example shoot_skins -- target/shots/skins
python tools/units_pictures.py ../target/shots/skins      # Pillow; oxipng if installed
cargo run --release -p cantus-ui --example shoot_skins -- target/shots/skins-es --es
python tools/units_pictures.py ../target/shots/skins-es --es   # the Spanish page's
```

It writes `assets/units/` (the cards and the large pictures, dark and light; `-es` for the
Spanish page, whose pictures show the editors in Spanish) and the README's `docs/images/units/`.

## The recording mode's and the studio's pictures

`assets/studio/record-*` and `mix-snap-*` (each in EN and ES) come from
`cargo run --release -p cantus-ui --example shoot_showcase -- <dir>` (1920 × 1080, Night and
Paper, EN and ES), converted to WebP at quality 82; the editor's, external gear's, the
surfaces' and ARA's (`-es` for Spanish, ARA's has one) are crops of the round-36 shots in `docs/round36/{editor,external,ara}/`.

## Publishing

`.github/workflows/site.yml` builds the site and publishes it, with `wrangler deploy` and
`web/wrangler.jsonc`, to the Cloudflare Worker that serves cantusdaw.com (static files from
`dist/`; the Worker is named `quiet-bread-21e8` in the dashboard) on every push to `master` that
touches `web/`. It needs the repository secrets `CLOUDFLARE_API_TOKEN` (permission *Account >
Workers Scripts > Edit*) and `CLOUDFLARE_ACCOUNT_ID`. By hand, upload `web/dist` from the
dashboard (*Workers & Pages* > the Worker > *Deployments*).
