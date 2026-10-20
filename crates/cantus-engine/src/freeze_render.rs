//! Rendering what a freeze or a bounce in place holds (round 29, ADR 0064): one offline
//! pass of the same processor real time runs, with a tap where the frozen sound ends — a
//! strip's pair **before its width** (after its inserts and EQ) or a track's **before its
//! level and balance** — and everything that does not reach it resting (the snapshot's
//! `rest` flags, `cantus_core::freeze::isolate`), so only what makes the sound is computed.
//!
//! **Alignment.** A strip's pair before the width lags the timeline by the compensation it
//! waits for and its own inserts' latency (`Processor::strip_latency`), a track's source by
//! its instrument's latency. A **freeze keeps it**: frame *f* of the render is what reached
//! the tap at the transport's frame *f*, and the frozen strip reports that latency as its
//! own (`Processor::compensate`), so every frame meets its fader, its pan and their
//! automation at the very sample it did live — the mix is the live mix to the bit, in real
//! time as offline. A **bounce trims it** (`TapOptions::trim`): frame 0 is the range's
//! first tick, what an audio clip with no latency of its own must hold.
//!
//! **Length.** The range, then its tail: the pass runs on until what it taps has been
//! exactly zero for `quiet_seconds` (a reverb rung out; denormals are flushed, so a tail
//! ends in zeros) or for `max_tail_seconds` at most; the frames kept end at the last one
//! that was not zero.
//!
//! The frames go to a raw file (interleaved `f32`, stereo) as they come — a render is never
//! whole in memory — and [`write_render`] makes the frozen file of them, [`write_wav`] a
//! bounce's WAV.

use crate::effects::EffectUnit;
use crate::messages::{EngineEvent, EngineMessage};
use crate::processor::Processor;
use crate::render::{RenderError, RenderProgress};
use crate::stems::StemTap;
use cantus_core::{Snapshot, Ticks};
use rtrb::RingBuffer;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;

/// Where the render is tapped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TapPoint {
    /// Snapshot strip `i`'s pair before its width: everything it gathers, its polarity,
    /// swap, inserts and EQ.
    Strip(usize),
    /// Snapshot track `i`'s source before its level and balance: its instrument and clips.
    Track(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TapOptions {
    pub sample_rate: u32,
    pub block_frames: usize,
    /// The range: frame 0 is `start`; the tail follows `end`.
    pub start: Ticks,
    pub end: Ticks,
    /// Stop the transport at `end` (a range cut out of a song: nothing that starts after it
    /// plays, what was playing rings out). Off: the song plays on.
    pub stop_at_end: bool,
    /// The tail ends after this much exact silence, or at `max_tail_seconds`.
    pub quiet_seconds: f64,
    pub max_tail_seconds: f64,
    /// Drop the tap's latency from the front (a bounce); kept for a freeze.
    pub trim: bool,
}

impl TapOptions {
    /// A freeze's: the whole song from tick 0, its tail up to 30 s.
    pub fn freeze(sample_rate: u32, end: Ticks) -> Self {
        TapOptions {
            sample_rate,
            block_frames: 1024,
            start: Ticks::ZERO,
            end,
            stop_at_end: false,
            quiet_seconds: 1.0,
            max_tail_seconds: 30.0,
            trim: false,
        }
    }

    /// A bounce's: the range, stopped at its end when it is cut out of the song, its tail
    /// up to 30 s, the latency trimmed.
    pub fn bounce(sample_rate: u32, start: Ticks, end: Ticks, stop_at_end: bool) -> Self {
        TapOptions {
            sample_rate,
            block_frames: 1024,
            start,
            end,
            stop_at_end,
            quiet_seconds: 1.0,
            max_tail_seconds: 30.0,
            trim: true,
        }
    }
}

/// A tapped render, in its raw file.
#[derive(Debug)]
pub struct Tapped {
    /// Interleaved stereo `f32`, little-endian; `frames` of it are the render (the rest a
    /// tail that turned out silent).
    pub raw: PathBuf,
    pub frames: usize,
    pub sample_rate: u32,
    /// The tap's latency: dropped from the front when trimmed, else what the frozen target
    /// reports as its own.
    pub latency: usize,
    pub peak: f32,
    /// Notes that played their streamed sample's start only.
    pub cut_notes: u64,
}

impl Drop for Tapped {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.raw);
    }
}

