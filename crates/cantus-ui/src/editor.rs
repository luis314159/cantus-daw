//! The audio editor (ADR 0096): an Edison-class window opened from an audio clip
//! (*Edit in audio editor*) and left back to it. Every edit is **destructive on the
//! take, never on the file it came from**: it makes a new file named by its content
//! (`edit-<hash>.wav` beside the project's audio), adds it to the pool and points the clip at
//! it — one undo step, the original file still there. The state here is the session's; the
//! heavy work runs on `jobs`' thread; what the window asks is `act`; what reaches the project
//! is `commit`; the drawing is `view`.

use self::jobs::{Done, Job, Loaded, Worker};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::waveform::SourcePeaks;
use cantus_core::audio::FadeCurve;
use cantus_core::audio_edit::{silence, AudioBuf, Joins, Level, Sel};
use cantus_core::{AudioSourceId, ClipId, SourceMarks, TrackId};
use cantus_dsp::denoise::{stft::Spectrogram, NoiseProfile, Params};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[path = "editor_act.rs"]
pub mod act;
#[path = "editor_commit.rs"]
pub mod commit;
#[path = "editor_job.rs"]
pub mod jobs;
#[path = "views/editor.rs"]
pub mod view;
#[path = "views/editor_clean.rs"]
pub mod view_clean;
#[path = "views/editor_tools.rs"]
pub mod view_tools;
#[path = "views/editor_wave.rs"]
pub mod view_wave;

pub use act::{act, action, close, open, open_file, paste_file, play, stop, Act};
pub use commit::{commit_commands, edit_dir, label_key};

/// Longest take the editor opens, in seconds of audio (the picture and the edits are
/// held in memory as 32-bit floats).
pub const MAX_SECONDS: f64 = 1_800.0;

/// What the view shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Show {
    Wave,
    Spectral,
    Both,
}

/// The window on the take: the first frame at the left edge and frames per point.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct View {
    pub start: f64,
    pub per: f64,
}

/// The tools' settings (kept between takes).
#[derive(Clone, Debug)]
pub struct Tools {
    pub fade: FadeCurve,
    pub gain_db: f32,
    pub level: Level,
    pub target_db: f32,
    /// RMS and LUFS normalize stop at this peak (`ceiling_on`).
    pub ceiling_on: bool,
    pub ceiling_db: f32,
    pub silence: silence::Settings,
    pub denoise: Params,
    pub atten_db: f32,
    pub declick: f32,
    /// Joins at zero crossings with a short crossfade (off: raw, sample-exact splices).
    pub smooth_joins: bool,
}

impl Default for Tools {
    fn default() -> Self {
        Tools {
            fade: FadeCurve::Linear,
            gain_db: 3.0,
            level: Level::Peak,
            target_db: -1.0,
            ceiling_on: true,
            ceiling_db: -1.0,
            silence: silence::Settings::default(),
            denoise: Params::default(),
            atten_db: -12.0,
            declick: 0.5,
            smooth_joins: true,
        }
    }
}

impl Tools {
    pub fn joins(&self) -> Joins {
        if self.smooth_joins {
            Joins::default()
        } else {
            Joins::RAW
        }
    }
}

/// A playing audition: the frames played and when it began.
#[derive(Copy, Clone, Debug)]
pub struct Play {
    pub at: Instant,
    pub from: usize,
    pub to: usize,
}

/// A time-frequency rectangle picked on the spectrogram: Hz.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Band {
    pub lo_hz: f32,
    pub hi_hz: f32,
}

/// One take open in the editor.
pub struct Session {
    pub track: TrackId,
    pub clip: ClipId,
    /// The pool file the buffer is: when the clip's source differs (an undo, a redo), the
    /// take is read again.
    pub source: AudioSourceId,
    pub name: String,
    /// The project the take belongs to (`CantusApp::epoch`): another project in its place
    /// closes the window.
    pub epoch: u64,
    pub buf: Option<Arc<AudioBuf>>,
    pub peaks: Option<Arc<SourcePeaks>>,
    pub spec: Option<Arc<Spectrogram>>,
    /// The visible span drawn at its own hop (a zoomed-in view): its first frame and picture.
    pub detail: Option<(usize, Arc<Spectrogram>)>,
    /// The span (start, end, hop) the view would draw in detail; asked of the thread once.
    pub detail_want: Option<(usize, usize, usize)>,
    pub detail_asked: Option<(usize, usize, usize)>,
    pub sel: Option<Sel>,
    pub band: Option<Band>,
    /// The file's markers, regions and loop points as the project keeps them (a mirror,
    /// read each frame; changed through commands).
    pub marks: SourceMarks,
    pub view: View,
    pub fit: bool,
    pub profile: Option<NoiseProfile>,
    pub play: Option<Play>,
    /// What the thread is doing, as a key.
    pub busy: Option<&'static str>,
    pub status: Option<(String, bool)>,
    /// The selection was dragged out of the window: it becomes a clip of its own.
    pub dragged_out: bool,
    pub gen: u64,
}

