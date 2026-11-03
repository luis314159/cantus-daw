//! Export the dancers as a video with the song (round 35, ADR 0088): the dialog's choices,
//! the export running on a thread of its own, its progress and how it ended.
//!
//! The work is `cantus_video`'s; the song is the export's own render (the same as *Export
//! audio*), and the dancers come from that render's own timelines (ADR 0087:
//! `RenderProgress::dance`), drawn at each frame's time through the one renderer — so the
//! video shows what the screen shows. The dialog is `views/video_export.rs`; the thread's
//! work is [`job`]; agents call [`start`] through `ai_tools_video.rs`.

#[path = "video_export_job.rs"]
pub mod job;

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_video::Container;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

/// The kinds of file offered, in the dialog's order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// H.264 + AAC: for every platform.
    #[default]
    Mp4,
    /// AV1 + Vorbis: royalty-free, slower.
    Webm,
    /// PNG frames with alpha: the layer for a video editor.
    Mov,
    /// Numbered PNG files and a WAV.
    Png,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Mp4, Format::Webm, Format::Mov, Format::Png];

    pub fn container(self) -> Container {
        match self {
            Format::Mp4 => Container::Mp4,
            Format::Webm => Container::WebM,
            Format::Mov => Container::MovPng,
            Format::Png => Container::PngSequence,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Format::Mp4 => "video.format.mp4",
            Format::Webm => "video.format.webm",
            Format::Mov => "video.format.mov",
            Format::Png => "video.format.png",
        }
    }

    /// Whether it keeps a transparent background.
    pub fn alpha(self) -> bool {
        matches!(self, Format::Mov | Format::Png)
    }
}

/// The picture's shape.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// 1080 × 1920: TikTok, Reels, Shorts.
    #[default]
    Vertical,
    /// 1080 × 1080.
    Square,
    /// 1920 × 1080: YouTube.
    Landscape,
    /// One of [`SIZES`].
    Custom,
}

impl Shape {
    pub const ALL: [Shape; 4] = [
        Shape::Vertical,
        Shape::Square,
        Shape::Landscape,
        Shape::Custom,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Shape::Vertical => "video.shape.vertical",
            Shape::Square => "video.shape.square",
            Shape::Landscape => "video.shape.landscape",
            Shape::Custom => "video.shape.custom",
        }
    }
}

/// The sizes *Custom* offers (even, within every encoder's reach).
pub const SIZES: [(u32, u32); 8] = [
    (720, 1280),
    (1080, 1350),
    (1440, 2560),
    (2160, 3840),
    (1280, 720),
    (2560, 1440),
    (3840, 2160),
    (2048, 2048),
];

/// What is behind the dancers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Back {
    /// The theme's stage colour.
    #[default]
    Stage,
    /// Nothing (MOV and PNG; the others fall back to the stage).
    Transparent,
    Green,
    Blue,
    Magenta,
    Black,
}

impl Back {
    pub const ALL: [Back; 6] = [
        Back::Stage,
        Back::Transparent,
        Back::Green,
        Back::Blue,
        Back::Magenta,
        Back::Black,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Back::Stage => "video.back.stage",
            Back::Transparent => "video.back.transparent",
            Back::Green => "video.back.green",
            Back::Blue => "video.back.blue",
            Back::Magenta => "video.back.magenta",
            Back::Black => "video.back.black",
        }
    }

    /// A chroma key's colour.
    pub fn chroma(self) -> Option<[u8; 4]> {
        use cantus_dance::render::{CHROMA_BLUE, CHROMA_GREEN, CHROMA_MAGENTA};
        match self {
            Back::Green => Some(CHROMA_GREEN),
            Back::Blue => Some(CHROMA_BLUE),
            Back::Magenta => Some(CHROMA_MAGENTA),
            _ => None,
        }
    }
}

/// How much of the song.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Range {
    #[default]
    Song,
    Selection,
    Loop,
}

impl Range {
    pub const ALL: [Range; 3] = [Range::Song, Range::Selection, Range::Loop];

