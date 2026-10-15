//! The snap group (ADR 0052): the grid and the snap of the editor being worked in — the
//! arrangement, the piano roll or the channel rack (FL's global snap, ours following the
//! focus). The editor's name over its grid (a menu) and its SNAP lamp; the same values the
//! editor's own toolbar shows, so the two never disagree. The rack is always on its grid:
//! its lamp is lit and does not switch off.

use super::{Style, STEP};
use crate::app::{CantusApp, CentralView, EditFocus};
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, led, with_brush, State};
use crate::views::workspace::Pane;
use crate::views::GRIDS;
use cantus_core::Ticks;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The editor the bar's snap speaks for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Editor {
    Arrangement,
    PianoRoll,
    Rack,
}

impl Editor {
    pub const ALL: [Editor; 3] = [Editor::Arrangement, Editor::PianoRoll, Editor::Rack];

    fn pane(self) -> Pane {
        match self {
            Editor::Arrangement => Pane::Arrangement,
            Editor::PianoRoll => Pane::PianoRoll,
            Editor::Rack => Pane::ChannelRack,
        }
    }
}

/// The editor being worked in: the half of the window the last edit happened in; the
/// mixer's focus leaves the arrangement's grid.
pub fn focused(app: &CantusApp) -> Editor {
    match app.focus {
        EditFocus::PianoRoll => match app.central {
            CentralView::ChannelRack => Editor::Rack,
            _ => Editor::PianoRoll,
        },
        _ => Editor::Arrangement,
    }
}

/// The grid and snap of `e`.
pub fn grid_of(app: &CantusApp, e: Editor) -> (Ticks, bool) {
    match e {
        Editor::Arrangement => (app.arrangement.grid, app.arrangement.snap),
        Editor::PianoRoll => (app.piano_roll.grid, app.piano_roll.snap),
        Editor::Rack => (app.rack.grid, true),
    }
}

/// Set `e`'s grid.
pub fn set_grid(app: &mut CantusApp, e: Editor, grid: Ticks) {
    match e {
        Editor::Arrangement => app.arrangement.grid = grid,
        Editor::PianoRoll => app.piano_roll.grid = grid,
        Editor::Rack => app.rack.grid = grid,
    }
}

/// Switch `e`'s snap (the rack has none to switch).
pub fn toggle_snap(app: &mut CantusApp, e: Editor) {
    match e {
        Editor::Arrangement => app.arrangement.snap = !app.arrangement.snap,
        Editor::PianoRoll => app.piano_roll.snap = !app.piano_roll.snap,
        Editor::Rack => {}
    }
}

fn grid_key(grid: Ticks) -> &'static str {
    GRIDS
        .iter()
        .find(|(_, g)| *g == grid.0)
        .map(|(k, _)| *k)
        .unwrap_or("grid.custom")
}

fn parts(ctx: &egui::Context, s: &Style) -> (f32, f32, f32) {
    let tk = s.tk();
    let name = Editor::ALL
        .iter()
        .map(|e| {
            let key = e.pane().title_key();
            text::measure(ctx, &Text::key(key, Role::Micro, s.t.text).caps(true), tk).x
        })
        .fold(0.0, f32::max);
    let grid = GRIDS
        .iter()
        .map(|(k, _)| k)
        .chain(std::iter::once(&"grid.custom"))
        .map(|k| text::measure(ctx, &Text::key(k, Role::Value, s.t.text), tk).x)
        .fold(0.0, f32::max)
        + 26.0 * s.k;
    let snap = text::measure(
        ctx,
        &Text::key("pianoroll.snap", Role::Control, s.t.text).caps(true),
        tk,
    )
    .x + 30.0 * s.k;
    (name, grid, snap)
}

pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    let (name, grid, snap) = parts(ctx, s);
    name.max(grid + STEP * s.k + snap)
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let e = focused(app);
    let (grid, snap) = grid_of(app, e);
    let (_, grid_w, snap_w) = parts(ui.ctx(), s);
    let row_h = s.d(22.0);
    let label = Rect::from_min_size(
        Pos2::new(r.left(), r.center().y - row_h - 3.0 * k),
        Vec2::new(r.width(), row_h * 0.7),
    );
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            label,
            &Text::key(e.pane().title_key(), Role::Micro, s.t.text_dim)
                .caps(true)
                .align(Align2::LEFT_BOTTOM)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let field = Rect::from_min_size(
        Pos2::new(r.left(), r.center().y - row_h * 0.5 + 6.0 * k),
        Vec2::new(grid_w, row_h),
    );
    let resp = ui.interact(field, Id::new("toolbar-grid"), Sense::click());
    let popup = egui::Popup::default_response_id(&resp);
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let st = State::of(&resp, open);
    let value = crate::views::clips_chrome::word(grid_key(grid));
    with_brush(ui.painter(), |b| {
        kit::well::menu_field(b, field, s, st, value, false)
    });
    if st.hovered {
        let name = format!("{} \u{2014} {}", t("topbar.grid"), t(e.pane().title_key()));
        let caps = Caps {
            menu: true,
            ..Caps::NONE
        };
        hint::set_text(ui.ctx(), &name, value, caps);
    }
    if open || resp.clicked() {
        egui::Popup::menu(&resp)
            .frame(egui::Frame::NONE)
            .show(|ui| {
                if let Some(g) = crate::views::arrangement::grid_menu(ui, grid) {
                    set_grid(app, e, g);
                    ui.close();
                }
            });
    }
    // The SNAP lamp.
    let sr = Rect::from_min_size(
        Pos2::new(field.right() + STEP * k, field.top()),
        Vec2::new(snap_w, row_h),
    );
    let resp = ui.interact(sr, Id::new("toolbar-snap"), Sense::click());
    if resp.clicked() {
        toggle_snap(app, e);
    }
    let (_, snap_now) = grid_of(app, e);
    let mut st = State::of(&resp, snap_now);
    if e == Editor::Rack {
        st.disabled = true;
    }
    with_brush(ui.painter(), |b| {
        led::button(b, sr, s, st, Src::Key("pianoroll.snap"))
    });
    if st.hovered {
        let tip = if e == Editor::Rack {
            t("topbar.snap_rack")
        } else {
            t("topbar.snap_tip")
        };
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = resp.on_hover_text(tip);
    }
    let _ = snap;
}
