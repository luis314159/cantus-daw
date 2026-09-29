//! The morph pad on the kit (FM8's Morph Square): four corners, each a snapshot of the
//! operators and the matrix, and a puck that mixes them. Click a corner to store the patch
//! there, right-click to forget it; drag the puck (one undo step). The position is two
//! ordinary numbers, so it automates and takes modulation like any other — its two cells
//! sit under the pad.

use super::drag_values;
use super::parts::{self, c};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{on_fill, with_alpha};
use crate::kit::knob::KnobSize;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::inst::fm::{morph_snapshot, MORPH_CORNERS};
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// Corner *i*'s place on the pad (x right, y up), as fractions.
const CORNERS: [(f32, f32); 4] = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];

/// The pad's card: the pad bleeding to the edges, X and Y below.
pub(super) fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    // X and Y as cells below the pad when the card is tall enough for both; otherwise the
    // pad takes the card (it is the control) and prints where the puck is.
    let cells_h = parts::cell_size(KnobSize::S, s).y;
    let with_cells = rect.height() - cells_h - s.s(41.0) >= rect.width() * 0.8;
    let disp = if with_cells {
        (rect.height() - cells_h - s.s(24.0 + 1.0 + 16.0)) / s.k
    } else {
        rect.height() / s.k - 25.0
    };
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key("fm.morph"),
        None,
        None,
        Some(disp.max(40.0)),
    );
    if let Some(d) = cr.display {
        pad(app, ui, track, spec, s, d);
    }
    if !with_cells {
        return;
    }
    let pct = |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = write!(b, "{:.0} %", v * 100.0);
    };
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        parts::below_display(&cr, s),
        &[c("morph.x").fmt(&pct), c("morph.y").fmt(&pct)],
    );
}

fn pad(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    d: Rect,
) {
    let k = s.k;
    let h = s.hue;
    let side = d.width().min(d.height());
    let sq = Rect::from_center_size(d.center(), Vec2::splat(side));
    let inner = sq.shrink(22.0 * k);
    let resp = ui.interact(
        inner.expand(8.0 * k),
        ui.id().with("fm-morph"),
        Sense::click_and_drag(),
    );
    let at = |fx: f32, fy: f32| {
        Pos2::new(
            inner.left() + fx * inner.width(),
            inner.bottom() - fy * inner.height(),
        )
    };
    let corner_at = |fx: f32, fy: f32| {
        let r = sq.shrink(11.0 * k);
        Pos2::new(r.left() + fx * r.width(), r.bottom() - fy * r.height())
    };
    let (x, y) = (spec.get("morph.x"), spec.get("morph.y"));
    let any = MORPH_CORNERS.iter().any(|k| spec.text(k).is_some());
    // The corners: store on a click, forget on a right-click.
    let mut hovered = [false; 4];
    for (i, (cx, cy)) in CORNERS.iter().enumerate() {
        let r = Rect::from_center_size(corner_at(*cx, *cy), Vec2::splat(16.0 * k));
        let cresp = ui.interact(r, ui.id().with(("fm-morph-corner", i)), Sense::click());
        hovered[i] = cresp.hovered();
        let stored = spec.text(MORPH_CORNERS[i]).is_some();
        if cresp.clicked() {
            app.set_inst_text(track, MORPH_CORNERS[i], Some(morph_snapshot(spec)));
        }
        if cresp.secondary_clicked() && stored {
            app.set_inst_text(track, MORPH_CORNERS[i], None);
        }
        if cresp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &tf("fm.morph.corner_tip", &[("n", &(i + 1).to_string())]),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
    }
    if resp.dragged() || resp.clicked() {
        if let Some(ptr) = resp.interact_pointer_pos() {
            let nx = ((ptr.x - inner.left()) / inner.width()).clamp(0.0, 1.0);
            let ny = ((inner.bottom() - ptr.y) / inner.height()).clamp(0.0, 1.0);
            drag_values(app, track, &[("morph.x", nx), ("morph.y", ny)]);
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        app.end_gesture();
    }
    parts::display_hint(ui, &resp, "fm.morph.tip", "");
    with_brush(&ui.painter_at(d), |b| {
        for i in 0..=4 {
            let f = i as f32 / 4.0;
            let gx = b.snap_mid(inner.left() + f * inner.width());
            let gy = b.snap_mid(inner.top() + f * inner.height());
            let c = if i == 0 || i == 4 {
                s.t.grid_strong
            } else {
                s.t.grid
            };
            b.vline(gx, inner.top(), inner.bottom(), b.px(), c);
            b.hline(inner.left(), inner.right(), gy, b.px(), c);
        }
        for (i, (cx, cy)) in CORNERS.iter().enumerate() {
            let c = corner_at(*cx, *cy);
            let r = Rect::from_center_size(c, Vec2::splat(14.0 * k));
            let stored = spec.text(MORPH_CORNERS[i]).is_some();
            if stored {
                b.rrect(r, s.r(2.0), Fill::Solid(h));
            }
            b.rrect_stroke(
                r,
                s.r(2.0),
                1.5 * k,
                if hovered[i] { h } else { s.t.text_dim },
            );
            let ink = if stored { on_fill(h) } else { s.t.text };
            parts::say(
                b,
                r,
                s,
                &(i + 1).to_string(),
                Role::Micro,
                ink,
                Align2::CENTER_CENTER,
            );
        }
        let p = at(x, y);
        let pc = if any { h } else { s.t.text_dim };
        b.circle(p, 9.0 * k, Fill::Solid(with_alpha(pc, 0.18)));
        b.circle_stroke(p, 9.0 * k, 1.0 * k, pc);
        b.circle(p, 5.0 * k, Fill::Solid(pc));
        if !any {
            parts::chip(
                b,
                Rect::from_center_size(
                    inner.center(),
                    Vec2::new(inner.width() + 8.0 * k, 16.0 * k),
                ),
                s,
                &t("fm.morph.empty"),
                s.t.text_dim,
                Align2::CENTER_CENTER,
            );
        }
    });
}
