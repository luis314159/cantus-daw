//! The video export's work on its own thread: the song rendered by the export's own render
//! (with the dancers' units keeping their timelines — ADR 0087), the audio at a video's
//! rate, every frame drawn from those timelines and encoded by `cantus_video`. Also the
//! download of Cisco's OpenH264 when the person allows it.

use super::{Back, VideoSettings};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::inst::{dancer, InstrumentKind};
use cantus_core::tempo::TempoMap;
use cantus_core::{Snapshot, Ticks, TrackId};
use cantus_dance::render::{Background, Stage};
use cantus_dance::{Character, DanceFrame};
use cantus_engine::{EffectUnit, NativeUnit, RenderOptions, RenderProgress};
use cantus_video::dance::{Dancer, SongFrames};
use cantus_video::{Control, Done, Request, Timing};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

/// The share of the bar the song's render takes; the frames take the rest.
const RENDER_SHARE: f32 = 0.25;

/// How far an export is, shared with the interface.
#[derive(Debug)]
pub struct Progress {
    cancel: AtomicBool,
    /// 0 the song, 1 the frames.
    phase: AtomicU8,
    pub render: RenderProgress,
    pub video: Control,
    started: Instant,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            cancel: AtomicBool::new(false),
            phase: AtomicU8::new(0),
            render: RenderProgress::default(),
            video: Control::default(),
            started: Instant::now(),
        }
    }
}

impl Progress {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.render.cancel();
        self.video.cancel();
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// The phase's locale key.
    pub fn phase(&self) -> &'static str {
        if self.phase.load(Ordering::Relaxed) == 0 {
            "video.phase.render"
        } else {
            "video.phase.frames"
        }
    }

    /// The whole export, 0…1.
    pub fn fraction(&self) -> f32 {
        if self.phase.load(Ordering::Relaxed) == 0 {
            RENDER_SHARE * self.render.fraction()
        } else {
            RENDER_SHARE + (1.0 - RENDER_SHARE) * self.video.fraction()
        }
    }

    /// Seconds still to go, from the pace so far (once there is a pace to go by).
    pub fn remaining(&self) -> Option<f64> {
        let f = self.fraction() as f64;
        let spent = self.started.elapsed().as_secs_f64();
        (f > 0.04 && spent > 1.0).then(|| spent * (1.0 - f) / f)
    }
}

/// One dancer of the project, as the video draws it.
#[derive(Clone)]
pub struct DancerSpec {
    pub track: TrackId,
    pub character: Arc<dyn Character>,
}

/// Everything the thread needs, taken from the app on the interface's thread.
pub struct Work {
    snapshot: Arc<Snapshot>,
    render: RenderOptions,
    units: Vec<EffectUnit>,
    instruments: Vec<NativeUnit>,
    dancers: Vec<DancerSpec>,
    default_character: Arc<dyn Character>,
    tempo: TempoMap,
    from: Ticks,
    stage_colour: [u8; 4],
    title: String,
    plan: Option<cantus_video::Plan>,
    vis: Option<crate::stage::vis_video::Setup>, // r35:stage — the visualiser (ADR 0091)
}

impl Work {
    pub fn new(
        app: &mut CantusApp,
        d: &VideoSettings,
        (from, to): (Ticks, Ticks),
        rate: u32,
        plan: Option<cantus_video::Plan>,
        dancers: Vec<DancerSpec>,
    ) -> Work {
        let snapshot = app
            .engine
            .compile_for_render(app.bus.project(), app.bus.revision());
        let render = RenderOptions {
            sample_rate: rate,
            channels: 2,
            start: from,
            end: Some(to),
            tail_seconds: 0.0,
            compensate_latency: true,
            ..Default::default()
        };
        let units = app.plugins.render_units(app.bus.project(), f64::from(rate));
        let instruments = app
            .plugins
            .render_instruments(app.bus.project(), f64::from(rate));
        let project_dir = crate::dance_import::project_dir(app); // r35:video — imports resolve as the stage does
        let (default_character, _) = app
            .dance_live
            .character(dancer::DEFAULT_CHARACTER, &project_dir);
        let title = title_line(app, d);
        let p = app.project();
        let vis = crate::stage::vis_video::setup_for(p, d, !title.is_empty()); // r35:stage
        let s = crate::kit::Style::current();
        let c = s.t.panel;
        Work {
            snapshot,
            render,
            units,
            instruments,
            dancers,
            default_character,
            tempo: p.tempo_map(),
            from,
            stage_colour: [c.r(), c.g(), c.b(), 255],
            title,
            plan,
            vis, // r35:stage
        }
    }
}

