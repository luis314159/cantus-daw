# Round 22 — beta-docs: notices, privacy, About, the manual, templates, notes, the version

Builder: **beta-docs**, branch `r22/beta-docs` (from `c8bc9f2`). `docs/BETA.md` blocker 6
(legal: notices, privacy, linked from About) and the recommended items (quick-start manual,
feedback templates, version and release notes).

## What is done

### 1. `THIRD-PARTY-NOTICES.md`, generated and checked

- `cargo run -p xtask -- notices` writes it; `cargo run -p xtask -- notices --check` (and
  the test `notices_tests::the_notices_name_every_crate_a_release_ships`, so
  `cargo test --workspace` in CI) **fails when a crate appears without a notice, when a
  crate's licence is not one Cantus may ship, or when the file is stale**.
- `cargo about` is not installed and was not needed: the closure comes from
  `cargo metadata --locked --filter-platform x86_64-pc-windows-msvc` (the one released
  target), following normal and build edges (never dev ones) from the two things a
  release ships, `cantus` and `cantus-plugins`. **280 crates from crates.io, 27 of them
  build-only** (bindgen, cc, winresource… marked *build*).
- The texts are the crates' own licence files as published (read from cargo's registry,
  where `cargo metadata` unpacks them); each distinct text is printed once with the crates
  that carry it (166 texts). Copies of the standard Apache-2.0 terms inside a file are
  replaced by a reference to the one copy at the end, keeping every copyright and NOTICE
  line around them. 19 crates ship no licence file (egui's family, accesskit,
  clipboard-win…): they are listed with their declared licence and authors, and the
  standard texts (`LICENSE-APACHE`, `xtask/notices/MIT.txt`, `BSL-1.0.txt`) are printed.
