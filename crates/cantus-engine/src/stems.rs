//! Stems: every strip's (or every track's) output as a file of its own, from **one**
//! offline pass (ADR 0021).
//!
//! The processor carries a [`StemTap`] while a stems render runs: at the two places where
//! a signal is mixed — a track into its strip, a strip out of its fader — it copies what it
//! is about to add. It reads and never writes, so the mix is the mix `render` makes, to the
//! bit, and the tap costs nothing when it is not attached (one `Option` test per segment).
//!
//! **Alignment.** Delay compensation (ADR 0018) makes every path reach the master at the
//! same moment, the total latency `L` after the timeline; a strip's fader output is earlier
//! than that by what is still ahead of it. So each stem is trimmed by its own lateness — a
//! track by nothing, a strip by the latency of everything up to and including its inserts
//! (`Processor::strip_latency`), the master by `L` — and sample 0 of every file is the
//! song's first tick. Then, with the master at unity, the stems of the strips that feed
//! the master, plus the tracks that go to it directly, add up to the master exactly.

use crate::effects::EffectUnit;
use crate::messages::{EngineEvent, EngineMessage, MAX_STRIPS, MAX_TRACKS};
use crate::processor::Processor;
use crate::render::RenderOptions;
use cantus_core::{EffectSlot, Snapshot};
use rtrb::RingBuffer;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "stems_master.rs"]
mod through;

/// Where a stem is taken from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StemSource {
    /// A mixer strip after its fader (index into `Snapshot::strips`): what it sends on,
    /// inserts, EQ, pan and mute included.
    Strip(usize),
    /// A track after its own level and balance, before its strip (index into
    /// `Snapshot::tracks`): the instrument as the channel rack sets it.
    Track(usize),
    /// Every track that feeds the master directly, summed: in a strips export, the part of
    /// the mix no strip carries.
    Direct,
    /// The master's output.
    Master,
}

/// One stem to write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StemSpec {
    pub source: StemSource,
    /// The file's name inside the folder (see [`stem_file_name`]).
    pub file_name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StemOptions {
    /// Sample rate, range, tail and block size, as for a bounce. `channels` and
    /// `normalize_peak_db` are ignored: stems are stereo and at the level they were mixed,
    /// or they would not add up to the mix.
    pub render: RenderOptions,
    /// Write a stem that came out as exact digital silence. Off by default: a desk opens
    /// with sixteen strips, and thirteen empty files are noise. The ones skipped are
    /// listed in [`StemsDone::silent`].
    pub keep_silent: bool,
    /// Pass every stem but the master's through its own copy of the master's insert
    /// chain, as if soloed into it (ADR 0026; off by default, see `stems_master.rs`).
    pub through_master: bool,
}

/// `NN Name.wav`: the number with two digits (strip or track number, from 1; 0 for the
/// master), a space, and the name with the characters no file system takes replaced.
pub fn stem_file_name(number: usize, name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = clean.trim().trim_end_matches('.').trim();
    if clean.is_empty() {
        format!("{number:02}.wav")
    } else {
        format!("{number:02} {clean}.wav")
    }
}

/// What the processor copies while a stems render runs: one block of every track's and
/// every strip's output, by slot, plus the direct-to-master sum. Allocated once, before
/// the render, for the render's block size.
#[derive(Debug)]
pub struct StemTap {
    frames: usize,
    strips: Vec<[Vec<f32>; 2]>,
    tracks: Vec<[Vec<f32>; 2]>,
    direct: [Vec<f32>; 2],
    /// A freeze's or a bounce's taps (round 29, ADR 0064), when asked for
    /// ([`StemTap::with_pre`]): each strip's pair before its width, each track's source
    /// before its level. Empty otherwise (the calls then do nothing).
    pre_strips: Vec<[Vec<f32>; 2]>,
    pre_tracks: Vec<[Vec<f32>; 2]>,
}

