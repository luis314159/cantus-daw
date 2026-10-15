//! **The pitch editor** (round 28, ADR 0054): an audio clip opened in the piano roll's
//! place — Melodyne's way of seeing a voice. The take's notes are **blobs** on a
//! piano-roll grid over its waveform, each as thick as the voice is loud, on the row of
//! the note it sounds; the **pitch curve** runs through them (what was sung, dim, under
//! what will sound). Sibilants and breaths are marked along the bottom.
//!
//! - **Edit** — click a blob to select it (Shift adds); drag it up or down to move its
//!   pitch (snapped to the key's scale, to semitones, or free — Alt for cents), sideways
//!   to move it in time; drag its ends to stretch it (its neighbours untouched); a
//!   double-click pulls its centre onto the note (or lets it go); drag on empty ground to
//!   select. ↑/↓ move the selection a step (Shift: an octave, Alt: 10 cents), Ctrl+A
//!   selects all, Ctrl+J joins, Q quantises, Esc lets the notes go (the keymap's actions,
//!   `CantusApp::pitch_action`).
//! - **Split** — click inside a blob to split it there.
//! - Below, **the note**: pitch centre, drift, vibrato, formant and gain of the selection;
//!   and **the macros**: correct pitch and correct drift by an amount, quantise timing to
//!   the grid.
//! - On the bar: the key (detected, editable), *Original* to compare (A/B), play from the
//!   selection, MIDI out (drag it onto an instrument track) and *Double*.
//!
//! Every edit is one `SetClipPitch`, merged over a drag: one undo step per gesture.

use crate::app::CantusApp;
use crate::kit::Style;
use crate::pitch::Take;
use cantus_core::pitch::PitchEdit;
use cantus_core::{ClipId, Ticks, TrackId};
use egui::{pos2, Pos2, Rect, Sense};

#[path = "pitch_bar.rs"]
mod bar;
#[path = "pitch_input.rs"]
mod input;
#[path = "pitch_paint.rs"]
mod paint;
#[path = "pitch_panel.rs"]
mod panel;

pub use panel::quantise;

/// The key strip's width, the ruler's height, the note panel's height (at 100 %).
const KEYS_W: f32 = 56.0;
const RULER_H: f32 = 22.0;
pub const PANEL_H: f32 = 112.0;
/// The rows' zoom range, points per semitone.
const ROW_MIN: f32 = 6.0;
const ROW_MAX: f32 = 40.0;

/// The editor's tools.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Edit,
    Split,
}

/// Where a pitch drag lands.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Snap {
    /// On the notes of the take's key.
    #[default]
    Scale,
    /// On semitones.
    Semitone,
    /// Anywhere (cents).
    Free,
}

impl Snap {
    pub const ALL: [Snap; 3] = [Snap::Scale, Snap::Semitone, Snap::Free];

    pub fn label_key(self) -> &'static str {
        match self {
            Snap::Scale => "pitch.snap.scale",
            Snap::Semitone => "pitch.snap.semitone",
            Snap::Free => "pitch.snap.free",
        }
    }
}

/// What a press in the canvas is doing.
#[derive(Clone, Debug)]
pub(crate) enum Gesture {
    /// Pressed on a blob, the direction not chosen yet.
    Press {
        id: u32,
        at_press: PitchEdit,
        from: Pos2,
    },
    Pitch {
        id: u32,
        at_press: PitchEdit,
        from: Pos2,
    },
    Move {
        at_press: PitchEdit,
        from: Pos2,
    },
    Stretch {
        id: u32,
        left: bool,
        at_press: PitchEdit,
        from: Pos2,
    },
    Marquee {
        from: Pos2,
        to: Pos2,
    },
}

