//! The routing cables' three looks (round 39; des-mixer's `options-cables-*` and the
//! README's "The routing cables, designed"; the owner's choice of 2026-11-09: *Flow* by
//! default, all three selectable): the jacks, the curves, the subway's lanes, the widths
//! and the cable tokens, the gleam's dashes, the hit test — pure geometry and colour, no
//! egui UI, no allocation once the caller's lists have grown.
//!
//! - **Patch** (A): one cubic per cable, P0 = the source's output jack, P1 straight below
//!   it, P2 straight below the target, P3 = the target's input jack; the sag
//!   k = min(58, 16 + 0.15·|dx|), so a longer cable hangs lower.
//! - **Flow** (B): Patch, plus a 7-px gleam every 64 px travelling from source to target
//!   while audio flows; its offset comes from the transport's time, never accumulated.
//! - **Subway** (C): down from the jack, across the channel, up into the target, corners
//!   r 4; a lane per kind (outputs +14, sends +26, keys +40 below the jacks), sub-lanes
//!   4 px apart where same-kind spans overlap (6 px margin), the shortest span first.

use super::look::Pal;
use crate::kit::color::mix;
use egui::{pos2, Color32, Pos2};

/// The jacks' centre below the band's top: the strip's foot row is 18, the jacks 9 up.
pub const JACK_Y: f32 = 9.0;
/// The sockets' row: the strip's foot, where the jacks are.
pub const JACK_ROW: f32 = 18.0;
/// The channel under the strips, at least (the deepest patch cable hangs 43.5 below).
pub const CHANNEL: f32 = 46.0;
/// A jack's hole.
pub const JACK_R: f32 = 4.0;
/// The input jack sits this far left of the strip's centre, the output this far right.
pub const JACK_OFF: f32 = 11.0;
/// Segments a curve is flattened to.
pub const SEGS: usize = 24;
/// How near the pointer must come to a cable's edge to hover it.
pub const HIT: f32 = 4.0;
/// The subway's corner radius.
pub const CORNER: f32 = 4.0;
/// The subway's lanes below the jacks, by kind, and the sub-lanes' pitch.
pub const LANE: [f32; 3] = [14.0, 26.0, 40.0];
pub const SUB: f32 = 4.0;
/// Two same-kind spans this near share no sub-lane.
pub const MARGIN: f32 = 6.0;
/// The gleam: a dash this long every period, at this speed (px/s) at silence…full.
pub const GLEAM: f32 = 7.0;
pub const PERIOD: f32 = 64.0;

/// A cable's kind, in the order a jack takes its colour (output, then send, then key).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Output,
    Send,
    Key,
}

impl Kind {
    pub fn rank(self) -> usize {
        match self {
            Kind::Output => 0,
            Kind::Send => 1,
            Kind::Key => 2,
        }
    }
}

/// A strip's input and output jacks' x: centre `c`, column `w` wide (Compact's 30-pt
/// strip brings them in so the holes stay on the strip).
pub fn jacks(c: f32, w: f32) -> (f32, f32) {
    let off = JACK_OFF.min(((w - 2.0) * 0.5 - JACK_R - 0.5).max(JACK_R + 1.0));
    (c - off, c + off)
}

/// The patch cable's sag for a span of `dx`.
pub fn sag(dx: f32) -> f32 {
    (16.0 + 0.15 * dx.abs()).min(58.0)
}

/// A cable's width: an output 2 px, a send 1 + 1.6 × its level (−36…0 dB), a key 1.5.
pub fn width(kind: Kind, db: f32) -> f32 {
    match kind {
        Kind::Output => 2.0,
        Kind::Send => 1.0 + 1.6 * ((db + 36.0) / 36.0).clamp(0.0, 1.0),
        Kind::Key => 1.5,
    }
}

