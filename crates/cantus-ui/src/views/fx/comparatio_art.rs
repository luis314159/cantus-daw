//! Comparatio's objects (design sheet v1): the console on its felt desk — two cheeks (walnut,
//! blue-grey caps or teak, by colourway) with their grain, the faceplate between them (its
//! fine brushing, a chamfer, slotted screws), the decks' meter windows behind smoked glass,
//! the big chrome selector with its pointer and the two jewel lamps over its positions, the
//! blind drawer's keycaps. Vectors at the style's scale, colours from
//! `kit::skin::comparator` (one console per colourway); nothing animates by itself.
//! Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::comparator as pal;
use crate::kit::skin::Material;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Vec2};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The console's finish for the face's colourway.
pub fn console(s: &Style) -> &'static pal::Console {
    match s.skin.material {
        Material::Brushed => &pal::STEEL,
        Material::Paper => &pal::IVORY,
        _ => &pal::WALNUT,
    }
}

/// Your mix's voice and the reference's, lit on dark glass.
pub fn voices(s: &Style) -> (Color32, Color32) {
    (
        s.skin.accent_in(true).unwrap_or_else(|| c(pal::SCREEN_INK)),
        s.skin
            .accent2_in(true)
            .unwrap_or_else(|| c(pal::SCREEN_INK)),
    )
}

pub fn shadow() -> Color32 {
    c(pal::SHADOW)
}

/// The felt desk over `r`: a dark ground with a soft light from above and a faint weave.
pub fn desk(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    b.rect(r, Fill::V(c(pal::DESK_LIGHT), c(pal::DESK)));
    let mut y = r.top() + 3.0 * k;
    let mut n = 0u32;
    while y < r.bottom() {
        b.hline(
            r.left(),
            r.right(),
            y,
            0.5 * k,
            with_alpha(c(pal::DESK_LIGHT), 0.10 + 0.08 * hash(n)),
        );
        y += 3.0 * k;
        n += 1;
    }
}

/// A cheek over `r` (a rounded end of solid wood or a metal cap): lit edge, grain curves,
/// its shadow. `left`: which way its rounded side faces.
pub fn cheek(b: &mut Brush, s: &Style, r: Rect, left: bool) {
    let k = s.k;
    let f = console(s);
    b.rrect_soft(
        r.translate(vec2(3.0 * k, 8.0 * k)),
        10.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.6)),
        12.0 * k,
    );
    b.rrect(r, 10.0 * k, Fill::V(c(f.cheek_light), c(f.cheek_dark)));
    let inner = r.shrink(2.0 * k);
    b.rrect(
        inner,
        9.0 * k,
        Fill::V(c(f.cheek), mix(c(f.cheek), c(f.cheek_dark), 0.6)),
    );
    // Grain: long gently waving lines down its length.
    let lines = (inner.width() / (3.2 * k)) as usize;
    for i in 0..lines {
        let x0 = inner.left() + (i as f32 + 0.5) * inner.width() / lines as f32;
        let amp = (1.0 + 2.5 * hash(i as u32 + 7)) * k;
        let ph = hash(i as u32 + 99) * 6.0;
        let pts: Vec<Pos2> = (0..=24)
            .map(|j| {
                let y = inner.top() + 6.0 * k + (inner.height() - 12.0 * k) * j as f32 / 24.0;
                pos2(x0 + amp * sin(j as f32 * 0.5 + ph), y)
            })
            .collect();
        b.polyline(
            &pts,
            0.6 * k,
            with_alpha(c(f.cheek_grain), 0.30 + 0.25 * hash(i as u32)),
        );
    }
    // The polish: a light along the rounded side.
    let x = if left {
        inner.left() + 3.0 * k
    } else {
        inner.right() - 3.0 * k
    };
    b.vline(
        x,
        inner.top() + 10.0 * k,
        inner.bottom() - 10.0 * k,
        1.2 * k,
        with_alpha(c(f.cheek_light), 0.5),
    );
}

