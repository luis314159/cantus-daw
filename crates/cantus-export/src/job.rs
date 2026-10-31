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
    /// The mix render's own progress and stop (`cantus_engine::render_with_progress`):
    /// frames done of the total while [`Phase::Render`] runs, and the flag a stop sets.
    pub render: cantus_engine::RenderProgress,
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
        // The render stops at its next block.
        self.render.cancel();
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn phase(&self) -> Phase {
        Phase::from_u8(self.phase.load(Ordering::Relaxed))
    }

    /// The whole export, 0…1: while a mix renders, its frames done count for the render's
    /// share of the bar.
    pub fn fraction(&self) -> f32 {
        let own = self.permille.load(Ordering::Relaxed) as f32 / 1000.0;
        if self.phase() == Phase::Render {
            own.max(MIX_RENDER_SHARE * self.render.fraction())
        } else {
            own
        }
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

/// The share of the bar a mix's render takes; converting, measuring and encoding the rest.
pub const MIX_RENDER_SHARE: f32 = 0.5;

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
    /// Samples past full scale that the format cuts off (every format but 32-bit float
    /// WAV): integers clip there, and decoders of MP3 and Ogg do.
    pub clipped: usize,
    /// Notes that played only the start of a streamed sample, read faster than streaming
    /// serves (ADR 0050, third review #9): said with the result.
    pub cut_notes: u64,
}

/// A file being written: `name.ext.part` until [`Part::commit`] renames it; dropped
/// before that, it is removed.
pub(crate) struct Part {
    tmp: PathBuf,
    path: PathBuf,
    done: bool,
    /// Never replace what is at `path` (`ExportSettings::no_replace`).
    no_replace: bool,
}

impl Part {
    pub(crate) fn new(path: &Path, no_replace: bool) -> Part {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".part");
        Part {
            tmp: path.with_file_name(name),
            path: path.to_path_buf(),
            done: false,
            no_replace,
        }
    }

    /// Put the part at its path: a rename, or — never replacing — [`place_new`].
    fn place(&self) -> std::io::Result<()> {
        if self.no_replace {
            place_new(&self.tmp, &self.path)
        } else {
            std::fs::rename(&self.tmp, &self.path)
        }
    }

    pub(crate) fn write(
        &self,
        f: impl FnOnce(&mut std::io::BufWriter<std::fs::File>) -> Result<(), ExportError>,
    ) -> Result<(), ExportError> {
        let file = new_part(&self.tmp)?;
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
        self.place()?;
        self.done = true;
        Ok(self.path.clone())
    }
}

/// Open a part to write: a new file, never through a link. A stale part left by a crash (a
/// plain file) is removed first; anything else there is refused (fix review 3 #8).
pub fn new_part(tmp: &Path) -> std::io::Result<std::fs::File> {
    if let Ok(m) = std::fs::symlink_metadata(tmp) {
        if m.file_type().is_file() {
            std::fs::remove_file(tmp)?;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "something that is not a file is where the part goes",
            ));
        }
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)
}

/// Put a written part at `path`, never replacing anything there: a hard link, which fails if
/// a file or a link is there. Where the file system has no hard links (FAT32, exFAT, some
/// network shares — fix review 3 #2) the system's no-replace rename
/// ([`cantus_rt::place::rename_no_replace`], fix review 4 #3): one step, which a crash
/// cannot leave half done. Only where that is missing too is the part copied into a file
/// made new, synced, and that file removed again if the copy fails. Once the target is in
/// place, removing the part is best effort (fix review 3 #7): the output is there either way.
pub fn place_new(part: &Path, path: &Path) -> std::io::Result<()> {
    place_new_with(
        part,
        path,
        |a, b| std::fs::hard_link(a, b),
        cantus_rt::place::rename_no_replace,
    )
}

/// [`place_new`] with the linking and renaming steps given (tests: a file system without
/// links, without a no-replace rename).
pub fn place_new_with(
    part: &Path,
    path: &Path,
    link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    rename: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let exists = |e: &std::io::Error| e.kind() == std::io::ErrorKind::AlreadyExists;
    match link(part, path) {
        Ok(()) => {}
        Err(e) if exists(&e) => return Err(e),
        Err(_) => match rename(part, path) {
            Ok(()) => return Ok(()),
            Err(e) if exists(&e) => return Err(e),
            Err(_) => copy_new(part, path)?,
        },
    }
    let _ = std::fs::remove_file(part);
    Ok(())
}

