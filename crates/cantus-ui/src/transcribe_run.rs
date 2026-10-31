//! A conversion from the dialog, turned each frame: the model installed if it must be
//! (its licence accepted by the click that asked for it, shown above the button), the
//! clip's audio written on a worker, the helper's job started and followed, and its notes
//! executed as one undo step. One conversion at a time; *Stop* ends any stage.

use super::{job_args, plan, source, write_input, Settings, Source, DRUM_MODEL, NOTE_MODEL};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::ml_service::MlJob;
use cantus_content::models::ModelStatus;
use cantus_core::{ClipId, TrackId};
use cantus_ml::client::{Outcome, Update};
use cantus_ml::transcribe::spec::{Mode, TranscribeResult, Transcription};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

/// The open dialog: which clip, and the choices.
#[derive(Clone, Debug)]
pub struct Dialog {
    pub track: TrackId,
    pub clip: ClipId,
    pub settings: Settings,
    /// Set when it opens: the clip's context menu behind it is closed.
    pub fresh: bool,
    /// The project it was opened on (`CantusApp::epoch`): another one open, it closes.
    pub epoch: u64,
}

/// Where a running conversion is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Downloading or preparing the model.
    Installing,
    /// Writing the clip's audio for the model.
    Reading,
    /// HT-Demucs takes the drums out of the mix.
    Separating,
    /// The model listens.
    Listening,
}

impl Stage {
    pub fn key(self) -> &'static str {
        match self {
            Stage::Installing => "transcribe.stage.install",
            Stage::Reading => "transcribe.stage.read",
            Stage::Separating => "transcribe.stage.separate",
            Stage::Listening => "transcribe.stage.listen",
        }
    }
}

pub struct Running {
    pub src: Source,
    pub settings: Settings,
    pub stage: Stage,
    /// 0…1 within the stage.
    pub done: f32,
    /// Its folder in the cache, shared with the worker that writes into it: removed when
    /// both have let go.
    dir: Arc<super::WorkDir>,
    /// Set by *Stop*: the worker stops between blocks.
    cancel: Arc<AtomicBool>,
    reading: Option<Receiver<Result<(), String>>>,
    /// Melody mode's own tracker, on a worker: the notes straight away.
    local: Option<Receiver<Result<Transcription, String>>>,
    job: Option<MlJob>,
    /// The model being installed for it.
    installing: Option<&'static str>,
    /// The project it was started on (`CantusApp::epoch`): another one open, it stops and
    /// nothing is made.
    epoch: u64,
}

#[derive(Default)]
pub struct State {
    pub dialog: Option<Dialog>,
    pub running: Option<Running>,
    /// The last conversion's end: what was made, or why not.
    pub last: Option<Result<String, String>>,
    next: u64,
    /// The window, for the workers to wake when they finish (a frame need not be drawn).
    pub repaint: Option<egui::Context>,
}

impl State {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }
}

/// Open the dialog on `clip` (its choices as last time).
pub fn open(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    let settings = app
        .transcribe
        .dialog
        .as_ref()
        .map(|d| d.settings.clone())
        .unwrap_or_else(|| Settings {
            classes: super::default_classes(app.ml().models().ready(DRUM_MODEL).is_some()),
            ..Settings::default()
        });
    app.transcribe.last = None;
    app.transcribe.dialog = Some(Dialog {
        track,
        clip,
        settings,
        fresh: true,
        epoch: app.epoch,
    });
}

pub fn close(app: &mut CantusApp) {
    if !app.transcribe.busy() {
        app.transcribe.dialog = None;
    }
}

/// The model a mode must have before it runs (melody is Cantus's own tracker; drums run
/// without one).
pub fn needed(mode: Mode) -> Option<&'static str> {
    (mode == Mode::Polyphonic).then_some(NOTE_MODEL)
}

