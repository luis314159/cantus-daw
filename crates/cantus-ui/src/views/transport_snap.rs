//! The snap group (ADR 0052): the grid and the snap of the editor being worked in — the
//! arrangement, the piano roll or the channel rack (FL's global snap, ours following the
//! focus); in the piano roll's pane, the pitch editor (an audio clip, its own grid, always
//! snapping) or the automation editor (an automation clip, on the arrangement's grid). The editor's glyph, its grid (a menu) and its SNAP lamp in one row; the same values the
//! editor's own toolbar shows, so the two never disagree. The rack is always on its grid:
//! its lamp is lit and does not switch off.

use super::paint::{self as p, ink1, ink3};
use super::Style;
use crate::app::{CantusApp, CentralView, EditFocus};
use crate::i18n::t;
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::{with_brush, State};
use crate::views::workspace::Pane;
use crate::views::GRIDS;
use cantus_core::Ticks;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui};

/// The editor the bar's snap speaks for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Editor {
    Arrangement,
    PianoRoll,
    Rack,
    /// The pitch editor in the piano roll's pane (an audio clip, ADR 0054).
    Pitch,
    /// The automation editor in the piano roll's pane (an automation clip, ADR 0027).
    Automation,
}

impl Editor {
    fn title_key(self) -> &'static str {
        match self {
            Editor::Arrangement => Pane::Arrangement.title_key(),
            Editor::PianoRoll => Pane::PianoRoll.title_key(),
            Editor::Rack => Pane::ChannelRack.title_key(),
            Editor::Pitch => "topbar.editor.pitch",
            Editor::Automation => "topbar.editor.automation",
        }
    }
}

/// The editor being worked in: the half of the window the last edit happened in; the
/// mixer's focus leaves the arrangement's grid.
pub fn focused(app: &CantusApp) -> Editor {
    match app.focus {
        EditFocus::PianoRoll => match app.central {
            CentralView::ChannelRack => Editor::Rack,
            // The pane shows the editor of the clip chosen (`piano_roll::show`).
            _ => {
                let clip = app
                    .selected_track
                    .zip(app.selected_clip)
                    .and_then(|r| crate::clips::clip_of(app.project(), r));
                match clip {
                    Some(c) if c.audio.is_some() => Editor::Pitch,
                    Some(c) if c.automation.is_some() => Editor::Automation,
                    _ => Editor::PianoRoll,
                }
            }
        },
        _ => Editor::Arrangement,
    }
}

/// The grid and snap of `e`.
pub fn grid_of(app: &CantusApp, e: Editor) -> (Ticks, bool) {
    match e {
        Editor::Arrangement | Editor::Automation => (app.arrangement.grid, app.arrangement.snap),
        Editor::PianoRoll => (app.piano_roll.grid, app.piano_roll.snap),
        Editor::Rack => (app.rack.grid, true),
        Editor::Pitch => (app.pitch.view.grid, true),
    }
}

/// Set `e`'s grid.
pub fn set_grid(app: &mut CantusApp, e: Editor, grid: Ticks) {
    match e {
        Editor::Arrangement | Editor::Automation => app.arrangement.grid = grid,
        Editor::PianoRoll => app.piano_roll.grid = grid,
        // As the pitch bar's own menu: a grid picked is what Quantise moves notes to, no
        // longer another track's notes (review of fix3).
        Editor::Pitch => {
            app.pitch.view.grid = grid;
            app.pitch.view.quantise_to = None;
        }
        // As the rack's own grid menu does: from the pattern's start, fitted to the width.
        Editor::Rack => {
            app.rack.grid = grid;
            app.rack.scroll_steps = 0.0;
            app.rack.fit = true;
        }
    }
}

/// Switch `e`'s snap (the rack has none to switch).
pub fn toggle_snap(app: &mut CantusApp, e: Editor) {
    match e {
        Editor::Arrangement | Editor::Automation => app.arrangement.snap = !app.arrangement.snap,
        Editor::PianoRoll => app.piano_roll.snap = !app.piano_roll.snap,
        Editor::Rack | Editor::Pitch => {}
    }
}

fn grid_key(grid: Ticks) -> &'static str {
    GRIDS
        .iter()
        .find(|(_, g)| *g == grid.0)
        .map(|(k, _)| *k)
        .unwrap_or("grid.custom")
}

/// The grid cell's width: the sheet's 52, wider when the focused editor's grid (or the
/// track a pitch editor quantises to) needs it — never cut.
fn grid_w(app: &CantusApp, ctx: &egui::Context, s: &Style) -> f32 {
    let e = focused(app);
    let target = match e {
        Editor::Pitch => app.selected_track.and_then(|tr| app.quantise_target(tr)),
        _ => None,
    };
    let words = match target {
        Some(o) => {
            let name = app.project().track(o).map_or("", |t| t.name.as_str());
            p::measure(ctx, name, Role::Body, 12.0, s).min(120.0 * s.k)
        }
        None => p::measure_key(ctx, grid_key(grid_of(app, e).0), Role::Body, 12.0, s),
    };
    (words + 26.0 * s.k).max(52.0 * s.k)
}

