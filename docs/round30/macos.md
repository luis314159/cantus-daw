# Round 30 — macos (items 2 and 3)

Branch `r30/macos`. ADR 0066. Everything for macOS that can be done without a Mac; the
owner's checklist for the borrowed one is `docs/mac-session.md` (Spanish).

## What was done

| Area | What | Where |
|---|---|---|
| Isolation | The cross-process bell: `os_sync_wait_on_address` (14.4+) or `__ulock_wait` (10.12+), both shared and found at run time with `dlsym`; `ipc::wait_kind` | `cantus-rt/src/ipc.rs` |
| | POSIX shm names within 31 bytes (`shm::MAX_NAME` = 30 on Apple), `shm_open`'s variadic mode (the one line that kept the workspace from compiling for macOS) | `cantus-rt/src/shm.rs` |
| | The helper dies with Cantus (kqueue `NOTE_EXIT` on the parent); crash blame: the pc from `mcontext` (PAC stripped), the images from dyld | `cantus-sandbox/src/platform.rs` |
| | The helper as a background AppKit app; its main-loop wait dispatches AppKit events, a request wakes it with a posted event | `cantus-sandbox/src/platform_macos.rs` |
| Real time | The device's I/O workgroup looked up through CoreAudio by cpal's device name; live workers join it after their time constraint; a cancelled one is fetched again | `cantus-rt/src/thread/macos.rs`, `thread.rs` (`use_device_workgroup`), one line in `cantus-engine/src/backend.rs` |
| Plug-ins | Bundle binaries (`Contents/MacOS`, `Info.plist`'s executable), CLAP bundles load, VST3 `bundleEntry(CFBundleRef)`/`bundleExit` (VST3 was refused on macOS before), `Plug-Ins` spelling | `cantus-host/src/bundle.rs`, `clap/library.rs`, `vst3/module.rs`, `vst3/mod.rs` |
| | Editors in an `NSWindow` (CLAP `cocoa`, VST3 `NSView`; points, no `set_scale`; floats over Cantus) | `cantus-host/src/window/cocoa.rs`, `clap/main.rs`, `clap/mod.rs`, `vst3/main_thread.rs` |
| | Test plug-ins: Cocoa editors (CLAP gain and synth), `bundleEntry`/`NSView` (VST3) | `cantus-test-plugins/**` |
| Interface | macOS keymap preset (default on a Mac): ⌘M left to the system (metronome on K), Redo ⌘⇧Z only, new action *Settings* on ⌘, in every preset; keymap file **version 10**; Option shown for Alt on a Mac | `cantus-ui/src/keymap.rs` |
| | Global menu bar: model in `cantus-ui/src/macos/menu.rs` (tested everywhere), AppKit in `cantus-launch/src/macmenu.rs` (`cantus-ui` stays `forbid(unsafe_code)`); keymap's keys as key equivalents; text edits handed to a focused field as egui events | `cantus-ui/src/macos/**`, `app.rs` (one call), `boot.rs` (`raw_input_hook`) |
| | Data in `~/Library/Application Support/Cantus`; the manual from the bundle's `Resources` | `cantus-ui/src/app.rs`, `docs.rs` |
| Packaging | `Info.plist` (`com.cantusdaw.Cantus`, macOS 11+, Retina, microphone reason), `InfoPlist.strings` EN/ES, hardened-runtime entitlements, `build-dmg.sh` (contract), `Cantus.icns` rendered from the mark by the app's build script | `packaging/macos/**`, `cantus-app/icon.rs`, `build.rs` |
| Proof | `macos.yml` on every push to master: clippy (both Mac targets), suite, fuzz, and the `.dmg` built and inspected; `null-hashes.yml`'s macOS leg (Apple silicon's list = Windows' = Linux's); `xtask null-hashes` accepts an ARM machine's baseline alone | `.github/workflows/*`, `xtask/src/null_hashes.rs` |
| Tooling | `tools/macos/xcheck.sh`: clippy for a Mac target of the whole workspace from Windows | `tools/macos/**` |
| Docs | ADR 0066, DEVELOPING §4/§5, manual EN/ES (audio on a Mac, plug-in folders, shortcuts and menu bar), BACKLOG §2, `docs/mac-session.md` | |