    fn scope(self) -> cantus_export::Scope {
        match self {
            Range::Song => cantus_export::Scope::Song,
            Range::Selection => cantus_export::Scope::Selection,
            Range::Loop => cantus_export::Scope::Loop,
        }
    }
}

/// The quality, as the dialog says it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Good,
    #[default]
    High,
    Best,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Good, Quality::High, Quality::Best];

    pub fn video(self) -> cantus_video::Quality {
        match self {
            Quality::Good => cantus_video::Quality::Good,
            Quality::High => cantus_video::Quality::High,
            Quality::Best => cantus_video::Quality::Best,
        }
    }
}

/// The dialog's choices, remembered between sessions (`Settings::video`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub format: Format,
    pub shape: Shape,
    /// The size *Custom* picked (one of [`SIZES`]).
    pub custom: (u32, u32),
    pub fps: u32,
    pub quality: Quality,
    pub background: Back,
    /// The song's title and tempo in the frames.
    pub title: bool,
    /// The light visualiser in the frames (round 35, ADR 0091 §4). // r35:stage
    pub visualiser: bool, // r35:stage
    pub range: Range,
    /// Cisco's OpenH264 allowed (Linux; Windows N): downloaded when first needed.
    pub openh264: bool,
    /// An AAC encoder the system has (FFmpeg, GStreamer) where the OS has none.
    pub system_aac: bool,
    /// The folder the last video went to.
    pub folder: Option<PathBuf>,
    /// This machine's measured pace, milliseconds a megapixel-frame, by format: what the
    /// estimate goes by once an export of that format has run (review 1 #10).
    pub pace: std::collections::BTreeMap<String, f64>,
}

impl Default for VideoSettings {
    fn default() -> Self {
        VideoSettings {
            format: Format::Mp4,
            shape: Shape::Vertical,
            custom: (1080, 1350),
            fps: 30,
            quality: Quality::High,
            background: Back::Stage,
            title: true,
            visualiser: true, // r35:stage
            range: Range::Song,
            openh264: false,
            system_aac: true,
            folder: None,
            pace: Default::default(),
        }
    }
}

impl VideoSettings {
    /// The picture's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        match self.shape {
            Shape::Vertical => (1080, 1920),
            Shape::Square => (1080, 1080),
            Shape::Landscape => (1920, 1080),
            Shape::Custom => {
                let (w, h) = self.custom;
                if SIZES.contains(&(w, h)) {
                    (w, h)
                } else {
                    (1080, 1350)
                }
            }
        }
    }

    /// Whether the background stays transparent in the file.
    pub fn transparent(&self) -> bool {
        self.background == Back::Transparent && self.format.alpha()
    }

    /// The file's extension (none for a sequence's folder).
    pub fn extension(&self) -> &'static str {
        self.format.container().extension()
    }
}

/// The video export's state in the interface.
#[derive(Default)]
pub struct VideoUi {
    /// The dialog, while it is open.
    pub dialog: Option<VideoSettings>,
    /// The dialog has just opened (its first frame closes menus left behind it).
    pub fresh: bool,
    /// Where the preview stands, seconds into the range.
    pub scrub: f64,
    /// The preview's own short render around the scrubbed time (the dancers as the file
    /// will have them).
    pub real: Option<PreviewRender>,
    /// The preview's texture, and what it shows (redrawn only when that changes).
    pub preview: Option<(String, egui::TextureHandle)>,
    /// How the last export ended, until the dialog closes.
    pub last: Option<Result<String, String>>,
    /// Cisco's library being fetched.
    pub openh264_fetch: Option<job::Fetch>,
    /// The encoders an MP4 would use, for the codecs asked (finding the system's AAC runs
    /// a program: asked once).
    plan: PlanState,
    /// How many times this window asked a worker for the encoders (the tests check that
    /// a MOV never does: a count of the whole process would also see another test's worker
    /// still finishing — GitHub's and WSL's Linux, where `ffmpeg -encoders` takes seconds).
    pub asked: usize,
    job: Option<Running>,
    /// An export asked for while an imported character is still being read: it starts
    /// when every character is in (or says which could not be read).
    waiting: Option<Waiting>,
}

