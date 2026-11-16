//! Fractus's grain cloud (design sheet v2): a scope's tube behind glass in the chassis, its
//! graticule ten divisions by eight (the centre lines finely ticked); time across (two
//! seconds back at the left, now at the right), pitch up (±24 semitones).
//!
//! On it: where the grains start (the position, its spray shaded), the pitches they may take
//! (the spread's band, the scale's degrees dotted), and **every grain sounding drawn as its
//! window** — as long as its size, at its pitch, its pitch written beside it when there is
//! room — with the **trail** of where it has read (`display::View`); the size's bracket; a
//! frozen cloud frosted, its "now" stopped.

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::art;
use super::display::{Reading, View};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::skin::sequencers as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::fractus::{GrainWindow, Scale};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// The milliseconds the scope spans: half the cloud's memory (`Cloud::shown`'s reach).
const SPAN_MS: f32 = (cantus_dsp::fractus::cloud::CLOUD_S * 500.0) as f32;

/// A grain's window at `x` (0 … 1 through the grain), as the cloud shapes it.
fn window(w: GrainWindow, x: f32) -> f32 {
    use cantus_dsp::math::{cos, exp};
    let x = x.clamp(0.0, 1.0);
    let decay = |x: f32| (exp(-5.0 * x) - exp(-5.0f32)) / (1.0 - exp(-5.0f32));
    let attack = |x: f32| {
        if x < 0.03 {
            let s = cantus_dsp::math::sin(std::f32::consts::FRAC_PI_2 * x / 0.03);
            s * s
        } else {
            1.0
        }
    };
    match w {
        GrainWindow::Hann => 0.5 - 0.5 * cos(std::f32::consts::TAU * x),
        GrainWindow::Gauss => {
            let d = (x - 0.5) / 0.16;
            exp(-0.5 * d * d)
        }
        GrainWindow::Tukey => {
            let e = 0.25;
            if x < e {
                0.5 - 0.5 * cos(std::f32::consts::PI * x / e)
            } else if x > 1.0 - e {
                0.5 - 0.5 * cos(std::f32::consts::PI * (1.0 - x) / e)
            } else {
                1.0
            }
        }
        GrainWindow::Expodec => attack(x) * decay(x),
        GrainWindow::Rexpodec => attack(1.0 - x) * decay(1.0 - x),
    }
}

/// How many points a grain's window is drawn with.
const WIN_PTS: usize = 17;

