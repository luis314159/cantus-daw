# Round 19 briefs — the engine side

## engine-rec — monitoring, recording and audio-clip playback in the engine

You own `crates/cantus-engine/src/recorder.rs`, the processor's input / monitoring /
recording / audio-clip paths, `samples.rs`'s audio loading, render and stems with audio,
and `cantus-core/src/snapshot.rs::compile_audio`.

**Input.** At the top of `Processor::process`, pop one block of input from `self.input`
into a preallocated buffer (`MAX_BLOCK_FRAMES × MAX_INPUTS`, deinterleaved). Two clocks
drift (input and output devices may differ): keep the ring near a target fill — when it
holds more than the target plus a block, drop the surplus a frame at a time; when it holds
less than a block, use what is there and hold the last frame; count both in `EngineStats`
(add fields). Prime: start consuming once a block is waiting. Meter every channel into
`Meters::{input_count, inputs}`.

**Monitoring** (ADR 0025 §4). Per track with `TrackSnapshot::input`: `Off` never; `On`
always; `Auto` while armed and (stopped, or recording). The input (mono into both sides,
or the stereo pair) × trim enters the track's signal **before** the channel's level and
balance — where the instrument's output is — so the strip's inserts apply. An idle track
that is monitoring is not idle. Works while the transport is stopped.

**Recording.** While the transport plays with recording on (`SetRecording`) and rings are
attached, write each armed track's input × trim (what is recorded is what is heard,
before any effect) into its `TrackRing`. Never block: a full ring drops and sends
`RecordMark::Overflow`. Send `Started { tick }` with the engine position of the first
recorded frame, `Wrapped { frames }` on each loop wrap, `Stopped { frames }` when the
transport stops or recording goes off.

**The handle** (`recorder.rs`, keep the contract's signatures): `begin_recording(dir)`
builds rings (≥ 4 s each) for the armed tracks with an input in the last snapshot sent
(keep a reference to it in `set_snapshot`), spawns a **writer thread** that owns the
consumers and a `hound` 32-bit-float WAV per track (file names from the track name plus a
date-time stamp, sanitised, unique in `dir`), flushes the header at least once a second,
and computes `LivePeaks` (min/max per `PEAK_FRAMES`, mono-summed) into a channel that
`poll_recording` drains. `finish_recording` detaches, lets the writer drain and finalise,
joins it, and returns `FinishedTake`s with `start_tick` already moved earlier by
`RecordLatency::total()` and `wraps` from the marks. `set_record_start` is the count-in
point. Handle the marks in `EngineHandle::poll` (the contract left an empty arm). The
input latency is set by the devices builder: expose a `pub(crate)` setter on
`HandleRecorder` for it; the output latency is the one ADR 0022 already measures.

**The click** (`Snapshot::recording`): `ClickMode::Always` must render exactly as today;
`Recording` clicks only while recording; `CountInOnly` only before the record start;
`click_db` is relative to today's level (0 dB = bit-identical).

**Audio clips** (ADR 0025 §1, §9). In the track loop, add each `AudioClipSnapshot` that
overlaps the segment: integer-sample playback (the clip's start rounded to the nearest
sample at the transport's rate — no interpolation, so a clip at 0 dB plays its file
bit-exact), `offset` in content ticks → frames the same way, gain, fade in/out with their
curves (`FadeCurve::gain`; fades never overlap — shorten both in proportion), reverse.
Mono files into both sides. Loop wraps and seeks land mid-clip correctly.
`SampleCache`: resolve a source's relative path against a project folder
(`SampleCache::set_project_dir`), decode (WAV/AIFF/FLAC/Ogg via the existing decoders),
**resample to the engine's rate on load** with a good windowed-sinc resampler (look at
`cantus_dsp::exemplar_resample`), cache by path and rate, attach to
`AudioClipSnapshot::data`; a missing file plays silence and is reported through
`SampleStatus`. Offline render and stems must attach the same way, so a bounce equals
real time.

**Tests (first):** monitored input at unity → strip bit-exact; armed-but-`Off` adds
nothing; an audio clip at 0 dB, no fades → its file bit-exact; block-size independence
(1…8192) with clips, fades and loops; offline = real time; record a known signal through a
manual processor, finish, then play the take back: lined up within one frame for a given
latency; a full ring drops and reports, never blocks; `Always` click bit-identical to
before; fades exactly 0 and 1 at their ends. Add an engine test file `tests/recording.rs`
and `tests/audio_clips.rs`.

## devices — drivers, the input stream, latency, the audio settings page

You own `crates/cantus-engine/src/devices.rs`, the stream setup in `backend.rs`
(`EngineHandle::start`), `measure_loopback` (move it into a `loopback.rs` of your own and
re-export it from `recorder.rs` with a one-line change), the `asio` cargo feature, and
the settings dialog's audio page in `cantus-ui`.

- `drivers()` (cpal's available hosts by name), `input_devices(driver)` with channel
  counts and the default flag.
- `EngineHandle::start`: pick the host by `AudioConfig::driver`; open the output as today;
  open the input (`input_device_name`: `None` default, `Some("")` none) at the output's
  rate (if the device cannot, open no input and report why — add an `input_error()`
  accessor), convert any sample format to f32, push into an `rtrb` ring (capacity ≥ 8
  blocks), and `processor.attach_input(consumer, channels)`. On ASIO prefer one device
  for both directions. Fill `HandleRecorder`'s input name and channel count.
- **Latencies:** measure the input latency from cpal's callback timestamps (capture vs
  callback) on the input thread, publish through an atomic, and set it on the handle in
  `poll()` via the setter engine-rec adds (coordinate: if it is not there yet, add
  `pub(crate) fn set_input_latency(&mut self, frames: u32)` to `HandleRecorder` yourself —
  a one-function hunk).
- **ASIO:** a cargo feature `asio` on `cantus-engine` and `cantus-app` (cpal's `asio`).
  The ASIO SDK is needed at build time (`CPAL_ASIO_DIR`) and LLVM for bindgen. Try to
  build it here (Steinberg distributes the SDK; since 2025 it is dual-licensed GPLv3). If
  it cannot be built on this machine, keep the code behind the feature, prove the default
  build is untouched, and write exactly how to build it in `docs/round19/devices.md`.
- **Loopback measurement:** with a cable from an output to an input, play a short
  impulse train and find it on the input by cross-correlation; return the measured round
  trip minus what the drivers report. It may pause the engine's own stream while it runs.
- **Settings dialog** (find it: `views/workspace_settings.rs` and `settings.rs`): an
  Audio page in the kit's style with driver, output device, input device, sample rate,
  buffer size, the latencies in ms (input, output, round trip), the record offset with
  *Measure*, the input channels listed. Strings in a new area `locales/areas/devices.*`
  (pre-registered in the contract). Render it with the shoot example and look at it.
- Tests: device enumeration does not panic without devices; the config round-trips; the
  ring conversion of i16/i32/u16 formats is exact; the loopback correlator finds a known
  delay in a synthetic signal to the sample.
