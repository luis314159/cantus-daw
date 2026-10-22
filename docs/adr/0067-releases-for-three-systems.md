# ADR 0067 — Releases for three systems: one tag, the updater per system, the website

**Status:** accepted (round 30) · **Date:** 2026-10-22 · **Amends:** ADR 0032 (§2, §4, §5,
§7: a release is three systems' files; how each applies an update), ADR 0048 (§7: three
stable names) · **Uses:** ADR 0065 (Linux, the AppImage), ADR 0066 (macOS, the disk image)

## Context

Until round 30 a release was Windows only: `release.yml` built, tested and published the
NSIS installer, the portable zip and the plug-ins; the updater asked for
`Cantus-<v>-windows-x64-setup.exe`; the website's card said *Coming soon* for Linux and
macOS. Round 30 makes Linux (ADR 0065) and macOS (ADR 0066) products, and the owner asked
that the round end with releases for all three. Three questions follow: how one tag makes
three systems' files; how a running Cantus on each system knows its file and applies it;
how the website says, truthfully, which systems can download today.

## Decisions

### 1. One tag, three jobs, one publish

`release.yml` has, beside Windows's `test` and `package`, a `linux` job (`ubuntu-22.04`)
and a `macos` job (`macos-latest`). Each one tests the workspace on its own system first
(clippy with `-D warnings` and `cargo test --workspace`), checks the tag against the
workspace's version, then builds its file through the packaging script the round's
contract fixed (`packaging/linux/build-appimage.sh <version> <out>`,
`packaging/macos/build-dmg.sh <version> <out>`), copies it under its stable name and hashes
both. Linux builds on 22.04, not `latest`: an AppImage runs on systems as new as the one
it was built on or newer (glibc), so the oldest supported LTS sets the floor. Its system
packages are a superset of `ci.yml`'s Linux job's, held by a test.

One `publish` job needs all three: a release is the three systems' files or nothing (a job
that failed for the runner's sake is re-run from the run's page; a release missing a
system silently would make the website and the updaters disagree with the notes). It
checks each system's sums as they arrive, joins the files, writes one `SHA256SUMS.txt`
(`<hex>  <name>`, sorted — what `sha256sum -c` and the updater read), refuses two systems
publishing the same name, checks the three versioned files and the three stable names,
and creates the release as before (the beta-is-latest rule of ADR 0048 §7 unchanged).
`workflow_dispatch` builds and tests everything and keeps the files as artifacts,
publishing nothing. The macOS signing secrets (`APPLE_CERT_P12_BASE64`,
`APPLE_CERT_PASSWORD`, `APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_PASSWORD`) are passed to the
script as environment variables; absent, the script signs ad hoc (ADR 0066).

### 2. The files and their names (made in `cantus-update/src/release.rs` only)

| Target | Versioned (the updater asks for it) | Stable (the website links it) |
|---|---|---|
| `WindowsX64` | `Cantus-<v>-windows-x64-setup.exe` | `Cantus-windows-x64-setup.exe` |
| `LinuxX64` | `Cantus-<v>-linux-x86_64.AppImage` | `Cantus-linux-x86_64.AppImage` |
| `MacUniversal` | `Cantus-<v>-macos-universal.dmg` | `Cantus-macos-universal.dmg` |

`Target::of(os, arch)` maps Rust's `OS`/`ARCH`: Windows x86-64, Linux x86-64, macOS on
either architecture (one universal image). Anything else — Linux on ARM, a native
Windows-on-ARM build, a BSD — has no file: the updater only shows the release's page.
The updater asks for the versioned name, never the stable copy (same bytes, but the name
says which version was verified). Every file is held to its SHA-256 — GitHub's digest and
the `SHA256SUMS.txt` line, as before; Authenticode is checked on the Windows installer
only (an AppImage and a disk image carry no Authenticode; a notarised disk image is
checked by Gatekeeper when it opens).

### 3. How each copy applies an update (`InstallKind`, `install::apply`)

`InstallKind` grows from Windows's three kinds to six, detected by
`InstallKind::detect_on(os, exe, $APPIMAGE, $FLATPAK_ID)`:

- **Installed** / **Portable** (Windows): as ADR 0032 — the installer runs silently with
  `/UPDATE` on quit (`/RELAUNCH` for *Restart and update*); a portable copy is shown the
  page.