fn io(e: std::io::Error) -> RenderError {
    RenderError::Wav(hound::Error::IoError(e))
}

/// Render `snapshot` (its `rest` flags already set) tapped at `point` into the raw file
/// `raw`. `units` are the hosted plug-ins' fresh instances (`render_units`), `hosted` the
/// hosted instruments'; `progress` counts frames and stops at `cancel`.
#[allow(clippy::too_many_arguments)]
pub fn render_tapped(
    snapshot: Arc<Snapshot>,
    point: TapPoint,
    opts: &TapOptions,
    mut units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    progress: &RenderProgress,
    raw: &Path,
) -> Result<Tapped, RenderError> {
    let block = opts
        .block_frames
        .clamp(1, crate::messages::MAX_BLOCK_FRAMES);
    let sr = opts.sample_rate as f64;
    let mut snap = (*snapshot).clone();
    snap.loop_region = None;
    snap.metronome = false;
    crate::samples::conform_audio(&mut snap, opts.sample_rate);
    let snap = Arc::new(snap);
    let effect_count: usize = snap.effect_slots().count();
    let mut instruments = crate::inst::units_for(&snap, sr as f32, |_, _| false);
    for u in &mut instruments {
        u.unit.set_offline(true);
    }
    instruments.extend(hosted.into_iter().map(Box::new));
    let (mut tx, rx_engine) =
        RingBuffer::<EngineMessage>::new(16 + effect_count + instruments.len());
    let (tx_engine, mut rx) = RingBuffer::<EngineEvent>::new(64 + effect_count + instruments.len());
    let mut p = Processor::without_midi(sr, rx_engine, tx_engine);
    p.set_profiling(false);
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
    let plan = crate::graph::compile(&snap);
    tx.push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan))
        .ok();
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
    p.attach_stem_tap(Box::new(StemTap::with_pre(block)));
    tx.push(EngineMessage::Seek(opts.start)).ok();
    tx.push(EngineMessage::Play).ok();
    let (slot, latency) = match point {
        TapPoint::Strip(i) => (p.strip_slot(i), p.strip_latency(i)),
        TapPoint::Track(i) => (p.track_slot(i), p.track_latency(i)),
    };
    let slot = slot.ok_or(RenderError::Empty)?;
    let late = if opts.trim { latency } else { 0 };
    // The range's frames, and — kept, not trimmed — the latency they arrive after.
    let content = (snapshot
        .tempo
        .seconds_between(opts.start, opts.end.max(opts.start))
        * sr)
        .ceil() as usize
        + (latency - late);
    let quiet = (opts.quiet_seconds.max(0.0) * sr).ceil() as usize;
    let longest = content + (opts.max_tail_seconds.max(0.0) * sr).ceil() as usize;
    let mut stop_at = opts.stop_at_end.then(|| {
        crate::transport::sample_at_map(&snapshot.tempo, opts.start.0 as f64, opts.end.0 as f64, sr)
            as usize
    });
    let mut out = BufWriter::new(std::fs::File::create(raw).map_err(io)?);
    let mut buf = vec![0f32; block * 2];
    let mut bytes = Vec::with_capacity(block * 8);
    // Frames written (after the trim), the last one that was not zero, the peak.
    let mut written = 0usize;
    let mut last: Option<usize> = None;
    let mut peak = 0f32;
    let mut done = 0usize;
    // The tail is counted at its longest, the total cut to what was done when it ends
    // sooner (review 1: the card sat at 100 % while a tail rang for up to 30 s).
    progress.total.store((late + longest) as u64, Relaxed);
    progress.done.store(0, Relaxed);
    loop {
        if progress.cancel.load(Relaxed) {
            drop(out);
            let _ = std::fs::remove_file(raw);
            return Err(RenderError::Cancelled);
        }
        // Past the range: on while the tail sounds.
        if written >= content {
            let silent_for = written - last.map_or(0, |l| l + 1);
            if silent_for >= quiet || written >= longest {
                break;
            }
        }
        if stop_at == Some(done) {
            tx.push(EngineMessage::Stop).ok();
            stop_at = None;
        }
        let mut n = block;
        if let Some(s) = stop_at.filter(|s| *s < done + n) {
            n = s - done;
        }
        p.process(&mut buf[..n * 2], 2);
        let tap = p.stem_tap().expect("attached above");
        let [l, r] = match point {
            TapPoint::Strip(_) => tap.strip_pre(slot),
            TapPoint::Track(_) => tap.track_pre(slot),
        };
        bytes.clear();
        for k in 0..n {
            if done + k < late {
                continue;
            }
            let (a, b) = (l[k], r[k]);
            if a != 0.0 || b != 0.0 {
                last = Some(written);
                peak = peak.max(a.abs()).max(b.abs());
            }
            bytes.extend_from_slice(&a.to_le_bytes());
            bytes.extend_from_slice(&b.to_le_bytes());
            written += 1;
        }
        drop(tap);
        out.write_all(&bytes).map_err(io)?;
        done += n;
        progress.done.store(done as u64, Relaxed);
        while rx.pop().is_ok() {}
    }
    out.flush().map_err(io)?;
    drop(out);
    if p.stats().stream_late > 0 {
        let _ = std::fs::remove_file(raw);
        return Err(RenderError::Unreadable(crate::render::unreadable_in(
            &snapshot,
        )));
    }
    progress.total.store(done as u64, Relaxed);
    progress.done.store(done as u64, Relaxed);
    Ok(Tapped {
        raw: raw.to_path_buf(),
        frames: last.map_or(0, |l| l + 1),
        sample_rate: opts.sample_rate,
        latency,
        peak,
        cut_notes: p.stats().stream_cut,
    })
}

