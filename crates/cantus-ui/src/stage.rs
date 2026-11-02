//! **The stage** (round 35, ADR 0091): where the dancers dance on screen.
//!
//! A dancer track's `view.mode` says where its character shows:
//!
//! - **floating** (0): over the workspace, a frameless, transparent figure standing at its
//!   `x`/`y` share of the workspace, `size` × [`geom::FLOAT_STANDARD`] of its height tall.
//!   Its frame and grips appear on hover (kit-drawn: move, resize, pin on top, dock, hide);
//!   `view.top` keeps it above the floating panels ([`float`]).
//! - **docked** (1): on the workspace's **Stage** tab, on the theme's stage ([`dock`]).
//! - **hidden** (2): nowhere, and costs nothing.
//!
//! **One path to the pixels.** Each dancer is drawn by `cantus-dance`'s one renderer into a
//! box of its own at the exact physical pixel size it is shown at ([`geom`]), through
//! [`crate::dance_texture::DanceTexture`], and only when its pose changed — at most 60 times
//! a second while the song plays, a slow breath ([`IDLE_RATE`]) while it is stopped, and
//! not at all while hidden or minimised ([`StageState::due`]).
//!
//! **Where the poses come from** ([`source`]): the song's position as heard and each
//! dancer's numbers, through the pure `cantus_dance::pose_at`.

#[path = "views/stage_chrome.rs"]
pub mod chrome;
#[path = "stage_companion.rs"]
pub mod companion;
#[cfg(windows)]
#[path = "stage_companion_menu_win.rs"]
pub mod companion_menu_win;
#[path = "stage_companion_paint.rs"]
pub mod companion_paint;
#[path = "views/stage_companion_view.rs"]
pub mod companion_view;
#[cfg(windows)]
#[path = "stage_companion_win.rs"]
pub mod companion_win;
#[path = "views/stage_dock.rs"]
pub mod dock;
#[path = "views/stage_float.rs"]
pub mod float;
#[path = "stage_geom.rs"]
pub mod geom;
#[path = "stage_source.rs"]
pub mod source;
#[path = "stage_vis.rs"]
pub mod vis;
#[path = "views/stage_vis_paint.rs"]
pub mod vis_paint;
#[path = "stage_vis_video.rs"]
pub mod vis_video;

use crate::dance_texture::DanceTexture;
use cantus_core::TrackId;
use cantus_dance::character::Character;
use cantus_dance::pose::Pose;
use cantus_dance::render::Stage;
use std::collections::HashMap;
use std::sync::Arc;

/// The stage's actions (keymap 15): `true` when `action` was one.
pub fn action(app: &mut crate::app::CantusApp, action: crate::keymap::Action) -> bool {
    use crate::keymap::Action;
    match action {
        Action::ToggleStage => toggle_shown(app),
        Action::ToggleCompanion => companion::toggle(app),
        Action::ToggleStreaming => companion::toggle_streaming(app),
        _ => return false,
    }
    true
}

/// Show or hide the dancers on screen: hidden, none over the workspace and the Stage tab
/// closed; shown again, the tab back if a dancer is docked.
pub fn toggle_shown(app: &mut crate::app::CantusApp) {
    use crate::views::workspace::Pane;
    let s = &mut app.settings.stage;
    s.hidden = !s.hidden;
    if s.hidden {
        app.workspace.hide(Pane::Stage);
    } else if app
        .stage
        .dancers()
        .iter()
        .any(|d| d.mode == ViewMode::Docked)
    {
        app.workspace.reveal(Pane::Stage);
    }
    app.store_settings();
}

/// View ▸ Stage, Companion window, Streaming mode (after the tempo track's entry).
pub fn view_menu(app: &mut crate::app::CantusApp, ui: &mut egui::Ui) {
    use crate::i18n::t;
    use crate::keymap::Action;
    let key = |app: &crate::app::CantusApp, a: Action| {
        app.keymap
            .bindings_for(a)
            .first()
            .map(|b| b.display())
            .unwrap_or_default()
    };
    let rows = [
        (
            Action::ToggleStage,
            "stage.view.stage",
            !app.settings.stage.hidden,
        ),
        (
            Action::ToggleCompanion,
            "stage.view.companion",
            app.settings.stage.companion_open,
        ),
        (
            Action::ToggleStreaming,
            "stage.view.streaming",
            app.settings.stage.streaming,
        ),
    ];
    for (a, label, on) in rows {
        let mut shown = on;
        let k = key(app, a);
        if ui
            .checkbox(&mut shown, format!("{}    {k}", t(label)))
            .changed()
        {
            action(app, a);
        }
    }
}

