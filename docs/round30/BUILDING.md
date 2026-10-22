# Round 30 — Cantus on three systems

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule:** every gap a feature leaves is closed in the same round. Whatever you
would write under "Left" comes back to you as work; only list as open what truly needs the
owner (a Mac in hand, their ears, their hardware, a paid account, a decision).

**This round is run lean (the owner, 2026-10-22: few tokens left).** Three builders, small
reviews. Read only what your brief needs; do not re-read whole crates; prefer `grep` and
line ranges; keep reports short and exact.

The ask (2026-10-22): releases of Windows, Linux and macOS at the end of the round.

1. **Linux, a complete product**: low-latency audio through PipeWire and JACK (ALSA as the
   fallback), real-time priority in `cantus-rt` (rtkit / `SCHED_FIFO`), plug-in editor
   windows on X11 and Wayland, the plug-in sandbox proven end to end, the scanner and its
   folders, file dialogs and system fonts, an **AppImage** built in CI (Flatpak after), the
   website's download card.
2. **macOS, everything that can be prepared without a Mac** (BACKLOG §2, "Prepared without
   a Mac"): the `.app`, its icon and the microphone permission, a universal binary,
   `cantus-scan` and `cantus-sandbox` inside the bundle, the `.dmg` built in CI; signing
   with the hardened runtime and notarisation in the workflow (secrets as placeholders);
   CoreAudio with the engine's threads in the device's audio workgroup; the sandbox without
   futex and a helper that dies with Cantus; editor windows in Cocoa; the ⌘/⌥ keymap
   preset, the global menu bar, trackpad and Retina; the suite and the fuzz green on the
   macOS runner and the golden hashes **identical** on the three systems.
3. **A short checklist for the owner's session with a borrowed Mac.**
4. **Releases:** one tag publishes the three installers on `cantus-daw`; the website says
   "Available" for Linux and macOS only once their release exists; the updater knows which
   file to ask for on each system.
5. The owner's frictions (none given at the start).

Read, in order: this file (the rules and **your** section); `CLAUDE.md`; `docs/STATUS.md`'s
header and round 29's section; `docs/BACKLOG-excellence.md` §2; ADR 0031 §2.6–2.7 (real
time), 0019 (editor windows), 0063 (plug-ins out of process), 0032/0048/0049 (releases, CI,
the public repository) as your brief needs.

## The builders

All three start from the contract commit on `r30/integration` and run at once (never more
than three compiling: a 31 GB machine).

