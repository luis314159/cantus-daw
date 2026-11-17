//! Cedo's bed and its fittings (design sheet v1): the bed the engine and the screen stand
//! on, in the colourway's finish (hammered enamel, gloss black lined in red, a mahogany
//! plinth edged in brass) with its chamfer and corner bolts; the cast brass nameplate with
//! its beaded border and raised letters; the readout flags (a word lit on a dark drum
//! behind a small window, its name painted over it); the pilot lamp that flashes on every
//! hit; the screen's riveted frame and its glass. Original to Cantus.

use super::super::canvas;
use super::art::{self, c, hash, lit};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::engines as pal;
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

/// The bed in `r`: its finish, its chamfer, a bolt in each corner.
pub fn bed(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let f = art::finish(s);
    let rad = 7.0 * k;
    b.rrect(
        r.translate(vec2(0.0, 2.0 * k)),
        rad,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.rrect(
        r,
        rad,
        Fill::Stops(&[
            (0.0, c(f.bed.light)),
            (0.10, c(f.bed.mid)),
            (0.88, c(f.bed.mid)),
            (1.0, c(f.bed.dark)),
        ]),
    );
    match f.surface {
        pal::Surface::Hammered => hammered(b, k, r.shrink(4.0 * k), f),
        pal::Surface::Gloss => gloss(b, k, r, f),
        pal::Surface::Wood => wood(b, k, r, f),
    }
    // The chamfer: lit along the top, shadowed along the foot.
    b.rrect_stroke(r, rad, 1.0 * k, with_alpha(c(f.edge), 0.55));
    b.hline(
        r.left() + rad,
        r.right() - rad,
        r.top() + 1.2 * k,
        1.0 * k,
        with_alpha(c(f.edge), 0.8),
    );
    b.hline(
        r.left() + rad,
        r.right() - rad,
        r.bottom() - 1.2 * k,
        1.4 * k,
        with_alpha(c(pal::SHADOW), 0.7),
    );
    let inset = 13.0 * k;
    for (i, p) in [
        pos2(r.left() + inset, r.top() + inset),
        pos2(r.right() - inset, r.top() + inset),
        pos2(r.left() + inset, r.bottom() - inset),
        pos2(r.right() - inset, r.bottom() - inset),
    ]
    .into_iter()
    .enumerate()
    {
        art::bolt(b, p, 4.2 * k, hash(i as u32 + 7), &f.steel);
    }
}

/// Hammered enamel: shallow dimples, each a shadow on its upper side and a light below.
fn hammered(b: &mut Brush, k: f32, r: Rect, f: &pal::Finish) {
    let cell = 26.0 * k;
    let cols = (r.width() / cell).ceil() as u32;
    let rows = (r.height() / cell).ceil() as u32;
    let dark = c(f.bed.dark);
    let light = c(f.grain);
    for j in 0..rows {
        for i in 0..cols {
            let n = j * 977 + i;
            let p = pos2(
                r.left() + (i as f32 + hash(n)) * cell,
                r.top() + (j as f32 + hash(n + 31_337)) * cell,
            );
            if !r.contains(p) {
                continue;
            }
            let rad = (6.0 + 6.0 * hash(n + 99)) * k;
            b.circle(
                p,
                rad,
                Fill::V(with_alpha(dark, 0.16), with_alpha(light, 0.10)),
            );
        }
    }
    // The enamel's soft sheen across the top.
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + r.height() * 0.3)),
        Fill::V(with_alpha(light, 0.10), with_alpha(light, 0.0)),
    );
}

/// Gloss lacquer: a long reflection and the lining.
fn gloss(b: &mut Brush, k: f32, r: Rect, f: &pal::Finish) {
    let g = c(f.grain);
    let (w, h) = (r.width(), r.height());
    b.polygon(
        &[
            pos2(r.left() + w * 0.18, r.top()),
            pos2(r.left() + w * 0.30, r.top()),
            pos2(r.left() + w * 0.22, r.bottom()),
            pos2(r.left() + w * 0.10, r.bottom()),
        ],
        with_alpha(g, 0.035),
    );
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + h * 0.18)),
        Fill::V(with_alpha(g, 0.12), with_alpha(g, 0.0)),
    );
    let line = r.shrink(7.0 * k);
    b.rrect_stroke(line, 4.0 * k, 1.6 * k, with_alpha(c(f.lining), 0.95));
    b.rrect_stroke(
        line.shrink(3.5 * k),
        3.0 * k,
        0.6 * k,
        with_alpha(c(f.lining), 0.55),
    );
}

