//! Latitudo's vectorscope card: the goniometer (the output's points, fading with age,
//! and their polar outline), with rings that say how loud, then the correlation and
//! balance meters under it.

use super::State;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::frame;
use cantus_dsp::latitudo::view::telemetry;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// The polar outline's bins over the upper half-turn.
pub const BINS: usize = 32;
/// Rings drawn at these levels (dB of a point's distance from the centre), when inside.
const RINGS: [f32; 4] = [0.0, -6.0, -12.0, -24.0];

/// The scope and its meters in `rect`; returns the scope's square.
pub fn show(st: &mut State, ui: &mut egui::Ui, s: &Style, tel: &[f32; 64], rect: Rect) -> Rect {
    let k = s.k;
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("st-scope"), Sense::hover());
    let row = 18.0 * k;
    let gap = 5.0 * k;
    let meters = Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - 2.0 * row - 3.0 * gap),
        rect.max,
    );
    let top = Rect::from_min_max(rect.min, pos2(rect.right(), meters.top()));
    let radius = (top.width().min(top.height()) * 0.5 - 12.0 * k).max(8.0);
    let c = top.center();
    let square = Rect::from_center_size(c, vec2(2.0 * radius, 2.0 * radius));

    // The zoom follows the level, so a quiet mix still fills the scope; the rings say
    // how loud a point is wherever the zoom is.
    let n = st.points.len();
    let mean: f32 = if n > 0 {
        st.points.iter().map(|(l, r)| l * l + r * r).sum::<f32>() / n as f32
    } else {
        0.0
    };
    let target = (0.42 / mean.sqrt().max(0.004)).clamp(0.5, 60.0);
    st.zoom = if st.zoom <= 0.0 {
        target
    } else {
        st.zoom + (target - st.zoom) * 0.08
    };
    let zoom = st.zoom;
    let at = |l: f32, r: f32| -> Pos2 {
        let x = (r - l) * FRAC_1_SQRT_2 * zoom;
        let y = (l + r) * FRAC_1_SQRT_2 * zoom;
        let d = (x * x + y * y).sqrt();
        let f = if d > 1.0 { 1.0 / d } else { 1.0 };
        pos2(c.x + x * f * radius, c.y - y * f * radius)
    };

    // The polar outline: where the energy points, folded to the upper half.
    let mut bins = [0.0f32; BINS];
    for (l, r) in &st.points {
        let (x, y) = ((r - l) * FRAC_1_SQRT_2, (l + r) * FRAC_1_SQRT_2);
        let (x, y) = if y < 0.0 { (-x, -y) } else { (x, y) };
        let d = (x * x + y * y).sqrt();
        if d > 1e-6 {
            let a = y.atan2(x).clamp(0.0, PI);
            let b = ((a / PI) * BINS as f32).min(BINS as f32 - 1.0) as usize;
            bins[b] += d;
        }
    }
    let top_bin = bins.iter().copied().fold(0.0f32, f32::max);
    for (p, b) in st.polar.iter_mut().zip(bins) {
        let v = if top_bin > 0.0 { b / top_bin } else { 0.0 };
        *p += (v - *p) * 0.15;
    }

    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        // The guides: the circle, the mid and side axes, the speakers' diagonals.
        b.circle_stroke(c, radius, px, ink.grid_strong);
        b.segment(
            pos2(c.x, c.y - radius),
            pos2(c.x, c.y + radius),
            px,
            ink.grid_strong,
            false,
        );
        b.segment(
            pos2(c.x - radius, c.y),
            pos2(c.x + radius, c.y),
            px,
            ink.grid,
            false,
        );
        let d = radius * FRAC_1_SQRT_2;
        for sx in [-1.0f32, 1.0] {
            b.segment(
                pos2(c.x - sx * d, c.y - d),
                pos2(c.x + sx * d, c.y + d),
                px,
                ink.grid,
                false,
            );
        }
        // Level rings.
        for db in RINGS {
            let rr = 10f32.powf(db / 20.0) * zoom * radius;
            if rr < radius * 0.97 && rr > 6.0 * k {
                b.circle_stroke(c, rr, px, ink.grid);
            }
        }
        // The points: newest brightest.
        let dot = (1.6 * k).max(px * 1.5);
        for (i, (l, r)) in st.points.iter().enumerate() {
            let age = (i + 1) as f32 / n.max(1) as f32;
            let p = at(*l, *r);
            let col = Ink::a(ink.hue, 0.12 + 0.68 * age * age);
            b.rect(Rect::from_center_size(p, vec2(dot, dot)), Fill::Solid(col));
        }
        // The polar outline.
        if top_bin > 0.0 {
            let mut pts = [Pos2::ZERO; BINS + 1];
            for (j, pt) in pts.iter_mut().enumerate() {
                let v = st.polar[j.min(BINS - 1)];
                let a = j as f32 / BINS as f32 * PI;
                let rr = radius * (0.08 + 0.9 * v);
                *pt = pos2(c.x + a.cos() * rr, c.y - a.sin() * rr);
            }
            b.polyline(&pts, 1.25 * k, Ink::a(ink.text, 0.55));
        }

        // Words last.
        let tk = s.tk();
        let plate = Some(ink.plate);
        let put = |b: &mut Brush, at: Pos2, align: Align2, w: &str| {
            canvas::put(
                b,
                at,
                align,
                &crate::kit::text::Text::new(w, Role::Micro, ink.dim),
                tk,
                plate,
            );
        };
        put(
            b,
            pos2(c.x, c.y - radius - 2.0 * k),
            Align2::CENTER_BOTTOM,
            "M",
        );
        put(
            b,
            pos2(c.x - d - 2.0 * k, c.y - d - 2.0 * k),
            Align2::RIGHT_BOTTOM,
            "L",
        );
        put(
            b,
            pos2(c.x + d + 2.0 * k, c.y - d - 2.0 * k),
            Align2::LEFT_BOTTOM,
            "R",
        );
        put(
            b,
            pos2(c.x - radius - 3.0 * k, c.y),
            Align2::RIGHT_CENTER,
            "S",
        );
        for db in RINGS {
            let rr = 10f32.powf(db / 20.0) * zoom * radius;
            if rr < radius * 0.97 && rr > 6.0 * k {
                canvas::putf(
                    b,
                    pos2(c.x + 3.0 * k, c.y - rr),
                    Align2::LEFT_BOTTOM,
                    Role::Micro,
                    ink.dim,
                    tk,
                    plate,
                    format_args!("{db:.0}"),
                );
            }
        }
        let corner = 6.0 * k;
        canvas::tag(
            b,
            pos2(rect.left() + corner, rect.top() + corner),
            Align2::LEFT_TOP,
            frame::tr("st.corr.short"),
            ink.text,
            tk,
            plate,
            format_args!("{:+.2}", tel[telemetry::CORR]),
        );
        let side = tel[telemetry::SIDE_DB];
        canvas::tag(
            b,
            pos2(rect.right() - corner, rect.top() + corner),
            Align2::RIGHT_TOP,
            frame::tr("st.side_mid"),
            ink.text,
            tk,
            plate,
            format_args!("{}", SideDb(side)),
        );
        if n == 0 {
            canvas::put_key(
                b,
                c,
                Align2::CENTER_CENTER,
                "st.empty",
                Role::Body,
                ink.dim,
                tk,
            );
        }
        meters_paint(b, st, s, &ink, tel, meters, row, gap);
    });
    if let Some(p) = resp.hover_pos() {
        let (name, tip) = if p.y < meters.top() {
            ("st.card.scope", "st.scope_tip")
        } else if p.y < meters.top() + gap + row + gap * 0.5 {
            ("st.corr", "st.corr_tip")
        } else {
            ("st.bal", "st.bal_tip")
        };
        hint::set_text(ui.ctx(), frame::tr(name), frame::tr(tip), Caps::NONE);
    }
    square
}