impl StemTap {
    pub fn new(block_frames: usize) -> Self {
        let pair = || [vec![0.0; block_frames], vec![0.0; block_frames]];
        StemTap {
            frames: 0,
            strips: (0..MAX_STRIPS).map(|_| pair()).collect(),
            tracks: (0..MAX_TRACKS).map(|_| pair()).collect(),
            direct: pair(),
            pre_strips: Vec::new(),
            pre_tracks: Vec::new(),
        }
    }

    /// A tap that also keeps every strip's pair before its width and every track's source
    /// before its level (a freeze's or a bounce's render, ADR 0064).
    pub fn with_pre(block_frames: usize) -> Self {
        let pair = || [vec![0.0; block_frames], vec![0.0; block_frames]];
        StemTap {
            pre_strips: (0..MAX_STRIPS).map(|_| pair()).collect(),
            pre_tracks: (0..MAX_TRACKS).map(|_| pair()).collect(),
            ..StemTap::new(block_frames)
        }
    }

    /// A strip's pair before its width, `at` frames into the block.
    pub(crate) fn strip_before_width(&mut self, slot: usize, at: usize, l: &[f32], r: &[f32]) {
        let n = l.len().min(self.frames.saturating_sub(at));
        if let Some([pl, pr]) = self.pre_strips.get_mut(slot) {
            pl[at..at + n].copy_from_slice(&l[..n]);
            pr[at..at + n].copy_from_slice(&r[..n]);
        }
    }

    /// A track's source before its level and balance, `at` frames into the block.
    pub(crate) fn track_source(&mut self, slot: usize, at: usize, l: &[f32], r: &[f32]) {
        let n = l.len().min(self.frames.saturating_sub(at));
        if let Some([pl, pr]) = self.pre_tracks.get_mut(slot) {
            pl[at..at + n].copy_from_slice(&l[..n]);
            pr[at..at + n].copy_from_slice(&r[..n]);
        }
    }

    /// Strip slot `slot`'s pair before its width this block (empty without [`Self::with_pre`]).
    pub fn strip_pre(&self, slot: usize) -> [&[f32]; 2] {
        match self.pre_strips.get(slot) {
            Some([l, r]) => [&l[..self.frames], &r[..self.frames]],
            None => [&[], &[]],
        }
    }

    /// Track slot `slot`'s source this block (empty without [`Self::with_pre`]).
    pub fn track_pre(&self, slot: usize) -> [&[f32]; 2] {
        match self.pre_tracks.get(slot) {
            Some([l, r]) => [&l[..self.frames], &r[..self.frames]],
            None => [&[], &[]],
        }
    }

    fn clear(pair: &mut [Vec<f32>; 2], n: usize) {
        for ch in pair.iter_mut() {
            ch[..n].iter_mut().for_each(|x| *x = 0.0);
        }
    }

    /// A new block of `frames`: everything starts silent, and whatever nothing writes to
    /// this block stays so.
    pub(crate) fn begin(&mut self, frames: usize) {
        let n = frames.min(self.direct[0].len());
        self.frames = n;
        for p in self
            .strips
            .iter_mut()
            .chain(self.tracks.iter_mut())
            .chain(self.pre_strips.iter_mut())
            .chain(self.pre_tracks.iter_mut())
        {
            Self::clear(p, n);
        }
        Self::clear(&mut self.direct, n);
    }

