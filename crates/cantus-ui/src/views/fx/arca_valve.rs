//! Arca's valves and chassis parts, drawn (ADR 0134; the owner's v1 verdict:
//! "the valves' insides alive, the chassis a real chassis"): a valve whose **heater** glows
//! as soon as the amp is on and whose **plate** reddens with the work it does, its getter's
//! silver flash in the dome, micas and rods, the pins in its base, standing in a socket with
//! its shield skirt; the transformers (laminations, bells, windings), the filter capacitors'
//! cans and the back plate. Vectors at the style's scale, colours from
//! `kit::skin::cabinets` and `kit::skin::pedals`; fine strokes.

use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::cabinets as cab;
use crate::kit::skin::pedals as pal;
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Color32, Pos2, Rect};

fn c(h: crate::kit::color::Hsl) -> Color32 {
    h.rgb()
}

/// A filled ellipse (solid: a gradient to nothing darkens the screen in the CPU renderer).
pub fn ellipse(b: &mut Brush, ctr: Pos2, rx: f32, ry: f32, col: Color32) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let clip = Rect::from_center_size(ctr, vec2(rx * 2.2, ry * 2.2));
    b.radial(clip, ctr, vec2(rx, ry), &[(0.0, col), (1.0, col)], 16);
}

/// A soft glow: nested faint ellipses of `col`, `amount` 0 … 1, inside `clip`.
pub fn glow(b: &mut Brush, clip: Rect, ctr: Pos2, rx: f32, ry: f32, col: Color32, amount: f32) {
    let a = amount.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let n = 24;
    for i in 0..n {
        let t = 1.0 - i as f32 / n as f32;
        let cl = with_alpha(col, 0.02 * a);
        b.radial(clip, ctr, vec2(rx * t, ry * t), &[(0.0, cl), (1.0, cl)], 16);
    }
}