/// The faceplate over `r`: its chamfer, the brushing, a slotted screw at each corner.
/// Returns its face.
pub fn faceplate(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = console(s);
    b.rect(r, Fill::V(c(f.edge), c(f.foot)));
    let face = r.shrink(2.0 * k);
    b.rect(face, Fill::V(c(f.top), c(f.face)));
    let mut y = face.top() + 2.0 * k;
    let mut n = 0u32;
    while y < face.bottom() {
        b.hline(
            face.left(),
            face.right(),
            y,
            0.5 * k,
            with_alpha(c(f.grain), 0.04 + 0.05 * hash(n + 40)),
        );
        y += 1.8 * k;
        n += 1;
    }
    for p in [
        face.left_top() + vec2(12.0, 12.0) * k,
        face.right_top() + vec2(-12.0, 12.0) * k,
        face.left_bottom() + vec2(12.0, -12.0) * k,
        face.right_bottom() + vec2(-12.0, -12.0) * k,
    ] {
        screw(b, s, p, 4.5 * k, hash((p.x + p.y) as u32));
    }
    face
}

/// A slotted screw at `p`, its slot turned `turn` (0 … 1).
pub fn screw(b: &mut Brush, s: &Style, p: Pos2, rad: f32, turn: f32) {
    let f = console(s);
    b.circle(
        p + vec2(0.0, rad * 0.25),
        rad * 1.15,
        Fill::Solid(with_alpha(shadow(), 0.4)),
    );
    b.circle(p, rad, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)));
    b.circle(p, rad * 0.78, Fill::V(c(pal::CHROME), c(pal::CHROME_LIGHT)));
    let a = turn * std::f32::consts::PI;
    let d = vec2(cos(a), sin(a)) * rad * 0.75;
    b.segment(p - d, p + d, rad * 0.26, c(pal::CHROME_DARK), false);
    let _ = f;
}

/// A window behind smoked glass over `r` (a deck's meters, the screen): a bezel, the glass,
/// its reflection drawn by [`glass_sheen`] last. Returns the glass.
pub fn window(b: &mut Brush, s: &Style, r: Rect, rad: f32) -> Rect {
    let k = s.k;
    let f = console(s);
    b.rrect(
        r.expand(2.0 * k),
        rad + 2.0 * k,
        Fill::V(c(f.foot), c(f.edge)),
    );
    b.rrect(r, rad, Fill::V(mix(c(f.foot), shadow(), 0.7), c(f.foot)));
    let g = r.shrink(3.0 * k);
    b.rrect(
        g,
        (rad - 2.0 * k).max(1.0),
        Fill::V(c(pal::GLASS_TOP), c(pal::GLASS_FOOT)),
    );
    g
}

/// The glass's reflection over `g`: a soft band along its top.
pub fn glass_sheen(b: &mut Brush, s: &Style, g: Rect) {
    let k = s.k;
    let band = Rect::from_min_max(g.min, pos2(g.right(), g.top() + g.height() * 0.22));
    b.rect(
        band.shrink2(vec2(2.0 * k, 1.0 * k)),
        Fill::V(
            with_alpha(c(pal::SHEEN), 0.06),
            with_alpha(c(pal::SHEEN), 0.0),
        ),
    );
    b.rrect_stroke(g, 3.0 * k, 0.7 * k, with_alpha(c(pal::SHEEN), 0.10));
}

/// Words printed on the faceplate at `at`.
pub fn print(b: &mut Brush, s: &Style, at: Pos2, align: Align2, t: &Text<'_>, scale: f32) -> Rect {
    let t = Text {
        color: c(console(s).print),
        ..*t
    };
    super::super::canvas::put(b, at, align, &t, s.k * scale, None)
}

/// A jewel lamp at `p`: its chrome bezel, its lens lit by `lit` (0 … 1) in `color`.
pub fn jewel(b: &mut Brush, s: &Style, p: Pos2, rad: f32, lit: f32, color: Color32) {
    let k = s.k;
    b.circle(
        p + vec2(0.0, 1.0 * k),
        rad + 3.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.4)),
    );
    b.circle(
        p,
        rad + 2.5 * k,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    let lit = lit.clamp(0.0, 1.0);
    b.circle(p, rad, Fill::Solid(mix(c(pal::LAMP_OFF), color, lit)));
    if lit > 0.05 {
        b.radial(
            Rect::from_center_size(p, Vec2::splat(rad * 7.0)),
            p,
            Vec2::splat(rad * 3.4),
            &[
                (0.0, with_alpha(color, 0.35 * lit)),
                (1.0, with_alpha(color, 0.0)),
            ],
            8,
        );
        b.circle(
            p,
            rad * 0.5,
            Fill::Solid(mix(color, c(pal::LAMP_HOT), 0.55 * lit)),
        );
    }
    // The jewel's cut: a star of fine facets and a glint.
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::FRAC_PI_3;
        let d = vec2(cos(a), sin(a)) * rad;
        b.segment(p, p + d, 0.4 * k, with_alpha(c(pal::GLINT), 0.18), false);
    }
    b.dot(
        p + vec2(-0.35, -0.4) * rad,
        rad * 0.25,
        with_alpha(c(pal::GLINT), 0.9),
    );
}

