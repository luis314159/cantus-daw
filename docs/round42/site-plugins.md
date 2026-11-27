# Round 42 — the effects as plug-ins on their own, from the site (`r42/site-plugins`)

The owner's request (2026-11-26, `docs/BACKLOG-excellence.md` §6a, "Instruments and effects to
download on their own"): a section of cantusdaw.com where people who come only for the effects
download them as plug-ins, without the DAW. The instruments come later; they are not plug-ins
yet. Branch `r42/site-plugins`, from `r42/integration` (43cee2c36) with private master's site
work (`site/lcp`, 290b82256) merged in.

**Nothing here is live.** No deploy, no push, no tag, no workflow run, no Actions variable or
secret touched. The section goes out with the owner's OK; its buttons stay disabled until a
release holds the zips (0.1.0-beta.18 is the first meant to).

## 1. The release side

**`xtask plugin-zips <out> [--from <dir>] [--no-build] [--sums]`** (`xtask/src/plugin_zips.rs`).
For the system it runs on, it writes into `<out>`:

- one zip per effect, `Cantus-plugin-<slug>-<platform>.zip`;
- one with all of them, `Cantus-plugins-all-<platform>.zip`.

`platform` is `windows-x64`, `linux-x86_64` or `macos-universal`, the other release files'
spelling (`cantus_update::release::Target::platform`). `slug` is the plug-in's
(`cantus_plugins::desc`), which is also the effect's page on the site. The names have no
version, so `releases/latest/download/<name>` always brings the newest (ADR 0048 §7). They are
made only in `cantus_update::release` (`plugin_zip_name`, `plugins_all_name`).

Each zip holds, under one top folder (`Cantus Aequalis 0.1.0-beta.18/`, `Cantus plug-ins …/`):

- `CLAP/Cantus <Name>.clap` and `VST3/Cantus <Name>.vst3/…`, in `bundle`'s layout;
- `LICENSE.txt` (GPL), `COPYRIGHT.txt`, `THIRD-PARTY-NOTICES.txt`;
- a `README.txt` for that system: the folders, the unsigned steps, the licence and the
  trademark line, the site.

A Mac bundle's program and a Linux library keep their executable bit (`archive::zip_modes`).
Every other zip the packager writes is unchanged.

**Where the bundles come from:**

- **Windows:** the stage the package step signed (`target/package/stage/{clap,vst3}`), so the
  per-effect zips carry the same signature as the installer's plug-ins once signing is set
  up. The folder's `SHA256SUMS.txt` is then written again over every file (`--sums`).
- **Linux:** the bundles `build-appimage.sh` already built into `target/bundled`.
- **macOS:** built for both architectures (`bundle --target <triple>`, new, with
  `MACOSX_DEPLOYMENT_TARGET=11.0`), joined with `lipo` and signed ad hoc (`codesign -s -`).
  The Mac had no plug-ins in its release before.

**`release.yml`:**

- the package job runs it after the installer's signature check, before the upload;
- the Linux job runs it inside the AppImage step, before the sums;
- the Mac job runs it as a step of its own, before the sums. Its `timeout-minutes` goes
  300 → 360: at beta.17 the whole job took 184, and Linux's builds after its suite, the
  plug-ins among them, took 63 of 216.
- the publish job refuses a release that lacks, on any system it publishes, the zip with all
  or one effect's zip (counted against Windows' set).

The release's `SHA256SUMS.txt` lists them like every other file. Windows' versioned
`Cantus-<v>-plugins-windows-x64.zip` stays, for the links already out there.

**What a release now carries more**, estimated from beta.16's 1.0 GB Windows plug-ins zip (43
plug-ins, the CLAP and the VST3 the same library twice): about 23 MB per effect per system, the
Mac's about twice that.

- Windows: about 1 GB of per-effect zips and 1 GB for the one with all.
- Linux: about the same.
- macOS: about twice the same.
- In all, about 130 more files and 8 GB more per release. It is free on a public repository,
  but publishing takes longer.
- A smaller zip waits on ADR 0020's note: one binary per kind instead of the whole family's
  DSP in each.

