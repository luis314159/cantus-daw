# Round 30 — linux (item 1): Cantus on Linux, a complete product

Branch `r30/linux`; ADR 0065. What was asked (BUILDING.md, "linux — item 1") and what
was done, how it was proved, the numbers, and what only the owner can do.

## What was done

| Area | Done |
|---|---|
| **Audio hosts** | cpal's `pipewire` and `jack` features on in the one Linux build (ALSA always). Driver list ranked PipeWire → JACK → ALSA, the default the best usable one, worked out once (cpal's `default_host()` rebuilt PipeWire's host per call). JACK offered only with a running server. |
| **Missing libraries** | JACK is `dlopen`ed — and `third_party/jack-sys` (CANTUS PATCH) no longer needs `jack.pc` to build, so nothing links it. libpipewire is linked: the AppImage carries a generated stand-in (`stub/make-stub.sh`) that `AppRun` uses only when the system has none; `test-fallback.sh` proves Cantus starts with it, offering ALSA and not PipeWire. |
| **Old PipeWire** | Built on Ubuntu 22.04 (0.3.48) with PipeWire 1.0.5's headers over the system library (`deps.sh`, pinned SHA-256): a symbol 0.3.48 lacks fails the link. cpal reads `pw_buffer.requested` only from a 0.3.49+ library, and 0 means the whole buffer (CANTUS PATCH). |
| **Real time** | `AudioHost::PipeWire`; a JACK/PipeWire callback thread their library left at normal priority is promoted by Cantus, one they made real-time is named. `priority_report()` (callback + each live worker) shown on the device page on every system ("Real-time priority: audio thread … · workers n of m real-time"), warning colour when the callback has none. |
| **Plug-in editors** | `window/x11.rs`: top-level X11 window (x11rb), transient for Cantus's, close box, fixed/resizable size hints, raise/activate, minimised and in-front checks. CLAP `x11` and VST3 `X11EmbedWindowID`. Cantus's own window is X11 on Linux (XWayland on Wayland) unless `CANTUS_WAYLAND=1`. |
| **Run loop** | `runloop.rs`: CLAP `timer-support` + `posix-fd-support`, VST3 `Linux::IRunLoop` on the frame; served from `idle` with no lock held; 15 ms frame pace while an editor is open; the sandbox's helper serves it from its own loop. |
| **Test plug-ins** | Both CLAP test plug-ins draw an X11 editor on their own connection, register their descriptor and a 20 ms timer with the host, and mark the server with what ran; the VST3 one registers a timer and a descriptor on `IRunLoop`. |
| **Isolation** | The sandbox suite on Linux (bridge, faults, crash blame, sessions) green; `PR_SET_PDEATHSIG` already there. |
| **Scanner** | `/usr/local/lib/clap` added; `VST3_PATH` (Flatpak's Linux audio extensions); a test of every folder. |
| **Desktop** | XDG places (`~/.config/cantus`, `~/.local/share/cantus`) with a one-time move of the older `~/.local/share/Cantus`; file dialogs: rfd's portal with zenity fallback (already the build's); fonts: fontconfig first (`fc-list` by language), then known files in system, user and Flatpak font folders. |
| **Packaging** | `build-appimage.sh` (contract interface), pinned type-2 runtime, both x86-64 levels + launcher, scanner, sandbox, our plug-ins, `.desktop`, icons (16–512 px from the mark, drawn by `cantus-app`'s build script), AppStream, MIME; the AppImage adds itself to the applications menu once per path. Flatpak manifest (`flatpak/`), built by a CI job on demand. `deps.sh` for CI and developers. |
| **CI** | Linux deps through `deps.sh`; tests under `xvfb-run`; an `appimage` job on Ubuntu 22.04 (quick mode + the fallback proof); a `flatpak` job (workflow_dispatch). `null-hashes.yml`'s Linux leg uses `deps.sh` too. |
| **Notices** | `xtask notices` now covers every released target (Windows, Linux, both macOS), 498 crates; `Zlib.txt` added; the AppImage runtime's notice. |
| **Docs** | ADR 0065; manual `docs/manual/{en,es}/linux.md`; DEVELOPING (Linux build, WSL audio servers, packaging); `packaging/linux/README.md`; `third_party/README.md`. |

## How it was tested — and the numbers

(filled below)