pub fn show(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let tube = art::window(b, s, r);
    // The tube's face, brighter at its middle.
    b.rect(tube, Fill::Solid(shade(ink.ground, -0.015)));
    b.radial(
        tube,
        tube.center(),
        vec2(tube.width() * 0.7, tube.height() * 0.75),
        &[
            (0.0, shade(ink.ground, 0.045)),
            (1.0, with_alpha(shade(ink.ground, -0.015), 0.0)),
        ],
        12,
    );
    let plot = Rect::from_min_max(
        pos2(tube.left() + 40.0 * k, tube.top() + 30.0 * k),
        pos2(tube.right() - 10.0 * k, tube.bottom() - 20.0 * k),
    );
    graticule(b, k, ink, plot);
    let grain_col = ink.voice(ink.hue, true);
    let y_of = |st: f32| plot.center().y - st.clamp(-24.0, 24.0) / 24.0 * plot.height() * 0.5;
    let x_of = |ms: f32| plot.right() - (ms / SPAN_MS).clamp(0.0, 1.0) * plot.width();
    // The pitches they may take: the spread's band, the scale's degrees in it.
    let centre = spec.get("cpitch");
    let spread = spec.get("crand");
    let (lo, hi) = ((centre - spread).max(-24.0), (centre + spread).min(24.0));
    b.rect(
        Rect::from_min_max(pos2(plot.left(), y_of(hi)), pos2(plot.right(), y_of(lo))),
        Fill::Solid(with_alpha(grain_col, 0.05)),
    );
    let scale = Scale::from_index(spec.get("cscale"));
    if scale != Scale::Free {
        let mut last = f32::NAN;
        for st in -24..=24 {
            let q = scale.quantise(st as f32);
            if q == last || q < lo - 0.01 || q > hi + 0.01 {
                continue;
            }
            last = q;
            canvas::dashed(
                b,
                pos2(plot.left(), y_of(q)),
                pos2(plot.right(), y_of(q)),
                0.8 * k,
                with_alpha(grain_col, 0.3),
                1.5 * k,
                3.0 * k,
            );
        }
    }
    // Where they start: the position, its spray.
    let pos = spec.get("cpos");
    let spray = spec.get("cspray");
    b.rect(
        Rect::from_min_max(
            pos2(x_of(pos + spray), plot.top()),
            pos2(x_of((pos - spray).max(0.0)), plot.bottom()),
        ),
        Fill::Solid(with_alpha(ink.text, 0.04)),
    );
    let px = x_of(pos);
    canvas::dashed(
        b,
        pos2(px, plot.top()),
        pos2(px, plot.bottom()),
        0.8 * k,
        with_alpha(ink.text, 0.45),
        3.0 * k,
        2.0 * k,
    );
    let frozen = now.frozen || spec.get("cfreeze") >= 0.5;
    let size_ms = spec.get("csize");
    let gw = (size_ms / SPAN_MS * plot.width()).max(3.0 * k);
    let shape = GrainWindow::from_index(spec.get("cwin"));
    let ice = art::c(pal::ICE);
    if frozen {
        // Frost on the glass, "now" stopped: a double bar.
        b.rect(plot, Fill::Solid(with_alpha(ice, 0.05)));
        for dx in [0.0, 3.5 * k] {
            b.vline(
                plot.right() - dx,
                plot.top(),
                plot.bottom(),
                1.4 * k,
                with_alpha(ice, 0.9),
            );
        }
    } else {
        b.vline(
            plot.right(),
            plot.top(),
            plot.bottom(),
            1.2 * k,
            with_alpha(ink.text, 0.6),
        );
    }
    trails(b, k, view, now, frozen, size_ms, plot, grain_col);
    let level = spec.get("clevel");
    let mut words: [Rect; 16] = [Rect::NOTHING; 16];
    for (n, &(p, pitch)) in now.shown().iter().enumerate() {
        let x = plot.right() - p.clamp(0.0, 1.0) * plot.width();
        let st = pitch * 24.0;
        let y = y_of(st);
        grain(b, k, pos2(x, y), gw, shape, grain_col, level);
        // Its pitch beside it, where no other's is.
        let txt = format!("{:+.0}", st);
        let t = Text::new(&txt, Role::Micro, grain_col).caps(false);
        let sz = crate::kit::text::measure(b.ctx(), &t, k);
        let at = Align2::CENTER_BOTTOM.anchor_size(pos2(x, y - 5.5 * k), sz);
        if plot.contains_rect(at) && words.iter().all(|w| !w.expand(1.0 * k).intersects(at)) {
            canvas::put(b, at.center(), Align2::CENTER_CENTER, &t, k, None);
            if let Some(slot) = words.get_mut(n) {
                *slot = at;
            }
        }
    }
    labels(
        b, k, ink, plot, now, px, pos, frozen, gw, size_ms, grain_col,
    );
    art::glass(b, tube, k);
}

/// The graticule: ten divisions across, eight up, dotted; the centre lines ticked in fifths
/// of a division; the frame.
fn graticule(b: &mut Brush, k: f32, ink: &Ink, plot: Rect) {
    let (dx, dy) = (plot.width() / 10.0, plot.height() / 8.0);
    let line = with_alpha(ink.grid_strong, 0.55);
    for i in 1..10 {
        let x = plot.left() + i as f32 * dx;
        canvas::dashed(
            b,
            pos2(x, plot.top()),
            pos2(x, plot.bottom()),
            0.7 * k,
            line,
            1.0 * k,
            3.0 * k,
        );
    }
    for j in 1..8 {
        let y = plot.top() + j as f32 * dy;
        canvas::dashed(
            b,
            pos2(plot.left(), y),
            pos2(plot.right(), y),
            0.7 * k,
            line,
            1.0 * k,
            3.0 * k,
        );
    }
    let c = plot.center();
    let axis = with_alpha(ink.grid_strong, 0.9);
    b.hline(plot.left(), plot.right(), c.y, 0.8 * k, axis);
    b.vline(c.x, plot.top(), plot.bottom(), 0.8 * k, axis);
    for i in 0..=50 {
        let x = plot.left() + i as f32 * dx / 5.0;
        let h = if i % 5 == 0 { 4.0 } else { 2.5 } * k;
        b.vline(x, c.y - h, c.y + h, 0.7 * k, axis);
    }
    for j in 0..=40 {
        let y = plot.top() + j as f32 * dy / 5.0;
        let w = if j % 5 == 0 { 4.0 } else { 2.5 } * k;
        b.hline(c.x - w, c.x + w, y, 0.7 * k, axis);
    }
    b.rrect_stroke(plot, 1.0 * k, 0.9 * k, with_alpha(ink.grid_strong, 0.9));
}

