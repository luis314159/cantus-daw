# ADR 0167 — The effects as plug-ins on their own: a zip per effect per system, and the site's Plug-ins pages

**Status:** Accepted (round 42, the `site-plugins` builder; the owner's request of 2026-11-26,
`docs/BACKLOG-excellence.md` §6a) · **Date:** 2026-11-26 · **Builds on:** ADR 0020 (our
plug-ins outside Cantus, the bundler), ADR 0048 §7 (stable names under
`releases/latest/download/`), ADR 0067 (three systems), ADR 0034 (the website) · **Schema:** none

## Context

The owner (2026-11-26): people who come only for the effects should be able to download
them as plug-ins from cantusdaw.com, without the DAW. The instruments are not plug-ins yet.

Before this ADR, a release carried the plug-ins in three ways, none of them usable from the
site:

- Windows: a single versioned zip of all 43 (`Cantus-<v>-plugins-windows-x64.zip`, about 1 GB).
- Linux: the bundles inside the AppImage only.
- macOS: none.

The site has two hard budgets:

- the WebAssembly, at most 250 000 B gzipped (244 040 B before this ADR);
- the home's page, at most 42 000 B gzipped (35 083 B before).

The site must never link a file that does not exist (`shelf.rs`, ADR 0067 §4).

## Decision

1. **Names.**
   - Each effect's zip per system: `Cantus-plugin-<slug>-<platform>.zip`.
   - All of them per system: `Cantus-plugins-all-<platform>.zip`.
   - `slug` is the plug-in's (`cantus_plugins::desc`), which is also its effect's page.
   - `platform` is `windows-x64`, `linux-x86_64` or `macos-universal`
     (`release::Target::platform`).
   - No version in the names, so the site links them through `releases/latest/download/`.
   - They are made only in `cantus_update::release`. An xtask test keeps the workflow and the
     site to them.
2. **Contents.**
   - Under one top folder: `CLAP/`, `VST3/`, the GPL text, the copyright, the notices, and a
     README for that system (its folders, the unsigned steps, the licence and the trademark
     line).
   - A Mac bundle's program and a Linux library keep their executable bit.
3. **Made by `xtask plugin-zips`, in each system's release job, from the bundles that job
   already has.**
   - Windows: from the signed stage.
   - Linux: from `build-appimage.sh`'s `target/bundled`.
   - macOS: built for both architectures (`bundle --target`), joined with `lipo` and signed
     ad hoc.
   - The zips are listed in each part's sums and in the release's `SHA256SUMS.txt`.
   - The publish job refuses a release that lacks, on any system, the zip with all or one
     effect's zip.
   - Windows' versioned zip stays.
4. **The site.**
   - The pages: `/plugins/` and `/plugins/<effect>/`, in both languages.
   - A button is a link only when the release the site is built against
     (`assets/release.json`) holds its file. Otherwise it is a disabled button with a note.
     The pages need no change when a release first carries the zips.
   - The plug-in files' facts are split out of `REL_FILES` (native `REL_PLUGINS`).
   - The facts and the pages' longer text reach the browser in `/plugins.json`, fetched only
     by these pages.
   - The pages are written as HTML strings, by the same functions in the pre-renderer and the
     browser. Only the players (with/without, Try it) are components. Yew components for this
     much text cost 12 KB of the WebAssembly's budget.
   - The system is a radio group that shows its card and steps by CSS (`:has()`; without it,
     Windows'), so the static page works before the app. The app ticks the visitor's system.
5. **Honesty in the copy.**
   - **The window:** the plug-ins' own window is Windows-only
     (`cfg(all(feature = "editor", windows))`); on Linux and macOS the host's controls show.
     The pages say so, and the effect cards now say "with the same face on Windows".
   - **macOS:** a preview. AU is marked "coming" until Apple signing.
   - **Unsigned:** the SmartScreen and Gatekeeper steps are given.
   - **Instruments:** a note only, no button.
6. **Release lesson L10** (`docs/RELEASING.md`), with its PREFLIGHT box: a rehearsal checks
   the zips on every system before a tag.

## Consequences

- **Size:** a release grows by about 130 files and an estimated 8 GB:
  - about 23 MB per effect per system, the Mac's about twice that;
  - the zips with all, about 1 GB each (2 GB on the Mac).

  The Mac job builds the plug-ins twice, and its limit goes 300 → 360 minutes. A smaller
  plug-in binary waits on ADR 0020's per-kind dispatch.
- **Page weight:**
  - the home grows only by the inlined `plugins.css`;
  - the per-page facts and text stay out of the WebAssembly;
  - adding an effect adds one card and one page with no code.
- **Signing:** the Mac's plug-ins are ad hoc until the Apple secrets exist. The same
  secrets later allow signing, notarising and an AU build.
- **What the tests hold:**
  - `xtask/src/plugin_zips_tests.rs`: the names; a zip's listing and modes; the
    workflow's steps and order; the site's names, folders and plug-in set equal to
    `desc::ALL`.
  - `web/src/plugins.rs` and `web/src/pages/plugins.rs`: no page links a zip the release
    lacks, every system has its card and steps, one `<h1>`, both languages.
  - `route.rs`: the new addresses parse, and an instrument is no plug-in page.