/// The last resort of [`place_new_with`]: the part copied into a file made new at `path`,
/// synced, and that file removed again if the copy fails.
fn copy_new(part: &Path, path: &Path) -> std::io::Result<()> {
    let mut to = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let copied = std::fs::File::open(part)
        .and_then(|mut from| std::io::copy(&mut from, &mut to))
        .and_then(|_| to.sync_all());
    if let Err(e) = copied {
        drop(to);
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

/// Put every part in its place, or none: each file a part would replace is moved aside
/// first (`name.ext.old`) and put back if any rename fails; the moved files go only once
/// every part is in. A destination another program holds open (a player with the last
/// export's stem) stops it before anything is replaced.
pub(crate) fn commit_all(parts: Vec<Part>) -> Result<Vec<PathBuf>, ExportError> {
    let mut aside: Vec<(PathBuf, PathBuf)> = Vec::new();
    let restore = |aside: &[(PathBuf, PathBuf)]| {
        for (path, old) in aside.iter().rev() {
            let _ = std::fs::rename(old, path);
        }
    };
    for part in &parts {
        if part.no_replace {
            if std::fs::symlink_metadata(&part.path).is_ok() {
                restore(&aside);
                return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
            }
            continue;
        }
        if part.path.exists() {
            let mut name = part.path.file_name().unwrap_or_default().to_os_string();
            name.push(".old");
            let old = part.path.with_file_name(name);
            if let Err(e) = std::fs::rename(&part.path, &old) {
                restore(&aside);
                return Err(e.into());
            }
            aside.push((part.path.clone(), old));
        }
    }
    let mut placed: Vec<PathBuf> = Vec::with_capacity(parts.len());
    for mut part in parts {
        if let Err(e) = part.place() {
            for p in &placed {
                let _ = std::fs::remove_file(p);
            }
            restore(&aside);
            return Err(e.into());
        }
        part.done = true;
        placed.push(part.path.clone());
    }
    for (_, old) in &aside {
        let _ = std::fs::remove_file(old);
    }
    Ok(placed)
}

impl Drop for Part {
    fn drop(&mut self) {
        if !self.done {
            let _ = std::fs::remove_file(&self.tmp);
        }
    }
}

/// How many samples of `a` the format chosen in `s` cuts at full scale.
pub(crate) fn clipped(a: &Audio, s: &ExportSettings) -> usize {
    if s.format == crate::settings::Format::Wav && !s.depth.is_int() {
        return 0;
    }
    a.samples.iter().filter(|x| x.abs() > 1.0).count()
}

pub(crate) fn peak_db(a: &Audio) -> f32 {
    let p = a.peak();
    if p > 0.0 {
        20.0 * cantus_dsp::math::log10(p)
    } else {
        f32::NEG_INFINITY
    }
}

/// [`encode_one`], and the file put in its place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_one(
    rendered: Audio,
    s: &ExportSettings,
    tags: &Tags,
    path: &Path,
    ctl: &Control,
    span: (f32, f32),
    normalise: bool,
) -> Result<(PathBuf, Audio, Option<Normalised>), ExportError> {
    let (part, a, report) = encode_one(rendered, s, tags, path, ctl, span, normalise)?;
    Ok((part.commit()?, a, report))
}

/// The pipeline after the render: channels, rate, loudness (when asked and not stems),
/// then the file, whole, beside its place (`name.ext.part`) until the caller commits it.
/// Progress from `from` to `to`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn encode_one(
    rendered: Audio,
    s: &ExportSettings,
    tags: &Tags,
    path: &Path,
    ctl: &Control,
    (from, to): (f32, f32),
    normalise: bool,
) -> Result<(Part, Audio, Option<Normalised>), ExportError> {
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
    let part = Part::new(path, s.no_replace);
    part.write(|w| encode::write(w, &a, s, tags, &mut ctl.span(Phase::Encode, enc_from, to)))?;
    if ctl.cancelled() {
        return Err(ExportError::Cancelled);
    }
    Ok((part, a, report))
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
        clipped: clipped(&a, s),
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
    // The render reports its frames into `ctl.render` (the bar's first half) and stops at
    // the next block after a stop. Sample 0 of the file is the range's first tick: the
    // project's latency is compensated, as the stems are.
    ctl.set(Phase::Render, 0.0);
    let mut opts = job.render;
    opts.channels = 2;
    opts.normalize_peak_db = None;
    opts.compensate_latency = true;
    let r = cantus_engine::render_with_progress(
        job.snapshot,
        &opts,
        job.units,
        job.instruments,
        &ctl.render,
    )
    .map_err(|e| match e {
        cantus_engine::RenderError::Empty => ExportError::Empty,
        cantus_engine::RenderError::Cancelled => ExportError::Cancelled,
        e => ExportError::Render(e.to_string()),
    })?;
    if ctl.cancelled() {
        return Err(ExportError::Cancelled);
    }
    let cut_notes = r.cut_notes;
    let rendered = Audio {
        rate: r.sample_rate,
        channels: r.channels,
        samples: r.samples,
    };
    let normalise = s.normalises();
    let span = (MIX_RENDER_SHARE, 1.0);
    let (file, a, loudness) = finish_one(rendered, s, tags, path, ctl, span, normalise)?;
    Ok(Done {
        files: vec![file],
        seconds: a.seconds(),
        rate: a.rate,
        peak_db: peak_db(&a),
        clipped: clipped(&a, s),
        loudness,
        cut_notes,
        ..Default::default()
    })
}