/// The project's dancers as a video shows them: in track order, those that show anywhere
/// (a hidden one, `view.mode` 2, shows nowhere — as on the stage), each with its character
/// resolved as the screen resolves it — an import read off this thread.
#[derive(Clone)]
pub enum Cast {
    /// Every character is in.
    Ready(Vec<DancerSpec>),
    /// An imported character is still being read.
    Loading,
    /// These characters could not be read (their keys, and why).
    Failed(Vec<(String, String)>),
}

/// Whether the dancer's numbers say it shows nowhere.
pub fn hidden(n: &cantus_core::NativeSpec) -> bool {
    n.get("view.mode").round() as i32 == 2
}

/// The dancers and their characters now (see [`Cast`]).
pub fn resolve_dancers(app: &mut CantusApp) -> Cast {
    let project_dir = crate::dance_import::project_dir(app); // r35:video — imports resolve as the stage does
    let wall = crate::dance_live::wall_seconds();
    let specs: Vec<(TrackId, String)> = app
        .project()
        .tracks
        .iter()
        .filter_map(|t| {
            let n = t.instrument.native()?;
            (n.kind == InstrumentKind::Dancer && !hidden(n)).then(|| {
                let id = n
                    .text(dancer::CHARACTER_KEY)
                    .unwrap_or(dancer::DEFAULT_CHARACTER)
                    .to_string();
                (t.id, id)
            })
        })
        .collect();
    let (mut out, mut loading, mut failed) = (Vec::new(), false, Vec::new());
    for (track, id) in specs {
        let r = app.dance_live.resolve(&id, &project_dir, wall);
        loading |= r.loading;
        if let Some(missing) = r.missing {
            failed.push((missing, r.why.unwrap_or_default()));
        }
        out.push(DancerSpec {
            track,
            character: r.character,
        });
    }
    if !failed.is_empty() {
        Cast::Failed(failed)
    } else if loading {
        Cast::Loading
    } else {
        Cast::Ready(out)
    }
}

/// The dancers as the preview draws them now (an import still loading or missing draws
/// the default meanwhile, as on the screen).
pub fn dancers(app: &mut CantusApp) -> Vec<DancerSpec> {
    let project_dir = crate::dance_import::project_dir(app); // r35:video — imports resolve as the stage does
    let wall = crate::dance_live::wall_seconds();
    let specs: Vec<(TrackId, String)> = app
        .project()
        .tracks
        .iter()
        .filter_map(|t| {
            let n = t.instrument.native()?;
            (n.kind == InstrumentKind::Dancer && !hidden(n)).then(|| {
                let id = n
                    .text(dancer::CHARACTER_KEY)
                    .unwrap_or(dancer::DEFAULT_CHARACTER)
                    .to_string();
                (t.id, id)
            })
        })
        .collect();
    specs
        .into_iter()
        .map(|(track, id)| DancerSpec {
            track,
            character: app.dance_live.resolve(&id, &project_dir, wall).character,
        })
        .collect()
}

/// Preview renders running now, the most there ever were at once, those cancelled, and
/// the thread the last compile ran on (the tests check none runs on the interface's).
pub static PREVIEW_LIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static PREVIEW_MOST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static PREVIEW_CANCELLED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
pub static PREVIEW_THREAD: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// One preview render under way: its answer, and the flag that stops it.
pub struct PreviewJob {
    pub rx: mpsc::Receiver<Result<crate::video_export::PreviewAnswer, String>>, // r35:stage
    pub progress: Arc<RenderProgress>,
}

