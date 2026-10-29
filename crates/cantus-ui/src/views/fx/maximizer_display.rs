//! Culmen's history (gain over time, with a lane per band) and its loudness card.

use super::bands::{band_ink, Grab};
use super::{target_of, State, BAND_KEYS, HISTORY, TARGETS};
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Brush, Fill, State as KitState, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::frame;
use crate::views::fx::host::FxHost;
use cantus_dsp::culmen::MaxSettings;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::fmt::Write as _;

/// The history's level scale, dBFS: a master lives in its top 30 dB.
pub const LOW_DB: f32 = -30.0;
/// The brickwall's reduction hangs from the top: this many dB fill a third of the plot.
pub const GR_SPAN: f32 = 12.0;
/// A band lane's half height holds this many dB of gain.
pub const LANE_SPAN: f32 = 6.0;
const LEVEL_TICKS: [f32; 6] = [-3.0, -6.0, -9.0, -12.0, -18.0, -24.0];

/// The height of a level on a level plot.
pub fn y_of(db: f32, r: Rect) -> f32 {
    r.bottom() - ((db - LOW_DB) / -LOW_DB).clamp(0.0, 1.0) * r.height()
}

fn db_at(y: f32, r: Rect) -> f32 {
    LOW_DB + (r.bottom() - y) / r.height().max(1.0) * -LOW_DB
}

/// `4.0:1` squeezes, `1:2.0` stretches, `∞:1` at the top of the range.
pub fn write_ratio(v: f32, out: &mut String) {
    let _ = if v >= 99.5 {
        write!(out, "\u{221e}:1")
    } else if v >= 1.0 {
        write!(out, "{v:.1}:1")
    } else {
        write!(out, "1:{:.1}", 1.0 / v.max(0.01))
    };
}

/// The bands drawn in lanes, in order: those in use, then the full band.
fn lanes(ms: &MaxSettings) -> impl Iterator<Item = usize> + '_ {
    (0..4).filter(|b| ms.band_in_use(*b))
}

