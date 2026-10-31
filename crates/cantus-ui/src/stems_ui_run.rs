//! A separation under way: for each source file, converted to the model's rate on a worker
//! (when it is not at it already), separated by the helper, converted back; then the
//! batch. Driven a step a frame by [`step`]; nothing here blocks the interface.

use super::convert::Frames as _;
use super::{convert, Card, Separated, Stem, Target};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_content::models::ReadyModel;
use cantus_core::AudioSourceId;
use cantus_ml::spec::{
    Device, Family, FileCheck, ModelSpec, RuntimeSpec, SeparateArgs, SeparateResult,
};
use cantus_ml::{Outcome, Update};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The rate HT-Demucs was trained at.
pub const MODEL_RATE: u32 = 44_100;

/// Where a run is.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Converting source `n` for the model.
    Preparing(usize),
    /// The helper separating source `n`.
    Separating(usize),
    /// Converting source `n`'s stems back.
    Returning(usize),
    Done,
    Failed(String),
    Cancelled,
}

/// One source file to separate.
pub struct Source {
    pub id: AudioSourceId,
    pub file: PathBuf,
    /// The folder its stems go to.
    pub out: PathBuf,
    /// Its rate and length, as decoded.
    pub rate: u32,
    pub frames: u64,
    stems: Vec<Stem>,
    /// The model's input when it had to be converted (removed once separated).
    converted: Option<PathBuf>,
}

enum Work {
    None,
    Prep(Receiver<Result<(PathBuf, u32, u64), String>>),
    Job(crate::ml_service::MlJob),
    Back(Receiver<Result<Vec<Stem>, String>>),
}

pub struct Run {
    /// "“Song.wav” on “Mix”".
    pub words: String,
    pub target: Target,
    pub clips: Vec<crate::clips::ClipRef>,
    pub model: ReadyModel,
    pub shifts: u32,
    pub device: Device,
    pub sources: Vec<Source>,
    pub phase: Phase,
    /// 0…1 over the whole run.
    pub done: f32,
    cancel: Arc<AtomicBool>,
    work: Work,
    scratch: PathBuf,
    /// The project it was started on (`CantusApp::epoch`): another one open, it is
    /// cancelled and never lands there (review 2 #4).
    epoch: u64,
    /// The window, woken when a worker answers, so the run moves on while nothing else
    /// asks for a frame (review 2 #5).
    wake: Option<egui::Context>,
    /// Run each time a worker asks whether it is cancelled ([`Run::hold_workers`]).
    gate: Option<Gate>,
}

/// What a worker runs before it asks whether it is cancelled.
pub type Gate = Arc<dyn Fn() + Send + Sync>;

impl Run {
    /// Have the workers run `gate` each time they ask whether they are cancelled: the
    /// tests hold a worker at a known point with it.
    #[doc(hidden)]
    pub fn hold_workers(&mut self, gate: Gate) {
        self.gate = Some(gate);
    }

    pub fn running(&self) -> bool {
        !matches!(
            self.phase,
            Phase::Done | Phase::Failed(_) | Phase::Cancelled
        )
    }

    fn share(&self, i: usize, within: f32, stage: usize) -> f32 {
        // Per source: 5 % to convert, 90 % to separate, 5 % back.
        let n = self.sources.len().max(1) as f32;
        let at = [0.0, 0.05, 0.95][stage] + within * [0.05, 0.9, 0.05][stage];
        (i as f32 + at) / n
    }
}

/// A folder of its own under `dir` for `name`'s stems (`name`, `name 2`, …).
fn fresh_dir(dir: &Path, name: &str) -> PathBuf {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.').to_string();
    let base = if clean.is_empty() {
        "stems".to_string()
    } else {
        clean
    };
    (1..)
        .map(|n| {
            if n == 1 {
                dir.join(&base)
            } else {
                dir.join(format!("{base} {n}"))
            }
        })
        .find(|p| !p.exists())
        .expect("a free name")
}

