# Round 30 — ship (item 4): releases for three systems

Branch `r30/ship`. ADR 0067. What was asked: one tag publishes the three installers on
`cantus-daw`; the website says *Available* for Linux and macOS only once their release
exists; the updater knows which file to ask for on each system.

## What was done

**The release workflow** (`.github/workflows/release.yml`). Beside Windows's `test` and
`package`, a `linux` job (`ubuntu-22.04`, the glibc floor of the AppImage) and a `macos`
job (`macos-latest`, both Rust targets added). Each runs clippy with `-D warnings` and the
whole suite on its system, checks the tag against the workspace's version, builds its file
through the contract's script (`bash packaging/linux/build-appimage.sh "$version" dist`,
`bash packaging/macos/build-dmg.sh "$version" dist`, the five Apple secrets passed as
environment variables), and uploads only that file, versioned and under its stable name,
with its sums. `publish` needs all three, checks each system's sums, joins the files into
one release with one sorted `SHA256SUMS.txt`, refuses a name published twice, checks the
three versioned files and the three stable names, and publishes as before (the
beta-is-latest rule unchanged). `windows-release.yml` is unchanged (it is a push check,
not a release). The `if: github.repository == 'luis314159/cantus-daw'` guards and the
action versions are as the other workflows have them.

**The updater per system** (`crates/cantus-update`). `release::Target` (`WindowsX64`,
`LinuxX64`, `MacUniversal`) from Rust's OS/arch; the file names made in one place
(`appimage_name`, `dmg_name`, `STABLE_APPIMAGE_NAME`, `STABLE_DMG_NAME`);
`Release::update_file(target)`. `InstallKind` gains `AppImage`, `Package` and `MacBundle`,
detected from the executable's path, `$APPIMAGE` and `$FLATPAK_ID`. `install::apply`:
Windows runs the installer as before; an AppImage is replaced atomically (copy beside,
`chmod 755`, flush, keep the old one as `.old` by hard link, rename over), relaunched only
after this process exits, and the `.old` removed when the new one draws its first frame;
the folder is checked writable before downloading; a Mac opens the verified `.dmg`
(in-place `.app` replacement is later — ADR 0067 §3, BACKLOG §2). Authenticode is checked
on the Windows installer only; every file is held to its SHA-256. `Config` has a new
`target` field.

**The update window** (`crates/cantus-ui/src/views/update.rs`, additive): the per-system
apply, the Mac's button and on-quit label, the AppImage's and the Mac's *Ready* text, the
package and no-file notes; new locale area `ship.{en,es}.txt` (9 keys), registered in
`i18n.rs`. `demo_setup` has `update-ready-mac` and `update-ready-appimage` for the shots.

**The website** (`web/`). `src/shelf.rs` asks `api.github.com/…/releases/latest` once a
visit from the browser, keeps the answer an hour in `localStorage`, and reports whether
the latest release holds `Cantus-linux-x86_64.AppImage` and `Cantus-macos-universal.dmg`.
The download card gives Linux and macOS visitors *Available* and a direct download when
it does, *Coming soon* and *Build from source* otherwise (and while the answer is pending
or failed); the side panel lists the three systems' states, and the system and audio lines
speak to the visitor's system; the roadmap item and the FAQ answer switch to "out now" once
both are out. `src/top.rs` holds the two new stable links. `wasm-bindgen-futures` is now
a direct dependency (it was already in the build through Yew).

**Docs.** The manual's chapter 1 (EN/ES) now has *On Windows*, *On a Mac*, *On Linux*
(download, open, Gatekeeper, microphone, FUSE, where the data lives, uninstall) and
chapter 13 says how each system applies an update; `docs/releases/TEMPLATE.md` (EN/ES,
the three files, how each updates); `docs/DEVELOPING.md` §5 *A release: one tag, three
systems*; ADR 0067; a BACKLOG §2 line for the Mac's in-place update.

## How it was tested

