//! **The companion window** and **the streaming mode** (ADR 0091 §§2–3).
//!
//! A small OS window of its own (an egui viewport, deferred), titled [`TITLE`], always on
//! top if asked, with no decorations and a **transparent** background (per-pixel alpha), in
//! which a character dances **with whatever plays** — no automation needed: the song's
//! position as heard while the transport runs; when it is stopped but the master sounds
//! (a preview, a live input), a beat of its own at the project's tempo; its energy from the
//! master's level. A character chosen in it, or the project's own dancers. Dragged by the
//! character, resized by its corner; right-click for its choices.
//!
//! **Streaming mode** is the same window as a capture source: the same fixed title and
//! class every session, a background transparent, chroma (green, blue, magenta) or the
//! theme's, its size and position fixed and kept between sessions, nothing else of the
//! interface in it (no frame on hover, no grip), and *Copy OBS setup* in its menu.
//!
//! **Cost.** It is drawn only when its pose changed (60 a second playing, a 15-a-second
//! breath stopped) and **not at all while minimised or hidden**: the viewport skips its
//! pass, the tap is turned off. Its CPU time a frame and share of a core are measured and
//! shown in its menu; GPU time is not measured (said in words).
//!
//! The main window's frame writes what to show into [`Shared`]; the viewport's pass
//! (`views/stage_companion_view.rs`) reads it, draws, and leaves its gestures there as
//! [`Action`]s for the next main frame to carry out.

use super::vis::{Visualiser, BANDS};
use super::{Slot, StageDancer, ViewMode};
use crate::app::CantusApp;
use cantus_core::inst::dancer;
use cantus_dance::character::Character;
use cantus_dance::pose::Pose;
use cantus_dance::{pose_at, DanceFrame};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// The window's title: what OBS's and Streamlabs' window capture find it by.
pub const TITLE: &str = "Cantus Stage";
/// Its class on Linux (X11 `WM_CLASS`, Wayland app id). On Windows every Cantus window has
/// winit's class, `Window Class`, and OBS matches the title and the program first.
pub const APP_ID: &str = "cantus-stage";
/// The smallest it gets, points: room for the menu.
pub const MIN_SIZE: [f32; 2] = [240.0, 300.0];
/// The largest it gets, points (a settings file's nonsense is brought back to this).
pub const MAX_SIZE: [f32; 2] = [4096.0, 4096.0];

/// `size` within [`MIN_SIZE`] and [`MAX_SIZE`] (one that is not a number: the standard).
pub fn sane_size(size: [f32; 2]) -> [f32; 2] {
    if !size.iter().all(|v| v.is_finite()) {
        return SIZES[1];
    }
    [
        size[0].clamp(MIN_SIZE[0], MAX_SIZE[0]),
        size[1].clamp(MIN_SIZE[1], MAX_SIZE[1]),
    ]
}

/// The sizes offered, points: three companions, then the shapes a stream or a video wants
/// (square, vertical, wide).
pub const SIZES: [[f32; 2]; 6] = [
    [320.0, 360.0],
    [400.0, 460.0],
    [540.0, 600.0],
    [600.0, 600.0],
    [540.0, 960.0],
    [960.0, 540.0],
];
/// How tall a character of size 1 stands in it, as a share of its height.
pub const STANDARD: f32 = 0.56;
/// Below this master peak (linear, −50 dBFS) the stopped song is silence.
pub const SOUNDING: f32 = 0.003_2;