/// A stem's file name in the chosen format: `NN Name.wav` becomes `NN Name.mp3`.
pub fn stem_output_name(wav_name: &str, s: &ExportSettings) -> String {
    let stem = wav_name.strip_suffix(".wav").unwrap_or(wav_name);
    format!("{stem}.{}", s.format.extension())
}

/// What a stems export renders, with the hosted plug-ins' and instruments' own instances
/// (loaded by the caller on the main thread), as a mix's.
pub struct StemsJob {
    pub snapshot: Arc<Snapshot>,
    pub options: StemOptions,
    pub specs: Vec<StemSpec>,
    pub units: Vec<EffectUnit>,
    pub instruments: Vec<NativeUnit>,
}

#[cfg(test)]
mod place_tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cantus-place-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Fix review 3 #2: where the file system has no hard links, the part is copied into a
    /// new file — never over one that is there.
    #[test]
    fn without_hard_links_the_part_is_copied_into_a_new_file() {
        let d = dir("fat");
        let (part, path) = (d.join("mix.wav.part"), d.join("mix.wav"));
        std::fs::write(&part, b"the export").unwrap();
        let no_links = |_: &Path, _: &Path| Err(std::io::Error::other("Incorrect function"));
        let no_rename = |_: &Path, _: &Path| Err(std::io::ErrorKind::Unsupported.into());
        place_new_with(&part, &path, no_links, no_rename).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"the export");
        assert!(!part.exists());
        std::fs::write(&part, b"again").unwrap();
        let e = place_new_with(&part, &path, no_links, no_rename).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"the export",
            "never replaced"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Fix review 3 #7: once linked, a part that cannot be removed does not fail the output.
    #[test]
    fn a_placed_output_succeeds_even_if_its_part_stays() {
        let d = dir("linked");
        let (part, path) = (d.join("a.part"), d.join("a.wav"));
        std::fs::write(&part, b"x").unwrap();
        // A link step that succeeds and takes the part away first: removing it then fails.
        let linked = |a: &Path, b: &Path| std::fs::rename(a, b);
        let unused = |_: &Path, _: &Path| -> std::io::Result<()> { unreachable!() };
        place_new_with(&part, &path, linked, unused).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"x");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Fix review 3 #8: a part is never opened through a link; a stale plain part goes.
    #[test]
    fn a_part_is_new_and_never_a_link() {
        let d = dir("part");
        let part = d.join("b.wav.part");
        std::fs::write(&part, b"stale").unwrap();
        drop(new_part(&part).unwrap());
        assert_eq!(
            std::fs::read(&part).unwrap(),
            b"",
            "the stale part replaced by a new one"
        );
        let _ = std::fs::remove_file(&part);
        std::fs::create_dir(&part).unwrap();
        assert!(new_part(&part).is_err(), "not a file: refused");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Fix review 4 #2: a link where the part goes, to a file elsewhere, is refused and that
    /// file left whole (`File::create` would have followed it and emptied it). Windows makes
    /// file links only with the privilege or developer mode: skipped where it cannot.
    #[test]
    fn a_link_where_the_part_goes_is_refused_and_its_file_untouched() {
        let (d, elsewhere) = (dir("part-link"), dir("part-link-elsewhere"));
        let (part, theirs) = (d.join("c.wav.part"), elsewhere.join("theirs.txt"));
        std::fs::write(&theirs, b"someone's file").unwrap();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&theirs, &part);
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(&theirs, &part);
        if let Err(e) = made {
            eprintln!("no file link could be made here ({e}): skipped");
            let _ = std::fs::remove_dir_all(&d);
            let _ = std::fs::remove_dir_all(&elsewhere);
            return;
        }
        assert!(new_part(&part).is_err(), "a link: refused");
        assert_eq!(std::fs::read(&theirs).unwrap(), b"someone's file");
        assert!(std::fs::symlink_metadata(&part)
            .unwrap()
            .file_type()
            .is_symlink());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    /// Fix review 4 #3: without hard links the part is renamed into place, never replacing —
    /// the copy, which a crash could leave half written, only where no such rename exists.
    #[test]
    fn without_hard_links_the_part_is_renamed_without_replacing() {
        let d = dir("rename");
        let (part, path) = (d.join("m.wav.part"), d.join("m.wav"));
        std::fs::write(&part, b"the export").unwrap();
        let no_links = |_: &Path, _: &Path| Err(std::io::Error::other("Incorrect function"));
        let renamed = std::cell::Cell::new(0);
        let rename = |a: &Path, b: &Path| {
            renamed.set(renamed.get() + 1);
            cantus_rt::place::rename_no_replace(a, b)
        };
        place_new_with(&part, &path, no_links, rename).unwrap();
        assert_eq!(renamed.get(), 1, "the rename, not a copy");
        assert_eq!(std::fs::read(&path).unwrap(), b"the export");
        assert!(!part.exists());
        std::fs::write(&part, b"again").unwrap();
        let e = place_new_with(&part, &path, no_links, rename).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"the export",
            "never replaced"
        );
        assert_eq!(std::fs::read(&part).unwrap(), b"again", "nor copied over");
        let _ = std::fs::remove_dir_all(&d);
    }
}