/// A polished plinth: the grain in long curves, a brass edging round it.
fn wood(b: &mut Brush, k: f32, r: Rect, f: &pal::Finish) {
    let lines = ((r.height() / (9.0 * k)) as u32).clamp(8, 90);
    let mut pts = [Pos2::ZERO; 48];
    for i in 0..lines {
        let base = r.top() + r.height() * (i as f32 + 0.5) / lines as f32;
        let amp = (2.0 + 6.0 * hash(i + 3)) * k;
        let freq = 1.5 + 2.5 * hash(i + 17);
        let ph = 6.28 * hash(i + 41);
        for (j, p) in pts.iter_mut().enumerate() {
            let t = j as f32 / 47.0;
            let y = base + amp * cantus_dsp::math::sin(ph + freq * 6.28 * t);
            *p = pos2(r.left() + r.width() * t, y.clamp(r.top(), r.bottom()));
        }
        let (col, a) = if i % 3 == 0 {
            (c(f.grain), 0.30)
        } else {
            (c(f.bed.dark), 0.28)
        };
        b.polyline(&pts, (0.6 + 0.8 * hash(i + 5)) * k, with_alpha(col, a));
    }
    // The varnish's sheen.
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + r.height() * 0.25)),
        Fill::V(
            with_alpha(c(pal::GLINT), 0.10),
            with_alpha(c(pal::GLINT), 0.0),
        ),
    );
    let edge = r.shrink(2.5 * k);
    b.rrect_stroke(edge, 6.0 * k, 3.0 * k, c(f.brass.mid));
    b.rrect_stroke(edge.shrink(1.2 * k), 5.0 * k, 0.7 * k, c(f.brass.light));
}

/// The cast nameplate at `at` (its left centre): a brass oval with a beaded border, the
/// maker's name raised on it and the engine's kind beside. Returns where it went.
pub fn nameplate(b: &mut Brush, s: &Style, at: Pos2, h: f32) -> Rect {
    let k = s.k;
    let f = art::finish(s);
    let big = 1.7;
    let name = Text::key("cedo.plate.name", Role::Title, Color32::TRANSPARENT).caps(true);
    let kind = Text::key("cedo.plate.kind", Role::Micro, Color32::TRANSPARENT).caps(true);
    let nw = ktext::measure(b.ctx(), &name, k * big);
    let kw = ktext::measure(b.ctx(), &kind, k);
    let pad = 16.0 * k;
    let gap = 14.0 * k;
    let w = pad * 2.0 + nw.x + gap + kw.x + 8.0 * k;
    let r = Rect::from_min_size(pos2(at.x, at.y - h * 0.5), vec2(w, h));
    let rad = h * 0.5;
    art::shadow(
        b,
        r.expand(8.0 * k),
        r.center() + vec2(0.0, 3.0 * k),
        r.size() * 0.56,
        0.9,
    );
    b.rrect(r, rad, Fill::V(c(f.brass.light), c(f.brass.dark)));
    let ground = r.shrink(4.0 * k);
    b.rrect(
        ground,
        rad - 4.0 * k,
        Fill::V(mix(c(f.name_ground), c(pal::SHADOW), 0.3), c(f.name_ground)),
    );
    // The beading: small domes round the ground's edge.
    let bead = 1.2 * k;
    let mut x = ground.left() + rad;
    while x < ground.right() - rad {
        for y in [ground.top() + 3.0 * k, ground.bottom() - 3.0 * k] {
            b.circle(pos2(x, y), bead, Fill::V(c(f.brass.light), c(f.brass.dark)));
        }
        x += 6.0 * k;
    }
    for side in [0.0f32, 1.0] {
        let cx = if side == 0.0 {
            ground.left() + rad
        } else {
            ground.right() - rad
        };
        for i in 0..9 {
            let a = std::f32::consts::PI
                * (0.5 + i as f32 / 8.0)
                * if side == 0.0 { 1.0 } else { -1.0 };
            let p = pos2(
                cx + cantus_dsp::math::cos(a) * (rad - 7.0 * k),
                ground.center().y + cantus_dsp::math::sin(a) * (rad - 7.0 * k),
            );
            b.circle(p, bead, Fill::V(c(f.brass.light), c(f.brass.dark)));
        }
    }
    // The raised letters: a shadow under, the brass's light on top.
    let y = ground.center().y;
    let x0 = ground.left() + pad;
    for (d, col) in [
        (vec2(0.8, 1.4) * k, with_alpha(c(pal::SHADOW), 0.8)),
        (vec2(0.0, 0.0), c(f.brass.light)),
    ] {
        canvas::put(
            b,
            pos2(x0, y) + d,
            Align2::LEFT_CENTER,
            &Text { color: col, ..name },
            k * big,
            None,
        );
    }
    let kx = x0 + nw.x + gap;
    b.vline(
        kx - gap * 0.5,
        ground.top() + 8.0 * k,
        ground.bottom() - 8.0 * k,
        1.0 * k,
        with_alpha(c(f.brass.mid), 0.8),
    );
    for (d, col) in [
        (vec2(0.5, 0.9) * k, with_alpha(c(pal::SHADOW), 0.8)),
        (vec2(0.0, 0.0), lit(&f.brass, 0.6)),
    ] {
        canvas::put(
            b,
            pos2(kx, y) + d,
            Align2::LEFT_CENTER,
            &Text { color: col, ..kind },
            k,
            None,
        );
    }
    r
}