/// Every dancer's timeline of a render, by track.
pub type Timelines = Vec<(cantus_core::TrackId, Vec<cantus_dance::DanceFrame>)>;

/// What the preview's render answers (r35:stage): the dancers' timelines, and its sound
/// analysed for the visualiser on the export's own instants (video time 0 at the range's
/// start; silent before the render's first second).
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewAnswer {
    pub timelines: Timelines,
    pub bands: std::sync::Arc<Vec<[f32; crate::stage::vis::BANDS]>>,
}

/// The preview's render: at most one under way (a newer moment stops the older one, and
/// starts once it has ended), and the last answer, by what it was asked for.
#[derive(Default)]
pub struct PreviewRender {
    running: Option<(String, job::PreviewJob)>,
    done: Option<(String, Result<PreviewAnswer, String>)>, // r35:stage
    /// The analysis of the render the preview shows now (r35:stage): `None` while it is
    /// being made or could not be.
    shown_bands: Option<std::sync::Arc<Vec<[f32; crate::stage::vis::BANDS]>>>,
}

impl PreviewRender {
    /// Stop what runs (the dialog closed, another project).
    pub fn cancel(&mut self) {
        if let Some((_, j)) = &self.running {
            j.cancel();
        }
    }
}

/// Where the preview's own render stands for the moment shown.
#[derive(Clone, Debug, PartialEq)]
pub enum PreviewState {
    Ready(Timelines),
    /// Under way (or waiting for an older one to stop): the dancers dance to the beat
    /// meanwhile, a "rendering…" chip says so.
    Rendering,
    /// It could not be made, in words: the beat-only preview stays, said in the chip.
    Failed(String),
}

/// The dancers' timelines for the preview at the scrubbed time.
pub fn preview_timelines(app: &mut CantusApp, d: &VideoSettings) -> PreviewState {
    let Some((from, _)) = range(app, d.range) else {
        return PreviewState::Failed(t("video.error.empty"));
    };
    let at = app.project().tempo_map().seconds_at(from.0 as f64) + app.video.scrub;
    let key = format!("{}|{}|{:.2}|{}", app.epoch, app.bus.revision(), at, from.0);
    let r = app.video.real.get_or_insert_with(Default::default);
    // An answer that came in (a stop's answer is dropped: a newer moment wanted it).
    let got = r
        .running
        .as_ref()
        .map(|(k, j)| (k.clone(), j.rx.try_recv()));
    match got {
        Some((k, Ok(res))) => {
            r.running = None;
            if res.as_ref().err().is_none_or(|e| !e.is_empty()) {
                r.done = Some((k, res));
            }
        }
        Some((k, Err(mpsc::TryRecvError::Disconnected))) => {
            r.running = None;
            r.done = Some((k, Err(t("video.preview.stopped"))));
        }
        _ => {}
    }
    if let Some((k, res)) = &r.done {
        if *k == key {
            r.shown_bands = res.as_ref().ok().map(|a| a.bands.clone()); // r35:stage
            return match res {
                Ok(a) => PreviewState::Ready(a.timelines.clone()), // r35:stage
                Err(e) => PreviewState::Failed(e.clone()),
            };
        }
    }
    r.shown_bands = None; // r35:stage
    let start = match &r.running {
        Some((k, _)) if *k == key => false,
        Some((_, j)) => {
            // A newer moment: the older render stops; this one starts once it has.
            j.cancel();
            false
        }
        None => true,
    };
    if start {
        let j = job::preview_render(app, at, from);
        app.video.real.get_or_insert_with(Default::default).running = Some((key, j));
    }
    PreviewState::Rendering
}

/// The analysed sound of the render the preview shows now (r35:stage; `None` while the
/// moment's render is being made): the visualiser in the preview draws from it, as the
/// export draws from its own render.
pub fn preview_bands(app: &CantusApp) -> Option<&[[f32; crate::stage::vis::BANDS]]> {
    app.video
        .real
        .as_ref()
        .and_then(|r| r.shown_bands.as_deref())
        .map(Vec::as_slice)
}