- `cargo test -p cantus-update`: 16 unit + 20 integration tests pass on Windows (new: the
  target and file for every OS/arch, the versioned file over the stable copy, an AppImage
  and a disk image downloaded through a proxy hold and verified with a Windows publisher
  set, a package or a target-less copy offered nothing, each system's kind detected, the
  AppImage replaced and kept and forgotten, a failed replacement changing nothing, the
  writable probe, the relaunch command; on Unix, the relaunch really waits for the old
  process).
- `cargo test -p xtask`: 36 pass (new: every system's stable name in the workflow and on
  the site, and the scripts called by the contract; the release's Linux packages a
  superset of CI's). `cargo test -p cantus-ui --lib -- i18n update`: pass (every English
  key in Spanish, the `ship` area included).
- `web`: `cargo test` (2 new), `trunk build --release`, and headless-Chrome shots of the
  card with a Linux user agent in both states (*Disponible* with *Descargar el AppImage*,
  and *Coming soon*), read.
- The workflow: parsed as YAML and every bash step checked with `bash -n` (WSL); the
  packaging and publish steps run locally with stub scripts outside the repository
  (versioned + stable names, one sorted sums file, extra files the script leaves kept out).
  `actionlint` is not installed. The workflows themselves run only on `cantus-daw`.
- `cargo clippy --workspace --all-targets` with `-D warnings` on Windows: clean. Linux
  (WSL, the branch as committed): clippy clean (workspace, and the plug-ins without
  editor). `check.sh` clones into one `~/cantus-ci` shared by all three builders, so a run
  can mix two branches' sources (seen once: a test compiled against another branch's
  `Config`); ship's Linux runs used a private clone (`~/cantus-ci-ship`). On Linux,
  `cantus-update`: 17 unit (the relaunch really waits for the old process) + 19
  integration pass. The workspace's Linux test run (sharing `~/cantus-ci/target` with the
  other builders) showed doctests unable to find crates (the shared target being rebuilt
  under them) and one failure of `tempo_review6_ui::one_kept_or_left_out_is_said_in_the_singular`,
  which passes when run alone on the same commit (a pre-existing order dependence on the
  interface language, not ship's code) — the integrator's full run is the word on it.
- Shots: `shoot … update-ready-mac update-ready-appimage update-ready` (light and dark)
  read; the Mac's on-quit label was cut ("OPEN THE DISK IMAGE WHEN…") and is now "Open it
  when I close Cantus", the same length as Windows's label that fits.

## Files outside ship's list (additive)

- `xtask/src/package_ci_tests.rs` — two tests (names in workflow and site; apt packages).
- `crates/cantus-ui/src/i18n.rs` — two `include_str!` lines for the `ship` area.
- `docs/BACKLOG-excellence.md` — one bullet in §2 (the Mac's in-place update).

## For the integrator

- When linux's `ci.yml` gains system packages, `release.yml`'s Linux line must hold them
  too (the test fails otherwise). Its line already has `libpipewire-0.3-dev
  libjack-jackd2-dev libclang-dev libfuse2` beside CI's four.
- After merging the three branches' manual edits, regenerate the website's manual
  (`powershell -File web/build-manual.ps1`) and commit `web/assets/manual/`; link
  linux's and macos's manual sections from chapter 1's *On Linux* / *On a Mac* if they
  are written elsewhere than chapter 2.
- The minimum macOS version is macos's (`Info.plist`); the manual and the site say
  "Apple silicon or Intel" without a number — add it once known.

## Only the owner

- The first three-system tag (`v0.2.0-beta.1` or as chosen) on `cantus-daw`, and re-running
  a system's job if its runner fails.
- The Apple secrets (Developer ID certificate, team id, Apple ID, app-specific password)
  for a notarised disk image; until then it is signed ad hoc.

## After the review (integrator, 2026-10-22)

1. `$APPIMAGE` is trusted only when the executable lies inside `$CANTUS_APPDIR`/`$APPDIR`
   and is not a cargo build (`install::own_appimage`, in detection, apply, the writable
   check and the `.old` clean-up); test with `$APPIMAGE` set and the program outside.
2. The updates folder keeps only the offered version's download (pruned when a check
   decides); the AppImage's download is renamed into place on one file system, else
   copied and removed.
3. The website sends one GitHub request per page (`shelf::Once`, shared by the download
   card and the roadmap).