## How it was tested

- **Cross-checked from Windows** with `-D warnings`: `tools/macos/xcheck.sh
  aarch64-apple-darwin --workspace --all-targets` and the same for
  `x86_64-apple-darwin` — clean. (Plain `cargo clippy --target …` stops at ring, ogg and
  LAME, whose build scripts need the macOS SDK's C compiler; the script stands in for it.)
- **Windows**: `cargo clippy --workspace --all-targets` clean; `cargo test` of cantus-rt,
  cantus-host, cantus-sandbox, cantus-launch, xtask, cantus-ui and cantus all green (the
  font-coverage test caught AppKit's private-use arrow codes as literals: now numbers).
- **Linux** (WSL, own clone): `check.sh r30/macos clippy` exit 0.
- **Retina**: `shoot target/shots-retina 2 eq` — the mixer with Aequalis open at scale 2
  (3000 × 1880): text, curves and icons crisp, nothing misaligned.
- **Minimum macOS: 11 (Big Sur)** — `LSMinimumSystemVersion` in `Info.plist` and
  `MACOSX_DEPLOYMENT_TARGET` in `build-dmg.sh`; Apple silicon and Intel.
- **Only the macOS runner can prove**: that the code runs — the bell's wake-ups and
  timeouts, the helper's death, the dyld spans, the Cocoa windows (opened by the sandbox
  and host suites with the test plug-ins), the menu bar's install, the workgroup lookup,
  `build-dmg.sh` end to end and the null hashes being identical on Apple silicon.

New tests: `cantus-rt/tests/bell_shared.rs` (4), `cantus-rt` macOS unit tests (2),
`cantus-sandbox/tests/parent_death.rs`, the images test in `platform.rs`,
`cantus-host/src/bundle.rs`, `cantus-host/tests/macos_folders.rs` (2),
`cantus-ui`: keymap (2), `macos/tests.rs` (6), `docs.rs` (1).

## Files outside my ownership (additive)

- `crates/cantus-engine/src/backend.rs`: one call, `use_device_workgroup` (linux owns the
  device code).
- `crates/cantus-host/src/window.rs`: the `cocoa` module lines and the `other` stub's cfg
  (linux edits the same dispatch for X11: expect a small conflict).
- `crates/cantus-host/src/clap/{main,mod}.rs`, `vst3/main_thread.rs`: an AppKit arm beside
  Win32/X11 (linux adds X11 arms there too).
- `crates/cantus-test-plugins/clap-{gain,synth}/src/gui.rs`: the Cocoa module and cfgs
  (linux adds X11 GUIs there).
- `crates/cantus-ui/src/app.rs` (`data_dir`'s macOS branch, the menu bar call,
  `OpenSettings`), `boot.rs`, `docs.rs`, `i18n.rs`, `lib.rs`.
- `crates/cantus-launch/**` (the AppKit menu bar), `crates/cantus-app/{icon,build}.rs`.
- `xtask/src/null_hashes.rs`, `docs/manual/{en,es}/README.md` (paragraphs in chapters 2,
  10 and 12; ship owns chapter 1), `docs/DEVELOPING.md`, `docs/BACKLOG-excellence.md`.
- No schema bump. Keymap version 9 → 10.

## Only the owner can do

- Pay for the **Apple Developer account**, make the Developer ID certificate and the
  app-specific password, and put the five `APPLE_*` secrets in `cantus-daw`
  (`docs/mac-session.md`, "Antes de la sesión").
- The **session with the Mac** (`docs/mac-session.md`): duplex, a real plug-in's editor,
  the microphone prompt, Gatekeeper, an update, trackpad and Retina, CPU on Apple silicon.
