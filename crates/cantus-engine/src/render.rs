//! Offline rendering: drive the *same* [`Processor`] the audio callback uses, block by
//! block, into memory or a WAV file. Because the processor is deterministic and
//! block-size independent, the result is bit-identical to real-time playback.

use crate::effects::EffectUnit;
use crate::messages::{EngineEvent, EngineMessage};
use crate::processor::Processor;
use cantus_core::{Snapshot, Ticks};
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
    let snap = Arc::new(snap);

    let effect_count: usize = snap.tracks.iter().map(|t| t.effects.len()).sum();
    let (mut tx, rx_engine) = RingBuffer::<EngineMessage>::new(16 + effect_count);
    let (tx_engine, mut rx) = RingBuffer::<EngineEvent>::new(64 + effect_count);
    let mut p = Processor::without_midi(sr, rx_engine, tx_engine);

    for track in &snap.tracks {
        for slot in &track.effects {
            let unit = EffectUnit::new(slot, sr as f32);
            tx.push(EngineMessage::AddEffect(Box::new(unit))).ok();
        }
    }
    tx.push(EngineMessage::SetSnapshot(snap)).ok();
    tx.push(EngineMessage::Seek(opts.start)).ok();
    tx.push(EngineMessage::Play).ok();

    let content_seconds = (end - opts.start)
        .max(Ticks::ZERO)
        .to_seconds(snapshot.tempo_bpm);
    let total_frames = ((content_seconds + opts.tail_seconds) * sr).ceil() as usize;
    let mut out = vec![0f32; total_frames * channels];
    let mut buf = vec![0f32; block * channels];

    let mut done = 0;
    while done < total_frames {
        let n = block.min(total_frames - done);
        let chunk = &mut buf[..n * channels];
        p.process(chunk, channels);
        out[done * channels..(done + n) * channels].copy_from_slice(chunk);
        done += n;
        // Keep the event queue from filling (it is tiny on purpose).
        while rx.pop().is_ok() {}
    }

    let mut peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
    if let Some(db) = opts.normalize_peak_db {
        if peak > 0.0 {
            let target = 10f32.powf(db / 20.0);
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
    })
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
