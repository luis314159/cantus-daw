# Round 19 — engine-rec: monitoring, recording and audio clips in the engine

Builder: engine-rec · branch `r19/engine-rec` · ADR 0025 (and 0005, 0022).

## What the engine does now

**Input** (`processor_rec.rs::Processor::read_input`). At the top of every block, after the
messages, the processor pops one block from the input ring (`Processor::attach_input`),
deinterleaves it into a preallocated `MAX_BLOCK_FRAMES × MAX_INPUTS` buffer and meters
every channel into `Meters::{input_count, inputs}`. Two clocks: the ring is kept near one
block of fill. Above the target plus a block, the surplus is shed — a frame per block while
it is under a block (inaudible drift correction), all of it when it is more (a stall's
backlog); below a block, what is there is used and the last frame held for the rest; an
empty ring gives silence and the ring primes again (it waits for a whole block). Counted
in `EngineStats::{input_dropped, input_repeated}`. Without an input, `input_count` is 0.

**Monitoring** (`Live::add_track_audio`). Per track slot the snapshot's `TrackInput` is
copied into a fixed table at each snapshot. Off never; On always; Auto while armed and
(stopped, or recording). The input × trim (mono into both sides, or the stereo pair)
joins the track's signal where the instrument's output is — before the channel's level,
balance and the strip, so the inserts apply. A monitoring track is never idle. Works
stopped. Trim at 0 dB is exactly 1.0.

**Recording** (`Processor::record_segment`, per segment, before the track loop). While the
transport plays with `SetRecording(true)` and rings are attached, each ring's track gets
its input × trim (dry: before any effect) as whole frames. A ring without room for the
segment loses the segment for that track: `EngineStats::record_overflow` and, once per
block, `RecordMark::Overflow { track, frames }`. Marks: `Started { tick }` (engine
position of the first recorded frame) when writing begins, `Wrapped { frames }` in
`wrap_loop`, `Stopped { frames }` at the next block start once the transport stopped,
recording went off or the rings were detached. Frames count from `AttachRecorder`, so a
pass that toggles recording off and on keeps one frame count (the handle keeps the first
`Started`).