impl PreviewJob {
    /// Stop it at its next block (a newer moment, the dialog closed, another project).
    pub fn cancel(&self) {
        if !self.progress.cancel.swap(true, Ordering::Relaxed) {
            PREVIEW_CANCELLED.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// The dancers' timelines around `at` (song seconds) for the dialog's preview: the song
/// compiled and rendered offline as the file's render does — automation, notes and
/// modulators — from up to four seconds before (as far back as the range's start) to just
/// after, **all on a worker**: this thread only copies the project (the export's own
/// compile waits for audio, pitch and warp renders, never acceptable while a dialog is
/// drawn). Its compile has no decoded audio or hosted plug-ins: a dancer reacting to an
/// audio track or a plug-in hears it silent here, and right in the file.
pub fn preview_render(app: &CantusApp, at: f64, range_start: Ticks) -> PreviewJob {
    let (tx, rx) = mpsc::channel();
    let project = app.bus.project().clone();
    let revision = app.bus.revision();
    let rate = app.engine.sample_rate.round() as u32;
    let progress = Arc::new(RenderProgress::default());
    let p = progress.clone();
    let spawned = std::thread::Builder::new()
        .name("cantus-video-preview".into())
        .spawn(move || {
            let live = PREVIEW_LIVE.fetch_add(1, Ordering::SeqCst) + 1;
            PREVIEW_MOST.fetch_max(live, Ordering::SeqCst);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                *PREVIEW_THREAD.lock().unwrap_or_else(|e| e.into_inner()) =
                    std::thread::current().name().unwrap_or("").to_string();
                let map = project.tempo_map();
                let first = map.seconds_at(range_start.0 as f64);
                let from = Ticks(map.tick_at_seconds((at - 4.0).max(first)).floor() as i64);
                let to = Ticks(map.tick_at_seconds(at + 0.1).ceil() as i64);
                let snapshot = Arc::new(Snapshot::compile(&project, revision));
                let opts = RenderOptions {
                    sample_rate: rate,
                    channels: 2,
                    start: from,
                    end: Some(to.max(from + Ticks(1))),
                    tail_seconds: 0.0,
                    compensate_latency: true,
                    ..Default::default()
                };
                p.dance.want_timelines();
                // r35:stage — its sound analysed on the export's own instants (video time 0
                // at the range's start), for the visualiser the file draws.
                let offset = (map.sample_at(from.0 as f64, f64::from(rate))
                    - map.sample_at(range_start.0 as f64, f64::from(rate)))
                .round() as i64; // r35:stage
                cantus_engine::render_with_progress(snapshot, &opts, Vec::new(), Vec::new(), &p)
                    .map(|r| crate::video_export::PreviewAnswer {
                        timelines: p.dance.timelines(),
                        bands: Arc::new(crate::stage::vis_video::analyse_from(
                            &r.samples,
                            r.channels as usize,
                            r.sample_rate,
                            offset,
                        )), // r35:stage
                    })
                    // A stop is the empty string: the preview drops its answer.
                    .map_err(|e| match e {
                        cantus_engine::RenderError::Cancelled => String::new(),
                        e => e.to_string(),
                    })
            }))
            .unwrap_or_else(|_| Err("the render stopped unexpectedly".into()));
            PREVIEW_LIVE.fetch_sub(1, Ordering::SeqCst);
            let _ = tx.send(r);
        });
    if let Err(e) = spawned {
        let (tx, rx2) = mpsc::channel();
        let _ = tx.send(Err(e.to_string()));
        return PreviewJob { rx: rx2, progress };
    }
    PreviewJob { rx, progress }
}

/// A dancer's timeline from the tempo map alone, a frame every 10 ms from `from` for
/// `seconds`: what a project without dancers shows — the default character to the beat.
pub fn beat_timeline(map: &TempoMap, from: Ticks, seconds: f64) -> Vec<DanceFrame> {
    let start = map.seconds_at(from.0 as f64);
    let ppq = cantus_core::time::PPQ as f64;
    let steps = (seconds * 100.0).ceil() as usize + 1;
    (1..=steps)
        .map(|i| {
            let s = start + i as f64 / 100.0;
            let tick = map.tick_at_seconds(s);
            let at = Ticks(tick.floor() as i64);
            let bar_start = map.bar_start(at);
            let bar_len = map.bar_end(at).0 - bar_start.0;
            DanceFrame {
                beats: tick / ppq,
                seconds: s,
                tempo_bpm: map.bpm_at(tick) as f32,
                playing: true,
                bar: map.bbt(at).0 as i32 - 1,
                bar_start_beats: bar_start.0 as f64 / ppq,
                bar_beats: (bar_len as f64 / ppq) as f32,
                ..DanceFrame::default()
            }
        })
        .collect()
}

