//! Offline rendering: drive the *same* [`Processor`] the audio callback uses, block by
//! block, into memory or a WAV file. Because the processor is deterministic and
//! block-size independent, the result is bit-identical to real-time playback.

use crate::effects::EffectUnit;
use crate::messages::{EngineEvent, EngineMessage};
use crate::processor::Processor;
use cantus_core::{Snapshot, Ticks};
use cantus_dsp::math;
use rtrb::RingBuffer;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("nothing to render: the project has no notes")]
    Empty,
    #[error("wav error: {0}")]
    Wav(#[from] hound::Error),
    #[error("the render was cancelled")]
    Cancelled,
    /// A streamed sample could not be read (second review #15): the bounce would be cut
    /// short where it could not, so there is none.
    #[error("a sample could not be read: {0}")]
    Unreadable(String),
}

/// A render's progress, for a dialog that shows it and may stop it
/// ([`render_with_progress`]): the renderer stores the frames done and the total, and reads
/// `cancel` between blocks. Shared with the UI through an `Arc`; every field is an atomic.
#[derive(Debug, Default)]
pub struct RenderProgress {
    pub done: std::sync::atomic::AtomicU64,
    pub total: std::sync::atomic::AtomicU64,
    pub cancel: std::sync::atomic::AtomicBool,
}

impl RenderProgress {
    /// Done over total, 0…1 (0 before the total is known).
    pub fn fraction(&self) -> f32 {
        use std::sync::atomic::Ordering::Relaxed;
        let t = self.total.load(Relaxed);
        if t == 0 {
            0.0
        } else {
            (self.done.load(Relaxed) as f64 / t as f64).min(1.0) as f32
        }
    }