/// The cable tokens (README, "Colours"): every one a theme token or a mix of two.
#[derive(Copy, Clone, Debug)]
pub struct Tokens {
    pub out: Color32,
    pub send: Color32,
    pub key: Color32,
    pub refused: Color32,
    pub channel: Color32,
    pub hole: Color32,
    /// A jack's ring at rest.
    pub ring: Color32,
    dark: bool,
    edge: Color32,
    ink1: Color32,
}

/// The cable tokens of a palette. `gutter` is the ground between panels (Slate's
/// `gutter`, the theme's deepest ground).
pub fn tokens(p: &Pal, gutter: Color32) -> Tokens {
    let n = p.dark;
    Tokens {
        out: p.accent,
        send: if n {
            mix(p.accent, p.ink1, 0.45)
        } else {
            mix(p.accent, p.base, 0.38)
        },
        key: if n {
            mix(p.solo, p.rec, 0.25)
        } else {
            mix(p.solo, Color32::from_rgb(0xff, 0x9a, 0x2e), 0.40)
        },
        refused: p.rec,
        channel: mix(p.base, gutter, 0.40),
        hole: if n { p.edge } else { mix(p.well, p.edge, 0.6) },
        ring: mix(p.ink4, p.ink3, 0.5),
        dark: n,
        edge: p.edge,
        ink1: p.ink1,
    }
}

impl Tokens {
    /// A kind's colour.
    pub fn of(&self, k: Kind) -> Color32 {
        match k {
            Kind::Output => self.out,
            Kind::Send => self.send,
            Kind::Key => self.key,
        }
    }

    /// The under-stroke: a darker shade of the cable's own colour.
    pub fn under(&self, col: Color32) -> Color32 {
        if self.dark {
            mix(col, self.edge, 0.55)
        } else {
            mix(col, self.ink1, 0.28)
        }
    }

    /// The gleam's ink.
    pub fn gleam(&self, col: Color32) -> Color32 {
        mix(col, Color32::WHITE, if self.dark { 0.6 } else { 0.7 })
    }

    /// D: a cord's darker edge (`jack.edge`).
    pub fn jack_edge(&self, col: Color32) -> Color32 {
        if self.dark {
            mix(col, self.edge, 0.55)
        } else {
            mix(col, self.ink1, 0.35)
        }
    }

    /// Whether the theme is dark (D's materials).
    pub fn dark(&self) -> bool {
        self.dark
    }
}

/// The gleam's speed (px/s) and opacity at a signal level 0…1.
pub fn gleam_speed(level: f32) -> f32 {
    40.0 + 80.0 * level.clamp(0.0, 1.0)
}
pub fn gleam_alpha(level: f32) -> f32 {
    0.35 + 0.65 * level.clamp(0.0, 1.0)
}

/// The gleam's offset along a cable at transport time `t` seconds: deterministic, so a
/// paused transport freezes it and costs nothing.
pub fn gleam_offset(t: f64, level: f32) -> f32 {
    (t * f64::from(gleam_speed(level))).rem_euclid(f64::from(PERIOD)) as f32
}

fn cubic(out: &mut Vec<Pos2>, p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) {
    for k in 0..=SEGS {
        let u = k as f32 / SEGS as f32;
        let w = 1.0 - u;
        let (a, b, c, d) = (w * w * w, 3.0 * w * w * u, 3.0 * w * u * u, u * u * u);
        out.push(pos2(
            a * p0.x + b * p1.x + c * p2.x + d * p3.x,
            a * p0.y + b * p1.y + c * p2.y + d * p3.y,
        ));
    }
}

/// A patch cable from jack `a` to jack `z` (the same row), sagging `k`: appended to
/// `out` as `SEGS + 1` points.
pub fn patch(out: &mut Vec<Pos2>, a: Pos2, z: Pos2, k: f32) {
    cubic(
        out,
        a,
        pos2(a.x, a.y + k),
        pos2(z.x, a.y + k),
        pos2(z.x, z.y),
    );
}

/// The cable being dragged: from jack `a` to the pointer `p` (P2 = (px, py + 0.55·k)).
pub fn dragged(out: &mut Vec<Pos2>, a: Pos2, p: Pos2, k: f32) {
    cubic(out, a, pos2(a.x, a.y + k), pos2(p.x, p.y + 0.55 * k), p);
}