    /// A track's contribution, `at` frames into the block: the same products the
    /// processor adds to the strip or the master.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn track(
        &mut self,
        slot: usize,
        to_master: bool,
        at: usize,
        l: &[f32],
        r: &[f32],
        gl: f32,
        gr: f32,
    ) {
        let n = l.len().min(self.frames.saturating_sub(at));
        let [tl, tr] = &mut self.tracks[slot];
        for ((t, s), g) in [(tl, l), (tr, r)].into_iter().zip([gl, gr]) {
            for (o, x) in t[at..at + n].iter_mut().zip(s) {
                *o = x * g;
            }
        }
        if to_master {
            let [dl, dr] = &mut self.direct;
            for ((d, s), g) in [(dl, l), (dr, r)].into_iter().zip([gl, gr]) {
                for (o, x) in d[at..at + n].iter_mut().zip(s) {
                    *o += x * g;
                }
            }
        }
    }

    /// What leaves a strip's fader, `at` frames into the block.
    pub(crate) fn strip(&mut self, slot: usize, at: usize, l: &[f32], r: &[f32], gl: f32, gr: f32) {
        let n = l.len().min(self.frames.saturating_sub(at));
        let [sl, sr] = &mut self.strips[slot];
        for ((t, s), g) in [(sl, l), (sr, r)].into_iter().zip([gl, gr]) {
            for (o, x) in t[at..at + n].iter_mut().zip(s) {
                *o = x * g;
            }
        }
    }

    fn pair(&self, source: Tapped) -> &[Vec<f32>; 2] {
        match source {
            Tapped::Strip(slot) => &self.strips[slot],
            Tapped::Track(slot) => &self.tracks[slot],
            Tapped::Direct => &self.direct,
        }
    }
}

/// A source resolved to where the tap keeps it.
#[derive(Copy, Clone, Debug)]
enum Tapped {
    Strip(usize),
    Track(usize),
    Direct,
}

#[derive(Debug, thiserror::Error)]
pub enum StemsError {
    #[error("nothing to render: the project has no notes")]
    Empty,
    #[error("stopped")]
    Cancelled,
    #[error("wav error: {0}")]
    Wav(#[from] hound::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// A streamed sample could not be read (third review #3): the stems would be cut short
    /// where it could not, so there are none.
    #[error("a sample could not be read: {0}")]
    Unreadable(String),
}

/// A stem rendered to memory.
#[derive(Clone, Debug, PartialEq)]
pub struct Stem {
    pub spec: StemSpec,
    /// Interleaved stereo, aligned to the song's start.
    pub samples: Vec<f32>,
    pub peak: f32,
    /// Exact digital silence from end to end.
    pub silent: bool,
}

/// What a stems export to a folder did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StemsDone {
    pub written: Vec<PathBuf>,
    /// Stems that were exact silence and were not written, by file name.
    pub silent: Vec<String>,
    pub seconds: f64,
    /// The project's total latency, in samples, that the stems were aligned against.
    pub latency: usize,
    /// Some path could not be given its delay line, so some stem may be early.
    pub unaligned: bool,
    /// Notes that played only the start of a streamed sample (third review #9).
    pub cut_notes: u64,
}

/// Where one stem's samples go as they are rendered.
enum Target {
    Memory(Vec<f32>),
    /// Opened at the first sound, so a stem that never makes one leaves no file behind.
    File {
        path: PathBuf,
        writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>,
        spec: hound::WavSpec,
        /// Silent frames seen before the file was opened.
        pending: u64,
    },
}

struct Out {
    /// Where the tap keeps it; `None` is the master's output.
    tapped: Option<Tapped>,
    /// Its own copy of the master chain, when stems go through it.
    through: Option<Box<through::Through>>,
    /// A source the engine does not hold: silence.
    silent: bool,
    /// Frames to drop from the start: how late this source is.
    late: usize,
    /// Frames kept so far.
    kept: usize,
    peak: f32,
    target: Target,
}

impl Out {
    fn push(
        &mut self,
        l: &[f32],
        r: &[f32],
        frames_at: usize,
        total: usize,
    ) -> Result<(), StemsError> {
        // Keep the frames in [late, late + total) of the render.
        let from = self.late.saturating_sub(frames_at).min(l.len());
        let room = total - self.kept;
        let to = l.len().min(from + room);
        if to <= from {
            return Ok(());
        }
        let (l, r) = (&l[from..to], &r[from..to]);
        self.kept += to - from;
        let block_peak = l.iter().chain(r.iter()).fold(0f32, |m, x| m.max(x.abs()));
        self.peak = self.peak.max(block_peak);
        match &mut self.target {
            Target::Memory(v) => {
                for (a, b) in l.iter().zip(r) {
                    v.push(*a);
                    v.push(*b);
                }
            }
            Target::File {
                path,
                writer,
                spec,
                pending,
            } => {
                if writer.is_none()
                    && block_peak == 0.0
                    && l.iter().chain(r.iter()).all(|x| *x == 0.0)
                {
                    *pending += l.len() as u64;
                    return Ok(());
                }
                let w = match writer {
                    Some(w) => w,
                    None => {
                        let mut w = hound::WavWriter::create(&*path, *spec)?;
                        for _ in 0..*pending * 2 {
                            w.write_sample(0.0f32)?;
                        }
                        writer.insert(w)
                    }
                };
                for (a, b) in l.iter().zip(r) {
                    w.write_sample(*a)?;
                    w.write_sample(*b)?;
                }
            }
        }
        Ok(())
    }
}

