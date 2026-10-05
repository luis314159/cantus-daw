//! The selected band's two-sided curve: level in across, level out up. The threshold
//! points slide along the curve, each side's slope is dragged up or down for its ratio,
//! and the band's level now is a dot riding on it.

use super::super::display::write_ratio;
use super::super::{key, State};
use super::{band_ink, curve_pos, Grab, CURVE_LO};
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::{with_brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::frame;
use crate::views::fx::host::FxHost;
use cantus_dsp::culmen::{telemetry, MaxSettings};
use cantus_dsp::multiplex;
use egui::{pos2, Align2, Pos2, Rect, Sense};
use std::cell::RefCell;

const TICKS: [f32; 5] = [-12.0, -24.0, -36.0, -48.0, -60.0];

thread_local! {
    static PILL: RefCell<String> = RefCell::new(String::with_capacity(16));
}

/// The level in at a point of the plot.
fn level_at(p: Pos2, r: Rect) -> (f32, f32) {
    let span = -CURVE_LO;
    (
        CURVE_LO + (p.x - r.left()) / r.width().max(1.0) * span,
        CURVE_LO + (r.bottom() - p.y) / r.height().max(1.0) * span,
    )
}

pub fn curve(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ms: &MaxSettings,
    tel: &[f32; 64],
    outer: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let band = st.selected.min(3);
    let b = ms.bands[band];
    // A square plot, centred in the display.
    let side = outer.width().min(outer.height()) - 8.0 * k;
    let r = Rect::from_center_size(outer.center(), egui::vec2(side, side));
    st.geometry.curve = r;
    let color = band_ink(band, s);
    let out_of = |level: f32| multiplex::transfer_db(level, &b) + b.gain_db;
    let thr_p = curve_pos(r, b.thr_db, out_of(b.thr_db));
    let lo_p = curve_pos(r, b.thr_lo_db, out_of(b.thr_lo_db));
    let resp = ui.interact(
        outer,
        ui.id().with(("max-curve", band)),
        Sense::click_and_drag(),
    );
    let hover = resp.hover_pos();
    let pick = |p: Pos2| -> Option<Grab> {
        if !r.expand(6.0 * k).contains(p) {
            return None;
        }
        if p.distance(thr_p) < 9.0 * k {
            return Some(Grab::Thr(false));
        }
        if p.distance(lo_p) < 9.0 * k {
            return Some(Grab::Thr(true));
        }
        let (lv, _) = level_at(p, r);
        let on_curve = (curve_pos(r, lv, out_of(lv)).y - p.y).abs() < 10.0 * k;
        if on_curve && lv > b.thr_db {
            Some(Grab::Ratio(false))
        } else if on_curve && lv < b.thr_lo_db {
            Some(Grab::Ratio(true))
        } else {
            None
        }
    };
    let hot = st.drag.or(hover.and_then(pick));
    let level_now = tel[telemetry::LEVELS + band];
    let gain_now = st.history.back().map_or(0.0, |c| c.bands_db[band]);
    with_brush(&ui.painter_at(outer), |br| {
        canvas::ground_b(br, outer, &ink, s.hue_tok);
        let px = br.px();
        for t in TICKS {
            let p = curve_pos(r, t, t);
            br.vline(br.snap_mid(p.x), r.top(), r.bottom(), px, ink.grid);
            br.hline(r.left(), r.right(), br.snap_mid(p.y), px, ink.grid);
        }
        br.segment(
            curve_pos(r, CURVE_LO, CURVE_LO),
            curve_pos(r, 0.0, 0.0),
            px,
            ink.grid_strong,
            false,
        );
        // The sides' regions: above the threshold, below the lower one.
        let upper = Rect::from_min_max(pos2(thr_p.x, r.top()), r.max);
        let lower = Rect::from_min_max(r.min, pos2(lo_p.x, r.bottom()));
        if b.ratio != 1.0 {
            br.rect(upper, Fill::Solid(Ink::a(color, 0.08)));
        }
        if b.ratio_lo != 1.0 {
            br.rect(lower, Fill::Solid(Ink::a(color, 0.05)));
        }
        let mut pts = [Pos2::ZERO; 121];
        for (i, p) in pts.iter_mut().enumerate() {
            let lv = CURVE_LO * (1.0 - i as f32 / 120.0);
            *p = curve_pos(r, lv, out_of(lv));
        }
        br.polyline(
            &pts,
            if b.on { 2.0 } else { 1.0 } * k,
            if b.on { color } else { ink.dim },
        );
        for (p, grab) in [(thr_p, Grab::Thr(false)), (lo_p, Grab::Thr(true))] {
            let lit = hot == Some(grab);
            br.circle(p, if lit { 6.0 } else { 4.5 } * k, Fill::Solid(color));
            br.circle_stroke(p, if lit { 6.0 } else { 4.5 } * k, px, ink.ground);
        }
        // The band now: its level in, and what comes out.
        if level_now > CURVE_LO - 20.0 {
            let p = curve_pos(r, level_now, level_now + gain_now + b.gain_db);
            br.circle(p, 4.0 * k, Fill::Solid(ink.text));
        }
        for t in TICKS {
            let p = curve_pos(r, t, CURVE_LO);
            canvas::putf(
                br,
                pos2(p.x, r.bottom() - 2.0 * k),
                Align2::CENTER_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                Some(ink.plate),
                format_args!("{t:.0}"),
            );
        }
        // The two sides, named on plates by their points.
        PILL.with(|cell| {
            let mut pill = cell.borrow_mut();
            pill.clear();
            write_ratio(b.ratio, &mut pill);
            let at = pos2(r.left() + 6.0 * k, r.top() + 6.0 * k);
            let row = canvas::tag(
                br,
                at,
                Align2::LEFT_TOP,
                frame::tr("max.above"),
                ink.text,
                tk,
                Some(ink.plate),
                format_args!("{:.1} dB \u{b7} {}", b.thr_db, pill),
            );
            pill.clear();
            write_ratio(b.ratio_lo, &mut pill);
            let at = pos2(r.left() + 6.0 * k, row.bottom() + 4.0 * k);
            canvas::tag(
                br,
                at,
                Align2::LEFT_TOP,
                frame::tr("max.below"),
                ink.text,
                tk,
                Some(ink.plate),
                format_args!("{:.1} dB \u{b7} {}", b.thr_lo_db, pill),
            );
        });
        canvas::edge_b(br, outer, &ink);
    });

    // Gestures.
    if resp.drag_started() {
        let at = ui.input(|i| i.pointer.press_origin()).or(hover);
        st.drag = at.and_then(pick);
    }
    if let (Some(g), Some(p)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let (lv, out) = level_at(p, r);
            let out = out - b.gain_db;
            match g {
                Grab::Thr(false) => {
                    let v = ((lv * 10.0).round() / 10.0).clamp(b.thr_lo_db.max(-60.0), 0.0);
                    host.set(key(band, "thr"), v, Some("curve"));
                }
                Grab::Thr(true) => {
                    let v = ((lv * 10.0).round() / 10.0).clamp(-90.0, b.thr_db);
                    host.set(key(band, "thr_lo"), v, Some("curve"));
                }
                Grab::Ratio(false) if lv > b.thr_db + 0.5 => {
                    let slope = (out - b.thr_db) / (lv - b.thr_db);
                    host.set(key(band, "ratio"), ratio_of(slope), Some("curve"));
                }
                Grab::Ratio(true) if lv < b.thr_lo_db - 0.5 => {
                    let slope = (out - b.thr_lo_db) / (lv - b.thr_lo_db);
                    host.set(key(band, "ratio_lo"), ratio_of(slope), Some("curve"));
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() && matches!(st.drag, Some(Grab::Thr(_) | Grab::Ratio(_))) {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        match hover.and_then(pick) {
            Some(Grab::Ratio(false)) => host.set(key(band, "ratio"), 1.0, None),
            Some(Grab::Ratio(true)) => host.set(key(band, "ratio_lo"), 1.0, None),
            _ => {}
        }
    }
    match hot {
        Some(Grab::Thr(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
        Some(Grab::Ratio(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical),
        _ => {}
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("max.card.curve"),
            frame::tr("max.curve_tip"),
            Caps::NONE,
        );
    }
}

/// A ratio from a slope of output against input: 1 / slope, kept in the parameter's range
/// and rounded as the cell prints it.
pub fn ratio_of(slope: f32) -> f32 {
    let r = if slope <= 0.01 { 100.0 } else { 1.0 / slope };
    let r = r.clamp(0.1, 100.0);
    if r >= 1.0 {
        (r * 10.0).round() / 10.0
    } else {
        // 1:x is printed to a tenth of x.
        1.0 / ((1.0 / r * 10.0).round() / 10.0)
    }
}