- **AppImage** (Linux): the AppImage is replaced. `$APPIMAGE` alone does not make a
  copy an AppImage — every child of any AppImage program inherits it (a terminal inside
  an AppImage editor), and a distribution's Cantus started from there would rename itself
  over that program's file. It counts only when the executable lies inside the mounted
  AppImage (`$CANTUS_APPDIR`, which our AppRun exports, or the runtime's `$APPDIR`), is not
  a cargo build, and `$APPIMAGE` names a file (`install::own_appimage`, used by the
  detection, the apply, the writable check and the `.old` clean-up alike). Before
  downloading, the folder is checked writable (`ship.error.not_writable` otherwise, with
  the page — a system-wide `/opt` copy is told at once, not at quit). On quit,
  `replace_appimage` moves the verified download beside the old one (a rename on one
  file system, else a copy and the download removed), makes
  it executable, flushes it, keeps the old one as `<name>.old` (a hard link, or a copy),
  and renames the new one over the old path — atomic: at no moment is there no Cantus at
  that path, and the running process keeps its own open file. *Restart and update* starts
  the new AppImage through `sh -c 'while kill -0 $pid …; exec $0'`, so it opens after this
  Cantus has closed, never beside it. The new one, on its first frame, removes the `.old`
  (`install::after_start`); until then the user can rename it back.
- **Downloads are not kept:** when a check decides, every file in the updates folder
  but the offered version's is deleted (an installed update, a skipped one, a partial
  file); up to date, all of them.
- **Package** (Linux, Flatpak, or a binary not from cargo's `target/`): the package
  manager updates it; the app says so and shows the page.
- **MacBundle** (macOS, inside `*.app/Contents/MacOS`): the disk image is downloaded and
  verified; on quit (or *Close and open the disk image*) it is opened with `open`, and the
  user drags Cantus onto *Applications*. **Replacing a signed `.app` in place** (Sparkle's
  way: unpack, verify the code signature and team, swap the bundle, relaunch) is later —
  it needs the signed builds and a Mac to test on; BACKLOG §2.
- **Development**: a cargo build anywhere; no automatic checks.

### 4. The website asks GitHub which systems are out

The download card (`web/src/body.rs`) says *Available* with a direct link for Linux or
macOS only when the latest release holds that system's file under its stable name. The
browser asks `api.github.com/repos/luis314159/cantus-daw/releases/latest` once a visit
(`web/src/shelf.rs`) — one request for the whole page, shared by the cards that want it — keeps the answer an hour in `localStorage` (GitHub allows 60
anonymous requests an hour per address), and shows *Coming soon* (and *Build from
source*) until the answer comes, when it fails, or when it says no — a link never answers
404. The side panel lists each system's state, the system and audio lines speak to the
visitor's system, and the roadmap and the FAQ say "out now" once both are. The request
carries nothing but the visitor's address, as following the download link does.

## Consequences

- One tag is the whole release; a failed system blocks publishing until re-run.
- A release made before round 30 has no Linux or macOS file: those copies are shown the
  page (no Linux or macOS copy of such a release exists anyway).
- macOS updates need one drag from the user until in-place replacement is built.
- The website's Linux and macOS cards change by themselves when the first three-system
  release is published; no site deploy is needed for that.
- `web/src/top.rs` holds the three stable links; a test keeps them equal to the updater's
  names and to the workflow's checks.

## What the tests hold

- `cantus-update/tests/check.rs`: `every_system_and_architecture_asks_for_its_own_file`
  (the target for every OS/arch pair, the names, the stable names, the versioned file
  chosen over the stable copy, a Windows-only release offering nothing to Linux or a Mac);
  `an_appimage_and_a_disk_image_download_and_verify_like_the_installer` (held by a proxy,
  retried, verified by hash with a Windows publisher set; a package or a target-less copy
  offered nothing to apply; a Mac not offered the Windows installer).
- `cantus-update/src/install.rs`: `each_system_reads_its_own_kind`,
  `an_appimage_is_replaced_whole_and_the_old_one_kept_until_the_new_one_starts`,
  `a_failed_replacement_changes_nothing`,
  `a_folder_is_writable_or_not_and_the_probe_leaves_nothing`,
  `the_new_appimage_starts_after_this_cantus_has_closed`,
  `the_relaunch_waits_for_the_process_then_runs_the_program` (Unix),
  `only_a_copy_that_updates_itself_applies_a_file`,
  `only_the_appimage_this_program_runs_from_is_its_own` (`$APPIMAGE` set but the program
  outside the mount, a cargo build, no mount named, a stale path).
- `cantus-update/tests/check.rs`: `old_downloads_are_deleted_and_the_offered_one_kept`.
- `xtask/src/package_ci_tests.rs`: `every_system_s_stable_name_is_in_the_workflow_and_on_the_site`
  (and the scripts called by the contract's interface),
  `the_release_s_linux_job_installs_every_package_ci_does`.
- `web/src/shelf.rs`: one request however many cards ask; a system is available only with its stable file; a kept answer
  lasts an hour and nothing malformed is believed.
