# Round 20 — packaging (installer, signing, updates)

> **Round 21:** there is no ASIO flavour any more — one Windows edition has WASAPI and ASIO
> (`xtask package` fetches the SDK, builds with it and checks the program lists ASIO).
> See `docs/round21/asio.md`; the flavour tables below describe round 20.

Builder: **packaging**, branch `r20/packaging`. BACKLOG §6. ADR **0030** (renumber if another
second-wave builder took 0030). The owner's decisions and purchases are in
[`signing.md`](signing.md) and at the end of this file.

## What is done

### One command makes a Windows release

```text
cargo run -p xtask -- package                 # standard flavour: setup.exe, portable zip, plug-ins zip, SHA256SUMS
cargo run -p xtask -- package --all           # standard and ASIO flavours
cargo run -p xtask -- package --asio          # ASIO only (CPAL_ASIO_DIR; LIBCLANG_PATH found if LLVM is in Program Files)
cargo run -p xtask -- package --no-build      # lay out what is built
cargo run -p xtask -- package --no-installer  # zips only, no NSIS needed
cargo run -p xtask -- sign-test-cert          # a self-signed certificate into target/signing-test/
cargo run -p xtask -- package --sign-test     # sign everything with it
cargo run -p xtask -- package --sign          # sign with CANTUS_SIGN=pfx|store|azure (xtask/src/sign.rs)
cargo run -p xtask -- version                 # the workspace version (the release workflow checks the tag)
```

Out, in `target/package/dist/`:

| File | What |
|---|---|
| `Cantus-<v>-windows-x64-setup.exe` | the installer (NSIS, per user, no admin) |
| `Cantus-<v>-windows-x64-portable.zip` | `Cantus-<v>/` with `cantus.exe`, texts, `portable.txt`, `plugins/CLAP`, `plugins/VST3` |
| `Cantus-<v>-windows-x64-asio-setup.exe`, `…-asio-portable.zip` | the same, built with `--features asio` (into `target/asio`) |
| `Cantus-<v>-plugins-windows-x64.zip` | the CLAP and VST3 plug-ins alone, for other DAWs |
| `SHA256SUMS.txt` | `sha256sum -c` format, every other file |

Files: `xtask/src/package.rs` (stage, sign, zip, sums; the CLI), `nsis.rs` (generated
includes, finding `makensis`), `sign.rs`, `archive.rs`, `package_tests.rs`;
`xtask/installer/cantus.nsi` (the hand-written script), `flexasio.ps1`, `README.txt`.
`LICENSE-GPL` added at the root (the GPL-3.0 text `LICENSE` pointed to but which was
missing; SHA-256 `3972dc97…986`, the FSF's text, matched from two mirrors since gnu.org
was unreachable through the proxy).

### The installer (NSIS — why, in ADR 0032 §1)

Per user, no UAC: `%LOCALAPPDATA%\Programs\Cantus`, HKCU only. Pages: welcome, the GPL
(informative — no "I accept"), components, folder, progress, finish (run Cantus). English
and Spanish, chosen from Windows' language; the words come from
`locales/areas/update.{en,es}.txt` (`update.installer.*`), turned into `LangString`s.

