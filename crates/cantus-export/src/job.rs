//! An export from start to finish, on the caller's (background) thread: render, convert,
//! normalise, encode, and the file put in its place only when it is whole.

use crate::encode;
use crate::process::{self, Audio, Normalised};
use crate::settings::{ExportSettings, Tags};
use crate::ExportError;
use cantus_core::Snapshot;
use cantus_engine::{EffectUnit, NativeUnit, RenderOptions, StemOptions, StemSpec};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// Where an export is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Render,
    Convert,
    Loudness,
    Encode,
}

impl Phase {
    fn from_u8(v: u8) -> Phase {
        match v {
            1 => Phase::Convert,
            2 => Phase::Loudness,
            3 => Phase::Encode,
            _ => Phase::Render,
        }
    }

    /// The phase's locale key, for the status line.
    pub fn key(self) -> &'static str {
        match self {
            Phase::Render => "export_audio.phase.render",
            Phase::Convert => "export_audio.phase.convert",
            Phase::Loudness => "export_audio.phase.loudness",
            Phase::Encode => "export_audio.phase.encode",
        }
    }
}

/// Shared between the export thread and the interface: how far it has got, and whether
/// it has been asked to stop.
#[derive(Debug, Default)]
pub struct Control {
    cancel: AtomicBool,
    phase: AtomicU8,
    /// The whole export's progress, in thousandths.
    permille: AtomicU32,
    /// The render reports no progress of its own (ADR 0033 §6): until it does, the bar
    /// shows it as a pulse.
    pub render_is_indeterminate: AtomicBool,
    /// Stop by itself once this far, in thousandths (0 = never): how the tests stop an
    /// export at an exact moment.
    stop_at: AtomicU32,
}

impl Control {
    pub fn new() -> Arc<Control> {
        Arc::new(Control::default())
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn phase(&self) -> Phase {
        Phase::from_u8(self.phase.load(Ordering::Relaxed))
    }

    /// The whole export, 0…1.
    pub fn fraction(&self) -> f32 {
        self.permille.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub(crate) fn set(&self, phase: Phase, fraction: f32) {
        self.phase.store(phase as u8, Ordering::Relaxed);
        let f = (fraction.clamp(0.0, 1.0) * 1000.0) as u32;
        self.permille.store(f, Ordering::Relaxed);
        let stop = self.stop_at.load(Ordering::Relaxed);
        if stop > 0 && f >= stop {
            self.cancel();
        }
    }

    /// Ask the export to stop by itself once it is `fraction` done (for tests).
    pub fn stop_at(&self, fraction: f32) {
        let f = ((fraction.clamp(0.0, 1.0) * 1000.0) as u32).max(1);
        self.stop_at.store(f, Ordering::Relaxed);
    }

    /// A progress callback for the part of the export between `from` and `to`.
    pub(crate) fn span(&self, phase: Phase, from: f32, to: f32) -> impl FnMut(f32) -> bool + '_ {
        move |f| {
            self.set(phase, from + (to - from) * f.clamp(0.0, 1.0));
            !self.cancelled()
        }
    }
}

/// What an export did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Done {
    pub files: Vec<PathBuf>,
    pub seconds: f64,
    /// The file's rate.
    pub rate: u32,
    /// What loudness normalisation did (a mix only).
    pub loudness: Option<Normalised>,
    /// The largest sample written, dBFS.
    pub peak_db: f32,
    /// Stems left out as silent, by file name.
    pub silent: Vec<String>,
    /// Some stem may be early (a path without its delay line).
    pub unaligned: bool,
}

/// A file being written: `name.ext.part` until [`Part::commit`] renames it; dropped
/// before that, it is removed.
pub(crate) struct Part {
    tmp: PathBuf,
    path: PathBuf,
    done: bool,
}

impl Part {
    pub(crate) fn new(path: &Path) -> Part {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".part");
        Part {
            tmp: path.with_file_name(name),
            path: path.to_path_buf(),
            done: false,
        }
    }

    pub(crate) fn write(
        &self,
        f: impl FnOnce(&mut std::io::BufWriter<std::fs::File>) -> Result<(), ExportError>,
    ) -> Result<(), ExportError> {
        let file = std::fs::File::create(&self.tmp)?;
        let mut w = std::io::BufWriter::with_capacity(1 << 20, file);
        f(&mut w)?;
        use std::io::Write;
        w.flush()?;
        w.into_inner()
            .map_err(|e| ExportError::Io(e.into_error()))?
            .sync_all()?;
        Ok(())
    }

    pub(crate) fn commit(mut self) -> Result<PathBuf, ExportError> {
        std::fs::rename(&self.tmp, &self.path)?;
        self.done = true;
        Ok(self.path.clone())
    }
}

impl Drop for Part {
    fn drop(&mut self) {
        if !self.done {
            let _ = std::fs::remove_file(&self.tmp);
        }
    }
}