**Tests** (`xtask/src/plugin_zips_tests.rs`):

- `every_effect_has_a_stable_plain_zip_name_on_every_system`: the names, unique, plain, no
  version, none another release file's.
- `a_mac_zip_holds_both_formats_runnable_and_the_texts`: from stand-in bundles, the zip's
  exact listing, 0755 on the programs and 0644 on the rest, the README's folders, its
  Gatekeeper steps, its licence and its trademark line.
- `a_windows_zip_comes_from_the_signed_stage`: the stage's layout, the README's CRLF and
  Unblock, and the installer's README naming the same folders.
- `the_release_workflow_makes_checks_and_publishes_every_plug_in_zip`: each job's step, in
  order (after the bundles, before the sums or the upload), and the publish job's checks of
  both names on every platform.
- `the_site_links_every_plug_in_by_its_stable_name_and_folders`: the site's names, repository
  and folders, and the set of effects it calls plug-ins equal to `desc::ALL`'s slugs.

`docs/RELEASING.md` has a lesson for it, **L10**, with its box in `docs/releases/PREFLIGHT.md`
(the existing test keeps the two one). `docs/beta-checklist.md` has the steps only the owner can
take ("Ronda 42 — los plug-ins solos").

## 2. The site

**The pages:**

- `/plugins/` and `/es/plugins/`;
- `/plugins/<effect>/` for each of the 43 exported effects (`units_data` `plugin: true`);
- 88 more pre-rendered pages, in the sitemap with the rest.

**`web/src/plugins.rs` holds:**