/// The colour every window is cleared to before it is drawn (eframe asks once a window):
/// see-through while the companion is open — its backdrop is what it paints; else the
/// theme's panel, as before. The main window paints over all of itself.
pub fn clear_color(app: Option<&CantusApp>, visuals: &egui::Visuals) -> [f32; 4] {
    if app.is_some_and(|a| a.settings.stage.companion_open) {
        [0.0; 4]
    } else {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

/// The variable that turns the windows' alpha off (set by [`restart_without_alpha`], or by
/// hand for a driver that misdraws with it).
pub const NO_ALPHA_VAR: &str = "CANTUS_NO_ALPHA";

/// Whether the windows are made with an alpha channel (Linux, macOS: the companion is an
/// egui window there, see-through by the GL configuration's alpha, which every window shares
/// and which is chosen when the first opens — so the splash is made transparent and the main
/// window turns opaque when the splash becomes it, `boot::become_main_window`). On Windows
/// the companion is a layered window drawn from our own pixels (`companion_win`) and needs
/// none.
pub fn alpha_wanted() -> bool {
    !cfg!(windows) && std::env::var_os(NO_ALPHA_VAR).is_none_or(|v| v.is_empty())
}

/// Whether the companion can be see-through here: always on Windows, by alpha elsewhere.
pub fn see_through() -> bool {
    cfg!(windows) || alpha_wanted()
}

/// The graphics driver offered no configuration with alpha: run this program again, the
/// same way, without it, and end with it.
pub fn restart_without_alpha() -> anyhow::Result<()> {
    log::warn!("no GL configuration with alpha: starting again without it");
    let exe = std::env::current_exe()?;
    let status = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(NO_ALPHA_VAR, "1")
        .status()?;
    std::process::exit(status.code().unwrap_or(0));
}

/// The viewport's id.
pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("cantus-stage-companion")
}

/// What is behind the character.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backdrop {
    /// See-through (per-pixel alpha): OBS's Game Capture with *Allow transparency*.
    #[default]
    Transparent,
    /// The theme's stage.
    Theme,
    /// A chroma key for a capture that keeps no alpha (Window Capture + Chroma Key).
    Green,
    Blue,
    Magenta,
}

impl Backdrop {
    pub const ALL: [Backdrop; 5] = [
        Backdrop::Transparent,
        Backdrop::Theme,
        Backdrop::Green,
        Backdrop::Blue,
        Backdrop::Magenta,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Backdrop::Transparent => "stage.backdrop.transparent",
            Backdrop::Theme => "stage.backdrop.theme",
            Backdrop::Green => "stage.backdrop.green",
            Backdrop::Blue => "stage.backdrop.blue",
            Backdrop::Magenta => "stage.backdrop.magenta",
        }
    }

    /// The chroma key's colour (straight RGBA), if it is one.
    pub fn chroma(self) -> Option<[u8; 4]> {
        use cantus_dance::render::{CHROMA_BLUE, CHROMA_GREEN, CHROMA_MAGENTA};
        match self {
            Backdrop::Green => Some(CHROMA_GREEN),
            Backdrop::Blue => Some(CHROMA_BLUE),
            Backdrop::Magenta => Some(CHROMA_MAGENTA),
            _ => None,
        }
    }

    pub fn next(self) -> Backdrop {
        let i = Self::ALL.iter().position(|b| *b == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// What the visualiser shows of the master.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisMode {
    Off,
    /// Its spectrum, a few dozen bands.
    #[default]
    Spectrum,
    /// Its level, one bar.
    Level,
}

impl VisMode {
    pub const ALL: [VisMode; 3] = [VisMode::Spectrum, VisMode::Level, VisMode::Off];

    pub fn label_key(self) -> &'static str {
        match self {
            VisMode::Off => "stage.vis.off",
            VisMode::Spectrum => "stage.vis.spectrum",
            VisMode::Level => "stage.vis.level",
        }
    }

    pub fn next(self) -> VisMode {
        let i = Self::ALL.iter().position(|b| *b == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// The companion's and the streaming window's choices (kept in the settings file).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StageSettings {
    /// The dancers are hidden on screen (View ▸ Stage, Alt+D): none over the workspace,
    /// no Stage tab — their own `view.mode` untouched.
    pub hidden: bool,
    /// The companion window was open when Cantus closed: it opens again at start (a
    /// streaming window OBS finds every session).
    pub companion_open: bool,
    /// Streaming mode: nothing but the stage in it, its size and place fixed.
    pub streaming: bool,
    /// Its outer position on the desktop, points (`None`: the system places it).
    pub pos: Option<[f32; 2]>,
    /// Windows: its outer top-left corner on the desktop in physical pixels, as the
    /// system gives it (`None`: near the bottom-right). Physical, because a point is a
    /// different size on each monitor: one read back at the primary's scale lands
    /// somewhere else; it is opened there directly and fitted onto a monitor that is.
    pub native_pos: Option<[i32; 2]>,
    /// Its inner size, points.
    pub size: [f32; 2],
    pub backdrop: Backdrop,
    pub on_top: bool,
    /// The character it shows (`builtin:<id>`).
    pub character: String,
    /// Show the project's own dancers instead.
    pub follow_project: bool,
    pub vis: VisMode,
    /// The bar and beat, and the song's name and tempo.
    pub bar_beat: bool,
    pub song: bool,
}

impl Default for StageSettings {
    fn default() -> Self {
        StageSettings {
            hidden: false,
            companion_open: false,
            streaming: false,
            pos: None,
            native_pos: None,
            size: SIZES[1],
            backdrop: Backdrop::Transparent,
            on_top: true,
            character: dancer::DEFAULT_CHARACTER.to_string(),
            follow_project: false,
            vis: VisMode::Spectrum,
            bar_beat: true,
            song: true,
        }
    }
}

/// What the companion's menu asks of the main window.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    Close,
    /// The next character of the cast (or the project's dancers after the last).
    NextCharacter,
    NextSize,
    NextBackdrop,
    NextVis,
    ToggleOnTop,
    ToggleBarBeat,
    ToggleSong,
    ToggleStreaming,
    CopyObsSetup,
    /// *Export video…* (the song with its dancers, `crate::video_export`).
    ExportVideo,
}

/// The visualiser's numbers as the window shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct VisSnap {
    pub bands: [f32; BANDS],
    pub level: f32,
    /// 1-based bar, 0-based beat in it, beats in the bar, and how far through the beat.
    pub bar: i64,
    pub beat: u32,
    pub beats_per_bar: u32,
    pub beat_phase: f32,
    pub playing: bool,
    pub song: Arc<str>,
    pub bpm: f32,
}