/// `−9.1 dB`, or `−∞` when there is no side at all.
struct SideDb(f32);

impl std::fmt::Display for SideDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 <= cantus_dsp::latitudo::view::SIDE_FLOOR_DB {
            f.write_str("\u{2212}\u{221e}")
        } else {
            write!(f, "{:+.1} dB", self.0)
        }
    }
}

/// The correlation and balance meters, one row each.
#[allow(clippy::too_many_arguments)]
fn meters_paint(
    b: &mut Brush,
    st: &State,
    s: &Style,
    ink: &Ink,
    tel: &[f32; 64],
    rect: Rect,
    row: f32,
    gap: f32,
) {
    let k = s.k;
    let tk = s.tk();
    let label_w = 40.0 * k;
    let value_w = 56.0 * k;
    let left = rect.left() + 6.0 * k;
    let right = rect.right() - 6.0 * k;
    let bar_l = left + label_w;
    let bar_r = right - value_w;
    let px = b.px();
    for (i, key) in ["st.corr.short", "fx.st.bal.short"].iter().enumerate() {
        let y0 = rect.top() + gap + i as f32 * (row + gap);
        let bar = Rect::from_min_max(pos2(bar_l, y0 + 3.0 * k), pos2(bar_r, y0 + row - 3.0 * k));
        let xv = |v: f32| bar.left() + (v.clamp(-1.0, 1.0) * 0.5 + 0.5) * bar.width();
        b.rrect(b.snap_rect(bar), 2.0 * k, Fill::Solid(s.t.meter_well));
        for t in [-0.5f32, 0.0, 0.5] {
            b.vline(
                b.snap_mid(xv(t)),
                bar.top(),
                bar.bottom(),
                px,
                ink.grid_strong,
            );
        }
        let mid = y0 + row * 0.5;
        canvas::put_key(
            b,
            pos2(left, mid),
            Align2::LEFT_CENTER,
            key,
            Role::Micro,
            ink.dim,
            tk,
        );
        if i == 0 {
            let v = tel[telemetry::CORR];
            let col = corr_color(v, ink);
            let (a, z) = (xv(0.0).min(xv(v)), xv(0.0).max(xv(v)));
            b.rect(
                Rect::from_x_y_ranges(a..=z.max(a + px), bar.shrink(1.0 * k).y_range()),
                Fill::Solid(col),
            );
            // The input's correlation: a thin mark; the lowest of three seconds: a
            // triangle under the bar.
            let xi = xv(tel[telemetry::IN_CORR]);
            b.vline(
                b.snap_mid(xi),
                bar.top() - 2.0 * k,
                bar.bottom() + 2.0 * k,
                1.5 * k,
                ink.text,
            );
            if let Some(w) = st.worst() {
                let xw = xv(w);
                let y = bar.bottom();
                b.polygon(
                    &[
                        pos2(xw, y - 1.0 * k),
                        pos2(xw + 4.0 * k, y + 4.0 * k),
                        pos2(xw - 4.0 * k, y + 4.0 * k),
                    ],
                    corr_color(w, ink),
                );
            }
            canvas::putf(
                b,
                pos2(right, mid),
                Align2::RIGHT_CENTER,
                Role::Value,
                ink.text,
                tk,
                None,
                format_args!("{v:+.2}"),
            );
        } else {
            let v = tel[telemetry::BAL];
            let (a, z) = (xv(0.0).min(xv(v)), xv(0.0).max(xv(v)));
            b.rect(
                Rect::from_x_y_ranges(a..=z.max(a + px), bar.shrink(1.0 * k).y_range()),
                Fill::Solid(ink.hue),
            );
            let xi = xv(tel[telemetry::IN_BAL]);
            b.vline(
                b.snap_mid(xi),
                bar.top() - 2.0 * k,
                bar.bottom() + 2.0 * k,
                1.5 * k,
                ink.text,
            );
            canvas::putf(
                b,
                pos2(right, mid),
                Align2::RIGHT_CENTER,
                Role::Value,
                ink.text,
                tk,
                None,
                format_args!("{}", Balance(v)),
            );
            for (x, w) in [(bar.left(), "L"), (bar.right(), "R")] {
                let align = if x == bar.left() {
                    Align2::LEFT_CENTER
                } else {
                    Align2::RIGHT_CENTER
                };
                let at = pos2(x + if x == bar.left() { 3.0 } else { -3.0 } * k, mid);
                canvas::put(
                    b,
                    at,
                    align,
                    &crate::kit::text::Text::new(w, Role::Micro, ink.dim),
                    tk,
                    None,
                );
            }
        }
    }
}

/// Green toward mono, amber around zero, the danger colour below it.
fn corr_color(v: f32, ink: &Ink) -> Color32 {
    if v < 0.0 {
        ink.danger
    } else if v < 0.3 {
        crate::kit::color::mix(ink.danger, ink.ok, v / 0.3)
    } else {
        ink.ok
    }
}

/// A balance as dB toward a side: `L 1.5 dB`, `R 0.8 dB`, or `C`.
struct Balance(f32);

impl std::fmt::Display for Balance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.0.clamp(-0.9999, 0.9999);
        let db = 10.0 * ((1.0 + b) / (1.0 - b)).log10();
        if db.abs() < 0.05 {
            f.write_str(frame::tr("st.centre"))
        } else if db < 0.0 {
            write!(f, "L {:.1} dB", -db)
        } else {
            write!(f, "R {db:.1} dB")
        }
    }
}
