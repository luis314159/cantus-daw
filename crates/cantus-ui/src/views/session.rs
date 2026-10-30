//! **The Session pane** (round 33, ADR 0077): Ableton Live's and Bitwig's clip launcher in
//! Cantus — the arrangement's tracks as columns (their colours, names, arm, mute, solo),
//! the scenes as rows with their launchers in the master column at the right, each slot
//! showing its clip (colour, name, a miniature of its notes or waveform, where it plays,
//! queued blinking, recording red) or, empty, its track's stop or record button; the stop
//! row and *stop all* under them; the global launch quantisation and arrangement record in
//! the pane's header; a chosen clip's panel (launch, loop and start, follow actions, its
//! own envelopes) at the bottom.
//!
//! - **The model is the session builder's** (`cantus_core::session`, ADR 0076): every
//!   edit here is one of its commands or batch builders — one undo step each.
//! - **What plays is the engine's**: the view tells it (`SessionMsg`) and folds what it
//!   hears back (`SessionEvent`, [`live`]) once a frame — never polled, never undone.
//! - **Keyboard** (keymap 14): the arrows move a slot cursor while the pane has the focus,
//!   Enter launches the slot, Shift+Enter its scene, and the session's actions — stop the
//!   track, stop all, record into the slot, arrangement record, back to arrangement,
//!   capture scene, the view's switch (Tab) — are actions like any other ([`keys`]).
//! - **MIDI learn**: slots, scenes, stop buttons, stop all, the scene cursor and the global
//!   quantisation are learnable like any control, with feedback ([`learn`]).
//! - **Controller grids** mirror it (`crate::controllers::grid`): the session ring they
//!   show is drawn here as a box.

use crate::app::CantusApp;
use cantus_core::session::{SceneId, SlotRef};
use cantus_core::{Command, Ticks, TrackId};
use cantus_engine::messages::{EngineMessage, SessionMsg};
use std::collections::HashMap;

#[path = "session_clip.rs"]
pub mod clip;
#[path = "session_demo.rs"]
pub mod demo;
#[path = "session_drop.rs"]
pub mod drop;
#[path = "session_env.rs"]
pub mod env;
#[path = "session_input.rs"]
mod input;
#[path = "session_keys.rs"]
pub mod keys;
#[path = "session_layout.rs"]
pub mod layout;
#[path = "session_learn.rs"]
pub mod learn;
#[path = "session_live.rs"]
pub mod live;
#[path = "session_menu.rs"]
mod menu;
#[path = "session_paint.rs"]
pub mod paint;
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

pub use cantus_core::session::SessionCommand;
pub use keys::action;

/// A session clip copied: its place relative to the cursor's slot, and its track.
#[derive(Clone, Debug, PartialEq)]
pub struct Copied {
    pub dx: i32,
    pub dy: i32,
    pub clip: cantus_core::session::SessionClip,
    pub from: TrackId,
}

/// A launcher held down (the mouse's press), released on the pointer's release.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pressed {
    Slot(SlotRef),
    Scene(SceneId),
}

/// A clip being dragged out of a slot.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SlotDrag {
    pub from: SlotRef,
    pub press: egui::Pos2,
    /// Moved far enough to be a drag (not a click).
    pub moving: bool,
}

/// A name being typed in place.
#[derive(Clone, Debug, PartialEq)]
pub enum Rename {
    Clip(SlotRef, String),
    Scene(SceneId, String),
}

/// An audio clip's waveform as kept: the revision and column count it was made at, and its
/// min/max columns.
pub type Wave = (u64, usize, Vec<(f32, f32)>);