/// Start separating `target` with `model` (already installed and ready).
pub fn start(app: &mut CantusApp, target: Target, model: ReadyModel) -> Result<(), String> {
    let p = app.project().clone();
    let clips = target.clips(&p);
    if clips.is_empty() {
        return Err(t("stems.status.no_audio"));
    }
    if target.spans_tracks(&p) {
        return Err(t("stems.status.one_track"));
    }
    let sources = sources_for(app, &clips, &model)?;
    let st = &app.settings.stems;
    let shifts = if st.best { super::BEST_SHIFTS } else { 0 };
    let scratch = crate::app::data_dir().join("cache").join("stems");
    if let Err(e) = check_space(&sources, &scratch, model.sources.len(), shifts) {
        for s in &sources {
            let _ = std::fs::remove_dir_all(&s.out);
        }
        return Err(e);
    }
    let device = super::device(app, &model.id);
    let words = target.words(&p);
    app.stems.runs.retain(|r| r.running());
    app.stems.runs.push(Run {
        words,
        target,
        clips,
        model,
        shifts,
        device,
        sources,
        phase: Phase::Preparing(0),
        done: 0.0,
        cancel: Arc::new(AtomicBool::new(false)),
        work: Work::None,
        scratch,
        epoch: app.epoch,
        wake: app.stems.repaint.clone(),
        gate: None,
    });
    Ok(())
}

/// Cancel run `i`: its job stopped, what it wrote removed.
pub fn cancel(app: &mut CantusApp, i: usize) {
    let Some(r) = app.stems.runs.get_mut(i) else {
        return;
    };
    r.cancel.store(true, Ordering::Relaxed);
    if let Work::Job(j) = &r.work {
        app.ml.cancel(j.id);
    }
}