/// The editor's state in the application.
pub struct EditorState {
    pub session: Option<Session>,
    pub show: Show,
    pub tools: Tools,
    pub clipboard: Option<AudioBuf>,
    /// Files an agent's edit wrote, waiting to see the edit made (else they go).
    pub pending: Vec<commit::Pending>,
    /// Agents' edits being made (a job counts itself from when it is planned to when its
    /// edit is planned back): no file is tidied or swept meanwhile.
    pub agent_jobs: Arc<std::sync::atomic::AtomicUsize>,
    /// *Remove unused audio*: the plan the dialog shows, when open.
    pub cleanup: Option<commit::Cleanup>,
    worker: Option<Worker>,
    gen: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        // When this run began: what tells a cut session's half-written file from one being
        // written now.
        let _ = commit::process_start();
        EditorState {
            session: None,
            show: Show::Both,
            tools: Tools::default(),
            clipboard: None,
            pending: Vec::new(),
            agent_jobs: Arc::default(),
            cleanup: None,
            worker: None,
            gen: 0,
        }
    }
}

impl EditorState {
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    /// Whether the window's thread is at work on the open take.
    pub fn busy(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.busy.is_some())
    }

    fn worker(&mut self) -> &Worker {
        self.worker.get_or_insert_with(Worker::new)
    }

    fn next_gen(&mut self) -> u64 {
        self.gen += 1;
        self.gen
    }
}

pub(crate) fn say(app: &mut CantusApp, text: String, warn: bool) {
    if let Some(s) = app.editor.session.as_mut() {
        s.status = Some((text, warn));
    }
}

/// Read the session's current source again.
pub(crate) fn load(app: &mut CantusApp) {
    let Some(source) = app.editor.session.as_ref().map(|s| s.source) else {
        return;
    };
    let Some(src) = app.bus.project().audio_source(source).cloned() else {
        return;
    };
    let path = app.audio_file(&src);
    let gen = app.editor.next_gen();
    let Some(s) = app.editor.session.as_mut() else {
        return;
    };
    s.gen = gen;
    s.buf = None;
    s.detail = None;
    s.detail_asked = None;
    s.busy = Some("editor.busy.load");
    app.editor.worker().send(gen, Job::Load { path });
}

/// Give the thread a job for the open take; `busy` says what the window shows meanwhile
/// (`None`: a picture, which holds no button back).
pub(crate) fn dispatch(app: &mut CantusApp, busy: Option<&'static str>, job: Job) {
    let Some(s) = app.editor.session.as_mut() else {
        return;
    };
    if busy.is_some() {
        s.busy = busy;
        // A new piece of work replaces what was said — but the picture drawn after an edit
        // keeps the edit's word.
        if busy != Some("editor.busy.picture") {
            s.status = None;
        }
    }
    let gen = s.gen;
    app.editor.worker().send(gen, job);
}