/// An export waiting for its characters.
struct Waiting {
    /// The project it was asked for (`CantusApp::epoch`): another one in its place (New,
    /// Open) drops it, never exported under the old one's name.
    epoch: u64,
    path: PathBuf,
    d: VideoSettings,
    replace: bool,
    since: std::time::Instant,
}

/// How long an export waits for its imported characters before it says so.
pub const CHARACTERS_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

struct Running {
    /// The format's key in `VideoSettings::pace`, and the export's megapixel-frames.
    pace: (String, f64),
    ctl: Arc<job::Progress>,
    rx: mpsc::Receiver<Result<cantus_video::Done, String>>,
    target: PathBuf,
    shown: Option<u32>,
    /// A warped clip's render was late at its compile: said when it lands.
    late: bool,
}

impl VideoUi {
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.waiting.is_some()
    }

    /// The running export's phase (a locale key), how far it is (0…1) and the seconds it
    /// expects still to take.
    pub fn progress(&self) -> Option<(&'static str, f32, Option<f64>)> {
        if self.waiting.is_some() {
            return Some(("video.phase.characters", 0.0, None));
        }
        let j = self.job.as_ref()?;
        Some((j.ctl.phase(), j.ctl.fraction(), j.ctl.remaining()))
    }
}

/// Open the dialog with the last choices.
pub fn open(app: &mut CantusApp) {
    app.video.dialog = Some(app.settings.video.clone());
    app.video.fresh = true;
    app.video.last = None;
    app.video.scrub = 0.0;
}

pub fn close(app: &mut CantusApp) {
    if let Some(d) = app.video.dialog.take() {
        remember(app, &d);
    }
    app.video.preview = None;
    app.video.last = None;
    // The preview's render stops with the dialog (review: it outlived it).
    if let Some(mut r) = app.video.real.take() {
        r.cancel();
    }
}

/// Keep the choices for next time.
pub fn remember(app: &mut CantusApp, d: &VideoSettings) {
    if app.settings.video != *d {
        app.settings.video = d.clone();
        app.store_settings();
    }
}

/// The range's start and end in ticks, if the project has it.
pub fn range(app: &CantusApp, r: Range) -> Option<(cantus_core::Ticks, cantus_core::Ticks)> {
    match r {
        Range::Song => {
            let end = app.project().content_end();
            (end > cantus_core::Ticks(0)).then_some((cantus_core::Ticks(0), end))
        }
        _ => crate::views::export::range(app, r.scope()),
    }
}

/// The file name offered: the song's name, safe for a file system.
pub fn file_name(app: &CantusApp, d: &VideoSettings) -> String {
    let base = crate::views::export::safe_name(&app.project().name);
    let base = if base.is_empty() {
        "Cantus".to_string()
    } else {
        base
    };
    match d.extension() {
        "" => base,
        ext => format!("{base}.{ext}"),
    }
}

/// Ask where, then start.
pub fn choose_and_start(app: &mut CantusApp) {
    let Some(d) = app.video.dialog.clone() else {
        return;
    };
    let mut picker = rfd::FileDialog::new();
    if let Some(dir) = d.folder.as_ref().filter(|f| f.is_dir()) {
        picker = picker.set_directory(dir);
    }
    let path = if d.format == Format::Png {
        picker.pick_folder().map(|dir| dir.join(file_name(app, &d)))
    } else {
        picker
            .add_filter(t(d.format.key()), &[d.extension()])
            .set_file_name(file_name(app, &d))
            .save_file()
    };
    let Some(path) = path else { return };
    // The save dialog asked about the name it returned; the file written is the name with
    // the format's extension, which may be another file that is there — asked here, so a
    // `clip` typed where `clip.mp4` is never replaces it unasked (review 1 #8).
    let target = with_extension(&path, &d);
    if must_ask(&path, &target) {
        let name = target.file_name().unwrap_or_default().to_string_lossy();
        let ok = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(t("export_audio.replace_file.title"))
            .set_description(tf("export_audio.replace_file.text", &[("file", &name)]))
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show();
        if ok != rfd::MessageDialogResult::Ok {
            return;
        }
    }
    if let Err(e) = start(app, &target, &d, true) {
        app.video.last = Some(Err(e));
    }
}

