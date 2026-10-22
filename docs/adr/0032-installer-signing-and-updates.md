# ADR 0032 — The installer, code signing and updates

**Status:** Accepted · **Date:** 2026-10-05 · **Round:** 20 (packaging) · **Extends:** ADR 0001 (licence), ADR 0020 (our plug-ins outside Cantus), ADR 0025 §10 (ASIO)

> Numbering: 0027–0029 are the first wave's. Written as 0030 on `r20/packaging`; the
> samples builder's ADR kept 0030, so this one became 0032 at the round-20 merge.

## Context

Cantus could only be run from source. BACKLOG §6 asks for what every DAW a musician
downloads has: an installer, a `.cantus` file association, a portable copy, signed
binaries so Windows' SmartScreen does not warn people off, and updates that arrive with
their release notes — without telemetry, and working on the networks people actually
have, including corporate ones whose proxy inspects TLS and quarantines downloads.

## Decisions

### 1. NSIS, per user, no administrator rights

| | WiX (MSI) | NSIS (EXE) |
|---|---|---|
| Per-user install without UAC | possible (`Scope="perUser"`), but MSI's per-user mode has sharp edges (ALLUSERS, upgrade codes that cannot cross scopes) | the natural mode: `RequestExecutionLevel user`, HKCU, `%LOCALAPPDATA%\Programs` |
| Licence of the toolset | MS-RL; since v6 the binaries come with the Open Source Maintenance Fee for revenue-earning users | zlib/libpng — nothing to pay or track, ever |
| Toolchain | .NET SDK + `dotnet tool` (v4+), or the old v3 with .NET Framework | one 2.4 MB portable zip; `makensis` also runs on Linux (CI can cross-build) |
| Silent update from the app | `msiexec /i … /qn`, MajorUpgrade rules | the same `.exe` with `/S` — the updater runs the file it verified |
| Enterprise deployment (GPO/Intune) | native | possible (silent switches), not native |
| Uninstall precision | Windows Installer tracks every file | ours: the script is generated from the staged file list, so every `File` has its `Delete` (a test holds it) |

NSIS wins on what the brief and the owner ask for: per-user without admin rights, a free
toolset with no strings, a small single-file installer that is also the update payload,
and an English/Spanish UI from the app's own locale files. An MSI for enterprise
deployment can be added later from the same staging folder; nothing here prevents it.

**What it installs.** `cantus.exe`, `LICENSE.txt` (GPL-3.0), `COPYRIGHT.txt` (which crates
are MIT/Apache) and `README.txt` into `%LOCALAPPDATA%\Programs\Cantus`; a Start-menu
shortcut; an uninstaller (Apps & features, HKCU). Optional, as components: the `.cantus`
association with Cantus's icon (on), a desktop shortcut (off), the CLAP plug-ins (on) and
the VST3 plug-ins (on), each into a `Cantus` folder in the **per-user** standard folders
(`%LOCALAPPDATA%\Programs\Common\CLAP` and `…\VST3`, the CLAP and VST3 specifications'
user locations). The choices are remembered (HKCU\Software\Cantus) and kept by updates.
The uninstaller removes exactly what was written, the association only where it is still
ours, and — only if ticked — the user's data folder.