pub(crate) fn peak_db(a: &Audio) -> f32 {
    let p = a.peak();
    if p > 0.0 {
        20.0 * cantus_dsp::math::log10(p)
    } else {
        f32::NEG_INFINITY
    }
}

/// The pipeline after the render: channels, rate, loudness (when asked and not stems),
/// then the file. Progress from `from` to `to`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_one(
    rendered: Audio,
    s: &ExportSettings,
    tags: &Tags,
    path: &Path,
    ctl: &Control,
    (from, to): (f32, f32),
    normalise: bool,
) -> Result<(PathBuf, Audio, Option<Normalised>), ExportError> {
    let span = to - from;
    ctl.set(Phase::Convert, from);
    let rate = s.file_rate(rendered.rate);
    let mut a = process::resample(process::to_channels(rendered, s.channel_count()), rate);
    if ctl.cancelled() {
        return Err(ExportError::Cancelled);
    }
    let mut report = None;
    let enc_from = if normalise {
        let at = from + span * 0.15;
        ctl.set(Phase::Loudness, at);
        let r = process::normalise(&mut a, &s.loudness, &mut || ctl.cancelled())
            .ok_or(ExportError::Cancelled)?;
        report = Some(r);
        from + span * 0.4
    } else {
        from + span * 0.1
    };
    let part = Part::new(path);
    part.write(|w| {
        encode::write(w, &a, s, tags, &mut ctl.span(Phase::Encode, enc_from, to))
    })?;
    if ctl.cancelled() {
        return Err(ExportError::Cancelled);
    }
    Ok((part.commit()?, a, report))
}

/// Encode audio that is already rendered (interleaved, at its rate) into `path`.
pub fn export_rendered(
    rendered: Audio,
    s: &ExportSettings,
    tags: &Tags,
    path: &Path,
    ctl: &Control,
) -> Result<Done, ExportError> {
    let normalise = s.normalises();
    let (file, a, loudness) = finish_one(rendered, s, tags, path, ctl, (0.0, 1.0), normalise)?;
    Ok(Done {
        files: vec![file],
        seconds: a.seconds(),
        rate: a.rate,
        peak_db: peak_db(&a),
        loudness,
        ..Default::default()
    })
}

/// What a mix export renders: the snapshot and the render's options, with the hosted
/// plug-ins' and instruments' own instances (loaded by the caller on the main thread).
pub struct MixJob {
    pub snapshot: Arc<Snapshot>,
    /// The session's rate, the range (`start`, `end` for a selection) and the tail.
    pub render: RenderOptions,
    pub units: Vec<EffectUnit>,
    pub instruments: Vec<NativeUnit>,
}

/// Render the mix (the song or the selection) and write it.
pub fn export_mix(
    job: MixJob,
    s: &ExportSettings,
    tags: &Tags,
    path: &Path,
    ctl: &Control,
) -> Result<Done, ExportError> {
    // The mix render reports no progress and cannot stop midway yet: the bar pulses
    // through it, and a stop takes effect when it returns. When pool's
    // `render_with_progress` lands (docs/round22/requests.md), pass
    // `&mut ctl.span(Phase::Render, 0.0, 0.5)` to it and drop the flag.
    ctl.render_is_indeterminate.store(true, Ordering::Relaxed);
    ctl.set(Phase::Render, 0.0);
    let mut opts = job.render;
    opts.channels = 2;
    opts.normalize_peak_db = None;
    let r = cantus_engine::render_with_instruments(job.snapshot, &opts, job.units, job.instruments)
        .map_err(|e| match e {
            cantus_engine::RenderError::Empty => ExportError::Empty,
            e => ExportError::Render(e.to_string()),
        })?;
    ctl.render_is_indeterminate.store(false, Ordering::Relaxed);
    if ctl.cancelled() {
        return Err(ExportError::Cancelled);
    }
    let rendered = Audio {
        rate: r.sample_rate,
        channels: r.channels,
        samples: r.samples,
    };
    let normalise = s.normalises();
    let (file, a, loudness) = finish_one(rendered, s, tags, path, ctl, (0.5, 1.0), normalise)?;
    Ok(Done {
        files: vec![file],
        seconds: a.seconds(),
        rate: a.rate,
        peak_db: peak_db(&a),
        loudness,
        ..Default::default()
    })
}

/// A stem's file name in the chosen format: `NN Name.wav` becomes `NN Name.mp3`.
pub fn stem_output_name(wav_name: &str, s: &ExportSettings) -> String {
    let stem = wav_name.strip_suffix(".wav").unwrap_or(wav_name);
    format!("{stem}.{}", s.format.extension())
}

/// What a stems export renders.
pub struct StemsJob {
    pub snapshot: Arc<Snapshot>,
    pub options: StemOptions,
    pub specs: Vec<StemSpec>,
}