/// Read a tapped render's frames in order, `dst.len() / 2` at a time.
fn frames_of(t: &Tapped) -> std::io::Result<impl FnMut(&mut [f32]) -> std::io::Result<()>> {
    let mut f = std::io::BufReader::new(std::fs::File::open(&t.raw)?);
    let mut bytes = Vec::new();
    Ok(move |dst: &mut [f32]| {
        bytes.resize(dst.len() * 4, 0);
        f.read_exact(&mut bytes)?;
        for (d, b) in dst.iter_mut().zip(bytes.chunks_exact(4)) {
            *d = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        Ok(())
    })
}

/// The frozen render file of `t` at `path`, labelled with its render key (checked when it
/// is opened) and its latency (`crate::freeze::label`).
pub fn write_render(t: &Tapped, path: &Path, key: &str) -> Result<(), RenderError> {
    let fill = frames_of(t).map_err(io)?;
    let label = crate::freeze::label(key, t.latency);
    crate::inst::stream_cache::write_at(path, &label, 2, t.sample_rate, t.frames, fill)
        .map_err(io)?;
    Ok(())
}

/// A 32-bit float stereo WAV of `t` at `path` (a bounce in place's audio file).
pub fn write_wav(t: &Tapped, path: &Path) -> Result<(), RenderError> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: t.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    let mut fill = frames_of(t).map_err(io)?;
    let mut chunk = vec![0f32; 8192 * 2];
    let mut left = t.frames;
    while left > 0 {
        let n = left.min(8192);
        fill(&mut chunk[..n * 2]).map_err(io)?;
        for s in &chunk[..n * 2] {
            w.write_sample(*s)?;
        }
        left -= n;
    }
    w.finalize()?;
    Ok(())
}