**Portable.** The portable zip holds the same files plus `portable.txt`; a copy with that
marker keeps settings, presets and autosaves in `data\` beside the executable
(`app.rs::directories_fallback`, three lines) and writes nothing to the profile or the
registry.

**ASIO — one edition** (amended in round 21; ADR 0025 §10). There is a single Windows
edition and it has every driver, WASAPI and ASIO; the round-20 `-asio` flavour and its file
names are gone (`cantus_update::release` names one installer and one portable zip, and the
updater no longer has a flavour). The installer removes the `Flavor` value an earlier ASIO
build stored. The ASIO SDK is GPL-3-compatible since 2025, so the binary can be shipped; the
SDK itself is never committed: `xtask asio-sdk` fetches it into a cache outside the
repository and checks its pinned SHA-256, and `xtask package` builds with
`ASIO_SYS_REQUIRE_SDK=1` and then asks the built program for its driver types, refusing a
release without ASIO.

**One program per processor level, and a launcher** (amended in round 22; ADR 0031 §5.3).
The install folder and the portable zip hold `cantus.exe` — a small launcher built for the
plain x86-64 baseline (`crates/cantus-launch`) — beside `cantus-x86-64-v3.exe` (AVX2, FMA,
BMI2) and `cantus-x86-64-v2.exe` (SSE4.2, POPCNT), `cantus-x86-64-v4.exe` too when a
release is made with `xtask package --v4`, and `cantus-scan.exe` (the plug-in scanner the
app starts from beside itself, which no release had carried). The launcher asks the
processor (and Windows, for the AVX register state) what it has, starts the highest level
whose program is there with the same arguments, waits for it and returns its exit code;
`CANTUS_CPU=v2` steps down, never up; `cantus.exe --launcher-info` says what it found; a
processor below v2 or a missing program is explained in a message box, in English and
Spanish. Everything that starts Cantus — the Start-menu and desktop shortcuts, the
`.cantus` association, "Open with", the finish page, `/RELAUNCH` — still names
`cantus.exe`, and the installer and uninstaller wait for **every** program of the folder to
close. The programs set the AppUserModelID `Cantus.Cantus` for their process and the
installer gives the shortcuts the same one (`appid.nsh`), so the window groups under the
shortcut and a pin starts the launcher. Still one installer, one portable zip and the
same file names: the updater does not change. Both programs make the same bits (the CI
job `null-hashes`).

**FlexASIO, fetched, never bundled** (the owner, 2026-10-06). The installer offers an unticked component, *FlexASIO driver (downloaded now)*. When ticked it runs
`flexasio.ps1` (embedded in the installer): it asks GitHub for FlexASIO's **latest**
release, takes its `FlexASIO-<version>.exe`, checks that what arrived is a Windows
program (`MZ`), not a proxy's page, of exactly the size GitHub reports (and its SHA-256
when GitHub gives a digest — FlexASIO's assets have none today, and its `.sigstore`
bundle needs a verifier we do not ship), and runs it elevated (UAC). Anything else is
skipped with a line in the install log and nothing is run; a silent install or update
never does it. PowerShell's `Invoke-WebRequest` was chosen over an NSIS download plug-in
because it uses Windows' own proxy settings, PAC scripts included, and certificate store
— the one way to behave as the browser does behind a corporate proxy — and NSIS needs no
third-party plug-ins. The same checks are in `cantus_update::flexasio` for the app's
audio setup (the audio-setup builder's first-run offer). ASIO4ALL is never downloaded or
bundled: its licence forbids redistribution, so the app only links to its page.

### 2. One task makes a release

`cargo run -p xtask -- package [--sign|--sign-test] [--no-build] [--no-installer]`
fetches and checks the ASIO SDK, builds the one edition with ASIO — the program once per
x86-64 level (v2 and v3; `--v4` adds AVX-512), the scanner and the launcher (round 22) —
checks each program names its level and lists ASIO and that the launcher starts one that does,
bundles the plug-ins (ADR 0020's `bundle`), stages the release,
signs, zips, generates `config.nsh`/`files.nsh`/`strings.nsh` beside the hand-written
`xtask/installer/cantus.nsi`, runs `makensis`, signs the installer, and writes
`SHA256SUMS.txt`. The file names are made in one place, `cantus_update::release`, which
the app's updater reads, so the two cannot drift.

### 3. Signing through the environment only

`signtool` (newest Windows SDK) signs `cantus.exe`, every plug-in library, the
uninstaller (`!uninstfinalize`, calling `xtask sign-file`) and the installer, SHA-256 with
an RFC 3161 timestamp. The signer is chosen by environment variables — a PFX file and its
password, a certificate in the store by thumbprint (hardware tokens, EV), or **Azure
Trusted Signing** through its `dlib` — so no secret is in the repository, and the log
masks the password. `xtask sign-test-cert` makes a self-signed certificate under
`target/` and `package --sign-test` signs a whole release with it: the pipeline is proved
end to end without buying anything. What the owner must buy is in
`docs/round20/signing.md`.

### 4. Updates: GitHub Releases, once a day, nothing sent

On start, an **installed or portable** Cantus with *Check daily* on (the default) asks
`GET https://api.github.com/repos/luis314159/cantus-daw/releases/latest` — at most once a day
after an answer, at most once an hour after a failure, with `If-None-Match` so a repeat
costs nothing against GitHub's 60-an-hour anonymous limit. A copy built from source never
checks by itself. The request carries `User-Agent: cantus-update` and nothing else: no
version, no machine, no identifier — **no telemetry**. Drafts and pre-releases are not
offered to a stable build. *(Round 22, the public beta:* a pre-release build — `0.1.0-beta.1` — asks
`GET /repos/luis314159/cantus-daw/releases?per_page=20` instead and takes the newest non-draft
release there, because `/releases/latest` never names a pre-release and a beta would never
hear of the next beta; a release build still asks `/releases/latest`. SemVer orders
`0.1.0-beta.1 < 0.1.0-beta.2 < 0.1.0`; `tests/check.rs` holds both.*) The release's notes (its Markdown, reduced to headings,
bullets and paragraphs; links keep only their text; nothing is fetched or rendered as
HTML) are shown in the update window. *Skip this version* is remembered.

