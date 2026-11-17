//! Purus's small parts (design sheet v1): the module cartridges slid into the housing (a
//! milled plate with its pull and its screws), the bat-handle toggle each module is
//! switched with, the jewel lamp beside it, the push keys, and the tuning fork that is the
//! de-hum's emblem — polished steel, its stem and tines shaded, the fundamental engraved on
//! its stem, its tines ringing with the hum it hears. Original to Cantus.

use super::art::{c, finish, needle, shadow};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::restoration as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// A module's cartridge over `r`: a raised plate in the housing's finish, a recess line
/// round it (it slides out), two screws and a pull at its left edge. Returns its face.
pub fn cartridge(b: &mut Brush, s: &Style, r: Rect, on: bool) -> Rect {
    let k = s.k;
    let f = finish(s);
    // The slot it sits in: a dark line round it.
    b.rrect(
        r.expand(2.0 * k),
        6.0 * k,
        Fill::Solid(mix(c(f.foot), shadow(), 0.5)),
    );
    b.rrect(r, 5.0 * k, Fill::V(c(f.edge), c(f.foot)));
    let face = r.shrink(1.5 * k);
    b.rrect(face, 4.5 * k, Fill::V(c(f.top), c(f.face)));
    let mut y = face.top() + 4.0 * k;
    let mut n = 0u32;
    while y < face.bottom() - 3.0 * k {
        b.hline(
            face.left() + 18.0 * k,
            face.right() - 6.0 * k,
            y,
            0.45 * k,
            with_alpha(c(f.grain), 0.05 + 0.04 * super::art::hash(n + 300)),
        );
        y += 2.2 * k;
        n += 1;
    }
    // The pull: a vertical slot at its left edge, for a fingertip.
    let pull = Rect::from_min_max(
        pos2(face.left() + 5.0 * k, face.top() + 10.0 * k),
        pos2(face.left() + 10.0 * k, face.bottom() - 10.0 * k),
    );
    b.rrect(
        pull,
        2.5 * k,
        Fill::V(mix(c(f.foot), shadow(), 0.55), c(f.face)),
    );
    b.vline(
        pull.right(),
        pull.top() + 2.0 * k,
        pull.bottom() - 2.0 * k,
        0.6 * k,
        with_alpha(c(f.edge), 0.6),
    );
    // An off module's face is a little dimmer: the light is on the ones working.
    if !on {
        b.rrect(face, 4.5 * k, Fill::Solid(with_alpha(shadow(), 0.10)));
    }
    face
}

/// A bat-handle toggle at `p` (its bushing's centre), up when `on`: a knurled nut, the
/// polished bat with its lit edge, its shadow on the plate.
pub fn toggle(b: &mut Brush, s: &Style, p: Pos2, on: bool, hot: bool) {
    let k = s.k;
    let f = finish(s);
    b.circle(
        p + vec2(0.0, 1.5 * k),
        9.5 * k,
        Fill::Solid(with_alpha(shadow(), 0.35)),
    );
    b.circle(p, 9.0 * k, Fill::V(c(pal::STEEL_LIGHT), c(pal::STEEL_DARK)));
    for i in 0..18 {
        let a = i as f32 / 18.0 * std::f32::consts::TAU;
        let d = vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a));
        b.segment(
            p + d * 7.4 * k,
            p + d * 9.0 * k,
            0.6 * k,
            with_alpha(c(pal::STEEL_DARK), 0.7),
            false,
        );
    }
    b.circle(p, 6.0 * k, Fill::V(c(pal::STEEL), c(pal::STEEL_DARK)));
    let dir = if on { -1.0 } else { 1.0 };
    let tip = p + vec2(0.0, dir * 17.0 * k);
    // The bat's shadow, then the bat: a tapered handle with a rounded end.
    b.segment(
        p + vec2(2.0 * k, 2.0 * k),
        tip + vec2(3.0 * k, 4.0 * k),
        6.0 * k,
        with_alpha(shadow(), 0.35),
        true,
    );
    b.segment(p, tip, 4.0 * k, c(pal::STEEL_DARK), true);
    b.segment(p, tip, 3.0 * k, c(pal::STEEL), true);
    b.segment(
        p + vec2(-0.8 * k, 0.0),
        tip + vec2(-0.8 * k, 0.0),
        0.9 * k,
        c(pal::STEEL_LIGHT),
        true,
    );
    b.circle(
        tip,
        3.2 * k,
        Fill::V(c(pal::STEEL_LIGHT), c(pal::STEEL_DARK)),
    );
    b.dot(tip + vec2(-1.0, -1.0) * k, 0.9 * k, c(pal::GLINT));
    if hot {
        b.circle_stroke(p, 11.0 * k, 0.8 * k, with_alpha(c(f.edge), 0.8));
    }
}