/// Gain over time: the output's peaks (fill) over the input's (line), the brickwall's
/// reduction from the top, the ceiling you drag, and a lane per band below.
pub fn history(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ms: &MaxSettings,
    tel: &[f32; 64],
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("max-history"), Sense::click_and_drag());
    let n_lanes = lanes(ms).count();
    let lane_h = 18.0 * k;
    let head_h = 18.0 * k;
    let plot = Rect::from_min_max(
        pos2(rect.left(), rect.top() + head_h),
        pos2(rect.right(), rect.bottom() - lane_h * n_lanes as f32 - 1.0),
    );
    st.geometry.history = plot;
    let y = |d: f32| y_of(d, plot);
    let gy = |g: f32| plot.top() + (g / GR_SPAN).clamp(0.0, 1.0) * plot.height() * 0.34;
    let hist = &st.history;
    let n = hist.len().min(HISTORY);
    let dx = plot.width() / HISTORY as f32;
    let x0 = plot.right() - dx * n as f32;
    let x_of = |i: usize| x0 + dx * (i as f32 + 0.5);
    let ceiling = ms.limiter.ceiling_db;
    let cy = y(ceiling);
    let hover = resp.hover_pos();
    let near = hover.is_some_and(|p| plot.contains(p) && (p.y - cy).abs() < 6.0 * k);
    let hot = near || st.drag == Some(Grab::Ceiling);
    let mut pts = [Pos2::ZERO; HISTORY];
    let mut top = [Pos2::ZERO; HISTORY];
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        for g in LEVEL_TICKS {
            b.hline(plot.left(), plot.right(), b.snap_mid(y(g)), px, ink.grid);
        }
        for sec in 1..10 {
            let x = plot.right() - sec as f32 * 100.0 * dx;
            b.vline(b.snap_mid(x), plot.top(), rect.bottom(), px, ink.grid);
        }
        if n > 1 {
            // The output's peaks, filled; the input's as a line over them.
            for (i, c) in hist.iter().enumerate().take(n) {
                pts[i] = pos2(x_of(i), y(c.output_db));
            }
            b.area(&pts[..n], plot.bottom(), ink.fill(1.0));
            b.polyline(&pts[..n], 1.25 * k, ink.hue);
            for (i, c) in hist.iter().enumerate().take(n) {
                pts[i] = pos2(x_of(i), y(c.input_db));
            }
            b.polyline(&pts[..n], 1.0 * k, Ink::a(ink.text, 0.55));
            // The brickwall's reduction, from the top.
            for (i, c) in hist.iter().enumerate().take(n) {
                pts[i] = pos2(x_of(i), gy(c.limit_db));
                top[i] = pos2(x_of(i), plot.top());
            }
            b.between(&top[..n], &pts[..n], Ink::a(ink.danger, 0.30));
            b.polyline(&pts[..n], 1.25 * k, ink.danger);
        }
        // The ceiling: dashed, heavier when it can be taken.
        canvas::dashed(
            b,
            pos2(plot.left(), cy),
            pos2(plot.right(), cy),
            if hot { 2.0 } else { 1.0 } * k,
            ink.text,
            6.0 * k,
            4.0 * k,
        );
        // The lanes.
        let mut ly = plot.bottom() + 1.0;
        for band in lanes(ms) {
            let lr = Rect::from_min_size(pos2(rect.left(), ly), vec2(rect.width(), lane_h));
            ly += lane_h;
            let color = band_ink(band, s);
            b.rect(
                b.snap_rect(lr),
                Fill::Solid(Ink::a(
                    crate::views::fx::dynfx::on_display(&ink, s.t.deck_title),
                    0.55,
                )),
            );
            let mid = lr.center().y;
            b.hline(lr.left(), lr.right(), b.snap_mid(mid), px, ink.grid_strong);
            if n > 1 {
                for (i, c) in hist.iter().enumerate().take(n) {
                    let g = (c.bands_db[band] / LANE_SPAN).clamp(-1.0, 1.0);
                    pts[i] = pos2(x_of(i), mid - g * (lane_h * 0.5 - 1.0));
                    top[i] = pos2(x_of(i), mid);
                }
                b.between(&top[..n], &pts[..n], Ink::a(color, 0.35));
                b.polyline(&pts[..n], 1.25 * k, color);
            }
            let now = hist.back().map_or(0.0, |c| c.bands_db[band]);
            canvas::tag(
                b,
                pos2(lr.left() + 6.0 * k, mid),
                Align2::LEFT_CENTER,
                frame::tr(BAND_KEYS[band]),
                color,
                tk,
                Some(ink.plate),
                format_args!("{now:+.1} dB"),
            );
        }
        // Words last, so nothing paints over them.
        for g in LEVEL_TICKS {
            canvas::putf(
                b,
                pos2(plot.left() + 3.0 * k, y(g) - 1.0),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                Some(ink.plate),
                format_args!("{g:.0}"),
            );
        }
        canvas::tag(
            b,
            pos2(plot.left() + 40.0 * k, cy + 3.0 * k),
            Align2::LEFT_TOP,
            frame::tr("max.ceiling_line"),
            ink.text,
            tk,
            Some(ink.plate),
            format_args!("{ceiling:.1} dB"),
        );
        let mid = rect.top() + head_h * 0.5;
        let now = hist.back().copied().unwrap_or_default();
        let r = canvas::tag(
            b,
            pos2(rect.right() - 8.0 * k, mid),
            Align2::RIGHT_CENTER,
            frame::tr("max.card.brickwall"),
            ink.danger,
            tk,
            None,
            format_args!("\u{2212}{:.1} dB", now.limit_db.max(0.0)),
        );
        let r = canvas::tag(
            b,
            pos2(r.left() - 12.0 * k, mid),
            Align2::RIGHT_CENTER,
            frame::tr("max.tp_max"),
            ink.hue,
            tk,
            None,
            format_args!("{:.1} dB", now.output_db.max(-99.9)),
        );
        let _ = (r, tel);
        // Hover: that moment, on plates.
        if let Some(pt) = hover.filter(|p| rect.contains(*p)) {
            let i = ((pt.x - x0) / dx).floor();
            if i >= 0.0 && (i as usize) < n {
                let c = hist[i as usize];
                let ago = (n - i as usize) as f32 * 0.01;
                b.vline(
                    b.snap_mid(pt.x),
                    plot.top(),
                    rect.bottom(),
                    px,
                    ink.grid_strong,
                );
                let at = pos2(plot.left() + 8.0 * k, plot.bottom() - 6.0 * k);
                let r = canvas::tag(
                    b,
                    at,
                    Align2::LEFT_BOTTOM,
                    "",
                    ink.text,
                    tk,
                    Some(ink.plate),
                    format_args!("\u{2212}{ago:.2} s"),
                );
                let r = canvas::tag(
                    b,
                    pos2(r.right() + 10.0 * k, at.y),
                    Align2::LEFT_BOTTOM,
                    frame::tr("max.input"),
                    ink.text,
                    tk,
                    Some(ink.plate),
                    format_args!("{:.1} dB", c.input_db.max(-99.9)),
                );
                let r = canvas::tag(
                    b,
                    pos2(r.right() + 10.0 * k, at.y),
                    Align2::LEFT_BOTTOM,
                    frame::tr("max.tp_max"),
                    ink.hue,
                    tk,
                    Some(ink.plate),
                    format_args!("{:.1} dB", c.output_db.max(-99.9)),
                );
                canvas::tag(
                    b,
                    pos2(r.right() + 10.0 * k, at.y),
                    Align2::LEFT_BOTTOM,
                    frame::tr("max.card.brickwall"),
                    ink.danger,
                    tk,
                    Some(ink.plate),
                    format_args!("\u{2212}{:.1} dB", c.limit_db.max(0.0)),
                );
            }
        }
        if n == 0 {
            canvas::put_key(
                b,
                plot.center(),
                Align2::CENTER_CENTER,
                "max.empty",
                Role::Body,
                ink.dim,
                tk,
            );
        }
        canvas::edge_b(b, rect, &ink);
    });

    // The ceiling is dragged on the display; double-click puts it back at −1 dB.
    if resp.drag_started() {
        let at = ui.input(|i| i.pointer.press_origin()).or(hover);
        if at.is_some_and(|p| plot.contains(p) && (p.y - cy).abs() < 6.0 * k) {
            st.drag = Some(Grab::Ceiling);
        }
    }
    if let (Some(Grab::Ceiling), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let v = (db_at(pt.y, plot) * 10.0).round() / 10.0;
            host.set("ceiling", v.clamp(-30.0, 0.0), Some("ceiling"));
        }
    }
    if resp.drag_stopped() && st.drag == Some(Grab::Ceiling) {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() && near {
        host.set("ceiling", -1.0, None);
    }
    if near || st.drag == Some(Grab::Ceiling) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("max.card.history"),
            frame::tr("max.history_tip"),
            Caps::NONE,
        );
    }
}