/// *Convert*: start, installing the model first when it must be (the click accepts the
/// licence the dialog shows beside it).
pub fn start(app: &mut CantusApp) {
    let Some(d) = app.transcribe.dialog.clone() else {
        return;
    };
    if app.transcribe.busy() {
        return;
    }
    let src = match source(app, d.track, d.clip) {
        Ok(s) => s,
        Err(e) => {
            app.transcribe.last = Some(Err(e));
            return;
        }
    };
    app.transcribe.next += 1;
    // Folders a Cantus that ended mid-conversion left behind go first.
    super::sweep();
    let dir = match super::WorkDir::new(app.transcribe.next) {
        Ok(d) => Arc::new(d),
        Err(e) => {
            app.transcribe.last = Some(Err(e));
            return;
        }
    };
    let mut r = Running {
        src,
        settings: d.settings.clone(),
        stage: Stage::Reading,
        done: 0.0,
        dir,
        cancel: Arc::new(AtomicBool::new(false)),
        reading: None,
        local: None,
        job: None,
        installing: None,
        epoch: app.epoch,
    };
    app.transcribe.last = None;
    if let Some(id) = needed(d.settings.mode) {
        if app.ml().models().ready(id).is_none() {
            if let Err(e) = install(app, id) {
                app.transcribe.last = Some(Err(e));
                return;
            }
            r.stage = Stage::Installing;
            r.installing = Some(id);
            app.transcribe.running = Some(r);
            return;
        }
    }
    begin_reading(&mut r, app.transcribe.repaint.clone());
    app.transcribe.running = Some(r);
}

/// Accept `id`'s licence (as the dialog shows it) and install it.
pub fn install(app: &mut CantusApp, id: &str) -> Result<(), String> {
    let models = app.ml_mut().models_mut();
    if !models.accepted(id) {
        models.accept(id).map_err(|e| e.to_string())?;
    }
    models.install(id)
}

fn begin_reading(r: &mut Running, wake: Option<egui::Context>) {
    r.stage = Stage::Reading;
    r.done = 0.0;
    if r.settings.mode == Mode::Melody {
        let (tx, rx) = channel();
        let (src, settings, cancel) = (r.src.clone(), r.settings.clone(), r.cancel.clone());
        let spawned = std::thread::Builder::new()
            .name("cantus-transcribe-melody".into())
            .spawn({
                let wake = wake.clone();
                move || {
                    let _ = tx.send(super::melody(&src, &settings, &|| {
                        cancel.load(Ordering::Relaxed)
                    }));
                    if let Some(w) = wake {
                        w.request_repaint();
                    }
                }
            });
        if spawned.is_err() {
            let (tx, rx2) = channel();
            let _ = tx.send(Err(t("transcribe.err.read")));
            r.local = Some(rx2);
        } else {
            r.local = Some(rx);
        }
        return;
    }
    let (tx, rx) = channel();
    let (src, rate, dir, cancel) = (
        r.src.clone(),
        super::input_rate(r.settings.mode),
        r.dir.clone(),
        r.cancel.clone(),
    );
    let spawned = std::thread::Builder::new()
        .name("cantus-transcribe-read".into())
        .spawn(move || {
            let _ = tx.send(write_input(&src, rate, &dir.join("input.wav"), &|| {
                cancel.load(Ordering::Relaxed)
            }));
            if let Some(w) = wake {
                w.request_repaint();
            }
        });
    match spawned {
        Ok(_) => {
            r.stage = Stage::Reading;
            r.done = 0.0;
            r.reading = Some(rx);
        }
        Err(e) => {
            let (tx, rx) = channel();
            let _ = tx.send(Err(e.to_string()));
            r.reading = Some(rx);
        }
    }
}

/// *Stop*.
pub fn stop(app: &mut CantusApp) {
    let Some(r) = app.transcribe.running.take() else {
        return;
    };
    if let Some(j) = &r.job {
        app.ml().cancel(j.id);
        app.ml().forget(j.id);
    }
    if let Some(id) = r.installing {
        app.ml_mut().models_mut().cancel(id);
    }
    // The worker stops between blocks and lets go of the folder, which then goes.
    r.cancel.store(true, Ordering::Relaxed);
    drop(r);
    app.transcribe.last = Some(Err(t("transcribe.stopped")));
}

