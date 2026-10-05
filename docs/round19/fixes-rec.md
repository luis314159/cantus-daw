# Round 19 — fixes-rec: the recording path, made correct

Builder: **fixes-rec** (`r19/fixes-rec`, from the integration commit 5190b92). ADR 0025
(§7 rewritten: the latency formula, sample-exact placement, gaps).

## What is fixed

1. **A take's latency is the whole trip.** `RecordLatency` is now
   `output` (device, DAC side) + `plugin` (ADR 0022's delay compensation, master chain
   included) + `input` (device, ADC side) + `queue` (the input ring's wait) + `offset`
   (`total()`; `measured()` is all but the offset). The ring's wait is *measured*: the
   input callback stamps each buffer (`devices::InputClock`: when, how many frames); at
   each pop the processor computes `avail − b_in + age` (`devices::queue_wait`, signed),
   smoothed into `EngineStats::input_queue`. Without a clock (hand-driven processor) the
   block stands for the last buffer, just arrived: the depth after popping. The device's
   output latency now reaches the take (`sync_devices` sets it); the plug-in latency comes
   from the stats as before. **Loopback**: the probe measures its own ring wait the same
   way (mean over the run, `LoopbackResult::queue`) and stores
   `offset = round_trip − reported − queue`, so the offset is what the drivers do not
   say, not the wait of that particular stream start. Simulated cable test: with the
   input reporting its buffer + jitter, the offset found is the cable's delay exactly.
2. **Sample-accurate placement.** `AudioRegion::slip_frames: i32` (`#[serde(default)]`,
   not written when 0 — files and the v18 fixture unchanged; no migration) moves the
   content by source frames under a whole-tick window. The snapshot carries it as
   seconds (`AudioClipSnapshot::slip`). The engine (`processor_rec::add_clips`) now
   places a clip from **one** rounded origin — the transport sample of the window's
   content origin `start − offset`, less the slip — instead of rounding start and offset
   separately: a split now plays on without a one-frame seam at awkward tempos (tested at
   97.3 and 133 BPM). `record_pass` sets the slip from the window's rounding remainder
   (`Window::slip_frames`). The UI honours it in the waveform, `window_frames`
   (normalise, the inspector's overview), and glue (refuses windows with another slip);
   split/trim keep it (it moves the content, not the window).
3. **Take math end to end.** `cantus-ui/tests/rec_e2e.rs` runs a real pass — manual
   processor, a loop of 48 000 frames, 3.5 passes, latencies 1 337 / 250 / 7 / 4 801
   frames, blocks of 512 / 480 / 333 / 1 024 — through `finish_recording` and
   `record_pass::build`, then renders each pass's clip alone: every frame of every pass
   (the last, partial one included) plays exactly the input frame played against it. The
   math between `FinishedTake` and `record_pass` was right; it is now held.
4. **Lost frames.** `EngineHandle::record_dropped()` (the most any track lost so far,
   from the overflow marks). The UI checks it every frame while recording: the status
   line says so at once (`rec.dropped_live`) and the clock's elapsed slot shows the
   warning glyph beside the time. **The writer pads a gap with silence**: each track
   ring has a gap queue (`TrackRing::gaps`); the processor announces a loss before the
   frames that follow it (and at stop/detach), so a take keeps the pass's length and
   everything after the gap is where it was played. The final report uses the max over
   tracks, not the sum.
5. **Count-in before bar 1.** `Transport::seek` accepts negative ticks (pre-roll; the
   app still clamps the user's seeks). Recording at bar 1 with a count-in starts the
   transport that many bars before zero; the whole count-in is heard (bit-identical to
   the click model) and the take still starts at the record position. Notes played in a
   pre-roll before zero are dropped from MIDI takes.
6. **Every armed instrument track sounds.** `EngineMessage::SetLiveEcho([Option<TrackId>;
   MAX_LIVE_ECHO])` / `EngineHandle::set_live_echo` (8 tracks; a fixed array, so the
   message stays small and the audio thread never frees one): the live notes (and sustain, pitch
   bend, all-notes-off) play on the echo tracks too; only the live track's notes are
   reported (the UI already copies them into every armed track). A track leaving the set
   lets go of its live notes. The UI sets it for the pass's armed instrument tracks and
   clears it at stop (`RecSession::echo`).
7. **The project folder.** `CantusApp::sync_audio_base()` tells
   `SampleCache::set_project_dir` the project's folder and forces a recompile when it
   changed; called on open, save (and so save as), new and demo load
   (`replace_project`), and every frame from `sync_engine` as a safety net. **Autosave**
   writes every relative audio path absolute (`autosave_copy`, `absolute_audio_paths`),
   so an autosave opened from anywhere (the fallback folder, a recovery) finds its audio;
   the project itself keeps its relative paths.
8. **Background conversion.** `SampleCache::set_background(true)` (a live engine's
   `compile` turns it on; manual handles, tests and offline renders stay synchronous):
   a source not loaded is queued to a `cantus-audio-loader` thread and the clip plays
   silence; `poll_loaded()` takes in what arrived and the app recompiles the snapshot
   (`sync_engine`). `SampleStatus::Loading`; the arrangement veils a loading clip and
   writes *Loading* on it.
9. **The piano roll refuses audio clips**: it shows *An audio clip has no notes. Its
   gain, fades and file are in the inspector* and edits nothing. The channel rack no
   longer picks an audio clip or a take as its pattern's clip.
10. **An audio track's inspector** (`record_session::is_audio_track`: an input, no note
    clips): no instrument row and no instrument section (no Firmus); the audio clip
    section of its selected (or first) clip takes that place; the input and recording
    rows are in the channel below, as before.

## Tests

- `cantus-engine/tests/recording_latency.rs` (4): the latency's parts sum;
  the ring's wait is measured (a block waiting → `input_queue` = the block, and 0 without
  it); a take with the ring's wait lines up to the frame; a stalled disk (writer held)
  loses frames, `record_dropped` says so during the pass, and the file keeps its length
  with silence in the gap and everything after it in place.
- `cantus-engine/tests/audio_slip.rs` (2): the fine offset moves content by frames
  (±7, 12, −13); a split is seamless at 120 / 97.3 / 133 BPM with any slip.
- `cantus-engine/tests/recording_preroll.rs` (3): a count-in before bar 1 plays in full
  and stops at bar 1; an echo track plays the live note (voices double, one report); a
  track leaving the echo lets go.
- `cantus-engine/tests/audio_loader.rs` (2): background load arrives whole and equal
  to a load on the spot, not queued twice; a missing file fails in the background too.
- `cantus-engine/tests/devices.rs`: the probe test now checks the ring wait and that the
  offset is the cable's delay exactly.
- `cantus-core`: `the_fine_offset_defaults_to_zero_and_round_trips`; the snapshot test
  checks the slip in seconds; the undo fuzz inserts audio clips with a slip.
- `cantus-ui/tests/rec_e2e.rs` (2): the end-to-end pass above; a real-number take
  (−53.48 ticks) gives offset 53 and slip +12, one undo step.
- `cantus-ui/tests/rec_fixes.rs` (6): an app on a hand-driven engine warns of lost frames
  while the pass runs and keeps the take's length; count-in pre-roll and the echo set;
  the cache follows open / save / save as / new, and autosaves hold absolute paths; the
  piano roll refuses an audio clip (a click draws nothing); an audio track's inspector
  has no instrument and shows its audio and input; split/glue keep the slip.

## Files touched (outside the recording files)

`cantus-core`: `audio.rs` (field + test), `snapshot.rs` (`slip`), `tests/undo_invariants.rs`.
`cantus-engine`: `messages.rs` (`input_queue`, `SetLiveEcho`, `MAX_LIVE_ECHO`),
`processor.rs` (echo, attach clock, flush gaps at detach), `processor_rec.rs`,
`transport.rs` (seek before zero), `devices.rs` (`InputClock`, `queue_wait`), `loopback.rs`,
`recorder.rs`, `backend.rs` (`set_live_echo`, background loading, stats → latency),
`samples.rs` (loader), `lib.rs`. `cantus-ui`: `app.rs` (`sync_audio_base`, autosave copy,
three one-line calls), `audio_files.rs`, `audio_clips.rs`, `record_pass.rs`,
`record_session.rs`, `views/{arrangement_audio, arrangement_audio_paint, inspector,
inspector_track, inspector_pads, piano_roll, channel_rack, record_options,
settings_audio}.rs`, locales (`recording`, `audioclips`), `Cargo.toml` (`rtrb` dev-dep),
`examples/shoot_rec.rs` (`rec-audio-track-*`).

## Looked at

`cargo run -p cantus-ui --example shoot_rec -- <dir> slate` writes the new
`rec-audio-track-slate.png`: the lead vocal (an audio track) in the inspector shows Track
→ Audio clip → Mixer input → Recording, no instrument; the clock shows the warning glyph
beside the pass's time. `rec-arrangement-slate.png` is unchanged.

Runs: `cargo test -p cantus-engine -p cantus-core -p cantus-project -p cantus-ui` all
green; `cargo clippy -p cantus-engine -p cantus-core -p cantus-project -p cantus-ui
--tests --examples` clean; `cargo fmt` applied.

## Signatures changed (contract)

- `RecordLatency` gained `plugin` and `queue`; `output` now means the device's output.
- `TrackRing` gained `gaps` (+ private fields): build it with `TrackRing::new`.
- `LoopbackResult` gained `queue`; `offset` subtracts it.
- `AudioRegion::slip_frames`, `AudioClipSnapshot::slip`, `SampleStatus::Loading` (a match
  on `SampleStatus` needs the arm).
- `CantusApp::poll_recording` is `pub` (the tests drive it); `start_recording` checks
  `input_channels()` (so the demo/test input device counts).

## Left

- The ring's wait assumes one `process` call per callback; an integer-format device with
  a buffer longer than the 4 096-frame scratch renders in several calls and the later
  ones read a little short (rare: such buffers are > 85 ms).
- `clamp_edit` ignores the slip (a window can show up to half a tick past a file's end,
  as silence).
- Rendering a clip's peaks uses the peak store's rate, assumed to be the source's.
- A pass's latency is read when it finishes; a re-prime of the input ring mid-pass
  (`input_repeated`) still shifts what follows it.
- The fixture `v18.cantus` was not regenerated (the field is skipped when 0).