/// Once a frame: settle the agents' pending files, take in what the thread finished,
/// notice another project or an undo that changed the clip's take, and end an audition.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    commit::settle_pending(app);
    if app
        .editor
        .session
        .as_ref()
        .is_some_and(|s| s.epoch != app.epoch)
    {
        close(app);
    }
    let mut again = false;
    while let Some((gen, done)) = app.editor.worker.as_ref().and_then(Worker::poll) {
        again = true;
        // A result of a take since read again is not about this one — but a file it wrote
        // for nothing goes (it is no one's).
        if app.editor.session.as_ref().map(|s| s.gen) != Some(gen) {
            match done {
                Done::Applied(Ok(a)) => commit::discard(app, &a.file, a.fresh),
                Done::Exported(Ok((p, fresh)), _) => commit::discard(app, &p, fresh),
                _ => {}
            }
            continue;
        }
        take(app, done);
    }
    // (Results are taken in even with the window closed: a file written for nothing goes.)
    let Some(s) = app.editor.session.as_ref() else {
        return;
    };
    let (track, clip, source, loading) = (
        s.track,
        s.clip,
        s.source,
        s.buf.is_none() && s.busy.is_some(),
    );
    // Undo, redo or another edit moved the clip to another file: read that one.
    match app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| c.audio.as_ref().map(|a| a.source))
    {
        None => {
            close(app);
            return;
        }
        Some(now) if now != source && !loading => {
            if let Some(s) = app.editor.session.as_mut() {
                s.source = now;
            }
            load(app);
            again = true;
        }
        _ => {}
    }
    let marks = app
        .project()
        .source_marks
        .get(&source)
        .cloned()
        .unwrap_or_default();
    let mut want = None;
    let mut out = false;
    if let Some(s) = app.editor.session.as_mut() {
        s.marks = marks;
        if s.detail_want != s.detail_asked && s.busy.is_none() {
            want = s.detail_want.zip(s.buf.clone());
            s.detail_asked = s.detail_want;
        }
        out = std::mem::take(&mut s.dragged_out);
    }
    if let Some(((start, end, hop), buf)) = want {
        dispatch(
            app,
            None,
            Job::Detail {
                buf,
                start,
                end,
                hop,
            },
        );
    }
    if out {
        act::to_new_clip(app);
    }
    if let Some(s) = app.editor.session.as_mut() {
        if let Some(p) = s.play {
            let frames = p.to - p.from;
            let rate = s.buf.as_ref().map_or(48_000, |b| b.rate) as f64;
            if p.at.elapsed().as_secs_f64() * rate >= frames as f64 {
                s.play = None;
            }
        }
        if s.busy.is_some() || s.play.is_some() || again || s.detail_want != s.detail_asked {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

/// One result of the thread, for the open take.
fn take(app: &mut CantusApp, done: Done) {
    match done {
        Done::Loaded(r) => match r {
            Ok(Loaded { buf, peaks }) => {
                if let Some(s) = app.editor.session.as_mut() {
                    s.buf = Some(buf.clone());
                    s.peaks = Some(peaks);
                    s.busy = None;
                    s.sel = s
                        .sel
                        .map(|x| x.clamp(buf.frames()))
                        .filter(|x| !x.is_empty());
                }
                dispatch(app, Some("editor.busy.picture"), Job::Picture { buf });
            }
            Err(e) => {
                if let Some(s) = app.editor.session.as_mut() {
                    s.busy = None;
                }
                say(app, tf("editor.load_failed", &[("error", &e)]), true);
            }
        },
        Done::Applied(r) => {
            if let Some(s) = app.editor.session.as_mut() {
                s.busy = None;
            }
            match r {
                Ok(a) => commit::finish_edit(app, a),
                Err(e) if e == jobs::UNCHANGED => say(app, t("editor.unchanged"), false),
                Err(e) => say(app, e, true),
            }
        }
        Done::Learned(r) => {
            if let Some(s) = app.editor.session.as_mut() {
                s.busy = None;
            }
            match r {
                Ok(p) => {
                    let frames = p.frames;
                    if let Some(s) = app.editor.session.as_mut() {
                        s.profile = Some(p);
                        // *Reduce noise* acts on the whole take unless something else is
                        // selected: the noise stretch is not what is to be cleaned.
                        s.sel = None;
                        s.band = None;
                    }
                    say(
                        app,
                        tf("editor.learned", &[("n", &frames.to_string())]),
                        false,
                    );
                }
                Err(e) => say(app, tf("editor.learn_failed", &[("error", &e)]), true),
            }
        }
        Done::Picture(p) => {
            if let Some(s) = app.editor.session.as_mut() {
                s.spec = Some(p);
                if s.busy == Some("editor.busy.picture") {
                    s.busy = None;
                }
            }
        }
        Done::Detail(start, p) => {
            if let Some(s) = app.editor.session.as_mut() {
                s.detail = Some((start, p));
            }
        }
        Done::Exported(r, from) => {
            if let Some(s) = app.editor.session.as_mut() {
                s.busy = None;
            }
            match r {
                Ok((path, fresh)) => commit::place_new_clip(app, path, fresh, from),
                Err(e) => say(app, e, true),
            }
        }
    }
}

/// The folder a file chosen with *Open file…* or *Paste from file…* is read from.
pub fn last_dir(app: &CantusApp) -> Option<PathBuf> {
    app.audio_base().map(|b| b.to_path_buf())
}