/// Each frame: move the running conversion on.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if app.transcribe.repaint.is_none() {
        app.transcribe.repaint = Some(ctx.clone());
    }
    // Another project open: the dialog closes and a conversion stops, nothing made.
    if app
        .transcribe
        .dialog
        .as_ref()
        .is_some_and(|d| d.epoch != app.epoch)
    {
        app.transcribe.dialog = None;
    }
    if app
        .transcribe
        .running
        .as_ref()
        .is_some_and(|r| r.epoch != app.epoch)
    {
        stop(app);
        app.transcribe.last = None;
        return;
    }
    let Some(mut r) = app.transcribe.running.take() else {
        return;
    };
    ctx.request_repaint_after(std::time::Duration::from_millis(60));
    let end = |app: &mut CantusApp, r: &Running, outcome: Result<String, String>| {
        r.cancel.store(true, Ordering::Relaxed);
        if let Ok(m) = &outcome {
            app.set_status(m.clone());
            app.transcribe.dialog = None;
        }
        app.transcribe.last = Some(outcome);
    };
    match r.stage {
        Stage::Installing => {
            let id = r.installing.unwrap_or(NOTE_MODEL);
            match app.ml().models().status(id) {
                ModelStatus::Ready => {
                    r.installing = None;
                    begin_reading(&mut r, app.transcribe.repaint.clone());
                }
                ModelStatus::Failed(e) => return end(app, &r, Err(e)),
                ModelStatus::Downloading { done, total } => {
                    r.done = done as f32 / total.max(1) as f32;
                }
                ModelStatus::Unavailable => {
                    return end(app, &r, Err(t("transcribe.err.unavailable")))
                }
                _ => {}
            }
        }
        Stage::Reading if r.local.is_some() => {
            let got = r.local.as_ref().map(|rx| rx.try_recv());
            match got {
                Some(Ok(Ok(tr))) => {
                    let outcome = apply(app, &r, &tr);
                    return end(app, &r, outcome);
                }
                Some(Ok(Err(e))) => return end(app, &r, Err(e)),
                Some(Err(TryRecvError::Empty)) => {}
                _ => return end(app, &r, Err(t("transcribe.err.read"))),
            }
        }
        Stage::Reading => {
            let got = r.reading.as_ref().map(|rx| rx.try_recv());
            match got {
                Some(Ok(Ok(()))) => {
                    r.reading = None;
                    let input = r.dir.join("input.wav");
                    let sep = (r.settings.mode == Mode::Drums && r.settings.separate)
                        .then(|| {
                            super::separate_args(
                                app.ml().models(),
                                input.clone(),
                                r.dir.join("stems"),
                            )
                        })
                        .flatten();
                    if let Some(a) = sep {
                        let v = serde_json::to_value(&a).unwrap_or_default();
                        r.job = Some(app.ml().start("separate", v));
                        r.stage = Stage::Separating;
                        r.done = 0.0;
                        app.transcribe.running = Some(r);
                        return;
                    }
                    let out = r.dir.join("notes.json");
                    match job_args(app.ml().models(), &r.settings, input, out) {
                        Ok(args) => {
                            let v = serde_json::to_value(&args).unwrap_or_default();
                            r.job = Some(app.ml().start("transcribe", v));
                            r.stage = Stage::Listening;
                            r.done = 0.0;
                        }
                        Err(e) => return end(app, &r, Err(e)),
                    }
                }
                Some(Ok(Err(e))) => return end(app, &r, Err(e)),
                Some(Err(TryRecvError::Disconnected)) | None => {
                    return end(app, &r, Err(t("transcribe.err.read")))
                }
                Some(Err(TryRecvError::Empty)) => {}
            }
        }
        Stage::Separating => {
            let mut ended = None;
            if let Some(j) = &r.job {
                for u in j.updates.try_iter() {
                    match u {
                        Update::Progress { done, .. } => r.done = done,
                        Update::Ended(_, o) => ended = Some(o),
                        _ => {}
                    }
                }
            }
            if ended.is_some() {
                // Whatever its end, the job's state goes (review 1, finding 10).
                if let Some(j) = r.job.take() {
                    app.ml().forget(j.id);
                }
            }
            match ended {
                Some(Outcome::Done(v)) => {
                    let drums = serde_json::from_value::<cantus_ml::spec::SeparateResult>(v)
                        .ok()
                        .and_then(|s| s.stems.into_iter().find(|f| f.name == "drums"))
                        .map(|f| f.path);
                    let Some(input) = drums else {
                        return end(app, &r, Err(t("transcribe.err.read")));
                    };
                    match job_args(
                        app.ml().models(),
                        &r.settings,
                        input,
                        r.dir.join("notes.json"),
                    ) {
                        Ok(args) => {
                            let v = serde_json::to_value(&args).unwrap_or_default();
                            r.job = Some(app.ml().start("transcribe", v));
                            r.stage = Stage::Listening;
                            r.done = 0.0;
                        }
                        Err(e) => return end(app, &r, Err(e)),
                    }
                }
                Some(Outcome::Cancelled) => return end(app, &r, Err(t("transcribe.stopped"))),
                Some(Outcome::Failed(f)) => {
                    return end(app, &r, Err(tf("transcribe.err.failed", &[("why", &f)])))
                }
                None => {}
            }
        }
        Stage::Listening => {
            let mut ended = None;
            if let Some(j) = &r.job {
                for u in j.updates.try_iter() {
                    match u {
                        Update::Progress { done, .. } => r.done = done,
                        Update::Ended(_, o) => ended = Some(o),
                        _ => {}
                    }
                }
            }
            if let Some(o) = ended {
                if let Some(j) = &r.job {
                    app.ml().forget(j.id);
                }
                let outcome = match o {
                    Outcome::Done(v) => finish(app, &r, v),
                    Outcome::Cancelled => Err(t("transcribe.stopped")),
                    Outcome::Failed(f) => Err(tf("transcribe.err.failed", &[("why", &f)])),
                };
                return end(app, &r, outcome);
            }
        }
    }
    app.transcribe.running = Some(r);
}

