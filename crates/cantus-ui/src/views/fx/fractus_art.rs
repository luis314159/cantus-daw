//! Fractus's objects (design sheet v2, the owner's "more elaborate"): the sequencer's
//! machined chassis (a finish per colourway: black anodised and brushed, white powder coat,
//! violet lacquer with chrome lips) with hex-socket screws, the windows cut in it (a
//! chamfer the cutter left bright, a gasket, glass), the step pads behind the glass (lit
//! from the floor up to their amount), the seven-segment step counter, the square LEDs, the
//! keys that play the lanes. Vectors at the style's scale, colours from
//! `kit::skin::sequencers`; nothing animates by itself — the display hands each part its
//! state. Original to Cantus.

use crate::kit::color::{mix, with_alpha, Hsl};
use crate::kit::skin::sequencers as pal;
use crate::kit::skin::Material;
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Color32, Pos2, Rect, Vec2};

pub fn c(h: Hsl) -> Color32 {
    h.rgb()
}

/// A number in 0 … 1 for `i` (a grain line, a speck, a screw's turn): the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The chassis's finish for the face's colourway.
pub fn finish(s: &Style) -> &'static pal::Chassis {
    match s.skin.material {
        Material::Paper => &pal::POWDER,
        Material::Lacquer => &pal::VIOLET,
        _ => &pal::ANODISED,
    }
}

/// A hex-socket cap screw at `p`, its socket turned `turn` (0 … 1).
pub fn hex_screw(b: &mut Brush, p: Pos2, rad: f32, turn: f32) {
    b.circle(
        p + vec2(0.0, rad * 0.3),
        rad * 1.2,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.45)),
    );
    // The counterbore, then the cap: lit on its upper left, its knurled rim.
    b.circle(p, rad * 1.12, Fill::Solid(c(pal::GASKET)));
    b.circle(p, rad, Fill::V(c(pal::IVORY), c(pal::EBONY_LIGHT)));
    b.circle(p, rad * 0.86, Fill::V(c(pal::EBONY_LIGHT), c(pal::IVORY)));
    // The hexagonal socket.
    let a0 = turn * std::f32::consts::FRAC_PI_3;
    let hex: [Pos2; 6] = std::array::from_fn(|i| {
        p + Vec2::angled(a0 + i as f32 * std::f32::consts::FRAC_PI_3) * rad * 0.48
    });
    b.polygon(&hex, c(pal::GASKET));
    b.circle(
        p + vec2(-0.4, -0.45) * rad,
        rad * 0.18,
        Fill::Solid(with_alpha(c(pal::IVORY), 0.7)),
    );
}

/// The machined chassis over `r`: its shadow on the deck, the face lit from above, the
/// finish's brushing or speckle, the bright cut edge round it, a hex screw in each corner.
pub fn chassis(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let f = finish(s);
    b.rrect_soft(
        r.translate(vec2(0.0, 3.0 * k)),
        8.0 * k,
        Fill::Solid(with_alpha(c(pal::SHADOW), 0.6)),
        9.0 * k,
    );
    b.rrect(
        r,
        7.0 * k,
        Fill::Stops(&[
            (0.0, c(f.top)),
            (0.08, mix(c(f.top), c(f.face), 0.6)),
            (0.55, c(f.face)),
            (1.0, mix(c(f.face), c(f.foot), 0.7)),
        ]),
    );
    if f.brushed {
        // Brushed: fine horizontal lines, each its own strength.
        let mut y = r.top() + 2.0 * k;
        let mut i = 0u32;
        while y < r.bottom() - 2.0 * k {
            let a = 0.03 + 0.09 * hash(i);
            b.hline(
                r.left() + 3.0 * k,
                r.right() - 3.0 * k,
                y,
                0.5 * k,
                with_alpha(c(f.grain), a),
            );
            y += 1.4 * k;
            i += 1;
        }
    } else {
        // Powder or lacquer: a fine speckle (lacquer's is its metal flake).
        let n = ((r.width() * r.height()) / (90.0 * k * k)) as u32;
        for i in 0..n.min(2_400) {
            let p = pos2(
                r.left() + 3.0 * k + hash(i * 2 + 1) * (r.width() - 6.0 * k),
                r.top() + 3.0 * k + hash(i * 2 + 2) * (r.height() - 6.0 * k),
            );
            let a = 0.05 + 0.12 * hash(i + 9_001);
            b.dot(p, 0.45 * k, with_alpha(c(f.grain), a));
        }
    }
    // The cut edge: bright along the top, a soft line round, the foot in shadow.
    b.rrect_stroke(r, 7.0 * k, 0.8 * k, with_alpha(c(f.edge), 0.55));
    b.hline(
        r.left() + 7.0 * k,
        r.right() - 7.0 * k,
        r.top() + 0.7 * k,
        1.0 * k,
        with_alpha(c(f.edge), 0.85),
    );
    b.hline(
        r.left() + 7.0 * k,
        r.right() - 7.0 * k,
        r.bottom() - 0.7 * k,
        1.0 * k,
        with_alpha(c(pal::SHADOW), 0.5),
    );
    if f.lips {
        b.rrect_stroke(
            r.shrink(2.0 * k),
            5.5 * k,
            0.9 * k,
            with_alpha(c(pal::IVORY), 0.35),
        );
    }
    let m = 8.0 * k;
    for (j, p) in [
        pos2(r.left() + m, r.top() + m),
        pos2(r.right() - m, r.top() + m),
        pos2(r.left() + m, r.bottom() - m),
        pos2(r.right() - m, r.bottom() - m),
    ]
    .into_iter()
    .enumerate()
    {
        hex_screw(b, p, 3.6 * k, hash(j as u32 + 31));
    }
}

