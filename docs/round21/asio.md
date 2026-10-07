# Round 21 — asio: one build with every driver, and ASIO that runs

Builder: **asio**, branch `r21/asio` (from `r20/integration`). ADR 0025 §10 and ADR 0032
amended (no new ADR number).

The owner, after trying round 20:

1. *"Shouldn't we be able to use any driver with one build? That's how modern DAWs
   work."* — the default build said "This build of Cantus was made without ASIO".
2. *"Even the ASIO build you gave me fails."* — Driver ASIO → ASIO4ALL v2 showed **No
   sound** and "ASIO4ALL v2 was disconnected…".

Both are fixed. Every distributed build has ASIO and WASAPI; ASIO4ALL (on a USB device)
runs input and output together for a minute without a gap, and a driver that stops is no
longer called "disconnected". Realtek's ASIO driver — and ASIO4ALL pointed at the same
built-in hardware — never stream on this laptop in any host (§1); Cantus says so and
offers the fix instead of claiming the device was unplugged.

## 1. What was wrong with ASIO (root cause, with evidence)

Reproduced on this machine (Windows 11; drivers *ASIO4ALL v2*, *Realtek ASIO*, *UMC ASIO
Driver* with the Behringer unplugged — never loaded on purpose) with the live tests in
`crates/cantus-engine/tests/devices.rs` and a cpal-only example,
`cargo run -p cantus-engine --features asio --example asio_duplex -- "<driver>" <mode> <s>`,
both traced with `CPAL_ASIO_TRACE=1` (added to the patched crates, below).

What the owner saw was **four faults stacked up**, three in cpal 0.18.2's ASIO host and
one in Cantus:

