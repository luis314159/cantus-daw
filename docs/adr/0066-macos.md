# ADR 0066 — Cantus on macOS, prepared without a Mac

**Status:** accepted (round 30) · **Date:** 2026-10-22 · **Extends:** ADR 0019 (plug-in
windows), ADR 0031 §2.6–2.7 and R4 (real time, workgroups), ADR 0063 (the sandbox),
ADR 0048 (CI) · **Amends:** BACKLOG §2 ("Prepared without a Mac")

## Context

The owner's ask (2026-10-22): releases for Windows, Linux and macOS at the end of round
30, with everything that can be prepared without a Mac done, so the borrowed Mac only
verifies. No Mac is at hand: the code is cross-checked from Windows with clippy for
`aarch64-apple-darwin` and `x86_64-apple-darwin` and proven on GitHub's `macos-latest`
runner (Apple silicon), which is free now that the repository is public.

Before this round the workspace already cross-compiled but for one variadic call; the
macOS pieces missing were the cross-process wait (Linux's futex had no macOS
counterpart: a 50 µs sleep loop stood in), the helper's death with Cantus, crash blame,
VST3 bundles, plug-in editor windows, the application bundle and its signing, and the
macOS conventions of the interface.

## Decision

1. **The exchange's wait** (`cantus-rt/src/ipc.rs`): a wait on the word's address shared
   between processes — `os_sync_wait_on_address` with `OS_SYNC_WAIT_ON_ADDRESS_SHARED`
   (public, macOS 14.4+) and, before 14.4, `__ulock_wait` with `UL_COMPARE_AND_WAIT_SHARED`
   (the kernel call the public one wraps, there since 10.12; Rust's standard library falls
   back to the same family). Both are found with `dlsym` once, off the audio thread
   (`ipc::wait_kind` says which). The brief named a semaphore for old systems; it loses:
   macOS has no `sem_timedwait`, and a caller's wait has a deadline. The sleep loop stays
   only as the last resort if neither symbol exists.
2. **Shared-memory names** fit `PSHMNAMLEN` (31 bytes with the `/`): `shm::MAX_NAME` is 30
   on Apple systems, `unique_name` writes the unique parts in hexadecimal and shortens the
   prefix, and a longer name is refused before the system's `ENAMETOOLONG`.
3. **Workgroups** (`cantus-rt/src/thread/macos.rs`): the device's
   `kAudioDevicePropertyIOThreadOSWorkgroup` is looked up through CoreAudio by the name
   cpal gives the device (`thread::use_device_workgroup`, called by the engine as a stream
   opens; one line in `backend.rs`), and every live pool worker that promotes itself joins
   it after its time constraint (R4's order). A cancelled workgroup (`EINVAL`: the device
   changed) is fetched again once. CoreAudio's own I/O thread is in the workgroup already;
   aggregate devices are devices to CoreAudio and cpal lists them.
4. **The sandbox's helper** dies with Cantus through a kqueue `EVFILT_PROC`/`NOTE_EXIT`
   watch on its parent (a parent already gone ends it at once); crash blame reads the
   faulting pc from the signal's `mcontext` (stripped of pointer authentication on Apple
   silicon) and the loaded images from dyld (`_dyld_*`, Mach-O segments) — what
   `dl_iterate_phdr` gives on Linux.