/// The key of a format's measured pace.
pub fn pace_key(f: Format) -> &'static str {
    match f {
        Format::Mp4 => "mp4",
        Format::Webm => "webm",
        Format::Mov => "mov",
        Format::Png => "png",
    }
}

/// The megapixel-frames of an export of `seconds` with `d`'s picture.
pub fn mpx_frames(d: &VideoSettings, seconds: f64) -> f64 {
    let (w, h) = d.size();
    w as f64 * h as f64 / 1e6 * seconds * d.fps as f64
}

/// The length of `d`'s range, in seconds.
pub fn length_of(app: &CantusApp, d: &VideoSettings) -> f64 {
    range(app, d.range).map_or(0.0, |(a, b)| {
        let map = app.project().tempo_map();
        map.seconds_at(b.0 as f64) - map.seconds_at(a.0 as f64)
    })
}

/// Keep what an export measured (half the old pace, half the new: one slow run does not
/// rule the next estimate).
fn learn_pace(app: &mut CantusApp, key: &str, ms: f64) {
    if !ms.is_finite() || ms <= 0.0 {
        return;
    }
    let mut keep = app.settings.video.clone();
    let p = keep.pace.get(key).map_or(ms, |old| (old + ms) / 2.0);
    keep.pace.insert(key.to_string(), p);
    if let Some(open) = app.video.dialog.as_mut() {
        open.pace = keep.pace.clone();
    }
    remember(app, &keep);
}

/// A duration as the estimate says it: honestly rounded, never to the tenth.
pub fn honest(seconds: f64) -> String {
    if seconds < 10.0 {
        t("video.time.few")
    } else if seconds < 60.0 {
        let n = ((seconds / 5.0).round() * 5.0) as u64;
        tf("video.time.seconds", &[("n", &n)])
    } else {
        let n = (seconds / 60.0).round().max(1.0) as u64;
        tf("video.time.minutes", &[("n", &n)])
    }
}

/// Whether writing `target` for the name `picked` the save dialog returned replaces a file
/// it did not ask about.
pub fn must_ask(picked: &Path, target: &Path) -> bool {
    target != picked && target.exists()
}

/// Start exporting `d` to `path` (`replace`: a file there may be replaced — the save dialog
/// asked). An error in words when it cannot start.
pub fn start(
    app: &mut CantusApp,
    path: &Path,
    d: &VideoSettings,
    replace: bool,
) -> Result<(), String> {
    start_since(app, path, d, replace, std::time::Instant::now())
}