/// A quarter circle from `from` round `c` to `to` (radius `r`), its inner points.
fn corner(out: &mut Vec<Pos2>, c: Pos2, a0: f32, a1: f32, r: f32) {
    const N: usize = 4;
    for k in 1..N {
        let a = a0 + (a1 - a0) * k as f32 / N as f32;
        out.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
}

/// A subway route from jack `a` down to the lane at `ly`, across, and up into jack `z`,
/// corners of radius 4 (less on a span too short for two).
pub fn subway(out: &mut Vec<Pos2>, a: Pos2, z: Pos2, ly: f32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let d = if z.x >= a.x { 1.0 } else { -1.0 };
    let r = CORNER.min((z.x - a.x).abs() * 0.5).min((ly - a.y).max(0.0));
    out.push(a);
    out.push(pos2(a.x, ly - r));
    // The first corner: centre (a.x + d·r, ly − r), from the left/right side to the bottom.
    let c1 = pos2(a.x + d * r, ly - r);
    let (s1, e1) = if d > 0.0 {
        (PI, FRAC_PI_2)
    } else {
        (0.0, FRAC_PI_2)
    };
    corner(out, c1, s1, e1, r);
    out.push(pos2(a.x + d * r, ly));
    out.push(pos2(z.x - d * r, ly));
    let c2 = pos2(z.x - d * r, ly - r);
    let (s2, e2) = if d > 0.0 {
        (FRAC_PI_2, 0.0)
    } else {
        (FRAC_PI_2, PI)
    };
    corner(out, c2, s2, e2, r);
    out.push(pos2(z.x, ly - r));
    out.push(z);
}

/// The subway's lanes: for each span (its kind and the x of its two jacks), its lane's
/// depth below the jacks. Inside a kind, the shortest span first takes the first sub-lane
/// no overlapping span (with [`MARGIN`]) holds; the kinds' bands follow one another, 8 px
/// apart at least, so no two cables whose spans overlap share a depth. `order` and `sub`
/// are scratch. Returns the deepest lane (0 with no span).
pub fn lanes(
    spans: &[(Kind, f32, f32)],
    order: &mut Vec<u32>,
    sub: &mut Vec<u16>,
    out: &mut Vec<f32>,
) -> f32 {
    order.clear();
    order.extend(0..spans.len() as u32);
    let len = |i: u32| {
        let (_, a, b) = spans[i as usize];
        (b - a).abs()
    };
    order.sort_unstable_by(|x, y| len(*x).total_cmp(&len(*y)).then(x.cmp(y)));
    sub.clear();
    sub.resize(spans.len(), u16::MAX);
    let mut count = [0u16; 3];
    for n in 0..order.len() {
        let i = order[n] as usize;
        let (kind, a, b) = spans[i];
        let (lo, hi) = (a.min(b), a.max(b));
        let mut k = 0u16;
        'next: loop {
            for &j in &order[..n] {
                let j = j as usize;
                let (kj, aj, bj) = spans[j];
                if kj == kind
                    && sub[j] == k
                    && !(hi + MARGIN < aj.min(bj) || lo > aj.max(bj) + MARGIN)
                {
                    k += 1;
                    continue 'next;
                }
            }
            break;
        }
        sub[i] = k;
        count[kind.rank()] = count[kind.rank()].max(k + 1);
    }
    // Each kind's band after the one above it: at its own depth, or 8 below the last
    // sub-lane above.
    let mut base = [0.0f32; 3];
    let mut floor = 0.0f32;
    for r in 0..3 {
        base[r] = LANE[r].max(floor);
        if count[r] > 0 {
            floor = base[r] + SUB * f32::from(count[r] - 1) + 2.0 * SUB;
        }
    }
    out.clear();
    let mut deepest = 0.0f32;
    for (i, (kind, _, _)) in spans.iter().enumerate() {
        let y = base[kind.rank()] + SUB * f32::from(sub[i]);
        deepest = deepest.max(y);
        out.push(y);
    }
    deepest
}

