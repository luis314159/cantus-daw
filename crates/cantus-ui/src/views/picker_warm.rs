//! The device picker's pictures made ahead (round 41's leftover, BACKLOG §6j; round 42):
//! each real-look picture was drawn from its device's own editor when its tile was first
//! asked for, so on a fresh profile the tiles further down showed their card for a moment
//! as one scrolled (FL shows every picture at once). Now, once the window has been quiet a
//! while after start-up, the pictures are made at background priority — one editor at a
//! time on a worker of its own (the interface thread only uploads each finished picture: a
//! capture drawn on it stalled the window up to 0.9 s), after anything a tile asks for —
//! the most used devices first (the recent ones, the
//! favourites, then the catalogue's order): those the atlas's page holds are kept in it, the
//! rest written to the disk cache, read back in a frame when their tile asks. With every
//! picture made the warm-up stops and asks for no frame: the frame budget at rest is
//! unchanged (`tests/picker_warm.rs`). r42:tuning-face

use super::picker_snap::Subject;
use crate::app::CantusApp;
use crate::picker::PickChoice;

/// When the warm-up may run.
#[derive(Clone, Debug)]
pub struct Warm {
    /// Seconds after the first frame before it starts (the start-up's own work first).
    pub start_delay: f64,
    /// Seconds without input before each step (it never competes with a hand on the mouse).
    pub quiet: f64,
    started: Option<f64>,
    last_input: f64,
    queued: bool,
    /// Every picture made or found (until the look changes).
    pub done: bool,
}

impl Default for Warm {
    fn default() -> Self {
        Warm {
            start_delay: 4.0,
            quiet: 1.5,
            started: None,
            last_input: 0.0,
            queued: false,
            done: false,
        }
    }
}

/// The pictures in the order to make them: the recently chosen and the favourites first,
/// then the catalogue as the picker lists it (instruments, then effects).
pub fn order(app: &mut CantusApp) -> Vec<Subject> {
    let sounds = app.samples.content().instruments();
    app.picker_view.refresh(&app.plugins.db, &sounds);
    let v = &app.picker_view;
    let subject_of = |key: &str| v.device(key).and_then(super::picker_grid::subject);
    let mut out: Vec<Subject> = Vec::new();
    let mut push = |s: Option<Subject>| {
        if let Some(s) = s {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    };
    for k in &v.prefs.recent {
        push(subject_of(k));
    }
    for k in &v.prefs.favourites {
        push(subject_of(k));
    }
    for d in &v.catalog {
        if matches!(d.choice, PickChoice::Instrument(_) | PickChoice::Pulsus) {
            push(super::picker_grid::subject(d));
        }
    }
    for d in &v.catalog {
        push(super::picker_grid::subject(d));
    }
    out
}

/// Once a frame the app paints (never a frame of its own — r42 fix-ui: round 38's rest,
/// `idle_repaint_e2e`): the list queued once the start-up is under way, the runner started
/// on it (`Thumbs::warm_start`), and its gate set — open at the instant the start-up's delay
/// and a quiet hand allow, closed while the song plays or records or the picker is open.
/// At rest no frame is painted and the runner goes on by itself; what it finished is taken
/// in by the next frame painted.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.picker_view.thumbs.enabled || app.picker_warm.done {
        return;
    }
    let gate = app.picker_view.thumbs.gate();
    // The picker open pumps its pictures itself, the runner's among them: it waits, and a
    // picture it finishes asks for its frame (a tile shows it).
    if app.picker.is_some() {
        if let Some(g) = &gate {
            g.close();
            g.set_wake(true);
        }
        return;
    }
    let (now, input) = ctx.input(|i| {
        let busy = !i.events.is_empty() || i.pointer.any_down() || i.pointer.is_moving();
        (i.time, busy)
    });
    let started = *app.picker_warm.started.get_or_insert(now);
    if input {
        app.picker_warm.last_input = now;
    }
    if let Some(g) = &gate {
        g.set_wake(false);
    }
    // r42 fix-ui (review ui#12): never while the song plays or records — the pictures can
    // wait, the audio's cores and caches cannot; the quiet is counted again from the stop.
    if app.engine.is_playing() || app.is_recording() {
        app.picker_warm.last_input = now;
        if let Some(g) = &gate {
            g.close();
        }
        return;
    }
    if !app.picker_warm.queued && now - started >= QUEUE_AFTER {
        app.picker_warm.queued = true;
        let subjects = order(app);
        app.picker_view.thumbs.warm(subjects);
    }
    app.picker_view.thumbs.pump(ctx);
    if app.picker_view.thumbs.gate().is_none() && app.picker_view.thumbs.warm_left() > 0 {
        // The runner draws in the look this thread draws in, as it is now.
        let s = &app.settings;
        let look = super::picker_thumbs::WorkerLook {
            faces: s.unit_faces,
            colourways: s
                .unit_colourways
                .iter()
                .map(|(u, c)| (u.clone(), c.clone()))
                .collect(),
            views: s
                .unit_views
                .iter()
                .map(|(u, v)| (u.clone(), v.clone()))
                .collect(),
            locale: crate::i18n::locale(),
        };
        app.picker_view.thumbs.set_worker_look(look);
        app.picker_view.thumbs.warm_start(ctx);
    }
    let wait_start = started + app.picker_warm.start_delay - now;
    let wait_quiet = app.picker_warm.last_input + app.picker_warm.quiet - now;
    if let Some(g) = app.picker_view.thumbs.gate() {
        g.open_in(wait_start.max(wait_quiet));
    }
    if app.picker_warm.queued
        && app.picker_view.thumbs.warm_left() == 0
        && app.picker_view.thumbs.idle()
        && app.picker_view.thumbs.gate().is_none()
    {
        app.picker_warm.done = true;
    }
}

/// Seconds after the first frame before the list is made (the start-up's first frames
/// first); the runner itself waits for [`Warm::start_delay`].
const QUEUE_AFTER: f64 = 0.5;