impl Default for VisSnap {
    fn default() -> Self {
        VisSnap {
            bands: [0.0; BANDS],
            level: 0.0,
            bar: 1,
            beat: 0,
            beats_per_bar: 4,
            beat_phase: 0.0,
            playing: false,
            song: Arc::from(""),
            bpm: 120.0,
        }
    }
}

/// The measured cost of the window's own pass.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Cost {
    /// CPU milliseconds a frame (its pass: the character drawn, the visualiser, the
    /// layout), smoothed.
    pub cpu_ms: f32,
    /// Frames a second it is drawn at, smoothed.
    pub fps: f32,
    /// Frames drawn since it opened.
    pub frames: u64,
}

impl Cost {
    /// The share of one core it takes, %.
    pub fn core_percent(&self) -> f32 {
        self.cpu_ms * self.fps / 10.0
    }
}

/// What the main window and the companion's pass share.
pub struct Shared {
    // ---- written by the main window ----
    pub settings: StageSettings,
    /// What to dance to (the song's frame; the dancer's numbers set for the companion).
    pub frame: DanceFrame,
    /// The characters and their poses as the main window last placed them (the project's
    /// dancers, when it follows the project), else one character posed from `frame`.
    pub dancers: Vec<(Arc<dyn Character>, Pose)>,
    pub follows: bool,
    pub vis: VisSnap,
    /// The name of the character shown (the menu's value).
    pub character_name: Arc<str>,
    // ---- written by the window's pass ----
    pub actions: Vec<Action>,
    /// Where it is on the desktop (outer, points) and how big inside, as the system says.
    pub outer: Option<egui::Rect>,
    pub inner: Option<egui::Rect>,
    pub minimized: bool,
    pub cost: Cost,
    /// When its last pass ran (egui's clock).
    pub last_pass: f64,
    // ---- the window's drawing ----
    /// Each dancer's texture (the egui window's path).
    pub(crate) slots: Vec<Slot>,
    pub(crate) hover_until: f64,
    pub redraws: u64,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            settings: StageSettings::default(),
            frame: DanceFrame::default(),
            dancers: Vec::new(),
            follows: false,
            vis: VisSnap::default(),
            character_name: Arc::from(""),
            actions: Vec::new(),
            outer: None,
            inner: None,
            minimized: false,
            cost: Cost::default(),
            last_pass: f64::NEG_INFINITY,
            slots: Vec::new(),
            hover_until: 0.0,
            redraws: 0,
        }
    }
}

/// The companion's state on the application (`app.stage.companion`).
#[derive(Default)]
pub struct Companion {
    pub shared: Arc<Mutex<Shared>>,
    pub(crate) vis: Visualiser,
    /// The beat of its own while the transport is stopped and the master sounds: beats
    /// counted from when sound began.
    free_beats: Option<f64>,
    last_now: f64,
    /// The position and size it was opened with (the viewport is not moved again unless
    /// a size is chosen).
    opened_at: Option<([f32; 2], Option<[f32; 2]>)>,
    /// When the window's rect last changed, to save it once it settles.
    rect_changed_at: Option<f64>,
    /// The tap is on.
    tapping: bool,
    /// The layered window of true per-pixel alpha (Windows).
    #[cfg(windows)]
    pub(crate) native: Option<super::companion_win::Native>,
}