/// The distance from `p` to a polyline.
pub fn dist(pts: &[Pos2], p: Pos2) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let l2 = ab.length_sq();
        let u = if l2 > 1e-9 {
            ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min(p.distance(a + ab * u));
    }
    best
}

/// The point half way along a polyline (where a test points at a cable).
pub fn halfway(pts: &[Pos2]) -> Pos2 {
    let total: f32 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total * 0.5;
    for w in pts.windows(2) {
        let l = w[0].distance(w[1]);
        if l >= left && l > 0.0 {
            return w[0] + (w[1] - w[0]) * (left / l);
        }
        left -= l;
    }
    pts.last().copied().unwrap_or(Pos2::ZERO)
}

/// The lowest point of a polyline (where a level pill rides).
pub fn lowest(pts: &[Pos2]) -> Pos2 {
    pts.iter()
        .copied()
        .fold(Pos2::new(0.0, f32::NEG_INFINITY), |m, p| {
            if p.y > m.y {
                p
            } else {
                m
            }
        })
}

/// The gleam's dashes along a polyline: a `dash`-long dash ([`GLEAM`]; D's cords [`JACK_GLEAM`]) every [`PERIOD`], the first
/// starting `offset` along it, each piece handed to `f` (a dash across a join is two).
pub fn gleams_of(pts: &[Pos2], offset: f32, dash: f32, mut f: impl FnMut(Pos2, Pos2)) {
    let mut s0 = 0.0f32; // the arc length at the segment's start
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let l = a.distance(b);
        if l < 1e-5 {
            continue;
        }
        let s1 = s0 + l;
        // The dashes [start, start + GLEAM) with start = offset + n·PERIOD, n ≥ −1.
        let mut n = ((s0 - offset - dash) / PERIOD).floor();
        loop {
            let start = offset + n * PERIOD;
            if start >= s1 {
                break;
            }
            let (x0, x1) = (start.max(s0), (start + dash).min(s1));
            if x1 > x0 && start + dash > 0.0 {
                f(a + (b - a) * ((x0 - s0) / l), a + (b - a) * ((x1 - s0) / l));
            }
            n += 1.0;
        }
        s0 = s1;
    }
}

// ---- D, "Jack": real ¼-inch patch cables (README, "Cable style D") ---------------------

/// A seated plug hangs this far below its jack's nut (collar 4 + barrel 7 + band 1.5 +
/// boot 9.5).
pub const PLUG_LEN: f32 = 22.0;
/// A free plug, held while dragging, from its tip to its boot's end.
pub const FREE_LEN: f32 = 36.0;
/// The channel D's cords hang in: the plugs, then the deepest catenary (52) and its width.
pub const JACK_CHANNEL: f32 = 84.0;
/// The catenary's samples.
pub const CAT_SEGS: usize = 28;
/// How long a dropped cable settles, ms.
pub const SETTLE_MS: f32 = 300.0;
/// The gleam on D's cords: an 8-px dash every 64.
pub const JACK_GLEAM: f32 = 8.0;

/// D's sag for a span: s = min(52, 12 + 0.16·span).
pub fn jack_sag(span: f32) -> f32 {
    (12.0 + 0.16 * span.abs()).min(52.0)
}

/// D's free cable's k while dragging: min(60, 18 + 0.18·span).
pub fn jack_free_k(span: f32) -> f32 {
    (18.0 + 0.18 * span.abs()).min(60.0)
}

/// D's cord widths: output 3.2, send 2.2 + 1.2·level, key 2.8.
pub fn jack_width(kind: Kind, db: f32) -> f32 {
    match kind {
        Kind::Output => 3.2,
        Kind::Send => 2.2 + 1.2 * ((db + 36.0) / 36.0).clamp(0.0, 1.0),
        Kind::Key => 2.8,
    }
}