/// One grain: its window as long as its size, at its pitch, a bright head where it reads.
fn grain(
    b: &mut Brush,
    k: f32,
    at: Pos2,
    w: f32,
    shape: GrainWindow,
    col: egui::Color32,
    level: f32,
) {
    let h = 9.0 * k;
    let mut top = [Pos2::ZERO; WIN_PTS];
    let mut bot = [Pos2::ZERO; WIN_PTS];
    for i in 0..WIN_PTS {
        let f = i as f32 / (WIN_PTS - 1) as f32;
        let e = window(shape, f) * h * 0.5;
        let x = at.x - w * 0.5 + f * w;
        top[i] = pos2(x, at.y - e);
        bot[i] = pos2(x, at.y + e);
    }
    let a = 0.45 + 0.4 * level.clamp(0.0, 1.0);
    b.rrect_soft(
        Rect::from_center_size(at, vec2(w + 4.0 * k, h + 2.0 * k)),
        h * 0.5,
        Fill::Solid(with_alpha(col, 0.16)),
        5.0 * k,
    );
    b.between(&top, &bot, with_alpha(col, a * 0.55));
    b.polyline(&top, 0.9 * k, with_alpha(col, a));
    b.polyline(&bot, 0.9 * k, with_alpha(col, a));
    b.circle(at, 1.9 * k, Fill::Solid(mix(col, art::c(pal::IVORY), 0.55)));
}

/// Behind each grain, where it has read: its place in the readings before, matched by its
/// pitch and the way a grain at that pitch moves (toward now when it reads faster than the
/// tape or the cloud is frozen, away when slower) and no farther than its life lets it
/// read, fading.
fn trails(
    b: &mut Brush,
    k: f32,
    view: &View,
    now: &Reading,
    frozen: bool,
    size_ms: f32,
    plot: Rect,
    col: egui::Color32,
) {
    let x_of = |p: f32| plot.right() - p.clamp(0.0, 1.0) * plot.width();
    let y_of = |pitch: f32| plot.center().y - pitch.clamp(-1.0, 1.0) * plot.height() * 0.5;
    for &(p0, pitch) in now.shown() {
        let speed = cantus_dsp::math::exp2(2.0 * pitch);
        let dir = if frozen {
            1.0
        } else if (speed - 1.0).abs() < 1e-3 {
            continue;
        } else {
            (speed - 1.0).signum()
        };
        // No farther than a grain reads in its life: past that it is another grain.
        let reach = if frozen { speed } else { (speed - 1.0).abs() } * size_ms / SPAN_MS;
        let mut cur = p0;
        let mut prev = pos2(x_of(p0), y_of(pitch));
        for age in 1..view.len() {
            let found = view
                .back(age)
                .iter()
                .filter(|(q, s)| (s - pitch).abs() < 1e-4 && (q - cur) * dir > 0.0)
                .map(|(q, _)| *q)
                .filter(|q| (q - cur).abs() < 0.05 && (q - p0).abs() <= reach)
                .fold(None, |best: Option<f32>, q| match best {
                    Some(bq) if (bq - cur).abs() <= (q - cur).abs() => Some(bq),
                    _ => Some(q),
                });
            let Some(q) = found else {
                break;
            };
            let here = pos2(x_of(q), y_of(pitch));
            let fade = 1.0 - age as f32 / view.len() as f32;
            b.segment(prev, here, 1.6 * k, with_alpha(col, 0.55 * fade), true);
            prev = here;
            cur = q;
        }
    }
}