/// The model's 44.1 kHz input for `file`: the file itself when it is a WAV at that rate,
/// else converted to `scratch` a block at a time (review 2 #8: a WAV is read in pieces;
/// another format is decoded whole, then converted and written in pieces). With its own
/// rate and length. `cancelled` is asked between blocks.
pub fn prepare(
    file: &Path,
    scratch: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(PathBuf, u32, u64), String> {
    if let Ok(r) = hound::WavReader::open(file) {
        let s = r.spec();
        if s.sample_rate == MODEL_RATE && s.channels <= 2 {
            return Ok((file.to_path_buf(), MODEL_RATE, r.duration() as u64));
        }
    }
    if cancelled() {
        return Err("cancelled".into());
    }
    std::fs::create_dir_all(scratch).map_err(|e| e.to_string())?;
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let out = scratch.join(format!(
        "in-{}-{}.wav",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let to_model = |src: &mut dyn convert::Frames, rate: u32| -> Result<u64, String> {
        let frames = src.frames();
        let n = cantus_engine::samples::resample::converted_len(frames as usize, rate, MODEL_RATE);
        if convert::wav_bytes(n as u64, src.channels() as u64) > convert::WAV_LIMIT {
            return Err(t("stems.error.too_long_input"));
        }
        let mut o = convert::Out::create(&out, src.channels(), MODEL_RATE)?;
        convert::convert(src, rate, MODEL_RATE, None, &mut o, cancelled)?;
        o.finish()?;
        Ok(frames)
    };
    let done = match convert::Wav::open(file) {
        Ok((mut w, rate)) => to_model(&mut w, rate).map(|f| (rate, f)),
        Err(_) => cantus_engine::samples::assets::decode_any(file).and_then(|d| {
            let mut m = convert::Memory {
                data: &d.frames,
                channels: d.channels.max(1) as usize,
            };
            to_model(&mut m, d.rate).map(|f| (d.rate, f))
        }),
    };
    match done {
        Ok((rate, frames)) => Ok((out, rate, frames)),
        Err(e) => {
            let _ = std::fs::remove_file(&out);
            Err(e)
        }
    }
}

/// The stems back at the source's `rate` and exactly `frames` long, in place, a block at
/// a time (review 2 #8). `cancelled` is asked between blocks.
pub fn back(
    stems: Vec<Stem>,
    rate: u32,
    frames: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Stem>, String> {
    let mut out = Vec::new();
    for s in stems {
        if cancelled() {
            return Err("cancelled".into());
        }
        if s.rate == rate && s.frames == frames {
            out.push(s);
            continue;
        }
        if convert::wav_bytes(frames, 2) > convert::WAV_LIMIT {
            return Err(t("stems.error.too_long_stems"));
        }
        let tmp = s.path.with_extension("back.part");
        let done = (|| {
            let (mut w, from) = convert::Wav::open(&s.path)?;
            let mut o = convert::Out::create(&tmp, w.channels(), rate)?;
            convert::convert(&mut w, from, rate, Some(frames), &mut o, cancelled)?;
            o.finish()
        })()
        .and_then(|_| std::fs::rename(&tmp, &s.path).map_err(|e| e.to_string()));
        if let Err(e) = done {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        out.push(Stem { rate, frames, ..s });
    }
    Ok(out)
}

fn args(r: &Run, input: &Path, out: &Path) -> SeparateArgs {
    SeparateArgs {
        runtime: RuntimeSpec {
            library: r.model.library.clone(),
            threads: 0,
            device: r.device,
            sha256: Some(r.model.library_sha256.clone()),
        },
        model: ModelSpec {
            graph: r.model.graph.clone(),
            // "fixture": the tests' stand-in network (the helper takes it only when started
            // for tests); every part a share of the time branch.
            family: if r.model.family == "fixture" {
                let n = r.model.sources.len().max(1);
                Family::Fixture {
                    spec: vec![0.0; n],
                    wave: vec![1.0 / n as f32; n],
                    delay_ms: 0,
                    crash_at: None,
                    panic_at: None,
                    hang_at: None,
                }
            } else {
                Family::HtDemucs
            },
            sources: r.model.sources.clone(),
            sample_rate: r.model.sample_rate,
            segment: r.model.segment,
            check: r
                .model
                .files
                .iter()
                .map(|(path, sha256)| FileCheck {
                    path: path.clone(),
                    sha256: sha256.clone(),
                })
                .collect(),
        },
        input: input.to_path_buf(),
        out_dir: out.to_path_buf(),
        overlap: 0.25,
        shifts: r.shifts,
    }
}

/// Move every run on by what has happened since the last frame. Another project open
/// since a run started cancels it, and closes the panel (review 2 #4).
pub fn step(app: &mut CantusApp) {
    super::models::tend(app);
    settle_pending(app);
    if app.stems.dialog.is_some() && app.stems.dialog_epoch != app.epoch {
        app.stems.dialog = None;
    }
    for i in 0..app.stems.runs.len() {
        let r = &app.stems.runs[i];
        if r.running() && r.epoch != app.epoch && !r.cancel.load(Ordering::Relaxed) {
            cancel(app, i);
        }
        step_one(app, i);
    }
}

fn fail(app: &mut CantusApp, i: usize, why: String) {
    let r = &mut app.stems.runs[i];
    for s in &mut r.sources {
        let _ = std::fs::remove_dir_all(&s.out);
        if let Some(c) = s.converted.take() {
            let _ = std::fs::remove_file(c);
        }
    }
    let cancelled = r.cancel.load(Ordering::Relaxed);
    r.phase = if cancelled {
        Phase::Cancelled
    } else {
        Phase::Failed(why.clone())
    };
    r.work = Work::None;
    let words = r.words.clone();
    if cancelled {
        app.set_status(tf("stems.status.cancelled", &[("target", &words)]));
    } else {
        app.set_error(tf(
            "stems.status.failed",
            &[("target", &words), ("why", &why)],
        ));
    }
}

fn step_one(app: &mut CantusApp, i: usize) {
    let r = &mut app.stems.runs[i];
    if !r.running() {
        return;
    }
    // A cancel: the job's end says when it stopped; a worker's answer says when it let
    // its files go — only then is what it wrote removed (review 2 #6).
    if r.cancel.load(Ordering::Relaxed) {
        let n = match r.phase {
            Phase::Preparing(n) | Phase::Separating(n) | Phase::Returning(n) => n,
            _ => 0,
        };
        match &r.work {
            Work::Job(_) => {}
            Work::Prep(rx) => match rx.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Ok(Ok((input, _, _))) => {
                    if input != r.sources[n].file {
                        r.sources[n].converted = Some(input);
                    }
                    fail(app, i, "cancelled".into());
                    return;
                }
                _ => {
                    fail(app, i, "cancelled".into());
                    return;
                }
            },
            Work::Back(rx) => {
                if let Err(std::sync::mpsc::TryRecvError::Empty) = rx.try_recv() {
                    return;
                }
                fail(app, i, "cancelled".into());
                return;
            }
            Work::None => {
                fail(app, i, "cancelled".into());
                return;
            }
        }
    }
    match r.phase.clone() {
        Phase::Preparing(n) => match &r.work {
            Work::None => {
                let (file, scratch, cancel, wake, gate) = (
                    r.sources[n].file.clone(),
                    r.scratch.clone(),
                    r.cancel.clone(),
                    r.wake.clone(),
                    r.gate.clone(),
                );
                let (tx, rx) = channel();
                let _ = std::thread::Builder::new()
                    .name("cantus-stems-prep".into())
                    .spawn(move || {
                        let asked = || {
                            if let Some(g) = &gate {
                                g();
                            }
                            cancel.load(Ordering::Relaxed)
                        };
                        let _ = tx.send(prepare(&file, &scratch, &asked));
                        if let Some(w) = wake {
                            w.request_repaint();
                        }
                    });
                r.work = Work::Prep(rx);
            }
            Work::Prep(rx) => match rx.try_recv() {
                Ok(Ok((input, rate, frames))) => {
                    r.sources[n].rate = rate;
                    r.sources[n].frames = frames;
                    if input != r.sources[n].file {
                        r.sources[n].converted = Some(input.clone());
                    }
                    let a = args(r, &input, &r.sources[n].out.clone());
                    let job = app
                        .ml
                        .start("separate", serde_json::to_value(&a).unwrap_or_default());
                    let r = &mut app.stems.runs[i];
                    r.work = Work::Job(job);
                    r.phase = Phase::Separating(n);
                    r.done = r.share(n, 0.0, 1);
                }
                Ok(Err(e)) => fail(app, i, e),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    fail(app, i, t("stems.error.worker"))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            },
            _ => {}
        },
        Phase::Separating(n) => {
            let Work::Job(job) = &r.work else {
                return;
            };
            let mut end = None;
            while let Ok(u) = job.updates.try_recv() {
                match u {
                    Update::Progress { done, .. } => r.done = r.share(n, done, 1),
                    Update::Ended(_, o) => end = Some(o),
                    _ => {}
                }
            }
            if end.is_some() {
                if let Some(c) = r.sources[n].converted.take() {
                    let _ = std::fs::remove_file(c);
                }
            }
            match end {
                None => {}
                Some(Outcome::Done(v)) => {
                    let res: Result<SeparateResult, _> = serde_json::from_value(v);
                    let Ok(res) = res else {
                        fail(app, i, t("stems.error.answer"));
                        return;
                    };
                    let stems: Vec<Stem> = res
                        .stems
                        .into_iter()
                        .map(|s| Stem {
                            part: s.name,
                            path: s.path,
                            frames: res.frames,
                            rate: res.sample_rate,
                        })
                        .collect();
                    let (rate, frames, cancel, wake, gate) = (
                        r.sources[n].rate,
                        r.sources[n].frames,
                        r.cancel.clone(),
                        r.wake.clone(),
                        r.gate.clone(),
                    );
                    let (tx, rx) = channel();
                    let _ = std::thread::Builder::new()
                        .name("cantus-stems-back".into())
                        .spawn(move || {
                            let asked = || {
                                if let Some(g) = &gate {
                                    g();
                                }
                                cancel.load(Ordering::Relaxed)
                            };
                            let _ = tx.send(back(stems, rate, frames, &asked));
                            if let Some(w) = wake {
                                w.request_repaint();
                            }
                        });
                    r.work = Work::Back(rx);
                    r.phase = Phase::Returning(n);
                    r.done = r.share(n, 0.0, 2);
                }
                Some(Outcome::Cancelled) => {
                    r.cancel.store(true, Ordering::Relaxed);
                    fail(app, i, "cancelled".into());
                }
                Some(Outcome::Failed(f)) => fail(app, i, f.to_string()),
            }
        }
        Phase::Returning(n) => {
            let Work::Back(rx) = &r.work else {
                return;
            };
            match rx.try_recv() {
                Ok(Ok(stems)) => {
                    r.sources[n].stems = stems;
                    r.work = Work::None;
                    if n + 1 < r.sources.len() {
                        r.phase = Phase::Preparing(n + 1);
                        r.done = r.share(n + 1, 0.0, 0);
                    } else {
                        commit(app, i);
                    }
                }
                Ok(Err(e)) => fail(app, i, e),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    fail(app, i, t("stems.error.worker"))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        Phase::Done | Phase::Failed(_) | Phase::Cancelled => {}
    }
}

/// The batch, now that every source is separated — into the project it was started on
/// only.
fn commit(app: &mut CantusApp, i: usize) {
    let r = &app.stems.runs[i];
    if r.epoch != app.epoch {
        app.stems.runs[i].cancel.store(true, Ordering::Relaxed);
        fail(app, i, "cancelled".into());
        return;
    }
    let results: Vec<Separated> = r
        .sources
        .iter()
        .map(|s| Separated {
            source: s.id,
            stems: s.stems.clone(),
        })
        .collect();
    let card = Card {
        id: r.model.id.clone(),
        name: r.model.name.clone(),
        version: r.model.version,
        licence: r.model.licence.clone(),
    };
    // Still the clips it was asked for (an edit meanwhile may have removed one).
    let clips: Vec<_> = r
        .clips
        .iter()
        .copied()
        .filter(|c| crate::audio_clips::region(app.project(), *c).is_some())
        .collect();
    let words = r.words.clone();
    let base = app.audio_base().map(Path::to_path_buf);
    let bus_name = r.target.bus_name(app.project());
    let part = |p: &str| part_name(p);
    let planned = super::plan(
        &mut app.bus,
        &clips,
        &results,
        &card,
        base.as_deref(),
        &super::today(),
        &part,
        &bus_name,
    );
    match planned {
        Ok(pl) => {
            let first = pl.tracks.first().copied();
            if app.exec(pl.command) {
                if let Some(t0) = first {
                    app.select_track(t0);
                }
                app.set_status(tf(
                    "stems.status.done",
                    &[("target", &words), ("n", &pl.tracks.len())],
                ));
                let r = &mut app.stems.runs[i];
                r.phase = Phase::Done;
                r.done = 1.0;
            } else {
                fail(app, i, t("stems.error.refused"));
            }
        }
        Err(e) => fail(app, i, e),
    }
}

/// A part's words (`stems.part.vocals`), or its name when the model has one Cantus does not
/// know.
pub fn part_name(p: &str) -> String {
    let key = format!("stems.part.{p}");
    let w = t(&key);
    if w == key {
        p.to_string()
    } else {
        w
    }
}

/// Whether a file of `frames` at `rate` is past what one WAV holds, as the model's 44.1 kHz
/// stereo input or as a stereo stem at its own rate.
pub fn too_long(frames: u64, rate: u32) -> bool {
    let input = cantus_engine::samples::resample::converted_len(frames as usize, rate, MODEL_RATE);
    convert::wav_bytes(input as u64, 2) > convert::WAV_LIMIT
        || convert::wav_bytes(frames, 2) > convert::WAV_LIMIT
}

/// What separating `sources` into `parts` writes at its peak, by folder: the converted
/// inputs in `scratch`; in each source's folder the stems (with *Best*'s passes beside
/// them until they are averaged) and a stem brought back to the file's rate.
pub fn space_needed(
    sources: &[Source],
    scratch: &Path,
    parts: usize,
    shifts: u32,
) -> Vec<(PathBuf, u64)> {
    let passes = shifts.max(1) as u64;
    let parts = parts as u64;
    let mut out = vec![(scratch.to_path_buf(), 0u64)];
    for s in sources {
        let n44 =
            cantus_engine::samples::resample::converted_len(s.frames as usize, s.rate, MODEL_RATE)
                as u64;
        let at_model = convert::wav_bytes(n44, 2);
        let at_rate = convert::wav_bytes(s.frames, 2);
        let wav = s
            .file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        if s.rate != MODEL_RATE || !wav {
            out[0].1 += at_model;
        }
        let separated = parts * at_model * (passes + u64::from(passes > 1));
        let back = if s.rate == MODEL_RATE {
            0
        } else {
            (parts * at_rate).max(separated) - separated + at_rate
        };
        out.push((s.out.clone(), separated + back));
    }
    out
}

/// Refused, in words, when a folder's disk lacks what [`space_needed`] says (and 64 MB to
/// spare); `disk` gives a folder's volume and its free bytes. Folders on one volume count
/// together (review 3 #2: known by the volume itself, not by equal free bytes).
pub fn check_space_with(
    sources: &[Source],
    scratch: &Path,
    parts: usize,
    shifts: u32,
    disk: &dyn Fn(&Path) -> Option<(String, u64)>,
) -> Result<(), String> {
    const SPARE: u64 = 64 << 20;
    let needs = space_needed(sources, scratch, parts, shifts);
    let mut disks: Vec<(String, u64, u64, PathBuf)> = Vec::new();
    for (path, need) in needs {
        let Some((volume, free)) = disk(&path) else {
            continue;
        };
        match disks.iter_mut().find(|d| d.0 == volume) {
            Some(d) => d.2 += need,
            None => disks.push((volume, free, need, path)),
        }
    }
    for (_, free, need, path) in disks {
        if need + SPARE > free {
            return Err(tf(
                "stems.error.space",
                &[
                    ("need", &crate::views::samples_content::size(need)),
                    ("free", &crate::views::samples_content::size(free)),
                    ("where", &path.display()),
                ],
            ));
        }
    }
    Ok(())
}

/// [`check_space_with`] the disks' own volumes and free space (review 2 #7).
pub fn check_space(
    sources: &[Source],
    scratch: &Path,
    parts: usize,
    shifts: u32,
) -> Result<(), String> {
    check_space_with(sources, scratch, parts, shifts, &|p| {
        Some((cantus_rt::disk::volume(p)?, cantus_rt::disk::space(p)?.free))
    })
}

/// The source files `clips` play, each with a folder chosen for its stems (the project's
/// audio folder's `Stems/`) — not made yet: [`make_dirs`] makes them, an agent's on its
/// worker (a job denied or never run leaves nothing).
pub fn plan_sources(
    app: &mut CantusApp,
    clips: &[crate::clips::ClipRef],
    model: &ReadyModel,
) -> Result<Vec<Source>, String> {
    let p = app.project().clone();
    let mut ids: Vec<AudioSourceId> = Vec::new();
    for c in clips {
        if let Some((_, a)) = crate::audio_clips::region(&p, *c) {
            if !ids.contains(&a.source) {
                ids.push(a.source);
            }
        }
    }
    // Every check first, so a refusal leaves no folder behind (review 3 #4).
    let mut found = Vec::new();
    for id in ids {
        let s = p.audio_source(id).ok_or("a file is gone")?;
        let file = app.audio_file(s);
        if !file.is_file() {
            return Err(tf("stems.error.missing", &[("file", &file.display())]));
        }
        // Past what a WAV file holds (4 GB) — the model's input at 44.1 kHz, or a stem at
        // the file's rate (an hour at 192 kHz is 5.5 GB): said before anything is done.
        if too_long(s.frames, s.sample_rate) {
            return Err(tf(
                "stems.error.too_long",
                &[("file", &s.name), ("rate", &s.sample_rate)],
            ));
        }
        found.push((id, file, s.name.clone(), s.sample_rate, s.frames));
    }
    let root = app.audio_record_dir().join("Stems");
    let mut out: Vec<Source> = Vec::new();
    for (id, file, name, rate, frames) in found {
        // A folder name no other source here took either; made by [`make_dirs`].
        let mut dir = fresh_dir(&root, &format!("{name} — {}", model.id));
        let mut n = 2;
        while out.iter().any(|s| s.out == dir) {
            dir = fresh_dir(&root, &format!("{name} — {} {n}", model.id));
            n += 1;
        }
        out.push(Source {
            id,
            file,
            out: dir,
            rate,
            frames,
            stems: Vec::new(),
            converted: None,
        });
    }
    Ok(out)
}

/// [`plan_sources`] with the folders made: a panel's run.
pub fn sources_for(
    app: &mut CantusApp,
    clips: &[crate::clips::ClipRef],
    model: &ReadyModel,
) -> Result<Vec<Source>, String> {
    let mut sources = plan_sources(app, clips, model)?;
    make_dirs(&mut sources)?;
    Ok(sources)
}

/// Make the sources' folders, each one new — taken by this run alone: made with
/// `create_dir`, so a folder another run made meanwhile (or made first) is never shared;
/// on `AlreadyExists` the next free name (final review #1) — or none: a failure removes
/// those made.
pub fn make_dirs(sources: &mut [Source]) -> Result<(), String> {
    for i in 0..sources.len() {
        match make_new_dir(&sources[i].out) {
            Ok(dir) => sources[i].out = dir,
            Err(e) => {
                for s in &sources[..i] {
                    let _ = std::fs::remove_dir_all(&s.out);
                }
                return Err(e);
            }
        }
    }
    Ok(())
}

/// `want`, or `want 2`, `want 3`…: the first that this call itself creates.
fn make_new_dir(want: &Path) -> Result<PathBuf, String> {
    let parent = want.parent().ok_or("no folder to make the stems in")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let base = want
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stems".into());
    for n in 1..10_000 {
        let dir = if n == 1 {
            parent.join(&base)
        } else {
            parent.join(format!("{base} {n}"))
        };
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("no free folder name for the stems".into())
}

/// Folders an agent's separation wrote, removed when this is dropped armed: a separation
/// that fails, a job dropped before its edit, an edit that cannot be planned.
pub struct Leftovers {
    dirs: Vec<PathBuf>,
    armed: bool,
}

impl Leftovers {
    pub fn new(dirs: Vec<PathBuf>) -> Leftovers {
        Leftovers { dirs, armed: true }
    }

    /// The edit is planned: its folders wait, as [`Pending`], to see whether it is made.
    pub fn pending(mut self, sources: Vec<AudioSourceId>, epoch: u64) -> Pending {
        self.armed = false;
        Pending {
            dirs: std::mem::take(&mut self.dirs),
            sources,
            epoch,
        }
    }
}

impl Drop for Leftovers {
    fn drop(&mut self) {
        if self.armed {
            for d in &self.dirs {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
}

/// An agent's planned edit of stems, its folders kept until it is known whether the edit
/// was made: the host makes it, asks the user first, or refuses it after the plan
/// (another project, a denied card, a timed-out one) — review of the integrated tree.
pub struct Pending {
    pub dirs: Vec<PathBuf>,
    /// The files the edit adds to the pool: there, the edit was made.
    pub sources: Vec<AudioSourceId>,
    pub epoch: u64,
}

/// Settle the agents' pending edits: one whose files are in the pool was made (its
/// folders stay, for undo and redo too); one in another project, or whose card is no
/// longer waiting (no agent's question pending), never will be — its folders go.
pub fn settle_pending(app: &mut CantusApp) {
    if app.stems.pending.is_empty() {
        return;
    }
    let asking = app.ai.agents().is_some_and(|a| a.asks().next().is_some());
    let epoch = app.epoch;
    let pool: Vec<AudioSourceId> = app.project().audio_pool.iter().map(|s| s.id).collect();
    app.stems.pending.retain(|p| {
        if p.epoch == epoch && p.sources.iter().any(|s| pool.contains(s)) {
            return false;
        }
        if p.epoch == epoch && asking {
            return true;
        }
        for d in &p.dirs {
            let _ = std::fs::remove_dir_all(d);
        }
        false
    });
}

/// Cantus closing: every pending edit settled now — its folders kept when its files are
/// in the pool, else removed — for nothing is left to settle them later (final review #2;
/// called from `close_session`).
pub fn settle_on_exit(app: &mut CantusApp) {
    let pool: Vec<AudioSourceId> = app.project().audio_pool.iter().map(|s| s.id).collect();
    for p in std::mem::take(&mut app.stems.pending) {
        if !p.sources.iter().any(|s| pool.contains(s)) {
            for d in &p.dirs {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
}

/// Whether `name` is a stems folder's as Cantus names them: `<file> _ <model>` (the dash
/// of `<file> — <model>` made safe), maybe with ` 2`, ` 3`… after it.
pub fn is_stems_folder(name: &str, models: &[String]) -> bool {
    let base = match name.rsplit_once(' ') {
        Some((head, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => head,
        _ => name,
    };
    models.iter().any(|m| {
        [format!(" _ {m}"), format!(" — {m}")]
            .iter()
            .any(|tail| base.len() > tail.len() && base.ends_with(tail.as_str()))
    })
}

/// A project opened: its `Audio/Stems/` folders that nothing in it plays and whose files
/// are all older than its last save are what a separation left when Cantus closed before
/// its edit was settled (or crashed): removed. Only folders named as Cantus names them
/// (final review #2).
pub fn sweep_orphans(app: &mut CantusApp) {
    let Some(project_file) = app.path.clone() else {
        return;
    };
    let Some(saved) = std::fs::metadata(&project_file)
        .ok()
        .and_then(|m| m.modified().ok())
    else {
        return;
    };
    let root = app.audio_record_dir().join("Stems");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let models: Vec<String> = app
        .ml
        .models()
        .manifest()
        .models
        .iter()
        .map(|m| m.id.clone())
        .collect();
    let played: Vec<PathBuf> = app
        .project()
        .audio_pool
        .clone()
        .iter()
        .map(|s| app.audio_file(s))
        .collect();
    for e in entries.flatten() {
        let dir = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if !dir.is_dir() || !is_stems_folder(&name, &models) {
            continue;
        }
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let here = canon(&dir);
        if played.iter().any(|f| canon(f).starts_with(&here)) {
            continue;
        }
        let older = std::fs::read_dir(&dir).map(|files| {
            files.flatten().all(|f| {
                f.metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t < saved)
            })
        });
        if older.unwrap_or(false) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// The files a command adds to the pool.
pub fn inserted_sources(c: &cantus_core::Command) -> Vec<AudioSourceId> {
    match c {
        cantus_core::Command::InsertAudioSource { source } => vec![source.id],
        cantus_core::Command::Batch { commands, .. } => {
            commands.iter().flat_map(inserted_sources).collect()
        }
        _ => Vec::new(),
    }
}

/// A converted input, removed when it goes out of scope.
struct Scratch(Option<PathBuf>);

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Every source separated, on this thread (an agent's job's worker): converted, separated
/// by the helper through `ml`, converted back. `progress` 0…1; `cancelled` asked often.
#[allow(clippy::too_many_arguments)]
pub fn separate_blocking(
    ml: &crate::ml_service::MlStarter,
    sources: &[Source],
    model: &ReadyModel,
    shifts: u32,
    device: Device,
    scratch: &Path,
    progress: &dyn Fn(f32),
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Separated>, String> {
    let n = sources.len().max(1) as f32;
    let mut out = Vec::new();
    let stub = Run {
        words: String::new(),
        target: Target::Clips(Vec::new()),
        clips: Vec::new(),
        model: model.clone(),
        shifts,
        device,
        sources: Vec::new(),
        phase: Phase::Done,
        done: 0.0,
        cancel: Arc::new(AtomicBool::new(false)),
        work: Work::None,
        scratch: scratch.to_path_buf(),
        epoch: 0,
        wake: None,
        gate: None,
    };
    for (i, s) in sources.iter().enumerate() {
        // The agent's cancel reaches the conversion too; the converted input goes on every
        // way out (review 2 #6).
        let (input, rate, frames) = prepare(&s.file, scratch, cancelled)?;
        let _converted = Scratch((input != s.file).then(|| input.clone()));
        let a = args(&stub, &input, &s.out);
        let job = ml.start(
            "separate",
            serde_json::to_value(&a).map_err(|e| e.to_string())?,
        );
        let mut asked = false;
        let res = loop {
            if cancelled() && !asked {
                asked = true;
                ml.cancel(job.id);
            }
            match job
                .updates
                .recv_timeout(std::time::Duration::from_millis(100))
            {
                Ok(Update::Progress { done, .. }) => progress((i as f32 + 0.05 + 0.9 * done) / n),
                Ok(Update::Ended(_, Outcome::Done(v))) => {
                    break serde_json::from_value::<SeparateResult>(v).map_err(|e| e.to_string())?
                }
                Ok(Update::Ended(_, Outcome::Cancelled)) => return Err("cancelled".into()),
                Ok(Update::Ended(_, Outcome::Failed(f))) => return Err(f.to_string()),
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(t("stems.error.worker"))
                }
            }
        };
        let stems: Vec<Stem> = res
            .stems
            .into_iter()
            .map(|st| Stem {
                part: st.name,
                path: st.path,
                frames: res.frames,
                rate: res.sample_rate,
            })
            .collect();
        let stems = back(stems, rate, frames, cancelled)?;
        out.push(Separated {
            source: s.id,
            stems,
        });
        progress((i + 1) as f32 / n);
    }
    Ok(out)
}

/// A run as the panel shows it, for the screenshots (`examples/shoot_stems.rs`): no work
/// behind it.
pub fn shown(target: Target, model: ReadyModel, phase: Phase, done: f32, words: String) -> Run {
    Run {
        words,
        target,
        clips: Vec::new(),
        model,
        shifts: 0,
        device: Device::Cpu,
        sources: Vec::new(),
        phase,
        done,
        cancel: Arc::new(AtomicBool::new(false)),
        work: Work::None,
        scratch: PathBuf::new(),
        epoch: 0,
        wake: None,
        gate: None,
    }
}