Components: **Cantus** (required: program, Start-menu shortcut, uninstaller, Apps &
features entry, "Open with" registration); **Open .cantus projects with Cantus** (on: the
`Cantus.Project` ProgID with the exe's icon, `SHChangeNotify`); **Desktop shortcut**
(off); **CLAP plug-ins** and **VST3 plug-ins** (on) into
`%LOCALAPPDATA%\Programs\Common\CLAP\Cantus` and `…\VST3\Cantus` — the per-user standard
folders; ASIO flavour only: **FlexASIO driver (downloaded now)** (off; below). Choices are
remembered in HKCU\Software\Cantus and restored by the next install or update; a
component unticked on a re-install is removed.

The uninstaller removes exactly what `files.nsh` installed (generated from the staged
list; `RMDir` without `/r`, deepest first), the association only if it still points to
Cantus, and — only if ticked — `%LOCALAPPDATA%\Cantus` (settings, presets, autosaves).

Switches: `/S` (silent), `/UPDATE` (what the app passes), `/RELAUNCH` (open Cantus when
done). The installer waits up to 30 s for `cantus.exe` to close (then asks Retry/Cancel;
silent: exits with code 2).

### FlexASIO at install time (the owner's request of 2026-10-06)

Kept **in the installer** (ADR 0032 §1): an unticked component of the ASIO flavour. It runs
`flexasio.ps1` from the installer's temp folder: GitHub API `releases/latest` of
`dechamps/FlexASIO` → its `FlexASIO-<version>.exe` → download → must start with `MZ`
(a proxy's HTML page is recognised and never run), must have the size GitHub reports,
and the digest when GitHub provides one (FlexASIO's assets have none; the `.sigstore`
bundle is not verified — no verifier is shipped) → `Start-Process -Verb RunAs -Wait`.
Exit codes map to an install-log line (held by a proxy / failed / installed); a declined
UAC prompt or no network just skips it. Never in silent mode, never remembered.
**Proved here**: `flexasio.ps1 -DownloadOnly` fetched FlexASIO-1.10b.exe (13 749 698 bytes)
through the Zscaler proxy and verified it; the installer itself is unsigned upstream
(`NotSigned`). For the app, `cantus_update::flexasio` has the same checks (`parse`,
`installer`, `check`, `run_elevated`) for the audio-setup builder's first-run offer.
ASIO4ALL is never downloaded or bundled (its licence forbids redistribution).

### Signing

`xtask/src/sign.rs`: `signtool` from the newest Windows SDK (or `CANTUS_SIGNTOOL`), SHA-256,
RFC 3161 timestamp (`CANTUS_SIGN_TIMESTAMP`, default DigiCert's; `none`), the signer from
the environment only — `pfx` (file + password), `store` (thumbprint: tokens, cloud cards),
`azure` (Trusted Signing's dlib + metadata). Signs `cantus.exe`, every `.clap` and VST3
binary, the uninstaller (`!uninstfinalize` → `xtask sign-file`) and the installer, then
checks each with `Get-AuthenticodeSignature` (stops on `NotSigned`/`HashMismatch`). The
password is masked in the log (a test holds it). What to buy: [`signing.md`](signing.md).

### Updates (`crates/cantus-update`, new; `views/update.rs`)

- **Check**: on start, installed or portable copies with *Check daily* on (default)
  ask `api.github.com/repos/luis314159/cantus/releases/latest` — once a day after an
  answer, once an hour after a failure, with the ETag (a 304 is free against GitHub's
  limit). `User-Agent: cantus-update`, nothing else: **no telemetry**. A copy built from
  source never checks by itself (Help → *Check for updates…* still works).
- **Offer**: a notice at the bottom right (*Cantus 0.2.0 is available*, ✕ hides it for the
  session) opens the **Updates** window: status, the release's notes (Markdown read into
  headings, bulleted lines and paragraphs, word-wrapped with the kit; links keep their
  text, nothing fetched), *Download*, *Release page*, *Skip this version*, *Later*.
  Portable copies are told to unzip the new zip; source builds to use git.
- **Verify** (installed copies): size, GitHub's asset digest **and** the
  `SHA256SUMS.txt` line, not empty, not HTML; Authenticode `Valid` by the same publisher
  when the build was signed (`CANTUS_PUBLISHER` at build time). A failing file is deleted.
  A proxy's quarantine page is recognised: *held*, retried every 2 min, 6 times.
- **Install**: *Install when I close Cantus* (on) runs `setup.exe /S /UPDATE` as the app
  quits (`app.rs`, the close path); *Restart and update* adds `/RELAUNCH` and quits now.
- **Proxy**: OS TLS and trust store (`ureq` + `native-tls` = SChannel); proxy from
  Settings, else `HTTPS_PROXY`…, else Windows' fixed proxy. Verified live on this
  machine behind Zscaler for the GitHub API (curl and the PowerShell path).
- **Settings → Updates**: *Check daily* (LED switch), *Proxy* (text field), *Check now*
  with the status line. Stored as `settings.json` → `updates`; the schedule in
  `update.json`.
- **Portable mode** in the app: `portable.txt` beside `cantus.exe` → config and data in
  `data\` beside it (`app.rs::directories_fallback`, 4 lines).

### Release workflow (`.github/workflows/release.yml`, not triggered)

On a pushed `v*.*.*` tag (or by hand without publishing): fmt/clippy/test on Windows; the
tag must equal the workspace version; NSIS 3.11 and the ASIO SDK 2.3.4 downloaded and
checked against pinned SHA-256s; LLVM from the runner; signing when secrets exist (PFX or
Azure Trusted Signing); `xtask package --all [--sign]`; artifact; `sha256sum -c`;
`gh release create` with `docs/releases/<tag>.md` as notes if present (else generated),
`--prerelease` for `-rc` tags. The ASIO SDK hash pinned is that of the zip downloaded in
round 19 (`C:\projects\daw-r19\asio\asiosdk.zip`); if Steinberg's file differs, the
step fails loudly — update `ASIO_SHA256` after checking it.

## Tools on this machine

- NSIS 3.11 portable: `%LOCALAPPDATA%\cantus-build-tools\nsis-3.11\` (outside the repo),
  from SourceForge; the first download was Zscaler's quarantine page, the retry two
  minutes later a zip whose SHA-256
  `c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1` matches
  SourceForge's own listing. `xtask` finds it there (or `CANTUS_MAKENSIS`, the PATH,
  `Program Files (x86)\NSIS`). Elsewhere: download `nsis-3.11.zip`, check that hash,
  unzip into that folder.
- signtool: Windows SDK 10.0.26100 (already installed).

## Tests

See the report at the end of the round (`cargo test -p cantus-update -p xtask`):

- `cantus-update` unit (10): SemVer parsing/precedence (SemVer §11's chain), refusals;
  notes Markdown; schedule (daily, hourly retry, clock going back); state round trip and
  broken file; install kind detection; installer switches; Windows proxy parsing.
- `cantus-update/tests/check.rs` (15): a **recorded GitHub API answer** (ripgrep's
  latest, verbatim) parses, with GitHub's digests; a Cantus-shaped release (same
  structure, our names) names every file the package task makes; versions compare,
  drafts/pre-releases not offered; HTML/not-a-release/bad tag refused; sums both shapes;
  **a bad hash, a wrong size, a proxy page refused**; signature rule (valid, unsigned,
  hash mismatch, wrong publisher); end to end with a table network — check → offer →
  download → verify → ready, reuse on the next start without a request, ETag/304 a day
  later; a tampered download deleted; a held download retried until the file arrives;
  every failure's key; up to date, skipped, portable; Windows reads an unsigned program as
  `NotSigned`; FlexASIO's **recorded** latest release names its installer and the checks
  refuse a page, a zip, a wrong size.
- `xtask` (7): **the package task produces the expected file list** (dist names, every zip's
  listing, per-flavour exe, CRLF readme, sums verified); the uninstaller removes exactly
  what the installer writes (File/Delete symmetry, RMDir order, no `/r`); installer strings
  come from the locale area in both languages and every `$(CANTUS_…)` the script uses
  exists; an installer compiles when NSIS is present; signing args mask the password;
  Azure and store arguments.
- `cantus-ui`: settings round trip includes `updates`.

## Proved on this machine

- Both installers (standard and ASIO, with the FlexASIO component) compile with NSIS 3.11
  from the test's stand-ins (`an_installer_compiles_when_nsis_is_here`).
- A silent install of the standard one (`/S /D=<scratch>`), checked by hand: the four
  files and `uninstall.exe`; both stand-in plug-ins in the per-user CLAP and VST3 folders
  with their bundle layout; `.cantus` → `Cantus.Project` with the exe's icon and
  `"…\cantus.exe" "%1"`; Apps & features entry (name, version, uninstall string); the
  Start-menu shortcut; no desktop shortcut; the choices stored (Assoc 1, Desktop 0, Clap 1,
  Vst3 1). Then `/S /UPDATE` with `Clap` set to 0: the CLAP folder removed, VST3 kept,
  installed into the remembered folder. Then `uninstall.exe /S`: every key, folder and
  shortcut gone (all eight checks false). Exit codes 0.
- The ASIO installer, built with the uninstaller-signing hook (`CANTUS_TEST_SIGN_WITH`):
  `!uninstfinalize` ran `xtask sign-file` on the uninstaller NSIS wrote; installed
  silently, its `uninstall.exe` reads *signed by CN=Cantus Test Signing (not trusted)*;
  Apps & features says *Cantus (ASIO)*, `Flavor = asio`; uninstalled clean. (Found on the
  way: the hook runs in the script's folder, so the program path must be absolute — it is,
  `current_exe()`.)
- `xtask sign-file` signed an installer with the test certificate and a DigiCert RFC 3161
  timestamp through the proxy; one flipped byte reads `HashMismatch`; the certificate was
  gone from the user's store after export.
- `flexasio.ps1 -DownloadOnly` fetched and verified FlexASIO-1.10b.exe through the proxy.
- The GitHub API answered through the proxy (curl, and the recorded fixtures).
- **The real thing, end to end**: `cargo run -p xtask -- package --sign-test` built
  `cantus` (release), the ten plug-ins (`plugin` profile), signed `cantus.exe`, the 10 CLAP
  and 10 VST3 binaries, the uninstaller (hook) and the installer, and wrote
  `Cantus-0.1.0-windows-x64-setup.exe` (38.2 MB; 142 MB of payload, LZMA 28 %),
  `…-portable.zip` (59.7 MB), `…-plugins-windows-x64.zip` (49.1 MB) and `SHA256SUMS.txt`.
  Installed silently: `cantus.exe` (27 MB, signed), the texts, the signed uninstaller,
  10 + 10 plug-ins in the per-user folders; uninstalled clean.
- Not done here: the ASIO flavour of the real app (`--asio`/`--all`; it needs
  `CPAL_ASIO_DIR` — round 19 built it on this machine), and looking at the installer's
  pages interactively.

## Files touched outside my area

- `Cargo.toml` (workspace: member, default member, `cantus-update` dependency), `Cargo.lock`.
- `crates/cantus-ui/Cargo.toml` (`cantus-update`), `src/i18n.rs` (the `update` area, one
  line per locale), `src/settings.rs` (`UpdateSettings`, the round-trip test literal),
  `src/app.rs` (field + init, `views::update::frame`, `on_quit` in the close path, the
  portable data folder in `directories_fallback`), `src/views/mod.rs` (`pub mod update`),
  `src/views/transport.rs` (Help → *Check for updates…*), `src/views/workspace_settings.rs`
  (the Updates section after Saving), `examples/shoot.rs` (`update`, `update-ready`,
  `update-notice`, `update-held`).
- `LICENSE-GPL` (new, the GPL-3.0 text `LICENSE` already referred to).
- `.github/workflows/release.yml` (new; `ci.yml` untouched).

## Left

- A local `package --all` (the ASIO flavour of the real app), and a look at the real
  installer's pages (welcome, licence, components) in both languages.
- macOS (DMG, notarisation) and Linux (AppImage/Flatpak) packages — BACKLOG §6.
- An MSI for enterprise deployment, if ever asked for (same staging folder).
- Files a previous version shipped and the new one does not stay until uninstall.
- The `.sigstore` bundle of FlexASIO is not verified (no verifier shipped); its assets
  have no GitHub digest, so it is checked by type and size only.
- A PAC proxy script is not evaluated by the Rust updater (Windows' fixed proxy, the
  environment and a typed proxy are); the installer's FlexASIO step does honour PAC.
- `docs/releases/v<x.y.z>.md` is where release notes go (the workflow reads it); none yet.

## The owner's decisions and purchases

1. **Code signing** — see [`signing.md`](signing.md): apply to SignPath Foundation (free,
   OSS), or Azure Trusted Signing (≈ US$10/month, if eligible from Mexico), or a Certum Open
   Source certificate (≈ €30–70/year). Then set the repository secrets/variables listed in
   `release.yml`, and `CANTUS_PUBLISHER` to the certificate's CN — once, for good.
2. **When to release**: bump `version` in `Cargo.toml`, optionally write
   `docs/releases/vX.Y.Z.md`, then tag `vX.Y.Z` and push the tag. The workflow refuses a
   tag that does not match the version.
3. **Updates on by default** (*Check daily*): confirm; it sends nothing but an anonymous
   request to GitHub.
4. **Plug-ins installed by default** into the per-user CLAP/VST3 folders: confirm (some
   DAWs scan only the all-users folders; the README says how to add the folder).
5. **The ASIO SDK hash** pinned in `release.yml` comes from round 19's zip; confirm it on
   the first CI run.