/// [`start`], for an export first asked for at `since` (one waiting for its characters).
fn start_since(
    app: &mut CantusApp,
    path: &Path,
    d: &VideoSettings,
    replace: bool,
    since: std::time::Instant,
) -> Result<(), String> {
    if app.export_in_progress() {
        return Err(t("video.error.busy"));
    }
    // Outboard gear cannot play in an offline render (ADR 0094 §6). r36:external
    if let Some(words) = crate::external_service::export_refusal(app) {
        return Err(words); // r36:external
    }
    // One render at a time through the plug-ins' render instances (review 1 #1), as the
    // audio export: a background freeze refresh gives way; a bake waits for nobody.
    if !crate::freeze::yield_refresh(app) {
        return Err(t("freeze.export_waits"));
    }
    if app.modulators.pending_bake.is_some() || app.warp.pending_bake.is_some() {
        let job = t("isolation.render.bake");
        return Err(tf("isolation.render.busy", &[("job", &job)]));
    }
    let Some((from, to)) = range(app, d.range) else {
        return Err(t(match d.range {
            Range::Song => "video.error.empty",
            Range::Selection => "export_audio.no_selection",
            Range::Loop => "export_audio.no_loop",
        }));
    };
    if app.engine.samples.loading() {
        return Err(t("video.error.samples"));
    }
    let rate = app.engine.sample_rate.round() as u32;
    if !app.plugins.render_ready(
        cantus_host::RenderFor::Export,
        app.bus.project(),
        f64::from(rate),
        false,
    ) {
        return Err(t("isolation.render.loading"));
    }
    let path = with_extension(path, d);
    // The encoders matter to an MP4 alone (review 2 #1): WebM, MOV and PNG never ask.
    let mp4_plan = (d.format == Format::Mp4).then(|| plan_now(app, d));
    if mp4_plan.as_ref().is_some_and(|p| p.video.is_none()) {
        return Err(t(no_h264_key()));
    }
    // Every dancer's character in before anything renders (an import read off this
    // thread): never the default in its place unasked.
    let cast = match job::resolve_dancers(app) {
        job::Cast::Ready(c) => c,
        job::Cast::Failed(f) => {
            let names: Vec<String> = f.iter().map(|(k, why)| format!("{k} ({why})")).collect();
            return Err(tf("video.error.character", &[("names", &names.join(", "))]));
        }
        job::Cast::Loading if since.elapsed() >= CHARACTERS_WAIT => {
            return Err(t("video.error.character_slow"));
        }
        job::Cast::Loading => {
            app.video.waiting = Some(Waiting {
                epoch: app.epoch,
                path,
                d: d.clone(),
                replace,
                since,
            });
            app.video.last = None;
            app.set_status(t("video.phase.characters"));
            return Ok(());
        }
    };
    let before = app.engine.renders_late();
    let work = job::Work::new(app, d, (from, to), rate, mp4_plan, cast);
    let late = app.late_since(before);
    let ctl = Arc::new(job::Progress::default());
    let (tx, rx) = mpsc::channel();
    let (c, p, settings) = (ctl.clone(), path.clone(), d.clone());
    let spawned = std::thread::Builder::new()
        .name("cantus-video".into())
        .spawn(move || {
            let r = job::run(work, &settings, &p, replace, &c);
            let _ = tx.send(r);
        });
    if let Err(e) = spawned {
        app.plugins.finish_render();
        return Err(tf("video.error.failed", &[("error", &e)]));
    }
    let mut remembered = d.clone();
    remembered.folder = path.parent().map(Path::to_path_buf);
    remember(app, &remembered);
    if let Some(open) = app.video.dialog.as_mut() {
        open.folder = remembered.folder;
    }
    let pace = (
        pace_key(d.format).to_string(),
        mpx_frames(d, length_of(app, d)),
    );
    app.video.job = Some(Running {
        ctl,
        rx,
        target: path,
        shown: None,
        late,
        pace,
    });
    app.video.last = None;
    app.set_status(t("video.status.started"));
    Ok(())
}

/// `path` with the format's extension (a sequence's folder as it is).
pub fn with_extension(path: &Path, d: &VideoSettings) -> PathBuf {
    let ext = d.extension();
    if ext.is_empty()
        || path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
    {
        return path.to_path_buf();
    }
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{ext}"));
    PathBuf::from(s)
}

/// The codecs this export may use: Cisco's library if it is switched on and here.
pub fn codecs(app: &CantusApp, d: &VideoSettings) -> cantus_video::Codecs {
    cantus_video::Codecs {
        openh264: if d.openh264 {
            job::openh264_install(app).and_then(|i| i.library())
        } else {
            None
        },
        system_aac: d.system_aac,
    }
}

/// The encoders, asked for some codecs: being found on a worker, or found.
#[derive(Default)]
enum PlanState {
    #[default]
    Unasked,
    Checking(cantus_video::Codecs, mpsc::Receiver<cantus_video::Plan>),
    Ready(cantus_video::Codecs, cantus_video::Plan),
}