/// Render stems to memory: every stem at full length, silent ones flagged.
pub fn render_stems(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
) -> Result<Vec<Stem>, StemsError> {
    render_stems_with(snapshot, opts, specs, &mut |slot, sr| {
        EffectUnit::new(slot, sr)
    })
}

/// [`render_stems`] with the effect units built by `make_unit` — for tests that stand a
/// unit of known latency in for a real effect.
pub fn render_stems_with(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    make_unit: &mut dyn FnMut(&EffectSlot, f32) -> EffectUnit,
) -> Result<Vec<Stem>, StemsError> {
    in_memory(snapshot, opts, specs, make_unit, Vec::new())
}

/// [`render_stems`] with the hosted plug-ins' and instruments' own instances, as
/// [`render_stems_to_folder_with`] takes them.
pub fn render_stems_hosted(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    mut units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
) -> Result<Vec<Stem>, StemsError> {
    in_memory(
        snapshot,
        opts,
        specs,
        &mut take_or_build(&mut units),
        hosted,
    )
}

/// An effect's unit: the one handed in for it (a hosted plug-in's instance), else the one
/// the engine builds.
fn take_or_build(units: &mut Vec<EffectUnit>) -> impl FnMut(&EffectSlot, f32) -> EffectUnit + '_ {
    move |slot, sr| match units.iter().position(|u| u.id == slot.id) {
        Some(i) => units.swap_remove(i),
        None => EffectUnit::new(slot, sr),
    }
}

fn in_memory(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    make_unit: &mut dyn FnMut(&EffectSlot, f32) -> EffectUnit,
    hosted: Vec<crate::inst::NativeUnit>,
) -> Result<Vec<Stem>, StemsError> {
    let mut outs: Vec<Target> = specs.iter().map(|_| Target::Memory(Vec::new())).collect();
    let (outs, _) = run(
        snapshot,
        opts,
        specs,
        make_unit,
        hosted,
        &mut outs,
        &mut |_| true,
    )?;
    Ok(outs
        .into_iter()
        .zip(specs)
        .map(|((target, peak), spec)| {
            let samples = match target {
                Target::Memory(v) => v,
                Target::File { .. } => Vec::new(),
            };
            Stem {
                spec: spec.clone(),
                silent: samples.iter().all(|x| *x == 0.0),
                samples,
                peak,
            }
        })
        .collect())
}