- the names (the same as the release's), the folders per system, the systems;
- each file's size and SHA-256, from the release the site is built against;
- the longer text.

**Gating (honesty):** a download button is a link only when the release the site is built
against (`assets/release.json`, refreshed by `site.yml` after every release) holds that file.
Otherwise it is a disabled button (`aria-disabled`, no `href`) with the note "arrives with the
next beta, 0.1.0-beta.18". It turns on with no change to the site: the release's publish job
dispatches the site's rebuild. Windows' "all at once" already links the versioned plug-ins zip
every Windows release has carried, which is a real file today.

**The wasm budget:** `build.rs` splits the plug-in zips out of `REL_FILES` into `REL_PLUGINS`,
native only. Over a hundred files with their hashes would have put the WebAssembly over its
250 000 B.

**The pages' bodies are HTML the pre-renderer writes** (`pages/plugins.rs`, natively only):

- into each page;
- into `/plugin-parts/<lang>/<page>.json`, which the page fetches (`main.rs` waits for it on a
  first load; after a move from another page, the page fetches it).

The browser carries only the shell and the players (with/without, Try it).

What the WebAssembly measured, each version over the budget until the last:

| Version | gzip |
|---|---|
| Before this branch | 244 040 B |
| Yew components | 260 670 B |
| Notes and cards as strings | 256 272 B |
| Everything as strings built in the browser | 257 029 B |
| Fetched parts, with a HashMap | 250 935 B |
| Without the HashMap (its hasher was new to the WebAssembly) | 249 860 B |
| One component for both pages, `tryit::fetch` reused | **249 381 B** |

Decision: ADR 0167.

**What each page has:**

- **Index:** the formats (CLAP, VST3, "AU · coming"), the zip with all for the visitor's
  system, every effect's card (its picture, kind and pitch), a note that the instruments are
  not plug-ins yet (no button), then the install steps, the unsigned steps, the licence and
  the way to the DAW.
- **An effect's page:** the same head and download card for that effect, with the other
  systems listed below it. Then its picture in the page's theme, what it does, a link to its
  effect page, its with/without excerpt and its "Try it" where they exist, the same notes,
  and the previous and the next.
- **The visitor's system** is a radio group, Windows, Linux and macOS: native keyboard and
  screen-reader behaviour. The choice shows that system's card and steps by CSS alone
  (`:has()`; without it, Windows'), so the static page works before the app runs. The app
  ticks the visitor's own system from the user agent (`platform.rs`).

**Honesty in the copy:**

- **The window:** the plug-ins' own editor is Windows-only. `clap/gui.rs` and `vst3/plugin.rs`
  are `cfg(all(feature = "editor", windows))`, so on Linux and macOS the host's own
  controls show. The pages say so. The effect cards' "with the same face" line now says
  "on Windows" and links the plug-in page.
- **macOS:** a preview, not yet loaded in a DAW on a real Mac. AU waits for Apple's signing.
- **Unsigned:**
  - Windows: SmartScreen's Keep → Keep anyway, and Properties → Unblock;
  - macOS: Privacy & Security → Open Anyway, or `xattr -dr com.apple.quarantine`;
  - Linux: nothing to allow.
- **Licence:** GPL-3.0-or-later, with what you make being yours, and the trademark line per
  `TRADEMARKS.md` (share the official zips as they are; a modified build under its own name).

**Links in:** the footer's Product column, the download page's line about the plug-ins
alone, the effects index, and every effect's page and card.

## 3. Verified here, and not

Verified here, on the built site (Trunk, 216 pages pre-rendered):

- **WebAssembly:** 249 381 B gzip (budget 250 000).
- **The home's page:** 35 768 B (EN) and 36 347 B (ES), within two round trips (42 000).
- **Lighthouse, the site budget** (`tools/web-budget.ps1 -NoBuild`, the home, calibrated CPU
  2.43×, 3 runs): LCP 1.66 s, 811 KB, CLS 0, accessibility 100. Every line within budget.
- **Lighthouse on the new pages** (one run, same CPU, not a gate):

  | Page | LCP | Size | CLS | Accessibility |
  |---|---|---|---|---|
  | `/plugins/` | 3.5 s | 446 KB | 0 | 100 |
  | `/es/plugins/fragor/` | 4.1 s | 535 KB | 0 | 100 |
  | `/plugins/aequalis/` | 3.8 s | 491 KB | 0 | 100 |
  | `/effects/aequalis/` (existing) | 4.7 s | 1.5 MB | | |
  | `/download/` (existing) | 3.3 s | | | |

- **Contrast** (`contrast.mjs`, light and dark, 1440 and 390 px): AA on every text of
  `/plugins/`, `/es/plugins/`, `/plugins/aequalis/`, `/es/plugins/fragor/` and
  `/plugins/delay/`, on the changed `/effects/aequalis/` and `/download/`, and on the preview's
  live pages. The lowest is 4.59:1.
- **Motion:** `motion.mjs` passes.
- **The system's radio group** switches the card and the steps for Windows, Linux and macOS
  by CSS (headless Edge, `/plugins/fragor/` and `/es/plugins/`).
- **Tests:**
  - `cargo test --features ssr`: 36 passed. `cargo fmt --check` is clean. Clippy, natively
    and for `wasm32`, adds no warning.
  - xtask: the five `plugin_zips_tests` pass, and so does `cargo clippy -p xtask -p
    cantus-update --all-targets` with `-D warnings`.
  - In the whole xtask run, `package::ci_tests::every_file_a_test_or_the_build_reads_runs_ci_when_it_changes`
    fails on `tools/soundlab/subjects/arcus-cello/subject.json` (commit 95898ce72, already in
    `r42/integration`, not this branch's).
- **Screenshots:** `F:\projects\daw-r42	mp\site-plugins\` — `now\` is the real state,
  `live-preview\` uses made-up facts.


Verified only in emulation, or not at all:

- No zip was built from real plug-ins: the workflow was not run (that is the rehearsal, with
  the owner's OK).
- `lipo`, `codesign` and the Mac plug-in build (`cdylib`, fat LTO, x86_64 built on an Apple
  silicon runner) have never run.
- The host steps (REAPER, Bitwig, FL Studio, Live, Cubase) and the SmartScreen and Gatekeeper
  wording are from the hosts' and systems' documentation and ADR 0020, not checked on screen
  this round. Hence the beta-checklist entry.
- The "live" state of the pages was seen only in a scratch pre-render with made-up facts,
  never committed.