/// The frames' background: the theme's stage colour (`stage`), a key colour, black, or
/// nothing where the file keeps transparency.
pub fn background_for(d: &VideoSettings, stage: [u8; 4]) -> Background {
    if d.transparent() {
        return Background::Transparent;
    }
    match d.background {
        Back::Black => Background::Solid([0, 0, 0, 255]),
        b => Background::Solid(b.chroma().unwrap_or(stage)),
    }
}

/// Render, then draw and encode. `Err("")` when stopped.
pub fn run(
    work: Work,
    d: &VideoSettings,
    path: &Path,
    replace: bool,
    p: &Progress,
) -> Result<Done, String> {
    let Work {
        snapshot,
        render,
        units,
        instruments,
        dancers,
        default_character,
        tempo,
        from,
        stage_colour,
        title,
        plan,
        vis, // r35:stage
    } = work;
    p.render.dance.want_timelines(); // r35:dancer — only a video keeps the dancers' timelines
    let rendered =
        cantus_engine::render_with_progress(snapshot, &render, units, instruments, &p.render)
            .map_err(|e| match e {
                cantus_engine::RenderError::Cancelled => String::new(),
                cantus_engine::RenderError::Empty => t("video.error.empty"),
                e => tf("video.error.render", &[("error", &e.to_string())]),
            })?;
    if p.cancelled() {
        return Err(String::new());
    }
    let timelines = p.render.dance.timelines();
    let mut audio = cantus_export::Audio {
        rate: rendered.sample_rate,
        channels: rendered.channels,
        samples: rendered.samples,
    };
    if !cantus_video::timing::AUDIO_RATES.contains(&audio.rate) {
        audio = cantus_export::process::resample(audio, 48_000);
    }
    let seconds = audio.seconds();
    let mut list: Vec<Dancer> = dancers
        .iter()
        .map(|spec| Dancer {
            character: spec.character.clone(),
            timeline: timelines
                .iter()
                .find(|(t, _)| *t == spec.track)
                .map(|(_, tl)| tl.clone())
                .unwrap_or_default(),
        })
        .collect();
    if list.is_empty() {
        list.push(Dancer {
            character: default_character,
            timeline: beat_timeline(&tempo, from, seconds),
        });
    }
    let (w, h) = d.size();
    let vis = crate::stage::vis_video::for_render(vis, &audio, &tempo, from); // r35:stage
    let overlay = crate::stage::vis_video::with_title(
        vis,
        title_overlay_for(title, background_for(d, stage_colour)),
    ); // r35:stage
    let frames_src = SongFrames {
        dancers: list,
        stage: Stage::new(w, h, background_for(d, stage_colour)),
        start_seconds: tempo.seconds_at(from.0 as f64),
        fps: d.fps,
        overlay,
    };
    let pcm = cantus_video::audio::Pcm {
        rate: audio.rate,
        channels: audio.channels,
        samples: audio.samples,
    };
    let req = Request {
        container: d.format.container(),
        width: w,
        height: h,
        fps: d.fps,
        frames: Timing::frames_for(d.fps, seconds).max(1),
        quality: d.quality.video(),
        alpha: d.transparent(),
        audio: &pcm,
        workers: 0,
        replace,
        plan,
    };
    p.phase.store(1, Ordering::Relaxed);
    cantus_video::export(&req, &frames_src, path, &p.video).map_err(|e| words(&e))
}

/// The title's ink and outline over the frames' background (review 1 #6): the ink of
/// the two that stands out more, by luminance — a dark one over Paper's light stage —
/// and the other round every glyph; over a transparent layer, light with a dark edge.
pub fn title_colours(background: Background) -> ([u8; 4], [u8; 4]) {
    cantus_video::title::ink_for(match background {
        Background::Solid(c) => Some(c),
        Background::Transparent => None,
    })
}

/// The title line drawn at the frames' foot (none when it is empty).
pub fn title_overlay_for(
    title: String,
    background: Background,
) -> Option<Box<cantus_video::dance::Overlay>> {
    if title.is_empty() {
        return None;
    }
    let font = crate::design::score_faces().bold;
    let (ink, edge) = title_colours(background);
    Some(Box::new(
        move |_n: u32, _t: f64, pm: &mut tiny_skia::Pixmap| {
            let (w, h) = (pm.width() as f32, pm.height() as f32);
            let px = (w.min(h) * 0.045).max(8.0);
            let (x, base, max) = (px, h - px * 1.4, w - 2.0 * px);
            cantus_video::title::draw(pm, font, &title, x, base, px, max, ink, Some(edge));
        },
    ))
}