/// The ids of the characters the menu steps through, in the picker's order.
pub fn character_ids() -> Vec<String> {
    cantus_dance::cast::builtin()
        .iter()
        .map(|c| c.info().id.clone())
        .collect()
}

/// The text *Copy OBS setup* puts on the clipboard, for the window's backdrop.
pub fn obs_setup(backdrop: Backdrop) -> String {
    let key = match backdrop {
        Backdrop::Transparent => "stage.obs.transparent",
        Backdrop::Theme => "stage.obs.theme",
        _ => "stage.obs.chroma",
    };
    crate::i18n::tf(
        key,
        &[
            ("title", &TITLE),
            ("colour", &crate::i18n::t(backdrop.label_key())),
        ],
    )
}

/// Open or close the companion window (View ▸ Companion window, its shortcut).
pub fn toggle(app: &mut CantusApp) {
    let s = &mut app.settings.stage;
    s.companion_open = !s.companion_open;
    app.store_settings();
}

/// Turn streaming mode on or off (View ▸ Streaming mode): on opens the window too.
pub fn toggle_streaming(app: &mut CantusApp) {
    let s = &mut app.settings.stage;
    s.streaming = !s.streaming;
    if s.streaming {
        s.companion_open = true;
    }
    app.store_settings();
}

fn apply(app: &mut CantusApp, c: &mut Companion, ctx: &egui::Context, actions: &[Action]) {
    if actions.is_empty() {
        return;
    }
    for a in actions {
        let s = &mut app.settings.stage;
        match a {
            Action::Close => s.companion_open = false,
            Action::NextCharacter => {
                let ids = character_ids();
                if s.follow_project {
                    s.follow_project = false;
                    s.character = ids.first().cloned().unwrap_or_default();
                } else {
                    match ids.iter().position(|i| *i == s.character) {
                        Some(i) if i + 1 < ids.len() => s.character = ids[i + 1].clone(),
                        _ => s.follow_project = true,
                    }
                }
            }
            Action::NextSize => {
                let now = s.size;
                let i = SIZES
                    .iter()
                    .position(|z| (z[0] - now[0]).abs() < 1.0 && (z[1] - now[1]).abs() < 1.0)
                    .map_or(0, |i| (i + 1) % SIZES.len());
                s.size = SIZES[i];
                if let Some(o) = c.opened_at.as_mut() {
                    o.0 = s.size;
                }
            }
            Action::NextBackdrop => s.backdrop = s.backdrop.next(),
            Action::NextVis => s.vis = s.vis.next(),
            Action::ToggleOnTop => s.on_top = !s.on_top,
            Action::ToggleBarBeat => s.bar_beat = !s.bar_beat,
            Action::ToggleSong => s.song = !s.song,
            Action::ToggleStreaming => s.streaming = !s.streaming,
            Action::CopyObsSetup => ctx.copy_text(obs_setup(s.backdrop)),
            Action::ExportVideo => {}
        }
    }
    app.store_settings();
    if actions.contains(&Action::ExportVideo) {
        // The dialog opens in the main window: brought back first when it is minimised or
        // hidden (streaming from the companion alone), else it would open unseen.
        show_main_window(ctx);
        crate::video_export::open(app);
    }
}

/// The main window restored, shown and brought to the front (a command; it also works from
/// `logic`, while no pass runs).
fn show_main_window(ctx: &egui::Context) {
    let (minimized, visible) = ctx.input(|i| {
        let v = i.viewport();
        (v.minimized == Some(true), v.visible())
    });
    let root = egui::ViewportId::ROOT;
    if minimized {
        ctx.send_viewport_cmd_to(root, egui::ViewportCommand::Minimized(false));
    }
    if visible == Some(false) {
        ctx.send_viewport_cmd_to(root, egui::ViewportCommand::Visible(true));
    }
    ctx.send_viewport_cmd_to(root, egui::ViewportCommand::Focus);
    ctx.request_repaint_of(root);
}

/// The song to dance to: as heard while the transport runs; stopped while the master
/// sounds, a beat of its own at the project's tempo from when the sound began; else still.
fn song_frame(app: &CantusApp, c: &mut Companion, now: f64, peak: f32) -> DanceFrame {
    let mut f = super::source::transport_frame(app);
    let dt = (now - c.last_now).clamp(0.0, 0.25);
    c.last_now = now;
    if f.playing || peak < SOUNDING {
        c.free_beats = None;
        return f;
    }
    let bpm = f64::from(f.tempo_bpm.max(1.0));
    let beats = c.free_beats.map_or(0.0, |b| b + dt * bpm / 60.0);
    c.free_beats = Some(beats);
    let per_bar = f64::from(f.bar_beats.max(1.0));
    let bar = (beats / per_bar).floor();
    f.playing = true;
    f.beats = beats;
    f.seconds = beats * 60.0 / bpm;
    f.bar = bar as i32;
    f.bar_start_beats = bar * per_bar;
    f
}