/// Where everything was drawn: screen ↔ source frames and pitches.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geo {
    pub keys: Rect,
    pub ruler: Rect,
    pub grid: Rect,
    pub panel: Rect,
    /// Points per frame, and the frame at the grid's left edge.
    pub px_per_frame: f64,
    pub scroll: f64,
    /// Points per semitone, and the pitch (MIDI, fractional) at the grid's top edge.
    pub row_h: f32,
    pub top: f32,
}

impl Geo {
    pub fn x(&self, frame: f64) -> f32 {
        self.grid.left() + ((frame - self.scroll) * self.px_per_frame) as f32
    }

    pub fn frame(&self, x: f32) -> f64 {
        self.scroll + (x - self.grid.left()) as f64 / self.px_per_frame
    }

    pub fn y(&self, midi: f32) -> f32 {
        self.grid.top() + (self.top - midi) * self.row_h
    }

    pub fn midi(&self, y: f32) -> f32 {
        self.top - (y - self.grid.top()) / self.row_h
    }
}

/// The editor's state, kept between frames (`app.pitch.view`).
#[derive(Debug)]
pub struct PitchView {
    /// Fit the next clip shown to the editor (set when it is opened).
    pub fit_pending: bool,
    pub fitted: Option<ClipId>,
    pub px_per_frame: f64,
    pub scroll: f64,
    pub row_h: f32,
    pub top: f32,
    /// Selected notes, by id.
    pub selected: Vec<u32>,
    pub tool: Tool,
    pub snap: Snap,
    pub(crate) gesture: Option<Gesture>,
    /// The macros' amounts, 0 … 1.
    pub correct_pitch: f32,
    pub correct_drift: f32,
    /// The grid *quantise timing* snaps to.
    pub grid: Ticks,
    /// Published each frame: where things were drawn (the input and the tests aim with it).
    pub geo: Option<Geo>,
    pub blobs: Vec<(u32, Rect)>,
    /// The MIDI chip being dragged out.
    pub midi_drag: bool,
}

impl Default for PitchView {
    fn default() -> Self {
        PitchView {
            fit_pending: false,
            fitted: None,
            px_per_frame: 0.0,
            scroll: 0.0,
            row_h: 14.0,
            top: 72.0,
            selected: Vec::new(),
            tool: Tool::Edit,
            snap: Snap::Scale,
            gesture: None,
            correct_pitch: 1.0,
            correct_drift: 0.5,
            grid: Ticks(cantus_core::PPQ / 4),
            geo: None,
            blobs: Vec::new(),
            midi_drag: false,
        }
    }
}