/// A readout flag at `right` (its right edge, centred on the strip's middle): its name
/// painted on the bed and its value lit on a dark drum in a brass window. Returns its rect.
#[allow(clippy::too_many_arguments)]
pub fn flag(
    b: &mut Brush,
    s: &Style,
    right: Pos2,
    name: &str,
    value: &str,
    light: Color32,
    h: f32,
) -> Rect {
    let k = s.k;
    let f = art::finish(s);
    let label = Text::new(name, Role::Micro, with_alpha(c(pal::ENAMEL), 0.85)).caps(true);
    let val = Text::new(value, Role::Value, light).caps(false);
    let lw = ktext::measure(b.ctx(), &label, k);
    let vw = ktext::measure(b.ctx(), &val, k);
    let win_w = vw.x + 18.0 * k;
    let win = Rect::from_min_size(pos2(right.x - win_w, right.y - h * 0.5), vec2(win_w, h));
    // The window: a brass bezel, the drum's curve (dark at its edges), the value lit.
    b.rrect(
        win.expand(2.5 * k),
        4.0 * k,
        Fill::V(c(f.brass.light), c(f.brass.dark)),
    );
    b.rrect(
        win,
        2.5 * k,
        Fill::Stops(&[
            (0.0, c(pal::SHADOW)),
            (0.35, c(pal::SCREEN_BACK)),
            (0.65, c(pal::SCREEN_BACK)),
            (1.0, c(pal::SHADOW)),
        ]),
    );
    b.radial(
        win,
        win.center(),
        vec2(win.width() * 0.6, win.height() * 0.8),
        &[
            (0.0, with_alpha(light, 0.16)),
            (1.0, with_alpha(light, 0.0)),
        ],
        8,
    );
    canvas::put(b, win.center(), Align2::CENTER_CENTER, &val, k, None);
    b.hline(
        win.left() + 2.0 * k,
        win.right() - 2.0 * k,
        win.top() + 1.0 * k,
        0.8 * k,
        with_alpha(c(pal::GLINT), 0.25),
    );
    let lr = canvas::put(
        b,
        pos2(win.left() - 8.0 * k, win.center().y),
        Align2::RIGHT_CENTER,
        &label,
        k,
        None,
    );
    let _ = lw;
    Rect::from_min_max(pos2(lr.left(), win.top()), win.max)
}

/// The width a flag takes, for the caller laying them out.
pub fn flag_width(b: &Brush, k: f32, name: &str, value: &str) -> f32 {
    let label = Text::new(name, Role::Micro, Color32::TRANSPARENT).caps(true);
    let val = Text::new(value, Role::Value, Color32::TRANSPARENT).caps(false);
    ktext::measure(b.ctx(), &label, k).x + 8.0 * k + ktext::measure(b.ctx(), &val, k).x + 18.0 * k
}