    /// Ask the render to stop at its next block ([`RenderError::Cancelled`]).
    pub fn cancel(&self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderOptions {
    pub sample_rate: u32,
    pub channels: u16,
    /// Render from here…
    pub start: Ticks,
    /// …to here. `None` = end of content.
    pub end: Option<Ticks>,
    /// Extra time after `end` so releases/reverbs ring out.
    pub tail_seconds: f64,
    /// Internal block size. Any value gives the same output; exposed for tests.
    pub block_frames: usize,
    /// Peak-normalise the result to this level in dBFS (`None` = leave as is).
    pub normalize_peak_db: Option<f32>,
    /// Start the result at the range's first tick: delay compensation makes the master
    /// late by the project's total latency (a look-ahead's), so that much more is rendered
    /// and the first frames dropped, as the stems trim the master. Off, the result is what
    /// playing gives, sample for sample, latency included — what the engine's tests
    /// compare; an export turns it on (round 23).
    pub compensate_latency: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            sample_rate: 48_000,
            channels: 2,
            start: Ticks::ZERO,
            end: None,
            tail_seconds: 2.0,
            block_frames: 512,
            normalize_peak_db: None,
            compensate_latency: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved samples.
    pub samples: Vec<f32>,
    pub peak: f32,
    /// Notes that played only the start of a streamed sample (read faster than streaming
    /// serves — third review #9): the export says so.
    pub cut_notes: u64,
}

impl Rendered {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration_seconds(&self) -> f64 {
        self.frames() as f64 / self.sample_rate as f64
    }
}

/// Render a snapshot to memory.
pub fn render(snapshot: Arc<Snapshot>, opts: &RenderOptions) -> Result<Rendered, RenderError> {
    render_with_units(snapshot, opts, Vec::new())
}

/// [`render`], with units built elsewhere for some of the effects — a hosted plug-in's
/// instance for this render, loaded and activated on the main thread (ADR 0019). Every
/// effect without one gets the unit the engine builds (a wire for a hosted plug-in).
pub fn render_with_units(
    snapshot: Arc<Snapshot>,
    opts: &RenderOptions,
    units: Vec<EffectUnit>,
) -> Result<Rendered, RenderError> {
    render_with_instruments(snapshot, opts, units, Vec::new())
}

/// [`render_with_units`], with instrument units built elsewhere too — a hosted
/// instrument's instance for this render (ADR 0028), labelled with its track and kind.
/// Every native instrument gets the unit the engine builds, as in [`render`].
pub fn render_with_instruments(
    snapshot: Arc<Snapshot>,
    opts: &RenderOptions,
    units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
) -> Result<Rendered, RenderError> {
    render_with_progress(snapshot, opts, units, hosted, &RenderProgress::default())
}

/// [`render_with_instruments`], reporting its progress into `progress` (frames done of the
/// total, after every block) and stopping with [`RenderError::Cancelled`] at the first
/// block after `progress.cancel` is set — the export dialog's bar and its Stop.
pub fn render_with_progress(
    snapshot: Arc<Snapshot>,
    opts: &RenderOptions,
    mut units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    progress: &RenderProgress,
) -> Result<Rendered, RenderError> {
    use std::sync::atomic::Ordering::Relaxed;
    let end = opts.end.unwrap_or(snapshot.content_end);
    if end <= opts.start && opts.end.is_none() {
        return Err(RenderError::Empty);
    }
    let channels = opts.channels.max(1) as usize;
    let block = opts
        .block_frames
        .clamp(1, crate::messages::MAX_BLOCK_FRAMES);
    let sr = opts.sample_rate as f64;

    // Loop is a playback convenience, not part of the rendered piece.
    let mut snap = (*snapshot).clone();
    snap.loop_region = None;
    snap.metronome = false;
    // Audio clips at the render's rate (ADR 0025 §9).
    crate::samples::conform_audio(&mut snap, opts.sample_rate);
    let snap = Arc::new(snap);

    let effect_count: usize = snap.effect_slots().count();
    let mut instruments = crate::inst::units_for(&snap, sr as f32, |_, _| false);
    // Offline, a voice streaming from disk waits for its blocks (ADR 0050 §6).
    for u in &mut instruments {
        u.unit.set_offline(true);
    }
    instruments.extend(hosted.into_iter().map(Box::new));
    let (mut tx, rx_engine) =
        RingBuffer::<EngineMessage>::new(16 + effect_count + instruments.len());
    let (tx_engine, mut rx) = RingBuffer::<EngineEvent>::new(64 + effect_count + instruments.len());
    let mut p = Processor::without_midi(sr, rx_engine, tx_engine);
    // Nobody reads an offline render's blame; the clock stamps are export time saved.
    p.set_profiling(false);
    // A frozen render's reader waits for its blocks (ADR 0064).
    p.set_offline(true);

    for slot in snap.effect_slots() {
        let unit = match units.iter().position(|u| u.id == slot.id) {
            Some(i) => units.swap_remove(i),
            None => EffectUnit::new(slot, sr as f32),
        };
        tx.push(EngineMessage::AddEffect(Box::new(unit))).ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(
        crate::automation::Runtime::build(&snap),
    ))
    .ok();
    // The graph's plan, compiled here and sent with it, as the application does.
    let plan = crate::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap, plan)).ok();
    // Delay compensation longer than the pool's start-up lines is asked for; answer it
    // before the first block so the render is aligned from its first sample.
    // The same for memory an effect's settings need (ADR 0022).
    p.settle();
    while let Ok(ev) = rx.pop() {
        match ev {
            EngineEvent::PdcNeed { count, delay } => {
                tx.push(EngineMessage::PdcLines(crate::pdc::lines_for(count, delay)))
                    .ok();
                p.settle();
            }
            EngineEvent::EffectNeed { effect, need } => {
                let memory = crate::effects::EffectMemory::build(need, sr as f32);
                tx.push(EngineMessage::EffectMemory { effect, memory }).ok();
                p.settle();
            }
            _ => {}
        }
    }
    tx.push(EngineMessage::Seek(opts.start)).ok();
    tx.push(EngineMessage::Play).ok();

