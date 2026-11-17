//! Loquax's objects (design sheet v2, the owner's "more elaborate"): the vocoder's
//! gooseneck microphone rising from its chrome socket (the ribbed neck, the satin head, the
//! woven grille over the foam, the on-air lamp lit by the voice), the machined bezels its
//! screen and meters sit in (a finish per colourway, screws, a road case's corners), the
//! meter bridge's needles, the pilot lamps. Vectors at the style's scale, colours from
//! `kit::skin::consoles`; nothing animates by itself — the display hands each part its
//! level. Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::consoles as pal;
use crate::kit::skin::Material;
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::{FRAC_PI_2, PI};

pub(super) fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i` (the grain's lines, a screw's turn): the same every frame.
pub(super) fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The bezels' finish for the face's colourway: brushed aluminium, black lacquer, steel.
pub fn finish(s: &Style) -> &'static pal::Finish {
    match s.skin.material {
        Material::Lacquer => &pal::LACQUER,
        Material::FineTolex | Material::Tolex => &pal::STEEL,
        _ => &pal::ALUMINIUM,
    }
}

/// A countersunk cross-head screw at `p`, turned `turn` (0 … 1).
pub fn screw(b: &mut Brush, p: Pos2, rad: f32, turn: f32) {
    b.circle(
        p + vec2(0.0, rad * 0.25),
        rad * 1.15,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(p, rad, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)));
    b.circle(p, rad * 0.78, Fill::V(c(pal::CHROME), c(pal::CHROME_LIGHT)));
    let a = turn * PI;
    for da in [0.0, FRAC_PI_2] {
        let d = Vec2::angled(a + da) * rad * 0.62;
        b.segment(
            p - d,
            p + d,
            rad * 0.26,
            with_alpha(c(pal::SHADOW), 0.75),
            false,
        );
        b.segment(
            p - d + vec2(0.0, rad * 0.12),
            p + d + vec2(0.0, rad * 0.12),
            rad * 0.1,
            with_alpha(c(pal::CHROME_LIGHT), 0.35),
            false,
        );
    }
}

/// A rivet (the road case's corners): a plain dome.
pub(super) fn rivet(b: &mut Brush, p: Pos2, rad: f32) {
    b.circle(p, rad, Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)));
    b.circle(
        p - vec2(rad, rad) * 0.3,
        rad * 0.3,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.8)),
    );
}

/// A machined bezel round `r`, `w` wide, in the colourway's finish, a screw (or, on the
/// road case, a steel corner and its rivet) at each corner; the glass's rect inside it.
pub fn bezel(b: &mut Brush, s: &Style, r: Rect, w: f32) -> Rect {
    let k = s.k;
    let f = finish(s);
    // Its shadow on the deck, then the frame: lit along the top, shadowed at the foot.
    b.rrect_soft(
        r.translate(vec2(0.0, 2.5 * k)),
        5.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
        7.0 * k,
    );
    b.rrect(
        r,
        4.0 * k,
        Fill::V(mix(c(f.face), c(f.top), 0.4), c(f.foot)),
    );
    let inner = r.shrink(w);
    // The finish's grain on the four rails only (the glass covers the rest).
    let step = 1.3 * k;
    let mut i = 0u32;
    let mut y = r.top() + 1.0 * k;
    while y < r.bottom() - 1.0 * k {
        let a = 0.04 + 0.16 * hash(i);
        let col = with_alpha(c(f.grain), a);
        if y < inner.top() || y > inner.bottom() {
            b.hline(r.left() + 2.0 * k, r.right() - 2.0 * k, y, 0.5 * k, col);
        } else {
            b.hline(r.left() + 1.0 * k, inner.left(), y, 0.5 * k, col);
            b.hline(inner.right(), r.right() - 1.0 * k, y, 0.5 * k, col);
        }
        y += step;
        i += 1;
    }
    // The light catching the outer edge and the top's chamfer; on lacquer a deep shine.
    b.hline(
        r.left() + 3.0 * k,
        r.right() - 3.0 * k,
        r.top() + 0.6 * k,
        1.0 * k,
        c(f.top),
    );
    b.rrect_stroke(r, 4.0 * k, 0.6 * k, with_alpha(c(f.top), 0.45));
    if s.skin.material == Material::Lacquer {
        let t = Rect::from_min_max(
            pos2(r.left() + 3.0 * k, r.top() + 1.5 * k),
            pos2(r.right() - 3.0 * k, inner.top() - 1.0 * k),
        );
        b.rect(
            t,
            Fill::V(with_alpha(c(f.grain), 0.22), with_alpha(c(f.grain), 0.0)),
        );
    }
    // The chamfer down to the glass: shadowed at the top, lit at the bottom.
    b.rrect(inner.expand(1.6 * k), 2.5 * k, Fill::V(c(f.foot), c(f.top)));
    if s.skin.material == Material::Lacquer {
        // Lacquer's chrome lips: round the glass and round the frame.
        b.rrect_stroke(
            inner.expand(2.2 * k),
            3.0 * k,
            0.9 * k,
            with_alpha(c(pal::CHROME), 0.75),
        );
        b.rrect_stroke(
            r.shrink(0.8 * k),
            3.5 * k,
            0.8 * k,
            with_alpha(c(pal::CHROME), 0.55),
        );
    }
    b.rrect(inner, 2.0 * k, Fill::Solid(c(pal::GASKET)));
    let corners = [
        pos2(r.left() + w * 0.5, r.top() + w * 0.5),
        pos2(r.right() - w * 0.5, r.top() + w * 0.5),
        pos2(r.left() + w * 0.5, r.bottom() - w * 0.5),
        pos2(r.right() - w * 0.5, r.bottom() - w * 0.5),
    ];
    let steel = matches!(s.skin.material, Material::FineTolex | Material::Tolex);
    for (j, p) in corners.into_iter().enumerate() {
        if steel {
            // A road case's corner: a steel cap over the frame's corner, riveted.
            let sz = w * 1.9;
            let cap = Rect::from_center_size(
                p + vec2(
                    if j % 2 == 0 { -1.0 } else { 1.0 },
                    if j < 2 { -1.0 } else { 1.0 },
                ) * w
                    * 0.2,
                vec2(sz, sz),
            );
            b.rrect(cap, 2.5 * k, Fill::V(c(pal::CHROME), c(pal::CHROME_DARK)));
            b.rrect_stroke(cap, 2.5 * k, 0.6 * k, with_alpha(c(pal::CHROME_LIGHT), 0.6));
            rivet(b, p, w * 0.24);
        } else {
            screw(b, p, w * 0.3, hash(j as u32 + 7));
        }
    }
    inner.shrink(1.0 * k)
}

/// Glass over `r`: a broad faint reflection across its upper left and a lit top edge.
pub fn glass(b: &mut Brush, r: Rect, k: f32) {
    let (w, h) = (r.width(), r.height());
    let sheen = [
        pos2(r.left() + w * 0.06, r.top()),
        pos2(r.left() + w * 0.34, r.top()),
        pos2(r.left() + w * 0.16, r.top() + h),
        pos2(r.left() - w * 0.02, r.top() + h),
    ];
    let clip = |p: Pos2| pos2(p.x.clamp(r.left(), r.right()), p.y);
    b.polygon(&sheen.map(clip), with_alpha(c(pal::GLASS), 0.03));
    let thin = [
        pos2(r.left() + w * 0.38, r.top()),
        pos2(r.left() + w * 0.42, r.top()),
        pos2(r.left() + w * 0.27, r.top() + h),
        pos2(r.left() + w * 0.23, r.top() + h),
    ];
    b.polygon(&thin.map(clip), with_alpha(c(pal::GLASS), 0.025));
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.top() + 0.5 * k,
        k,
        with_alpha(c(pal::GLASS), 0.16),
    );
}

/// A pilot lamp at `p`: a chrome bezel, a lens of `lens`, its glow (`lit` 0 … 1).
pub fn lamp(b: &mut Brush, p: Pos2, rad: f32, lens: Color32, lit: f32) {
    let l = lit.clamp(0.0, 1.0);
    if l > 0.0 {
        b.radial(
            Rect::from_center_size(p, Vec2::splat(rad * 6.0)),
            p,
            Vec2::splat(rad * 2.8),
            &[
                (0.0, with_alpha(lens, 0.5 * l)),
                (1.0, with_alpha(lens, 0.0)),
            ],
            10,
        );
    }
    b.circle(
        p,
        rad * 1.35,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    b.circle(p, rad * 1.08, Fill::Solid(c(pal::GASKET)));
    let dark = mix(lens, c(pal::SHADOW), 0.72);
    b.circle(
        p,
        rad,
        Fill::V(
            mix(dark, mix(lens, c(pal::CHROME_LIGHT), 0.45), l),
            mix(mix(dark, c(pal::SHADOW), 0.4), lens, l * 0.8),
        ),
    );
    b.circle_stroke(
        p,
        rad * 0.55,
        0.18 * rad,
        with_alpha(c(pal::CHROME_LIGHT), 0.12 + 0.2 * l),
    );
    b.circle(
        p + vec2(-0.35, -0.38) * rad,
        rad * 0.22,
        Fill::Solid(with_alpha(c(pal::CHROME_LIGHT), 0.75)),
    );
}

/// A point of the cubic from `p0` to `p3` at `t`.
pub(super) fn cubic(p: [Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    pos2(
        p.iter().zip(w).map(|(q, w)| q.x * w).sum(),
        p.iter().zip(w).map(|(q, w)| q.y * w).sum(),
    )
}

/// How many points the gooseneck is sampled at.
pub(super) const NECK: usize = 48;

/// The gooseneck microphone standing in `r`: its chrome socket at the foot, the ribbed neck
/// curving up, the satin head with its on-air lamp, the woven grille; `level` (0 … 1) the
/// voice reaching it — the lamp, a glow behind the grille and the sound's arcs in `glow`.
/// Gives the grille's centre.
pub fn microphone(b: &mut Brush, s: &Style, r: Rect, level: f32, glow: Color32) -> Pos2 {
    let k = s.k;
    let lv = level.clamp(0.0, 1.0);
    let hw = (r.width() * 0.4).min(r.height() * 0.17).max(18.0 * k);
    let rr = hw * 0.5;
    let hx = r.left() + r.width() * 0.62;
    let sx = r.left() + r.width() * 0.36;
    // The head, from the top: the grille, its ring, the body, the collar.
    let gy0 = r.top() + 4.0 * k;
    let gh = hw * 1.25;
    let ring_y = gy0 + gh;
    let rh = (hw * 0.14).max(3.5 * k);
    let body_top = ring_y + rh;
    let bh = hw * 0.95;
    let collar_top = body_top + bh;
    let neck_top = collar_top + (hw * 0.18).max(4.0 * k);
    // The socket, at the foot.
    let base = Rect::from_min_max(
        pos2(sx - hw * 0.85, r.bottom() - 8.0 * k),
        pos2(sx + hw * 0.85, r.bottom()),
    );
    let nut = Rect::from_min_max(
        pos2(sx - hw * 0.3, base.top() - (hw * 0.34).max(8.0 * k)),
        pos2(sx + hw * 0.3, base.top() - 3.0 * k),
    );
    neck(
        b,
        k,
        [
            pos2(sx, nut.top() + 1.0 * k),
            pos2(
                sx - r.width() * 0.16,
                nut.top() - (nut.top() - neck_top) * 0.45,
            ),
            pos2(
                hx + r.width() * 0.06,
                neck_top + (nut.top() - neck_top) * 0.5,
            ),
            pos2(hx, neck_top - 1.0 * k),
        ],
        (hw * 0.2).max(5.0 * k),
    );
    socket(b, k, base, nut);
    // The collar where the neck enters, a chrome cylinder.
    let collar = Rect::from_min_max(
        pos2(hx - hw * 0.22, collar_top - 1.0 * k),
        pos2(hx + hw * 0.22, neck_top),
    );
    b.hrect(collar, &cylinder());
    // The body: satin black, narrowing to the collar, lit down its left.
    let body = [
        pos2(hx - hw * 0.5, body_top),
        pos2(hx + hw * 0.5, body_top),
        pos2(hx + hw * 0.24, collar_top),
        pos2(hx - hw * 0.24, collar_top),
    ];
    b.polygon(&body, c(pal::SATIN));
    b.polygon(
        &[
            pos2(hx - hw * 0.36, body_top),
            pos2(hx - hw * 0.2, body_top),
            pos2(hx - hw * 0.09, collar_top),
            pos2(hx - hw * 0.16, collar_top),
        ],
        with_alpha(c(pal::SATIN_LIGHT), 0.45),
    );
    b.segment(
        body[1],
        body[2],
        0.7 * k,
        with_alpha(c(pal::SATIN_LIGHT), 0.6),
        false,
    );
    lamp(
        b,
        pos2(hx, body_top + bh * 0.4),
        (hw * 0.075).max(2.2 * k),
        glow,
        lv,
    );
    // The ring: machined chrome, knurled.
    let ring = Rect::from_min_max(pos2(hx - hw * 0.53, ring_y), pos2(hx + hw * 0.53, body_top));
    b.hrect(ring, &cylinder());
    let mut x = ring.left() + 1.2 * k;
    while x < ring.right() - 1.0 * k {
        b.vline(
            x,
            ring.top() + 0.6 * k,
            ring.bottom() - 0.6 * k,
            0.4 * k,
            with_alpha(c(pal::CHROME_DARK), 0.5),
        );
        x += 1.6 * k;
    }
    b.hline(
        ring.left(),
        ring.right(),
        ring.bottom(),
        0.8 * k,
        with_alpha(c(pal::SHADOW), 0.6),
    );
    grille(b, k, hx, gy0, ring_y, rr, lv, glow);
    let centre = pos2(hx, gy0 + gh * 0.5);
    // The voice reaching the grille: arcs from the left, fainter outward.
    if lv > 0.01 {
        for i in 0..4 {
            let rad = rr * 1.55 + i as f32 * 5.0 * k;
            b.arc(
                centre,
                rad,
                PI - 0.5,
                PI + 0.5,
                0.9 * k,
                with_alpha(glow, lv * (0.75 - 0.17 * i as f32)),
                true,
            );
        }
    }
    centre
}

/// A chrome cylinder's light across it, left to right.
pub(super) fn cylinder() -> [(f32, Color32); 5] {
    [
        (0.0, c(pal::CHROME_DARK)),
        (0.22, c(pal::CHROME_LIGHT)),
        (0.45, c(pal::CHROME)),
        (0.8, mix(c(pal::CHROME), c(pal::CHROME_DARK), 0.6)),
        (1.0, c(pal::CHROME_DARK)),
    ]
}

/// The ribbed gooseneck along the cubic `ctrl`, `nw` thick.
fn neck(b: &mut Brush, k: f32, ctrl: [Pos2; 4], nw: f32) {
    let mut pts = [Pos2::ZERO; NECK + 1];
    for (i, p) in pts.iter_mut().enumerate() {
        *p = cubic(ctrl, i as f32 / NECK as f32);
    }
    b.polyline(
        &pts.map(|p| p + vec2(2.5, 3.0) * k),
        nw * 1.1,
        with_alpha(c(pal::SHADOW), 0.4),
    );
    b.polyline(&pts, nw, c(pal::COIL));
    // The side toward the light (upper left) of each point.
    let left = |i: usize| {
        let t = (pts[(i + 1).min(NECK)] - pts[i.saturating_sub(1)]).normalized();
        vec2(t.y, -t.x)
    };
    let mut lit = [Pos2::ZERO; NECK + 1];
    let mut dark = [Pos2::ZERO; NECK + 1];
    for i in 0..=NECK {
        lit[i] = pts[i] + left(i) * nw * 0.24;
        dark[i] = pts[i] - left(i) * nw * 0.3;
    }
    b.polyline(&lit, nw * 0.24, with_alpha(c(pal::COIL_LIGHT), 0.5));
    b.polyline(&dark, nw * 0.2, with_alpha(c(pal::COIL_SHADE), 0.7));
    // The coils: a groove and its lit ridge every few points along the neck.
    let step = (nw * 0.36).max(1.7 * k);
    let mut acc = 0.0;
    for i in 1..=NECK {
        let seg = pts[i] - pts[i - 1];
        let len = seg.length();
        if len <= 0.0 {
            continue;
        }
        let t = seg / len;
        let n = left(i);
        while acc < len {
            let p = pts[i - 1] + t * acc;
            b.segment(
                p + n * nw * 0.5,
                p - n * nw * 0.5,
                0.55 * k,
                with_alpha(c(pal::COIL_SHADE), 0.85),
                false,
            );
            let q = p + t * step * 0.4;
            b.segment(
                q + n * nw * 0.42,
                q - n * nw * 0.05,
                0.45 * k,
                with_alpha(c(pal::COIL_LIGHT), 0.4),
                false,
            );
            acc += step;
        }
        acc -= len;
    }
}

/// The panel socket: a chrome flange on its plate (two screws), the knurled nut.
fn socket(b: &mut Brush, k: f32, base: Rect, nut: Rect) {
    b.rrect_soft(
        base.translate(vec2(0.0, 2.0 * k)),
        2.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
        4.0 * k,
    );
    b.rrect(
        base,
        2.0 * k,
        Fill::V(c(pal::CHROME_LIGHT), c(pal::CHROME_DARK)),
    );
    b.hline(
        base.left() + 2.0 * k,
        base.right() - 2.0 * k,
        base.top() + 0.6 * k,
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.9),
    );
    let sr = (base.height() * 0.28).max(1.6 * k);
    for (j, x) in [base.left() + sr * 2.0, base.right() - sr * 2.0]
        .into_iter()
        .enumerate()
    {
        screw(b, pos2(x, base.center().y), sr, hash(j as u32 + 3));
    }
    let flange = Rect::from_min_max(
        pos2(nut.left() - 3.0 * k, nut.bottom()),
        pos2(nut.right() + 3.0 * k, base.top() + 0.5 * k),
    );
    b.hrect(flange, &cylinder());
    b.hrect(nut, &cylinder());
    let mut x = nut.left() + 1.0 * k;
    while x < nut.right() - 0.5 * k {
        b.vline(
            x,
            nut.top() + 0.8 * k,
            nut.bottom() - 0.8 * k,
            0.45 * k,
            with_alpha(c(pal::CHROME_DARK), 0.55),
        );
        x += 1.5 * k;
    }
    b.hline(
        nut.left(),
        nut.right(),
        nut.top() + 0.4 * k,
        0.8 * k,
        with_alpha(c(pal::CHROME_LIGHT), 0.9),
    );
}

/// The grille: a capsule `2·rr` wide from `top` down to `foot`, woven wire over the dark
/// foam, the voice's glow behind it, its light on the dome's left.
#[allow(clippy::too_many_arguments)]
fn grille(b: &mut Brush, k: f32, hx: f32, top: f32, foot: f32, rr: f32, lv: f32, glow: Color32) {
    let cy = top + rr;
    let mut shape = [Pos2::ZERO; 19];
    for (i, p) in shape.iter_mut().take(17).enumerate() {
        let a = PI + PI * i as f32 / 16.0;
        *p = pos2(hx + rr * a.cos(), cy + rr * a.sin());
    }
    shape[17] = pos2(hx + rr, foot);
    shape[18] = pos2(hx - rr, foot);
    b.polygon(&shape, c(pal::FOAM));
    let bound = Rect::from_min_max(pos2(hx - rr, top), pos2(hx + rr, foot));
    if lv > 0.01 {
        b.radial(
            bound,
            pos2(hx, cy + (foot - cy) * 0.35),
            vec2(rr * 0.85, (foot - top) * 0.42),
            &[
                (0.0, with_alpha(glow, 0.45 * lv)),
                (1.0, with_alpha(glow, 0.0)),
            ],
            8,
        );
    }
    // The half-width of the capsule at `y`.
    let half = |y: f32| {
        if y < cy {
            let d = cy - y;
            (rr * rr - d * d).max(0.0).sqrt()
        } else {
            rr
        }
    };
    let mesh = (rr * 0.11).max(2.0 * k);
    let wire = c(pal::WIRE);
    let mut y = top + mesh * 0.5;
    while y < foot - 0.3 * k {
        let h = half(y);
        b.segment(
            pos2(hx - h + 0.4 * k, y),
            pos2(hx + h - 0.4 * k, y),
            0.5 * k,
            with_alpha(wire, 0.42),
            false,
        );
        y += mesh;
    }
    let mut x = hx - rr + mesh * 0.5;
    while x < hx + rr {
        let d = x - hx;
        let y0 = cy - (rr * rr - d * d).max(0.0).sqrt();
        // The wires catch the light on the left of the cylinder.
        let u = (x - (hx - rr)) / (2.0 * rr);
        b.segment(
            pos2(x, y0 + 0.4 * k),
            pos2(x, foot),
            0.5 * k,
            with_alpha(wire, 0.2 + 0.35 * (1.0 - u)),
            false,
        );
        x += mesh;
    }
    // The cylinder's light and shade over the straight part, the dome's highlight.
    b.hrect(
        Rect::from_min_max(pos2(hx - rr, cy), pos2(hx + rr, foot)),
        &[
            (0.0, with_alpha(c(pal::SHADOW), 0.55)),
            (0.22, with_alpha(c(pal::SHADOW), 0.0)),
            (0.34, with_alpha(c(pal::CHROME_LIGHT), 0.12)),
            (0.5, with_alpha(c(pal::SHADOW), 0.0)),
            (1.0, with_alpha(c(pal::SHADOW), 0.65)),
        ],
    );
    b.radial(
        bound,
        pos2(hx - rr * 0.32, cy - rr * 0.35),
        Vec2::splat(rr * 0.42),
        &[
            (0.0, with_alpha(c(pal::CHROME_LIGHT), 0.3)),
            (1.0, with_alpha(c(pal::CHROME_LIGHT), 0.0)),
        ],
        6,
    );
    // The rim: a chrome wire round the capsule.
    let mut rim = [Pos2::ZERO; 19];
    rim[0] = pos2(hx - rr, foot);
    rim[1..18].copy_from_slice(&shape[..17]);
    rim[18] = pos2(hx + rr, foot);
    b.polyline(&rim, 1.1 * k, with_alpha(c(pal::CHROME), 0.75));
    b.polyline(&rim[2..9], 0.6 * k, with_alpha(c(pal::CHROME_LIGHT), 0.7));
}

/// A needle meter's dial in a face: the scale's arc round a pivot at the face's foot.
#[derive(Copy, Clone, Debug)]
pub struct Dial {
    pub pivot: Pos2,
    pub rad: f32,
}

/// The needle's swing either side of upright, radians.
const SWING: f32 = 0.8;

impl Dial {
    pub fn of(face: Rect) -> Dial {
        Dial {
            pivot: pos2(face.center().x, face.bottom() - face.height() * 0.04),
            rad: (face.height() * 0.66).min(face.width() * 0.46),
        }
    }

    /// The angle of the position `p` (0 … 1 along the scale).
    pub fn angle(&self, p: f32) -> f32 {
        -FRAC_PI_2 + (p.clamp(0.0, 1.0) * 2.0 - 1.0) * SWING
    }

    /// The point at `p` along the scale, `out` beyond the arc.
    pub fn at(&self, p: f32, out: f32) -> Pos2 {
        self.pivot + Vec2::angled(self.angle(p)) * (self.rad + out)
    }
}

/// A needle meter's face in `face`, lit from behind in `col`: its scale (major ticks at
/// `major`, minor at `minor`, positions 0 … 1), the red from `red` to the end, the needle
/// at `needle`, the pivot's cover, the glass. The display writes the numbers.
#[allow(clippy::too_many_arguments)]
pub fn meter(
    b: &mut Brush,
    k: f32,
    face: Rect,
    col: Color32,
    major: &[f32],
    minor: &[f32],
    red: f32,
    needle: f32,
) -> Dial {
    let d = Dial::of(face);
    b.rrect(face, 2.0 * k, Fill::V(c(pal::FACE_LIT), c(pal::FACE)));
    b.radial(
        face,
        d.pivot - vec2(0.0, d.rad * 0.55),
        vec2(face.width() * 0.6, face.height() * 0.62),
        &[(0.0, with_alpha(col, 0.13)), (1.0, with_alpha(col, 0.0))],
        10,
    );
    let ink = mix(col, c(pal::NEEDLE), 0.45);
    b.arc(
        d.pivot,
        d.rad,
        d.angle(0.0),
        d.angle(red),
        0.9 * k,
        ink,
        false,
    );
    b.arc(
        d.pivot,
        d.rad + 1.6 * k,
        d.angle(red),
        d.angle(1.0),
        3.2 * k,
        with_alpha(c(pal::RED), 0.9),
        false,
    );
    // The scale's second, finer arc just inside it.
    b.arc(
        d.pivot,
        d.rad - 4.0 * k,
        d.angle(0.0),
        d.angle(1.0),
        0.5 * k,
        with_alpha(ink, 0.45),
        false,
    );
    for &p in minor {
        b.segment(d.at(p, 0.0), d.at(p, 3.0 * k), 0.7 * k, ink, false);
    }
    for &p in major {
        let tick = if p >= red { c(pal::RED) } else { ink };
        b.segment(d.at(p, -2.0 * k), d.at(p, 6.0 * k), 1.1 * k, tick, false);
    }
    // The needle and its shadow on the face.
    let dir = Vec2::angled(d.angle(needle));
    let (n0, n1) = (
        d.pivot + dir * d.rad * 0.18,
        d.pivot + dir * (d.rad + 5.0 * k),
    );
    let sh = vec2(1.6, 2.4) * k;
    b.segment(
        n0 + sh,
        n1 + sh,
        1.3 * k,
        with_alpha(c(pal::SHADOW), 0.45),
        false,
    );
    b.segment(n0, n1, 1.2 * k, c(pal::NEEDLE), false);
    // The pivot's cover: a half-disc of satin black on the face's foot.
    let cr = (d.rad * 0.14).max(6.0 * k);
    let cc = pos2(d.pivot.x, face.bottom());
    let mut cover = [Pos2::ZERO; 13];
    for (i, p) in cover.iter_mut().enumerate() {
        let a = PI + PI * i as f32 / 12.0;
        *p = cc + Vec2::angled(a) * cr;
    }
    b.polygon(&cover, c(pal::SATIN));
    b.arc(
        cc,
        cr - 0.4 * k,
        PI,
        PI * 2.0,
        0.8 * k,
        with_alpha(c(pal::SATIN_LIGHT), 0.7),
        false,
    );
    glass(b, face, k);
    d
}