/// The Session pane's state.
pub struct SessionView {
    /// What plays, as the engine said.
    pub live: live::Live,
    /// The keyboard's slot cursor (also the chosen slot: its clip's panel shows).
    pub cursor: Option<SlotRef>,
    /// The pane has the keyboard's focus (the last press of the pointer was in it).
    pub focused: bool,
    /// How far the grid is scrolled (points at 100 %).
    pub scroll: egui::Vec2,
    /// Every column's width (points at 100 %): a drag on any head's edge sets them all.
    pub col_w: f32,
    /// The clip panel: shown, and its height (points).
    pub panel_open: bool,
    pub panel_h: f32,
    pub drag: Option<SlotDrag>,
    pub rename: Option<Rename>,
    /// Session clips copied (Ctrl+C): each with its place relative to the first.
    /// Each with the track it came from (its envelopes follow it to another track).
    pub clipboard: Vec<Copied>,
    /// The scene copied with its row (a scene chosen when copied): pasted back as a scene.
    pub clipboard_scene: Option<cantus_core::session::Scene>,
    /// The per-clip envelope editor's state.
    pub env: env::EnvState,
    /// The last frame's layout (drop targets, the tests).
    pub geo: Option<layout::Geometry>,
    /// The session rings the grids show, drawn as boxes: (first track, first scene,
    /// columns, rows).
    pub rings: Vec<(usize, usize, usize, usize)>,
    /// What the view sent the engine, when the tests ask to see it.
    pub sent: Option<Vec<SessionMsg>>,
    /// The project (`CantusApp::epoch`) the whole state was last asked for.
    resent: Option<u64>,
    /// The pane drew this frame.
    pub(crate) drawn_pass: Option<u64>,
    /// A divider or the panel's edge being dragged: where it started.
    resize: Option<(layout::Hit, f32, f32)>,
    /// The launcher the pointer holds.
    pub pressed: Option<Pressed>,
    /// The scene (its launcher) was chosen last, not the slot: Del, Ctrl+D, F2 and the
    /// colour act on the scene.
    pub scene_chosen: bool,
    /// What a right-click opened the menu on.
    pub menu_on: Option<layout::Hit>,
    /// The track colours as the cells draw them.
    pub tints: crate::views::clips_chrome::Tints,
    /// The audio clips' waveforms: per clip, the revision and columns they were made at.
    pub waves: HashMap<cantus_core::ClipId, Wave>,
    /// The launcher a key holds (Enter, Shift+Enter): let go with the key.
    pub key_held: Option<Pressed>,
    /// A controller's mapped action is running (it needs no focus on the pane).
    pub controller_fire: bool,
    /// The pass the arrangement's timeline was last drawn in (a marked line there): a
    /// timeline not drawn last pass keeps no geometry a drop could land on.
    pub arrangement_pass: Option<u64>,
}

impl Default for SessionView {
    fn default() -> Self {
        SessionView {
            live: live::Live::default(),
            cursor: None,
            focused: false,
            scroll: egui::Vec2::ZERO,
            col_w: layout::COL_W,
            panel_open: true,
            panel_h: layout::PANEL_H,
            drag: None,
            rename: None,
            clipboard: Vec::new(),
            clipboard_scene: None,
            env: env::EnvState::default(),
            geo: None,
            rings: Vec::new(),
            sent: None,
            resent: None,
            drawn_pass: None,
            resize: None,
            pressed: None,
            scene_chosen: false,
            menu_on: None,
            tints: Default::default(),
            waves: HashMap::new(),
            key_held: None,
            controller_fire: false,
            arrangement_pass: None,
        }
    }
}

impl SessionView {
    /// Keep what the view sends, for a test to read.
    pub fn record_sent(&mut self) {
        self.sent = Some(Vec::new());
    }
}

/// Tell the engine.
pub fn send(app: &mut CantusApp, msg: SessionMsg) {
    if let Some(log) = app.session_view.sent.as_mut() {
        log.push(msg);
    }
    app.engine.send(EngineMessage::Session(msg));
}

/// A slot's launcher pressed (the mouse, a key, a pad at `velocity`).
pub fn launch(app: &mut CantusApp, slot: SlotRef, velocity: f32) {
    // Shown queued at once: the conductor answers when the press is planned.
    if app.project().slot_clip(slot).is_some() {
        let now = app.engine.position();
        app.session_view.live.press(slot, now);
    }
    send(app, SessionMsg::Launch { slot, velocity });
}

/// A slot's launcher let go.
pub fn release(app: &mut CantusApp, slot: SlotRef) {
    send(app, SessionMsg::Release { slot });
}

/// A scene's launcher pressed.
pub fn launch_scene(app: &mut CantusApp, scene: SceneId, velocity: f32) {
    let now = app.engine.position();
    app.session_view.live.press_scene(scene, now);
    send(app, SessionMsg::LaunchScene { scene, velocity });
}

pub fn release_scene(app: &mut CantusApp, scene: SceneId) {
    send(app, SessionMsg::ReleaseScene { scene });
}

pub fn stop_track(app: &mut CantusApp, track: TrackId) {
    send(app, SessionMsg::StopTrack { track });
}

pub fn stop_all(app: &mut CantusApp) {
    send(app, SessionMsg::StopAll);
}

pub fn back_to_arrangement(app: &mut CantusApp, track: Option<TrackId>) {
    send(app, SessionMsg::BackToArrangement { track });
}

/// Record into a slot: refused (said) when its track is not armed.
pub fn record(app: &mut CantusApp, slot: SlotRef) {
    let armed = app.project().track(slot.track).is_some_and(|t| t.armed);
    if !armed {
        app.set_status(crate::i18n::t("session.status.arm_first"));
        return;
    }
    if matches!(
        app.session_view.live.phase(slot),
        cantus_engine::messages::SlotPhase::Recording { .. }
            | cantus_engine::messages::SlotPhase::RecordQueued { .. }
    ) {
        send(app, SessionMsg::StopRecording { slot });
    } else {
        send(app, SessionMsg::Record { slot });
    }
}