/// Redraws a second while the song plays (a dancer's pose changes every frame): 30, what a
/// video plays at — half the display's frames, half the drawing and uploading.
pub const PLAY_RATE: f64 = 30.0;
/// Redraws a second while a dancer is dragged or resized: the display's rate.
pub const DRAG_RATE: f64 = 60.0;
/// Redraws a second while it is stopped: a slow breath and a blink.
pub const IDLE_RATE: f64 = 12.0;

/// The redraw rate for a dancer: dragged, playing, or breathing.
pub fn rate(playing: bool, dragging: bool) -> f64 {
    if dragging {
        DRAG_RATE
    } else if playing {
        PLAY_RATE
    } else {
        IDLE_RATE
    }
}

/// Where a dancer shows (`view.mode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ViewMode {
    Floating,
    Docked,
    Hidden,
}

impl ViewMode {
    pub const ALL: [ViewMode; 3] = [ViewMode::Floating, ViewMode::Docked, ViewMode::Hidden];

    /// The mode a `view.mode` value stands for.
    pub fn of(v: f32) -> ViewMode {
        match v.round() as i32 {
            1 => ViewMode::Docked,
            2 => ViewMode::Hidden,
            _ => ViewMode::Floating,
        }
    }

    /// Its `view.mode` value.
    pub fn value(self) -> f32 {
        match self {
            ViewMode::Floating => 0.0,
            ViewMode::Docked => 1.0,
            ViewMode::Hidden => 2.0,
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            ViewMode::Floating => "stage.mode.floating",
            ViewMode::Docked => "stage.mode.docked",
            ViewMode::Hidden => "stage.mode.hidden",
        }
    }
}

/// One dancer as the stage shows it this frame.
#[derive(Clone)]
pub struct StageDancer {
    pub track: TrackId,
    /// The track's name (the frame's label, the screen reader's).
    pub name: Arc<str>,
    pub character: Arc<dyn Character>,
    pub pose: Pose,
    pub mode: ViewMode,
    /// Above the floating panels (`view.top`).
    pub top: bool,
}

/// Which surface a texture belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    /// Over the workspace.
    Float,
    /// On the docked Stage tab.
    Dock,
}

/// One dancer's texture on one surface, and what it last drew.
pub(crate) struct Slot {
    pub tex: DanceTexture,
    /// The pose and sub-stage it last drew: drawing them again would give the same pixels.
    pub last: Option<(Pose, Stage)>,
    /// When it last drew (egui's clock, seconds).
    pub drawn_at: f64,
    /// The frame it was last shown on: a slot not shown for a frame is let go.
    pub seen: u64,
}

impl Slot {
    pub(crate) fn new(name: String, pass: u64) -> Slot {
        Slot {
            tex: DanceTexture::new(name),
            last: None,
            drawn_at: 0.0,
            seen: pass,
        }
    }

    /// The texture of `ch` in `pose` on `stage`, drawn again if it is due; with the seconds
    /// the drawing took when it was drawn.
    pub(crate) fn texture(
        &mut self,
        ctx: &egui::Context,
        ch: &dyn Character,
        pose: &Pose,
        stage: &Stage,
        rate: f64,
    ) -> (Option<egui::TextureId>, Option<f64>) {
        let (now, max_side) = ctx.input(|i| (i.time, i.max_texture_side));
        if stage.width as usize > max_side || stage.height as usize > max_side {
            // Bigger than the graphics card takes (a dancer of size 4 on a 4K screen):
            // not drawn rather than drawn wrong.
            return (None, None);
        }
        if StageState::due(self, pose, stage, now, rate) {
            let t = std::time::Instant::now();
            let id = self.tex.draw(ctx, stage, &[(ch, *pose)]);
            self.last = Some((*pose, *stage));
            self.drawn_at = now;
            return (id, Some(t.elapsed().as_secs_f64()));
        }
        (self.tex.id(), None)
    }
}

/// A drag of a dancer's body or grip, while it lasts.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Drag {
    /// Moving: the pointer's offset from the feet, in physical pixels.
    Move { track: TrackId, grab: egui::Vec2 },
    /// Resizing from the top grip.
    Size { track: TrackId },
}