/// The sag `t_ms` after a drop: s0·(1 + 0.12·e^(−t/90)·cos(2πt/160)) — 12 % over, ringing
/// down — and exactly s0, still, from 300 ms on.
pub fn settle(s0: f32, t_ms: f32) -> f32 {
    if !(0.0..SETTLE_MS).contains(&t_ms) {
        return s0;
    }
    s0 * (1.0 + 0.12 * (-t_ms / 90.0).exp() * (std::f32::consts::TAU * t_ms / 160.0).cos())
}

/// The catenary's parameter a for a span `l` and a sag `s`: a·(cosh(l/2a) − 1) = s,
/// by 80 steps of bisection (the left side falls as a grows).
pub fn catenary_a(l: f32, s: f32) -> f32 {
    let (mut lo, mut hi) = (0.05f64, 1e5f64);
    let (l, s) = (f64::from(l.abs()), f64::from(s.max(1e-3)));
    for _ in 0..80 {
        let a = (lo + hi) * 0.5;
        let f = a * ((l / (2.0 * a)).min(60.0).cosh() - 1.0) - s;
        if f > 0.0 {
            lo = a;
        } else {
            hi = a;
        }
    }
    ((lo + hi) * 0.5) as f32
}

fn quad(out: &mut Vec<Pos2>, p0: Pos2, c: Pos2, p1: Pos2, n: usize) {
    for k in 1..=n {
        let u = k as f32 / n as f32;
        let w = 1.0 - u;
        out.push(pos2(
            w * w * p0.x + 2.0 * w * u * c.x + u * u * p1.x,
            w * w * p0.y + 2.0 * w * u * c.y + u * u * p1.y,
        ));
    }
}

/// A seated cord from boot end `a` to boot end `z` (the same row), sagging `s` with the
/// catenary's parameter `ca`: a quadratic leaves each boot straight down (control 7 below
/// it) to the catenary's third point, the catenary (28 samples, from 3 below the boots)
/// between. Appended to `out`.
pub fn jack_cord(out: &mut Vec<Pos2>, a: Pos2, z: Pos2, s: f32, ca: f32) {
    let yb = a.y + 3.0;
    let xm = (a.x + z.x) * 0.5;
    let at = |i: usize| {
        let x = a.x + (z.x - a.x) * i as f32 / CAT_SEGS as f32;
        pos2(x, yb + s - ca * (((x - xm) / ca).cosh() - 1.0))
    };
    out.push(a);
    quad(out, a, pos2(a.x, a.y + 7.0), at(2), 4);
    for i in 3..=CAT_SEGS - 2 {
        out.push(at(i));
    }
    quad(out, at(CAT_SEGS - 2), pos2(z.x, z.y + 7.0), z, 4);
}

/// The cord being dragged: a cubic with vertical tangents from boot end `a` to the free
/// plug's boot end `p` (P1 = a + (0, k), P2 = p + (0, 0.7·k)).
pub fn jack_dragged(out: &mut Vec<Pos2>, a: Pos2, p: Pos2) {
    let k = jack_free_k(p.x - a.x);
    cubic(out, a, pos2(a.x, a.y + k), pos2(p.x, p.y + 0.7 * k), p);
}

/// D's materials (README, "Shading layers"): the plug's metal and boot, the nut, the
/// hole and the shadow — a real object's colours, the same in every theme of a light.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Material {
    pub metal_hi: Color32,
    pub metal_mid: Color32,
    pub metal_lo: Color32,
    pub boot: Color32,
    pub boot_hi: Color32,
    pub nut: Color32,
    pub hole: Color32,
    pub shadow: Color32,
    /// The shadow's two passes' opacity (w + 3, w + 6).
    pub shadow_a: (u8, u8),
}

