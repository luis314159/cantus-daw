//! Purus's objects (design sheet v1): the restorer's self-healing cutting mat (a millimetre
//! grid, the centimetres bolder, a printed rule along its edge), the instrument's machined
//! housing (a milled face with a chamfer that catches the light, cap screws, its feet's
//! shadows on the mat), the screen's bezel and glass, the maker's plate. Vectors at the
//! style's scale, colours from `kit::skin::restoration` (one finish and one mat per
//! colourway); nothing animates by itself. Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::restoration as pal;
use crate::kit::skin::Material;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i` (a speck, a screw's turn): the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The housing's finish for the face's colourway.
pub fn finish(s: &Style) -> &'static pal::Finish {
    match s.skin.material {
        Material::Matte => &pal::ENAMEL,
        Material::Carbon => &pal::GRAPHITE,
        _ => &pal::CHAMPAGNE,
    }
}

/// The mat for the face's colourway.
pub fn mat_of(s: &Style) -> &'static pal::Mat {
    match s.skin.material {
        Material::Matte => &pal::MAT_NAVY,
        Material::Carbon => &pal::MAT_SLATE,
        _ => &pal::MAT_GREEN,
    }
}

/// The residue's colour (what is taken out), lit on the dark screen.
pub fn residue(s: &Style) -> Color32 {
    s.skin.accent_in(true).unwrap_or_else(|| c(pal::SCREEN_INK))
}

/// The hum's needles' colour, lit on the dark screen.
pub fn needle(s: &Style) -> Color32 {
    s.skin
        .accent2_in(true)
        .unwrap_or_else(|| c(pal::SCREEN_INK))
}

pub fn shadow() -> Color32 {
    c(pal::SHADOW)
}

/// The cutting mat over `r`: its ground, a grid of millimetres (every 5 px at 100 %) with
/// the centimetres bolder, a rule printed along its top edge with every 5 cm numbered, and
/// the faint scuffs of a blade.
pub fn mat(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let m = mat_of(s);
    b.rect(r, Fill::V(c(m.ground), mix(c(m.ground), shadow(), 0.25)));
    let step = 5.0 * k;
    let fine = with_alpha(c(m.fine), 0.55);
    let bold = with_alpha(c(m.bold), 0.55);
    let mut i = 0;
    let mut x = r.left() + step;
    while x < r.right() {
        let w = if i % 10 == 9 { 0.9 * k } else { 0.5 * k };
        b.vline(
            b.snap_mid(x),
            r.top(),
            r.bottom(),
            w,
            if i % 10 == 9 { bold } else { fine },
        );
        x += step;
        i += 1;
    }
    let mut j = 0;
    let mut y = r.top() + step;
    while y < r.bottom() {
        let w = if j % 10 == 9 { 0.9 * k } else { 0.5 * k };
        b.hline(
            r.left(),
            r.right(),
            b.snap_mid(y),
            w,
            if j % 10 == 9 { bold } else { fine },
        );
        y += step;
        j += 1;
    }
    // The rule printed along its top edge: a tick a millimetre, longer at five, a number
    // each centimetre.
    let ink = with_alpha(c(m.print), 0.75);
    let mut i = 0;
    let mut x = r.left() + 6.0 * k;
    while x < r.right() - 6.0 * k {
        let h = if i % 10 == 0 {
            7.0
        } else if i % 5 == 0 {
            5.0
        } else {
            3.0
        } * k;
        b.vline(b.snap_mid(x), r.top(), r.top() + h, 0.6 * k, ink);
        if i % 10 == 0 && i > 0 && r.width() > 0.0 {
            let n = i / 10;
            let mut buf = [0u8; 4];
            let w = format_num(n, &mut buf);
            let t = Text::new(w, Role::Micro, ink).caps(false);
            super::super::canvas::put(
                b,
                pos2(x + 2.0 * k, r.top() + 12.0 * k),
                Align2::LEFT_CENTER,
                &t,
                k * 0.8,
                None,
            );
        }
        x += 0.5 * step;
        i += 1;
    }
    // Blade scuffs: a few fine light strokes at shallow angles.
    for n in 0..9u32 {
        let p = pos2(
            r.left() + hash(n * 3 + 1) * r.width(),
            r.top() + hash(n * 3 + 2) * r.height(),
        );
        let a = (hash(n * 3 + 3) - 0.5) * 0.6;
        let d = vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a))
            * (30.0 + 60.0 * hash(n + 40))
            * k;
        b.segment(p, p + d, 0.6 * k, with_alpha(c(m.bold), 0.18), false);
    }
}

/// A small number's digits into `buf`, with no allocation.
fn format_num(n: usize, buf: &mut [u8; 4]) -> &str {
    let mut v = n.min(9_999);
    let mut i = 4;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("")
}

/// The instrument's housing over `r` with a soft shadow on the mat: a milled body, a
/// chamfer lit along the top and left, the face's fine machining lines, a cap screw in
/// each corner. Returns the face inside the chamfer.
pub fn housing(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = finish(s);
    // The shadow it casts on the mat, and its feet's darker ones.
    b.rrect_soft(
        r.translate(vec2(4.0 * k, 9.0 * k)),
        12.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.55)),
        14.0 * k,
    );
    for x in [r.left() + 40.0 * k, r.right() - 40.0 * k] {
        b.rrect_soft(
            Rect::from_center_size(pos2(x, r.bottom() + 3.0 * k), vec2(46.0 * k, 8.0 * k)),
            4.0 * k,
            Fill::Solid(with_alpha(shadow(), 0.7)),
            4.0 * k,
        );
    }
    b.rrect(r, 12.0 * k, Fill::V(c(f.edge), c(f.foot)));
    let face = r.shrink(3.0 * k);
    b.rrect(face, 10.0 * k, Fill::V(c(f.top), c(f.face)));
    // The milled face: fine lines a mill leaves, faint.
    let mut y = face.top() + 6.0 * k;
    let mut n = 0u32;
    while y < face.bottom() - 6.0 * k {
        let a = 0.05 + 0.06 * hash(n);
        b.hline(
            face.left() + 8.0 * k,
            face.right() - 8.0 * k,
            y,
            0.5 * k,
            with_alpha(c(f.grain), a),
        );
        y += 2.5 * k;
        n += 1;
    }
    // The foot's shade and a sheen across the upper third.
    b.rrect(
        Rect::from_min_max(pos2(face.left(), face.bottom() - 18.0 * k), face.max),
        10.0 * k,
        Fill::V(with_alpha(c(f.foot), 0.0), with_alpha(c(f.foot), 0.35)),
    );
    b.rrect_stroke(face, 10.0 * k, 1.0 * k, with_alpha(c(f.edge), 0.75));
    for p in [
        face.left_top() + vec2(14.0, 14.0) * k,
        face.right_top() + vec2(-14.0, 14.0) * k,
        face.left_bottom() + vec2(14.0, -14.0) * k,
        face.right_bottom() + vec2(-14.0, -14.0) * k,
    ] {
        cap_screw(b, s, p, 5.0 * k);
    }
    face
}

/// A hex-socket cap screw at `p`, sunk in its counterbore.
pub fn cap_screw(b: &mut Brush, s: &Style, p: Pos2, rad: f32) {
    let f = finish(s);
    b.circle(p, rad * 1.35, Fill::V(c(f.foot), c(f.edge)));
    b.circle(p, rad, Fill::V(c(pal::STEEL_LIGHT), c(pal::STEEL_DARK)));
    let hex: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_3 + 0.3;
            p + vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad * 0.48
        })
        .collect();
    b.polygon(&hex, c(pal::STEEL_DARK));
    b.circle_stroke(p, rad, 0.6 * s.k, with_alpha(c(pal::GLINT), 0.5));
}

/// A screen's bezel over `r`: a machined recess (dark inner wall, a lit lower lip), then
/// the glass. Returns the glass.
pub fn bezel(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = finish(s);
    b.rrect(r, 8.0 * k, Fill::V(c(f.foot), c(f.edge)));
    let wall = r.shrink(3.0 * k);
    b.rrect(
        wall,
        6.0 * k,
        Fill::V(mix(c(f.foot), shadow(), 0.6), c(f.foot)),
    );
    let glass = wall.shrink(4.0 * k);
    b.rrect(
        glass,
        4.0 * k,
        Fill::V(c(pal::GLASS_TOP), c(pal::GLASS_FOOT)),
    );
    glass
}

/// The glass's sheen over `r`: a faint diagonal band, drawn last.
pub fn sheen(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let w = r.width() * 0.18;
    let x0 = r.left() + r.width() * 0.62;
    let top = [pos2(x0, r.top()), pos2(x0 + w, r.top())];
    let bottom = [
        pos2(x0 - r.height() * 0.35, r.bottom()),
        pos2(x0 + w - r.height() * 0.35, r.bottom()),
    ];
    b.polygon(
        &[top[0], top[1], bottom[1], bottom[0]],
        with_alpha(c(pal::SHEEN), 0.035),
    );
    b.rrect_stroke(r, 4.0 * k, 0.8 * k, with_alpha(c(pal::SHEEN), 0.10));
}

/// Words printed on the housing at `at`; returns where they went.
pub fn print(b: &mut Brush, s: &Style, at: Pos2, align: Align2, t: &Text<'_>, scale: f32) -> Rect {
    let f = finish(s);
    let t = Text {
        color: c(f.print),
        ..*t
    };
    super::super::canvas::put(b, at, align, &t, s.k * scale, None)
}

/// Words engraved in the housing (the cut and its lit lower lip).
pub fn engrave(
    b: &mut Brush,
    s: &Style,
    at: Pos2,
    align: Align2,
    t: &Text<'_>,
    scale: f32,
) -> Rect {
    let f = finish(s);
    let lip = Text {
        color: with_alpha(c(f.edge), 0.7),
        ..*t
    };
    super::super::canvas::put(b, at + vec2(0.0, 0.8 * s.k), align, &lip, s.k * scale, None);
    let t = Text {
        color: c(f.engrave),
        ..*t
    };
    super::super::canvas::put(b, at, align, &t, s.k * scale, None)
}

/// The maker's plate at `r`: a raised machined plate, two cap screws, the name engraved
/// big and the model printed beside it.
pub fn plate(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let f = finish(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 2.0 * k)),
        5.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.4)),
        4.0 * k,
    );
    b.rrect(r, 5.0 * k, Fill::V(c(f.edge), c(f.face)));
    b.rrect_stroke(r, 5.0 * k, 0.8 * k, with_alpha(c(f.foot), 0.8));
    for x in [r.left() + 10.0 * k, r.right() - 10.0 * k] {
        cap_screw(b, s, pos2(x, r.center().y), 3.2 * k);
    }
    let name = Text::key("picker.dev.purus.name", Role::Title, Color32::TRANSPARENT).caps(true);
    engrave(
        b,
        s,
        pos2(r.left() + 22.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &name,
        1.2,
    );
}