/// A freeze's render, prepared on the interface's thread (the snapshot compiled and every
/// file it plays loaded, `EngineHandle::compile_for_render`) and run on a worker.
#[derive(Debug)]
pub struct FreezeJob {
    pub target: cantus_core::FreezeTarget,
    /// The content key it renders (`cantus_core::freeze::content_key`).
    pub content: String,
    /// Its render key and where it goes.
    pub key: String,
    pub path: PathBuf,
    pub snapshot: Arc<Snapshot>,
    pub point: TapPoint,
    pub opts: TapOptions,
}

/// Prepare the render of `target` as `project` holds it now, at the engine's rate, into
/// the store's folder. The target plays live in it (its own render is what is being
/// made); everything that does not reach it rests.
pub fn prepare(
    engine: &mut crate::EngineHandle,
    project: &cantus_core::Project,
    target: cantus_core::FreezeTarget,
) -> Result<FreezeJob, String> {
    use cantus_core::freeze::{self, FreezeTarget};
    let content = freeze::content_key(project, target).ok_or("gone")?;
    let rate = engine.sample_rate.round() as u32;
    let key = crate::freeze::render_key(&content, rate);
    let dir = engine
        .frozen
        .write_dir()
        .ok_or("no folder for frozen renders")?
        .to_path_buf();
    let path = dir.join(crate::freeze::file_name(&key));
    let mut alone = project.clone();
    // Live: the target, and every frozen target its region is not inside of — one whose
    // region holds the target would rest it (review 1: a nested strip's refresh came out
    // silent). The frozen targets inside its region play their renders.
    let region = freeze::region(project, target);
    for s in &mut alone.strips {
        if s.frozen.is_some()
            && (FreezeTarget::Strip(s.id) == target || !region.strips.contains(&s.id))
        {
            s.frozen = None;
        }
    }
    for t in &mut alone.tracks {
        if t.frozen.is_some()
            && (FreezeTarget::Track(t.id) == target || !region.tracks.contains(&t.id))
        {
            t.frozen = None;
        }
    }
    // Made unsoloed (solo is no part of a frozen sound — review 1: a solo elsewhere
    // rendered silence under the unsoloed key).
    freeze::neutral(&mut alone, target);
    let compiled = engine.compile_for_render(&alone, 0);
    let mut snap = (*compiled).clone();
    freeze::isolate(&mut snap, &region);
    let point = match target {
        FreezeTarget::Strip(s) => TapPoint::Strip(alone.strip_index(s).ok_or("gone")?),
        FreezeTarget::Track(t) => TapPoint::Track(alone.track_index(t).ok_or("gone")?),
    };
    let end = snap
        .loop_region
        .as_ref()
        .map_or(snap.content_end, |l| l.end.max(snap.content_end))
        .max(alone.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.end));
    Ok(FreezeJob {
        target,
        content,
        key,
        path,
        snapshot: Arc::new(snap),
        point,
        opts: TapOptions::freeze(rate, end),
    })
}

impl FreezeJob {
    /// Render and write the frozen file (on a worker). `units` and `hosted` are the hosted
    /// plug-ins' and instruments' fresh instances for the render.
    pub fn run(
        &self,
        units: Vec<EffectUnit>,
        hosted: Vec<crate::inst::NativeUnit>,
        progress: &RenderProgress,
    ) -> Result<PathBuf, RenderError> {
        let raw = self.path.with_extension("raw.part");
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d).map_err(io)?;
        }
        let t = render_tapped(
            self.snapshot.clone(),
            self.point,
            &self.opts,
            units,
            hosted,
            progress,
            &raw,
        )?;
        write_render(&t, &self.path, &self.key)?;
        Ok(self.path.clone())
    }
}
