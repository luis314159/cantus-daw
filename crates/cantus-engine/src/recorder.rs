//! Audio recording (ADR 0025): the input port the processor reads, the rings it writes
//! armed tracks into, the writer thread that turns them into WAV files, and the API the
//! application records through.
//!
//! The types and the `EngineHandle` methods below are what the UI codes against (the
//! round-19 contract's signatures, kept).
//!
//! How it fits together:
//!
//! - The backend (`crate::backend`, `crate::devices`) opens an input stream next to the
//!   output stream and pushes its interleaved samples into an `rtrb` ring; the processor
//!   gets the consumer end with [`Processor::attach_input`](crate::Processor::attach_input).
//!   Tests push into a ring of their own and drive the processor by hand.
//! - Each block, the processor pops one block of input, meters it (`Meters::inputs`),
//!   adds each monitored track's channels into that track's signal (before the channel's
//!   level, so its strip's inserts apply), and — while the transport plays with recording
//!   on — copies each armed track's input into that track's ring in [`RecordRings`].
//! - A writer thread (spawned by [`EngineHandle::begin_recording`]) drains the rings into
//!   one WAV per armed track per pass, flushing the header often enough that a crash
//!   loses at most a second.
//! - The processor reports where frame 0 of the pass landed on the timeline, and each loop
//!   wrap, as [`RecordMark`]s; the handle folds them into the [`FinishedTake`]s, already
//!   moved earlier by the round-trip latency so a take lines up with what was heard.
//!
//! **A pass, from the handle's side.** [`EngineHandle::begin_recording`] builds a ring
//! (`RING_SECONDS`) and a 32-bit float WAV for every armed track with an input in the last
//! snapshot sent, spawns the writer, and attaches the rings. The UI then plays with
//! recording on; the engine writes only while both are true, and marks the pass's start,
//! its loop wraps and its end. [`EngineHandle::finish_recording`] detaches the rings — on a
//! live stream it waits for the engine to hand them back, so every frame it pushed is in
//! the ring — stops the writer, which drains what is left and finalises each file, and
//! folds the marks into one [`FinishedTake`] per track. A track that recorded nothing
//! leaves no file behind.

use crate::backend::EngineHandle;
use crate::messages::{EngineMessage, Meters};
use cantus_core::{Snapshot, TrackId};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Seconds of audio each track's recording ring holds: how long the disk may stall before
/// frames are lost.
pub const RING_SECONDS: usize = 8;
/// The writer updates every file's header at least this often, so a crash loses at most
/// this much of what was recorded.
const FLUSH_EVERY: Duration = Duration::from_millis(500);
/// How long the writer sleeps when there was nothing to write.
const WRITER_IDLE: Duration = Duration::from_millis(4);
/// How long `finish_recording` waits for a live engine to hand the rings back.
const DETACH_TIMEOUT: Duration = Duration::from_secs(2);

/// Most input channels the processor meters and routes.
pub const MAX_INPUTS: usize = 32;

/// The processor's end of the input stream. Built empty; `attach_input` fills it.
#[derive(Debug, Default)]
#[allow(dead_code)] // Contract: the engine builder reads them.
pub struct InputPort {
    /// The input ring's consumer (interleaved, `channels` per frame).
    pub(crate) ring: Option<rtrb::Consumer<f32>>,
    pub(crate) channels: usize,
    /// When the input's buffers arrive (a live input stream); `None` for a ring a test
    /// fills by hand, whose block is taken as just arrived.
    pub(crate) clock: Option<Arc<crate::devices::InputClock>>,
}

/// One armed track's recording ring, as the processor holds it.
#[derive(Debug)]
pub struct TrackRing {
    pub track: TrackId,
    /// Interleaved, 1 or 2 channels as the track's input says.
    pub channels: usize,
    pub producer: rtrb::Producer<f32>,
    /// Where the processor lost frames (a full ring): `(file frame, frames)`, sent before
    /// the frames that follow the gap, so the writer pads the gap with silence and the
    /// file keeps the pass's length and its timing. `None`: gaps are only counted.
    pub gaps: Option<rtrb::Producer<(u64, u64)>>,
    /// Frames of the file so far, pushed and padded (the processor's count).
    pub(crate) at: u64,
    /// Frames lost since the last gap was sent.
    pub(crate) pending: u64,
}

