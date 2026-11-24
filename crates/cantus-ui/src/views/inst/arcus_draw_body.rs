//! The instrument drawn (a child of `views::inst::arcus::draw`): a cello or a double bass in
//! its real proportions, lying across the display with its scroll to the right.

use super::Now;
use crate::kit::color::{mix, with_alpha};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{ArcusSettings, Technique};
use egui::{pos2, vec2, Color32, Pos2, Rect};

/// An instrument's measures, m from its bottom end: the body's length and its widest
/// bout, the bridge, the nut, the scroll's end, where the fingerboard starts, the string
/// spacing at the bridge; and its outline's half-widths (along the body, over the widest).
struct Shape {
    body: f32,
    width: f32,
    bridge: f32,
    nut: f32,
    end: f32,
    board: f32,
    spacing: f32,
    outline: &'static [(f32, f32)],
}

/// A cello: a 750 mm body, 690 mm of string (Arcus's tables).
const CELLO: Shape = Shape {
    body: 0.75,
    width: 0.44,
    bridge: 0.35,
    nut: 1.04,
    end: 1.25,
    board: 0.47,
    spacing: 0.016,
    outline: &[
        (0.0, 0.30),
        (0.02, 0.62),
        (0.08, 0.88),
        (0.2, 1.0),
        (0.32, 0.92),
        (0.40, 0.72),
        (0.47, 0.60),
        (0.54, 0.62),
        (0.60, 0.76),
        (0.70, 0.86),
        (0.80, 0.84),
        (0.90, 0.66),
        (0.97, 0.36),
        (1.0, 0.16),
    ],
};

/// A double bass: an 1 100 mm body with sloped shoulders, 1 060 mm of string.
const BASS: Shape = Shape {
    body: 1.10,
    width: 0.68,
    bridge: 0.55,
    nut: 1.61,
    end: 1.85,
    board: 0.70,
    spacing: 0.022,
    outline: &[
        (0.0, 0.28),
        (0.03, 0.6),
        (0.1, 0.88),
        (0.22, 1.0),
        (0.34, 0.93),
        (0.42, 0.76),
        (0.5, 0.68),
        (0.57, 0.72),
        (0.63, 0.78),
        (0.70, 0.74),
        (0.78, 0.62),
        (0.86, 0.46),
        (0.94, 0.28),
        (1.0, 0.14),
    ],
};

fn shape(i: Instrument) -> &'static Shape {
    match i {
        Instrument::Bass => &BASS,
        _ => &CELLO,
    }
}

/// The outline's half-width at `u` (0 … 1 along the body): a Catmull–Rom curve through its
/// points, so the bouts flow into the waist without a flat or a bump.
fn half(sh: &Shape, u: f32) -> f32 {
    let p = sh.outline;
    let u = u.clamp(0.0, 1.0);
    let i = p
        .windows(2)
        .position(|w| u <= w[1].0)
        .unwrap_or(p.len() - 2);
    let at = |j: isize| p[j.clamp(0, p.len() as isize - 1) as usize].1;
    let (w0, w1, w2, w3) = (at(i as isize - 1), at(i as isize), at(i as isize + 1), at(i as isize + 2));
    let (u1, u2) = (p[i].0, p[i + 1].0);
    let t = ((u - u1) / (u2 - u1).max(1e-6)).clamp(0.0, 1.0);
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * w1
        + (w2 - w0) * t
        + (2.0 * w0 - 5.0 * w1 + 4.0 * w2 - w3) * t2
        + (3.0 * w1 - w0 - 3.0 * w2 + w3) * t3)
}

/// The colours the instrument is drawn in, from the face's own (its amber's varnish) and the
/// display's ink: a dark (ebony, the f-holes) and a pale (the hair, the strings) that keep
/// their sense in both themes.
struct Ink {
    wood: Color32,
    wood_edge: Color32,
    glow: Color32,
    deep: Color32,
    pale: Color32,
    metal: Color32,
    lit: Color32,
    bow: Color32,
}

fn ink(s: &Style) -> Ink {
    let amber = crate::views::inst::face::voice2(s, s.hue);
    let garnet = crate::views::inst::face::accent_on(s, s.hue);
    let (deep, pale) = if s.t.dark {
        (s.t.ink, s.t.text)
    } else {
        (s.t.text, s.t.panel_light)
    };
    Ink {
        wood: mix(amber, deep, 0.42),
        wood_edge: mix(amber, deep, 0.7),
        glow: with_alpha(mix(amber, pale, 0.35), 0.55),
        deep: mix(deep, amber, 0.08),
        pale: mix(pale, amber, 0.12),
        metal: mix(s.t.text_dim, pale, 0.4),
        lit: amber,
        bow: garnet,
    }
}