| # | Fault | Evidence |
|---|---|---|
| A | **ASIO4ALL sends `kAsioResetRequest` ~0.7 s after `ASIOStart`, before its first buffer (~1.1 s), then streams normally.** cpal ignores reset requests only while a stream is `Starting`; ours were already `Playing`, so cpal reported `StreamInvalidated`, and Cantus (round 20) mapped every `StreamInvalidated` to **lost** → "ASIO4ALL v2 was disconnected", red chip, "No sound". | Engine on unpatched cpal: `ERROR audio output lost: Stream reset was requested by the ASIO driver`. Trace: `0.666 s driver event kAsioResetRequest (output stream, state 2)`, `1.087 s first buffer callback`, then 1 632 callbacks in 20 s without a gap. |
| B | **Adding the input restarted a running driver.** cpal starts the driver when the *first* stream is built; building the second direction calls `ASIOStop`, `ASIODisposeBuffers`, `ASIOCreateBuffers` (both) and `ASIOStart` again — under the streams' mutex that the buffer callback takes, while `ASIOStop` waits for the callback in flight. | Engine on unpatched cpal, ASIO4ALL duplex: `started in 10.26 s`, `callbacks (in, out) after 0.8 s: (0, 0)`. With Realtek, `ASIOStop` of a running driver took **10.0 s** (its timeout): `bufferSwitch #1 … 0.001 s`, `stopping the running driver`, `bufferSwitch #2 … 10.037 s`. |
| C | **Finding one driver loaded every driver before it.** `output_devices().find(name)` loads each installed driver in turn; asking for *Realtek ASIO* loaded *ASIO4ALL v2* first, after which Realtek's driver failed to load, and the walk went on to load *UMC ASIO Driver* with no UMC attached (the round-19 crash path). Each ASIO4ALL load takes 6–13 s here, and it was loaded twice per open. | Unpatched probe (cpal 0.18.2 from crates.io): `no ASIO driver named Realtek ASIO`. Trace: `load ASIO4ALL v2: loading` → `ASIOInit` at 12.75 s, unload, load again 13.73 → 22.30 s. |
| D | **Cantus: a stall and a reset were "lost", and a reopen fought the old device.** `device_lost()` was *stream lost or no callback for 1 s* (ASIO4ALL's first callback takes 1.1–1.8 s: "lost" every time); `restart_audio` opened the new engine **before** dropping the old one, so ASIO4ALL (kernel streaming) tried to open the Realtek endpoint Cantus still held through WASAPI. | `devices.rs`/`routing.rs`/`app.rs` of round 20; the owner's message names "disconnected". |

**Realtek ASIO** is a different story: on this machine the driver itself stops after a
three-buffer pre-roll — `bufferSwitch` 0, 1, 0 within 1 ms of `ASIOStart`, then nothing —
with output **alone** too, with nothing else streaming to its endpoint (checked with the
endpoint's audio sessions: all inactive), with the host thread pumping window messages,
with `ASIOOutputReady` forced on, and with no `ASIOGetSamplePosition` calls. Our callbacks
return in microseconds (traced: *enter → callbacks done* on the driver's thread). It also
refuses every rate in `ASIOCanSampleRate` and now offers only a 1 024 buffer, where round 19
saw ~480. Later the same day, with the Jabra USB headset unplugged, **ASIO4ALL** — then
exposing the same built-in hardware (4 inputs, 256 buffer) — gave no buffer at all either,
cpal-only example included. The built-in Realtek/Intel SST audio does not stream through
ASIO or kernel streaming on this laptop: that is the driver and hardware
(`rthdasio64.dll`, the SST path), not the host. Cantus now reports it as
*"The ASIO driver stopped responding"* with *Restart audio*, restarts once by itself, and
the audio setup keeps suggesting WASAPI for the computer's own audio (as round 20 did).

## 2. The fix

### cpal and asio-sys, patched (`third_party/`, `[patch.crates-io]`)

Vendored unchanged first (commit `37db4bc`), then patched; `third_party/README.md` lists
every change (each marked `CANTUS PATCH`), the licence (Apache-2.0) and the text offered
upstream. In short:

- **Buffers created once, before the first start**: building a stream on a stopped driver
  only records its direction; `Stream::play()` creates the buffers of every recorded
  direction in **one** `ASIOCreateBuffers` and calls `ASIOStart` once. Cantus builds output
  and input, then plays both. A direction added to a running driver still rebuilds it,
  but the driver is stopped **before** the streams' mutex is taken.
- **A reset/resync request before the first buffer** is part of starting and is ignored
  (traced) — fault A.
- **`device_by_id` loads only the driver named and keeps it loaded** while the device lives,
  so the stream build does not load it again — fault C (one 6 s ASIO4ALL load instead of
  two; no other driver touched).
- A driver that refuses every rate keeps its current one (Realtek), so its input is offered.
- `is_available()` follows `asio_sys::SDK_LINKED`; asio-sys looks for the SDK
  (`CPAL_ASIO_DIR`, else the cache `xtask asio-sdk` fills), never downloads it, and without
  it (or without libclang) builds stubs with a cargo warning — unless
  `ASIO_SYS_REQUIRE_SDK=1` (release builds) makes that an error.
- `CPAL_ASIO_TRACE=1` traces loading, `ASIOInit`, `ASIOCreateBuffers`, `ASIOStart`, every
  driver message (with the stream's state and whether buffers flow) and the first buffer
  switches step by step. That is how faults A–C and Realtek's pre-roll were told apart.

cpal 0.18.2 is the newest release; its `master` has a new duplex API for ALSA, JACK and
WebAudio and nothing for ASIO's buffer cycle (checked 2026-10-06).

### Cantus's own handling (`cantus-engine`, `cantus-ui`)

- **The device has a state** (`DeviceState`: Off, Starting, Running, Stalled,
  RestartRequested, Lost), classified by a pure function (`devices::classify`) from what
  the streams report and from **clocks the callbacks stamp themselves** (`Beat`), so a slow
  UI frame never looks like a stall. Only `DeviceNotAvailable`/`HostUnavailable` is **Lost**;
  `StreamInvalidated` (an ASIO reset, a rate the driver changed, WASAPI's invalidated
  resources) is **RestartRequested**; no callback for 1 s once running — or none within a
  4 s start grace — is **Stalled**, for the output *or* an open input.
- **Said in words**: "The ASIO driver stopped responding. Restart the audio to bring it
  back…" (`Stalled`), "The ASIO driver asked to be restarted…" (`Reset`), with *Restart
  audio*; the chip reads "UMC204HD · not responding" in red, never "disconnected". Only a
  gone device is "disconnected".
- **One automatic restart**: when the device stalls or asks to be reset, Cantus reopens it
  with the same settings once and says so on the status line; if it happens again within
  60 s the problem stays on screen with its button (never during a recording).
- **A deliberate reopen does not fight the watcher**: `restart_audio` closes the running
  device first (ASIO loads one driver at a time; ASIO4ALL needs the endpoint free), and the
  hot-plug watcher's news waits 3 s after a reopen (an interface's endpoints can flicker
  while its driver reopens) and is acted on afterwards — a real unplug is still noticed.
- **What the driver dictates is read back and shown**: the buffer the output really runs
  (`EngineHandle::device_buffer()` — the last callback's frames, else the stream's own
  report), the rate (`rate_refused()` when an ASIO driver cannot run at the one asked, and
  the engine falls back to the driver's own instead of failing). The chip and popover
  print what runs; on ASIO a different buffer than the one chosen is a problem in words
  ("The driver runs 512 samples, not the 128 chosen: ASIO drivers often set the buffer in
  their own control panel") with *Use 512 samples*. A buffer outside the device's range is
  left to the driver instead of failing the open.
- **Opening order**: output and input are built, then started (output first). On ASIO the
  input is always the output's driver (one driver at a time); a named driver that does not
  load is an error, never a silent switch to another driver.

## 3. One build with every driver (the decision)

- **Releases: always ASIO + WASAPI.** `cargo run -p xtask -- package` fetches the SDK
  (`xtask asio-sdk`: PowerShell's `Invoke-WebRequest`, then `curl.exe`, into
  `%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\`, the zip checked against the pinned
  SHA-256 `d5ebf0c2…3a335e2eca`, a proxy's page refused), builds with
  `ASIO_SYS_REQUIRE_SDK=1`, then runs `cantus.exe --audio-drivers` and **refuses to package
  a program that does not list ASIO**. One edition: `Cantus-<v>-windows-x64-setup.exe`,
  `…-portable.zip`, the plug-ins zip, `SHA256SUMS.txt`. The `-asio` names, `--asio`/`--all`
  and the updater's flavour are gone (`cantus_update::release`); the installer always
  offers the FlexASIO step and deletes the old `Flavor` registry value. The release
  workflow runs `xtask asio-sdk` before testing and packaging.
- **Developers: ASIO on by default, degrading.** `cargo build -p cantus` turns the `asio`
  feature on. With the SDK cached (one `cargo run -p xtask -- asio-sdk`) and LLVM installed
  it has ASIO; without them it still builds, prints *"building WITHOUT ASIO: …; run cargo
  run -p xtask -- asio-sdk"*, and the app says it is a **developer build** without the
  SDK (never "install the ASIO edition"). `cargo test -p cantus-engine`/`-p cantus-ui`
  without `--features asio` are unchanged for other builders.
- Recorded in **ADR 0025 §10** (amended) and **ADR 0032** (§1 ASIO, §2, §7).

## 4. Tests

Always run (fakes, no device):

- `cantus-engine/tests/devices.rs`: `a_stalled_driver_is_stalled_not_lost` (running,
  stalled output, stalled input, an input that never started, the start grace, a reset
  request, lost wins), `a_reset_request_asks_for_a_restart_and_an_unplug_is_lost` (cpal's
  errors → telemetry), `the_beat_says_how_long_ago_a_callback_ran`,
  `a_handle_without_a_device_is_off` — 12 pass (3 live ones ignored).
- `cantus-ui/tests/audio_setup_flows.rs`:
  `a_stalled_driver_is_restarted_once_and_never_called_disconnected` (one automatic
  restart, then the problem with *Restart audio*; the chip red "not responding"; no "lost"
  card; the words checked in EN and ES; a reset request restarted too; a real loss still
  lost), `a_deliberate_reopen_does_not_trigger_the_lost_watcher` (a scripted watcher whose
  UMC endpoints vanish during the reopen and return: nothing lost; a real unplug after the
  quiet moment is), `the_buffer_the_driver_dictates_is_shown`.
- `cantus-update/tests/check.rs` (one edition's names), `xtask` package tests (one
  edition's file list and zips; `the_asio_sdk_is_checked_and_found`: the pinned hash equal
  in `release.yml`, a proxy page and a wrong file refused, the SDK found in a zip's tree and
  unpacked, `--audio-drivers` read).

Live (this machine; `CANTUS_TEST_SECONDS=60`; `live_streams_run_without_a_stall` fails on a
gap of 0.5 s in either direction once started, or a lost device):

| Driver, mode | Result |
|---|---|
| ASIO4ALL v2, **duplex** (in + out), 60 s | **pass** — one driver load (6 s, ASIO4ALL's own), `ASIOCreateBuffers: 2 in + 2 out`, one `ASIOStart`; the start-up reset request ignored; first buffer at 1.6 s; 5 033 callbacks each way, longest gap 69 ms, 0 xruns, state Running, 44.1 kHz / 512 read back |
| ASIO4ALL v2, **output only**, 60 s | **pass** — 5 014 callbacks, longest gap 75 ms |
| WASAPI (default devices), duplex, 60 s | **pass** — 5 997 / 5 969 callbacks, longest gap 95 ms, buffer 480 read back (5 capture xruns under a heavily loaded machine; no stall) |
| Realtek ASIO, output only / duplex | **fails, in the driver** — 3 pre-roll buffers then nothing (state Stalled after 1 s; the app says "not responding"); same on unpatched cpal, which cannot even open it (see §1) |
| ASIO4ALL v2 after the Jabra USB headset was unplugged (later the same day) | **no callbacks at all**, output only too, cpal-only example included — ASIO4ALL now exposes the built-in Realtek/Intel SST hardware (4 inputs, 256 buffer) and that hardware never streams through kernel streaming, exactly like Realtek's own ASIO driver. The passing runs above were ASIO4ALL on the **Jabra EVOLVE 20** (USB, 2 in / 2 out at 44.1 kHz). Cantus reports it as stalled, restarts once, then offers *Restart audio*; it is never "disconnected". |

Commands: `CANTUS_TEST_SECONDS=60 CANTUS_TEST_DRIVER=ASIO CANTUS_TEST_OUTPUT="ASIO4ALL v2"
[CANTUS_TEST_INPUT=none] [CANTUS_TEST_RATE=48000] [CANTUS_TEST_BUFFER=256] cargo test -p
cantus-engine --features asio --test devices -- --ignored --nocapture
live_streams_run_without_a_stall` (add `CPAL_ASIO_TRACE=1` for the trace).

Suites (this branch, `--features asio`, SDK linked): `cantus-engine` all pass (devices: 12 +
3 ignored live); `cantus-ui` **602 passed, 0 failed** (`samples_browser`'s
`the_starter_offer_installs_the_recommended_pack_in_one_click` failed once inside the full
run on a machine loaded by the parallel round and passed alone — not this area);
`cantus-update` 24 pass; `xtask` 7 pass (the installer compiled with NSIS 3.11).

Release: `cargo run -p xtask -- asio-sdk` fetched the SDK through the Zscaler proxy
(PowerShell's request failed once, `curl.exe` got it; hash matched), then `cargo run -p
xtask -- package` built `cantus` (release) with `ASIO_SYS_REQUIRE_SDK=1`, checked
`cantus.exe --audio-drivers` → `audio drivers: WASAPI, ASIO` / `asio sdk: linked`, bundled
the 10 plug-ins and wrote `Cantus-0.1.0-windows-x64-setup.exe` (25.7 MB),
`…-portable.zip` (64.4 MB), `Cantus-0.1.0-plugins-windows-x64.zip` (52.0 MB) and
`SHA256SUMS.txt` — one edition, no `-asio` files. `package --no-build --no-installer`
then wrote the same names. **Found on the way (round 20's, not changed here):** `bundle
--no-build` lays out the *last* plug-in library built under every plug-in's name (each
became the 12 MB Fervor), so `package --no-build` must follow a full `package` only if
nothing else was bundled since — the integrator may want `bundle` to keep one library per
plug-in. The owner's copy: `C:\projects\daw-r21\builds\cantus.exe` (the release
program, ASIO linked).

## 5. Files

- **Patched crates** (new): `third_party/cpal/` (cpal 0.18.2; `src/host/asio/{stream,device,mod}.rs`
  changed), `third_party/asio-sys/` (0.4.0; `build.rs`, `src/lib.rs`,
  `src/bindings/mod.rs` changed; `LICENSE` added), `third_party/README.md`; workspace
  `Cargo.toml` (`[patch.crates-io]`, `exclude`), `Cargo.lock`.
- `cantus-engine`: `src/devices.rs` (`asio_built()`, `DeviceState`, `classify`, `Beat`,
  `restart_requested`/`output_frames` telemetry, `stream_error` mapping, `pick_output`,
  `pick_input` on ASIO, `output_rate`, `buffer_request`, `InputPlan::build`,
  `device_state`/`device_buffer`/`rate_refused`), `src/backend.rs` (**only
  `EngineHandle::start`**: device lookup, rate/buffer read back, build both then start; plus
  a marked TODO for the rt builder's MMCSS hook), `src/routing.rs` (`device_lost` = gone
  only), `src/lib.rs` (export), `Cargo.toml` (feature note, example), `examples/asio_duplex.rs`
  (new), `tests/devices.rs` (4 always-run tests, a 60 s live test, rate/buffer variables).
- `cantus-ui`: `src/app.rs` (`restart_audio` closes the old device first),
  `src/audio_setup/{mod,apply,plug,problems,demo,drivers}.rs` (state in `Live`, automatic
  restart, reopen quiet, `Reset`/`Dictated`/`RateRefused` problems, `report()`),
  `src/views/{audio_chip,audio_popover,audio_drivers,audio_firstrun,audio_popover_lists,settings_audio,update}.rs`,
  `locales/areas/devices.{en,es}.txt`, `tests/audio_setup_flows.rs` (3 tests),
  `tests/audio_setup_first.rs` (build-agnostic suggestion).
- `cantus-app`: `Cargo.toml` (`default = ["asio"]`), `src/main.rs` (`--audio-drivers`).
- `cantus-update`: `src/release.rs` (no flavour), `src/updater.rs`, `src/lib.rs`,
  `src/flexasio.rs` (comment), `tests/check.rs`, `tests/fixtures/{cantus-latest.json,SHA256SUMS.txt}`.
- `xtask`: `src/asio_sdk.rs` (new), `src/package.rs`, `src/package_tests.rs`, `src/nsis.rs`,
  `src/main.rs`, `installer/cantus.nsi`, `installer/flexasio.ps1`.
- `.github/workflows/release.yml`; ADR 0025 §10, ADR 0032; notes at the top of
  `docs/round19/devices.md` and `docs/round20/packaging.md`; this file.

**For the integrator:** the rt builder's request — `processor.promote_on_first_block(
AudioHost::from_driver_name(..))` — is a marked one-line TODO in `EngineHandle::start`
(`backend.rs`, right after the processor is made; `host.id().name()` is the driver name).
The graph builder owns the rest of `backend.rs`; this branch changed only `start` there.
`cargo test --workspace` now builds asio-sys (stubs when the SDK is absent, with a cargo
warning); CI fetches the SDK first.

## 6. For the owner — the Behringer UMC

1. Run the new build (`C:\projects\daw-r21\builds\cantus.exe`, or the setup from
   `xtask package`). It has ASIO without any special edition: audio chip ▸ Driver ▸
   **ASIO**. `cantus.exe --audio-drivers` prints `audio drivers: WASAPI, ASIO`.
2. Plug the UMC in **before** choosing *UMC ASIO Driver* (with the interface absent its
   driver must not be loaded; Cantus no longer loads it on the way to another driver).
3. Choose the UMC's card (or Driver ASIO ▸ *UMC ASIO Driver*). Input and output open
   together now. If the chip says a buffer other than the one you picked, that is the
   UMC driver's own setting (its control panel): Cantus shows what it runs.
4. Strum: the meter should move on Input 1; play for a minute and watch the chip stay
   green. If it ever says *not responding*, Cantus restarts it once by itself; if it says
   it again, click *Restart audio* and tell me what the status line said.
5. To send me a trace: start Cantus from a terminal with `set CPAL_ASIO_TRACE=1` and
   `set RUST_LOG=info`, reproduce, and copy what it prints.
6. The laptop's built-in sound (Realtek on Intel Smart Sound) does not stream through ASIO
   here — neither Realtek's own ASIO driver nor ASIO4ALL pointed at it gets past three
   buffers, in any host. Use WASAPI for the built-in sound; ASIO4ALL works with a USB
   device (it ran the Jabra headset for a minute without a gap). The UMC's own driver is
   the one to use with the UMC.
