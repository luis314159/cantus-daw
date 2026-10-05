# Round 19 — devices (drivers, the input stream, latency, the Audio page)

Builder: **devices**, branch `r19/devices`. ADR 0025 §7, §8, §10.

## What is done

### Engine (`cantus-engine`)

- **`devices.rs`** — `drivers()` (cpal's available hosts, the default first),
  `input_devices(driver)` (name, channels, default flag), `output_devices_on(driver)`;
  `host(driver)` picks a host by name, case-insensitive, falling back to the default when
  the name is unknown or not built in. `ASIO_BUILT` says whether this build has ASIO.
- **Sample formats.** `InputSample` / `OutputSample` convert every format cpal knows
  (i8/i16/I24/i32/i64, u8/u16/U24/u32/u64, f32/f64) to and from `f32`, scaled by the half
  range (i16: 1/32768) so every value lands where its definition puts it.
  `push_converted` pushes a callback's samples into the ring without blocking; what does
  not fit is dropped and counted (`DeviceTelemetry::input_dropped`,
  `EngineHandle::input_dropped`). The output now also plays in the device's own format
  when it does not take `f32` (ASIO interfaces often take only integers), converting from
  a scratch buffer allocated on the UI thread.
- **`EngineHandle::start`** picks the host by `AudioConfig::driver`, opens the output as
  before, then the input (`input_device_name`: `None` the default — on ASIO the output
  device itself when it records, so one driver serves both directions; `Some("")` none)
  **at the output's rate**: the device's default config when it runs at that rate,
  otherwise the supported range that offers it (same channel count as the default
  preferred, then `f32`, then most channels). A fixed block size outside the input's
  range falls back to its own. The input's rings are made and attached first, and its
  stream starts **after the output's**, so the output — what must never fail — is up
  before the input is touched. If the input cannot open, the output runs without it and
  `EngineHandle::input_error()` says why. The input pushes into an `rtrb` ring of
  `max(8 × block, 16 384)` frames × channels, attached with
  `processor.attach_input(consumer, channels)`; `HandleRecorder`'s input name and channel
  count are filled.
- **Latencies.** The input callback measures `callback − capture` and the output
  callback `playback − callback` from cpal's timestamps (WASAPI: QPC), in frames at the
  engine's rate, smoothed (the timestamps jitter by a frame or two from one callback to the
  next, and a take should not move with that), and publish them through atomics
  (`DeviceTelemetry`, with callback counters).
  `EngineHandle::device_latency()` returns both (`DeviceLatency { input, output }`,
  `round_trip()`); `poll()` calls `sync_devices()`, which sets the record latency's input
  through `HandleRecorder::set_input_latency` (the one-function hunk in `recorder.rs`).
- **Loopback (`loopback.rs`)** — `measure_loopback` moved here and is re-exported from
  `recorder.rs`. It runs *inside the engine's own streams* (so it works on ASIO, which
  opens a device once): while a probe runs, the output callback plays it instead of
  calling the processor, and the input callback sends its samples to the probe's ring
  instead of the processor's. The probe is 8 Hann-windowed noise bursts (each its own
  noise, so no burst correlates with another), a quarter second apart; the input is
  recorded aligned the way the processor pops it (the frame captured at input index *k* is
  processed at output frame `first_pop + k`), so the round trip measured is **the one the
  engine sees**. An f64 FFT cross-correlates every input channel with the probe; the
  channel whose peak stands highest over the correlation's RMS wins (≥ 8), **and** each
  burst is then judged on its own at that lag (normalised correlation ≥ 0.3): at least 6
  of the 8 must agree, so a chance peak in noise or an echo-cancelled microphone gives
  `NotFound`, never a wrong number. The probe waits in silence until the input delivers
  (an input that has just opened can take a few hundred ms).
  The result is `round_trip − (reported input + output)`, the offset to store.
  Non-blocking API for the UI: `begin_loopback`, `poll_loopback`, `is_measuring`
  (`LoopbackResult`, `LoopbackError` with a locale key each); `measure_loopback` is the
  blocking wrapper the contract named.
- **A stalled device** — `EngineHandle::output_stalled()`: the output has not called back
  for a second while open (unplugged, or a driver that stopped); the Audio page warns.
  `device_callbacks()` gives the counts for a diagnosis.
- **ASIO** — cargo feature `asio` on `cantus-engine` (→ `cpal/asio`), `cantus-ui` and
  `cantus` (the app). See *Building with ASIO* below.

### UI (`cantus-ui`)