/// Show the editor for an audio clip (the piano roll calls this).
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    let s = Style::current();
    let take = app.pitch_take(track, clip);
    let edit = app.pitch_edit(track, clip, &take);
    bar::show(app, ui, track, clip, &take, edit.as_ref());
    let full = ui.available_rect_before_wrap();
    if full.height() < 120.0 || full.width() < 200.0 {
        return;
    }
    ui.allocate_rect(full, Sense::hover());
    let panel_h = (crate::views::fx::controls::card_height(crate::kit::knob::KnobSize::M) + 8.0)
        .max(PANEL_H)
        * s.k;
    let panel_h = panel_h.min(full.height() * 0.5);
    let panel = Rect::from_min_max(pos2(full.left(), full.bottom() - panel_h), full.max);
    let body = Rect::from_min_max(full.min, pos2(full.right(), panel.top()));
    let keys = Rect::from_min_max(
        pos2(body.left(), body.top() + s.s(RULER_H)),
        pos2(body.left() + s.s(KEYS_W), body.bottom()),
    );
    let ruler = Rect::from_min_max(
        pos2(keys.right(), body.top()),
        pos2(body.right(), body.top() + s.s(RULER_H)),
    );
    // The scroll bars (that are also zooms, the piano roll's): under the rows and beside.
    let bar_w = s.s(crate::views::clips_chrome::BAR);
    let grid = Rect::from_min_max(
        pos2(keys.right(), ruler.bottom()),
        pos2(body.right() - bar_w, body.bottom() - bar_w),
    );
    let hbar = Rect::from_min_max(
        pos2(grid.left(), grid.bottom()),
        pos2(grid.right(), body.bottom()),
    );
    let vbar = Rect::from_min_max(
        pos2(grid.right(), grid.top()),
        pos2(body.right(), grid.bottom()),
    );
    let keys = Rect::from_min_max(keys.min, pos2(keys.right(), grid.bottom()));
    let Some(edit) = edit else {
        paint::waiting(ui, body, &take, &s);
        app.pitch.view.geo = None;
        if matches!(take, Take::Pending) {
            crate::views::workspace_ticker::wake_after(
                ui.ctx(),
                std::time::Duration::from_millis(60),
            );
        }
        return;
    };
    fit(app, track, clip, &edit, grid);
    input::navigate(app, ui, grid, keys);
    input::bars(app, ui, grid, hbar, vbar, &edit);
    let v = &app.pitch.view;
    let geo = Geo {
        keys,
        ruler,
        grid,
        panel,
        px_per_frame: v.px_per_frame,
        scroll: v.scroll,
        row_h: v.row_h,
        top: v.top,
    };
    app.pitch.view.geo = Some(geo);
    let analysis = match &take {
        Take::Ready(a) => Some(a.clone()),
        _ => None,
    };
    let blobs = paint::canvas(app, ui, track, clip, &edit, analysis.as_deref(), &geo, &s);
    app.pitch.view.blobs = blobs;
    input::canvas(app, ui, track, clip, &edit, analysis.as_deref(), &geo);
    panel::show(app, ui, track, clip, &edit, panel, &s);
    if matches!(take, Take::Pending) {
        crate::views::workspace_ticker::wake_after(ui.ctx(), std::time::Duration::from_millis(60));
    }
}

/// Fit the clip's window to the grid's width and its notes to its height, once per clip
/// opened (and whenever the editor is asked to).
fn fit(app: &mut CantusApp, track: TrackId, clip: ClipId, edit: &PitchEdit, grid: Rect) {
    let v = &app.pitch.view;
    if !(v.fit_pending || v.fitted != Some(clip) || v.px_per_frame <= 0.0) {
        return;
    }
    let (a, b) = match (
        app.clip_time(track, clip),
        app.project().track(track).and_then(|t| t.clip(clip)),
    ) {
        (Some(time), Some(c)) => {
            // Ordered: a clip played backwards maps its start to the window's far end.
            let (x, y) = (time.frame(c.start), time.frame(c.end()));
            (x.min(y).max(0.0), x.max(y).min(edit.frames as f64))
        }
        _ => (0.0, edit.frames as f64),
    };
    let (a, b) = if b > a {
        (a, b)
    } else {
        (0.0, edit.frames.max(1) as f64)
    };
    let span = (b - a) * 1.04;
    let (lo, hi) = edit.notes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), n| {
        (lo.min(n.out_centre()), hi.max(n.out_centre()))
    });
    let (lo, hi) = if lo <= hi {
        (lo - 3.0, hi + 3.0)
    } else {
        (48.0, 72.0)
    };
    let v = &mut app.pitch.view;
    v.fit_pending = false;
    v.fitted = Some(clip);
    v.selected.clear();
    v.gesture = None;
    v.px_per_frame = grid.width() as f64 / span.max(1.0);
    v.scroll = a - (b - a) * 0.02;
    v.row_h = (grid.height() / (hi - lo).max(6.0)).clamp(ROW_MIN, ROW_MAX);
    v.top = (lo + hi) * 0.5 + grid.height() / v.row_h * 0.5;
}

/// The notes under a screen rectangle (the marquee).
pub(crate) fn notes_in(blobs: &[(u32, Rect)], r: Rect) -> Vec<u32> {
    blobs
        .iter()
        .filter(|(_, b)| b.intersects(r))
        .map(|(id, _)| *id)
        .collect()
}
