# ADR 0048 — The CI budget, and the release's stable file name

**Status:** Accepted · **Date:** 2026-10-13 · **Round:** 26 (builder *release*) ·
**Amends:** ADR 0032 (§2, §7: the release's files and how a tag is published) ·
**Schema:** none

## Context

The private repository's GitHub Actions minutes ran out on 2026-10-13. GitHub bills a
private repository's hosted-runner minutes against the plan's monthly quota (2 000 on
Free, 3 000 on Pro) with a multiplier per system: **Linux ×1, Windows ×2, macOS ×10**, each
job rounded up to the minute. Every push to `master` ran, whatever it changed:

| Job (before) | Runner | Est. minutes | × | Billed |
|---|---|---|---|---|
| test — fmt, clippy, the suite, the engine's fuzz, the fork fuzz, a release build | ubuntu | 35 | 1 | 35 |
| the same | windows | 45 | 2 | 90 |
| the same | **macos** | 35 | **10** | **350** |
| perf (quick suite; `continue-on-error`, reports only) | windows | 15 | 2 | 30 |
| null hashes (cantus-bench built 3×: baseline, v2, v3) | windows | 30 | 2 | 60 |
| null hashes | ubuntu | 20 | 1 | 20 |
| null hashes, Windows = Linux | ubuntu | 1 | 1 | 1 |
| supply chain (`cargo install cargo-audit` compiled every run) | ubuntu | 6 | 1 | 6 |
| **every push, any file** | | | | **≈ 590** |

The minutes are estimates (warm caches, private-repository standard runners; the
repository's *Settings ▸ Billing ▸ Usage* gives the real ones and should replace them):
the shape matters more than the digits. macOS was 60 % of the bill; a commit touching only
`docs/` paid it all; three Windows jobs and one Linux job rebuilt the engine for checks that
cannot fail on a UI change; and a hung test could bill six hours ×2 or ×10 (no timeouts).
At ≈ 590 a push, the Free quota lasts three or four pushes.

The website's download button pointed at the releases page, because every installer's
name carries its version (`Cantus-0.1.0-beta.1-windows-x64-setup.exe`) and no fixed link
could name it. And GitHub's `releases/latest` — the only fixed link to a release — never
names a pre-release, which is how `release.yml` published every `-beta` tag: the first
public beta would have had no "latest" release at all.

## Decision

### 1. macOS only weekly or by hand (`macos.yml`)

macOS left `ci.yml`. `macos.yml` runs by hand (*Actions ▸ macOS ▸ Run workflow*, or
`gh workflow run macos.yml --ref <branch>`) and on Mondays at 06:17 UTC, when a one-minute
Linux job finds that code (the same "code" as §2) differs between `HEAD` and the commit
the **last successful macOS run on master** tested (`gh run list … --status success`,
`git diff --quiet <that> HEAD -- <paths>`; no such run, or a commit no longer in the
history, runs it). Not by commit date: a merge of commits written weeks earlier counts
(the review's finding). A quiet week costs one minute. It runs clippy, the suite and both fuzz runs; not fmt
(system-independent, ci.yml runs it) and not the release build. **Run it by hand before a
release tag and after any change to `cfg(target_os = "macos")` / non-x86-64 code** — the
round-25 macOS fixes were never seen green in CI.

What this machine can check of macOS (round 26, `aarch64-apple-darwin` target installed,
no Apple SDK, no C cross-compiler): `cargo clippy --all-targets -D warnings` is clean for
`cantus-core`, `cantus-dsp`, `cantus-project`, `cantus-rt`, `cantus-launch`,
`cantus-engine`, `cantus-update`, `cantus-ai` and `cantus-host`. `cantus-content` and
`cantus-plugins` (through `ring`) and `cantus-export`, `cantus-ui`, `cantus-app` (through
`ogg_next_sys`) need a C compiler for the target and stop at their build scripts — those,
and every test run, need the real runner.

### 2. No run when a commit changes only documents (`ci.yml`)

`ci.yml` triggers through `paths` with `!` patterns (not `paths-ignore`, which cannot
re-include a file after excluding its folder; later patterns win): everything, minus
the root's `*.md` (README, CLAUDE, CLA…; `*` does not cross `/`), `docs/**`,
`research/**`, `tools/**`, `web/**` (the site has `site.yml`) and `.github/**` —
Markdown anywhere else is code: `xtask/notices/head.md`, which the notices test reads,
was missed by a first `**.md` (the review's finding) — plus back in **what a test reads**: `docs/manual/**` (xtask builds the manual
and checks its pictures and links), `THIRD-PARTY-NOTICES.md` (the notices check),
`PRIVACY.md` (About's links), `web/src/top.rs` (§6's test), `ci.yml` itself and
`release.yml` (the ASIO SDK hash and §6). Every other file a test reads was found by
searching `include_str!`/`include_bytes!` and `fs::read*` of the workspace: they are all
under `crates/`, `xtask/` or the root's licence files, which trigger.

`xtask/src/package_ci_tests.rs` holds it: the push and pull-request lists are equal; a
GitHub-semantics glob says code and every document a test reads trigger, while `docs/`,
`research/`, root `*.md`, `web/` and other workflows alone do not; and every file any
crate compiles in with `include_str!`/`include_bytes!` (rustfmt's wrapped form too) or
reads at run time from the workspace or the crate (`ws.join("…")`, `workspace().join`,
`CARGO_MANIFEST_DIR`, a path with folders) triggers — seen failing with
`THIRD-PARTY-NOTICES.md` taken out of the list, and on `head.md` before the fix. A new test that reads a document must
add it to both lists — the test names the file that is missing.

If the CI check is ever made a *required* status, a skipped workflow leaves the check
pending: then use a ruleset that requires it only for paths that trigger it, or a
always-green "docs only" job.

### 3. The null hashes only when what they render can change (`null-hashes.yml`)

Their own workflow, on pushes and pull requests touching the crates the corpus renders
(`cantus-core`, `-dsp`, `-engine`, `-project`, `-rt`, `-launch`, `-bench`), the demo songs
and the factory presets they load (`cantus-ui/src/demos/**`, `views/fx/**`,
`views/inst/**`), `xtask`'s `null_hashes.rs` / `levels.rs`, the lock file, the toolchain
and `.cargo/`; or by hand. Same jobs, same comparison, same guarantee on every commit that
can break it.

### 4. `cargo audit` when the dependencies change, and weekly (`audit.yml`)

On `Cargo.lock` / `**/Cargo.toml` changes, on Mondays (advisories are published against
what is already locked) and by hand. `rust-cache` keeps `~/.cargo/bin` (`cache-bin`), so
`cargo install cargo-audit --locked` compiles only when a new cargo-audit is released.

### 5. Perf by hand (`perf.yml`)

The quick suite never failed a build (`continue-on-error` until a hosted-runner baseline
exists): it moved to `workflow_dispatch`, to be run while gathering that baseline and
after performance work. When it becomes a gate, it gets a trigger again (§3's kind of
paths). `docs/perf/benchmark.md` §6 should say so (not this builder's file).

### 6. Smaller things, same guarantees

- **`concurrency`** on every workflow: a new push to the same ref cancels the run still
  going for the older one (`site.yml` keeps its own non-cancelling group: a deploy is not
  interrupted).
- **`timeout-minutes`** on every job (90 for the suites, 5–60 for the rest).
- **The pinned toolchain only**: `dtolnay/rust-toolchain@stable` installed a stable compiler
  that `rust-toolchain.toml` then overrode; `rustup show active-toolchain || rustup
  toolchain install` installs the pinned one (with clippy and rustfmt), as `site.yml` did.
- **fmt once**, on Linux; the **release build** on Linux every code push. What differs on
  Windows in release — `windows_subsystem = "windows"` in `cantus-app` and `cantus-launch`
  (`cfg_attr(all(windows, not(debug_assertions)), …)`), and a few `cfg!(debug_assertions)`
  branches (a first version of this ADR said there were none: the review's finding) — is
  built by `windows-release.yml` (`cargo build --release -p cantus -p cantus-launch` on
  `windows-latest`) when those crates, the manifests, the lock file or the toolchain
  change, and by `release.yml` at every tag: ≈ 25 billed minutes only when it can break,
  instead of 24 on every code push. `CARGO_PROFILE_RELEASE_DEBUG=0` in CI (debug info
  changes no generated code).
- **`rust-cache` saves only from `master`** (`save-if`): pull requests read master's
  cache and do not evict it from the 10 GB budget with their own.

### Minutes per push, after

| A push that changes… | Runs | Billed (est.) |
|---|---|---|
| only documents | nothing | **0** |
| code (most pushes: UI, app, host, content…) | test ubuntu 35 + test windows 37 × 2 | **≈ 110** |
| the DSP/engine side, demos, presets, the lock file | + null hashes 30 × 2 + 20 + 1 | **≈ 190** |
| dependencies | + audit ≈ 2 | ≈ 192 |
| the app or launcher crates, manifests, toolchain | + Windows release build ≈ 12 × 2 | + ≈ 25 |
| **weekly, fixed** | macOS 1 + 30 × 10 (only if code changed that week) + audit 2 | ≈ 300 a week |

A code push goes from ≈ 590 to ≈ 110 billed minutes (−80 %); a documents push from
≈ 590 to 0. The weekly macOS run is now the largest single item (≈ 1 200 a month in busy
weeks): if the quota is still short, change its cron to monthly (`17 6 1 * *`) or delete
the `schedule` — one line — and run it by hand before tags. **The lasting fix is going
public**: GitHub's standard runners are free for public repositories, and macOS can then
return to every push (add `ci.yml`'s `push` trigger to `macos.yml`).

### 7. The installer under a stable name, and the button that downloads it

- `cantus_update::release::STABLE_INSTALLER_NAME = "Cantus-windows-x64-setup.exe"`, beside
  the versioned `installer_name(v)`. `xtask package` copies the **signed** installer to
  that name in `dist/` (same bytes) and `SHA256SUMS.txt` lists both. The updater is
  unchanged: it still looks for the versioned name, so no shipped copy is affected.
- `release.yml`'s publish job checks `dist/Cantus-windows-x64-setup.exe` exists before
  publishing.
- **A beta is "latest" until a stable release exists.** The publish job asks GitHub for the
  current latest release: if it is a stable version (no `-`), a new `-beta`/`-rc` tag is
  published as a pre-release; otherwise (none yet, or only betas) it is published with
  `--latest`. Only *404 Not Found* means "no latest release"; any other failure of the
  lookup (network, rate limit, outage) stops the job instead of guessing (the review's
  finding). So `https://github.com/luis314159/cantus-daw/releases/latest/download/Cantus-windows-x64-setup.exe`
  always answers. The updater never read the flag: a stable build refuses any version with
  a suffix (`Release::is_update_for`), and a pre-release build reads the release list
  (ADR 0032 §4) — so this changes no update offer.
- The website (`web/src/top.rs::DOWNLOAD`): the Windows card's primary button is
  *Download the installer* on that link; *All downloads, release notes and checksums*
  links the releases page (portable zip, plug-ins zip, sums). macOS, Linux and phones
  keep their answers.
- `package_ci_tests.rs::the_stable_installer_name_is_the_same_in_the_packager_the_workflow_and_the_site`
  (seen failing first): the packager's file list (`package_tests.rs`, which also checks
  the copy is byte-identical and in the sums when NSIS is present), `release.yml`'s check
  and the site's URL — built from `cantus_update::updater::REPO` — name the same file.
- **Until the repository and its releases are public, the link answers 404** (GitHub hides
  a private repository's releases), exactly as the old releases-page link did; the site
  deploys only from `master` (`site.yml`), when the owner merges.

### 8. Signing through SignPath: the staged packaging (built, inert until configured)

The PFX, store and Azure routes sign the NSIS uninstaller in the middle of makensis
(`!uninstfinalize` runs `xtask sign-file`; the Azure variables reach it through the job's
environment), so they need nothing more. SignPath signs remotely, after a request is
approved, so it cannot: the programs must be signed before makensis embeds them, the
uninstaller too, and the installer after — two requests, two approvals per release. Built
in round 26:

- `xtask package --stage-only`: build, check and lay out `target/package/stage/`
  (`app/`, `clap/`, `vst3/`) with `uninstall.exe` at its top — made by an
  **uninstaller-only NSIS pass**: the same script compiled with `UNINSTALLER_ONLY` (and
  a file list whose install macros are empty) into `cantus-uw.exe`, whose `.onInit` writes
  the uninstaller beside itself and quits.
- `xtask package --from-stage <dir>`: the zips, the installer — carrying the signed
  uninstaller as a file (`PREBUILT_UNINSTALLER`) instead of writing one — its stable copy
  and the sums, from a stage that came back signed. Nothing is built or signed.
- `xtask sums <dir>`: rewrite `SHA256SUMS.txt` after the signed installer replaced the
  unsigned one.

`docs/signing/signpath-release.draft.yml` wires them; it stays out of `.github/` until the
SignPath project exists. The job that submits to SignPath, and whatever builds what it
signs, runs without `rust-cache` (a clean build of the tagged source).

### 9. Every signed file names its product and version (added the same round)

SignPath requires every signed binary to carry its product name and version (*"set all
product name attributes to your project's name, all product version attributes to the
same value in each build"*). The programs and the launcher had them (`cantus-app/icon.rs`
through `winresource`), the installer too (`VIAddVersionKey`); **`cantus-scan.exe` and the
plug-in libraries did not**. Now `crates/cantus-host/build.rs` gives the scanner (product
*Cantus*, the workspace version, *Cantus plug-in scanner*) and `crates/cantus-plugins/build.rs`
each plug-in library (the same, described by the plug-in's full name, chosen from the one
effect feature `xtask bundle` switches on) a version resource; `winresource` links it into
every linked output of the package (the `cdylib`, the scanner, the test executables),
never into a crate that depends on it. `xtask package` (`package_meta.rs`) reads the
`StringFileInfo` of every file it is about to sign, and of the installer it built, and
refuses the release when one lacks *Cantus* or the version. Windows reads them back
(Explorer, `Get-Item … .VersionInfo`) on a bundled *Cantus Aequalis* and the scanner. The
NSIS uninstaller carries the installer's version resource and is checked too: by
`xtask sign-file` before it signs it (the PFX/Azure routes), and by `--stage-only` /
`--from-stage` (the SignPath route).

### 10. Only OSI-licensed components in what is signed (added the same round)

Two components were under licences the OSI has not approved, though neither was needed:

- **egui's default fonts** — Ubuntu Light (Ubuntu Font Licence) and Hack (Bitstream Vera
  licence), with Noto Emoji. No family chain ever used them: `install_fonts` kept from
  egui's list only the faces named `…emoji…`, which matched `emoji-icon-font` alone.
  `default_fonts` is now off for egui and eframe; `emoji-icon-font.ttf` (MIT) is bundled in
  `crates/cantus-ui/assets/fonts/` with the same 0.9 scale, so every chain is the same as
  before — one Plex face, then the emoji face. `tests/font_coverage.rs` checks every
  character of every locale file and every non-ASCII character or `\u{…}` escape in the
  interface's source against the chains (nine gaps that already existed are listed in it —
  see the report); 85 screenshots (views, verse, mixer; both languages, three themes,
  75–200 %) are byte-identical before and after (one mixer shot differs between two full
  runs of the same build and matches when rendered alone). The notices print the Plex
  and emoji-icon-font licence files from `assets/fonts/`.
- **`webpki-roots`** (the Mozilla root list, CDLA-Permissive-2.0, a data licence) came
  with `ureq`'s `rustls` feature in `cantus-content`, which takes its roots from the
  platform verifier and never read the list. The feature list is now
  `rustls-no-provider`, `_ring`, `platform-verifier` (ring as before, by ureq's own
  feature for it): the crate is gone from every target, a real HTTPS download of the
  starter pack through the corporate proxy works on Windows, and Linux builds and tests
  `cantus-content` and `cantus-update` with CI's flags.

- **Names in other scripts: the system's fonts, never ours to ship.** After the Plex and
  emoji chain, `font_fallback.rs` appends the operating system's own faces (Windows:
  Yu Gothic, Microsoft YaHei, Malgun Gothic, Nirmala UI, Leelawadee UI, Segoe UI, Segoe UI
  Symbol/Emoji from `%WINDIR%\Fonts`; Noto/DejaVu on Linux; the system's on macOS), read
  at run time and never bundled or distributed — no licence of ours is involved. Lazily:
  only when the kit lays out (or the user types, pastes or composes with an input method)
  a character no bundled face has. The interface thread only does set lookups; a loader
  thread discovers the candidate faces once, reads each face once (by file and collection
  index), keeps its character map for later characters and installs the faces that map
  what was asked, each once in every chain; the fonts are reinstalled by the next frame
  (measured here: a Japanese label had Yu Gothic on screen 55 ms later; the frame that
  asked was not held). Armed only by the real window — `boot`'s font stage — so tests,
  shots and benchmarks keep the bundled fonts and their pixels.
  egui does not shape text: Arabic and Hebrew show left to right and unjoined, Devanagari
  and Thai without reordering or conjuncts — a known issue in the release notes and the
  manual. Tests: `tests/font_fallback.rs` (scripts, unarmed = bundled only, system faces
  cover Japanese, Korean, Hindi, Thai, Arabic, Hebrew where present) and
  `tests/font_fallback_live.rs` (armed through the boot stage's call: a Japanese label
  brings the face without blocking the frame, IME preedit and commit ask, a repeated miss
  is O(1)), `tests/font_fallback_faces.rs` (a face's map serves later characters; each face
  once); pictures with `examples/shoot_scripts.rs`.
- **The ASIO SDK's driver-side files.** asio-sys compiled every `.cpp` in the SDK's
  `common/`, `host/` and `host/pc/`, including Microsoft's COM-server sample code
  (`combase.cpp`, `dllentry.cpp`: "All Rights Reserved", no licence) that only a *driver*
  uses. `third_party/asio-sys/build.rs` now compiles the three host files Cantus calls —
  `common/asio.cpp`, `host/asiodrivers.cpp`, `host/pc/asiolist.cpp`, Steinberg's, under the
  SDK's GPL-3.0 option (the review's finding). The app links with ASIO, `--audio-drivers`
  lists it, and the `enumerate` example loads the installed software drivers.

### 11. The installer shows the privacy statement and asks about the update check

SignPath's privacy condition asks software that transfers data to show its policy while
installing and to offer to turn it off there. Cantus collects nothing, but its daily update
check runs by itself, so the installer now has a *Privacy* page after the folder page: what
Cantus sends (the update check, sound packs and updates on request, FlexASIO when ticked),
where the installed `PRIVACY.txt` is, and *Check for updates once a day (recommended)*,
ticked unless Cantus's own `settings.json` already says `"check_daily": false`. The choice is
left in `%LOCALAPPDATA%\Cantus\installer-choices.txt`; Cantus applies it to its settings at
its next start and removes the file (`settings::apply_installer_choices`, called from
`Files::read`). A silent install or an update shows no page and writes nothing. The strings
are the app's (`update.installer.privacy_*`, both languages).

## Consequences

- A documents-only commit no longer gets a green check; a code commit's check is the
  Linux + Windows suite. macOS, the null hashes, audit and perf show in their own
  workflows when they run.
- macOS regressions can sit up to a week (or until someone runs it): the release checklist
  must include a manual macOS run.
- Adding a test that reads a document means adding the document to `ci.yml` — the test
  fails and names it.
- Every release has a second copy of the installer (≈ the installer's size more upload and
  storage) and two lines for it in `SHA256SUMS.txt`.
- Before a stable release exists, the beta carries GitHub's *Latest* badge.

## What the tests hold

- `package_tests.rs`: `the_uninstaller_is_written_prebuilt_or_made_alone` (the three
  modes in config, script and file list), `a_stage_reads_back_from_its_folder`, and with
  NSIS `the_uninstaller_is_made_alone_and_carried_by_the_installer_when_nsis_is_here` (the
  writer runs, the uninstaller names Cantus and the version, the installer compiles around
  it); `an_installer_compiles_when_nsis_is_here` compiles the privacy page in both
  languages.
- `cantus-ui` `settings::tests`: `the_installers_update_choice_is_applied_once`,
  `the_settings_file_says_check_daily_false_as_the_installer_reads_it`.

- `xtask/src/package_meta.rs`: `the_strings_are_read_from_a_version_resource`,
  `a_file_without_its_name_or_version_is_reported`; `package_tests.rs`:
  `a_release_stops_when_a_file_it_signs_has_no_version_resource` (seen failing with the
  check taken out), `a_release_whose_files_carry_their_version_resource_is_made`.
- `crates/cantus-host/tests/version_resource.rs`,
  `crates/cantus-plugins/tests/version_resource.rs` (Windows): the package's own linked
  output carries the resource (seen failing before the build scripts); the plug-ins' build
  script names every plug-in `desc.rs` has.
- `crates/cantus-ui/tests/font_coverage.rs`: every character the interface shows has a
  glyph in its chain (bar the listed gaps, which must leave the list once filled); egui's
  default fonts are not built in and every chain is one Plex face then the emoji face
  (seen failing before the switch).

- `xtask/src/package_ci_tests.rs`: `the_glob_reads_patterns_as_github_does`,
  `ci_runs_for_code_and_for_what_tests_read_but_not_for_documents_alone`,
  `every_file_compiled_in_runs_ci_when_it_changes`,
  `the_stable_installer_name_is_the_same_in_the_packager_the_workflow_and_the_site`.
- `xtask/src/package_tests.rs`: the release's file list has the stable name;
  `an_installer_compiles_when_nsis_is_here` checks it is the same bytes and in the sums.
- The workflows pass `actionlint` 1.7.12 (checksum-verified; shellcheck not run).