- **`views/settings_audio.rs`** — the Audio page of Settings in the kit: *Audio device*
  (driver, output device, input device with its channel count, sample rate, buffer size
  with its milliseconds; *Apply*, *Rescan devices*, what is running now, the errors —
  device, input, dropped samples), *Latency* (input, output and round trip in ms and
  samples as readouts; the record offset as a knob, *Measure* beside it, and what the
  last measurement found), *Inputs* (the open input's name and channel count, and a
  meter on each pair, 1–2, 3–4 …, with a held peak). Devices are scanned when the page
  first shows, when the driver changes and on *Rescan* — never every frame (WASAPI's
  enumeration costs tens of ms). Choosing another driver resets the device choices to
  that driver's defaults. The record offset applies at once (no restart) and is stored.
- `workspace_settings.rs` keeps the window and the other sections; its audio section and
  list moved into the new file. `app.rs`: the stored record offset is handed to every
  engine that starts (two 3-line hunks).
- Strings in `locales/areas/devices.{en,es}.txt`.
- `examples/shoot.rs` renders it: `cargo run -p cantus-ui --example shoot -- <dir> 1
  settings` (a demo interface: ASIO, eight inputs, latencies, a measured offset).

## Building with ASIO

What it needs: the Steinberg ASIO SDK (GPL-3-compatible since 2025), LLVM's `libclang`
for bindgen, and MSVC (its `vcvars` are invoked by `asio-sys`'s build script).

1. LLVM: install it (`winget install LLVM.LLVM`), then set
   `LIBCLANG_PATH=C:\Program Files\LLVM\bin` (this machine has both).
2. The SDK: download `https://download.steinberg.net/sdk_downloads/ASIO-SDK_2.3.4_2025-10-15.zip`
   (what `https://www.steinberg.net/asiosdk` redirects to), unzip it, and set
   `CPAL_ASIO_DIR` to the `ASIOSDK` folder inside (the one holding `common`, `host`).
   Without `CPAL_ASIO_DIR`, `asio-sys`'s build script downloads the SDK into the temp
   folder itself through PowerShell; a proxy that answers with an HTML page (Zscaler did
   here, for the `steinberg.net/asiosdk` redirect) breaks that, so the direct link is
   safer.
3. `cargo build -p cantus --features asio` (or `-p cantus-engine --features asio`). Use
   another `--target-dir` (`target/asio`) if you switch back and forth, or every switch
   rebuilds cpal and everything above it. **Done on this machine:** `cargo build -p
   cantus-engine --features asio --target-dir target/asio` and its tests build and run
   (SDK 2.3.4, LLVM in `C:\Program Files\LLVM`, MSVC 2022); the default build has no
   ASIO code at all (`ASIO_BUILT` is false, the driver list is cpal's default hosts).
4. At run time ASIO appears in Settings → Audio → Driver when a driver is installed (an
   interface's own, or ASIO4ALL / FlexASIO for testing).

## Tests

- `crates/cantus-engine/tests/devices.rs` (8 run always, 2 `#[ignore]`d that open the
  machine's devices):
  - `enumeration_never_panics` — every driver, an unknown driver name, at most one
    default each.
  - `integer_formats_convert_exactly` — i16/i32/u16/u8/i8/I24/U24/u32/f64 land exactly
    (i16::MIN → −1, 16384 → 0.5, u16 32768 → 0 …).
  - `output_formats_round_trip_every_step` — every i16 and u16 survives f32 and back;
    clamping, NaN → 0.
  - `the_ring_takes_converted_samples_and_drops_what_does_not_fit` — never blocks, counts
    the drop.
  - `the_correlator_finds_a_known_delay_to_the_sample` — delays 0, 1, 37, 480, 4801,
    30 000 found exactly under noise and a polarity flip; all 8 bursts agree at the lag and
    none a sample off; noise alone stays under the threshold and agrees with < 6 bursts.
  - `the_probe_measures_the_round_trip_the_processor_sees` — the probe driven through a
    simulated cable at blocks of 64/256/1024 with input delivery jitter: round trip =
    first pop + cable delay, to the sample, on the right channel of 1–4.
  - `a_probe_with_no_cable_says_so` (`NotFound`, `NoInputData`),
    `a_handle_without_a_device_refuses_to_measure`.
  - `live_streams_report_their_latencies`, `live_loopback` (ignored; `CANTUS_TEST_DRIVER`,
    `CANTUS_TEST_OUTPUT`, `CANTUS_TEST_INPUT=none|<name>`, `CANTUS_LOOPBACK_DUMP=<dir>`
    writes the probe and what came back as WAVs).
- `cantus-ui`: `settings::tests::the_recording_devices_round_trip_through_the_file`
  (driver, input, "no input" as `""`, the offset, and a pre-19 file reading as defaults).
  The lib tests (126), `kit_lint` (4), `app_flows` (73) pass; `cargo clippy -p
  cantus-engine -p cantus-ui --tests` is clean; `cargo fmt --check` is clean.
- The device tests pass in both builds, live ones included: the default (WASAPI) and
  `--features asio` with `CANTUS_TEST_DRIVER=ASIO` (Realtek ASIO).

### What the live runs showed on this machine (Realtek laptop audio, Windows 11)

- **WASAPI:** output 576–1512 frames (12–31 ms) reported depending on the run, input
  268–560 frames; both streams run (input ~100 callbacks/s, output ~400/s). The
  laptop microphone hears the speakers, but through its voice processing (echo
  cancellation): the correlation is smeared and only a few bursts come back, so the
  measurement correctly answers `NotFound` instead of a wrong number (an earlier version
  without the per-burst check returned a false 34 599-frame trip). A real cable was not
  available here.
- An input that has just opened took about 280 ms to deliver its first samples: the probe
  now waits in silence for the input before it plays.
- **ASIO (built here, see below):** the machine has *Realtek ASIO* and *UMC ASIO Driver*
  installed. Output alone on Realtek ASIO runs (the device plays **i24**, so the
  integer-output path is exercised: 80 callbacks in 0.8 s, 528 frames reported). With
  input **and** output on Realtek ASIO the driver stops calling back after a few buffers
  — cpal stops, re-creates and restarts the driver's buffers when the second direction
  is added, and this driver does not survive that. Starting the output first (as now)
  did not change it. `EngineHandle::output_stalled()` detects it (no callback for a
  second) and the Audio page says so.
- **ASIO and scanning:** cpal's ASIO device list loads every installed driver in turn;
  after a scan had loaded *UMC ASIO Driver* (no interface plugged in), opening *Realtek
  ASIO* crashed the process (access violation). The scan now lists ASIO drivers from the
  registry (`HKLM\SOFTWARE\ASIO`, where they register, as `asio-sys` reads them) without
  loading any, so their channel counts read 0 (unknown; the page leaves them out), and the
  hosts are made once per process and shared. With that the whole test file runs on ASIO. To try on a real interface (the UMC); if it stalls
  too, the fix belongs in cpal's ASIO host (create both directions before the first
  `ASIOStart`).

## Notes for the other builders and the integrator

- **engine-rec:** until the processor pops `self.input`, the input ring fills and every
  further sample is dropped and counted (`input_dropped`, shown on the Audio page) —
  harmless, and it stops once the processor consumes. The channel count passed to
  `attach_input` is the device's (it can exceed `MAX_INPUTS`; meter and route the first
  `MAX_INPUTS`). If the input fails to start after the output, the processor still holds a
  ring that nothing fills (silence) and `input_channel_count()` is 0.
- **Latency for takes:** `RecordLatency::input` is set from the driver's input report on
  every `poll()`. The *device's* output latency (the DAC side, `device_latency().output`)
  is not ADR 0022's plug-in latency: a take should move by `device input + device output +
  plug-in latency (ADR 0022) + offset`. engine-rec / the integrator should make
  `RecordLatency::output` = `device_latency().output` + `stats().latency_samples`. The
  loopback offset is measured against `device_latency().round_trip()`, consistent with
  that. Any fill the drift logic deliberately keeps in the input ring (its target) adds
  to the input's latency: add it to the input side too.
- `HandleRecorder::set_input_latency` is the one-function hunk in `recorder.rs`; the
  `measure_loopback` stub there is now `pub use crate::loopback::measure_loopback;`.
- `cantus-engine`'s `[dev-dependencies]` gained `env_logger` (the live tests log). The
  `asio` feature adds `asio-sys`, `bindgen`, `clang-sys` … to `Cargo.lock` (compiled only
  with the feature).
- Shared files touched: `backend.rs` (start, a `dev` field, `poll` calls
  `sync_devices()`, `output_devices()` delegates), `lib.rs` (2 lines), `recorder.rs` (2
  hunks), `app.rs` (the stored offset handed to each new engine: 2 hunks), `views/mod.rs`
  (1 line), `examples/shoot.rs` (a `settings` kind), the three `Cargo.toml`s (the
  feature), `settings.rs` (a test).
- The settings window is now 600 × 800 by default (the Audio page is longer).
