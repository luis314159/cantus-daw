//! Libra's balance (ADR 0133, provisional): the display's centre, a brass pair of scales.
//!
//! - The **beam tilts with the balance** (the heavier side down, as a balance does) and its
//!   pointer reads it on a fine scale above the pivot (L 100 … C … R 100).
//! - The **arms' reach is the width**: narrow, the pans hang close to the post; wide, far out.
//! - Each **pan holds its channel**: its name, its level in a slot under it (dBFS), its
//!   polarity (Ø, inverted) and its delay — each element saying what it is.
//! - The **correlation** runs along the foot of the stand.
//! - **Dragged**: across for the balance, up and down for the width; a double-click centres it.

use super::super::canvas::{self, Ink};
use super::super::frame;
use crate::kit::color::with_alpha;
use crate::kit::skin::scales as paint;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill};
use cantus_dsp::libra::t;
use cantus_dsp::modfx_clock::tel;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

/// The beam's tilt at full balance, radians (12°).
const TILT: f32 = 0.21;

fn rgb(h: crate::kit::color::Hsl) -> Color32 {
    h.rgb()
}

/// The balance's geometry in its rect: the pivot, the half beam at full width, the chains'
/// length, the foot's top.
pub struct Geo {
    pub pivot: Pos2,
    pub arm: f32,
    pub chain: f32,
    pub foot: f32,
}

pub fn geo(r: Rect, k: f32) -> Geo {
    let pivot = pos2(r.center().x, r.top() + r.height() * 0.30);
    Geo {
        pivot,
        arm: (r.width() * 0.44).min(r.height() * 0.62),
        chain: r.height() * 0.30,
        foot: r.bottom() - 52.0 * k,
    }
}

/// What a pan shows of its channel.
pub struct Pan<'a> {
    pub name: &'a str,
    pub level_db: f32,
    pub inverted: bool,
    pub delay: &'a str,
}

/// A vertical brass bar from `top` to `bottom`, `w` wide, lit from the left.
fn rod(b: &mut Brush, x: f32, top: f32, bottom: f32, w: f32) {
    let r = Rect::from_min_max(pos2(x - w * 0.5, top), pos2(x + w * 0.5, bottom));
    b.hrect(
        r,
        &[
            (0.0, rgb(paint::BRASS_SHADE)),
            (0.35, rgb(paint::BRASS_LIGHT)),
            (0.7, rgb(paint::BRASS)),
            (1.0, rgb(paint::BRASS_SHADE)),
        ],
    );
}