/// Render stems into `folder` as 32-bit float WAV files named by their specs, in one pass.
/// `progress` hears the fraction done after every block and stops the render by
/// answering `false`; the files written so far are then removed.
pub fn render_stems_to_folder(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    folder: &Path,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<StemsDone, StemsError> {
    render_stems_to_folder_with(
        snapshot,
        opts,
        specs,
        folder,
        Vec::new(),
        Vec::new(),
        progress,
    )
}

/// [`render_stems_to_folder`] with the hosted plug-ins' and instruments' own instances for
/// this render, loaded on the main thread with the live ones' state (ADR 0019, ADR 0028),
/// as [`crate::render_with_progress`] takes them: an effect with a unit here gets it, every
/// other the unit the engine builds. One instance cannot be in two places, so the copies
/// of the master chain a stem goes *through* (`through_master`) build their own units —
/// a hosted insert on the master is a wire there.
pub fn render_stems_to_folder_with(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    folder: &Path,
    mut units: Vec<EffectUnit>,
    hosted: Vec<crate::inst::NativeUnit>,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<StemsDone, StemsError> {
    std::fs::create_dir_all(folder)?;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: opts.render.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut targets: Vec<Target> = specs
        .iter()
        .map(|s| Target::File {
            path: folder.join(&s.file_name),
            writer: None,
            spec,
            pending: 0,
        })
        .collect();
    let result = run(
        snapshot,
        opts,
        specs,
        &mut take_or_build(&mut units),
        hosted,
        &mut targets,
        progress,
    );
    let (outs, mut done) = match result {
        Ok(r) => r,
        Err(e) => {
            // Nothing half-written is left behind.
            for t in targets {
                if let Target::File {
                    path,
                    writer: Some(w),
                    ..
                } = t
                {
                    drop(w);
                    let _ = std::fs::remove_file(path);
                }
            }
            return Err(e);
        }
    };
    for ((target, _), s) in outs.into_iter().zip(specs) {
        let Target::File {
            path,
            writer,
            spec,
            pending,
        } = target
        else {
            continue;
        };
        match writer {
            Some(w) => {
                w.finalize()?;
                done.written.push(path);
            }
            None if opts.keep_silent => {
                let mut w = hound::WavWriter::create(&path, spec)?;
                for _ in 0..pending * 2 {
                    w.write_sample(0.0f32)?;
                }
                w.finalize()?;
                done.written.push(path);
            }
            None => done.silent.push(s.file_name.clone()),
        }
    }
    Ok(done)
}

/// The one pass: the same set-up as `render::render` — the loop and the click off, the
/// effect units delivered, delay compensation settled before the first block — with the
/// tap attached, and every block handed to the stems that want it.
#[allow(clippy::type_complexity)]
fn run(
    snapshot: Arc<Snapshot>,
    opts: &StemOptions,
    specs: &[StemSpec],
    make_unit: &mut dyn FnMut(&EffectSlot, f32) -> EffectUnit,
    hosted: Vec<crate::inst::NativeUnit>,
    targets: &mut Vec<Target>,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<(Vec<(Target, f32)>, StemsDone), StemsError> {
    let ro = &opts.render;
    let end = ro.end.unwrap_or(snapshot.content_end);
    if end <= ro.start && ro.end.is_none() {
        return Err(StemsError::Empty);
    }
    let block = ro.block_frames.clamp(1, crate::messages::MAX_BLOCK_FRAMES);
    let sr = ro.sample_rate as f64;

    let mut snap = (*snapshot).clone();
    snap.loop_region = None;
    snap.metronome = false;
    crate::samples::conform_audio(&mut snap, ro.sample_rate);
    let snap = Arc::new(snap);
    let effect_count: usize = snap.effect_slots().count();
    // Native instruments' units, as `render::render` sends them.
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
    // A frozen render's reader waits for its blocks (ADR 0064).
    p.set_offline(true);
    for slot in snap.effect_slots() {
        let unit = make_unit(slot, sr as f32);
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
    p.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(crate::pdc::lines_for(count, delay)))
                .ok();
            p.settle();
        }
    }
    p.attach_stem_tap(Box::new(StemTap::new(block)));
    tx.push(EngineMessage::Seek(ro.start)).ok();
    tx.push(EngineMessage::Play).ok();

    // Where each stem is kept, and how late it is.
    let latency = p.stats().latency_samples as usize;
    let mut outs: Vec<Out> = Vec::with_capacity(specs.len());
    for (spec, target) in specs.iter().zip(targets.drain(..)) {
        // A source the engine does not hold (past its limits) is silence, not an error.
        let (tapped, late, silent) = match spec.source {
            StemSource::Master => (None, latency, false),
            StemSource::Direct => (Some(Tapped::Direct), 0, false),
            StemSource::Track(i) => match p.track_slot(i) {
                Some(slot) => (Some(Tapped::Track(slot)), 0, false),
                None => (None, 0, true),
            },
            StemSource::Strip(i) => match p.strip_slot(i) {
                Some(slot) => (Some(Tapped::Strip(slot)), p.strip_latency(i), false),
                None => (None, 0, true),
            },
        };
        let through = match (opts.through_master, spec.source) {
            (true, StemSource::Strip(_) | StemSource::Track(_) | StemSource::Direct) => {
                through::Through::new(&snapshot, sr as f32, block, make_unit)
            }
            _ => None,
        };
        // The chain delays the stem by its latency: trimmed like the stem's own.
        let late = late + through.as_ref().map_or(0, |t| t.latency());
        outs.push(Out {
            tapped,
            through,
            silent,
            late,
            kept: 0,
            peak: 0.0,
            target,
        });
    }

    // Through the tempo map (ADR 0060), as `render` measures it.
    let content_seconds = snapshot.tempo.seconds_between(ro.start, end.max(ro.start));
    let total = ((content_seconds + ro.tail_seconds) * sr).ceil() as usize;
    let longest = outs.iter().map(|o| o.late).max().unwrap_or(0);
    let frames = total + longest;
    // A range that ends before the song stops there, as `render` does: the tail rings out
    // and plays nothing that starts after the range.
    let mut stop_at = ro.end.map(|e| {
        crate::transport::sample_at_map(&snapshot.tempo, ro.start.0 as f64, e.0 as f64, sr) as usize
    });
    let mut buf = vec![0f32; block * 2];
    let mut ml = vec![0f32; block];
    let mut mr = vec![0f32; block];
    let zeros = vec![0f32; block];
    let mut done = 0usize;
    while done < frames {
        if stop_at == Some(done) {
            tx.push(EngineMessage::Stop).ok();
            stop_at = None;
        }
        let mut n = block.min(frames - done);
        if let Some(s) = stop_at.filter(|s| *s < done + n) {
            n = s - done;
        }
        let chunk = &mut buf[..n * 2];
        p.process(chunk, 2);
        for ((frame, a), b) in chunk.chunks_exact(2).zip(ml.iter_mut()).zip(mr.iter_mut()) {
            *a = frame[0];
            *b = frame[1];
        }
        let tap = p.stem_tap().expect("attached above");
        let mut failure = None;
        for o in outs.iter_mut() {
            let (l, r): (&[f32], &[f32]) = match (o.tapped, o.silent) {
                (_, true) => (&zeros[..n], &zeros[..n]),
                (None, false) => (&ml[..n], &mr[..n]),
                (Some(t), false) => {
                    let [l, r] = tap.pair(t);
                    (&l[..n], &r[..n])
                }
            };
            let mut through = o.through.take();
            let (l, r) = match through.as_mut() {
                Some(t) => {
                    t.run(l, r);
                    t.out(n)
                }
                None => (l, r),
            };
            let pushed = o.push(l, r, done, total);
            o.through = through;
            if let Err(e) = pushed {
                failure = Some(e);
                break;
            }
        }
        done += n;
        while rx.pop().is_ok() {}
        if failure.is_none() && !progress(done as f32 / frames as f32) {
            failure = Some(StemsError::Cancelled);
        }
        if let Some(e) = failure {
            // Hand the targets back so the caller can remove what was opened.
            targets.extend(outs.into_iter().map(|o| o.target));
            return Err(e);
        }
    }
    // As the mix (`render`): a streamed voice that ended early offline fails the stems,
    // naming the file — never stems silently cut short (third review #3).
    if p.stats().stream_late > 0 {
        targets.extend(outs.into_iter().map(|o| o.target));
        return Err(StemsError::Unreadable(crate::render::unreadable_in(
            &snapshot,
        )));
    }
    let summary = StemsDone {
        written: Vec::new(),
        silent: Vec::new(),
        seconds: total as f64 / sr,
        latency,
        unaligned: p.stats().pdc_unserved > 0,
        cut_notes: p.stats().stream_cut,
    };
    Ok((
        outs.into_iter().map(|o| (o.target, o.peak)).collect(),
        summary,
    ))
}
