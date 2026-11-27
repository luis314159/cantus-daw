# Developing Cantus on a new machine

Everything needed to build, test and package Cantus is either in this repository or
fetched by a command below, so a fresh clone works like the owner's machine. What is
deliberately **not** in the repository — the ASIO SDK, build tools, test plug-ins, sample
packs, builds — is listed in [What lives outside the clone](#what-lives-outside-the-clone),
with where each comes from.

Windows 10/11 x64 is the platform Cantus ships on; Linux and macOS build for development.

## 1. Install the tools (once per machine)

| Tool | Why | How (Windows) |
|---|---|---|
| **Git** | the repository | `winget install Git.Git` |
| **rustup** | Rust | `winget install Rustlang.Rustup` (or https://rustup.rs). The repository's `rust-toolchain.toml` pins the compiler (1.97.1, with clippy and rustfmt); rustup installs it by itself the first time `cargo` runs in the clone. |
| **Visual Studio 2022 Build Tools** | the MSVC linker and C compiler (LAME, Vorbis and the ASIO glue are C/C++) | `winget install Microsoft.VisualStudio.2022.BuildTools`, then in its installer: *Desktop development with C++* (MSVC v143 and a Windows 11 SDK). |
| **LLVM** | `libclang.dll`, which generates the ASIO bindings | `winget install LLVM.LLVM` (into `C:\Program Files\LLVM`; elsewhere, set `LIBCLANG_PATH` to its `bin`). |
| nasm *(releases only)* | rav1e's assembly for WebM video, 2.8× faster with the same bytes (ADR 0088); `xtask package` refuses to build without it — developer builds work without it, slower | `winget install NASM.NASM` (then add `C:\Program Files\NASM` to `PATH`). Linux: `sudo apt install nasm` (`packaging/linux/deps.sh` installs it). macOS: `brew install nasm`. |
| NSIS 3 *(packaging only)* | the installer | `nsis-3.11.zip` from SourceForge, SHA-256 `c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1`, unzipped into `%LOCALAPPDATA%\cantus-build-tools\nsis-3.11\` (or set `CANTUS_MAKENSIS`). |
| signtool *(signing only)* | Authenticode | comes with the Windows SDK above. |

**Linux** (Ubuntu 22.04 or 24.04; ADR 0065): `bash packaging/linux/deps.sh` installs what
the build needs with apt — ALSA, PipeWire's headers (`libpipewire-0.3-dev`,
`libspa-0.2-dev`), clang and `libclang-dev` (bindgen makes the
PipeWire bindings), D-Bus, xkbcommon, Wayland, Xvfb (the plug-in window tests need an X
server: `xvfb-run -a cargo test`) and squashfs-tools (the AppImage). On 22.04, whose
PipeWire is 0.3.48, it also unpacks PipeWire 1.0.5's headers into
`target/pipewire-headers` and prints the `PKG_CONFIG_PATH` to export. Other distributions:
the same packages by their names. JACK needs nothing: it is opened at run time
(`third_party/jack-sys` builds without its pkg-config file).

## 2. Clone and fetch the ASIO SDK

```text
git clone https://github.com/luis314159/cantus-daw.git
cd cantus-daw
cargo run -p xtask -- asio-sdk
```

`asio-sdk` downloads Steinberg's ASIO SDK 2.3.4, checks its pinned SHA-256 and unpacks it
into `%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\` — outside the clone: it is
Steinberg's to publish and is never committed (ADR 0025 §10). It also checks that LLVM is
found. A zip downloaded by hand can be given with `CANTUS_ASIO_SDK_ZIP=<path>`.

Without the SDK, Cantus still builds and runs with WASAPI; ASIO is then missing and the
build says so in a warning. Release builds set `ASIO_SYS_REQUIRE_SDK=1` so that can never
ship by accident. **One build serves every driver** — never make feature-gated editions.

## 3. Build and run

```text
cargo run --release -p cantus                    # build and open Cantus
cargo run --release -p cantus -- --audio-drivers # prints "audio drivers: WASAPI, ASIO"
cargo run --release -p cantus -- --locale es     # in Spanish
```

Hosted plug-ins are scanned by a separate program the app starts from beside itself; build
it once with `cargo build --release -p cantus-host --bin cantus-scan` (the release package
ships it). Third-party plug-ins run in another one, the plug-in sandbox (ADR 0063): build it
beside the app with `cargo build --release -p cantus-sandbox` (a plain `cargo build` at the
root builds both). Without it, isolated plug-ins run inside Cantus and the status line says
so; `CANTUS_SANDBOX_EXE` points Cantus (and the tests) at another build of it. The first release build takes 10–15 minutes. `target\release\cantus.exe` is self-contained
(fonts, themes, icons, demo songs are inside); copy it anywhere.

A low-latency driver for testing: ASIO4ALL or FlexASIO (the app links both from
*Audio setup*), or the interface maker's own. Never let an ASIO driver load while its
interface is unplugged — some crash the process that loads them.

**The README's pictures** come from one command, headless (no screen, no audio device):
`cargo run --release -p cantus-ui --example shoot_readme` writes every one of them to
`docs/images/`. Add names to shoot only some, and a folder first to write them elsewhere:
`… --example shoot_readme -- <dir> fx- inst-`. The example's header lists what each picture
shows. Set `CANTUS_SALAMANDER_ARCHIVE` to the Salamander Grand Piano's `.tar.gz` (the
manifest's download) to show it in Exemplar; without it the picture is Exemplar's demo. With [oxipng](https://crates.io/crates/oxipng) installed (`cargo install oxipng`), it
also optimises them losslessly, about a fifth smaller. Look at every picture before you
commit it.

## 4. Test

```text
cargo test --workspace                       # everything (≈ 1 700 tests, 25–40 min cold)
cargo test -p cantus-engine --features fuzz  # the scheduler's fuzz
cargo clippy --workspace --all-targets       # must be clean
cargo run -p xtask -- notices --check        # third-party notices up to date
```

The whole suite is about 8 100 tests now. Run it with `--no-fail-fast`, so one failing
binary does not hide the rest (`docs/RELEASING.md`). cantus-ui's ~370 test executables
take about 25 GB. Where the disk cannot hold them, `bash tools/test/ui-batches.sh` runs
cantus-ui in batches of 20 and deletes each batch's executables after it runs.

The tools that run and check it (`tools/test/`, `tools/release/`):

- `bash tools/test/suite-windows.sh [out-dir]`: the whole suite on Windows in one go. It
  runs the workspace without cantus-ui, then cantus-ui in batches, with no debug info, and
  prints the disk and the totals. It took 93 minutes on 2026-11-20. In WSL the same is
  `tools/wsl/check.sh <branch> test` (Linux, below); it takes a branch name or a commit.
- `bash tools/test/disk-watch.sh /c`: beside a WSL suite. WSL's disk image is on C:, and it
  only grows.
- `bash tools/test/ab-compare.sh --crate … --test … --files …`: an optimisation's proof. It
  builds the same test binary twice: the named files as they are at a base commit, and as
  they are in the working tree. Then it runs an ignored test on both, alternately, compares
  their dumps byte for byte and shows both costs. Use it for the same bits with less CPU,
  as in beta.16's Fragor solver (`fragor_tests::cpu_synth_fuzz_board`).
- `pwsh tools/release/release-body.ps1 get|set <tag> <file>`: a published release's text on
  cantus-daw. `set` checks that GitHub then holds the file to the byte, and refuses a text
  that quotes the RVR1960 (public text carries the RV1909). Editing a release only with the
  owner's OK.
- `pwsh tools/release/gh-api.ps1 GET|POST|PATCH <path> [body] [-OutFile f]`: GitHub's API
  for cantus-daw with the token from Windows Credential Manager (target `cantus-daw-github`),
  read at the moment of use and never printed: runs, jobs, logs, a rehearsal's dispatch
  (`POST repos/luis314159/cantus-daw/actions/workflows/release.yml/dispatches '{"ref":"master"}'`).
- `pwsh -NoProfile -File tools/release/watch-run.ps1 -RunId <id>`: a release or rehearsal
  watched as its jobs finish, one poll every five minutes (L8); through a filter, use
  `grep --line-buffered` (a buffered grep hid two finished jobs on 2026-11-21).
- `bash tools/test/scala-archive.sh [dir]`: the Scala scale archive (version 94, 5 401
  files) through Cantus's Scala reader (round 42). It fetches the archive from its fixed
  Wayback Machine copy, checks its SHA-256, unpacks it outside the repository (it has no
  licence: never committed; default `$CANTUS_REFS/tuning`, the `tuning` set) and runs the `#[ignore]` test `tuning_scala_archive`. `SLOT=1`
  wraps the build in `tools/build/slot.sh`.
- `python -I tools/test/timing_tests.py crates/<crate>/src`: the crate's non-ignored tests
  that time something (L3: each must take the binary's turn, `crate::timing_turn` in
  cantus-dsp).
- `python -I tools/merge/keep_both.py <files>`: an integrator's additive conflicts resolved
  by keeping both sides (ours, then theirs), CRLF kept; refuses a diff3 base. Check the seam
  after (a blank line between two added sections).
- `bash tools/wsl/guarded.sh <ref> clippy|test [floor-GB]`: `check.sh` in WSL, shut down if
  C: falls under the floor (WSL's memory grows Windows' paging file on C:).
  `bash tools/wsl/list-clones.sh` and `tools/wsl/prune-clones.sh` (run inside WSL): the old
  CI clones' sizes, and their removal (a clone with uncommitted changes is kept).

On a Windows set to another language, building the plug-ins' library prints `warning:
linker stdout: Creando biblioteca … .dll.lib` (`linker_messages`): rustc hides MSVC's
"Creating library" line only in English. It is harmless (CI's English runners never show
it) but a local `RUSTFLAGS=-D warnings` build stops on it.

Ignored by default, run when the hardware or files are there:

- `cargo test -p cantus-engine -- --ignored` — a real audio device;
- `cargo test -p cantus-host --test real_instruments -- --ignored --nocapture --test-threads 1`
  with `CANTUS_PLUGINS` set — free instruments, listed with links and hashes in
  [instruments/TEST-PLUGINS.md](instruments/TEST-PLUGINS.md);
- `cargo run -p xtask -- perf` — the performance bench (`docs/perf/`).

Tests and the soak harness can keep their files away from your own settings with
`CANTUS_DATA_DIR=<folder>`.

### Sound references and measurement

The sound tools (`tools/soundlab`, `tools/arcus` and its study, `tools/listening` and the
`*_abx` examples, `tools/chorda-voicing`, `tools/tympanum-fit`, `tools/tympanum-real`, `tools/test/scala-archive.sh`)
measure against recordings that never enter the repository. They live in one folder outside
every round's folder, so deleting a round deletes no data; the catalogue rebuilds it on any
machine. [`tools/sound-refs/setup.md`](../tools/sound-refs/setup.md) is the walk-through.

- **The catalogue:** `tools/sound-refs/catalog.toml` — every set (36: Arcus's, the
  bowed-strings study's 27, the recorded strings' sources, `strings-hq` (r42 strings-refs: Iowa
  MIS 2012, Musopen's K465 session, the Good-sounds bass), the Scala archive, the lab's
  example, Tympanum's and Chorda's packs, the EBU loudness set and MDB Drums by hand), with
  its URLs, DOI, version, licence and what it allows (ship, ship with credit, measure only),
  the tools that read it and its sub-path. `files/<set>.tsv`: each file's size, SHA-256 and
  origin; `sources/`: the sets' own `SOURCE.md` notes (the licence verbatim).
- **Fetch:** `python -I tools/sound-refs/fetch.py list | fetch [set…] | verify [set…] |
  manual`. Remote zips are read by range requests (only the members a set takes), downloads
  resume, every file's first bytes and SHA-256 are checked, a proxy's HTML page is waited out,
  TLS and the proxy are never bypassed. `--packs [--cargo]` adds Cantus's own packs (copied
  from the app's folder when it holds them, else installed by the pack code).
- **The folders:** `CANTUS_REFS` (default `F:\projects\cantus-refs`; `~/cantus-refs` off
  Windows), `CANTUS_WORK` (scratch, renders, kits; default `cantus-work` beside it),
  `CANTUS_STUDY_REFDB` (the study's measured database, default `$CANTUS_REFS/derived/refdb`),
  `CANTUS_PY` (default the venv `$CANTUS_REFS/venv`). `tools/sound-refs/cantus_paths.py` and
  `cantus_paths.sh` are the one place each language reads them; the older per-tool names
  still override.
- **Python:** one venv from `tools/sound-refs/requirements.txt` (numpy, scipy, soundfile,
  matplotlib, pinned) serves every sound tool.
- **Derived data:** `bash tools/sound-refs/rebuild_refdb.sh [--pass5]` rebuilds the study's
  refdb from the references (validate → catalog → extract → phrases → aggregate → plots →
  cnsm, friction, bowing) and compares it with the distributions committed in
  `research/instruments-next/bowed-refdb/`; `make_abx.py`, `synth_bar.py refs` and
  `tools/content/strings/map.sh` remake the other derived sets.
- **Tympanum against real kits** (round 42, `tools/tympanum-real/`; `docs/round42/tympanum-real.md`): the `drums-*` sets (DrumGizmo's Aasimonster, Cantus's CC0 drum packs joined by `content_root.py`, Freesound's real playing by `freesound.py`, the Groove MIDI Dataset's MIDI), the `tympanum_real` example (any kit's hits, notes, DrumGizmo builds), `pipeline.py` (hits, grooves, compare), `transcribe.py` (real drumming to MIDI), `position.py`, `gmd_levels.py`, `law.py` (model 5's level law), `preview.py` and `report.py` (the owner's named previews and Spanish report). Remote zips whose chain Python's store refuses are read through curl (the system's store), never with `-k`.
- **By hand only:** the EBU loudness test set (tech.ebu.ch answers scripts with a browser
  challenge), the study's papers, MDB Drums: `fetch.py manual` prints the steps, `fetch.py
  record <set>` then writes their checksums.

### Linux, from Windows (what GitHub Actions' Linux jobs see)

CI builds with `RUSTFLAGS=-D warnings` on Ubuntu too, and code behind `#[cfg(windows)]`
leaves Linux-only warnings (dead code) that a Windows build never shows. Check them before
pushing, in WSL Ubuntu 24.04, without sudo:

```text
wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/projects/cantus/tools/wsl/fetch-deps.sh'          # once
wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/projects/cantus/tools/wsl/check.sh <branch> clippy' # or: test
```

`fetch-deps.sh` installs rustup and unpacks the `-dev` packages (ALSA, D-Bus, OpenSSL,
Wayland, xkbcommon, pkg-config, PipeWire, libclang) into `~/linux-deps/root`, and a
PipeWire daemon, WirePlumber and jackd2 into `~/linux-deps/rt`; `check.sh` clones the main
repository into `~/cantus-ci` (`CANTUS_CI_DIR=~/elsewhere` for a clone of one's own, as
parallel builders need), checks out the branch and runs clippy or the tests with CI's
flags. WSLg is an X server (and a Wayland one), so the plug-in window tests run there.
A machine set up before round 30 runs `fetch-deps.sh` once more: the build now needs
PipeWire's headers and libclang, which it adds.

To open Cantus's PipeWire and JACK hosts in WSL (there is no sound card: a null sink and
source, or JACK's dummy driver, run on a timer):

```text
wsl -d Ubuntu-24.04 -- bash -lc 'bash /mnt/f/projects/cantus/tools/wsl/check.sh <branch> test'
wsl -d Ubuntu-24.04 -- bash -lc 'bash ~/cantus-ci/tools/wsl/audio-servers.sh pipewire'   # or: jack
wsl -d Ubuntu-24.04 -- bash -lc 'bash ~/cantus-ci/tools/wsl/audio-servers.sh run env \
  CANTUS_TEST_DRIVER=PipeWire CANTUS_TEST_BUFFER=128 \
  cargo test -p cantus-engine --test devices -- --ignored live_streams --nocapture'
wsl -d Ubuntu-24.04 -- bash -lc 'bash ~/cantus-ci/tools/wsl/audio-servers.sh stop'
```

### macOS, from Windows (what the macOS runner compiles)

There is no Mac to build on, but every line of Rust a Mac compiles can be checked here
(ADR 0066). `rustup target add aarch64-apple-darwin x86_64-apple-darwin` once; then
`cargo clippy --target aarch64-apple-darwin -p <crate>` works for crates whose build
scripts compile no C (`cantus-rt`, `cantus-host`, `cantus-sandbox`…). For the whole
workspace, `tools/macos/xcheck.sh` (Git Bash, LLVM installed) stands in for the C compiler
of ring, ogg and LAME:

```text
tools/macos/xcheck.sh aarch64-apple-darwin --workspace --all-targets
tools/macos/xcheck.sh x86_64-apple-darwin --workspace --all-targets
```

It proves the code compiles, not that it runs: `macos.yml` runs the suite on GitHub's
`macos-latest` (Apple silicon) on every push to master, and builds the `.dmg`.

## 5. Package a release

```text
cargo run -p xtask -- asio-sdk                  # once per machine
cargo run -p xtask -- package                   # build, lay out, installer, zips, sums
cargo run -p xtask -- package --no-installer    # zips only (no NSIS needed)
cargo run -p xtask -- sign-test-cert            # a self-signed certificate for trying --sign-test
cargo run -p xtask -- bundle                    # our effects as .clap/.vst3 for other DAWs
cargo run -p xtask -- plugin-zips <out>         # each effect zipped on its own + all, for this system (the site's Plug-ins pages)
cargo run -p xtask -- manual                    # the manual as HTML into target/manual
```

Output goes to `target/package/dist/`. A release holds a small `cantus.exe` launcher
(`crates/cantus-launch`) that starts `cantus-x86-64-v3.exe` or `cantus-x86-64-v2.exe` by
the processor, plus `cantus-scan.exe` (the plug-in scanner) and `cantus-sandbox.exe` (the
plug-in sandbox, ADR 0063); each level builds in
`target/cpu-<level>`. `cargo run -p xtask -- null-hashes` proves the levels give the same
bits. A development build (`cargo build -p cantus`) is a single plain program. The real signing certificate (SignPath) is not set
up yet (`docs/BETA.md`). `.github/workflows/release.yml` does the same on a pushed
`v*.*.*` tag.

### A release: one tag, three systems (ADR 0067)

**Read `docs/RELEASING.md` first**: its procedure (candidate, rehearsal, checklist, tag) and
its lessons. The tag's first job, *preflight*, refuses it without
`docs/releases/v<version>.preflight.md` ticking every lesson and naming a green rehearsal.

1. Set the version in the workspace's `Cargo.toml` (and `web/src/top.rs`'s `VERSION`),
   write `docs/releases/v<version>.md` from `docs/releases/TEMPLATE.md`, commit, publish
   to `cantus-daw` (`tools/public/publish.sh`).
2. `git tag v<version> && git push origin v<version>` in `cantus-daw`. `release.yml` runs
   three jobs at once — Windows (`windows-latest`: fmt, clippy, the suite, then
   `xtask package`), Linux (`ubuntu-22.04`: clippy, the suite, `cargo build --release`,
   then `packaging/linux/build-appimage.sh <version> dist`) and macOS (`macos-latest`:
   clippy, the suite, then `packaging/macos/build-dmg.sh <version> dist`) — each checking
   that the tag is the workspace's version. A tag publishes only when all three pass; a
   job that failed for the runner's sake is re-run from the run's page.
3. The publish job checks each system's sums, joins every file into one release with one
   `SHA256SUMS.txt`, checks the three versioned files and their stable names
   (`Cantus-windows-x64-setup.exe`, `Cantus-linux-x86_64.AppImage`,
   `Cantus-macos-universal.dmg`) and creates the release. Within an hour the website's
   download card says *Available* for Linux and macOS (it asks GitHub which files the
   latest release holds), and every copy's updater finds its own file.

*Run workflow* on the Actions page builds and tests all three without publishing (the
files are kept as the run's artifacts): the rehearsal every tag needs first. Secrets, all optional: Windows signing as the
workflow's header lists; macOS `APPLE_CERT_P12_BASE64`, `APPLE_CERT_PASSWORD`,
`APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_PASSWORD` (without them the disk image is signed
ad hoc and not notarised: Gatekeeper asks the user to allow it once).

### macOS: `packaging/macos/build-dmg.sh`

On a Mac (Xcode's command-line tools, `rustup target add aarch64-apple-darwin
x86_64-apple-darwin`), from the repository's root:

```text
packaging/macos/build-dmg.sh 0.1.0-beta.12 target/dmg-out
```

It builds `cantus`, `cantus-scan` and `cantus-sandbox` for both architectures, joins them
with `lipo`, lays out `Cantus.app` (`packaging/macos/Info.plist`, the icon its build
script renders, `InfoPlist.strings` in English and Spanish, the manual and the licences in
`Resources`) and writes `Cantus-<version>-macos-universal.dmg`. With the five secrets in
the environment (`APPLE_CERT_P12_BASE64`, `APPLE_CERT_PASSWORD`, `APPLE_TEAM_ID`,
`APPLE_ID`, `APPLE_APP_PASSWORD`) it signs with the Developer ID certificate and the
hardened runtime (`Cantus.entitlements`), notarises the app and the image and staples
both; without them it signs ad hoc. `CANTUS_DMG_NO_BUILD=1` reuses the binaries already
built. `docs/mac-session.md` is the checklist for a session on a real Mac.

### Linux: the AppImage and the Flatpak

```text
bash packaging/linux/deps.sh                       # once (sudo; Ubuntu 22.04 for releases)
cargo build --release
bash packaging/linux/build-appimage.sh 0.2.0 target/dist
bash packaging/linux/test-fallback.sh target/dist/Cantus-0.2.0-x86_64.AppImage
```

`build-appimage.sh` (the interface `release.yml` calls, round 30) builds the two levels
(`target/cpu-x86-64-v{2,3}`) and the plug-in bundles, lays out `target/appimage/Cantus.AppDir`
(the launcher as `cantus`, both programs, `cantus-scan`, `cantus-sandbox`, the plug-ins,
the `.desktop` file, icons, AppStream metadata, the `.cantus` MIME type, the licence and
notices, and the libpipewire stand-in), and joins the pinned AppImage runtime (type 2,
no FUSE 2 needed) to a zstd squashfs of it. `CANTUS_APPIMAGE_QUICK=1` packs the
`target/release` program alone. The AppImage must be built on the oldest system it
supports (Ubuntu 22.04, glibc 2.35). `packaging/linux/README.md` says why each piece is
there; `packaging/linux/flatpak/` holds the Flatpak manifest (its header says how to build
it; Flathub's submission is the owner's).

## What lives outside the clone

| What | Where | Where it comes from |
|---|---|---|
| ASIO SDK | `%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\` | `cargo run -p xtask -- asio-sdk` |
| NSIS | `%LOCALAPPDATA%\cantus-build-tools\nsis-3.11\` | step 1 |
| Cantus's settings, autosaves, recovery journal, sample packs | `%LOCALAPPDATA%\Cantus\` (a portable copy keeps them beside its exe) | made by the app; packs download on demand from `crates/cantus-content/manifest/content.json` (ADR 0030) |
| Test instruments | any folder named by `CANTUS_PLUGINS` | [instruments/TEST-PLUGINS.md](instruments/TEST-PLUGINS.md) |
| Sound references (recordings, measured data, the sound tools' venv) | `CANTUS_REFS` (`F:\projects\cantus-refs`) | `python -I tools/sound-refs/fetch.py fetch` (§4, *Sound references and measurement*) |
| The sound tools' scratch, renders, listening kits and their keys | `CANTUS_WORK` (`F:\projects\cantus-work`) | the tools themselves |
| Builds | `target/` (git-ignored) | the commands above |

## Environment variables

| Variable | Effect |
|---|---|
| `LIBCLANG_PATH` | the folder with `libclang.dll`, if LLVM is not in `C:\Program Files\LLVM\bin` |
| `CPAL_ASIO_DIR` | use an ASIO SDK folder of your own instead of the cache |
| `CANTUS_ASIO_SDK_ZIP` | the SDK zip, if `asio-sdk` cannot download it |
| `ASIO_SYS_REQUIRE_SDK=1` | fail the build instead of building without ASIO (releases) |
| `CANTUS_MAKENSIS` | the path of `makensis.exe` |
| `CANTUS_DATA_DIR` | where Cantus keeps its settings and autosaves (tests, soak) |
| `CANTUS_PLUGINS` | the test instruments' folder |
| `CANTUS_GOLDEN_DUMP` | print the measured size of a golden-hash change |
| `CANTUS_REFS`, `CANTUS_WORK`, `CANTUS_STUDY_REFDB`, `CANTUS_PY` | the sound tools' references, scratch, the study's refdb and Python (`tools/sound-refs/setup.md`) |

## Off-line tools (round 40): voicing, measuring, listening

Not part of the build; each folder's README has the setup, the exact commands and what the
output means. Python ones take their packages from the folder's `requirements.txt` and run with
`python -I`; none names a machine's folders (environment variables with defaults).

| folder | what |
|---|---|
| [`tools/build/`](../tools/build/README.md) | `slot.sh`: at most three cargo builds (and one WSL check) at once on a machine shared by several builders |
| [`tools/chorda-voicing/`](../tools/chorda-voicing/README.md) | the voicing lab: Chorda against the Salamander — a candidate's spec and tables (`SPEC.md`, `KNOBS.md`), the two analyses and their scores, the tuning gate, the probes, the search, the previews, shipping a candidate as Rust |
| [`tools/tympanum-fit/`](../tools/tympanum-fit/README.md) | Tympanum: fitting recorded kits into model 5's tables, the calibration cycles, the hybrid's residual, the drum analyses' cues (`accept_b10.py`), the representation ladder |
| [`tools/arcus/study/`](../tools/arcus/study/README.md) | the bowed-strings reference study (round 42): real cellos, basses, violas and violins measured by one estimator into bands (`research/instruments-next/bowed-refdb/`) that Arcus's gates judge it by; recordings under `CANTUS_REFS` (or `CANTUS_STUDY_DOWNLOADS`), never in the repository; `tools/sound-refs/rebuild_refdb.sh` rebuilds the per-note records |
| [`tools/listening/`](../tools/listening/README.md) | the blind tests — one page for every instrument (`crates/cantus-ui/examples/abx_kit/`: `abx.html` with a subject file per instrument, `abx_browser_check.mjs`), Chorda's `chorda_abx`, <!-- r42:tymp-abx --> Tympanum's `tympanum_abx` (modelled and hybrid against DRSKit / Virtuosity, through the engine), the ladder's kit —, publishing a kit as a claude.ai page, reading results (`abx_read.py`, per item), <!-- r42:strings-refs --> `strings_hq_pairs.py` (the owner's named pairs: an Iowa 2012 note and a Musopen phrase beside Arcus's, loudness-matched, 16-bit) |
| [`tools/soundlab/`](../tools/soundlab/README.md) | <!-- r42:soundlab --> the sound lab (round 42, ADR 0160): every instrument measured against recordings and blind-tested from **one subject file** — `lab.py check|refs|render|measure|report|all|kit` (the references' catalogue under `SOUNDLAB_REFS`, default `CANTUS_REFS`, never in the repository; the held-out band report as JSON and HTML in `SOUNDLAB_WORK`, default `$CANTUS_WORK/soundlab`; the blind kit with ABX, preferences and MUSHRA on the shared `abx_kit` page), the documented estimators (`soundlab/features.py`: BS.1770 loudness, YIN pitch, harmonics and inharmonicity, centroid and its trajectory, noise between harmonics, attack, t_steady, decay per partial and per band, vibrato and its jitter, stereo, room), `adapters/study_catalogue.py`, <!-- r42:strings-refs --> `adapters/strings_hq.py` (a family's catalogue plus the `strings-hq` set) and `adapters/strings_hq_phrases.py` (`pick`: the Musopen K465 spot-mic windows where each player dominates, plus HSD's stems, as a subject's `phrases.json`; `measure`: the phrase measures), `subjects/arcus-cello/`, `arcus-violin/`, `arcus-viola/`, `arcus-bass/` (each with a README of how it was built) and the worked example `subjects/example-tibia/`; `tests/run.py [--light]` (EBU Tech 3341/3342, BS.1770-4's coefficients, TinySOL's pitches, the study's records); the protocol a new instrument follows is its README's |
| `cantus-dsp/examples/fides_sounds.rs`, `cantus-ui/examples/fides_listen.rs` | Fides (round 41, ADR 0150): every exciter on every resonator's level and partials straight from the DSP (`-- levels`, `-- describe <exc> <res>`, `-- wav <dir>`); every factory preset rendered through the engine as WAV and its level fitted to its role (`-- <dir> [fit]`) <!-- r41:fides --> |
| `crates/cantus-ui/examples/nitor_listen.rs` | <!-- r41:nitor --> Nitor's presets for the ear: each beside its dry source, both at −18 LUFS, as WAV pairs (`cargo run -p cantus-ui --example nitor_listen -- <dir>`) |
| `crates/cantus-ui/examples/shoot_nitor.rs` | <!-- r41:nitor --> Nitor's design sheet: its states in Slate English and Paper Spanish, 75–200 %, at rest and playing (`cargo run -p cantus-ui --example shoot_nitor -- <dir>`) |
| `crates/cantus-ui/examples/shoot_tuning.rs` | <!-- r42:tuning-face --> Options ▸ Tuning…'s design sheet: its states (equal, a temperament, the same on a narrow screen, following the song's key, a Scala scale, a track's own), the inspector's override row and Chorda's tuning line, in Slate Lapis Night and Paper at 100 and 200 % (`cargo run -p cantus-ui --example shoot_tuning -- <dir> [kind…] [--zoom=1,2] [--theme=night\|paper] [--lang=es]`) |
| `tools/content/strings/` | <!-- r42:strings-rec --> the recorded strings' packs (ADR 0158): `fetch_sources.sh <dir> [pack]` fetches VSCO 2 CE's strings, the Bigcat cello, the Smolken bass, Meatbass and Sneakybass outside the repository (checking each file's first bytes behind the proxy); `map.sh <dir> [pack]` measures every sample (`cantus-content/examples/strings_map`: the octave fix by YIN, each zone's tune from the partials' fit and YIN on what it plays, the loop and its click check through Exemplar) and `merge.py` writes the packs into `content.json` (with `[deltas.json]`, the levels); `render.sh <dir> <out>` renders the owner's listening files and the ABX phrases, and `cargo run --release -p cantus-ui --example strings_listen -- fit <dir> <deltas.json>` levels every preset the house's way; `cantus-content/examples/strings_tune_probe.rs` compares the pitch measures (`CANTUS_PROBE_FILES` for zones one by one) |
| `crates/cantus-ui/examples/shoot_strings_rec.rs` | <!-- r42:strings-rec --> the piano roll's articulation lane and the note's menu, Night and Paper, English and Spanish, 100 and 200 % (`cargo run -p cantus-ui --example shoot_strings_rec -- <dir>`) |
| [`tools/arcus/`](../tools/arcus/README.md) | <!-- r42:arcus --> Arcus, the bowed string (round 42, ADR 0157): `listen.sh` (the owner's renders: the cello and the double bass, notes at three dynamics, a legato phrase with a CC1 swell, pizzicato, tremolo), `abx.sh` (the blind ABX's phrase `abx_phrase.mid` — written by `abx_phrase.py` — rendered at −1 dBFS and at −23 LUFS), `compare.sh` / `examples/arcus_compare.rs` (a render beside a recording: harmonics, the noise between them, centroid, onset), `presets.sh` (every preset's level on its role's phrase; `fit` sets it), `match.sh` (Arcus measured against recorded cellos and basses; `fit` fits model 2's match), `study_compare.py` (Arcus by the reference study's estimator against the band of real instruments), `pitch-table.sh` (the bowed-pitch table), `gate.sh` (the gate's probes); `pass4/` (round 42 pass 4, `CANTUS_PY`: `measure.sh NAME` renders the held-out notes and runs `study_compare.py`, `table.py A.json B.json` sets two such measures side by side, `bow_kinematics.py` the cellist's bow strokes from Pàmies-Vilà's motion capture, `lampis_attacks.py` a Guettler diagram from Lampis's cello-G attacks, `lampis_harmonics.py` the real string's harmonics against force, β and speed, `harmonics_by_register.py` and `pizz_detail.py` Arcus's harmonics per register and its plucks' T60 and thump against the references) |
| [`tools/arcus/`](../tools/arcus/README.md) (the upper strings) | <!-- r42:arcus-upper --> Arcus's violin and viola (round 42, `docs/round42/arcus-upper.md`): `upper-body.sh` (the violin's body fitted to the CNSM dataset's measured bridge admittance, written into `arcus_upper_body_data.rs`), `upper-pitch.sh` (their bowed-pitch table at their voices' rate), `upper-compare.sh` (Arcus beside the Iowa MIS and VSCO 2 CE recordings, each feature's distance beside the recordings' own), `upper-listen.sh` (the owner's renders and the blind ABX's pairs at −23 LUFS, `abx.html` set up beside them), `upper-measure.sh` + `upper_study.py` (the study's own estimator on Arcus's notes against the real instruments' band) |
| [`tools/arcus/demo.sh`](../tools/arcus/README.md) | <!-- r42:arcus-demo --> Arcus heard as music (round 42): an original *Andante cantabile* in D major for violin, viola, cello and double bass, the instruments entering and leaving (`examples/arcus_demo/`) — built in a running app through the picker, the preset browser and the commands, saved as `.cantus`, exported through the app's export (48 kHz 24-bit, −16 LUFS under −1 dBTP) with stems and a Spanish `index.html`; checks a second export and the saved project's are the same to the byte, and measures levels, each instrument's audibility per section, clicks at note boundaries and the hall's tail |
| `tools/arcus/fiddle/` | <!-- r42 fiddle --> the country fiddle (round 42, ADR 0164, `docs/round42/fiddle.md`; `CANTUS_PY`, `python -I`): `fiddle_measure.py run|table` (the fiddle study: real fiddling — the `fiddle` reference set, `tools/sound-refs` — against classical violins and our renders: fast notes' bursts, settling, rings and re-articulation, double stops, rhythm, vibrato, intonation, timbre, room; `fiddle_lib.py`, `fiddle_groups.py`, `fiddle_report.py`, `fiddle_truth.py` its parts), `fiddle_quick.py` (the headline measures for a few files beside the references'), `fiddle_ltas.py` (third-octave spectra beside the references'), `transcribe.py` (a reference phrase as notes for a comparison clip, the owner's ear only), `listen_page.py` (the Spanish listening page and the 16-bit chat copies); with `examples/fiddle_lab.rs` (`cargo run -p cantus-dsp --release --example fiddle_lab -- <dir> [key=value …]`: the lab's phrases before and after, both sides summed) and `arcus_demo -- <dir> fiddle` (the songs; `FIDDLE_TREN_AEAE=1` for *Tren* in AEAE) |
| `tools/arcus-figures/` | <!-- r42:arcus-upper --> Arcus's realistic musicians (the woman, the man): `build_figures.py <makehuman/data> <system assets> crates/cantus-ui/assets/figures [--preview DIR]` builds `woman.fig` / `man.fig` from MakeHuman 1.x's CC0 assets (the base mesh, targets, skeleton and weights from github.com/makehumancommunity/makehuman, the suit, the shoes and the skins from `makehuman_system_assets_cc0.zip`; numpy and Pillow), `mh.py` (the assets' formats), `preview.py` (a small rasteriser to look while building), `sheet.py` (a shot or a crop beside a reference photograph for the design sheet), `sheet_v7.py` (sheet v7: every instrument with both figures, the side-by-sides and the hands' crops), `thin.py` (thins the live arms' meshes, called by `build_figures.py`), `hand_rig.py` + `hand_design.py` (a bow hold designed away from the app: `arm_aims` mirrored, rendered from three sides) and `hand_left.py` (a left-hand pose dumped by `CANTUS_DUMP_HAND=1` from `tests/arcus_hold.rs`); `left_hand.py` (the violin's and the viola's left hand authored from data: `solve <dump> --only=<who>-<inst>` per process, `pack <dump>` writes `assets/figures/left_hand.bin`, `look <dump> <dir> who inst string finger at` renders a row from four cameras; the dump from `CANTUS_HAND_DUMP=<file> cargo test -p cantus-ui --test arcus_hand_table -- --ignored dump`), `hand_sheet.py` (the 4-up: photograph 4, the app from its angle through `shoot_arcus --cam=`, the app's own camera at 200 %, photograph 3), `nup.py` (pictures side by side, labelled), `build_figures.py --only=woman|man` (one figure); <!-- r42:arcus-hair --> `hair_maps.py <in.png> <out.png> [--view <png>]` (a hair card's texture as its maps: strand detail without the large-scale light, the strands' way, coverage; called by `build_figures.py`), `hair_sheet.py <shoot_arcus.exe> <dir> [<sheet-v8 dir>]` (the faces and heads at 200 %, Photographic, Night and Paper, and before/after against sheet v8); <!-- r42:arcus-hand --> `hand_study.py <dump.json> <dir> [who inst string finger at]` (a solved left-hand pose from the app's camera, photograph 5's, 2's and 4's, with the photographs' picture measures — `left_hand_refs.md` holds the photographs' numbers; `HAND_AZ`/`HAND_EL` turn the app's camera), `hand_sheet_v10.py <shoot_arcus.exe> <dump.json> <dir> [--old-dump=<dump>] [--refs=<dir>] [--look=] [--theme=] [--only=who-inst]` (the app's own picture round the left hand from the earlier camera, the new one and photograph 5's angle, the hand and sleeve, the whole picture from both cameras; with `--refs` the photographs beside — keep those sheets outside the tree), `lids.py [--dir=<figures>] [--preview <dir>]` (the closed eyelids' morph, `assets/figures/<who>_lids.bin`; run after every build of the figures — it names vertices by index); <!-- r42:arcus-polish --> `measure_arms.py <makehuman/data> <system assets> [--only=woman; <!-- r42:arcus-hands --> <!-- r42:arcus-hands pass 2 --> `hand_solid.py` (the neck, fingerboard, strings and body from the table's dump as a sampled signed distance — `left_hand.py` keeps the skin out of it; `left_hand.py pen <dump> <dir> <who> <inst> <string>,<finger>,<at>` draws what goes in, red; `solve … --strings=0,1 --seed=<dir>` and `fix … --strings= --ats= --worse= --keys=` split and target the work), `hand_check.py <dump> [--pen]` (the solved rows that break the gates' rules, as `--keys=` for `fix`), `hand_cam.py <dump> <who> <inst> <string> <semis> <half>` (`shoot_arcus --cam=` for the app's own camera zoomed on the left hand, ~400 % at half 8 with `--scales=2`), `hand_zoom.py` (crops side by side), `hand_sheet_v14.py` (sheet v14 on the camera of d630eb474, first, third and high position and the bow hand); <!-- r42:arcus-hands pass 3 --> `finger_profile.py <figures> [woman|man]` (each finger's breadth and depth along it, and its nail, as the app loads the figure — the numbers `build_figures.py::finger_form` is set against), `left_hand.py fix … --dense` and `hand_check.py <dump> --pen --dense` (every skin point kept out of and checked against the instrument, as `arcus_hand_table`'s gate measures it), `CANTUS_HAND_TUNE="a,b,_,d…"` (the hands' skin's numbers in `views/inst/arcus_draw_skin.rs` overridden in `shoot_arcus` while designing: 0–2 the gains, 3–5 the sheen, 6–8 the blood tint, 9 the tips, 10 the knuckles, 11 the creases' floor); `hand_sheet_v12.py <before shots> <after shots> <dir> [--refs=<dir>]` (both figures' left hand in first and high position and the bow hand at 100 % — own pixels and 2x nearest — and 200 %, Night and Paper, both looks, before and after; with `--refs` the photographs beside — keep those sheets outside the tree); `left_hand.py` and `hand_design.py` read the figures from `CANTUS_FIGURE_DIR` as the app does, and the app reads `left_hand.bin` from it too (solve, pack and shoot without a compile); `CANTUS_BOW_GRIP="kc=a,b,c;hd=..;lean=..;mid=..;hang=..;slant=.."` overrides the bow hold's numbers (`scene/figure/grip.rs::BOW`) in `shoot_arcus` while designing it |man]` (the figures' arm anthropometry: segment lengths, the skin's and the sleeve's girths at the deltoid, the upper arm, the elbow, the forearm and the wrist), `polish_sheet.py <before shots> <after shots> <out> [--numbers=<file>]` (before/after sheets of the violinists from `shoot_arcus` output: the whole picture at 100 %, crops of the face, ear, neck, hands and arms at 200 %, both looks and themes); `CANTUS_FIGURE_DIR=<folder>` makes the app read `woman.fig`/`man.fig` and their lids from a folder `build_figures.py` wrote instead of the embedded ones — look at a rebuilt figure in `shoot_arcus` without compiling (unset in the app) |
| [`tools/video/`](../tools/video/README.md) | the launch video (round 41), from a clean machine to the cut — `voice/`: the takes (`takes_eleven.py` with its character ledger through `eleven-api.ps1`, whose key stays in Windows Credential Manager; `eleven_voices.ps1`; Chatterbox `takes.py`/`breathe.py`/`ref.py`; Qwen `takes_qwen.py`/`qwen_run.sh`), the gate (`gate.py`: Whisper word for word, levels, pace), the listening pages (`abpage.py`, `compare.py`, `page_shot.sh`), the chosen track (`choose.py`), the pins (`requirements*.txt`, `models.txt`, `fetch_models.sh`); `cards/`: the titles and end card (HTML rendered by headless Chrome, `render.mjs`, `view.py`); `film/`: `draft.sh`, `draft3.sh`, `int_worktree.sh`, `gently.sh`, `watchdog.sh`, `check_audio.sh`, `cut_frames.sh`, `heart_diff.sh`, `sheet.sh`, `review_sheet.sh`, the pinned ffmpeg (`ffmpeg.txt`); with `crates/cantus-ui/examples/film.rs` (the picture) and `soundtrack.rs` (the mix, in Cantus) |
| [`tools/social/`](../tools/social/render.mjs) | the social accounts' images (`docs/social/`): avatars, the YouTube banner, watermark and the launch video's thumbnails, the 3:1 headers — `social.html`/`.css`/`.js` drawn in Lapis Night, rendered by headless Chrome at each platform's size (`node tools/social/render.mjs <out-dir>`, through `web/tools/cdp.mjs`), and `sheet.png`, every image with its circle crop, safe area and covered corner marked (`sheet.html`) |

The design for round 41's micro-tuning is [`research/instruments/microtuning.md`](../research/instruments/microtuning.md).

## Where to start reading

[`CLAUDE.md`](../CLAUDE.md) holds the working conventions; [`STATUS.md`](STATUS.md) says what
exists and what is next; [`BETA.md`](BETA.md) the road to the first public beta;
[`adr/`](adr/) every structural decision.

## Notes for machines behind a corporate proxy

A proxy that scans downloads (Zscaler and the like) may answer with an HTML page for a few
minutes instead of the file. `xtask asio-sdk` refuses anything that is not the pinned zip;
wait and run it again. Do not disable TLS checks or go around the proxy.