/// A jewel lamp at `p`, lit by `lit` (0 … 1) in `color`: a faceted lens in a bezel.
pub fn lamp(b: &mut Brush, s: &Style, p: Pos2, rad: f32, lit: f32, color: Color32) {
    let k = s.k;
    b.circle(
        p,
        rad + 1.6 * k,
        Fill::V(c(pal::STEEL_LIGHT), c(pal::STEEL_DARK)),
    );
    let off = c(pal::LAMP_OFF);
    let body = mix(off, color, lit.clamp(0.0, 1.0));
    b.circle(p, rad, Fill::Solid(body));
    if lit > 0.05 {
        b.radial(
            Rect::from_center_size(p, vec2(rad, rad) * 6.0),
            p,
            vec2(rad, rad) * 3.0,
            &[
                (0.0, with_alpha(color, 0.30 * lit)),
                (1.0, with_alpha(color, 0.0)),
            ],
            8,
        );
        b.circle(
            p,
            rad * 0.45,
            Fill::Solid(mix(color, c(pal::LAMP_HOT), 0.6 * lit)),
        );
    }
    // The facets: three fine lines and a glint.
    for i in 0..3 {
        let a = i as f32 * 2.1 + 0.4;
        let d = vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad * 0.85;
        b.segment(
            p - d * 0.2,
            p + d,
            0.4 * k,
            with_alpha(c(pal::GLINT), 0.25),
            false,
        );
    }
    b.dot(
        p + vec2(-0.35, -0.4) * rad,
        rad * 0.22,
        with_alpha(c(pal::GLINT), 0.85),
    );
}

/// A push key over `r` labelled `label`, lit (pressed) when `lit`: a machined cap on its
/// guide, its legend printed, a light strip along its top when pressed.
pub fn key(b: &mut Brush, s: &Style, r: Rect, label: &str, lit: bool, hot: bool, color: Color32) {
    let k = s.k;
    let f = finish(s);
    b.rrect(
        r.translate(vec2(0.0, 1.5 * k)),
        4.0 * k,
        Fill::Solid(with_alpha(shadow(), 0.45)),
    );
    let cap = if lit {
        r.translate(vec2(0.0, 1.0 * k))
    } else {
        r
    };
    b.rrect(
        cap,
        4.0 * k,
        Fill::V(c(pal::STEEL_DARK), mix(c(pal::STEEL_DARK), shadow(), 0.5)),
    );
    let top = cap.shrink(1.2 * k);
    b.rrect(
        top,
        3.0 * k,
        Fill::V(
            mix(c(pal::STEEL_DARK), c(pal::STEEL), 0.35),
            c(pal::STEEL_DARK),
        ),
    );
    let strip = Rect::from_min_max(
        pos2(top.left() + 5.0 * k, top.top() + 2.5 * k),
        pos2(top.right() - 5.0 * k, top.top() + 4.5 * k),
    );
    b.rrect(
        strip,
        1.0 * k,
        Fill::Solid(if lit { color } else { c(pal::LAMP_OFF) }),
    );
    if lit {
        b.rrect_soft(
            strip,
            1.0 * k,
            Fill::Solid(with_alpha(color, 0.45)),
            3.0 * k,
        );
    }
    let ink = if lit {
        c(pal::LAMP_HOT)
    } else {
        c(pal::STEEL_LIGHT)
    };
    let t = Text::new(label, Role::Micro, ink).caps(true);
    super::super::canvas::put(
        b,
        pos2(top.center().x, top.center().y + 2.5 * k),
        Align2::CENTER_CENTER,
        &t,
        k,
        None,
    );
    if hot {
        b.rrect_stroke(
            r.expand(1.5 * k),
            5.0 * k,
            0.8 * k,
            with_alpha(c(f.edge), 0.9),
        );
    }
}