/// The companion's own numbers: the defaults, standing in the middle above the
/// visualiser, its energy following the master's level.
fn companion_values(f: &mut DanceFrame, level: f32, vis: bool) {
    let set = |f: &mut DanceFrame, k: &str, v: f32| {
        if let Some(i) = dancer::KEYS.iter().position(|x| *x == k) {
            f.values[i] = v;
        }
    };
    set(f, "x", 0.5);
    set(f, "y", if vis { 0.80 } else { 0.92 });
    set(f, "react", 0.55);
    set(f, "level", level);
}

/// One main-window frame of the companion: carry out what its window asked, feed the
/// visualiser, hand the window what to show, and show it (or nothing, closed).
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    run(app, ctx, true);
}

/// The application's logic while its main window is minimised or hidden (eframe then runs
/// no pass of it, only `App::logic`): the companion keeps dancing — the engine's reports
/// read, the window handed what to show and asked to draw — at its own rate.
pub fn logic(app: &mut CantusApp, ctx: &egui::Context) {
    let hidden = ctx.input(|i| {
        let v = i.viewport();
        v.minimized == Some(true) || v.visible() == Some(false)
    });
    if !hidden || !app.settings.stage.companion_open {
        return;
    }
    app.engine.poll();
    // Following the project's dancers, the companion keeps them moving itself: the main
    // window's frame, which reads their feeds and gathers them, does not run now.
    if app.settings.stage.follow_project {
        crate::dance_live::frame(app);
        let mut st = std::mem::take(&mut app.stage);
        super::source::gather(app, &mut st);
        app.stage = st;
    }
    run(app, ctx, false);
    let playing = app
        .stage
        .companion
        .shared
        .lock()
        .map(|s| s.frame.playing && !s.minimized)
        .unwrap_or(false);
    let next = if playing {
        std::time::Duration::from_secs_f64(1.0 / 30.0)
    } else {
        std::time::Duration::from_secs_f64(1.0 / super::IDLE_RATE)
    };
    ctx.request_repaint_after(next);
}