    // Through the tempo map (ADR 0060); one tempo, `(end − start).to_seconds(bpm)`.
    let content_seconds = snapshot
        .tempo
        .seconds_between(opts.start, end.max(opts.start));
    let total_frames = ((content_seconds + opts.tail_seconds) * sr).ceil() as usize;
    // With `compensate_latency`, sample 0 is the range's first tick: the master's lateness
    // is rendered on and dropped from the front.
    let latency = if opts.compensate_latency {
        p.stats().latency_samples as usize
    } else {
        0
    };
    let frames = total_frames + latency;
    // A range that ends before the song does stops there: its tail rings out what was
    // playing (releases, delays, reverbs) and plays nothing that starts after it. The
    // song itself plays to its end, every note to its own.
    // The stop falls on the sample the transport reaches `end` at (its own arithmetic: a
    // note starting exactly there must not fire before the stop).
    let mut stop_at = opts.end.map(|e| {
        crate::transport::sample_at_map(&snapshot.tempo, opts.start.0 as f64, e.0 as f64, sr)
            as usize
    });
    let mut out = vec![0f32; total_frames * channels];
    let mut buf = vec![0f32; block * channels];

    progress.total.store(frames as u64, Relaxed);
    progress.done.store(0, Relaxed);
    let mut done = 0;
    while done < frames {
        if progress.cancel.load(Relaxed) {
            return Err(RenderError::Cancelled);
        }
        if stop_at == Some(done) {
            tx.push(EngineMessage::Stop).ok();
            stop_at = None;
        }
        let mut n = block.min(frames - done);
        if let Some(s) = stop_at.filter(|s| *s < done + n) {
            // The block ends where the range does (the processor is block-size
            // independent, so the split moves no sample).
            n = s - done;
        }
        let chunk = &mut buf[..n * channels];
        p.process(chunk, channels);
        let skip = latency.saturating_sub(done).min(n);
        if skip < n {
            let at = done + skip - latency;
            out[at * channels..(at + n - skip) * channels]
                .copy_from_slice(&chunk[skip * channels..]);
        }
        done += n;
        progress.done.store(done as u64, Relaxed);
        // Keep the event queue from filling (it is tiny on purpose).
        while rx.pop().is_ok() {}
    }

    // A streamed voice that could not be read offline (its file and its source unreadable,
    // or a disk too slow for the pool's patience) ended early: no silent difference.
    if p.stats().stream_late > 0 {
        return Err(RenderError::Unreadable(unreadable_in(&snapshot)));
    }
    let mut peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    if let Some(db) = opts.normalize_peak_db {
        if peak > 0.0 {
            let target = math::powf(10f32, db / 20.0);
            let g = target / peak;
            out.iter_mut().for_each(|x| *x *= g);
            peak = target;
        }
    }
    Ok(Rendered {
        sample_rate: opts.sample_rate,
        channels: opts.channels,
        samples: out,
        peak,
        cut_notes: p.stats().stream_cut,
    })
}

/// The files of `snapshot`'s failed streams, by name (or why none is named).
pub(crate) fn unreadable_in(snapshot: &Snapshot) -> String {
    let mut names: Vec<String> = snapshot
        .tracks
        .iter()
        .flat_map(|t| t.assets.iter())
        .filter_map(|(_, a)| crate::inst::stream::failed_source(a.as_deref()?))
        .map(|p| p.display().to_string())
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        "a streamed sample was not read in time".into()
    } else {
        names.join(", ")
    }
}

/// Render straight to a 32-bit float WAV file.
pub fn render_to_wav(
    snapshot: Arc<Snapshot>,
    opts: &RenderOptions,
    path: &Path,
) -> Result<Rendered, RenderError> {
    let r = render(snapshot, opts)?;
    write_wav(&r, path)?;
    Ok(r)
}

pub fn write_wav(r: &Rendered, path: &Path) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: r.channels,
        sample_rate: r.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for s in &r.samples {
        w.write_sample(*s)?;
    }
    w.finalize()
}
