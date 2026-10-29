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
cd cantus
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
files are kept as the run's artifacts). Secrets, all optional: Windows signing as the
workflow's header lists; macOS `APPLE_CERT_P12_BASE64`, `APPLE_CERT_PASSWORD`,
`APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_PASSWORD` (without them the disk image is signed
ad hoc and not notarised: Gatekeeper asks the user to allow it once).

### macOS: `packaging/macos/build-dmg.sh`

On a Mac (Xcode's command-line tools, `rustup target add aarch64-apple-darwin
x86_64-apple-darwin`), from the repository's root:

```text
packaging/macos/build-dmg.sh 0.1.0-beta.7 target/dmg-out
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
bash packaging/linux/test-fallback.sh target/dist/Cantus-0.2.0-linux-x86_64.AppImage
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

## Where to start reading

[`CLAUDE.md`](../CLAUDE.md) holds the working conventions; [`STATUS.md`](STATUS.md) says what
exists and what is next; [`BETA.md`](BETA.md) the road to the first public beta;
[`adr/`](adr/) every structural decision.

## Notes for machines behind a corporate proxy

A proxy that scans downloads (Zscaler and the like) may answer with an HTML page for a few
minutes instead of the file. `xtask asio-sdk` refuses anything that is not the pinned zip;
wait and run it again. Do not disable TLS checks or go around the proxy.