/// A window cut in the chassis round `r`: the chamfer (its upper wall in shadow, its lower
/// catching the light), chrome lips on lacquer, the gasket. The glass's rect inside.
pub fn window(b: &mut Brush, s: &Style, r: Rect) -> Rect {
    let k = s.k;
    let f = finish(s);
    let cut = 4.0 * k;
    b.rrect(
        r,
        4.5 * k,
        Fill::V(
            mix(c(f.foot), c(pal::SHADOW), 0.5),
            mix(c(f.face), c(f.edge), 0.7),
        ),
    );
    b.rrect_stroke(r, 4.5 * k, 0.7 * k, with_alpha(c(f.edge), 0.5));
    let inner = r.shrink(cut);
    if f.lips {
        b.rrect_stroke(
            inner.expand(1.2 * k),
            3.5 * k,
            1.1 * k,
            with_alpha(c(pal::IVORY), 0.75),
        );
    }
    b.rrect(inner, 3.0 * k, Fill::Solid(c(pal::GASKET)));
    inner.shrink(1.5 * k)
}

/// Glass over `r`: a broad faint reflection across its upper left, a thin one, a lit edge.
pub fn glass(b: &mut Brush, r: Rect, k: f32) {
    let (w, h) = (r.width(), r.height());
    let clip = |p: Pos2| pos2(p.x.clamp(r.left(), r.right()), p.y);
    let broad = [
        pos2(r.left() + w * 0.04, r.top()),
        pos2(r.left() + w * 0.30, r.top()),
        pos2(r.left() + w * 0.14, r.top() + h),
        pos2(r.left() - w * 0.04, r.top() + h),
    ];
    b.polygon(&broad.map(clip), with_alpha(c(pal::IVORY), 0.028));
    let thin = [
        pos2(r.left() + w * 0.34, r.top()),
        pos2(r.left() + w * 0.37, r.top()),
        pos2(r.left() + w * 0.23, r.top() + h),
        pos2(r.left() + w * 0.20, r.top() + h),
    ];
    b.polygon(&thin.map(clip), with_alpha(c(pal::IVORY), 0.022));
    b.hline(
        r.left() + 2.0 * k,
        r.right() - 2.0 * k,
        r.top() + 0.5 * k,
        k,
        with_alpha(c(pal::IVORY), 0.14),
    );
}

/// A step pad behind the glass in `r`: its well and rim; painted, lit from the floor up to
/// `level` (0 … 1, the block's amount) in `col` with a hot top edge and a glow; `hot` while
/// the playhead is on it and the lane sounds.
///
/// `odd`: the pad is in an odd beat (beats 2 and 4), its well a shade lighter, so the beats
/// read as groups of pads.
pub fn pad(
    b: &mut Brush,
    r: Rect,
    k: f32,
    level: Option<f32>,
    col: Color32,
    hot: bool,
    odd: bool,
) {
    let rad = (1.6 * k).min(r.width() * 0.3);
    let floor = if odd {
        mix(c(pal::PAD), c(pal::PAD_RIM), 0.22)
    } else {
        c(pal::PAD)
    };
    b.rrect(r, rad, Fill::V(c(pal::GASKET), floor));
    b.rrect_stroke(r, rad, 0.6 * k, with_alpha(c(pal::PAD_RIM), 0.9));
    // The well's lower lip catching the light.
    b.hline(
        r.left() + rad,
        r.right() - rad,
        r.bottom() - 0.6 * k,
        0.6 * k,
        with_alpha(c(pal::IVORY), 0.06),
    );
    let Some(level) = level else {
        return;
    };
    let well = r.shrink(1.2 * k);
    // The pad's whole height is 100 %: tinted, its top ruled, so the lit part reads as a share.
    b.rrect(well, rad * 0.7, Fill::Solid(with_alpha(col, 0.11)));
    b.hline(
        well.left() + 0.6 * k,
        well.right() - 0.6 * k,
        well.top() + 0.4 * k,
        0.8 * k,
        with_alpha(col, 0.45),
    );
    let top = well.bottom() - well.height() * level.clamp(0.0, 1.0).max(0.08);
    let lit = Rect::from_min_max(pos2(well.left(), top), well.max);
    let a = if hot { 1.0 } else { 0.78 };
    b.rrect_soft(
        lit.expand(2.0 * k),
        rad,
        Fill::Solid(with_alpha(col, if hot { 0.45 } else { 0.22 })),
        5.0 * k,
    );
    b.rrect(
        lit,
        rad * 0.7,
        Fill::V(
            with_alpha(mix(col, c(pal::IVORY), 0.25), a),
            with_alpha(col, a * 0.85),
        ),
    );
    // The lit top: the amount's edge, hot.
    b.hline(
        lit.left() + 0.6 * k,
        lit.right() - 0.6 * k,
        lit.top() + 0.5 * k,
        1.0 * k,
        with_alpha(mix(col, c(pal::IVORY), if hot { 0.75 } else { 0.5 }), a),
    );
}