/// The tube's words: the pitch rows at the left, the time under it, the grains' count and
/// the position over it, the size's bracket, *Frozen*.
#[allow(clippy::too_many_arguments)]
fn labels(
    b: &mut Brush,
    k: f32,
    ink: &Ink,
    plot: Rect,
    now: &Reading,
    px: f32,
    pos_ms: f32,
    frozen: bool,
    gw: f32,
    size_ms: f32,
    grain_col: egui::Color32,
) {
    for st in [-24i32, -12, 0, 12, 24] {
        let y = plot.center().y - st as f32 / 24.0 * plot.height() * 0.5;
        canvas::putf(
            b,
            pos2(plot.left() - 5.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{st:+}{}", if st == 24 { " st" } else { "" }),
        );
    }
    let y = plot.bottom() + 4.0 * k;
    canvas::put_key(
        b,
        pos2(plot.left(), y),
        Align2::LEFT_TOP,
        "fractus.d.past",
        Role::Micro,
        ink.dim,
        k,
    );
    canvas::put_key(
        b,
        pos2(plot.right(), y),
        Align2::RIGHT_TOP,
        "fractus.d.now",
        Role::Micro,
        ink.dim,
        k,
    );
    let count = canvas::putf(
        b,
        pos2(plot.left() + 4.0 * k, plot.top() - 12.0 * k),
        Align2::LEFT_BOTTOM,
        Role::Micro,
        ink.text,
        k,
        None,
        format_args!("{} {:.0}", tr("fractus.d.grains"), now.grains.max(0.0)),
    );
    // The position over its line, kept inside the tube and clear of the count.
    let pos_w = format!("{} {:.0} ms", tr("fractus.l.position"), pos_ms);
    let t = Text::new(&pos_w, Role::Micro, ink.dim).caps(false);
    let sz = crate::kit::text::measure(b.ctx(), &t, k);
    let x = px.clamp(plot.left() + sz.x * 0.5, plot.right() - sz.x * 0.5);
    let at = Align2::CENTER_BOTTOM.anchor_size(pos2(x, plot.top() - 12.0 * k), sz);
    if !at.expand(4.0 * k).intersects(count) {
        canvas::put(b, at.center(), Align2::CENTER_CENTER, &t, k, None);
    }
    // The size's bracket: a grain's length, named.
    let by = plot.bottom() - 9.0 * k;
    let words = format!("{} {:.0} ms", tr("fractus.l.size"), size_ms);
    let wt = Text::new(&words, Role::Micro, grain_col).caps(false);
    let wsz = crate::kit::text::measure(b.ctx(), &wt, k);
    let total = gw + 6.0 * k + wsz.x;
    let x0 = plot.center().x + 6.0 * k - total * 0.5;
    let tick = 3.0 * k;
    b.rect(
        Rect::from_min_max(
            pos2(x0 - 3.0 * k, by - wsz.y * 0.5 - 1.0 * k),
            pos2(x0 + total + 3.0 * k, by + wsz.y * 0.5 + 1.0 * k),
        ),
        Fill::Solid(with_alpha(ink.ground, 0.85)),
    );
    b.hline(x0, x0 + gw, by, 1.0 * k, grain_col);
    b.vline(x0, by - tick, by + tick, 1.0 * k, grain_col);
    b.vline(x0 + gw, by - tick, by + tick, 1.0 * k, grain_col);
    canvas::put(
        b,
        pos2(x0 + gw + 6.0 * k, by),
        Align2::LEFT_CENTER,
        &wt,
        k,
        None,
    );
    if frozen {
        let ice = art::c(pal::ICE);
        canvas::put(
            b,
            pos2(plot.center().x, plot.top() + 6.0 * k),
            Align2::CENTER_TOP,
            &Text::key("fractus.d.frozen", Role::Micro, ice),
            k,
            Some(with_alpha(ink.ground, 0.9)),
        );
    }
}