#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    ink: &Ink,
    r: Rect,
    pan: f32,
    width: f32,
    corr: f32,
    pans: [Pan<'_>; 2],
    live: bool,
    k: f32,
) {
    let g = geo(r, k);
    let p = g.pivot;
    // The scale over the pivot: a fine arc of the balance, ticks every 10 %, L C R.
    let scale_r = 46.0 * k;
    let ang = |v: f32| -std::f32::consts::FRAC_PI_2 + v * TILT * 2.4;
    b.arc(
        p,
        scale_r,
        ang(-1.0),
        ang(1.0),
        0.8 * k,
        with_alpha(rgb(paint::BRASS), 0.8),
        false,
    );
    for i in -10..=10 {
        let a = ang(i as f32 / 10.0);
        let u = vec2(a.cos(), a.sin());
        let len = if i % 5 == 0 { 7.0 } else { 3.5 };
        b.segment(
            p + u * scale_r,
            p + u * (scale_r + len * k),
            if i % 5 == 0 { 1.0 } else { 0.7 } * k,
            rgb(paint::BRASS),
            false,
        );
    }
    for (v, w) in [(-1.0f32, "L"), (0.0, "C"), (1.0, "R")] {
        let a = ang(v);
        let at = p + vec2(a.cos(), a.sin()) * (scale_r + 15.0 * k);
        canvas::put(
            b,
            at,
            Align2::CENTER_CENTER,
            &Text::new(w, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    // The stand: a stepped foot, the post, the pivot's boss.
    let foot = Rect::from_center_size(pos2(p.x, g.foot + 6.0 * k), vec2(g.arm * 0.9, 9.0 * k));
    let step = Rect::from_center_size(pos2(p.x, g.foot - 1.0 * k), vec2(g.arm * 0.45, 6.0 * k));
    for f in [foot, step] {
        b.rrect(
            f,
            2.5 * k,
            Fill::V(rgb(paint::BRASS_LIGHT), rgb(paint::BRASS_SHADE)),
        );
        b.rrect_stroke(f, 2.5 * k, 0.7 * k, rgb(paint::BRASS_EDGE));
    }
    rod(b, p.x, p.y, step.top(), 6.0 * k);
    // The beam, tilted: the heavier side (the balance's) down.
    let tilt = pan.clamp(-1.0, 1.0) * TILT;
    let dir = vec2(tilt.cos(), tilt.sin());
    let normal = vec2(-dir.y, dir.x);
    let reach = g.arm * (0.55 + 0.225 * width.clamp(0.0, 2.0));
    let ends = [p - dir * reach, p + dir * reach];
    let beam = [
        ends[0] - normal * (1.6 * k),
        p - normal * (4.0 * k),
        ends[1] - normal * (1.6 * k),
        ends[1] + normal * (1.6 * k),
        p + normal * (4.0 * k),
        ends[0] + normal * (1.6 * k),
    ];
    b.polygon(
        &beam.map(|q| q + vec2(2.0, 3.0) * k),
        with_alpha(rgb(paint::BRASS_EDGE), 0.5),
    );
    b.polygon(&beam, rgb(paint::BRASS));
    b.polyline(
        &[beam[0], beam[1], beam[2]],
        0.8 * k,
        rgb(paint::BRASS_LIGHT),
    );
    b.polyline(
        &[beam[3], beam[4], beam[5]],
        0.8 * k,
        rgb(paint::BRASS_EDGE),
    );
    // The pointer, fixed to the beam, reading the scale.
    let up = -normal;
    b.segment(
        p,
        p + up * (scale_r - 3.0 * k),
        1.2 * k,
        rgb(paint::BRASS_LIGHT),
        true,
    );
    b.dot(
        p + up * (scale_r - 3.0 * k),
        1.6 * k,
        rgb(paint::BRASS_LIGHT),
    );
    b.circle(
        p,
        7.0 * k,
        Fill::V(rgb(paint::BRASS_LIGHT), rgb(paint::BRASS_SHADE)),
    );
    b.circle_stroke(p, 7.0 * k, 0.8 * k, rgb(paint::BRASS_EDGE));
    b.dot(p, 2.0 * k, rgb(paint::BRASS_EDGE));
    // The pans on their chains, hanging plumb.
    for (end, pn) in ends.into_iter().zip(pans.iter()) {
        b.dot(end, 2.6 * k, rgb(paint::BRASS_LIGHT));
        let rim = end.y + g.chain;
        let half = 38.0 * k;
        for dx in [-half * 0.92, 0.0, half * 0.92] {
            b.segment(
                end,
                pos2(end.x + dx, rim),
                0.7 * k,
                rgb(paint::CHAIN),
                false,
            );
        }
        bowl(b, pos2(end.x, rim), half, k);
        pan_words(b, ink, pos2(end.x, rim), half, pn, live, k);
    }
    // The correlation along the foot.
    let bar = Rect::from_center_size(pos2(p.x, g.foot + 22.0 * k), vec2(g.arm * 1.1, 3.0 * k));
    b.rrect(bar, 1.5 * k, Fill::Solid(rgb(paint::SLOT)));
    let x = bar.left() + bar.width() * (corr.clamp(-1.0, 1.0) + 1.0) * 0.5;
    let col = if corr < 0.0 { ink.danger } else { ink.ok };
    b.rrect(
        Rect::from_min_max(
            pos2(bar.center().x.min(x), bar.top()),
            pos2(bar.center().x.max(x), bar.bottom()),
        ),
        1.5 * k,
        Fill::Solid(with_alpha(col, if live { 1.0 } else { 0.4 })),
    );
    b.vline(
        bar.center().x,
        bar.top() - 3.0 * k,
        bar.bottom() + 3.0 * k,
        0.8 * k,
        ink.grid_strong,
    );
    let mut cw = String::from(frame::tr("libra.corr"));
    cw.push(' ');
    let _ = std::fmt::Write::write_fmt(&mut cw, format_args!("{corr:+.2}"));
    for (w, at, al) in [
        (
            "\u{2212}1",
            bar.left_center() - vec2(5.0 * k, 0.0),
            Align2::RIGHT_CENTER,
        ),
        (
            "+1",
            bar.right_center() + vec2(5.0 * k, 0.0),
            Align2::LEFT_CENTER,
        ),
    ] {
        canvas::put(
            b,
            at,
            al,
            &Text::new(w, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    canvas::put(
        b,
        pos2(bar.center().x, bar.bottom() + 4.0 * k),
        Align2::CENTER_TOP,
        &Text::new(&cw, Role::Micro, ink.text).caps(false),
        k,
        None,
    );
}

/// A shallow brass bowl whose rim is at `rim`, `half` wide each side.
fn bowl(b: &mut Brush, rim: Pos2, half: f32, k: f32) {
    let depth = half * 0.32;
    let pts: Vec<Pos2> = (0..=24)
        .map(|i| {
            let u = i as f32 / 24.0 * std::f32::consts::PI;
            pos2(rim.x - half * u.cos(), rim.y + depth * u.sin())
        })
        .collect();
    b.polygon(&pts, rgb(paint::BRASS_SHADE));
    let inner: Vec<Pos2> = pts
        .iter()
        .map(|q| pos2(q.x, rim.y + (q.y - rim.y) * 0.55))
        .collect();
    b.polygon(&inner, rgb(paint::BRASS));
    b.hline(
        rim.x - half,
        rim.x + half,
        rim.y,
        1.2 * k,
        rgb(paint::BRASS_LIGHT),
    );
    b.polyline(&pts, 0.7 * k, rgb(paint::BRASS_EDGE));
}

/// What a pan holds of its channel: its name on the bowl, its level in a slot under it,
/// its polarity and its delay, each with its word.
fn pan_words(b: &mut Brush, ink: &Ink, rim: Pos2, half: f32, pn: &Pan<'_>, live: bool, k: f32) {
    canvas::put(
        b,
        pos2(rim.x, rim.y + half * 0.32 + 4.0 * k),
        Align2::CENTER_TOP,
        &Text::new(pn.name, Role::Value, ink.text).caps(false),
        k,
        None,
    );
    let top = rim.y + half * 0.32 + 26.0 * k;
    let slot = Rect::from_center_size(pos2(rim.x, top), vec2(half * 1.9, 5.0 * k));
    b.rrect(slot, 2.5 * k, Fill::Solid(rgb(paint::SLOT)));
    let db = if live { pn.level_db } else { -200.0 };
    let u = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    if u > 0.0 {
        b.rrect(
            Rect::from_min_size(slot.min, vec2(slot.width() * u, slot.height())),
            2.5 * k,
            Fill::Solid(if db > -0.1 { ink.danger } else { ink.hue }),
        );
    }
    let mut lw = String::from(frame::tr("libra.level"));
    lw.push(' ');
    if db > -120.0 {
        let _ = std::fmt::Write::write_fmt(&mut lw, format_args!("{db:.0} dB"));
    } else {
        lw.push('\u{2013}');
    }
    let line = |b: &mut Brush, i: f32, w: &str, c: Color32| {
        canvas::put(
            b,
            pos2(rim.x, slot.bottom() + (6.0 + 13.0 * i) * k),
            Align2::CENTER_TOP,
            &Text::new(w, Role::Micro, c).caps(false),
            k,
            None,
        );
    };
    line(b, 0.0, &lw, ink.text);
    line(b, 1.0, pn.delay, ink.text);
    if pn.inverted {
        let mut pw = String::from("\u{d8} ");
        pw.push_str(frame::tr("libra.inverted"));
        line(b, 2.0, &pw, ink.danger);
    }
    let _: Vec2 = Vec2::ZERO;
}

/// The balance's telemetry for its pans: (left, right) peaks, dBFS.
pub fn peaks(tl: &[f32; 64]) -> (f32, f32) {
    if tl[tel::BLOCKS] > 0.0 {
        (tl[t::PEAK_L], tl[t::PEAK_R])
    } else {
        (-200.0, -200.0)
    }
}
