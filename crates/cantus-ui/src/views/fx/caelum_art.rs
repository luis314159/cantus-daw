//! Caelum's objects (design sheet v2, the owner's "more elaborate"): the observatory's
//! window (a mitred metal frame with a raised bead, slotted screws, engraved scales on its
//! rails, the glass), the sill's plate the endless controls sit in (lit pushbuttons in
//! their bezels, the infinity lamp and the freeze lamp), the frost a frozen tank lays on
//! the glass, the glyphs (∞ and a crystal) drawn as lines. Vectors at the style's scale,
//! colours from `kit::skin::observatory` (one metal per colourway); nothing animates by
//! itself — the display hands each part its state. Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::observatory as pal;
use crate::kit::skin::Material;
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::{FRAC_PI_3, PI, TAU};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i` (a star, a grain line, a screw's turn): the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The objects' metal for the face's colourway: brass (Night), rose gold (Dawn), bronze
/// with verdigris (Aurora).
pub fn metal(s: &Style) -> &'static pal::Metal {
    match s.skin.material {
        Material::Paper => &pal::ROSE,
        Material::Glass => &pal::BRONZE,
        _ => &pal::BRASS,
    }
}

/// A slotted screw at `p`, its slot turned `turn` (0 … 1): an instrument maker's screw.
pub fn screw(b: &mut Brush, s: &Style, p: Pos2, rad: f32, turn: f32) {
    let m = metal(s);
    b.circle(
        p + vec2(0.0, rad * 0.3),
        rad * 1.15,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(
        p,
        rad * 1.1,
        Fill::Solid(mix(c(m.foot), c(pal::SHADOW), 0.4)),
    );
    b.circle(p, rad, Fill::V(c(m.edge), c(m.foot)));
    b.circle(p, rad * 0.8, Fill::V(c(m.face), c(m.top)));
    let d = Vec2::angled(turn * PI) * rad * 0.78;
    b.segment(
        p - d,
        p + d,
        rad * 0.28,
        with_alpha(c(m.engrave), 0.9),
        false,
    );
    b.segment(
        p - d + vec2(0.0, rad * 0.16),
        p + d + vec2(0.0, rad * 0.16),
        rad * 0.1,
        with_alpha(c(m.edge), 0.45),
        false,
    );
}

/// An engraved line from `a` to `z`: the cut, and the light its lower lip catches.
pub fn engrave(b: &mut Brush, s: &Style, a: Pos2, z: Pos2, w: f32) {
    let m = metal(s);
    b.segment(a, z, w, with_alpha(c(m.engrave), 0.85), false);
    let d = vec2(0.0, 0.7 * s.k);
    b.segment(a + d, z + d, w * 0.6, with_alpha(c(m.edge), 0.30), false);
}

/// The metal's face over `r`: lit at the top, its foot in shadow, `lines` of the brush
/// across it (horizontal), its polished edge.
fn sheet(b: &mut Brush, s: &Style, r: Rect, rad: f32, seed: u32) {
    let k = s.k;
    let m = metal(s);
    b.rrect(
        r,
        rad,
        Fill::Stops(&[
            (0.0, c(m.top)),
            (0.10, mix(c(m.top), c(m.face), 0.55)),
            (0.55, c(m.face)),
            (1.0, mix(c(m.face), c(m.foot), 0.7)),
        ]),
    );
    let mut y = r.top() + 1.5 * k;
    let mut i = seed;
    while y < r.bottom() - 1.5 * k {
        let a = 0.03 + 0.10 * hash(i);
        b.hline(
            r.left() + rad * 0.6,
            r.right() - rad * 0.6,
            y,
            0.5 * k,
            with_alpha(c(m.grain), a),
        );
        y += 1.3 * k;
        i += 1;
    }
    b.rrect_stroke(r, rad, 0.7 * k, with_alpha(c(m.edge), 0.5));
    b.hline(
        r.left() + rad,
        r.right() - rad,
        r.top() + 0.6 * k,
        1.0 * k,
        with_alpha(c(m.edge), 0.9),
    );
}

/// The observatory's window frame round `r`, `w` wide: its shadow on the deck, four mitred
/// rails brushed along their length, a raised bead round the middle, the chamfer down to
/// the glass, a slotted screw at each corner and in the middle of each long rail. The
/// glass's rect inside.
pub fn frame(b: &mut Brush, s: &Style, r: Rect, w: f32) -> Rect {
    let k = s.k;
    let m = metal(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 3.5 * k)),
        8.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.6)),
        10.0 * k,
    );
    b.rrect(
        r,
        6.0 * k,
        Fill::Stops(&[
            (0.0, c(m.top)),
            (0.04, mix(c(m.top), c(m.face), 0.5)),
            (0.5, c(m.face)),
            (1.0, mix(c(m.face), c(m.foot), 0.6)),
        ]),
    );
    let inner = r.shrink(w);
    // The brushing follows each rail; the rails meet at mitres.
    let step = 1.3 * k;
    let mut i = 0u32;
    let mut d = 1.2 * k;
    while d < w - 0.5 * k {
        let a = 0.035 + 0.12 * hash(i);
        let col = with_alpha(c(m.grain), a);
        b.hline(r.left() + d, r.right() - d, r.top() + d, 0.5 * k, col);
        b.hline(r.left() + d, r.right() - d, r.bottom() - d, 0.5 * k, col);
        let a2 = 0.035 + 0.12 * hash(i + 500);
        let col2 = with_alpha(c(m.grain), a2);
        b.vline(r.left() + d, r.top() + d, r.bottom() - d, 0.5 * k, col2);
        b.vline(r.right() - d, r.top() + d, r.bottom() - d, 0.5 * k, col2);
        d += step;
        i += 1;
    }
    // The mitres: a fine dark joint from each outer corner to the inner one.
    for (o, n) in [
        (r.left_top(), inner.left_top()),
        (r.right_top(), inner.right_top()),
        (r.left_bottom(), inner.left_bottom()),
        (r.right_bottom(), inner.right_bottom()),
    ] {
        b.segment(o, n, 0.7 * k, with_alpha(c(m.engrave), 0.55), false);
    }
    // The bead: a raised moulding round the middle of the rails.
    let bead = r.shrink(w * 0.5);
    b.rrect_stroke(
        bead.translate(vec2(0.0, 0.6 * k)),
        3.0 * k,
        1.0 * k,
        with_alpha(c(m.foot), 0.75),
    );
    b.rrect_stroke(bead, 3.0 * k, 0.8 * k, with_alpha(c(m.edge), 0.55));
    // The polished outer edge, lit along the top.
    b.rrect_stroke(r, 6.0 * k, 0.8 * k, with_alpha(c(m.edge), 0.5));
    b.hline(
        r.left() + 6.0 * k,
        r.right() - 6.0 * k,
        r.top() + 0.6 * k,
        1.0 * k,
        with_alpha(c(m.edge), 0.95),
    );
    // The chamfer down to the glass: its upper wall in shadow, its lower catching light.
    b.rrect(
        inner.expand(2.0 * k),
        3.0 * k,
        Fill::V(mix(c(m.foot), c(pal::SHADOW), 0.5), c(m.top)),
    );
    b.rrect(inner, 2.0 * k, Fill::Solid(c(pal::GASKET)));
    let spots = [
        pos2(r.left() + w * 0.5, r.top() + w * 0.5),
        pos2(r.right() - w * 0.5, r.top() + w * 0.5),
        pos2(r.left() + w * 0.5, r.bottom() - w * 0.5),
        pos2(r.right() - w * 0.5, r.bottom() - w * 0.5),
        pos2(r.center().x, r.top() + w * 0.5),
        pos2(r.center().x, r.bottom() - w * 0.5),
    ];
    for (j, p) in spots.into_iter().enumerate() {
        screw(b, s, p, w * 0.24, hash(j as u32 + 41));
    }
    inner.shrink(1.0 * k)
}

/// Glass over `r`: a broad faint reflection across its upper left, a thin one, a lit edge,
/// and the shade the frame throws inward along the top.
pub fn glass(b: &mut Brush, r: Rect, k: f32) {
    let (w, h) = (r.width(), r.height());
    let clip = |p: Pos2| pos2(p.x.clamp(r.left(), r.right()), p.y);
    let broad = [
        pos2(r.left() + w * 0.05, r.top()),
        pos2(r.left() + w * 0.27, r.top()),
        pos2(r.left() + w * 0.13, r.top() + h),
        pos2(r.left() - w * 0.06, r.top() + h),
    ];
    b.polygon(&broad.map(clip), with_alpha(c(pal::GLASS), 0.026));
    let thin = [
        pos2(r.left() + w * 0.31, r.top()),
        pos2(r.left() + w * 0.335, r.top()),
        pos2(r.left() + w * 0.21, r.top() + h),
        pos2(r.left() + w * 0.185, r.top() + h),
    ];
    b.polygon(&thin.map(clip), with_alpha(c(pal::GLASS), 0.02));
    b.rect(
        Rect::from_min_max(r.min, pos2(r.right(), r.top() + 7.0 * k)),
        Fill::V(
            with_alpha(c(pal::SHADOW), 0.35),
            with_alpha(c(pal::SHADOW), 0.0),
        ),
    );
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.top() + 0.5 * k,
        k,
        with_alpha(c(pal::GLASS), 0.14),
    );
}

/// A metal plate over `r` (the sill the endless controls sit in): its shadow, its brushed
/// face, an inset bead, a slotted screw at each end. The rect inside the screws.
pub fn plate(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let m = metal(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 2.5 * k)),
        6.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
        8.0 * k,
    );
    sheet(b, s, r, 5.0 * k, 900);
    b.rrect_stroke(
        r.shrink(3.5 * k),
        3.0 * k,
        0.7 * k,
        with_alpha(c(m.engrave), 0.35),
    );
    b.rrect_stroke(
        r.shrink(3.5 * k).translate(vec2(0.0, 0.7 * k)),
        3.0 * k,
        0.5 * k,
        with_alpha(c(m.edge), 0.25),
    );
    let sr = 3.6 * k;
    for (j, p) in [
        pos2(r.left() + 10.0 * k, r.center().y),
        pos2(r.right() - 10.0 * k, r.center().y),
    ]
    .into_iter()
    .enumerate()
    {
        screw(b, s, p, sr, hash(j as u32 + 77));
    }
    plate_inner(r, k)
}

/// The rect inside a [`plate`]'s screws.
pub fn plate_inner(r: Rect, k: f32) -> Rect {
    Rect::from_min_max(
        pos2(r.left() + 20.0 * k, r.top() + 6.0 * k),
        pos2(r.right() - 20.0 * k, r.bottom() - 6.0 * k),
    )
}

/// What a pushbutton shows: how lit its lens is, hovered, held down.
#[derive(Copy, Clone, Debug, Default)]
pub struct Press {
    pub lit: f32,
    pub hover: bool,
    pub down: bool,
}

/// A lit pushbutton in its bezel over `r`: the metal ring, the well, the cap — dark glass
/// unlit, lit through in `lens` (its glow spilling onto the plate). The cap's rect (where
/// its words and glyph go).
pub fn button(b: &mut Brush, s: &Style, r: Rect, lens: Color32, p: Press) -> Rect {
    let k = s.k;
    let m = metal(s);
    let l = p.lit.clamp(0.0, 1.0);
    if l > 0.0 {
        b.radial(
            r.expand(14.0 * k),
            r.center(),
            r.size() * 0.5 + Vec2::splat(12.0 * k),
            &[
                (0.0, with_alpha(lens, 0.30 * l)),
                (0.6, with_alpha(lens, 0.10 * l)),
                (1.0, with_alpha(lens, 0.0)),
            ],
            8,
        );
    }
    b.rrect(
        r.translate(vec2(0.0, 1.2 * k)),
        4.5 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.5)),
    );
    b.rrect(r, 4.5 * k, Fill::V(c(m.edge), c(m.foot)));
    b.rrect(r.shrink(1.0 * k), 4.0 * k, Fill::V(c(m.top), c(m.face)));
    let well = r.shrink(2.4 * k);
    b.rrect(well, 3.0 * k, Fill::Solid(c(pal::GASKET)));
    let cap = well
        .shrink(1.3 * k)
        .translate(vec2(0.0, if p.down { 0.8 * k } else { 0.0 }));
    let dark = c(pal::CAP);
    let hot = mix(lens, c(pal::STAR), 0.35);
    b.rrect(
        cap,
        2.5 * k,
        Fill::V(
            mix(mix(dark, c(pal::CAP_LIGHT), 0.35), hot, l * 0.85),
            mix(dark, mix(lens, dark, 0.35), l),
        ),
    );
    // The cap's top edge catching the light, brighter when hovered.
    b.hline(
        cap.left() + 2.0 * k,
        cap.right() - 2.0 * k,
        cap.top() + 0.6 * k,
        0.8 * k,
        with_alpha(
            if l > 0.3 {
                c(pal::STAR)
            } else {
                c(pal::CAP_LIGHT)
            },
            if p.hover { 0.9 } else { 0.55 },
        ),
    );
    if p.hover {
        b.rrect_stroke(cap, 2.5 * k, 0.8 * k, with_alpha(c(m.edge), 0.55));
    }
    cap
}

/// The ink for words on a cap: dark on a lit cap, pale on a dark one.
pub fn cap_ink(lens: Color32, lit: f32) -> Color32 {
    if lit > 0.5 {
        mix(c(pal::CAP), lens, 0.12)
    } else {
        with_alpha(c(pal::STAR), 0.78)
    }
}

/// Semitones as the instruments write them: "0", "+12", "−5".
pub struct Signed(pub f32);

impl std::fmt::Display for Signed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0.round();
        if v == 0.0 {
            write!(f, "0")
        } else if v > 0.0 {
            write!(f, "+{v:.0}")
        } else {
            write!(f, "\u{2212}{:.0}", -v)
        }
    }
}

/// A pilot lamp at `p`: a metal bezel, a lens of `lens`, its glow (`lit` 0 … 1).
pub fn pilot(b: &mut Brush, s: &Style, p: Pos2, rad: f32, lens: Color32, lit: f32) {
    let m = metal(s);
    let l = lit.clamp(0.0, 1.0);
    if l > 0.0 {
        b.radial(
            Rect::from_center_size(p, Vec2::splat(rad * 7.0)),
            p,
            Vec2::splat(rad * 3.2),
            &[
                (0.0, with_alpha(lens, 0.5 * l)),
                (1.0, with_alpha(lens, 0.0)),
            ],
            10,
        );
    }
    b.circle(
        p + vec2(0.0, rad * 0.25),
        rad * 1.45,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    b.circle(p, rad * 1.4, Fill::V(c(m.edge), c(m.foot)));
    b.circle(p, rad * 1.12, Fill::Solid(c(pal::GASKET)));
    let dark = mix(lens, c(pal::SHADOW), 0.75);
    b.circle(
        p,
        rad,
        Fill::V(
            mix(dark, mix(lens, c(pal::STAR), 0.45), l),
            mix(mix(dark, c(pal::SHADOW), 0.4), lens, l * 0.8),
        ),
    );
    b.circle(
        p + vec2(-0.35, -0.38) * rad,
        rad * 0.24,
        Fill::Solid(with_alpha(c(pal::STAR), 0.7)),
    );
}

/// The lemniscate (∞) centred at `p`, `w` across, drawn as a line `t` thick.
pub fn infinity(b: &mut Brush, p: Pos2, w: f32, t: f32, col: Color32) {
    let a = w * 0.5;
    let pts: [Pos2; 49] = std::array::from_fn(|i| {
        let u = i as f32 / 48.0 * TAU;
        let (su, cu) = (sin(u), cos(u));
        let d = 1.0 + su * su;
        pos2(p.x + a * cu / d, p.y + a * su * cu / d)
    });
    b.polyline(&pts, t, col);
}

/// A crystal (the freeze) centred at `p`, `rad` to its arms' tips: six arms, each with two
/// pairs of side branches.
pub fn crystal(b: &mut Brush, p: Pos2, rad: f32, t: f32, col: Color32) {
    for i in 0..6 {
        let a = -PI * 0.5 + i as f32 * FRAC_PI_3;
        let dir = Vec2::angled(a);
        b.segment(p, p + dir * rad, t, col, true);
        for (at, len) in [(0.45, 0.32), (0.72, 0.22)] {
            let q = p + dir * rad * at;
            for side in [-1.0f32, 1.0] {
                let br = Vec2::angled(a + side * FRAC_PI_3);
                b.segment(q, q + br * rad * len, t * 0.8, col, true);
            }
        }
    }
}

/// Frost on the glass over `r`, as much as `amount` (0 … 1): a pale haze in each corner and
/// fronds of ice growing inward from it, each branching.
pub fn frost(b: &mut Brush, r: Rect, k: f32, amount: f32) {
    let a = amount.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let ice = c(pal::ICE);
    let reach = r.height().min(r.width()) * 0.42;
    let corners = [
        (r.left_top(), vec2(1.0, 1.0)),
        (r.right_top(), vec2(-1.0, 1.0)),
        (r.left_bottom(), vec2(1.0, -1.0)),
        (r.right_bottom(), vec2(-1.0, -1.0)),
    ];
    for (ci, (o, dir)) in corners.into_iter().enumerate() {
        b.radial(
            r,
            o,
            Vec2::splat(reach),
            &[
                (0.0, with_alpha(ice, 0.22 * a)),
                (0.5, with_alpha(ice, 0.07 * a)),
                (1.0, with_alpha(ice, 0.0)),
            ],
            8,
        );
        for f in 0..9u32 {
            let seed = ci as u32 * 100 + f;
            // A frond's direction within the corner's quarter, its length.
            let ang = (0.08 + 0.84 * hash(seed)) * PI * 0.5;
            let d = vec2(cos(ang) * dir.x, sin(ang) * dir.y);
            let len = reach * (0.35 + 0.6 * hash(seed + 7)) * a.sqrt();
            let start = o + vec2(dir.x, dir.y) * 2.0 * k;
            let end = start + d * len;
            b.segment(start, end, 0.8 * k, with_alpha(ice, 0.45 * a), false);
            // Its branches, shorter toward the tip, at sixty degrees.
            let n = 5;
            for j in 1..n {
                let u = j as f32 / n as f32;
                let q = start + d * len * u;
                let bl = len * 0.28 * (1.0 - u) * (0.6 + 0.4 * hash(seed * 13 + j));
                for side in [-1.0f32, 1.0] {
                    let rot = side * FRAC_PI_3;
                    let (sr, cr) = (sin(rot), cos(rot));
                    let bd = vec2(d.x * cr - d.y * sr, d.x * sr + d.y * cr);
                    b.segment(q, q + bd * bl, 0.6 * k, with_alpha(ice, 0.32 * a), false);
                }
            }
        }
    }
}