/// A label | value pair in half a row: the name in caps at the left, the value in a well.
#[allow(clippy::too_many_arguments)]
pub(super) fn pair(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    label: Src<'_>,
    role: Role,
    color: egui::Color32,
    value: std::fmt::Arguments<'_>,
) {
    let k = s.k;
    let well_r = Rect::from_min_max(
        pos2(rect.right() - rect.width() * 0.56, rect.top()),
        rect.max,
    );
    let lr = Rect::from_min_max(rect.min, pos2(well_r.left() - 4.0 * k, rect.bottom()));
    let t = Text {
        src: label,
        role: Role::Micro,
        caps: true,
        align: Align2::LEFT_CENTER,
        policy: Policy::Abbreviate,
        color: s.t.label,
    };
    let _ = ktext::paint(b, lr, &t, s.tk());
    well::paint(b, well_r, s, KitState::REST, None);
    canvas::line_in(
        b,
        well_r.shrink2(vec2(5.0 * k, 0.0)),
        role,
        color,
        s.tk(),
        value,
    );
}

/// `−14.2`, or a dash while there is nothing to read.
pub(super) struct Lu(pub Option<f64>);

impl std::fmt::Display for Lu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(v) if v.is_finite() && v > -99.0 => write!(f, "{v:.1}"),
            _ => f.write_str("\u{2014}"),
        }
    }
}

/// The verdict on a measured loudness and true peak against the target, and whether it
/// passes.
pub fn verdict(integrated: Option<f64>, max_tp: f32, target: f32, ceiling: f32) -> (bool, String) {
    let Some(i) = integrated else {
        return (false, t("max.verdict.none"));
    };
    let tol = target_of(target, ceiling).map_or(1.0, |i| TARGETS[i].tolerance) as f64;
    let diff = i - target as f64;
    let tp_ok = max_tp <= ceiling + 0.005;
    let mut words = if diff.abs() <= tol {
        t("max.verdict.on")
    } else if diff > 0.0 {
        tf("max.verdict.loud", &[("lu", &format!("{:.1}", diff))])
    } else {
        tf("max.verdict.quiet", &[("lu", &format!("{:.1}", -diff))])
    };
    if !tp_ok {
        words.push_str(" \u{b7} ");
        words.push_str(&tf(
            "max.verdict.tp",
            &[("db", &format!("{:.1}", max_tp - ceiling))],
        ));
    }
    (diff.abs() <= tol && tp_ok, words)
}