/// A valve standing in `r` (its pins at the bottom): `heater` and `plate` 0 … 1. `power`:
/// a power valve (taller plate, an octal base of brown bakelite); else a small noval valve
/// with its socket's shield skirt.
pub fn valve(b: &mut Brush, k: f32, r: Rect, heater: f32, plate: f32, power: bool) {
    let w = r.width();
    let (h, p) = (heater.clamp(0.0, 1.0), plate.clamp(0.0, 1.0));
    let warm = c(pal::FILAMENT);
    let red = mix(warm, c(cab::COPPER), 0.55);
    let base_h = if power { w * 0.40 } else { w * 0.22 };
    let glass = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - base_h * 0.75));
    let rad = w * 0.5;
    // The light it throws, behind everything.
    glow(
        b,
        glass.expand(w * 1.6),
        pos2(glass.center().x, glass.top() + glass.height() * 0.55),
        w * 1.7,
        glass.height() * 0.75,
        warm,
        0.35 * h + 0.65 * p,
    );
    // The base: octal bakelite with its key, or a noval's glass foot in its shield skirt.
    if power {
        let base = Rect::from_min_max(
            pos2(r.left() - w * 0.04, r.bottom() - base_h),
            pos2(r.right() + w * 0.04, r.bottom()),
        );
        b.rrect(
            base,
            w * 0.12,
            Fill::V(c(pal::BAKELITE_BROWN_TOP), c(pal::BAKELITE_BROWN)),
        );
        b.hline(
            base.left() + w * 0.1,
            base.right() - w * 0.1,
            base.top() + base_h * 0.3,
            0.5 * k,
            with_alpha(c(pal::SHADOW), 0.45),
        );
    } else {
        let skirt = Rect::from_min_max(
            pos2(r.left() - w * 0.14, r.bottom() - base_h),
            pos2(r.right() + w * 0.14, r.bottom()),
        );
        b.rrect(
            skirt,
            1.5 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        for f in [0.25f32, 0.5, 0.75] {
            b.vline(
                skirt.left() + skirt.width() * f,
                skirt.top() + 1.0 * k,
                skirt.bottom() - 1.0 * k,
                0.5 * k,
                with_alpha(c(pal::CHROME_DARK), 0.7),
            );
        }
    }
    // The glass: faintly tinted, lit edges.
    b.rrect(
        glass,
        rad,
        Fill::V(
            with_alpha(c(pal::GLASS), 0.10),
            with_alpha(c(pal::GLASS), 0.18),
        ),
    );
    // Inside: the micas, the rods, the plate (reddening with work), the heater in its gaps.
    let plate_h = glass.height() * if power { 0.44 } else { 0.38 };
    let core = pos2(glass.center().x, glass.top() + glass.height() * 0.56);
    let pr = Rect::from_center_size(core, vec2(w * 0.56, plate_h));
    let (m_top, m_bot) = (pr.top() - 4.0 * k, pr.bottom() + 4.0 * k);
    for f in [0.2f32, 0.8] {
        b.vline(
            glass.left() + w * f,
            m_top - 2.0 * k,
            m_bot + 2.0 * k,
            0.6 * k,
            with_alpha(c(pal::GETTER), 0.8),
        );
    }
    for y in [m_top, m_bot] {
        b.rect(
            Rect::from_center_size(pos2(core.x, y), vec2(w * 0.74, 1.6 * k)),
            Fill::Solid(with_alpha(c(pal::MICA), 0.75)),
        );
    }
    b.rect(pr, Fill::V(c(pal::PLATE), c(pal::PLATE_SEAM)));
    if p > 0.0 {
        b.rect(pr, Fill::Solid(with_alpha(red, 0.55 * p)));
    }
    b.vline(core.x, pr.top(), pr.bottom(), 0.6 * k, c(pal::PLATE_SEAM));
    for x in [pr.left() + 1.0 * k, pr.right() - 1.0 * k] {
        b.vline(
            x,
            pr.top(),
            pr.bottom(),
            0.7 * k,
            with_alpha(c(pal::GETTER), 0.5),
        );
    }
    // The heater through the slit and the gaps: first light, the brightest point.
    let hot = mix(warm, c(pal::CHROME_LIGHT), 0.5);
    if h > 0.0 {
        b.vline(
            core.x,
            pr.top() + 2.0 * k,
            pr.bottom() - 2.0 * k,
            0.9 * k,
            with_alpha(warm, 0.9 * h),
        );
        for y in [pr.top() - 2.0 * k, pr.bottom() + 2.0 * k] {
            ellipse(
                b,
                pos2(core.x, y),
                w * 0.16,
                1.6 * k,
                with_alpha(hot, 0.85 * h),
            );
            glow(b, glass, pos2(core.x, y), w * 0.4, w * 0.25, warm, h);
        }
    }
    // The getter's flash: silver in the dome, a dark mirror with its bright point.
    let dome = pos2(glass.center().x, glass.top() + rad * 0.62);
    ellipse(
        b,
        dome,
        rad * 0.78,
        rad * 0.42,
        with_alpha(c(pal::GETTER_DARK), 0.85),
    );
    ellipse(
        b,
        dome - vec2(0.0, rad * 0.08),
        rad * 0.62,
        rad * 0.26,
        with_alpha(c(pal::GETTER), 0.55),
    );
    b.circle(
        dome - vec2(rad * 0.3, rad * 0.12),
        1.1 * k,
        Fill::Solid(c(pal::CHROME_LIGHT)),
    );
    // The glass's own light: a fine highlight down its left, its outline.
    b.vline(
        glass.left() + w * 0.16,
        glass.top() + rad,
        glass.bottom() - 3.0 * k,
        0.9 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.55),
    );
    b.rrect_stroke(glass, rad, 0.7 * k, with_alpha(c(pal::GLASS), 0.55));
    // The pins under the base.
    let n = if power { 4 } else { 5 };
    for i in 0..n {
        let x = r.left() + w * (0.2 + 0.6 * i as f32 / (n - 1) as f32);
        b.vline(
            x,
            r.bottom(),
            r.bottom() + 2.5 * k,
            0.6 * k,
            c(pal::CHROME_DARK),
        );
    }
}

/// A transformer in `r`: its laminated core seen from the side (fine lines), the bells over
/// its ends, the copper of its winding between them, its mounting feet.
pub fn transformer(b: &mut Brush, k: f32, r: Rect, bells: bool) {
    let core = r.shrink2(vec2(r.width() * 0.08, r.height() * 0.06));
    b.rect(core, Fill::V(c(cab::CASTING), c(cab::CHASSIS)));
    let mut y = core.top() + 1.2 * k;
    while y < core.bottom() {
        b.hline(
            core.left(),
            core.right(),
            y,
            0.4 * k,
            with_alpha(c(cab::CHASSIS_EDGE), 0.6),
        );
        y += 1.6 * k;
    }
    let wind = Rect::from_center_size(
        core.center(),
        vec2(core.width() * 0.46, core.height() * 0.78),
    );
    b.rrect(wind, 2.0 * k, Fill::V(c(cab::COPPER_LIGHT), c(cab::COPPER)));
    let mut x = wind.left() + 1.4 * k;
    while x < wind.right() - 1.0 * k {
        b.vline(
            x,
            wind.top() + 1.0 * k,
            wind.bottom() - 1.0 * k,
            0.4 * k,
            with_alpha(c(cab::COPPER), 0.9),
        );
        x += 1.8 * k;
    }
    if bells {
        for side in [core.left(), core.right()] {
            let bell = Rect::from_center_size(
                pos2(side, core.center().y),
                vec2(core.width() * 0.34, core.height() * 1.04),
            );
            b.rrect(
                bell,
                3.0 * k,
                Fill::V(c(cab::COVERING_TOP), c(cab::COVERING)),
            );
            b.rrect_stroke(
                bell,
                3.0 * k,
                0.6 * k,
                with_alpha(c(cab::CHASSIS_EDGE), 0.9),
            );
            b.circle(
                bell.center() - vec2(0.0, bell.height() * 0.3),
                1.2 * k,
                Fill::Solid(c(pal::CHROME_LIGHT)),
            );
        }
    }
    for x in [r.left() + 2.0 * k, r.right() - 2.0 * k] {
        b.rect(
            Rect::from_center_size(pos2(x, r.bottom() - 1.5 * k), vec2(6.0 * k, 3.0 * k)),
            Fill::Solid(c(cab::CHASSIS_EDGE)),
        );
    }
}