/// The group's widths: compact (the magnet and a ▾) and full (the magnet and the grid).
pub fn widths(app: &CantusApp, ctx: &egui::Context, s: &Style) -> (f32, f32) {
    (40.0 * s.k, 28.0 * s.k + grid_w(app, ctx, s))
}

/// The magnet key and the grid in one well: snap on tints the magnet with the accent; the
/// grid's value and ▾ open the grid list. Compact, the ▾ alone opens it.
pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style, compact: bool) {
    let k = s.k;
    let e = focused(app);
    let (grid, _) = grid_of(app, e);
    let row = p::row(r, s);
    let gw = if compact {
        12.0 * k
    } else {
        grid_w(app, ui.ctx(), s)
    };
    let mr = p::cell(row, 0.0, 28.0 * k);
    let field = p::cell(row, 28.0 * k, gw);
    // The same group may be on the bar (compact) and in the More panel (its grid): ids per
    // layer.
    let layer = ui.layer_id().id;
    let sresp = ui.interact(mr, Id::new("toolbar-snap").with(layer), Sense::click());
    if sresp.clicked() {
        toggle_snap(app, e);
    }
    let resp = ui.interact(field, Id::new("toolbar-grid").with(layer), Sense::click());
    let popup = egui::Popup::default_response_id(&resp);
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    // The pitch editor quantising to another track's notes: that track's name, as its own
    // bar says, and no grid checked in the list (review of fix3).
    let target = match e {
        Editor::Pitch => app.selected_track.and_then(|tr| app.quantise_target(tr)),
        _ => None,
    };
    let value = match target {
        Some(o) => app.project().track(o).map_or("", |t| t.name.as_str()),
        None => crate::views::clips_chrome::word(grid_key(grid)),
    };
    let (_, snap_now) = grid_of(app, e);
    let mut st = State::of(&sresp, snap_now);
    if matches!(e, Editor::Rack | Editor::Pitch) {
        st.disabled = true;
        st.lit = true;
    }
    let gst = State::of(&resp, open);
    with_brush(ui.painter(), |b| {
        b.register(row, "toolbar_key", crate::kit::cap::WELL_KEY_TOKENS, &[]);
        p::joined(b, &[mr, field], s);
        p::tint(b, mr, s, st.lit(snap_now), s.t.highlight, 0.2);
        let ist = State {
            disabled: false,
            ..st
        };
        p::icon(b, mr, s, Glyph::Magnet, ist, s.t.highlight, 24.0);
        p::tint(b, field, s, gst.lit(false), s.t.highlight, 0.2);
        let chev_ink = if gst.hovered || open {
            ink1(s)
        } else {
            ink3(s)
        };
        if compact {
            p::chevron(b, field.center(), 2.0 * k, chev_ink, s);
        } else {
            let vr = Rect::from_min_max(
                Pos2::new(field.left() + 8.0 * k, field.top()),
                Pos2::new(field.right() - 16.0 * k, field.bottom()),
            );
            p::words(
                b,
                vr,
                value,
                Role::Body,
                12.0,
                ink1(s),
                Align2::LEFT_CENTER,
                s,
            );
            p::chevron(
                b,
                Pos2::new(field.right() - 10.0 * k, field.center().y),
                2.5 * k,
                chev_ink,
                s,
            );
        }
    });
    if gst.hovered {
        // The track is what Quantise moves to, not a grid (review of fix4): the hint says
        // so, as the pitch bar's caption does, and holds the whole name the field may cut.
        let what = if target.is_some() {
            t("pitch.quantise_to")
        } else {
            t("topbar.grid")
        };
        let name = format!("{what} \u{2014} {}", t(e.title_key()));
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
                let checked = if target.is_some() { Ticks(-1) } else { grid };
                if let Some(g) = crate::views::arrangement::grid_menu(ui, checked) {
                    set_grid(app, e, g);
                    ui.close();
                }
            });
    }
    // The key's words for a screen reader: its name and whether it is on.
    sresp.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            !st.disabled,
            snap_now,
            t("pianoroll.snap"),
        )
    });
    if sresp.hovered() {
        let tip = if e == Editor::Rack {
            t("topbar.snap_rack")
        } else if e == Editor::Pitch {
            t("topbar.snap_pitch")
        } else {
            t("topbar.snap_tip")
        };
        let tip = format!("{tip} \u{2014} {}", t(e.title_key()));
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        let _ = sresp.on_hover_text(tip);
    }
}
