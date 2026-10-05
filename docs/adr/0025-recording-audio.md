# ADR 0025 — Recording audio, and audio clips

**Status:** accepted (round 19, contract) · **Date:** 2026-10-04

## Context

The user wants to *record from an audio interface professionally, with every feature the
DAWs that are best at recording have* (FL Studio, Pro Tools, Cubase, Logic, Studio One,
Reaper), and then polish everything so songs can be made in Cantus. Until now Cantus had
no audio input, no audio clips and no arm button: the mixer's *external audio in* and the
playlist's audio clips were both listed as blocked on this (STATUS, rounds 12–13).

## Decision

**1. An audio clip is a `Clip` with `audio: Some(AudioRegion)`.** Its window (`start`,
`length`, `offset`) works exactly as for notes, so split, trim, slip, move, mute, colour
and glue need no second implementation. The content timeline is the source file: content
tick `t` is the file at `seconds(t)` at the project tempo (content tick 0 = frame 0). A
tempo change keeps clip *positions* in bars and plays audio at its natural speed; warping
is a later decision.

**2. The project has an audio pool** (`Project::audio_pool`): every file once, by id, with
its format and an optional SHA-256. Clips point at sources, so takes, comps and copies
share one file and one cache entry. Paths are relative to the project's folder once saved
(`<project folder>/Audio/`), absolute before (imports, and recordings in the session's
scratch folder until the first save). Saving moves recordings and copies imports into the
audio folder.

**3. Any track can hold audio clips** (Bitwig's hybrid track). The clips' audio and the
instrument's output are summed before the channel's level, so the strip's inserts, sends
and the rest apply. An *audio track* is a track created with an input, no instrument
played and no note clips; the UI names it so.

**4. Inputs and monitoring.** `Track::input` picks the device channel(s) (mono or a
stereo pair), a trim, and a monitor mode — Off, **Auto** (default: hear the input while
armed, except while playing back without recording) and On. The monitored input enters
the track *before* the channel level, so a guitar is heard through its strip's amp,
compressor and reverb while it is recorded **dry** (what is recorded is the input after
trim, never the effects).

**5. Take lanes and comping.** `Clip::lane`: 0 is the playing lane, 1… are take lanes,
kept and drawn under the track (`Track::takes_open`) and never heard. Loop recording makes
**one file per pass per track** and one clip per loop pass on a take lane, the last pass on
lane 0. A comp is lane-0 clips pointing into the takes' sources: swiping across a take
lane replaces that range of lane 0 with a window on that take. Nothing is ever destroyed;
`TakeMode::Replace` is the explicit tape mode.

**6. Punch and count-in keep handles.** The whole pass is written to disk from the moment
the transport starts (count-in and pre-roll included); punch in/out and the record start
only set the clip's *window*. A take can always be extended into what was played before
or after it, as in Pro Tools.

**7. Latency.** A frame the processor records at engine position `X` was played against
what the musician heard `L` frames earlier, and the take is moved back by exactly that
(`RecordLatency::total`, round 19's fixes):

    L = output   (the output device: callback → playback, from the driver's timestamps)
      + plugin   (delay compensation in force, master chain included: ADR 0022's
                  `latency_samples` — what is rendered at X is heard as X − plugin)
      + input    (the input device: capture → callback of a buffer's first frame)
      + queue    (how long the popped frames waited in the input ring)
      + offset   (the user's correction, or a loopback measurement's)

The ring's wait is measured, not assumed, because it changes from one stream start to
the next (the phase between the input's and the output's callbacks). The input callback
stamps each buffer it pushes (`InputClock`: when, and how many frames); when the
processor pops a block it knows `avail` (frames waiting), the last buffer's length `b_in`
and its age, so the frame placed at the block's first sample waited
`queue = avail − b_in + age` frames after its buffer arrived (`devices::queue_wait`;
signed, since a buffer's first frame was captured `b_in` frames before its last). It is
smoothed (`EngineStats::input_queue`). Without a clock (a processor driven by hand) the
block stands for the last buffer, arrived just now: the depth after popping.

The **loopback** measurement plays its probe inside the engine's own streams and finds
the round trip as the processor would see it; it measures the probe's own ring wait the
same way (averaged over the run) and stores `offset = round_trip − (input + output) −
queue` — what the drivers do not say, free of the wait of that particular stream start.
Plug-in latency is not in the probe's path and not in its offset.

**Placement to the sample.** A clip's window is in whole ticks (25 frames at 120 BPM and
48 kHz). `AudioRegion::slip_frames` moves the content under the window by source frames:
the source frame under content tick `t` is `seconds(t) × rate + slip_frames`. A finished
take's window and offset are rounded to ticks and the remainder goes into `slip_frames`,
so frame 0 lands where `FinishedTake::start_tick` says (pass `k ≥ 1` of a loop: its first
frame `wraps[k−1]` belongs at the loop's start less `L`). The engine places a clip from
**one** rounded origin — the transport sample of the window's content origin
(`start − offset`) less the slip — so the halves of a split play on without a seam at any
tempo. A full recording ring loses frames but the writer pads the gap with silence
(`TrackRing::gaps`), so a take keeps the pass's length and everything after the gap stays
where it was played; `EngineHandle::record_dropped` says how much while the pass runs.

**8. Real-time rules hold** (ADR 0005). The input stream pushes into an `rtrb` ring; the
processor pops one block per callback, meters it, monitors it and copies armed tracks
into per-track rings that a **writer thread** drains into WAV files (32-bit float),
updating the header at least once a second so a crash loses at most that. Rings and
writers are built on the UI thread and sent in. A full ring drops frames and says so
(`RecordMark::Overflow`) — it never blocks the audio thread.

**9. Playback is from memory**, through the same `SampleCache` the kit uses (decoded once,
resampled to the device rate on load, never evicted while the session lives), attached to
the snapshot as `AudioClipSnapshot::data`. Offline render and stems use the same path, so a
bounce is bit-identical to what is heard. Disk streaming is a later step once projects
outgrow memory.

**10. Drivers.** WASAPI shared mode is the default on Windows; **ASIO** is offered when
Cantus is built with it (cpal's `asio` feature; the ASIO SDK is GPL-3-compatible since
2025 and Cantus is GPL per ADR 0001). The input device and the output device may differ;
with two clocks the input ring absorbs drift by dropping or repeating a frame, and counts
it.

## Consequences

- Schema 18; every new field defaults to the old behaviour (`migrate_17_to_18` writes
  nothing). Fixture `v18.cantus` has an armed audio track, a take on lane 1 and a clip
  with gain and fades whose file is absent (a missing source opens and plays silence).
- What the tests hold (the builders add them): a monitored input at unity with no clips
  reaches the strip bit-exact; an audio clip at 0 dB with no fades plays its file
  bit-exact; playback is block-size independent; offline render equals real time;
  record → finish → lay out → play back gives every loop pass exactly the frames played
  against it, to the sample, with a latency that is not a whole tick
  (`cantus-ui/tests/rec_e2e.rs`); a full ring drops and reports without blocking, and the
  take keeps its length; fades are exactly 0 and 1 at their ends.
- Later: warping/time-stretch, disk streaming, clip-based pitch, AAF/OMF and audio in
  DAWproject (ADR 0021), a spectral view.