/// The stage's own state (on the application: `app.stage`).
#[derive(Default)]
pub struct StageState {
    pub(crate) slots: HashMap<(TrackId, Surface), Slot>,
    /// The companion window and the streaming mode (ADR 0091 §§2–3).
    pub companion: companion::Companion,
    /// Track names as last shown, shared frame to frame.
    pub(crate) names: HashMap<TrackId, Arc<str>>,
    /// Characters by their `character` value, made once.
    pub(crate) cast: HashMap<String, Arc<dyn Character>>,
    /// This frame's dancers (reused).
    pub(crate) dancers: Vec<StageDancer>,
    pub(crate) drag: Option<Drag>,
    /// Where each dancer's body was shown this frame (points): what the pointer grabs.
    pub(crate) bodies: Vec<(TrackId, Surface, egui::Rect)>,
    /// The egui pass the dancers were gathered for (the tab and the overlay share them).
    pub(crate) gathered: Option<u64>,
    /// The dancer whose frame shows, and until when it lingers after the pointer left.
    pub(crate) hover: Option<(TrackId, Surface, f64)>,
    /// Frames the stage has been through (`float::show` counts them).
    pub passes: u64,
    /// Textures drawn since the app started: what the redraw policy is measured by.
    pub redraws: u64,
    /// Time spent drawing textures, in seconds (the cost shown, ADR 0091 §5).
    pub draw_seconds: f64,
    /// The physical pixels drawn by the last redraws (the cost's other half).
    pub drawn_px: u64,
}

impl StageState {
    /// Whether a slot showing `pose` on `stage` must draw now: it changed, and its last
    /// drawing is older than the rate allows (60 a second playing or dragged, the idle
    /// breath stopped).
    pub(crate) fn due(slot: &Slot, pose: &Pose, stage: &Stage, now: f64, rate: f64) -> bool {
        if slot.last.as_ref() == Some(&(*pose, *stage)) {
            return false;
        }
        if slot.last.is_none() {
            return true;
        }
        // A hair under the period, so a frame that arrives on time is never skipped.
        now - slot.drawn_at >= 0.9 / rate || now < slot.drawn_at
    }

    /// Draw `ch` in `pose` on `stage` into the slot of `key` if it is due; returns the
    /// texture to show (the last one when nothing changed).
    pub(crate) fn texture(
        &mut self,
        ctx: &egui::Context,
        key: (TrackId, Surface),
        ch: &dyn Character,
        pose: &Pose,
        stage: &Stage,
        rate: f64,
    ) -> Option<egui::TextureId> {
        let pass = self.passes;
        let slot = self
            .slots
            .entry(key)
            .or_insert_with(|| Slot::new(format!("stage-{}-{:?}", key.0 .0, key.1), pass));
        slot.seen = pass;
        let (id, drawn) = slot.texture(ctx, ch, pose, stage, rate);
        if let Some(secs) = drawn {
            self.draw_seconds += secs;
            self.drawn_px += u64::from(stage.width) * u64::from(stage.height);
            self.redraws += 1;
        }
        id
    }

    /// Let go of the slots not shown this frame (a dancer hidden, removed, moved).
    pub(crate) fn sweep(&mut self) {
        let pass = self.passes;
        self.slots.retain(|_, s| s.seen == pass);
    }

    /// The character a `character` value names, made once and kept.
    pub fn character(&mut self, id: &str) -> Arc<dyn Character> {
        if let Some(c) = self.cast.get(id) {
            return c.clone();
        }
        let c = source::resolve(id);
        self.cast.insert(id.to_string(), c.clone());
        c
    }

    /// A track's name as an `Arc`, made again only when it changed.
    pub(crate) fn name(&mut self, track: TrackId, name: &str) -> Arc<str> {
        match self.names.get(&track) {
            Some(n) if **n == *name => n.clone(),
            _ => {
                let n: Arc<str> = Arc::from(name);
                self.names.insert(track, n.clone());
                n
            }
        }
    }

    /// The dancers shown this frame, as last gathered.
    pub fn dancers(&self) -> &[StageDancer] {
        &self.dancers
    }

    /// The dancer whose frame shows (the pointer over it, or lingering), and where.
    pub fn hovered(&self, now: f64) -> Option<(TrackId, Surface)> {
        self.hover
            .filter(|(_, _, until)| *until > now)
            .map(|(t, s, _)| (t, s))
    }

    /// Where a dancer's body was shown this frame, in points.
    pub fn body(&self, track: TrackId, surface: Surface) -> Option<egui::Rect> {
        self.bodies
            .iter()
            .find(|(t, s, _)| *t == track && *s == surface)
            .map(|(_, _, r)| *r)
    }

    /// Whether a dancer is being moved or resized.
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The textures alive now (one per dancer shown).
    pub fn textures(&self) -> usize {
        self.slots.len()
    }

    /// The last drawn pixmap of a dancer on a surface (tests and the cost readout).
    pub fn pixmap(&self, track: TrackId, surface: Surface) -> Option<&tiny_skia::Pixmap> {
        self.slots
            .get(&(track, surface))
            .and_then(|s| s.tex.pixmap())
    }
}