/// Gaps a track's ring can announce before the writer catches up; a full gap queue makes
/// the processor drop what follows too, so the file never slips.
const GAP_QUEUE: usize = 256;

impl TrackRing {
    /// A ring without a gap queue (its losses are only counted).
    pub fn new(track: TrackId, channels: usize, producer: rtrb::Producer<f32>) -> TrackRing {
        TrackRing {
            track,
            channels,
            producer,
            gaps: None,
            at: 0,
            pending: 0,
        }
    }

    /// Send the frames lost since the last gap, if any. Returns whether the writer will
    /// know about every loss so far (and so whether frames may be pushed after it).
    pub(crate) fn flush_gap(&mut self) -> bool {
        if self.pending == 0 {
            return true;
        }
        let Some(q) = self.gaps.as_mut() else {
            // Nobody pads: the file just loses the frames (a test's ring).
            self.pending = 0;
            return true;
        };
        if q.push((self.at, self.pending)).is_ok() {
            self.at += self.pending;
            self.pending = 0;
            true
        } else {
            false
        }
    }
}

/// Every armed track's ring for one recording pass. Built on the UI thread with capacity
/// for several seconds and sent to the engine; handed back as garbage when it ends.
#[derive(Debug, Default)]
pub struct RecordRings {
    pub tracks: Vec<TrackRing>,
}

/// What the processor tells the handle about a pass, in engine time.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RecordMark {
    /// The pass's first recorded frame was rendered at this engine position (ticks, f64).
    Started { tick: f64 },
    /// The loop wrapped after this many recorded frames (every armed track shares it).
    Wrapped { frames: u64 },
    /// A ring was full: this many frames of this track were lost (the disk fell behind).
    Overflow { track: TrackId, frames: u64 },
    /// The pass ended after this many frames.
    Stopped { frames: u64 },
}

/// The latencies a take is moved by, in frames at the engine's rate (ADR 0025 §7).
///
/// A frame the processor records at engine position `X` was played by the musician
/// against what they heard `total()` frames earlier: the output device's latency (the
/// DAC side), the plug-in delay compensation in force (ADR 0022 — what the engine renders
/// at `X` is heard as `X − plugin`), the input device's latency (the ADC side), how long
/// the frame waited in the input ring before the processor popped it, and the user's
/// correction (a loopback measurement's, which is measured against the same sum).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordLatency {
    /// The input device's latency (capture → callback), as the driver reports it.
    pub input: u32,
    /// The output device's latency (callback → playback), as the driver reports it.
    pub output: u32,
    /// Plug-in delay compensation, the master chain's included (`EngineStats::latency_samples`).
    pub plugin: u32,
    /// How long popped input frames waited in the input ring (`EngineStats::input_queue`;
    /// signed, see `crate::devices::queue_wait`).
    pub queue: i64,
    /// The user's correction, from the settings or a loopback measurement.
    pub offset: i64,
}

impl RecordLatency {
    /// How much earlier than where it was rendered a recorded frame belongs.
    pub fn total(&self) -> i64 {
        self.input as i64 + self.output as i64 + self.plugin as i64 + self.queue + self.offset
    }

    /// What a loopback measurement is compared with: everything but the user's offset.
    pub fn measured(&self) -> i64 {
        self.total() - self.offset
    }
}

/// A slice of a recording in progress, for drawing it while it grows: the min and max of
/// each `PEAK_FRAMES` frames, mono-summed.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePeaks {
    pub track: TrackId,
    /// Frames recorded so far on this track's pass.
    pub frames: u64,
    /// New (min, max) pairs since the last poll, one per `PEAK_FRAMES` frames.
    pub peaks: Vec<(f32, f32)>,
}

/// Frames per live peak.
pub const PEAK_FRAMES: usize = 256;

/// One finished pass on one track: a WAV on disk and where it goes on the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishedTake {
    pub track: TrackId,
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u8,
    pub frames: u64,
    /// The timeline position of the file's first frame, in ticks, latency already
    /// compensated. Usually before the record start (the count-in is kept as a handle).
    pub start_tick: f64,
    /// Frame offsets in the file where the loop wrapped: pass `k` is
    /// `wraps[k-1]..wraps[k]` (`0` and `frames` at the ends).
    pub wraps: Vec<u64>,
    /// Frames lost to a full ring (0 when nothing was lost).
    pub dropped: u64,
}