/// The big selector at `p`, radius `rad`, its pointer at `angle` (radians from up): a
/// knurled chrome skirt, a domed cap, a long pointer with a white line, its shadow.
pub fn selector(b: &mut Brush, s: &Style, p: Pos2, rad: f32, angle: f32, hot: bool) {
    let k = s.k;
    let f = console(s);
    b.circle(
        p + vec2(2.0 * k, 6.0 * k),
        rad * 1.05,
        Fill::Solid(with_alpha(shadow(), 0.5)),
    );
    b.circle(p, rad, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)));
    let n = 48;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let d = vec2(cos(a), sin(a));
        b.segment(
            p + d * rad * 0.86,
            p + d * rad,
            0.8 * k,
            with_alpha(c(pal::CHROME_DARK), 0.55),
            false,
        );
    }
    b.circle(p, rad * 0.84, Fill::V(c(pal::CHROME), c(pal::CHROME_DARK)));
    b.circle(p, rad * 0.62, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME)));
    // The pointer: a bar across the cap, longer toward its tip.
    let d = vec2(sin(angle), -cos(angle));
    let tip = p + d * rad * 1.18;
    let tail = p - d * rad * 0.55;
    b.segment(
        tail + vec2(1.5, 3.0) * k,
        tip + vec2(1.5, 3.0) * k,
        rad * 0.30,
        with_alpha(shadow(), 0.45),
        true,
    );
    b.segment(tail, tip, rad * 0.28, c(pal::CHROME_DARK), true);
    b.segment(
        tail,
        tip,
        rad * 0.22,
        mix(c(f.face), c(pal::CHROME_DARK), 0.4),
        true,
    );
    b.segment(p, tip - d * rad * 0.08, 1.6 * k, c(pal::CHROME_LIGHT), true);
    b.dot(
        p + vec2(-0.3, -0.35) * rad * 0.6,
        rad * 0.10,
        with_alpha(c(pal::GLINT), 0.8),
    );
    if hot {
        b.circle_stroke(p, rad * 1.06, 1.0 * k, with_alpha(c(f.edge), 0.85));
    }
}

/// A keycap over `r` with its engraved `letter`, lit (`lit`) by `color`, its state marked
/// by a lamp strip under the letter (`mark`: a colour for right or wrong).
pub fn keycap(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    letter: &str,
    lit: bool,
    hot: bool,
    color: Color32,
) {
    let k = s.k;
    let f = console(s);
    b.rrect(
        r.translate(vec2(0.0, 2.5 * k)),
        6.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.55)),
    );
    let cap = if lit {
        r.translate(vec2(0.0, 1.5 * k))
    } else {
        r
    };
    b.rrect(cap, 6.0 * k, Fill::V(c(pal::KEY_TOP), c(pal::KEY_BODY)));
    let top = cap
        .shrink2(vec2(4.0 * k, 3.0 * k))
        .translate(vec2(0.0, -1.5 * k));
    b.rrect(
        top,
        4.0 * k,
        Fill::V(mix(c(pal::KEY_TOP), c(pal::GLINT), 0.06), c(pal::KEY_TOP)),
    );
    let ink = if lit { color } else { c(pal::KEY_INK) };
    let t = Text::new(letter, Role::Title, ink).caps(true);
    super::super::canvas::put(b, top.center(), Align2::CENTER_CENTER, &t, k * 1.1, None);
    if lit {
        b.rrect_soft(top, 4.0 * k, Fill::Solid(with_alpha(color, 0.18)), 6.0 * k);
    }
    if hot {
        b.rrect_stroke(
            r.expand(1.5 * k),
            7.0 * k,
            0.9 * k,
            with_alpha(c(f.edge), 0.9),
        );
    }
}