/// The title line for the dialog's choices (empty when the title is off).
pub fn title_line(app: &CantusApp, d: &VideoSettings) -> String {
    if !d.title {
        return String::new();
    }
    let p = app.project();
    let (from, _) = super::range(app, d.range).unwrap_or_default();
    let bpm = p.tempo_map().bpm_at(from.0 as f64);
    tf(
        "video.title_line",
        &[("name", &p.name), ("bpm", &format!("{bpm:.0}"))],
    )
}

/// The title overlay for the dialog's choices.
pub fn title_overlay(
    app: &CantusApp,
    d: &VideoSettings,
) -> Option<Box<cantus_video::dance::Overlay>> {
    let c = crate::kit::Style::current().t.panel;
    title_overlay_for(
        title_line(app, d),
        background_for(d, [c.r(), c.g(), c.b(), 255]),
    )
}

/// An export's error in the interface's words (a stop is the empty string).
pub fn words(e: &cantus_video::VideoError) -> String {
    use cantus_video::VideoError as E;
    match e {
        E::Cancelled => String::new(),
        E::NoEncoder("H.264") => t(super::no_h264_key()),
        E::NoEncoder(what) => tf("video.error.no_encoder", &[("what", what)]),
        E::Io(e) => tf("video.error.io", &[("error", &e.to_string())]),
        e => tf("video.error.failed", &[("error", &e.to_string())]),
    }
}

/// Cisco's OpenH264 in the person's data folder (`None` where Cisco has no binary).
pub fn openh264_install(_app: &CantusApp) -> Option<cantus_video::openh264::Install> {
    cantus_video::openh264::Install::new(crate::app::data_dir().join("Codecs").join("OpenH264"))
}

/// Cisco's words while its library is installed (the About box).
pub fn openh264_credit() -> Option<&'static str> {
    cantus_video::openh264::Install::new(crate::app::data_dir().join("Codecs").join("OpenH264"))?
        .credit()
}

/// Remove Cisco's library (its switch turned off and the person asked).
pub fn remove_openh264(app: &mut CantusApp) {
    let Some(inst) = openh264_install(app) else {
        return;
    };
    match inst.remove() {
        Ok(()) => app.set_status(t("video.openh264.removed")),
        Err(e) => app.set_error(tf("video.error.io", &[("error", &e.to_string())])),
    }
}

/// Cisco's library being downloaded.
pub struct Fetch {
    pub bytes: Arc<AtomicU64>,
    pub of: u64,
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<PathBuf, String>>,
}

/// Download Cisco's library (the person switched it on).
pub fn fetch_openh264(app: &mut CantusApp) {
    if app.video.openh264_fetch.is_some() {
        return;
    }
    let Some(inst) = openh264_install(app) else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    let bytes = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let (b, c) = (bytes.clone(), cancel.clone());
    let of = inst.blob.size;
    let spawned = std::thread::Builder::new()
        .name("cantus-openh264".into())
        .spawn(move || {
            let url = cantus_video::openh264::url(&inst.blob);
            let r = inst.install(&cantus_content::fetch::Net::new(), &url, &c, &mut |n, _| {
                b.store(n, Ordering::Relaxed)
            });
            let _ = tx.send(r.map_err(|e| words(&e)));
        });
    if spawned.is_ok() {
        app.video.openh264_fetch = Some(Fetch {
            bytes,
            of,
            cancel,
            rx,
        });
    }
}

/// Stop the download.
pub fn cancel_fetch(app: &mut CantusApp) {
    if let Some(f) = &app.video.openh264_fetch {
        f.cancel.store(true, Ordering::Relaxed);
    }
}

/// Once a frame: the download's end.
pub fn poll_fetch(app: &mut CantusApp) {
    let Some(f) = &app.video.openh264_fetch else {
        return;
    };
    match f.rx.try_recv() {
        Ok(Ok(_)) => {
            app.video.openh264_fetch = None;
            app.set_status(t("video.openh264.ready"));
        }
        Ok(Err(e)) => {
            app.video.openh264_fetch = None;
            if !e.is_empty() {
                app.set_error(tf("video.openh264.failed", &[("error", &e)]));
            }
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => app.video.openh264_fetch = None,
    }
}