/// The handle's side of recording, held in `EngineHandle::rec`.
#[derive(Debug, Default)]
pub struct HandleRecorder {
    pub(crate) latency: RecordLatency,
    pub(crate) input_peaks: Vec<f32>,
    pub(crate) input_name: Option<String>,
    pub(crate) input_channels: usize,
    /// The last snapshot sent: which tracks are armed, with which inputs, at what tempo.
    pub(crate) snapshot: Option<Arc<Snapshot>>,
    /// Input peaks since `input_peaks` was last called.
    peaks_since: Vec<f32>,
    /// The pass being recorded, from `begin_recording` to `finish_recording`.
    pass: Option<Pass>,
    record_start: Option<cantus_core::Ticks>,
}

impl HandleRecorder {
    /// The input latency the driver reports, in frames (the devices side measures it).
    pub(crate) fn set_input_latency(&mut self, frames: u32) {
        self.latency.input = frames;
    }

    /// The output device's latency (the devices side measures it).
    pub(crate) fn set_output_latency(&mut self, frames: u32) {
        self.latency.output = frames;
    }

    /// What the engine's stats say about the latency: plug-in delay compensation and the
    /// input ring's wait.
    pub(crate) fn engine_latency(&mut self, s: &crate::messages::EngineStats) {
        self.latency.plugin = s.latency_samples.min(u32::MAX as u64) as u32;
        self.latency.queue = s.input_queue.round() as i64;
    }

    /// One block's input meters, folded into the peaks since the last read.
    pub(crate) fn meter(&mut self, m: &Meters) {
        let n = m.input_count.min(MAX_INPUTS);
        if self.peaks_since.len() != n {
            self.peaks_since.resize(n, 0.0);
        }
        for (p, v) in self.peaks_since.iter_mut().zip(&m.inputs[..n]) {
            *p = p.max(*v);
        }
    }

    /// A mark from the engine, folded into the pass.
    pub(crate) fn mark(&mut self, mark: RecordMark) {
        let Some(pass) = self.pass.as_mut() else {
            return;
        };
        match mark {
            // The first start is the file's first frame; a later one (recording switched
            // off and on again in one pass) continues the same file.
            RecordMark::Started { tick } => {
                pass.started.get_or_insert(tick);
            }
            RecordMark::Wrapped { frames } => pass.wraps.push(frames),
            RecordMark::Overflow { track, frames } => {
                match pass.dropped.iter_mut().find(|(t, _)| *t == track) {
                    Some((_, n)) => *n += frames,
                    None => pass.dropped.push((track, frames)),
                }
            }
            RecordMark::Stopped { frames } => pass.stopped = Some(frames),
        }
    }

    /// The engine handed rings back: dropped here, off the audio thread.
    pub(crate) fn rings_back(&mut self, rings: Box<RecordRings>) {
        drop(rings);
        if let Some(pass) = self.pass.as_mut() {
            pass.rings_out = false;
        }
    }
}

impl EngineHandle {
    /// Channels on the open input device (0 without one).
    pub fn input_channel_count(&self) -> usize {
        self.rec.input_channels
    }

    /// The open input device's name, if one is open.
    pub fn input_device_name(&self) -> Option<&str> {
        self.rec.input_name.as_deref()
    }

    /// Each input channel's peak (linear) since the last call — the input meters.
    pub fn input_peaks(&mut self) -> &[f32] {
        let rec = &mut self.rec;
        std::mem::swap(&mut rec.input_peaks, &mut rec.peaks_since);
        rec.peaks_since.clear();
        rec.peaks_since.resize(rec.input_peaks.len(), 0.0);
        &rec.input_peaks
    }

    pub fn record_latency(&self) -> RecordLatency {
        self.rec.latency
    }

    /// The user's latency correction, in frames (from the settings or a loopback test).
    pub fn set_record_offset(&mut self, frames: i64) {
        self.rec.latency.offset = frames;
    }