### 5. Verify before anything runs

An installed copy downloads the installer into `…\Cantus\updates\` and checks:
not empty and not an HTML page; the size GitHub reported; the SHA-256 in the asset's
`digest` (GitHub computes it at upload) **and** the line in the release's
`SHA256SUMS.txt` — every hash present must match, and at least one must exist; and, when
the running build was signed (`CANTUS_PUBLISHER` set at build time), Windows'
Authenticode verdict must be `Valid` with that same publisher. A file that fails is
deleted. It installs **on quit** (on by default, a tick box), or now with *Restart and
update*: the installer runs with `/S /UPDATE [/RELAUNCH]`, waits for `cantus.exe` to
close, keeps the components chosen before, and reopens Cantus if asked. A portable copy is
told and given the release page.

### 6. Networks as they are

HTTPS goes through the operating system's TLS and trust store (`ureq` with `native-tls`:
SChannel on Windows). A corporate proxy that inspects TLS has its root installed by IT,
so the check works there exactly as the browser does — and **verification is never
turned off**. The proxy is the one typed in Settings, else the `HTTPS_PROXY` family, else
Windows' fixed proxy; a PAC script is not evaluated (a transparent client such as
Zscaler's needs none). A proxy that quarantines a download answers `200 OK` with its own
page: the updater recognises the page, deletes it, says so, and tries again every two
minutes, six times — about the ten minutes such a hold lasts.

### 7. A release is a tag

`.github/workflows/release.yml` runs on a pushed `v*.*.*` tag (the owner decides when):
tests (built with the ASIO SDK, as shipped), the tag must equal the workspace version, NSIS
and the ASIO SDK are fetched and verified by SHA-256 (the SDK through `xtask asio-sdk`), the
one edition is packaged and signed when signing secrets exist, and
`gh release create` publishes the files with notes from `docs/releases/<tag>.md` (or
GitHub's generated notes). `workflow_dispatch` builds without publishing.

## Consequences

- New crate `cantus-update` (GPL-3.0-or-later; `ureq`, `native-tls` — MIT/Apache).
  `xtask` gains `zip` (MIT) and depends on `cantus-update` for the names.
- `LICENSE-GPL` (the GPL-3.0 text) is added at the root: `LICENSE` already pointed to it,
  and a GPL binary must ship with it.
- Until a certificate is bought, releases are unsigned: SmartScreen warns on first runs,
  and updates are verified by SHA-256 only. Signing later changes no code.
- Old files of a previous version that the new one no longer ships stay in the install
  folder until uninstall (the uninstaller knows only its own list) — harmless today, worth
  a manifest of the previous install if files ever move.
- The macOS DMG/notarisation and Linux AppImage/Flatpak of BACKLOG §6 are not done.