/// The encoders an MP4 would use with `d`'s codecs, once known: finding them starts
/// programs (`ffmpeg -encoders`, `gst-inspect-1.0`) and creates the system's encoders,
/// so it runs on a worker and the dialog says "checking…" meanwhile (review 1 #4).
pub fn plan(app: &mut CantusApp, d: &VideoSettings) -> Option<cantus_video::Plan> {
    let c = codecs(app, d);
    match &app.video.plan {
        PlanState::Ready(k, p) if *k == c => return Some(p.clone()),
        PlanState::Checking(k, rx) if *k == c => {
            let p = rx.try_recv().ok()?;
            app.video.plan = PlanState::Ready(c, p.clone());
            return Some(p);
        }
        _ => {}
    }
    app.video.asked += 1;
    app.video.plan = PlanState::Checking(c.clone(), ask(c));
    None
}

/// Find the encoders for `c` on a worker (never the interface's thread: it starts programs
/// and creates the system's encoders — review 2 #1); the answer comes on the channel.
fn ask(c: cantus_video::Codecs) -> mpsc::Receiver<cantus_video::Plan> {
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("cantus-video-codecs".into())
        .spawn({
            let tx = tx.clone();
            let c = c.clone();
            move || {
                let _ = tx.send(compute(&c));
            }
        });
    if worker.is_err() {
        // No thread to be had: answered as if the system had nothing (refused in words).
        let _ = tx.send(cantus_video::Plan {
            video: None,
            audio: cantus_video::select::AudioPath::Mp3,
        });
    }
    rx
}

/// The plans computed in this process, and the name of the thread the last one ran on (the
/// tests check that the interface's never does).
pub static PLANS: std::sync::Mutex<(usize, String)> = std::sync::Mutex::new((0, String::new()));

fn compute(c: &cantus_video::Codecs) -> cantus_video::Plan {
    let p = cantus_video::select::plan(c);
    let mut seen = PLANS.lock().unwrap_or_else(|e| e.into_inner());
    seen.0 += 1;
    seen.1 = std::thread::current().name().unwrap_or("").to_string();
    p
}

/// The encoders, waiting for them to be found (an export about to start).
pub fn plan_now(app: &mut CantusApp, d: &VideoSettings) -> cantus_video::Plan {
    let c = codecs(app, d);
    if let PlanState::Checking(k, rx) = &app.video.plan {
        if *k == c {
            if let Ok(p) = rx.recv() {
                app.video.plan = PlanState::Ready(c, p.clone());
                return p;
            }
        }
    }
    if let PlanState::Ready(k, p) = &app.video.plan {
        if *k == c {
            return p.clone();
        }
    }
    // Asked on a worker and waited for: an export (or an agent's) needs the answer now,
    // and the work still never runs on this thread.
    app.video.asked += 1;
    let p = ask(c.clone()).recv().unwrap_or(cantus_video::Plan {
        video: None,
        audio: cantus_video::select::AudioPath::Mp3,
    });
    app.video.plan = PlanState::Ready(c, p.clone());
    p
}

/// What to say when there is no H.264 encoder here.
pub fn no_h264_key() -> &'static str {
    if cfg!(windows) {
        "video.error.windows_n"
    } else {
        "video.error.no_h264"
    }
}

/// Stop the running export.
pub fn stop(app: &mut CantusApp) {
    if app.video.waiting.take().is_some() {
        app.set_status(t("video.status.stopped"));
    }
    if let Some(j) = &app.video.job {
        j.ctl.cancel();
    }
}

/// Once a frame: the export's progress on the status line, and its end.
pub fn poll(app: &mut CantusApp) {
    job::poll_fetch(app);
    if let Some(w) = app.video.waiting.take() {
        if w.epoch != app.epoch {
            // Asked for another project (New, Open meanwhile): dropped, and said.
            app.set_status(t("video.status.dropped"));
            app.video.last = None;
        } else if let Err(e) = start_since(app, &w.path, &w.d, w.replace, w.since) {
            app.set_error(e.clone());
            app.video.last = Some(Err(e));
        }
    }
    let Some(j) = &mut app.video.job else { return };
    let result = match j.rx.try_recv() {
        Ok(r) => r,
        Err(mpsc::TryRecvError::Empty) => {
            let pct = (j.ctl.fraction() * 100.0).floor() as u32;
            if j.shown != Some(pct) {
                j.shown = Some(pct);
                let phase = t(j.ctl.phase());
                app.set_status(tf(
                    "video.status.progress",
                    &[("phase", &phase), ("pct", &pct)],
                ));
            }
            return;
        }
        Err(mpsc::TryRecvError::Disconnected) => Err(t("video.error.died")),
    };
    let (target, late, pace) = (j.target.clone(), j.late, j.pace.clone());
    app.video.job = None;
    if let Ok(done) = &result {
        learn_pace(app, &pace.0, done.wall * 1000.0 / pace.1.max(1e-6));
    }
    finish(app, result, &target, late);
}