/// D's materials in a dark or a light theme.
pub fn material(dark: bool) -> Material {
    let c = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    if dark {
        Material {
            metal_hi: c(0xf1f4f7),
            metal_mid: c(0xa3adb7),
            metal_lo: c(0x4a535d),
            boot: c(0x1b2026),
            boot_hi: c(0x323a43),
            nut: c(0x5d6772),
            hole: c(0x0b0e11),
            shadow: c(0x000000),
            shadow_a: (77, 31),
        }
    } else {
        Material {
            metal_hi: c(0xf1f4f7),
            metal_mid: c(0xb4bdc6),
            metal_lo: c(0x6c7782),
            boot: c(0x2c353e),
            boot_hi: c(0x46515c),
            nut: c(0x9aa5b0),
            hole: c(0x0b0e11),
            shadow: c(0x1c2a38),
            shadow_a: (46, 18),
        }
    }
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// A plug as an SVG document `w × h` logical px drawn at `scale`, its axis at x = 4 and
/// its nut (seated) or tip (free) at y = 0 — the README's shapes, part for part: collar
/// 5 × 4, barrel 7 × 7 r 1 with three knurl lines, the kind band 7 × 1.5, the boot tapering
/// 6.6 → 4.2 with three ribs; a free plug adds tip, insulator, ring, insulator and sleeve
/// with its highlight. Rasterised once per (shape, colours, scale) into the layer atlas.
pub fn plug_svg(
    m: &Material,
    band: Color32,
    boot: Option<Color32>,
    free: bool,
    scale: f32,
) -> String {
    let (w, h) = plug_size(free);
    let (pw, ph) = ((w * scale).ceil(), (h * scale).ceil());
    let mut s = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{pw}' height='{ph}' viewBox='0 0 {} {}'>\
         <defs><linearGradient id='m' x1='0' x2='1' y1='0' y2='0'>\
         <stop offset='0' stop-color='{lo}'/><stop offset='0.32' stop-color='{hi}'/>\
         <stop offset='0.6' stop-color='{mid}'/><stop offset='1' stop-color='{lo}'/></linearGradient>\
         <linearGradient id='b' x1='0' x2='1' y1='0' y2='0'>\
         <stop offset='0' stop-color='{bt}'/><stop offset='0.35' stop-color='{bh}'/>\
         <stop offset='1' stop-color='{bt}'/></linearGradient></defs>",
        pw / scale,
        ph / scale,
        lo = hex(m.metal_lo),
        hi = hex(m.metal_hi),
        mid = hex(m.metal_mid),
        bt = hex(m.boot),
        bh = hex(m.boot_hi),
    );
    let cx = 4.0;
    let mut y = 0.0;
    if free {
        let dark = "#14181c";
        s += &format!(
            "<path d='M{a} 3Q{a} 0 {cx} 0Q{b} 0 {b} 3Z' fill='url(#m)'/>\
             <rect x='{a}' y='3' width='3.2' height='1' fill='{dark}'/>\
             <rect x='{c}' y='4' width='3.6' height='2.2' fill='url(#m)'/>\
             <rect x='{c}' y='6.2' width='3.6' height='1' fill='{dark}'/>\
             <rect x='{d}' y='7.2' width='3.8' height='8' fill='url(#m)'/>\
             <rect x='{e}' y='7.6' width='0.6' height='7' fill='#ffffff' opacity='0.6'/>",
            a = cx - 1.6,
            b = cx + 1.6,
            c = cx - 1.8,
            d = cx - 1.9,
            e = cx - 1.1,
        );
        y = 14.0;
    }
    s += &format!(
        "<rect x='{}' y='{}' width='5' height='4' fill='url(#m)'/>\
         <rect x='{}' y='{}' width='7' height='7' rx='1' fill='url(#m)'/>",
        cx - 2.5,
        y + 1.0,
        cx - 3.5,
        y + 5.0
    );
    for k in 0..3 {
        s += &format!(
            "<rect x='{}' y='{}' width='7' height='0.6' fill='{}' opacity='0.55'/>",
            cx - 3.5,
            y + 6.5 + 2.0 * k as f32,
            hex(m.metal_lo)
        );
    }
    s += &format!(
        "<rect x='{}' y='{}' width='7' height='1.5' fill='{}'/>",
        cx - 3.5,
        y + 12.0,
        hex(band)
    );
    let boot = boot.map_or_else(|| "url(#b)".to_string(), hex);
    s += &format!(
        "<path d='M{} {t}L{} {t}L{} {e}L{} {e}Z' fill='{boot}'/>",
        cx - 3.3,
        cx + 3.3,
        cx + 2.1,
        cx - 2.1,
        t = y + 13.5,
        e = y + PLUG_LEN,
    );
    for k in 0..3 {
        let hw = 3.1 - 0.35 * k as f32;
        s += &format!(
            "<rect x='{}' y='{}' width='{}' height='0.7' fill='#000000' opacity='0.35'/>",
            cx - hw,
            y + 15.5 + 2.2 * k as f32,
            2.0 * hw
        );
    }
    s + "</svg>"
}

