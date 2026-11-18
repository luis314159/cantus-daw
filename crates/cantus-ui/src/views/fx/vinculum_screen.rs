//! Vinculum's screen (design sheet v1): a vacuum-fluorescent glass in its window, dark in
//! every theme. On the left the **static transfer curve** at the settings — each model's
//! own law (`cantus_dsp::vinculum::law`), the make-up and the mix included, the unity line
//! under it, the threshold marked — with the signal's point on it (its input peak against
//! its output peak) and the reduction in large figures; on the right **the gain reduction
//! over the last eight seconds** hanging from the top, over the input's and the output's
//! levels (and the key's, dashed, when another strip is the key).

use super::super::canvas::{self, Ink};
use super::art;
use super::display::{View, FLOOR_DB, HISTORY, RATE};
use super::face::model;
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::math;
use cantus_dsp::vinculum::law::{self, MuLaw, OptoLaw};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// The transfer plot's range, dBFS (both axes).
const LO: f32 = -54.0;
const HI: f32 = 6.0;
/// Points along the curve.
const N: usize = 97;

/// The static reduction, dB, the model gives an input `x` dBFS held steady.
pub fn static_gr(spec: &EffectSpec, x: f32) -> f32 {
    let thr = spec.get("thr");
    let limit = spec.get("char") >= 0.5;
    match model(spec) {
        1 => {
            // The cell is lit by the output: solve the output `y` whose input `y + GR(y)`
            // is `x` (monotonic, so a bisection).
            let lawc = if limit {
                OptoLaw::limit()
            } else {
                OptoLaw::compress()
            };
            let gr_of = |y: f32| OptoLaw::gr_db(lawc.conductance(math::db_to_gain(y - thr)));
            let (mut lo, mut hi) = (x - law::MAX_GR_DB, x);
            if hi + gr_of(hi) <= x {
                return 0.0;
            }
            for _ in 0..40 {
                let m = 0.5 * (lo + hi);
                if m + gr_of(m) < x {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            (x - 0.5 * (lo + hi)).max(0.0)
        }
        2 => {
            let l = if limit {
                MuLaw::limit()
            } else {
                MuLaw::compress()
            };
            l.equilibrium(x - thr)
        }
        _ => {
            let i = spec.get("ratio").round().clamp(0.0, 2.0) as usize;
            law::vca_gr_db(
                x,
                thr,
                cantus_core::fx::vinculum::RATIOS[i],
                cantus_dsp::vinculum::VCA_KNEE_DB,
            )
        }
    }
}

/// The output, dBFS, for an input `x` held steady: the reduction, the make-up and the mix
/// (parallel compression bends the curve back toward the unity line).
pub fn transfer(spec: &EffectSpec, x: f32) -> f32 {
    let gr = static_gr(spec, x);
    let mix = spec.get("mix").clamp(0.0, 1.0);
    let wet = math::db_to_gain(spec.get("makeup") - gr);
    let g = (1.0 - mix) + mix * wet;
    x + 20.0 * math::log10(g.max(1e-6)) + spec.get("out")
}

/// The local ratio at the input `x`: how many decibels in for one out (the curve's slope,
/// without the make-up and the mix).
pub fn ratio_at(spec: &EffectSpec, x: f32) -> f32 {
    let d = 0.25;
    let y0 = x - d - static_gr(spec, x - d);
    let y1 = x + d - static_gr(spec, x + d);
    (2.0 * d / (y1 - y0).max(1e-4)).clamp(1.0, 99.0)
}

/// The screen in `r`.
#[allow(clippy::too_many_arguments)]
pub fn show(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    spec: &EffectSpec,
    view: &View,
    now: &super::display::Reading,
    keyed: bool,
) {
    let k = s.k;
    let glass = art::window(b, s, r, 5.0 * k);
    canvas::ground_b(b, glass, ink, None);
    let inner = glass.shrink(10.0 * k);
    let side = (inner.height() - 14.0 * k).min(inner.width() * 0.42);
    let plot = Rect::from_min_size(pos2(inner.left() + 24.0 * k, inner.top()), vec2(side, side));
    transfer_plot(b, s, ink, plot, spec, now);
    let hist = Rect::from_min_max(
        pos2(plot.right() + 26.0 * k, inner.top()),
        pos2(inner.right() - 22.0 * k, inner.bottom() - 14.0 * k),
    );
    if hist.width() > 60.0 * k {
        history(b, s, ink, hist, view, keyed);
    }
    canvas::edge_b(b, glass, ink);
    art::glass(b, glass, k);
}

fn transfer_plot(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    plot: Rect,
    spec: &EffectSpec,
    now: &super::display::Reading,
) {
    let k = s.k;
    let xy = |x: f32, y: f32| {
        pos2(
            plot.left() + plot.width() * (x - LO) / (HI - LO),
            plot.bottom() - plot.height() * ((y - LO) / (HI - LO)).clamp(-0.02, 1.02),
        )
    };
    // The grid: every 6 dB fine, every 12 dB stronger, numbered along the foot and the left.
    let mut d = LO;
    while d <= HI + 0.01 {
        let strong = ((d as i32) % 12) == 0;
        let col = with_alpha(if strong { ink.grid_strong } else { ink.grid }, 0.7);
        let w = if strong { 0.8 } else { 0.5 } * k;
        let (a, z) = (xy(d, LO), xy(d, HI));
        b.vline(a.x, z.y, a.y, w, col);
        let (a, z) = (xy(LO, d), xy(HI, d));
        b.hline(a.x, z.x, a.y, w, col);
        if strong && d < HI {
            canvas::putf(
                b,
                pos2(xy(d, LO).x, plot.bottom() + 2.0 * k),
                Align2::CENTER_TOP,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{d:.0}"),
            );
            canvas::putf(
                b,
                pos2(plot.left() - 3.0 * k, xy(LO, d).y),
                Align2::RIGHT_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{d:.0}"),
            );
        }
        d += 6.0;
    }
    // The unity line.
    canvas::dashed(
        b,
        xy(LO, LO),
        xy(HI, HI),
        0.8 * k,
        with_alpha(ink.dim, 0.7),
        4.0 * k,
        4.0 * k,
    );
    // The threshold: a mark on the foot and a faint line up to the curve.
    let thr = spec.get("thr");
    if (LO..=HI).contains(&thr) {
        let x = xy(thr, LO).x;
        canvas::dashed(
            b,
            pos2(x, plot.bottom()),
            xy(thr, transfer(spec, thr)),
            0.7 * k,
            with_alpha(ink.hue, 0.45),
            2.0 * k,
            3.0 * k,
        );
        b.polygon(
            &[
                pos2(x, plot.bottom() - 6.0 * k),
                pos2(x - 4.0 * k, plot.bottom()),
                pos2(x + 4.0 * k, plot.bottom()),
            ],
            ink.hue,
        );
    }
    // The curve.
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        let x = LO + (HI - LO) * i as f32 / (N - 1) as f32;
        *p = xy(x, transfer(spec, x));
    }
    canvas::trace(b, &pts, 1.8 * k, ink.hue, ink);
    // The signal's point: its input peak against its output peak, with lines to the axes.
    let second = ink.look.trace2;
    if now.seen && now.in_db > LO {
        let p = xy(now.in_db.min(HI), now.out_db.clamp(LO, HI));
        canvas::dashed(
            b,
            pos2(p.x, plot.bottom()),
            p,
            0.6 * k,
            with_alpha(second, 0.6),
            2.0 * k,
            2.0 * k,
        );
        canvas::dashed(
            b,
            pos2(plot.left(), p.y),
            p,
            0.6 * k,
            with_alpha(second, 0.6),
            2.0 * k,
            2.0 * k,
        );
        b.radial(
            plot,
            p,
            egui::Vec2::splat(9.0 * k),
            &[
                (0.0, with_alpha(second, 0.6)),
                (1.0, with_alpha(second, 0.0)),
            ],
            8,
        );
        b.circle(p, 3.2 * k, Fill::Solid(second));
    }
    // The reduction in large figures in the plot's empty corner, the ratio where it works.
    let at = pos2(plot.left() + 8.0 * k, plot.top() + 6.0 * k);
    let t = canvas::put(
        b,
        at,
        Align2::LEFT_TOP,
        &Text::key("vinculum.d.reduction", Role::Micro, ink.dim),
        k,
        None,
    );
    let gr = if now.seen { now.gr } else { 0.0 };
    let big = canvas::putf(
        b,
        pos2(at.x, t.bottom() + 2.0 * k),
        Align2::LEFT_TOP,
        Role::Hero,
        second,
        k,
        None,
        format_args!("{gr:.1} dB"),
    );
    let x_now = if now.seen {
        now.in_db.max(LO)
    } else {
        thr + 6.0
    };
    let ratio = ratio_at(spec, x_now.max(thr + 0.5));
    let _ = big;
    // The ratio now and the threshold in the corner under the curve (it bends away from
    // there), each on a plate.
    let plate = Some(ink.plate);
    let mut y = plot.bottom() - 8.0 * k;
    for (key, value) in [
        ("fx.vinculum.thr", format!("{thr:.1} dB")),
        ("vinculum.d.ratio", format!("{ratio:.1}:1")),
    ] {
        let v = canvas::put(
            b,
            pos2(plot.right() - 6.0 * k, y),
            Align2::RIGHT_BOTTOM,
            &Text::new(&value, Role::Value, ink.text).caps(false),
            k,
            plate,
        );
        canvas::put(
            b,
            pos2(v.left() - 8.0 * k, v.center().y),
            Align2::RIGHT_CENTER,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            plate,
        );
        y = v.top() - 6.0 * k;
    }
}

/// The history in `r`: the reduction hanging from the top (0 … 24 dB, its scale on the
/// right), the levels rising from the foot (−60 … 0 dBFS, the scale on the left).
fn history(b: &mut Brush, s: &Style, ink: &Ink, area: Rect, view: &View, keyed: bool) {
    let k = s.k;
    let second = ink.look.trace2;
    let key_ink = ink.third(s, crate::kit::Function::Mod);
    // The legend along the top, its entries wrapped onto a second line when the pane is
    // narrow (the Spanish words are longer); the plot under it.
    let entries: [(&str, egui::Color32, bool); 4] = [
        ("vinculum.d.gr", second, true),
        ("vinculum.d.output", ink.hue, true),
        ("vinculum.d.input", ink.dim, true),
        ("vinculum.d.keylevel", key_ink, keyed),
    ];
    let line = 14.0 * k;
    let (mut x, mut y) = (area.left(), area.top() + 0.5 * line);
    for (key, col, on) in entries {
        if !on {
            continue;
        }
        let w = 12.0 * k
            + crate::kit::text::measure(b.ctx(), &Text::key(key, Role::Micro, ink.dim), k).x;
        if x > area.left() && x + w > area.right() + 20.0 * k {
            x = area.left();
            y += line;
        }
        b.rect(
            Rect::from_center_size(pos2(x + 4.0 * k, y), vec2(8.0 * k, 3.0 * k)),
            Fill::Solid(col),
        );
        let t = canvas::put(
            b,
            pos2(x + 12.0 * k, y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        x = t.right() + 14.0 * k;
    }
    let r = Rect::from_min_max(pos2(area.left(), y + 0.5 * line + 4.0 * k), area.max);
    let lvl = |db: f32| r.bottom() - r.height() * ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let red = |gr: f32| r.top() + r.height() * (gr / 24.0).clamp(0.0, 1.0);
    for db in [-48.0f32, -36.0, -24.0, -12.0] {
        let y = lvl(db);
        b.hline(r.left(), r.right(), y, 0.5 * k, with_alpha(ink.grid, 0.7));
        canvas::putf(
            b,
            pos2(r.left() - 3.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{db:.0}"),
        );
    }
    for gr in [6.0f32, 12.0, 18.0] {
        canvas::putf(
            b,
            pos2(r.right() + 3.0 * k, red(gr)),
            Align2::LEFT_CENTER,
            Role::Micro,
            with_alpha(second, 0.85),
            k,
            None,
            format_args!("{gr:.0}"),
        );
    }
    // The seconds along the foot.
    let span = HISTORY as f32 / RATE;
    for sec in [2.0f32, 4.0, 6.0] {
        let x = r.right() - r.width() * sec / span;
        b.vline(x, r.top(), r.bottom(), 0.5 * k, with_alpha(ink.grid, 0.5));
        canvas::putf(
            b,
            pos2(x, r.bottom() + 2.0 * k),
            Align2::CENTER_TOP,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("\u{2212}{sec:.0} s"),
        );
    }
    b.rect(
        Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + 0.8 * k)),
        Fill::Solid(with_alpha(second, 0.5)),
    );
    if view.is_empty() {
        return;
    }
    let n = view.len();
    let step = r.width() / (HISTORY - 1) as f32;
    let x0 = r.right() - step * (n - 1) as f32;
    let mut grs = [Pos2::ZERO; HISTORY];
    let mut outs = [Pos2::ZERO; HISTORY];
    let mut ins = [Pos2::ZERO; HISTORY];
    let mut keys = [Pos2::ZERO; HISTORY];
    for (i, p) in view.points().enumerate() {
        let x = x0 + step * i as f32;
        grs[i] = pos2(x, red(p.gr));
        outs[i] = pos2(x, lvl(p.out_db.max(FLOOR_DB)));
        ins[i] = pos2(x, lvl(p.in_db.max(FLOOR_DB)));
        keys[i] = pos2(x, lvl(p.key_db.max(FLOOR_DB)));
    }
    // The input as a faint body, the output a line over it.
    b.area(&ins[..n], r.bottom(), with_alpha(ink.dim, 0.20));
    b.polyline(&ins[..n], 0.7 * k, with_alpha(ink.dim, 0.6));
    canvas::trace(b, &outs[..n], 1.2 * k, ink.hue, ink);
    if keyed {
        b.dashed(&keys[..n], 1.0 * k, key_ink, 4.0 * k, 3.0 * k);
    }
    // The reduction: a red curtain from the top, its edge a bright line.
    b.area(&grs[..n], r.top(), with_alpha(second, 0.30));
    canvas::trace(b, &grs[..n], 1.6 * k, second, ink);
}
