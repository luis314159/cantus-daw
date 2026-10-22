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

All in WSL Ubuntu 24.04 (WSLg is an X server) from a private clone of the branch, with
CI's flags (`-D warnings`), plus Windows for every crate touched.

**Audio hosts in WSL** (`tools/wsl/audio-servers.sh`: no sound card, so PipeWire runs a
null sink and source on its timer driver and JACK its dummy driver; the callbacks, the
negotiation and the scheduling are real). The engine's live tests
(`devices -- --ignored live_streams…`), duplex (input and output open), 48 kHz:

| Host | Server | Buffer | Callbacks | xruns | Reported latency | Real-time priority |
|---|---|---|---|---|---|---|
| PipeWire | PipeWire 1.0.5 + WirePlumber 0.4.17 | 128 (2.67 ms) | 3 747 in 10 s (374.7/s = 48 000/128) | 0 | in 0 / out 0 frames — the null sink has no device delay; the graph runs at the asked quantum (`pw-top`: QUANT 128) | refused in WSL (no rtkit, `RLIMIT_RTPRIO` 0), said so on the line; **as root: callback `SCHED_FIFO priority 50`, workers 6 of 6** |
| PipeWire | same | 64 (1.33 ms) | 14 995 in 20 s (749.8/s) | 0 | quantum 64 | as above |
| PipeWire | same | 32 (0.67 ms) | 29 946 in / 29 937 out in 20 s (1 497/s) | 0 | quantum 32 | as above |
| JACK | jackd2 1.9.21, dummy, `--no-realtime`, period 128 | 128 (2.67 ms) | 7 500 in 20 s (375/s) | 0 | in 133 / out 252 frames (2.8 / 5.3 ms, JACK's port latencies) | the JACK thread was not real-time (`--no-realtime`): Cantus tried, rtkit absent → refused, said so |
| ALSA | — | — | WSL has no ALSA card: listed, last | — | — | — |

The driver list with JACK running and PipeWire stopped: `JACK` (default), `ALSA`; with
PipeWire running: `PipeWire` (default), `ALSA` (JACK not offered: no server).

**Libraries missing at run time.** The program's `NEEDED`: `libdbus-1.so.3`,
`libpipewire-0.3.so.0`, `libasound.so.2`, libc/libm/libgcc — no `libjack` (opened at run
time; the build no longer needs JACK at all). `test-fallback.sh` on the AppImage in WSL,
whose bare system has no libpipewire (libasound lent, as every desktop has it): *"OK:
started with the stand-in libpipewire; ALSA offered, PipeWire not"* — the stand-in has
26 symbols. In CI (Ubuntu 22.04, which has libpipewire) the same test forces the stand-in.

**Plug-in editors.** `clap_window_x11.rs` on WSLg: the test plug-in's view is a child of
our window at 320 × 200 (the window manager applies the size hints), its 20 ms timer ran
≥ 5 times in 300 ms of frames, its expose was read through the run loop and drawn, the
close box closes it. The picture, taken from the X server:
`docs/round30/linux-x11-editor.png`. VST3: the view attaches to an X11 id, its timer
(≥ 5 calls in 400 ms) and its descriptor (once, one byte) run from `idle`, and stop when
it is removed.

**Isolation.** The sandbox's suites (bridge, faults 1–5, blame, sessions, freeze,
`rt_alloc`) pass on Linux. `rt_alloc`'s "late block" count failed once under the load of
three builders compiling at once (2 late blocks for 1) and passed 3 of 3 alone: a timing
test, as on Windows.

**Suites.** Linux: the whole workspace — 3 902 passed, 53 ignored; the 4 that failed in
that run were fixed and re-run green (the VST3 test read another builder's test plug-in
through a shared target folder; the Japanese-font test met `fc-match`'s any-face answer —
now `fc-list` only; the notices tests; `frame_budget`'s hover allocations, 18 against 16,
once, under load — green re-run). Windows: see the report's counts.

**AppImage.** `build-appimage.sh` in WSL, quick and full (two levels, launcher, plug-ins):
see the sizes below; `--version` and `--audio-drivers` run from the image.