    /// Prepare a pass: a ring and a WAV writer for every armed track with an input in the
    /// current snapshot, files in `dir`. Call before starting the transport with
    /// recording on. Returns how many tracks will record.
    pub fn begin_recording(&mut self, dir: &Path) -> Result<usize, String> {
        if self.rec.pass.is_some() {
            return Err("a recording pass is already running".into());
        }
        let Some(snapshot) = self.rec.snapshot.clone() else {
            return Ok(0);
        };
        let armed: Vec<_> = snapshot
            .tracks
            .iter()
            .filter(|t| t.armed)
            .filter_map(|t| t.input.as_ref().map(|i| (t.id, t.name.as_str(), i.stereo)))
            .collect();
        if armed.is_empty() {
            return Ok(0);
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let rate = self.sample_rate.round().max(1.0) as u32;
        let stamp = utc_stamp(std::time::SystemTime::now());
        let mut rings = RecordRings::default();
        let mut writers = Vec::with_capacity(armed.len());
        let mut tracks = Vec::with_capacity(armed.len());
        for (track, name, stereo) in armed {
            let channels = if stereo { 2 } else { 1 };
            let path = unique_path(dir, name, &stamp);
            let spec = hound::WavSpec {
                channels: channels as u16,
                sample_rate: rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let writer = match hound::WavWriter::create(&path, spec) {
                Ok(w) => w,
                Err(e) => {
                    // Nothing half-made is left behind.
                    drop(writers);
                    for t in &tracks {
                        let PassTrack { path, .. } = t;
                        let _ = std::fs::remove_file(path);
                    }
                    return Err(format!("{}: {e}", path.display()));
                }
            };
            let (producer, consumer) =
                rtrb::RingBuffer::<f32>::new(rate as usize * RING_SECONDS * channels);
            let (gaps, gap_rx) = rtrb::RingBuffer::<(u64, u64)>::new(GAP_QUEUE);
            let mut ring = TrackRing::new(track, channels, producer);
            ring.gaps = Some(gaps);
            rings.tracks.push(ring);
            writers.push(WriterTrack {
                track,
                path: path.clone(),
                channels,
                consumer,
                gaps: gap_rx,
                writer: Some(writer),
                samples: 0,
                pos: 0,
                error: None,
                peak: (f32::MAX, f32::MIN, 0),
                peaks: Vec::new(),
            });
            tracks.push(PassTrack {
                track,
                path,
                channels,
            });
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (peaks_tx, peaks_rx) = mpsc::channel();
        let flag = stop.clone();
        let hold = Arc::new(AtomicBool::new(false));
        let held = hold.clone();
        let writer = std::thread::Builder::new()
            .name("cantus-record-writer".into())
            .spawn(move || run_writer(writers, flag, held, peaks_tx))
            .map_err(|e| e.to_string())?;
        let count = tracks.len();
        let mut pass = Pass {
            tracks,
            rate,
            tempo_bpm: snapshot.tempo_bpm,
            tempo: snapshot.tempo.clone(),
            stop,
            hold,
            writer: Some(writer),
            peaks: peaks_rx,
            started: None,
            wraps: Vec::new(),
            dropped: Vec::new(),
            stopped: None,
            rings_out: true,
        };
        let failures = self.send_failures();
        self.send(EngineMessage::AttachRecorder(Box::new(rings)));
        if self.send_failures() > failures {
            pass.rings_out = false;
            let outcomes = pass.end();
            for o in outcomes {
                let _ = std::fs::remove_file(&o.path);
            }
            return Err("the engine's message queue is full".into());
        }
        self.rec.pass = Some(pass);
        Ok(count)
    }

    /// Where the take begins on the timeline when recording starts with a count-in (the
    /// transport is started that many bars earlier). Before it the click may sound alone
    /// (`ClickMode::CountInOnly`); the file still starts with the transport, so the
    /// count-in is a handle. `None` when not counting in.
    pub fn set_record_start(&mut self, at: Option<cantus_core::Ticks>) {
        self.rec.record_start = at;
        self.send(EngineMessage::SetRecordStart(at));
    }

    /// Frames lost so far this pass because the disk fell behind (a full recording
    /// ring): the most any armed track has lost, from the engine's overflow marks. The
    /// takes keep their length (the gaps are silence), but what was played there is gone;
    /// the UI warns while recording. 0 when no pass runs.
    pub fn record_dropped(&self) -> u64 {
        self.rec
            .pass
            .as_ref()
            .map_or(0, |p| p.dropped.iter().map(|d| d.1).max().unwrap_or(0))
    }

    /// Hold the writer (it stops draining the rings) or let it go: a disk that stalls,
    /// for the tests.
    #[doc(hidden)]
    pub fn hold_record_writer(&mut self, hold: bool) {
        if let Some(p) = self.rec.pass.as_ref() {
            p.hold.store(hold, Ordering::Release);
        }
    }

    /// Whether a pass is prepared or running.
    pub fn is_recording_audio(&self) -> bool {
        self.rec.pass.is_some()
    }

    /// What arrived since the last call, for drawing the takes as they grow. Called once
    /// a frame while recording.
    pub fn poll_recording(&mut self) -> Vec<LivePeaks> {
        let Some(pass) = self.rec.pass.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<LivePeaks> = Vec::new();
        while let Ok(p) = pass.peaks.try_recv() {
            match out.iter_mut().find(|o| o.track == p.track) {
                Some(o) => {
                    o.frames = p.frames;
                    o.peaks.extend_from_slice(&p.peaks);
                }
                None => out.push(p),
            }
        }
        out
    }

    /// End the pass (call after stopping the transport): the writers finish their files
    /// and the takes come back, one per armed track.
    pub fn finish_recording(&mut self) -> Vec<FinishedTake> {
        if self.rec.pass.is_none() {
            return Vec::new();
        }
        self.send(EngineMessage::DetachRecorder);
        // A live engine hands the rings back after the last block it wrote: once they are
        // back, everything it recorded is in them (and its marks have arrived, in order).
        if self.is_live() {
            let since = Instant::now();
            while self.rec.pass.as_ref().is_some_and(|p| p.rings_out)
                && since.elapsed() < DETACH_TIMEOUT
            {
                self.poll();
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let Some(mut pass) = self.rec.pass.take() else {
            return Vec::new();
        };
        let outcomes = pass.end();
        let latency = self.rec.latency.total();
        // Ticks per frame at the tempo the pass was recorded at (one tempo); through the
        // tempo map it was recorded under otherwise (ADR 0060), so a take played across a
        // ramp lands where it was played.
        let tpf = pass.tempo_bpm * cantus_core::PPQ as f64 / (60.0 * pass.rate as f64);
        let back = |started: f64| {
            if pass.tempo.is_constant() {
                started - latency as f64 * tpf
            } else {
                pass.tempo
                    .tick_after_seconds(started, -(latency as f64) / f64::from(pass.rate.max(1)))
            }
        };
        let mut takes = Vec::new();
        for (o, t) in outcomes.into_iter().zip(&pass.tracks) {
            let frames = o.samples / t.channels.max(1) as u64;
            let Some(started) = pass.started.filter(|_| frames > 0) else {
                // Nothing was recorded on this track: no file is left behind.
                let _ = std::fs::remove_file(&o.path);
                continue;
            };
            if let Some(e) = o.error {
                log::error!("recording {}: {e}", o.path.display());
            }
            takes.push(FinishedTake {
                track: t.track,
                path: o.path,
                sample_rate: pass.rate,
                channels: t.channels as u8,
                frames,
                start_tick: back(started),
                wraps: pass
                    .wraps
                    .iter()
                    .copied()
                    .filter(|w| *w > 0 && *w < frames)
                    .collect(),
                dropped: pass
                    .dropped
                    .iter()
                    .find(|(track, _)| *track == t.track)
                    .map_or(0, |d| d.1),
            });
        }
        takes
    }
}

/// The loopback measurement lives with the devices (`crate::loopback`).
pub use crate::loopback::measure_loopback;

/// One armed track of a pass, as the handle remembers it.
#[derive(Debug)]
struct PassTrack {
    track: TrackId,
    path: PathBuf,
    channels: usize,
}

/// A pass in progress, on the handle's side.
#[derive(Debug)]
struct Pass {
    /// In the order the writer holds them.
    tracks: Vec<PassTrack>,
    rate: u32,
    tempo_bpm: f64,
    /// The tempo map the pass was recorded under (ADR 0060).
    tempo: Arc<cantus_core::TempoMap>,
    stop: Arc<AtomicBool>,
    /// While set the writer does not drain (a test standing in for a disk that stalls).
    hold: Arc<AtomicBool>,
    writer: Option<JoinHandle<Vec<WriterOutcome>>>,
    peaks: mpsc::Receiver<LivePeaks>,
    /// From the engine's marks.
    started: Option<f64>,
    wraps: Vec<u64>,
    dropped: Vec<(TrackId, u64)>,
    stopped: Option<u64>,
    /// The engine holds the rings (they have not come back yet).
    rings_out: bool,
}

impl Pass {
    /// Stop the writer and wait for it: it drains what is left and finalises every file.
    fn end(&mut self) -> Vec<WriterOutcome> {
        self.stop.store(true, Ordering::Release);
        match self.writer.take().map(JoinHandle::join) {
            Some(Ok(outcomes)) => outcomes,
            Some(Err(_)) => {
                log::error!("the recording writer panicked");
                self.tracks
                    .iter()
                    .map(|t| WriterOutcome {
                        path: t.path.clone(),
                        samples: 0,
                        error: Some("the writer stopped".into()),
                    })
                    .collect()
            }
            None => Vec::new(),
        }
    }
}

/// What the writer made of one track.
#[derive(Debug)]
struct WriterOutcome {
    path: PathBuf,
    /// Samples written (frames × channels).
    samples: u64,
    error: Option<String>,
}

type Wav = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// Frames the writer moves from a ring at a time.
const BATCH_FRAMES: usize = 1024;

/// One track on the writer thread: the ring's consumer, the file, and the live peaks.
struct WriterTrack {
    track: TrackId,
    path: PathBuf,
    channels: usize,
    consumer: rtrb::Consumer<f32>,
    /// Where the processor lost frames: padded with silence here.
    gaps: rtrb::Consumer<(u64, u64)>,
    writer: Option<Wav>,
    /// Samples in the file (frames × channels), padding included.
    samples: u64,
    /// Frames of the pass so far (read or padded), whether or not the file took them.
    pos: u64,
    error: Option<String>,
    /// The peak being gathered: (min, max, frames so far).
    peak: (f32, f32, usize),
    peaks: Vec<(f32, f32)>,
}

impl WriterTrack {
    fn frames(&self) -> u64 {
        self.pos
    }

    /// One frame into the file and the live peaks.
    fn put(&mut self, frame: &[f32]) {
        self.pos += 1;
        if let Some(w) = self.writer.as_mut() {
            for s in frame {
                if let Err(e) = w.write_sample(*s) {
                    // A full disk: keep draining (so the engine never overflows on our
                    // account), keep what was written, say why.
                    self.error = Some(e.to_string());
                    self.writer = None;
                    break;
                }
            }
        }
        // A frame is only counted once it is in the file.
        if self.writer.is_some() {
            self.samples += frame.len() as u64;
        }
        let mono = frame.iter().sum::<f32>() / frame.len().max(1) as f32;
        let (lo, hi, k) = &mut self.peak;
        *lo = lo.min(mono);
        *hi = hi.max(mono);
        *k += 1;
        if *k == PEAK_FRAMES {
            self.peaks.push((*lo, *hi));
            self.peak = (f32::MAX, f32::MIN, 0);
        }
    }

    /// Silence for every gap that begins where the file has got to.
    fn pad_gaps(&mut self) -> usize {
        let zero = [0.0f32; 2];
        let ch = self.channels.clamp(1, 2);
        let mut padded = 0;
        while let Ok(&(at, len)) = self.gaps.peek() {
            if at > self.frames() {
                break;
            }
            let _ = self.gaps.pop();
            for _ in 0..len {
                self.put(&zero[..ch]);
            }
            padded += len as usize * ch;
        }
        padded
    }

    /// Move everything waiting in the ring into the file, padding the gaps the processor
    /// announced where they fall. Returns how many samples moved.
    fn drain(&mut self, live: &mpsc::Sender<LivePeaks>) -> usize {
        let ch = self.channels.max(1);
        let mut moved = 0;
        loop {
            // The data first, then the gaps: a gap is announced before the frames that
            // follow it, so every gap inside what is read here is already visible.
            let n = self.consumer.slots();
            moved += self.pad_gaps();
            // Whole frames only (the processor pushes whole frames, so this is all of it),
            // and only up to the next gap.
            let mut frames = n / ch;
            if let Ok(&(at, _)) = self.gaps.peek() {
                frames = frames.min(at.saturating_sub(self.frames()) as usize);
            }
            if frames == 0 {
                break;
            }
            // A batch at a time through a buffer on the stack (the chunk borrows the ring).
            let frames = frames.min(BATCH_FRAMES);
            let mut batch = [0.0f32; BATCH_FRAMES * 2];
            let Ok(chunk) = self.consumer.read_chunk(frames * ch) else {
                break;
            };
            let (a, b) = chunk.as_slices();
            for (o, x) in batch.iter_mut().zip(a.iter().chain(b)) {
                *o = *x;
            }
            chunk.commit_all();
            for f in batch[..frames * ch].chunks_exact(ch) {
                self.put(f);
            }
            moved += frames * ch;
        }
        if !self.peaks.is_empty() {
            let _ = live.send(LivePeaks {
                track: self.track,
                frames: self.frames(),
                peaks: std::mem::take(&mut self.peaks),
            });
        }
        moved
    }
}

/// The writer thread: drain every ring into its file until told to stop, then drain what
/// is left and finalise.
fn run_writer(
    mut tracks: Vec<WriterTrack>,
    stop: Arc<AtomicBool>,
    hold: Arc<AtomicBool>,
    live: mpsc::Sender<LivePeaks>,
) -> Vec<WriterOutcome> {
    let mut flushed = Instant::now();
    loop {
        // Read the flag first: whatever was pushed before it was set is drained below.
        let stopping = stop.load(Ordering::Acquire);
        if !stopping && hold.load(Ordering::Acquire) {
            std::thread::sleep(WRITER_IDLE);
            continue;
        }
        let moved: usize = tracks.iter_mut().map(|t| t.drain(&live)).sum();
        if flushed.elapsed() >= FLUSH_EVERY {
            for t in tracks.iter_mut() {
                if let Some(w) = t.writer.as_mut() {
                    if let Err(e) = w.flush() {
                        t.error = Some(e.to_string());
                    }
                }
            }
            flushed = Instant::now();
        }
        if stopping {
            break;
        }
        if moved == 0 {
            std::thread::sleep(WRITER_IDLE);
        }
    }
    tracks
        .into_iter()
        .map(|mut t| {
            if let Some(w) = t.writer.take() {
                if let Err(e) = w.finalize() {
                    t.error = Some(e.to_string());
                }
            }
            WriterOutcome {
                path: t.path,
                samples: t.samples,
                error: t.error,
            }
        })
        .collect()
}

/// `Name 2026-10-04 153210` (UTC): the track's name with what no file system takes
/// replaced, and the moment the pass was prepared.
fn utc_stamp(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// A file name for a track's take that no file in `dir` has yet.
fn unique_path(dir: &Path, name: &str, stamp: &str) -> PathBuf {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_end_matches('.').trim();
    let base = if clean.is_empty() {
        format!("Take {stamp}")
    } else {
        format!("{clean} {stamp}")
    };
    let mut path = dir.join(format!("{base}.wav"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} {n}.wav"));
        n += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamp_is_the_calendar_date() {
        let t = std::time::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
        // 2026-09-21 14:13:20 UTC.
        assert_eq!(utc_stamp(t), "2026-09-21 141320");
        assert_eq!(utc_stamp(std::time::UNIX_EPOCH), "1970-01-01 000000");
    }

    #[test]
    fn names_are_cleaned_and_never_reused() {
        let dir = tempfile::tempdir().unwrap();
        let a = unique_path(dir.path(), "Gtr: L/R?", "S");
        assert_eq!(a.file_name().unwrap(), "Gtr_ L_R_ S.wav");
        std::fs::write(&a, b"x").unwrap();
        let b = unique_path(dir.path(), "Gtr: L/R?", "S");
        assert_eq!(b.file_name().unwrap(), "Gtr_ L_R_ S 2.wav");
        assert_eq!(
            unique_path(dir.path(), " . ", "S").file_name().unwrap(),
            "Take S.wav"
        );
    }
}