- Allowed licences (`notices::ALLOWED`): MIT, Apache-2.0 (and WITH LLVM-exception),
  BSD-2/3, ISC, Zlib, 0BSD, Unlicense, BSL-1.0, CC0, MPL-2.0, Unicode-3.0/DFS-2016,
  CDLA-Permissive-2.0, and the fonts' OFL-1.1 and Ubuntu-font-1.0; an expression passes
  when every AND term and one OR side are allowed (`self_cell`'s `Apache-2.0 OR
  GPL-2.0-only` passes on Apache). Every crate today passes.
- The hand-written head (`xtask/notices/head.md`): Cantus itself and where its complete
  corresponding source is; the patched cpal/asio-sys (Apache-2.0, the §4(b) modification
  notice); the **Steinberg ASIO SDK 2.3.4 under its GPL-3.0 option** (the SDK's own
  licence words quoted, its download URL and pinned SHA-256); the **ASIO and VST trademark
  attribution** in the words the *Steinberg ASIO Usage Guidelines* §15 require ("ASIO is
  a registered trademark of Steinberg Media Technologies GmbH"); libm with musl's,
  FreeBSD's and Sun's notices (its `LICENSE.txt` is printed in full); the fonts (IBM Plex,
  OFL; egui's Ubuntu Light, Hack, Noto Emoji and emoji-icon-font with their texts); the
  Bible translations (the **RVR1960 notice word for word**, RV1909 and WEB public domain,
  WEB's trademark); the CC0 packs with credits; FlexASIO (MIT, downloaded on request) and
  ASIO4ALL (link only); NSIS (zlib, its LZMA under CPL-1.0); the Rust standard library.
- `xtask package` now ships `THIRD-PARTY-NOTICES.txt` and `PRIVACY.txt` beside
  `cantus.exe` (and the notices in the plug-ins zip).

### 2. `PRIVACY.md` (EN and ES)

No telemetry. Exactly what the update check sends (read from `crates/cantus-update`:
the URL, the four headers, no body; `User-Agent: cantus-update`; the ETag), when, what is
kept (`update.json`), how to turn it off; what the content manager downloads (only on
*Install*, from the manifest's hosts — `raw.githubusercontent.com`, `github.com` — with
`User-Agent: Cantus/<version>` and `Range`), the installer's optional FlexASIO step, pages
opened in the browser, third-party plug-ins, and every file that stays local.

### 3. About, redesigned in the kit (`views/about.rs`, `docs.rs`)

The stock egui window is gone. The kit's floating panel: the logo (the user's logo choice),
"Cantus" in the brand's gold, *Version 0.1.0-beta.1 · public beta*, what it is, *Created by
Luis Ángel Almazán —* [lalmazan.com], the motto (Colossians 3:23 from the shipped
translation of the reader's language, cited), **Licence** (the GPL in plain words, buttons
*Licence (GPL-3.0)*, *Third-party notices*, *Privacy*, the no-telemetry line), **Bible
verses at start** (each translation's credit; the RVR1960 notice word for word),
**Trademarks** (ASIO, VST, the SDK's GPL option), **Help and feedback** (*Manual*,
*Release notes*, *Report a bug*, *Source code*) and a line asking for beta reports. Esc or
✕ closes it. Strings in `locales/areas/about.{en,es}.txt`.

`docs::target` opens the copy installed beside `cantus.exe` when it is there
(`THIRD-PARTY-NOTICES.txt`, `PRIVACY.txt`, `LICENSE.txt`, `manual/<lang>/index.html`), else
the page on GitHub. **Help** now reads: *Manual*, *Release notes*, *Keyboard shortcuts* —
*Report a bug…*, *Suggest a feature…* — *Check for updates…*, *About Cantus*.

Render: `cargo run -p cantus-ui --example shoot -- <dir> 1 about`.

### 4. The quick-start manual (`docs/manual/{en,es}/README.md`, `docs/manual/img/`)

Fifteen chapters, the same in both languages (a test holds it): install, first-run audio
setup (ASIO/WASAPI, the chip and its popover, Get ASIO), the workspace, recording (arm,
input, monitoring, count-in, punch, loop takes, take modes, comping, MIDI and the typing
keyboard), editing (clips, the piano roll's seven tools, the channel rack, automation),
mixing (strips, inserts, sends, routing, device windows), mastering (the master chain, the
demos' order, Latitudo, Culmen's targets and *Learn drive*, Fervor), export (WAV, stems,
MIDI, DAWproject), the sample browser and sound packs, the device picker and plug-in
folders, projects/autosave/settings, the shortcut sheet, privacy and updates, **known
issues** (from `docs/BETA.md` and what the code says) and how to report a bug. Written
from the code (three read-only research passes over the views, keymap and locale files),
with the interface's own words in each language; screens rendered with the shoot
examples, downscaled and palettised (`docs/manual/img/`, < 4 MB in all, a test holds it).

`cargo run -p xtask -- manual` turns it into `target/manual/{en,es}/index.html` (a small
Markdown converter in `xtask/src/manual.rs`, no new crate; links to `PRIVACY.md` and the
notices become the installed copies), and `xtask package` installs it as
`manual\{en,es}\index.html`, which *Help ▸ Manual* opens in the interface's language.

### 5. GitHub templates (`.github/`)

Issue forms, bilingual: **bug report** (what happened, steps, how often, version, install
kind, Windows, CPU/RAM, audio interface, driver — WASAPI / ASIO own / FlexASIO / ASIO4ALL,
rate and buffer, third-party plug-ins, files), **hardware report** (interface, connection
and driver version, driver used, rate/buffer/round trip, playback, duplex recording,
dropouts after 10 min, hot-plug, loopback latency, system), **feature request** (the task,
the idea, area, weight); `config.yml` (no blank issues; links to the manual and privacy);
a **PR template** (tests, clippy/fmt, both locales, schema bump, notices regenerated,
screenshots, the licence by crate).

### 6. Release notes — `docs/releases/v0.1.0-beta.1.md`

What is in, install (unsigned: SmartScreen's *Run anyway*), known issues, how to give
feedback; English first (the updater shows the first 80 lines), then Spanish. The release
workflow already reads `docs/releases/<tag>.md`.

### 7. The version: `0.1.0-beta.1`

- `Cargo.toml` `[workspace.package] version` (every crate inherits it; `Cargo.lock`
  follows). The installer's `VIProductVersion` was already made numeric by
  `nsis::version4` (`0.1.0.0`); the file names come from `cantus_update::release`.
- The CLAP host now tells plug-ins the workspace version (`cantus-host`, was a literal
  `0.1.0`); the proof splash too.
- **The updater and pre-releases** — a real bug for the beta: the release workflow
  publishes `v0.1.0-beta.1` as a GitHub *pre-release*, and GitHub's
  `/releases/latest` never returns a pre-release, so a beta would never have heard of
  `beta.2`. A pre-release build now asks `/releases?per_page=20` and takes the newest
  non-draft release (`release::newest`); a release build still asks `/latest`. ADR 0032 §4
  amended.
- Tests: `0.1.0-beta.1 < 0.1.0-beta.2 < 0.1.0-beta.10 < 0.1.0-rc.1 < 0.1.0 < 0.1.1`; the
  workspace version parses; a beta is offered the next beta and then the release, never
  the reverse; the newest of a list is picked (drafts and non-version tags left out); the
  list's answer is kept and reused on `304`.
- No tag was created.

### Also fixed on the way

- `Cargo.toml`'s `repository`/`homepage`, the CLAP host's URL and the plug-ins'
  `URL`/`MANUAL_URL`/`SUPPORT_URL` named `github.com/lalmazan/cantus`; the repository (git
  remote, updater, installer) is `github.com/luis314159/cantus`. All now say the latter.
- The record key's tooltip read "Record (R) (R)" (the label carried the key and `tip()`
  appended it again).
- Spanish typos: "Selecciónar", "selecciónado".
- `LICENSE` listed only three GPL crates; it now lists every crate by licence and points
  to the notices and privacy statement. The release `README.txt` names them and the manual.

## Tests

- `cantus-update` — 4 new in `tests/check.rs` (the pre-release chain; the newest of a list;
  a beta offered the next beta then the release, the newest beta up to date, an empty list;
  the 304 reuse). All 28 + 10 unit pass.
- `xtask` — `notices_tests.rs` (5: SPDX expressions; the Apache collapse keeps the notices
  around it; a missing notice, a forbidden licence and a stale file are reported; a crate
  without a licence file gets the standard text and a missing standard text is flagged;
  **the real closure against the committed file**), `manual_tests.rs` (3: the converter;
  GitHub's anchors and the link mapping; both languages build with the same chapters, every
  picture exists, every in-page link has its heading, pictures < 4 MB), `package_tests.rs`
  (the docs are staged, zipped, uninstalled folder by folder; the manual is not in the
  plug-ins zip).
- `cantus-ui` — `tests/about.rs` (3: the window in English and Spanish names the maker,
  the version, the licence, the RVR1960 notice word for word, the trademarks and the motto;
  each button returns its document and Esc closes; every document About points at exists
  in the repository); `docs.rs` unit test (installed copies preferred, the web otherwise).

## Files outside my area

- `Cargo.toml` (version, repository URLs), `Cargo.lock` (the workspace's own versions only).
- `crates/cantus-host/src/clap/host.rs` (version from the workspace, URL),
  `crates/cantus-plugins/src/desc.rs` (URLs).
- `crates/cantus-ui`: `src/app.rs` (the stock About replaced by one call),
  `src/views/transport.rs` (the Help menu), `src/lib.rs`, `src/views/mod.rs`,
  `src/i18n.rs` (one area per locale), `locales/en.txt`/`es.txt` (record tooltip, typos),
  `examples/shoot.rs` (`about` kind), `examples/proof/splash.rs` (version).
- `xtask/src/main.rs`, `package.rs`, `package_tests.rs` (`Inputs::docs`; **the build
  builder also changes `package.rs`** — the merge is additive: one field, one loop, one
  function).
- `docs/adr/0032-…md` (§4 amendment), `LICENSE`, `xtask/installer/README.txt`.

## Left, and notes for the integrator

1. **Export chapter**: the export builder is adding MP3/FLAC/Ogg and an export dialog this
   round. After merging, update the manual's chapter 8 (both languages), the known-issues
   bullet "Export is WAV only", and the release notes' known issues; re-render a picture of
   the dialog if it helps.
2. **New crates from other builders** (export's encoders, pool's deques…): run
   `cargo run -p xtask -- notices` after the merge and commit the file — the test fails
   until then, by design. An LGPL encoder (LAME) would fail the allow-list: add
   `LGPL-2.1-or-later` to `notices::ALLOWED` only with the owner's decision recorded, and a
   notice in the head about where its source is.
3. **The ASIO SDK's source** is not in the repository; the GPL asks that the corresponding
   source stay available while binaries ship. Steinberg's URL is named; mirroring the SDK
   zip as an asset of each release (the GPL-3 option allows it) would make that
   independent of Steinberg — the owner's call.
4. Soak/UI findings the manual works around rather than fixes (they are in
   `crates/cantus-ui`, the soak builder's area): *Add audio track* does not arm the new
   track though the code comment says so; the count-in is silent unless the metronome is
   on; no recovery prompt (autosaves must be renamed by hand); New/Open do not ask about
   unsaved changes. When those change, update chapter 11 and 14.
5. `STATUS.md` was not touched (the integrator's).
6. The trademark search for "Cantus", the RVR1960 written confirmation and code signing
   remain the owner's (BETA.md).