5. **Plug-ins**: `.clap` and `.vst3` bundles' binaries are `Contents/MacOS/<executable>`
   (the `Info.plist`'s, else the bundle's name, else the only file — `cantus_host::bundle`);
   VST3 enters through `bundleEntry(CFBundleRef)`/`bundleExit`; the folders are
   `/Library/Audio/Plug-Ins/{CLAP,VST3}` and the user's. **Editors in Cocoa**
   (`cantus-host/src/window/cocoa.rs`): an `NSWindow` whose content view is the parent
   handed to CLAP's `cocoa` API or VST3's `NSView`; sizes are points (no `set_scale`, the
   gui extension's rule for logical-pixel APIs); a window with an owner floats above Cantus
   and hides when Cantus is not active. In the sandbox the helper becomes a background
   AppKit application (`Accessory`) whose main-loop wait dispatches AppKit's events, woken
   by an application-defined event a request posts. The test plug-ins get Cocoa editors.
   **AU hosting stays in BACKLOG §2** (and our effects as AU).
6. **The interface**: a **macOS keymap preset** (Cantus's map with the system's chords left
   alone — ⌘M minimises, so the metronome takes Logic's K; Redo only on ⌘⇧Z — and ⌘, for
   Settings, a new action in every preset), the default on a Mac; keymap file version 10
   fills ⌘, in where it is free. The **global menu bar** mirrors the app's menus with the
   keymap's own key equivalents (remapping a key moves it in the menu); its model is plain
   data in `cantus-ui` (tested everywhere), its AppKit side in `cantus-launch::macmenu`
   (`cantus-ui` stays `forbid(unsafe_code)`). A text edit chosen while a text field has the
   focus is handed to egui as the events its key makes, so ⌘C still copies the field.
   Trackpad pinch and two-finger scroll reach the views through egui (`Zoom` and point
   wheels, the editors' shared `wheel`); Finder drops are egui's dropped files, as on
   Windows. Data lives in `~/Library/Application Support/Cantus`; the manual is read from
   the bundle's `Resources`.
7. **The application** (`packaging/macos/`): `Info.plist` (`com.cantusdaw.Cantus`, macOS 11
   or later, high resolution, the microphone's reason in English and Spanish), the
   hardened runtime's entitlements (audio input, library validation off for third-party
   plug-ins; no JIT, no unsigned executable memory), `Cantus.icns` rendered from the mark
   at build time (`cantus-app/icon.rs`, as the Windows `.ico`), and `build-dmg.sh <version>
   <out-dir>`: both architectures built and `lipo`ed, `cantus-scan` and `cantus-sandbox`
   beside the app in `Contents/MacOS`, signed with a Developer ID and notarised (the app,
   then the `.dmg`, each stapled) only when the five `APPLE_*` secrets are present,
   ad hoc otherwise.
8. **Proof**: `macos.yml` runs on every push to master — clippy for both Mac targets, the
   whole suite, the schedule fuzz, and `build-dmg.sh` with a look inside the image — and
   `null-hashes.yml` has a macOS leg: Apple silicon's baseline list must equal Windows'
   and Linux's x86-64-v3 lists line for line.

## Consequences

- One binary runs on macOS 11 and later, Apple silicon and Intel, and takes the public
  wait where it exists; nothing in the engine changed (`forbid(unsafe_code)` holds).
- What only a Mac in hand can prove is listed in `docs/mac-session.md`: duplex for ten
  minutes, a real plug-in's editor, the microphone prompt, the `.dmg` past Gatekeeper,
  an update, trackpad feel and Retina, CPU on Apple silicon.
- Not yet: opening a `.cantus` by double-clicking it in Finder (an Apple Event winit does
  not pass on; the bundle declares no document type until it does), AU hosting, signing
  without the owner's Apple Developer account.

## What the tests hold

- `cantus-rt/tests/bell_shared.rs`: every system has a real wait (macOS `os_sync` or
  `ulock`); a ring through one mapping wakes a sleeper on another mapping of the same
  pages within 500 ms; a timed wait ends at its timeout; generated names fit `MAX_NAME`
  and a longer one is refused.
- `cantus-rt` (macOS): four-character codes; devices are found by name (unknown names find
  nothing).
- `cantus-sandbox/tests/parent_death.rs` (Linux and macOS): the helper outlives its
  parent by less than 5 s; `platform.rs`: the images listed cover this program and the C
  library, in different spans.
- `cantus-host`: bundle binaries (plist, name, only file); `tests/macos_folders.rs`: the
  standard folders and bundles as candidates; the VST3 backend's suite and the sandbox's
  run on macOS with the bundle laid out as `Contents/MacOS/<name>`.
- `cantus-ui`: the macOS preset leaves the reserved chords free, has K, ⌘⇧Z and ⌘,, is
  the first-start map on a Mac and survives its file; a version-9 file gets ⌘,; the menu
  bar's model (six menus, translated labels, the keymap's keys, no duplicates, no bare
  letters), AppKit's key characters, the text edits' events, the trackpad's pinch and
  swipe reaching `wheel`, a bundle's `Resources` for the manual, Application Support.
