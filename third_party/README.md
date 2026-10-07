# third_party — patched crates

Two crates from crates.io, vendored **with a small patch** and wired in through
`[patch.crates-io]` in the workspace `Cargo.toml`. Both are Apache-2.0 (`LICENSE` in each
folder; asio-sys's crate ships without the file, so the Apache-2.0 text from cpal, the same
project, is copied beside it). Every change is marked `CANTUS PATCH` in the source.

| Crate | Version | Upstream |
|---|---|---|
| `cpal` | 0.18.2 (crates.io, checksum `6f02e8d0…681f1`) | <https://github.com/RustAudio/cpal> |
| `asio-sys` | 0.4.0 (crates.io, checksum `477f1cf5…021b9`) | same repository, `asio-sys/` |

The first commit that added this folder holds both crates **unchanged**, so
`git diff <that commit> -- third_party/` is exactly the patch. Drop the folder and the
`[patch.crates-io]` lines once upstream carries the same fixes (checked 2026-10-06: cpal's
`master` has a new duplex API for ALSA/JACK/WebAudio, nothing for ASIO's buffer cycle).

## Why

Round 19 and the owner's tests in round 20 found Cantus's ASIO path failing:

1. **Duplex stall.** cpal's ASIO host starts the driver (`ASIOStart`) when the *first*
   stream is built. Building the second direction then calls `ASIOStop`,
   `ASIODisposeBuffers`, `ASIOCreateBuffers` (both directions) and `ASIOStart` again on a
   driver that has already streamed. Realtek's ASIO driver stops calling back a few buffers
   after that cycle (round 19). It also did all of this **while holding the streams' mutex**
   that the buffer-switch callback takes, and `ASIOStop` waits for the callback in
   progress: a driver whose stop waits on its callback thread could deadlock or hang there.
2. **Finding one driver loaded all the others.** `Host::output_devices()` loads every
   installed driver in turn to read its channels, so opening *Realtek ASIO* first loaded
   *ASIO4ALL v2*, and opening a driver after loading one whose interface is unplugged (the
   UMC's) crashed a process (round 19).
3. **The SDK at build time.** asio-sys's build script downloaded the SDK itself when
   `CPAL_ASIO_DIR` was unset (a proxy answering with its own page broke it) and failed the
   whole build without the SDK or libclang, so ASIO had to be an opt-in feature — and a
   default build had no ASIO at all.

## What changed

### cpal (`src/host/asio/`)

- **`stream.rs` — buffers created once, before the first start.** Building a stream on a
  driver that is not running only *records* the direction (channels, buffer size, with
  null buffer pointers). `Stream::play()` creates the buffers of **every recorded
  direction in one `ASIOCreateBuffers`** and then calls `ASIOStart` once
  (`start_driver`). So `build output → build input → play → play` (what Cantus does) never
  stops or rebuilds a running driver. Both directions share one buffer size (one
  `ASIOCreateBuffers` has one); without a fixed size it is the driver's preferred size
  (`ASIOGetBufferSize`), read before creation.
- **A direction added to a running driver** (a stream built while another plays) keeps
  cpal's behaviour — stop, rebuild both, restart — but the driver is stopped **before**
  the streams' mutex is taken (`get_or_create_stream`), so the callback never waits on a
  lock held across `ASIOStop`.
- Building a stream no longer starts the driver by itself; `play()` does (cpal's contract
  already says a stream may need `play()` to start).
- `CPAL_ASIO_TRACE=1` prints each step (recorded, `ASIOCreateBuffers`, `ASIOStart`, the
  first buffer callback, every driver message with the stream's state) with a timestamp:
  how the duplex behaviour was diagnosed, kept for the next driver that misbehaves.
- **`stream.rs` — a reset request before the first buffer is part of starting.** cpal
  ignores `kAsioResetRequest`/`kAsioResyncRequest` only while a stream is `Starting`.
  ASIO4ALL v2 sends a reset request ~0.7 s after `ASIOStart`, before its first buffer
  (~1.1 s), then streams steadily; cpal passed it on as `StreamInvalidated`, which an app
  must treat as "rebuild the stream" — and reopening only made it happen again. Requests
  that arrive before the stream's first buffer callback are now ignored (and traced).
- **`mod.rs`, `device.rs` — one driver at a time.** `HostTrait::device_by_id` on the ASIO
  host loads only the driver its id names (`Devices::named`) instead of walking every
  driver; Cantus finds ASIO devices with it. Driver-load failures during enumeration are
  traced instead of silently skipped. A device found this way **keeps its driver loaded**
  while it lives, so building its streams does not unload and reload it (loading ASIO4ALL
  takes 6–13 s on this machine; it was loaded twice).
- **`device.rs` — a driver that refuses every rate.** Realtek's ASIO driver answers
  `ASIOCanSampleRate` with an error for every rate, the one it runs at included, which left
  the device with no configuration (so no input). The driver's current rate is kept.
- **`mod.rs` — `is_available()`** returns `asio_sys::SDK_LINKED`: a build without the SDK
  (below) does not offer an ASIO host that can never load a driver.

- **`src/host/mod.rs`** — `#[allow(dead_code)]` on `frames_to_duration` (unused in a
  Windows build without `asio`; a path dependency's warnings are not capped and CI denies
  warnings).

### asio-sys

- **`build.rs` — the SDK is looked for, never downloaded.** `CPAL_ASIO_DIR` (must hold the
  SDK), else `%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\ASIOSDK` (where `cargo run
  -p xtask -- asio-sdk` puts it after checking its SHA-256). libclang from `LIBCLANG_PATH`,
  else `C:\Program Files\LLVM\bin`. Without either the crate builds its **stub bindings**
  (which load no driver), prints a `cargo:warning` saying so and how to fix it, and
  rebuilds by itself once the SDK appears in the cache. `ASIO_SYS_REQUIRE_SDK=1` turns that
  into a hard error — release builds set it.
- **`src/lib.rs` — `SDK_LINKED`**: whether the real SDK is linked (`cfg(asio_sdk)`).
- **`src/bindings/mod.rs` — `Driver::is_running()` / `has_buffers()`**, which the cpal
  patch needs to stop a running driver before taking its lock; `trace()` (the same
  `CPAL_ASIO_TRACE=1`): loading, `ASIOInit`, `ASIOStart`, whether the driver uses
  `ASIOOutputReady`, every `asioMessage`, and the first buffer switches step by step (the
  thread, entering, the callbacks done, `ASIOOutputReady` done) — how a driver that stops
  calling back is told from a host that blocks it.

## Evidence (this machine, Windows 11, 2026-10-06)

`docs/round21/asio.md` has the full runs. In short:

- Unpatched cpal 0.18.2 (a probe built from crates.io), asking for *Realtek ASIO*: the
  enumeration loads *ASIO4ALL v2* first, Realtek's driver then fails to load, and the walk
  goes on to load *UMC ASIO Driver* with no UMC attached — the device is never found.
- Engine on unpatched cpal, *ASIO4ALL v2* duplex: `started in 10.26 s`, no callback after
  0.8 s, and *"audio output lost: Stream reset was requested by the ASIO driver"*.
- Patched, same: one load, one `ASIOCreateBuffers: 2 in + 2 out`, one `ASIOStart`, the
  start-up reset request traced and ignored (`buffers flowing false`), 60 s with 5 033
  callbacks in each direction, longest gap 69 ms.

## Offered upstream

The patch is meant for RustAudio/cpal as two pull requests:

1. *asio: create both directions' buffers in one ASIOCreateBuffers before the first
   ASIOStart; stop a running driver before taking the streams lock* (`stream.rs`).
2. *asio: ignore reset/resync requests before the first buffer* (ASIO4ALL's start-up
   request).
3. *asio: `device_by_id` loads only the named driver and keeps it loaded; a driver that
   refuses every rate keeps its current one; `is_available` follows the SDK* plus
   *asio-sys: do not download the SDK in build.rs; build stubs with a warning when it is
   missing* (the last may be a feature flag upstream, since it changes behaviour).

Text for the first, ready to paste:

> **ASIO: duplex streams restart a running driver.** Building an input stream after an
> output stream (or the reverse) on the same ASIO driver stops the running driver,
> disposes its buffers, creates both directions and starts it again. Several drivers
> (Realtek ASIO here) stop calling back after that cycle; others take seconds. This defers
> `ASIOCreateBuffers`/`ASIOStart` to `Stream::play()`: building only records the
> direction, and the first `play()` creates every recorded direction in one call and starts
> the driver once. Adding a direction to a running driver still rebuilds it, but the driver
> is now stopped before the streams mutex is taken, since the buffer callback takes that
> mutex and `ASIOStop` waits for the callback in flight.
