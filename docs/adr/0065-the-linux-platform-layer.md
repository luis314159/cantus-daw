# ADR 0065 — The Linux platform layer

**Status:** accepted (round 30) · **Date:** 2026-10-22 · **Extends:** ADR 0019 (plug-in
editor windows), ADR 0025 (audio devices), ADR 0031 §2.6 (real-time priority), ADR 0032 /
0048 (releases), ADR 0063 (plug-ins out of process)

## Context

The owner's ask (2026-10-22): releases of Windows, Linux and macOS at the end of round 30;
Linux "a complete product": low-latency audio through PipeWire and JACK with ALSA as the
fallback, real-time priority, plug-in editors on X11 and Wayland, the sandbox proven end to
end, the scanner's folders, file dialogs and fonts, an AppImage built in CI (a Flatpak
after), the website's download card.

What existed: `cantus-rt`'s `thread/linux.rs` (`SCHED_FIFO` directly, rtkit through
`audio_thread_priority`), the sandbox's Linux paths (futex exchange, `PR_SET_PDEATHSIG`),
cpal's ALSA host only, no plug-in editor at all on Linux (`window.rs` answered
`Unsupported`), CLAP folders without `/usr/local/lib/clap`, data in
`~/.local/share/Cantus` for everything.

## Decisions

### 1. Audio: cpal's PipeWire and JACK hosts, in the one build

`cantus-engine` turns on cpal's `pipewire` and `jack` features on Linux. The driver list
ranks **PipeWire, then JACK, then ALSA** (`devices::preference`); the default is the first
of them that is usable now. JACK's host says it is always available (its library is
opened at run time): it is offered only when it has a device, which needs a running
server. cpal's `default_host()` builds a new host on every call (PipeWire's enumerates the
graph), so the default is worked out once.

- **JACK** is `dlopen`ed (`libjack.so.0`, the `jack` crate's `dynamic_loading`): a
  missing library is a host with no devices. `jack-sys`'s build script still asks
  pkg-config for `jack.pc`, so `libjack-jackd2-dev` is a build need; `--as-needed` drops
  the link (no `NEEDED libjack` in the binary — checked in `docs/round30/linux.md`).
- **PipeWire** is linked (`pipewire-rs` has no run-time loading). Two consequences, each
  handled:
  - *A system without libpipewire must still start Cantus.* The AppImage carries a
    **stand-in** `libpipewire-0.3.so.0` (`packaging/linux/stub/make-stub.sh`: every symbol
    the programs take from the real library, functions returning zero) that `AppRun` puts
    first on the library path only when the system has none. cpal offers the PipeWire
    host only when PipeWire's socket exists, so the stand-in is never called; were it,
    every constructor answers null and the host reports itself unavailable.
    `packaging/linux/test-fallback.sh` proves it (CI forces the stand-in; WSL, which has no
    libpipewire, takes it by itself). The same pattern carries `libxkbcommon-x11` (which
    winit `dlopen`s for an X11 window and cannot do without) for a system that lacks it:
    each library has its own fallback folder, used only when the system has none.
  - *The AppImage is built on Ubuntu 22.04 (glibc 2.35), whose PipeWire is 0.3.48*, older
    than the headers cpal's host compiles against (0.3.49: `pw_buffer.requested`). `packaging/linux/deps.sh`
    unpacks PipeWire 1.0.5's headers (Ubuntu 24.04's packages, SHA-256 pinned) and points
    pkg-config at them **with the system's 0.3.48 library** as the one linked: any symbol
    0.3.48 lacks fails the link, so the AppImage starts there. One field 0.3.48 does not
    fill, `pw_buffer.requested` (0.3.49), is read only from a 0.3.49+ library (a CANTUS
    PATCH in `third_party/cpal`, which also treats 0 — "no suggestion" — as the whole
    buffer instead of an empty one).

### 2. Real-time priority, reported

