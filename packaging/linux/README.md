# Cantus on Linux — packaging (ADR 0065)

| File | What it is |
|---|---|
| `deps.sh` | The Ubuntu packages a Linux build needs (ALSA, PipeWire, clang for PipeWire's bindings, X11/Wayland, Xvfb, squashfs). On Ubuntu 22.04 it also unpacks PipeWire 1.0.5's headers (pinned) over the system's 0.3.48 library. |
| `build-appimage.sh <version> <out-dir>` | The AppImage, from the repository root after `cargo build --release`: `<out-dir>/Cantus-<version>-linux-x86_64.AppImage`. |
| `test-fallback.sh <AppImage>` | Proof that the AppImage starts where libpipewire is missing. |
| `AppRun` | The AppImage's entry: picks the libpipewire stand-in when the system has none. |
| `stub/make-stub.sh` | Makes that stand-in from the symbols the programs take from the real library. |
| `com.cantusdaw.Cantus.desktop`, `.metainfo.xml`, `.mime.xml` | The menu entry, the AppStream metadata, the `.cantus` file type. |
| `flatpak/` | The Flatpak manifest (Flathub's submission is the owner's). |

The icons are drawn at build time by `crates/cantus-app/build.rs` (the mark of ADR 0046)
into `target/release/build/cantus-*/out/hicolor/<n>x<n>/apps/com.cantusdaw.Cantus.png`.

## Why a stand-in libpipewire

cpal's PipeWire host links `libpipewire-0.3.so.0` (pipewire-rs has no run-time loading),
and a program whose library is missing does not start at all. Every desktop distribution
Cantus supports ships the library (Ubuntu 22.04's desktop installs it for screen sharing),
but a minimal system may not. There AppRun puts `usr/lib/fallback/pipewire` first on the library
path: a library of the same name whose every function returns zero. Cantus starts; the
PipeWire host is not offered, because cpal offers it only when PipeWire's socket exists —
which it cannot without PipeWire. JACK is opened at run time (`libjack.so.0`, through the
`jack` crate's dynamic loading) and ALSA is everywhere.

## The build machine

Ubuntu 22.04 (glibc 2.35): the AppImage runs on that and anything newer. The program links
against 22.04's libraries; PipeWire's newer headers compile cpal's host, and a symbol that
22.04's 0.3.48 library lacks would fail the link — the guarantee that the AppImage starts
there. A field 0.3.48 does not fill (`pw_buffer.requested`) is read only from a 0.3.49+
library (`third_party/cpal`, CANTUS PATCH).

## Trying the audio hosts in WSL

`tools/wsl/fetch-deps.sh` unpacks a PipeWire daemon, WirePlumber and jackd2 into
`~/linux-deps/rt` (no sudo). `tools/wsl/audio-servers.sh pipewire|jack` starts one with a
dummy (timer-driven) device, so the PipeWire and JACK hosts can be opened and measured;
`docs/round30/linux.md` has the numbers.

## libxkbcommon-x11

winit opens `libxkbcommon-x11.so.0` at run time for an X11 window (Cantus's window is X11 on
Linux, ADR 0065 §3) and stops without it. Desktops with X11 or XWayland have it; the
AppImage carries it and `libxcb-xkb.so.1` (both MIT) in `usr/lib/fallback/xkb`, which
AppRun uses only where the system has none — found missing in WSL's bare Ubuntu.