/// Paint the instrument of `fs` playing `n` in `r`.
pub fn instrument(b: &mut Brush, r: Rect, fs: &ArcusSettings, n: &Now, s: &Style) {
    let sh = shape(fs.instrument);
    let k = (r.width() / sh.end).min(0.8 * r.height() / sh.width);
    let x0 = r.center().x - 0.5 * sh.end * k;
    let cy = r.center().y;
    let x = |m: f32| x0 + m * k;
    let c = ink(s);
    // ---- the body: its outline, its varnish's light, the purfling ----
    let steps = 120;
    let mut top = Vec::with_capacity(steps + 1);
    let mut bot = Vec::with_capacity(steps + 1);
    let mut top_in = Vec::with_capacity(steps + 1);
    let mut bot_in = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let u = i as f32 / steps as f32;
        let hw = 0.5 * sh.width * k * half(sh, u);
        let px = x(u * sh.body);
        top.push(pos2(px, cy - hw));
        bot.push(pos2(px, cy + hw));
        top_in.push(pos2(px, cy - (hw - s.s(3.5)).max(0.0)));
        bot_in.push(pos2(px, cy + (hw - s.s(3.5)).max(0.0)));
    }
    b.between(&top, &bot, c.wood);
    b.radial(
        r,
        pos2(x(0.24 * sh.body), cy - 0.12 * sh.width * k),
        vec2(0.32 * sh.body * k, 0.34 * sh.width * k),
        &[(0.0, c.glow), (1.0, with_alpha(c.glow, 0.0))],
        8,
    );
    b.polyline(&top, s.s(1.6), c.wood_edge);
    b.polyline(&bot, s.s(1.6), c.wood_edge);
    b.polyline(&top_in, s.s(0.8), with_alpha(c.deep, 0.6));
    b.polyline(&bot_in, s.s(0.8), with_alpha(c.deep, 0.6));
    // ---- the f-holes, either side of the bridge ----
    let xb = x(sh.bridge);
    for side in [-1.0f32, 1.0] {
        let yc = cy + side * 0.15 * sh.width * k;
        let len = 0.17 * sh.body * k;
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0 - 0.5;
                pos2(xb + t * len, yc + side * 0.035 * sh.width * k * (std::f32::consts::PI * t * 2.0).sin())
            })
            .collect();
        b.polyline(&pts, s.s(1.6), c.deep);
        b.circle(pts[0], s.s(2.2), Fill::Solid(c.deep));
        b.circle(pts[16], s.s(2.2), Fill::Solid(c.deep));
    }
    // ---- the neck, the fingerboard, the pegbox and the scroll ----
    let xn = x(sh.nut);
    let board_w = |m: f32| {
        let u = ((m - sh.board) / (sh.nut - sh.board)).clamp(0.0, 1.0);
        (4.6 - 1.4 * u) * sh.spacing * k
    };
    let neck = Rect::from_min_max(
        pos2(x(sh.body * 0.96), cy - 1.6 * sh.spacing * k),
        pos2(xn, cy + 1.6 * sh.spacing * k),
    );
    b.rrect(neck, s.r(2.0), Fill::Solid(c.wood));
    let board_top: Vec<Pos2> = (0..=8)
        .map(|i| {
            let m = sh.board + (sh.nut - sh.board) * i as f32 / 8.0;
            pos2(x(m), cy - 0.5 * board_w(m))
        })
        .collect();
    let board_bot: Vec<Pos2> = board_top.iter().map(|p| pos2(p.x, 2.0 * cy - p.y)).collect();
    b.between(&board_top, &board_bot, c.deep);
    let pegbox = Rect::from_min_max(
        pos2(xn, cy - 1.3 * sh.spacing * k),
        pos2(x(sh.end) - 2.2 * sh.spacing * k, cy + 1.3 * sh.spacing * k),
    );
    b.rrect(pegbox, s.r(2.0), Fill::Solid(c.wood_edge));
    let scroll = pos2(x(sh.end) - 1.6 * sh.spacing * k, cy);
    b.circle(scroll, 1.9 * sh.spacing * k, Fill::Solid(c.wood));
    b.circle_stroke(scroll, 1.9 * sh.spacing * k, s.s(1.2), c.wood_edge);
    b.circle_stroke(scroll, 1.0 * sh.spacing * k, s.s(1.0), c.wood_edge);
    for j in 0..4 {
        let px = pegbox.left() + pegbox.width() * (0.2 + 0.2 * j as f32);
        let py = if j % 2 == 0 { pegbox.top() } else { pegbox.bottom() };
        let tip = pos2(px, py + if j % 2 == 0 { -2.0 } else { 2.0 } * sh.spacing * k);
        b.segment(pos2(px, py), tip, s.s(3.0), c.deep, true);
    }
    // ---- the tailpiece and the bridge ----
    let tail = [
        pos2(x(0.03 * sh.body), cy - 0.9 * sh.spacing * k),
        pos2(x(sh.bridge - 0.2 * sh.body), cy - 2.0 * sh.spacing * k),
    ];
    let tail_b: Vec<Pos2> = tail.iter().map(|p| pos2(p.x, 2.0 * cy - p.y)).collect();
    b.between(&tail, &tail_b, c.deep);
    let bridge = Rect::from_center_size(
        pos2(xb, cy),
        vec2(s.s(4.0), 4.6 * sh.spacing * k),
    );
    b.rrect(bridge, s.r(1.0), Fill::Solid(c.pale));
    // ---- the strings: the sounding one lit and swinging ----
    let spec = fs.instrument.spec();
    let open = spec.strings[n.string].open_hz();
    let f = 440.0 * ((n.key as f64 - 69.0) / 12.0).exp2();
    let stop = (open / f).clamp(0.1, 1.0) as f32;
    let string_y = |i: usize, m: f32| {
        // Spread at the bridge, closer at the nut (the fingerboard narrows); the lowest
        // string at the bottom.
        let u = ((m - sh.bridge) / (sh.nut - sh.bridge)).clamp(0.0, 1.0);
        let gap = sh.spacing * k * (1.0 - 0.35 * u);
        cy + (1.5 - i as f32) * gap
    };
    let x_tail = tail[1].x;
    for i in 0..4 {
        let sounding = i == n.string && (n.live || n.level > 0.0);
        let w = s.s(1.0 + 0.35 * (3 - i) as f32);
        let col = if i == n.string { c.lit } else { c.metal };
        let ya = string_y(i, sh.bridge);
        b.segment(pos2(x_tail, ya), pos2(xb, ya), w, with_alpha(col, 0.7), false);
        b.segment(pos2(xb, ya), pos2(xn, string_y(i, sh.nut)), w, col, false);
        if sounding && n.level > 0.0 {
            // The string's swing between the bridge and the finger, as loud as it sounds.
            let xf = xb + (xn - xb) * stop;
            let amp = s.s(6.0) * n.level.sqrt().min(1.0);
            let lo: Vec<Pos2> = (0..=16)
                .map(|j| {
                    let t = j as f32 / 16.0;
                    let px = xb + (xf - xb) * t;
                    let py = string_y(i, sh.bridge + (sh.nut - sh.bridge) * stop * t);
                    pos2(px, py - amp * (std::f32::consts::PI * t).sin())
                })
                .collect();
            let hi: Vec<Pos2> = lo.iter().enumerate().map(|(j, p)| {
                let t = j as f32 / 16.0;
                pos2(p.x, p.y + 2.0 * amp * (std::f32::consts::PI * t).sin())
            }).collect();
            b.between(&lo, &hi, with_alpha(c.lit, 0.25));
        }
    }
    // ---- the left hand's finger where the note stops the string ----
    let xf = xb + (xn - xb) * stop;
    let yf = string_y(n.string, sh.bridge + (sh.nut - sh.bridge) * stop);
    if stop < 0.995 {
        let tip = Rect::from_center_size(pos2(xf, yf - s.s(5.0)), vec2(s.s(9.0), s.s(13.0)));
        b.rrect(tip, s.r(4.5), Fill::Solid(with_alpha(c.pale, 0.9)));
        b.rrect_stroke(tip, s.r(4.5), s.s(1.0), c.deep);
    }
    // ---- the bow, or the plucking finger ----
    let xc = xb + (xf - xb) * n.beta.clamp(0.0, 0.5);
    let yc = string_y(n.string, sh.bridge + (sh.nut - sh.bridge) * stop * n.beta);
    if fs.technique == Technique::Pizzicato {
        let tip = Rect::from_center_size(pos2(xc + s.s(10.0), yc - s.s(6.0)), vec2(s.s(10.0), s.s(14.0)));
        b.rrect(tip, s.r(5.0), Fill::Solid(with_alpha(c.pale, 0.9)));
        b.rrect_stroke(tip, s.r(5.0), s.s(1.0), c.bow);
        for j in 0..3 {
            let dx = s.s(4.0 + 4.0 * j as f32);
            b.segment(pos2(xc + s.s(16.0) + dx, yc - s.s(14.0)), pos2(xc + s.s(16.0) + dx, yc - s.s(8.0)), s.s(1.0), c.bow, true);
        }
        return;
    }
    bow(b, r, s, &c, fs, n, pos2(xc, yc), sh, k);
}