/// A square LED in its black bezel at `p`, `side` across, `lit` 0 … 1 in `col`.
pub fn led(b: &mut Brush, p: Pos2, side: f32, col: Color32, lit: f32) {
    let l = lit.clamp(0.0, 1.0);
    let r = Rect::from_center_size(p, Vec2::splat(side));
    if l > 0.0 {
        b.rrect_soft(
            r.expand(side * 0.5),
            side * 0.4,
            Fill::Solid(with_alpha(col, 0.35 * l)),
            side * 0.8,
        );
    }
    b.rrect(
        r.expand(side * 0.18),
        side * 0.2,
        Fill::Solid(c(pal::GASKET)),
    );
    let off = mix(col, c(pal::SHADOW), 0.78);
    b.rrect(
        r,
        side * 0.12,
        Fill::V(
            mix(off, mix(col, c(pal::IVORY), 0.5), l),
            mix(mix(off, c(pal::SHADOW), 0.3), col, l),
        ),
    );
    b.hline(
        r.left() + side * 0.15,
        r.right() - side * 0.15,
        r.top() + side * 0.18,
        side * 0.12,
        with_alpha(c(pal::IVORY), 0.15 + 0.35 * l),
    );
}

/// The segments a digit lights (a … g: the top, clockwise, the middle last).
const SEGS: [u8; 10] = [
    0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101,
    0b000_0111, 0b111_1111, 0b110_1111,
];

/// A seven-segment digit in `r`, slanted a little: `d` lit in `col`, the others faint.
pub fn digit(b: &mut Brush, r: Rect, d: u8, col: Color32) {
    let w = r.width();
    let h = r.height();
    let t = (w * 0.2).max(1.0);
    let slant = w * 0.12;
    let at = |x: f32, y: f32| pos2(r.left() + x + slant * (1.0 - y / h), r.top() + y);
    let g = t * 0.15;
    let h2 = t * 0.5;
    let (x0, x1) = (t * 0.5, w - t * 0.5 - slant);
    let (y0, y1, y2) = (t * 0.5, h * 0.5, h - t * 0.5);
    let m = SEGS[usize::from(d.min(9))];
    let parts = [
        ((x0, y0), (x1, y0)),
        ((x1, y0), (x1, y1)),
        ((x1, y1), (x1, y2)),
        ((x0, y2), (x1, y2)),
        ((x0, y1), (x0, y2)),
        ((x0, y0), (x0, y1)),
        ((x0, y1), (x1, y1)),
    ];
    for (i, (a, z)) in parts.into_iter().enumerate() {
        // Each segment a hexagon from `a` to `z`, `t` thick.
        let pts: [Pos2; 6] = if (a.1 - z.1).abs() < 0.01 {
            [
                at(a.0 + g, a.1),
                at(a.0 + g + h2, a.1 - h2),
                at(z.0 - g - h2, z.1 - h2),
                at(z.0 - g, z.1),
                at(z.0 - g - h2, z.1 + h2),
                at(a.0 + g + h2, a.1 + h2),
            ]
        } else {
            [
                at(a.0, a.1 + g),
                at(a.0 + h2, a.1 + g + h2),
                at(z.0 + h2, z.1 - g - h2),
                at(z.0, z.1 - g),
                at(z.0 - h2, z.1 - g - h2),
                at(a.0 - h2, a.1 + g + h2),
            ]
        };
        let on = m & (1 << i) != 0;
        b.polygon(&pts, if on { col } else { with_alpha(col, 0.08) });
    }
}