The callback of ALSA (and any host whose thread is cpal's) is promoted as before. JACK's
and PipeWire's callback threads belong to their libraries, which make them real-time
(through rtkit when the process may not); on Linux `promote_callback` now **checks**: a
thread they left at normal priority (no rtkit, no `RLIMIT_RTPRIO`, a JACK server started
with `--no-realtime`) is promoted by Cantus after all; one they made real-time is left
alone and named (`driver-owned thread: SCHED_FIFO, left alone`). `AudioHost::PipeWire` is
new.

`cantus_rt::thread::priority_report()` keeps what the callback and each live pool worker
got (the callback's answer is stored with `try_lock`, never waiting, once per stream;
a worker's with its promotion and removed with it). The device page shows it on every
system: *Real-time priority: audio thread SCHED_FIFO priority 50 · workers 3 of 3
real-time*, in the warning colour when the audio thread has none.

### 3. Plug-in editors: X11 windows of our own

`window/x11.rs`: a top-level X11 window on its own connection (`x11rb`, pure Rust), created
and synced before the editor is opened so the plug-in's own Xlib/XCB connection can parent
its view to it — CLAP `x11`, VST3 `X11EmbedWindowID`. `WM_DELETE_WINDOW` for the close
box, `WM_NORMAL_HINTS` min = max for an editor that cannot resize, `WM_TRANSIENT_FOR`
Cantus's window so it stays above it, `_NET_ACTIVE_WINDOW` to bring it forward,
`WM_STATE`/`_NET_WM_STATE_HIDDEN` for minimised, `_NET_CLIENT_LIST_STACKING` for "in
front" (as the Win32 window's checks of ADR 0051). Nothing dispatches the connection's
events: `closed()` and `take_resize()`, asked every frame, read them; sizes the host set
itself are told apart from the user's by remembering what was asked.

**Wayland.** A plug-in cannot embed into a Wayland surface (neither CLAP nor VST3 has a
Wayland embedding that hosts or plug-ins implement), and an X11 window cannot be kept
above a Wayland one. So Cantus's own window is an **X11 window on Linux** whenever an X
server is there (`DISPLAY`, which XWayland sets): eframe's event-loop hook asks winit for
X11 (`linux_desktop::event_loop_hook`). `CANTUS_WAYLAND=1` gives a native Wayland window
instead; editors then open as plain top-levels. This is what Bitwig, REAPER and Ardour do.

### 4. The run loop editors need

Linux has no system loop to hang timers and descriptors on: a GUI toolkit inside a plug-in
asks the host. `cantus-host/src/runloop.rs` keeps one per instance — timers (served at 10
ms at least; a late one fires once, not once per period missed) and descriptors (`poll`
with no wait) — and the instance's `idle` serves it once a frame, calling back with **no
lock held** so a handler may (un)register itself:

- CLAP: the host extensions `clap.timer-support` and `clap.posix-fd-support` (main thread
  only, as the spec says), calling `on_timer` / `on_fd`;
- VST3: the frame is also `Steinberg::Linux::IRunLoop` on Linux (the SDK's editors ask the
  `IPlugFrame` for it), calling `ITimerHandler::onTimer` / `IEventHandler::onFDIsSet`.

The frame loop keeps a 15 ms pace while any editor window is open
(`PluginHost::any_window_open`), idle or not. In the sandbox's helper (ADR 0063) the same
`idle` runs from its own loop every 15 ms, and its windows are the same `x11.rs`.

### 5. The desktop

- **Places**: configuration in `$XDG_CONFIG_HOME/cantus`, data in `$XDG_DATA_HOME/cantus`
  (`linux_desktop.rs`); an older build's `~/.local/share/Cantus` is moved to the data place
  on the first start and its top-level files copied to the configuration place.
  `CANTUS_DATA_DIR` and a portable copy still put everything in one folder.
- **File dialogs**: `rfd`'s XDG desktop portal backend, which falls back to `zenity` (a GTK
  dialog) where there is no portal.
- **Fonts**: the CJK, Indic and Thai fallback asks fontconfig first (`fc-list`, the faces
  for the script's language), then looks for the usual files by name in the system's,
  the user's and a Flatpak's font folders.
- **Plug-in folders**: CLAP `~/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap`, `CLAP_PATH`;
  VST3 `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3`, `VST3_PATH` (Flatpak's Linux
  audio extensions and other hosts use it), bundles' `Contents/x86_64-linux/*.so`.
- **Identity**: `com.cantusdaw.Cantus` — the `.desktop` file, the icon (the mark of ADR
  0046 drawn by `cantus-app`'s build script at 16–512 px), AppStream metadata, and the
  `application/x-cantus-project` type for `*.cantus`.

### 6. Packaging: an AppImage now, a Flatpak prepared

`packaging/linux/build-appimage.sh <version> <out-dir>` (the interface of BUILDING.md):
the launcher and both x86-64 levels (as Windows), `cantus-scan`, `cantus-sandbox`, Cantus's
effects as CLAP/VST3, the desktop files, the stand-in libpipewire; the **type-2 AppImage
runtime pinned by SHA-256** (static: no `libfuse2` on the user's system) joined to a zstd
squashfs — no `appimagetool`, which has no pinned release. Built on Ubuntu 22.04. A
started AppImage puts itself in the user's applications menu (its `.desktop` with
`Exec=` the AppImage's path, icons, MIME type), once, unless `CANTUS_NO_DESKTOP_ENTRY=1`;
the updater (ADR 0067) replaces the file in place, so the entry stays right.

`packaging/linux/flatpak/com.cantusdaw.Cantus.yml`: Freedesktop 24.08, X11 socket (see 3),
PipeWire and PulseAudio sockets, rtkit, network, home, and Flathub's
`org.freedesktop.LinuxAudio.Plugins` extension point with `CLAP_PATH`/`VST3_PATH`.

## Consequences

- One Linux build has every driver; nothing to choose at download.
- Building on Linux needs clang, PipeWire's and JACK's `-dev` packages (CI and WSL's
  `fetch-deps.sh` have them).
- On Wayland, Cantus's window goes through XWayland (fractional scaling is the
  compositor's, as for every X11 program) until plug-in formats can embed in Wayland.
- AU, LV2 and LADSPA are not hosted (BACKLOG); Linux plug-ins are CLAP and VST3.
- Not done here: Flathub's submission (the owner's account and review), distribution
  packages (`.deb`/`.rpm`), ARM64 Linux.

## What the tests hold

- `cantus-engine/tests/devices.rs`: the driver order (PipeWire < JACK < ALSA, ALSA always
  there); the live tests print the priority report.
- `cantus-rt/tests/model.rs`, `machine.rs`: `PipeWire` maps to its host and owns its
  thread's priority; a Linux JACK/PipeWire thread left at normal priority is promoted or
  says why; the report follows the threads.
- `cantus-host/src/runloop.rs`: timers due once when late, descriptors ready only for what
  was asked.
- `cantus-host/tests/clap_window_x11.rs` (an X server: WSLg, Xvfb in CI): the test plug-in's
  view is a child of our window at the editor's size; its 20 ms timer and its X
  connection's descriptor are served by the host's frames (the view counts timer calls and
  draws on its expose event, marks a test reads from the server); the window goes on close
  and on its close box.
- `cantus-host/tests/vst3_backend.rs`: the VST3 view attaches to an X11 window id, its
  timer and descriptor on the frame's `IRunLoop` are served by `idle`, and unregistered
  when it is removed.
- `cantus-host/tests/linux_folders.rs`: the plug-in folders and `CLAP_PATH`/`VST3_PATH`.
- `cantus-ui`: the XDG places and their defaults, the move of an older folder once, X11
  unless Wayland is asked for, the AppImage's menu entry once per path, fontconfig's lines.
- `packaging/linux/test-fallback.sh`: the AppImage starts with the stand-in libpipewire,
  offers ALSA and not PipeWire.