/// The pilot lamp at `p`: a domed lens in a knurled bezel, lit `on` (0 … 1) in `light`.
pub fn lamp(b: &mut Brush, s: &Style, p: Pos2, rad: f32, light: Color32, on: f32) {
    let k = s.k;
    let f = art::finish(s);
    let on = on.clamp(0.0, 1.0);
    if on > 0.0 {
        b.radial(
            Rect::from_center_size(p, Vec2::splat(rad * 8.0)),
            p,
            Vec2::splat(rad * 3.6),
            &[
                (0.0, with_alpha(light, 0.45 * on)),
                (1.0, with_alpha(light, 0.0)),
            ],
            8,
        );
    }
    b.circle(p, rad * 1.55, Fill::V(c(f.steel.light), c(f.steel.dark)));
    let teeth = 24;
    for i in 0..teeth {
        let a = std::f32::consts::TAU * i as f32 / teeth as f32;
        let d = art::dir(a);
        b.segment(
            p + d * rad * 1.3,
            p + d * rad * 1.55,
            0.6 * k,
            with_alpha(c(f.steel.dark), 0.8),
            false,
        );
    }
    let off = c(pal::LAMP_OFF);
    let lens = mix(off, light, 0.25 + 0.75 * on);
    b.circle(
        p,
        rad,
        Fill::V(
            mix(lens, c(pal::GLINT), 0.25 * on),
            mix(lens, c(pal::SHADOW), 0.35),
        ),
    );
    b.circle(
        p + vec2(-0.3, -0.35) * rad,
        rad * 0.32,
        Fill::Solid(with_alpha(c(pal::GLINT), 0.5 + 0.3 * on)),
    );
}

/// The screen's riveted frame round `r`; returns the glass inside it.
pub fn frame(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = art::finish(s);
    let band = 11.0 * k;
    art::shadow(
        b,
        r.expand(12.0 * k),
        r.center() + vec2(0.0, 5.0 * k),
        r.size() * 0.55,
        0.8,
    );
    b.rrect(r, 6.0 * k, Fill::V(lit(&f.iron, 0.8), c(f.iron.dark)));
    b.rrect_stroke(r, 6.0 * k, 0.8 * k, with_alpha(c(pal::SHADOW), 0.8));
    b.hline(
        r.left() + 6.0 * k,
        r.right() - 6.0 * k,
        r.top() + 1.0 * k,
        0.9 * k,
        lit(&f.iron, 1.0),
    );
    if f.surface == pal::Surface::Gloss {
        b.rrect_stroke(
            r.shrink(band * 0.5),
            4.0 * k,
            0.9 * k,
            with_alpha(c(f.lining), 0.95),
        );
    }
    let glass = r.shrink(band);
    // The inner bevel: shadowed at the top, lit at the foot.
    b.rrect(
        glass.expand(1.6 * k),
        3.0 * k,
        Fill::V(c(pal::SHADOW), lit(&f.iron, 0.9)),
    );
    // Rivets along the band, evenly, the corners first.
    let mid = band * 0.5;
    let rr = 2.1 * k;
    let along = |a: Pos2, z: Pos2, b: &mut Brush| {
        let len = (z - a).length();
        let n = (len / (34.0 * k)).round().max(1.0) as usize;
        for i in 0..=n {
            art::rivet(b, art::lerp(a, z, i as f32 / n as f32), rr, &f.brass);
        }
    };
    let (l, t, rt, bt) = (
        r.left() + mid,
        r.top() + mid,
        r.right() - mid,
        r.bottom() - mid,
    );
    along(pos2(l, t), pos2(rt, t), b);
    along(pos2(l, bt), pos2(rt, bt), b);
    along(pos2(l, t), pos2(l, bt), b);
    along(pos2(rt, t), pos2(rt, bt), b);
    glass
}

/// Glass over the screen: a reflection along its top and a faint diagonal sheen.
pub fn glass(b: &mut Brush, r: Rect, k: f32) {
    let g = c(pal::GLINT);
    let (w, h) = (r.width(), r.height());
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + h * 0.16)),
        Fill::V(with_alpha(g, 0.07), with_alpha(g, 0.0)),
    );
    let clip = |q: Pos2| {
        pos2(
            q.x.clamp(r.left(), r.right()),
            q.y.clamp(r.top(), r.bottom()),
        )
    };
    b.polygon(
        &[
            pos2(r.left() + w * 0.70, r.top()),
            pos2(r.left() + w * 0.80, r.top()),
            pos2(r.left() + w * 0.64, r.bottom()),
            pos2(r.left() + w * 0.54, r.bottom()),
        ]
        .map(clip),
        with_alpha(g, 0.025),
    );
    b.hline(
        r.left() + 3.0 * k,
        r.right() - 3.0 * k,
        r.top() + 0.6 * k,
        0.8 * k,
        with_alpha(g, 0.18),
    );
}