/// Whether MIDI note `n` is a black key.
pub fn black(n: i32) -> bool {
    matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

/// One key of the keyboard as [`layout`] lays it.
#[derive(Copy, Clone, Debug)]
pub struct Key {
    pub note: i32,
    pub rect: Rect,
    pub black: bool,
}

/// The keys from `lo` to `hi` across `r` (a whole white key at each end): the white keys
/// first, then the black ones over them.
pub fn layout(r: Rect, lo: i32, hi: i32, out: &mut [Option<Key>; 16]) {
    let lo = if black(lo) { lo - 1 } else { lo };
    let hi = if black(hi) { hi + 1 } else { hi };
    let whites = (lo..=hi).filter(|n| !black(*n)).count().max(1);
    let ww = r.width() / whites as f32;
    let bw = ww * 0.62;
    let bh = r.height() * 0.6;
    let mut slot = 0usize;
    let mut i = 0usize;
    for n in (lo..=hi).filter(|n| !black(*n)) {
        if slot < out.len() {
            let x = r.left() + i as f32 * ww;
            out[slot] = Some(Key {
                note: n,
                rect: Rect::from_min_max(pos2(x, r.top()), pos2(x + ww, r.bottom())),
                black: false,
            });
            slot += 1;
        }
        i += 1;
    }
    let mut i = 0usize;
    for n in lo..=hi {
        if !black(n) {
            i += 1;
        } else if slot < out.len() {
            let x = r.left() + i as f32 * ww;
            out[slot] = Some(Key {
                note: n,
                rect: Rect::from_min_max(
                    pos2(x - bw * 0.5, r.top()),
                    pos2(x + bw * 0.5, r.top() + bh),
                ),
                black: true,
            });
            slot += 1;
        }
    }
}

/// One key drawn: ivory or ebony, pressed and lit in `glow` while `on`, a little dimmer
/// when it plays no lane (`lane` false).
pub fn key(b: &mut Brush, k: f32, key: &Key, on: bool, lane: bool, glow: Color32) {
    let r = key.rect;
    let press = if on { 1.4 * k } else { 0.0 };
    if key.black {
        let body = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() + press * 0.5));
        b.rrect_soft(
            body.translate(vec2(1.2 * k, 2.0 * k)),
            1.5 * k,
            Fill::Solid(with_alpha(c(pal::SHADOW), 0.55)),
            3.0 * k,
        );
        if on {
            b.rrect_soft(
                body.expand(2.0 * k),
                2.0 * k,
                Fill::Solid(with_alpha(glow, 0.4)),
                7.0 * k,
            );
        }
        let face = if on {
            mix(c(pal::EBONY), glow, 0.7)
        } else {
            c(pal::EBONY)
        };
        b.rrect(
            body,
            1.5 * k,
            Fill::V(face, mix(face, c(pal::EBONY_LIGHT), 0.35)),
        );
        // The lit bevel down its left and the slope at its front.
        let front = Rect::from_min_max(
            pos2(body.left() + 1.2 * k, body.bottom() - 5.0 * k),
            pos2(body.right() - 1.2 * k, body.bottom() - 1.0 * k),
        );
        b.rrect(
            front,
            1.0 * k,
            Fill::V(
                with_alpha(c(pal::EBONY_LIGHT), 0.6),
                with_alpha(c(pal::EBONY_LIGHT), 0.1),
            ),
        );
        b.vline(
            body.left() + 1.4 * k,
            body.top() + 1.0 * k,
            body.bottom() - 6.0 * k,
            0.8 * k,
            with_alpha(c(pal::EBONY_LIGHT), 0.7),
        );
        return;
    }
    let body = Rect::from_min_max(
        pos2(r.left() + 0.6 * k, r.top()),
        pos2(r.right() - 0.6 * k, r.bottom() - 2.0 * k + press),
    );
    let ivory = if lane {
        c(pal::IVORY)
    } else {
        mix(c(pal::IVORY), c(pal::IVORY_SHADE), 0.6)
    };
    let face = if on { mix(ivory, glow, 0.55) } else { ivory };
    if on {
        b.rrect_soft(
            body.expand(1.0 * k),
            2.0 * k,
            Fill::Solid(with_alpha(glow, 0.45)),
            7.0 * k,
        );
    }
    b.rrect(
        body,
        2.0 * k,
        Fill::Stops(&[
            (0.0, mix(face, c(pal::IVORY_SHADE), 0.35)),
            (0.12, face),
            (0.85, face),
            (1.0, mix(face, c(pal::IVORY_SHADE), 0.6)),
        ]),
    );
    b.vline(
        body.right() - 0.4 * k,
        body.top(),
        body.bottom(),
        0.8 * k,
        with_alpha(c(pal::IVORY_SHADE), 0.9),
    );
    if !on {
        // Its front lip, which a pressed key hides.
        b.hline(
            body.left() + 1.0 * k,
            body.right() - 1.0 * k,
            body.bottom() - 1.0 * k,
            1.6 * k,
            with_alpha(c(pal::IVORY_SHADE), 0.8),
        );
    }
}