/// The bow across the string at `at`: tilted to reach that string over the bridge's arch,
/// its hair pressed in as hard as it presses, its arrow its speed and direction.
#[allow(clippy::too_many_arguments)]
fn bow(
    b: &mut Brush,
    r: Rect,
    s: &Style,
    c: &Ink,
    fs: &ArcusSettings,
    n: &Now,
    at: Pos2,
    sh: &Shape,
    k: f32,
) {
    // The bow crosses at an angle that reaches its string over the arched bridge: the
    // lowest string with the bow tipped one way, the highest the other.
    let tilt = (n.string as f32 - 1.5) * 0.11;
    let dir = vec2(tilt.sin(), tilt.cos());
    let len = (0.75 * sh.width * k).min(r.height() * 1.05);
    let press = (n.force / n.hi.max(1e-3)).clamp(0.0, 1.5);
    let hair_a = at - dir * (0.5 * len);
    let hair_b = at + dir * (0.5 * len);
    // The stick, a little off the hair, bending towards it as the bow presses.
    let normal = vec2(dir.y, -dir.x);
    let gap = s.s(7.0) * (1.0 - 0.45 * press.min(1.0));
    let stick: Vec<Pos2> = (0..=12)
        .map(|j| {
            let t = j as f32 / 12.0;
            let bend = 1.0 - (2.0 * t - 1.0).powi(2);
            hair_a + (hair_b - hair_a) * t + normal * (gap + s.s(4.0) * (1.0 - bend))
        })
        .collect();
    b.polyline(&stick, s.s(2.6), c.wood_edge);
    // The hair: pale, its contact on the string darker and wider as it presses.
    b.segment(hair_a, hair_b, s.s(3.0), with_alpha(c.pale, 0.95), true);
    let contact = s.s(4.0 + 6.0 * press.min(1.0));
    b.segment(
        at - dir * contact,
        at + dir * contact,
        s.s(3.0 + 2.0 * press.min(1.2)),
        mix(c.pale, c.bow, (0.4 + 0.5 * press).min(1.0)),
        true,
    );
    // The frog at the bow's lower end.
    let frog = Rect::from_center_size(hair_b + normal * s.s(4.0), vec2(s.s(10.0), s.s(16.0)));
    b.rrect(frog, s.r(2.0), Fill::Solid(c.deep));
    // Its motion: down-bow (the frog moving away) or up; tremolo's short strokes.
    let speed = (n.speed.abs() / 0.35).clamp(0.0, 1.0);
    let along = if n.speed < 0.0 { -1.0 } else { 1.0 };
    let base = at + normal * s.s(22.0);
    let arrow = |b: &mut Brush, from: Pos2, l: f32, sign: f32| {
        let to = from + dir * (sign * l);
        b.segment(from, to, s.s(1.8), c.bow, true);
        let back = dir * (-sign * s.s(5.0));
        b.segment(to, to + back + normal * s.s(3.5), s.s(1.8), c.bow, true);
        b.segment(to, to + back - normal * s.s(3.5), s.s(1.8), c.bow, true);
    };
    if fs.technique == Technique::Tremolo {
        arrow(b, base, s.s(10.0), 1.0);
        arrow(b, base + normal * s.s(9.0), s.s(10.0), -1.0);
    } else if speed > 0.02 {
        arrow(b, base, s.s(8.0) + 0.3 * len * speed, along);
    }
}