/// The end of an export: a plug-in that stopped or never loaded for the render fails it
/// (the file lacks it), one begun from a stale state and a late warped clip are said —
/// as the audio export does (review 1 #2).
fn finish(
    app: &mut CantusApp,
    result: Result<cantus_video::Done, String>,
    target: &Path,
    late: bool,
) {
    let lost = app.plugins.render_failures();
    app.plugins.finish_render();
    let (losses, warnings) = crate::views::plugin_notice::render_words(&lost, "export", "");
    match result {
        Ok(_) if losses.is_some() => {
            let text = losses.unwrap_or_default();
            app.video.last = Some(Err(text.clone()));
            app.set_error(text);
        }
        Ok(done) => {
            let mut text = summary(&done, target);
            if let Some(w) = &warnings {
                text = format!("{text} {w}");
            }
            if late {
                text = format!("{text} {}", t("warp.status.late"));
            }
            app.video.last = Some(Ok(text.clone()));
            if warnings.is_some() || late {
                app.set_error(text);
            } else {
                app.set_status(text);
            }
        }
        Err(e) if e.is_empty() => {
            app.set_status(t("video.status.stopped"));
            app.video.last = None;
        }
        Err(e) => {
            app.set_error(e.clone());
            app.video.last = Some(Err(e));
        }
    }
}

/// Wait for the running export (tests; the interface polls).
pub fn wait(app: &mut CantusApp) {
    while app.video.busy() {
        poll(app);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// How an export ended, in words: the file, its length, size and time, and what to know.
pub fn summary(done: &cantus_video::Done, target: &Path) -> String {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut s = tf(
        "video.done",
        &[
            ("file", &name),
            ("seconds", &format!("{:.1}", done.seconds)),
            ("mb", &format!("{:.1}", done.bytes as f64 / 1e6)),
            ("took", &format!("{:.1}", done.wall)),
        ],
    );
    if done.warnings.contains(&cantus_video::Warning::Mp3Audio) {
        s.push(' ');
        s.push_str(&t("video.warn.mp3"));
    }
    s
}

/// Once a frame from the app: the dialog and the export's end.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    poll(app);
    crate::views::video_export::show(app, ctx);
}

/// File ▸ Export video… (the File menu is egui's own, as its other entries).
pub fn menu_item(app: &mut CantusApp, ui: &mut egui::Ui) {
    if ui.button(t("video.menu")).clicked() {
        ui.close();
        open(app);
    }
}

/// Why a render cannot start now, in words naming the export running (audio or video);
/// `None` when none runs (review 2 #3: never a silent refusal).
pub fn busy_words(app: &CantusApp) -> Option<String> {
    if !app.export_in_progress() {
        return None;
    }
    let job = if app.video.busy() {
        t("video.job")
    } else {
        t("isolation.render.export")
    };
    Some(tf("isolation.render.busy", &[("job", &job)]))
}

/// The window is closing: a video being made stops and its work in progress is removed
/// (the export removes its part when stopped) before the process ends (review 2 #2).
pub fn settle_on_exit(app: &mut CantusApp) {
    app.video.waiting = None;
    let Some(j) = app.video.job.take() else {
        return;
    };
    j.ctl.cancel();
    let _ = j.rx.recv_timeout(std::time::Duration::from_secs(60));
    app.plugins.finish_render();
}