/// The tuning fork lying in its fitted recess over `r` (stem to the left, tines to the
/// right): polished steel — a ball-ended stem, a U-shaped yoke, two long tines — the
/// fundamental engraved beside it (`words`), its tines ringing with the hum heard (`ring`,
/// 0 … 1: fine arcs beyond their tips).
pub fn fork(b: &mut Brush, s: &Style, r: Rect, words: &str, ring: f32, t_now: f32) {
    let k = s.k;
    let f = finish(s);
    let cy = r.center().y - 4.0 * k;
    let w = (r.height() * 0.12).clamp(4.0 * k, 7.0 * k);
    let gap = (r.height() * 0.22).clamp(8.0 * k, 13.0 * k);
    let x0 = r.left() + 8.0 * k;
    let yoke_x = r.left() + r.width() * 0.40;
    let tine_end = r.right() - 26.0 * k;
    let recess = mix(c(f.foot), shadow(), 0.55);
    let lip = with_alpha(c(f.edge), 0.7);
    // The shape twice: the fitted recess (wider, dark, its lower lip lit), then the steel.
    let shape = |b: &mut Brush, grow: f32, dy: f32, col: Color32| {
        b.segment(
            pos2(x0, cy + dy),
            pos2(yoke_x - gap, cy + dy),
            w + grow,
            col,
            true,
        );
        b.circle(pos2(x0, cy + dy), w * 0.9 + grow * 0.5, Fill::Solid(col));
        b.arc(
            pos2(yoke_x, cy + dy),
            gap,
            std::f32::consts::FRAC_PI_2,
            1.5 * std::f32::consts::PI,
            w + grow,
            col,
            false,
        );
        for side in [-1.0f32, 1.0] {
            b.segment(
                pos2(yoke_x, cy + dy + side * gap),
                pos2(tine_end, cy + dy + side * gap),
                w + grow,
                col,
                true,
            );
        }
    };
    shape(b, 4.0 * k, 1.2 * k, lip);
    shape(b, 4.0 * k, 0.0, recess);
    shape(b, 0.0, 2.0 * k, with_alpha(shadow(), 0.5));
    shape(b, 0.0, 0.0, c(pal::STEEL_DARK));
    shape(b, -w * 0.35, -w * 0.12, c(pal::STEEL));
    // The highlights along the top of each bar.
    let hl = c(pal::STEEL_LIGHT);
    b.segment(
        pos2(x0 + 2.0 * k, cy - w * 0.3),
        pos2(yoke_x - gap, cy - w * 0.3),
        w * 0.18,
        hl,
        true,
    );
    for side in [-1.0f32, 1.0] {
        let y = cy + side * gap - w * 0.3;
        b.segment(
            pos2(yoke_x + 2.0 * k, y),
            pos2(tine_end - 2.0 * k, y),
            w * 0.18,
            hl,
            true,
        );
    }
    b.dot(pos2(x0 - w * 0.25, cy - w * 0.3), w * 0.22, c(pal::GLINT));
    // Ringing: fine arcs beyond each tip, as many as the hum is loud.
    for side in [-1.0f32, 1.0] {
        let tip = pos2(tine_end + w * 0.6, cy + side * gap);
        let arcs = (ring * 3.0).ceil() as usize;
        for a in 0..arcs {
            let rad = (5.0 + a as f32 * 4.0 + 2.0 * (t_now * 6.0).fract()) * k;
            let alpha = ring.clamp(0.0, 1.0) * (0.8 - a as f32 * 0.22);
            b.arc(
                tip,
                rad,
                -0.7,
                0.7,
                0.9 * k,
                with_alpha(needle(s), alpha),
                true,
            );
        }
    }
    // The fundamental engraved under the stem.
    let t = Text::new(words, Role::Label, Color32::TRANSPARENT).caps(false);
    let at = pos2(x0 + 2.0 * k, cy + gap + w + 10.0 * k);
    super::art::engrave(b, s, at, Align2::LEFT_CENTER, &t, 1.0);
}