/// A slot's launcher pressed, as the mouse, a key, a pad and a learnt control all press it:
/// an empty slot of an armed track records into it; anything else launches (a clip, an
/// empty slot's stop, a recording slot's take ended at the boundary and played on).
pub fn press_slot(app: &mut CantusApp, slot: SlotRef, velocity: f32) {
    let empty = app.project().slot_clip(slot).is_none();
    let armed = app.project().track(slot.track).is_some_and(|t| t.armed);
    let recording = matches!(
        app.session_view.live.phase(slot),
        cantus_engine::messages::SlotPhase::Recording { .. }
            | cantus_engine::messages::SlotPhase::RecordQueued { .. }
    );
    if empty && armed && !recording {
        record(app, slot);
    } else {
        launch(app, slot, velocity);
    }
}

/// Arrangement record on or off (the conductor answers, and commits what it heard).
pub fn set_arrangement_record(app: &mut CantusApp, on: bool) {
    send(app, SessionMsg::SetArrangementRecord(on));
}

/// Fresh ids, as the command bus gives them.
pub(crate) fn with_ids<R>(
    app: &mut CantusApp,
    f: impl FnOnce(&cantus_core::Project, cantus_core::session::edit::Ids) -> R,
) -> R {
    let project = app.project().clone();
    let bus = &mut app.bus;
    let mut ids = || bus.alloc_id();
    f(&project, &mut ids)
}

/// Run a session command (one undo step).
pub fn exec(app: &mut CantusApp, c: cantus_core::session::SessionCommand) -> bool {
    app.exec(Command::Session(c))
}

/// Every frame, whether the pane shows or not: what the engine said folded in, a lost
/// state asked for again, the grids driven. (What arrangement record heard is committed
/// by the session's conductor, `crate::session_play`.)
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    let events = app.engine.take_session_events();
    // Another project in this one's place: what played is forgotten, and asked again.
    if app.session_view.resent != Some(app.epoch) {
        if app.session_view.resent.is_some() {
            app.session_view.live.clear();
            app.session_view.cursor = None;
            app.session_view.rename = None;
        }
        app.session_view.resent = Some(app.epoch);
        if app.project().has_session() {
            send(app, SessionMsg::Resend);
        }
    }
    if !events.is_empty() {
        for e in events {
            app.session_view.live.apply(e);
        }
        ctx.request_repaint();
    }
    app.session_view.live.expire(std::time::Instant::now());
    // A launcher a key held: let go with the key (a gate clip stops, a repeat ends).
    if let Some(held) = app.session_view.key_held {
        let action = match held {
            Pressed::Slot(_) => crate::keymap::Action::SessionLaunch,
            Pressed::Scene(_) => crate::keymap::Action::SessionLaunchScene,
        };
        let keys: Vec<egui::Key> = app
            .keymap
            .bindings_for(action)
            .iter()
            .map(|b| b.key)
            .collect();
        if !ctx.input(|i| keys.iter().any(|k| i.key_down(*k))) {
            app.session_view.key_held = None;
            match held {
                Pressed::Slot(s) => release(app, s),
                Pressed::Scene(s) => release_scene(app, s),
            }
        }
    }
    if std::mem::take(&mut app.session_view.live.overflowed) {
        app.session_view.live.clear();
        send(app, SessionMsg::Resend);
    }
    // The pane lost the focus when it was not drawn (a hidden tab).
    let pass = ctx.cumulative_pass_nr();
    // Not drawn last pass (a hidden tab): no focus, and no slots a drop could land in.
    if app.session_view.drawn_pass.is_none_or(|p| p + 1 < pass) {
        app.session_view.focused = false;
        app.session_view.geo = None;
    }
    // The same for the arrangement behind the Session tab: a clip let go over where its
    // timeline was must not land in it.
    if app
        .session_view
        .arrangement_pass
        .is_none_or(|p| p + 1 < pass)
    {
        app.arrangement.forget_geometry();
    }
    if app.session_view.live.blinking() {
        ctx.request_repaint_after(std::time::Duration::from_millis(60));
    }
    crate::controllers::grid::frame(app);
}

/// The pane.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    app.session_view.drawn_pass = Some(ui.ctx().cumulative_pass_nr());
    input::pane(app, ui);
}

/// The scene ids, top to bottom.
pub fn scene_ids(app: &CantusApp) -> Vec<SceneId> {
    app.project().session.scenes.iter().map(|s| s.id).collect()
}

/// The track ids, left to right (the arrangement's order).
pub fn track_ids(app: &CantusApp) -> Vec<TrackId> {
    app.project().tracks.iter().map(|t| t.id).collect()
}

/// The transport's tick the view draws at (what is heard).
pub fn now(app: &CantusApp) -> Ticks {
    app.engine.position()
}