/// A plug's canvas, logical px: 8 wide; a seated one 22 long, a free one 36.
pub fn plug_size(free: bool) -> (f32, f32) {
    (8.0, if free { FREE_LEN } else { PLUG_LEN })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    /// The README's numbers: the sag, the widths, the jacks.
    #[test]
    fn the_spec_numbers() {
        assert_eq!(sag(0.0), 16.0);
        assert_eq!(sag(200.0), 46.0);
        assert_eq!(sag(1000.0), 58.0);
        assert!((width(Kind::Send, -6.0) - 2.333).abs() < 0.01);
        assert!((width(Kind::Send, -22.0) - 1.622).abs() < 0.01);
        assert_eq!(width(Kind::Output, 0.0), 2.0);
        assert_eq!(width(Kind::Key, 0.0), 1.5);
        assert_eq!(jacks(100.0, 60.0), (89.0, 111.0));
        let (i, o) = jacks(100.0, 30.0);
        assert!(
            i - JACK_R >= 86.0 && o + JACK_R <= 114.0,
            "Compact: {i} {o}"
        );
    }

    /// The patch cable hangs from its jacks (vertical tangents) and its deepest point is
    /// 0.75·k below them.
    #[test]
    fn a_patch_cable_hangs_from_its_jacks() {
        let mut pts = Vec::new();
        let (a, z) = (pos2(100.0, 50.0), pos2(300.0, 50.0));
        let k = sag(z.x - a.x);
        patch(&mut pts, a, z, k);
        assert_eq!(pts.len(), SEGS + 1);
        assert_eq!((pts[0], pts[SEGS]), (a, z));
        assert!(
            (pts[1].x - a.x).abs() < 2.0 && pts[1].y - a.y > 2.0 * (pts[1].x - a.x).abs(),
            "down from the jack"
        );
        let low = lowest(&pts);
        assert!((low.y - (a.y + 0.75 * k)).abs() < 0.01, "{low:?} {k}");
        assert!(dist(&pts, low) < 1e-3, "{}", dist(&pts, low));
        assert!(
            dist(&pts, pos2(200.0, 0.0)) > 40.0,
            "{}",
            dist(&pts, pos2(200.0, 0.0))
        );
    }

    /// The subway: down, across at the lane, up — every run horizontal or vertical but for
    /// the r 4 corners, both ends at their jacks, the run at the lane.
    #[test]
    fn a_subway_route_is_orthogonal() {
        for (a, z) in [
            (pos2(100.0, 50.0), pos2(300.0, 50.0)),
            (pos2(300.0, 50.0), pos2(120.0, 50.0)),
            (pos2(100.0, 50.0), pos2(103.0, 50.0)),
        ] {
            let mut pts = Vec::new();
            subway(&mut pts, a, z, 90.0);
            assert_eq!((pts[0], *pts.last().unwrap()), (a, z));
            assert!(pts.iter().all(|p| p.y <= 90.0 + 1e-3 && p.y >= 50.0 - 1e-3));
            assert!(pts.iter().any(|p| (p.y - 90.0).abs() < 1e-3));
            let lo = a.x.min(z.x) - 1e-3;
            let hi = a.x.max(z.x) + 1e-3;
            assert!(pts.iter().all(|p| p.x >= lo && p.x <= hi), "{pts:?}");
        }
    }

    /// C's promise, held over random desks (2–64 strips, outputs, sends and keys, shared
    /// jacks, nested and equal spans): two cables whose spans overlap never share a depth —
    /// their runs are at least a sub-lane apart — and a kind's lanes never reach into the
    /// next kind's.
    #[test]
    fn the_subways_lanes_never_overlap() {
        let (mut order, mut sub, mut out) = (Vec::new(), Vec::new(), Vec::new());
        for seed in 1..400u64 {
            let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
            let n = 2 + r.below(63) as usize;
            let pitch = [32.0, 62.0, 96.0][r.below(3) as usize];
            let x = |i: usize| 40.0 + i as f32 * pitch;
            let m = r.below(3 * n as u64 + 1) as usize;
            let mut spans = Vec::new();
            for _ in 0..m {
                let a = r.below(n as u64) as usize;
                let mut b = r.below(n as u64 + 1) as usize; // n: the master, at the left
                if b == a {
                    b = n;
                }
                let kind = [Kind::Output, Kind::Send, Kind::Send, Kind::Key][r.below(4) as usize];
                let (ox, ix) = (
                    jacks(x(a), pitch).1,
                    if b == n { 11.0 } else { jacks(x(b), pitch).0 },
                );
                spans.push((kind, ox, ix));
            }
            let deepest = lanes(&spans, &mut order, &mut sub, &mut out);
            assert_eq!(out.len(), spans.len());
            for i in 0..spans.len() {
                assert!(
                    out[i] >= LANE[spans[i].0.rank()],
                    "a kind keeps its own depth"
                );
                assert!(out[i] <= deepest);
                for j in i + 1..spans.len() {
                    let (ki, ai, bi) = spans[i];
                    let (kj, aj, bj) = spans[j];
                    let overlap =
                        !(ai.max(bi) + MARGIN < aj.min(bj) || ai.min(bi) > aj.max(bj) + MARGIN);
                    if overlap {
                        assert!(
                            (out[i] - out[j]).abs() >= SUB - 1e-3,
                            "seed {seed}: {:?} at {} and {:?} at {}",
                            spans[i],
                            out[i],
                            spans[j],
                            out[j]
                        );
                    }
                    if ki.rank() < kj.rank() {
                        assert!(out[i] < out[j], "seed {seed}: kinds in order");
                    } else if kj.rank() < ki.rank() {
                        assert!(out[j] < out[i], "seed {seed}: kinds in order");
                    }
                }
            }
        }
    }

    /// The gleam: a dash every 64 px whose place moves with the transport's time toward
    /// the target, and stands still when the time does.
    #[test]
    fn the_gleam_travels_with_the_transport() {
        let pts = [pos2(0.0, 0.0), pos2(200.0, 0.0)];
        let starts = |off: f32| {
            let mut v = Vec::new();
            gleams_of(&pts, off, GLEAM, |a, _| v.push(a.x));
            v
        };
        let at = |t: f64| starts(gleam_offset(t, 0.5));
        assert_eq!(at(1.0), at(1.0), "deterministic");
        let (a, b) = (at(1.0), at(1.05));
        assert!(!a.is_empty() && a != b, "it moves while time runs");
        assert!(
            b[b.len() - 1] >= a[a.len() - 1] || b.len() != a.len(),
            "toward the target"
        );
        let mut total = 0.0;
        gleams_of(&pts, 10.0, GLEAM, |a, z| total += a.distance(z));
        assert!((total - 3.0 * GLEAM).abs() < 1e-3, "{total}");
        assert!(gleam_speed(1.0) == 120.0 && gleam_speed(0.0) == 40.0);
        assert!((gleam_alpha(0.0) - 0.35).abs() < 1e-6 && gleam_alpha(1.0) == 1.0);
    }
}