/// A filter capacitor's aluminium can in `r`: its rolled top, its sleeve, its clamp.
pub fn capacitor(b: &mut Brush, k: f32, r: Rect) {
    b.rrect(
        r,
        r.width() * 0.18,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    let sleeve = Rect::from_min_max(pos2(r.left(), r.top() + r.height() * 0.22), r.max);
    b.rrect(
        sleeve,
        r.width() * 0.12,
        Fill::V(c(cab::CASTING), c(cab::CHASSIS)),
    );
    b.hline(
        r.left(),
        r.right(),
        r.top() + r.height() * 0.08,
        0.6 * k,
        c(pal::CHROME_DARK),
    );
    let clamp = Rect::from_min_max(
        pos2(r.left() - 1.0 * k, r.bottom() - r.height() * 0.22),
        pos2(r.right() + 1.0 * k, r.bottom() - r.height() * 0.14),
    );
    b.rect(clamp, Fill::Solid(c(pal::CHROME_DARK)));
    b.vline(
        r.left() + r.width() * 0.25,
        sleeve.top() + 2.0 * k,
        r.bottom() - 2.0 * k,
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.35),
    );
}

/// The chassis's back plate in `r`: dark steel, rows of vent slots, screws, a fuse cap and
/// two speaker jacks — fine detail behind the valves.
pub fn back_plate(b: &mut Brush, k: f32, r: Rect) {
    b.rect(r, Fill::V(c(cab::CHASSIS_TOP), c(cab::CHASSIS)));
    b.hline(
        r.left(),
        r.right(),
        r.top() + 0.5 * k,
        0.6 * k,
        with_alpha(c(cab::CHASSIS_EDGE), 0.9),
    );
    b.hline(
        r.left(),
        r.right(),
        r.bottom() - 0.5 * k,
        0.6 * k,
        with_alpha(c(pal::SHADOW), 0.8),
    );
    // Vent slots, two staggered rows across the middle.
    let (y0, y1) = (r.top() + r.height() * 0.32, r.top() + r.height() * 0.62);
    let mut x = r.left() + r.width() * 0.22;
    let mut i = 0;
    while x < r.right() - r.width() * 0.22 {
        let y = if i % 2 == 0 { y0 } else { y1 };
        b.rrect(
            Rect::from_center_size(pos2(x, y), vec2(9.0 * k, 2.2 * k)),
            1.1 * k,
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.85)),
        );
        x += 7.0 * k;
        i += 1;
    }
    // Screws at the ends, a fuse cap and two jacks at the right.
    for x in [r.left() + 8.0 * k, r.right() - 8.0 * k] {
        let p = pos2(x, r.center().y);
        b.circle(
            p,
            2.4 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.segment(
            p - vec2(1.5, 1.5) * k,
            p + vec2(1.5, 1.5) * k,
            0.5 * k,
            c(pal::CHROME_DARK),
            false,
        );
    }
    let fx = r.left() + r.width() * 0.12;
    b.circle(
        pos2(fx, r.center().y),
        4.0 * k,
        Fill::V(c(pal::BAKELITE_TOP), c(pal::BAKELITE)),
    );
    b.circle_stroke(
        pos2(fx, r.center().y),
        4.0 * k,
        0.5 * k,
        c(pal::CHROME_DARK),
    );
    for i in 0..2 {
        let p = pos2(
            r.right() - r.width() * 0.10 - i as f32 * 14.0 * k,
            r.center().y,
        );
        b.circle(
            p,
            3.8 * k,
            Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
        );
        b.circle(p, 1.8 * k, Fill::Solid(c(pal::SHADOW)));
    }
}