| Builder | Branch / worktree | Delivers | ADR |
|---|---|---|---|
| **linux** | `r30/linux`, `F:\projects\daw-r30\linux` | item 1 (+ Linux's part of item 4: the AppImage script) | 0065 |
| **macos** | `r30/macos`, `F:\projects\daw-r30\macos` | item 2 and item 3 (+ the `.dmg` script) | 0066 |
| **ship** | `r30/ship`, `F:\projects\daw-r30\ship` | item 4: the release workflow for three systems, the updater per system, the website, the manual and release notes | 0067 |

Schema: no bump is expected. If you truly need one, take **42** (linux), **43** (macos) and
say so in your report; the integrator renumbers. The keymap version (9) is macos's to bump.

### The interfaces between builders (fixed by this contract)

- `packaging/linux/build-appimage.sh <version> <out-dir>` (linux) — run on Ubuntu 22.04
  (CI) from the repository root after `cargo build --release`; writes
  `<out-dir>/Cantus-<version>-linux-x86_64.AppImage`. It bundles `cantus`, `cantus-scan`
  and `cantus-sandbox` (and whatever else the Windows installer ships beside the app).
- `packaging/macos/build-dmg.sh <version> <out-dir>` (macos) — run on `macos-latest`;
  builds both architectures, `lipo`s them, makes `Cantus.app`, signs and notarises **only
  when** the secrets are present (`APPLE_CERT_P12_BASE64`, `APPLE_CERT_PASSWORD`,
  `APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_PASSWORD`), otherwise ad-hoc signs; writes
  `<out-dir>/Cantus-<version>-macos-universal.dmg`.
- Stable names (the website's links through `releases/latest/download/`):
  `Cantus-linux-x86_64.AppImage`, `Cantus-macos-universal.dmg` beside
  `Cantus-windows-x64-setup.exe` (ship owns these names in `cantus-update/src/release.rs`).
- Each script is runnable by hand and documented in `docs/DEVELOPING.md` (its builder's
  paragraph); ship calls them from `release.yml` and never edits them (ask the integrator).

### Who owns which files

Touch only your files; for one line elsewhere, make it additive and name it in the report.

- **linux**: `crates/cantus-rt/src/{thread,topology}/linux.rs` and the Linux parts of
  `cantus-rt`; `crates/cantus-host/src/window.rs` (the dispatch) and a new
  `window/x11.rs`; the Linux run loop VST3 needs (`IRunLoop`, timers and fds); the audio
  hosts (`cantus-engine`'s device/host code, `cantus-ui/src/audio_setup/**`, the cpal
  features `pipewire`/`jack`); the scanner's Linux folders; file dialogs and
  `font_fallback.rs` on Linux; the test plug-ins' X11 GUIs; `packaging/linux/**`; `ci.yml`
  (if Linux needs system packages); locale area `linux.{en,es}.txt`.
- **macos**: `crates/cantus-rt/src/{thread,topology}/macos.rs`, `ipc.rs`/`shm.rs`'s
  non-Linux unix parts; `cantus-sandbox`'s macOS parts (helper death, crash blame);
  `crates/cantus-host/src/window/cocoa.rs` (one `mod` line in `window.rs`, additive); the
  scanner's macOS folders and bundles (additive beside linux's); `keymap.rs` and the macOS
  preset, the menu bar, trackpad gestures, Retina; the test plug-ins' Cocoa GUIs;
  `packaging/macos/**` (Info.plist, `.icns`, entitlements); `macos.yml` and the macOS leg
  of `null-hashes.yml`; `docs/mac-session.md` (the owner's checklist, **in Spanish**);
  locale area `macos.{en,es}.txt`.
- **ship**: `.github/workflows/release.yml` (and `windows-release.yml` if it is merged into
  it); `crates/cantus-update/**`; `web/**`; `docs/manual/**` (the install pages for Linux
  and macOS); the release notes; locale area `ship.{en,es}.txt` if the updater's UI needs
  strings.

## Rules (the usual ones, short)

- Work only in your worktree. Commit WIP often (`r30/<name>`); a cut session resumes.
  Write files in pieces under ~300 lines per tool call.
- `.cargo/config.toml` in your worktree (untracked) sets `jobs = 3` and no debug info; set
  `TMP`/`TEMP` to `F:\projects\daw-r30\tmp`. Turn incremental off if linking fails.
- Windows: `cargo clippy --workspace --all-targets` and the tests of every crate you touch,
  with `RUSTFLAGS=-D warnings`. Linux: `wsl -d Ubuntu-24.04 -- bash -lc 'bash
  /mnt/f/projects/cantus/tools/wsl/check.sh r30/<name> clippy'` (and `test`); WSLg shows X11
  and Wayland windows, so a plug-in editor can be opened and shot there. macOS: no Mac —
  `cargo clippy --target aarch64-apple-darwin` (installed) for what cross-checks; the real
  runner sees it after the integrator publishes. `cargo fmt --all` before each commit.
- Real-time rules (ADR 0005): no allocation, lock, block or I/O on the audio thread;
  `cantus-engine` stays `forbid(unsafe_code)`; OS code lives in `cantus-rt` (and, for
  plug-in windows, `cantus-host`). Never a spinning thread inside a real-time class.
- A new crate needs its notice (`cargo run -p xtask -- notices`) and an approved licence.
- Strings in both locales; the kit for any UI (no stock widgets, no literal colours).
- Your ADR (number above), your part of `docs/DEVELOPING.md`, the manual pages your feature
  needs (EN and ES), and `docs/round30/<name>.md`: what was done, how it was tested, the
  numbers, and **only** what needs the owner.
- Never touch `master`, never push, never stage files outside your worktree.

## linux — item 1

Make Linux a first-class product, measured. Start by building and running the whole suite
in WSL (`check.sh r30/linux test`) to learn what already works; `cantus-rt` already has
`thread/linux.rs` (rtkit through `audio_thread_priority`, `SCHED_FIFO`) and the sandbox
already runs on Linux (futex exchange) — prove them end to end rather than rewriting.

- **Audio:** cpal's `pipewire` and `jack` hosts on in the one Linux build (ALSA always);
  the device page lists the hosts with the best first (PipeWire, then JACK, then ALSA),
  buffer size and rate where the host allows, and remembers the choice; a missing
  PipeWire/JACK library at run time must not stop Cantus from starting (dlopen or a
  fallback — prove it). Duplex and the latency figure on PipeWire. The audio callback
  thread and the pool workers get real-time priority (rtkit when `RLIMIT_RTPRIO` is 0) and
  report what they got (the same status line Windows shows for MMCSS).
- **Plug-in editors:** CLAP `x11` and VST3 `X11EmbedWindowID` in a top-level X11 window of
  ours (through XWayland when Cantus runs on Wayland: a plug-in cannot embed into a Wayland
  surface), sized and resized as on Windows, kept above Cantus (transient-for), closed
  cleanly; VST3's `IRunLoop` (timer and fd handlers) driven from the UI thread; the same
  in the sandbox's helper. Test plug-ins get an X11 GUI so tests and a shot prove it.
- **Isolation end to end:** the sandbox suite green on Linux; a real crash (the test
  plug-ins' faults) recovered; the helper dies with Cantus (`PR_SET_PDEATHSIG`).
- **Scanner:** `~/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap`, `$CLAP_PATH`, `~/.vst3`,
  `/usr/lib/vst3`, `/usr/local/lib/vst3` (VST3 bundles' `Contents/x86_64-linux/*.so`).
- **Desktop:** file dialogs through the XDG portal (and GTK fallback) with `rfd`; system
  fonts and CJK fallback through fontconfig paths; data in `$XDG_DATA_HOME/cantus`, config
  in `$XDG_CONFIG_HOME/cantus`; a `.desktop` file and icons.
- **AppImage:** `packaging/linux/build-appimage.sh` (the contract above), built and run in
  WSL; then a Flatpak manifest (`packaging/linux/flatpak/`) that builds — Flathub
  submission is the owner's.
- ADR 0065 (the Linux platform layer), the manual's Linux install and audio page (EN/ES),
  `docs/round30/linux.md` with a table of what was tried (WSL's PipeWire, JACK if
  installable) and the measured latency.

## macos — items 2 and 3

No Mac: everything must compile for `aarch64-apple-darwin` and `x86_64-apple-darwin` and
pass on GitHub's macOS runner (the integrator publishes to `cantus-daw` and reports back
its failures to you). Cross-check with clippy for the Apple targets as far as the build
allows; write the code to be reviewed by reading it, with tests that the runner will run.

- **The app:** `packaging/macos/` — `Info.plist` (bundle id `com.cantusdaw.Cantus`,
  `NSMicrophoneUsageDescription` in EN and ES `InfoPlist.strings`, high-resolution
  capable, minimum macOS stated), `Cantus.icns` from the existing icon, entitlements for
  the hardened runtime (audio input, `disable-library-validation`, JIT none), the bundle
  layout with `cantus-scan` and `cantus-sandbox` in `Contents/MacOS`; data under
  `~/Library/Application Support/Cantus`; `build-dmg.sh` (the contract above).
- **Audio and real time:** CoreAudio through cpal (buffer size and rate on the device page,
  aggregate devices listed); the pool workers joined to the device's audio workgroup and
  Mach time-constraint threads (`thread/macos.rs` may have part of it — finish and test).
- **Isolation:** the exchange without futex (`os_sync_wait_on_address` on 14.4+, a
  semaphore before; decided at run time), POSIX shm names within 31 characters, the helper
  dying with Cantus (kqueue `NOTE_EXIT` on the parent, or a pipe), crash blame from signals.
  The same sandbox tests as Linux run on the macOS runner.
- **Plug-ins:** scanning `/Library/Audio/Plug-Ins/{CLAP,VST3}` and `~/Library/…`, `.clap`
  and `.vst3` bundles; editor windows in Cocoa (`window/cocoa.rs`: an `NSWindow` owning the
  plug-in's `NSView`, CLAP `cocoa`, VST3 `NSView`), in the helper when isolated; the test
  plug-ins get a Cocoa GUI. AU hosting stays in BACKLOG (say so in the ADR).
- **The interface:** a macOS keymap preset (⌘ for Ctrl, ⌥ for Alt, the standard shortcuts,
  default on macOS; keymap version bump with its migration), the global menu bar (the app's
  menus mirrored), trackpad pinch-zoom and two-finger scroll in the views, Retina checked
  in a shot at scale 2, Finder drops, the system's CJK fallback fonts.
- **Proof:** `macos.yml` runs on every push to `master` now that the repository is public
  (free minutes), plus the macOS leg of `null-hashes.yml` comparing the three systems'
  hashes (the job fails naming any difference).
- **The owner's checklist** `docs/mac-session.md` (Spanish): what to install, what to test
  (an interface in duplex 10 minutes, a real plug-in with its editor, the microphone
  permission, the `.dmg` past Gatekeeper, an update from an older build, trackpad and
  Retina, CPU on Apple silicon), and how to write the results down (a table to fill).
- ADR 0066 (macOS), the manual's macOS install page (EN/ES), `docs/round30/macos.md`.

## ship — item 4

- **One tag, three installers:** `release.yml` gets Linux (`ubuntu-22.04`, calls
  `packaging/linux/build-appimage.sh`) and macOS (`macos-latest`, calls
  `packaging/macos/build-dmg.sh`, secrets passed as env, placeholders documented) jobs
  beside Windows; each tests on its system first; one job publishes all files to the
  release (versioned + stable names, SHA-256 sums). Until linux's and macos's scripts
  merge, stub them in your worktree only (do not commit stubs into their paths).
- **The updater per system:** the asset name for the running system and architecture;
  how each applies an update — Windows as today; Linux replaces the AppImage file it runs
  from (`$APPIMAGE`), atomically, keeping the old one until the new one starts, or says
  "download" when not run as an AppImage (a distro package); macOS downloads the `.dmg`,
  verifies it, and opens it for the user (replacing a signed `.app` in place comes later —
  say so). Tests for the choice of asset on every system/arch and for each apply path that
  can run off its system.
- **The website:** the Linux and macOS cards say "Available" with a download link only when
  the latest release holds that system's file (checked at run time from the browser
  against the GitHub API, cached, with "Coming soon" as the fallback when the check fails),
  in English and Spanish; look at it (the site's shot or a screenshot).
- **Docs:** the manual's download/install chapter for three systems (EN/ES; the system
  pages' bodies are linux's and macos's — link them), the release notes template with the
  three files, `docs/DEVELOPING.md`'s release section, ADR 0067, `docs/round30/ship.md`.