/// The job's notes into the project: one undo step.
fn finish(app: &mut CantusApp, r: &Running, v: serde_json::Value) -> Result<String, String> {
    let res: TranscribeResult = serde_json::from_value(v).map_err(|e| e.to_string())?;
    let t_: Transcription =
        serde_json::from_slice(&std::fs::read(&res.file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    apply(app, r, &t_)
}

/// Notes into the project: one undo step, the new clip selected.
fn apply(app: &mut CantusApp, r: &Running, t_: &Transcription) -> Result<String, String> {
    let p = plan(&mut app.bus, &r.src, &r.settings, t_)?;
    if p.notes == 0 {
        return Err(t("transcribe.err.nothing"));
    }
    if !app.exec(p.command) {
        return Err(t("transcribe.err.exec"));
    }
    app.select_clip(p.track, p.clip);
    Ok(tf(
        "transcribe.done",
        &[("n", &p.notes), ("name", &r.src.clip_name)],
    ))
}

/// The dialog as it looks mid-conversion, for the screenshots (`examples/shoot_transcribe.rs`):
/// a conversion at `stage`, `done` through it, that does nothing.
#[doc(hidden)]
pub fn pretend_running(app: &mut CantusApp, stage: Stage, done: f32) {
    let Some(d) = app.transcribe.dialog.clone() else {
        return;
    };
    let Ok(src) = source(app, d.track, d.clip) else {
        return;
    };
    app.transcribe.running = Some(Running {
        src,
        settings: d.settings,
        stage,
        done,
        dir: match super::WorkDir::new(u64::MAX) {
            Ok(d) => Arc::new(d),
            Err(_) => return,
        },
        cancel: Arc::new(AtomicBool::new(false)),
        reading: None,
        local: None,
        job: None,
        installing: None,
        epoch: app.epoch,
    });
}