fn run(app: &mut CantusApp, ctx: &egui::Context, in_pass: bool) {
    let mut c = std::mem::take(&mut app.stage.companion);
    let shared = c.shared.clone();
    let (actions, minimized, outer, inner) = {
        let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
        (
            std::mem::take(&mut sh.actions),
            sh.minimized,
            sh.outer,
            sh.inner,
        )
    };
    apply(app, &mut c, ctx, &actions);
    // A size from a settings file edited by hand, or stored from a window the system made
    // a pixel wide, is brought back within reason before anything is drawn at it.
    let sane = sane_size(app.settings.stage.size);
    if sane != app.settings.stage.size {
        app.settings.stage.size = sane;
    }
    let st = app.settings.stage.clone();
    let tap_wanted = st.companion_open && st.vis != VisMode::Off && !minimized;
    // Checked every frame against the engine's own tap: an engine made again (the audio
    // device reopened) starts with its tap off, and is re-armed here.
    if let Some(t) = app.engine.master_tap() {
        if t.is_on() != tap_wanted {
            t.set_on(tap_wanted);
        }
    }
    c.tapping = tap_wanted;
    if !st.companion_open {
        c.opened_at = None;
        #[cfg(windows)]
        {
            c.native = None;
        }
        app.stage.companion = c;
        return;
    }
    // egui's clock in a pass; the wall clock from `logic` (while the main window is
    // minimised its input's time is the last shown frame's). A step between the two is
    // clamped where it matters (the free beat, the redraw's rate).
    let now = if in_pass {
        ctx.input(|i| i.time)
    } else {
        crate::dance_live::wall_seconds()
    };
    if c.tapping {
        let vis = &mut c.vis;
        if let Some(t) = app.engine.master_tap() {
            t.drain(|pairs| vis.feed(pairs));
        }
    }
    let m = app.engine.meters().master;
    let peak = if minimized { 0.0 } else { m[0].max(m[1]) };
    let sample_rate = app.engine.sample_rate as f32;
    c.vis.update(now, sample_rate, peak);
    let mut frame = song_frame(app, &mut c, now, peak);
    companion_values(&mut frame, c.vis.level, st.vis != VisMode::Off);

    // What to show: the project's dancers, or the chosen character.
    let follows = st.follow_project;
    let ch = app.stage.character(&st.character);
    let project: Vec<StageDancer> = if follows {
        app.stage
            .dancers()
            .iter()
            .filter(|d| d.mode != ViewMode::Hidden)
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    {
        let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
        if sh.settings != st {
            sh.settings = st.clone();
        }
        sh.frame = frame;
        sh.follows = follows;
        sh.dancers.clear();
        if follows {
            sh.dancers
                .extend(project.iter().map(|d| (d.character.clone(), d.pose)));
        } else {
            let pose = pose_at(&frame, ch.as_ref(), now);
            sh.dancers.push((ch.clone(), pose));
        }
        if follows {
            if *sh.character_name != *crate::i18n::t("stage.companion.project_dancers") {
                sh.character_name =
                    Arc::from(crate::i18n::t("stage.companion.project_dancers").as_str());
            }
        } else if *sh.character_name != *ch.info().name {
            sh.character_name = Arc::from(ch.info().name.as_str());
        }
        let v = &mut sh.vis;
        v.bands = c.vis.bands;
        v.level = c.vis.level;
        let per_bar = frame.bar_beats.max(1.0);
        let in_bar = (frame.beats - frame.bar_start_beats).max(0.0);
        v.bar = i64::from(frame.bar) + 1;
        v.beats_per_bar = per_bar.round().max(1.0) as u32;
        v.beat = (in_bar.floor() as u32).min(v.beats_per_bar - 1);
        v.beat_phase = frame.beats.rem_euclid(1.0) as f32;
        v.playing = frame.playing;
        v.bpm = frame.tempo_bpm;
        let song = app.project().name.as_str();
        if *v.song != *song {
            v.song = Arc::from(song);
        }
    }

    // Windows: a layered window of true per-pixel alpha, drawn from our own pixels (no egui
    // viewport; egui embeds viewports in tests, which keep the path below).
    #[cfg(windows)]
    if !ctx.embed_viewports() {
        super::companion_win::run(app, &mut c, ctx, &shared, now, frame.playing);
        if !minimized && c.vis.moving() {
            crate::views::workspace_ticker::moving(
                ctx,
                crate::views::workspace_ticker::Reason::Animation,
            );
        }
        app.stage.companion = c;
        return;
    }

    // Its place, as it was opened (moved again only when a size is chosen).
    let opened = *c.opened_at.get_or_insert((st.size, st.pos));
    let mut vb = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_app_id(APP_ID)
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(!st.streaming)
        .with_inner_size(opened.0)
        .with_min_inner_size(MIN_SIZE)
        .with_window_level(if st.on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        });
    if let Some(p) = opened.1 {
        vb = vb.with_position(p);
    }
    if in_pass {
        let cb = shared.clone();
        ctx.show_viewport_deferred(viewport_id(), vb, move |ui, class| {
            crate::stage::companion_view::show(ui, class, &cb);
        });
    }
    if !minimized && (frame.playing || c.vis.moving()) {
        ctx.request_repaint_of(viewport_id());
    }
    if !minimized && c.vis.moving() {
        crate::views::workspace_ticker::moving(
            ctx,
            crate::views::workspace_ticker::Reason::Animation,
        );
    }

    // Keep its place and size once they settle (dragged, resized by its corner).
    let s = &mut app.settings.stage;
    let pos = outer.map(|r| [r.min.x, r.min.y]);
    let size = inner.map(|r| [r.width(), r.height()]);
    let moved = pos.is_some_and(|p| s.pos != Some(p));
    let resized =
        size.is_some_and(|z| (z[0] - s.size[0]).abs() > 0.5 || (z[1] - s.size[1]).abs() > 0.5);
    if moved || resized {
        if let Some(p) = pos {
            s.pos = Some(p);
        }
        if let Some(z) = size {
            s.size = z;
        }
        c.rect_changed_at = Some(now);
    }
    if c.rect_changed_at.is_some_and(|t| now - t > 1.5) {
        c.rect_changed_at = None;
        app.store_settings();
    } else if c.rect_changed_at.is_some() {
        crate::views::workspace_ticker::wake_after(ctx, std::time::Duration::from_millis(1600));
    }
    app.stage.companion = c;
}