**The handle** (`recorder.rs`, contract signatures kept). `set_snapshot` keeps the last
snapshot (`HandleRecorder::snapshot`). `begin_recording(dir)`: for every armed track with
an input, a ring of `RING_SECONDS` (8 s) and a 32-bit float WAV named
`<track name> YYYY-MM-DD HHMMSS.wav` (UTC — there is no time-zone source without a new
dependency; sanitised; ` 2`, ` 3`… when taken); a writer thread
(`cantus-record-writer`) drains the rings, flushes every header every 0.5 s, and sends
`LivePeaks` (min/max of the channel mean per `PEAK_FRAMES`) over a channel that
`poll_recording` drains and merges per track. `finish_recording`: sends `DetachRecorder`;
on a live stream waits (≤ 2 s) for the rings to come back — then every frame and mark is
in; stops the writer, which drains and finalises; returns one `FinishedTake` per track
that recorded something (an idle track's file is deleted). `start_tick` = `Started` −
`RecordLatency::total()` frames at the pass's tempo; `wraps` are the interior wrap points
only (the ends `0` and `frames` are implicit); `dropped` sums the overflow marks.
`EngineHandle::poll` folds `Record` marks and the input meters; `input_peaks()` returns
the peaks since its last call. `RecordLatency::output` follows `EngineStats::latency_samples`
(ADR 0022's number) from `poll`; `HandleRecorder::set_input_latency(frames)` is the
`pub(crate)` setter the devices builder calls; `set_record_offset` is the user's.

**The click.** `ClickMode::Always` unchanged (bit-identical, tested against the click
model). `Recording` triggers only while recording is on; `CountInOnly` only before the
record start (`set_record_start` → new `EngineMessage::SetRecordStart(Option<Ticks>)`);
no record start, no click. `click_db` sets `Click::set_gain(0.5 · 10^(dB/20))` — 0 dB is
exactly the old 0.5. (The click's gain clamps at 1.0, so +6 dB is the top.)

**Audio clips** (`processor_rec.rs::add_clips`, in the track loop). Integer-sample
playback: a clip starts at the transport sample nearest its start tick (computed from the
transport's origin and sample counter, `Transport::{origin_tick, samples}`, so it is the
same for every block size and across loop wraps and seeks); the file frame under sample
`s` is `round(offset) + (s − start)`; gain; fades in frames from the edges — the fade in
is exactly 0 on the first frame and 1 from its length; the fade out mirrors it onto the
last frame; overlapping fades are shortened in proportion; reverse reads the window
backwards; mono into both sides. No interpolation, so 0 dB and no fades is the file to
the bit. Clips only sound while playing.

**`compile_audio`** (snapshot.rs): lane 0, unmuted, source in the pool, sorted by start. A
**looping** audio clip (`content_length`) is now expanded into one window per
repetition (at most 4096), the clip's fade in on the first piece and fade out on the last.

**Loading** (`samples.rs`). `SampleCache::set_project_dir`, `resolve`, `load_audio(path,
rate)` (WAV/AIFF/FLAC/Ogg, ≤ 2 channels, cached by resolved path and rate, never evicted,
failures remembered until `retry_audio`), `attach_audio(snapshot, rate)` (errors returned,
the clip silent), `audio_status(path)`. `EngineHandle::compile` attaches at the device's
rate. Conversion is `samples/resample.rs`: an exact-rational-phase Kaiser-windowed sinc
(β 10, 32 zero crossings, cutoff 95 % of the narrower Nyquist, taps normalised for DC),
split across cores; same rate in is the same samples out. `samples::conform_audio`
converts attached data for an offline render or stems at another rate; `render` and the
stems pass call it, so a bounce at the device's rate is what was heard.

## Tests

All of `cantus-engine` and `cantus-core` pass (engine: 20 unit + every integration file;
core: 80 unit + undo fuzz); `cargo clippy -p cantus-engine -p cantus-core --tests` clean;
`cargo fmt` applied. New: `audio_clips.rs` 10, `recording.rs` 17, `recording_rt.rs` 1,
unit 8. `cantus-project` was not re-run (only `compile_audio` changed in core).


- `tests/audio_clips.rs` — a clip at 0 dB plays its file bit-exact (to the output and,
  through a strip, in the track stem); mono into both sides; fades exactly 0 and 1 at
  their ends for every curve; overlapping fades shortened in proportion; gain, offset and
  reverse; a render from inside a clip lands mid-clip; block-size independence (1…8192)
  with clips and fades; loops wrapping mid-clip at an awkward tempo, identical for every
  block size and irregular sizes; offline = real time.
- `tests/recording.rs` — monitored input at unity → output bit-exact; through a strip
  (−6 dB law → exactly half); trim; the monitor-mode table (armed-but-Off adds nothing);
  input meters per channel; the ring's drift handling (prime, shed one, shed a backlog,
  hold the last frame, run dry); a pass records the input bit-exact as 32-bit float with
  a sane name; a stereo pair with trim; nothing armed / idle pass leaves no file; loop
  wraps marked at 24 000 / 48 000; record → finish → play back lines up (within one frame
  when the latency is a whole tick, within half a tick otherwise — see below); a full
  ring drops, reports and never blocks; live peaks arrive and headers are flushed during
  the pass; the `Always` click bit-identical to the click model; click level; `Recording`
  and `CountInOnly` modes.
- `tests/recording_rt.rs` — counting allocator: monitoring, two rings (one overflowing),
  loop wraps, a clip with a fade and the click for three seconds: zero heap events in
  `process`.
- Unit: `samples::resample` (identity, sine accuracy both directions < −80 dB, nothing
  folds, DC, an odd rate pair without a table), the writer's file names and UTC stamp,
  `compile_audio` (windows, takes/muted/orphans excluded, loop pieces and their fades).

## Files outside my area

- `crates/cantus-engine/src/messages.rs`: `EngineStats::{input_dropped, input_repeated,
  record_overflow}`, `EngineMessage::SetRecordStart(Option<Ticks>)`.
- `crates/cantus-engine/src/backend.rs` (small hunks, none in `start`): `compile` attaches
  audio; `set_snapshot` keeps the snapshot for the recorder; `poll` folds meters, the
  output latency and `Record` marks, and hands rings back to the recorder.
- `crates/cantus-engine/src/processor.rs`: `mod rec` (`processor_rec.rs`), one field, the
  block start (`read_input`, `record_block_start`), two drain arms, `live_install` in
  `install_snapshot`, the click gate at the beat, `record_segment` before the track loop,
  `add_track_audio` after the instrument, `record_wrapped` in `wrap_loop`. Nothing in the
  strips or the master section.
- `render.rs` / `stems.rs`: one `conform_audio` line each.

## Notes for the others and the integrator

- **Placement resolution.** A clip's `start` and `offset` are whole ticks: at 120 BPM and
  48 kHz a tick is 25 frames, so a take can only be placed within ±12 frames (0.26 ms) of
  where `start_tick` says, and two takes of one pass share the error (they stay
  phase-coherent with each other). Sample-exact placement needs a sub-tick field on
  `AudioRegion` (e.g. `offset_frames`) — a schema change for a later round.
- **ui-rec:** `FinishedTake::wraps` holds only the interior wrap points; pass `k` is
  `[wraps[k-1] or 0, wraps[k] or frames)`. `start_tick` may be negative (latency before a
  start at zero): window the clip from zero with an offset. Call `finish_recording` after
  `stop()`; it waits for a live engine itself.
- **devices:** `HandleRecorder::set_input_latency` exists (call it from `poll`); fill
  `input_name`/`input_channels` as planned. The processor primes on the first whole block
  in the ring, so the input ring should hold at least two blocks (≥ 8 asked).
- **ui-audio:** `SampleCache::set_project_dir` must be called when a project is opened or
  saved (relative paths), `audio_status(path)` reports a missing file; `retry_audio`
  after relinking. Loading a long file at another rate converts it on the UI thread the
  first time (about a second for a 3-minute 44.1 kHz file in a release build); a
  background loader is a later step.
- **master:** untouched; the click still goes in after the master section.
