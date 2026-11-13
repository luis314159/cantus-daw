//! The pedals' artwork (Fragor, ADR 0131 — reusable by any pedal drawn with `pedal_draw`):
//! surface textures, large illustrations that own the face, ornamental borders and a
//! lettering style per pedal. Original to Cantus — no product's graphics, logos or
//! lettering — and all vectors in fine strokes, crisp from 75 % to 200 %. Deterministic:
//! every scattered mark is placed by a hash, so the face is the same each frame.

use crate::kit::color::with_alpha;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// A hash in 0 … 1, for scattering marks.
fn h(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// The paint's surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Texture {
    Smooth,
    /// Metal flake: fine bright specks.
    Sparkle,
    /// Hammertone: shallow dimples, each lit on one side.
    Hammer,
    /// Crinkle: short wrinkles.
    Crinkle,
    /// Brushed metal: hairlines.
    Brushed,
}

/// Lay `t` over the face `r` (`u` the enclosure's scale), in the paint's lighter `hi` and
/// darker `lo`.
pub fn texture(b: &mut Brush, u: f32, r: Rect, t: Texture, hi: Color32, lo: Color32, seed: u32) {
    let inner = r.shrink(5.0 * u);
    let at = |i: u32| {
        pos2(
            inner.left() + inner.width() * h(seed + i * 2),
            inner.top() + inner.height() * h(seed + i * 2 + 1),
        )
    };
    match t {
        Texture::Smooth => {}
        Texture::Sparkle => {
            for i in 0..220 {
                let p = at(i);
                let a = 0.25 + 0.55 * h(seed + 900 + i);
                b.circle(p, 0.55 * u, Fill::Solid(with_alpha(hi, a)));
            }
        }
        Texture::Hammer => {
            for i in 0..90 {
                let p = at(i);
                let rad = (3.0 + 4.0 * h(seed + 700 + i)) * u;
                b.arc(
                    p,
                    rad,
                    PI * 0.9,
                    PI * 1.6,
                    0.7 * u,
                    with_alpha(hi, 0.16),
                    false,
                );
                b.arc(p, rad, -0.1, PI * 0.6, 0.7 * u, with_alpha(lo, 0.22), false);
            }
        }
        Texture::Crinkle => {
            for i in 0..160 {
                let p = at(i);
                let a = TAU * h(seed + 500 + i);
                let d = vec2(a.cos(), a.sin()) * 3.0 * u;
                let n = vec2(-d.y, d.x) * 0.35;
                b.polyline(
                    &[p - d, p - d * 0.3 + n, p + d * 0.3 - n, p + d],
                    0.6 * u,
                    with_alpha(lo, 0.28),
                );
            }
        }
        Texture::Brushed => {
            let mut y = inner.top();
            let mut i = 0;
            while y < inner.bottom() {
                let a = 0.05 + 0.08 * h(seed + i);
                b.hline(inner.left(), inner.right(), y, 0.5 * u, with_alpha(hi, a));
                y += 1.6 * u;
                i += 1;
            }
        }
    }
}

/// The illustration printed on a face.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Motif {
    Plain,
    /// A deco sunrise: rays and arcs behind three sines bent into soft clipping.
    Waves,
    /// A schematic: an op-amp with its feedback loop, diodes to ground, on a crimson band.
    Circuit,
    /// A lightning bolt of square waves over a crystal lattice.
    Square,
    /// A sunburst with a transistor's emblem and its rings.
    Burst,
    /// Nested triangles over a halftone, crossed by the scooped middle's curve.
    Nested,
}

fn sine_path(r: Rect, cycles: f32, f: impl Fn(f32) -> f32, out: &mut [Pos2]) {
    let n = out.len() - 1;
    for (j, p) in out.iter_mut().enumerate() {
        let x = j as f32 / n as f32;
        let y = f((TAU * cycles * x).sin());
        *p = pos2(
            r.left() + r.width() * x,
            r.center().y - y * r.height() * 0.5,
        );
    }
}

fn zigzag(b: &mut Brush, a: Pos2, z: Pos2, w: f32, color: Color32) {
    let d = z - a;
    let n = vec2(-d.y, d.x).normalized() * 3.0;
    let mut pts = [Pos2::ZERO; 10];
    for (i, p) in pts.iter_mut().enumerate() {
        let t = i as f32 / 9.0;
        let off = if i == 0 || i == 9 {
            0.0
        } else if i % 2 == 1 {
            1.0
        } else {
            -1.0
        };
        *p = a + d * t + n * off * (d.length() / 30.0).min(1.0);
    }
    b.polyline(&pts, w, color);
}

/// Paint `m` over the art area `r` (`u` the enclosure's scale) in `print` with `accent`.
pub fn motif(b: &mut Brush, u: f32, r: Rect, m: Motif, print: Color32, accent: Color32) {
    let fine = 0.8 * u;
    let c0 = r.center();
    let mut pts = [Pos2::ZERO; 97];
    match m {
        Motif::Plain => {}
        Motif::Waves => {
            // The sun: rays fanning up from the horizon, arcs over it.
            let o = pos2(c0.x, r.top() + r.height() * 0.62);
            for i in 0..=18 {
                let a = PI + PI * i as f32 / 18.0;
                let d = vec2(a.cos(), a.sin());
                b.segment(
                    o + d * 18.0 * u,
                    o + d * r.width() * 0.62,
                    fine * 0.8,
                    with_alpha(accent, 0.30),
                    false,
                );
            }
            for (k, rad) in [16.0f32, 24.0, 32.0].iter().enumerate() {
                b.arc(
                    o,
                    rad * u,
                    PI,
                    TAU,
                    fine,
                    with_alpha(accent, 0.75 - 0.15 * k as f32),
                    false,
                );
            }
            b.hline(r.left(), r.right(), o.y, fine, with_alpha(print, 0.5));
            // The three waves across it.
            let wr = Rect::from_center_size(
                pos2(c0.x, r.top() + r.height() * 0.30),
                vec2(r.width(), r.height() * 0.42),
            );
            for (i, g) in [1.0f32, 3.0, 9.0].iter().enumerate() {
                sine_path(wr, 1.5, |s| (g * s).tanh() / g.tanh(), &mut pts);
                let (w, col) = match i {
                    2 => (1.5 * u, accent),
                    1 => (fine, with_alpha(print, 0.8)),
                    _ => (fine, with_alpha(print, 0.45)),
                };
                b.polyline(&pts, w, col);
            }
            // Hatching under the horizon.
            let mut x = r.left();
            while x < r.right() {
                b.segment(
                    pos2(x, o.y + 3.0 * u),
                    pos2(x + 6.0 * u, r.bottom()),
                    fine * 0.7,
                    with_alpha(print, 0.18),
                    false,
                );
                x += 5.0 * u;
            }
        }
        Motif::Circuit => {
            let band = [
                pos2(r.left() + r.width() * 0.62, r.top()),
                pos2(r.right(), r.top()),
                pos2(r.right(), r.top() + r.height() * 0.18),
                pos2(r.left() + r.width() * 0.38, r.bottom()),
                pos2(r.left(), r.bottom()),
                pos2(r.left(), r.top() + r.height() * 0.82),
            ];
            b.polygon(&band, with_alpha(accent, 0.85));
            let hh = r.height() * 0.42;
            let tri_c = pos2(c0.x - 6.0 * u, c0.y - 4.0 * u);
            let a = pos2(tri_c.x - hh * 0.5, tri_c.y - hh * 0.5);
            let z = pos2(tri_c.x - hh * 0.5, tri_c.y + hh * 0.5);
            let out = pos2(tri_c.x + hh * 0.5, tri_c.y);
            b.polyline(&[a, z, out, a], 1.3 * u, print);
            for (dy, s) in [(-0.22f32, "−"), (0.22, "+")] {
                let y = tri_c.y + hh * dy;
                b.segment(
                    pos2(r.left() + 6.0 * u, y),
                    pos2(a.x, y),
                    fine,
                    print,
                    false,
                );
                let mark = pos2(a.x + 6.0 * u, y);
                b.segment(
                    mark - vec2(2.5 * u, 0.0),
                    mark + vec2(2.5 * u, 0.0),
                    fine,
                    print,
                    false,
                );
                if s == "+" {
                    b.segment(
                        mark - vec2(0.0, 2.5 * u),
                        mark + vec2(0.0, 2.5 * u),
                        fine,
                        print,
                        false,
                    );
                }
                b.circle(pos2(r.left() + 6.0 * u, y), 1.4 * u, Fill::Solid(print));
            }
            // The feedback loop: a resistor over the triangle, back to the − input.
            let top = tri_c.y - hh * 0.5 - 10.0 * u;
            let back = pos2(a.x - 8.0 * u, tri_c.y - hh * 0.22);
            b.polyline(&[back, pos2(back.x, top)], fine, print);
            zigzag(
                b,
                pos2(back.x + 4.0 * u, top),
                pos2(out.x + 6.0 * u, top),
                fine,
                print,
            );
            b.polyline(
                &[pos2(back.x, top), pos2(back.x + 4.0 * u, top)],
                fine,
                print,
            );
            b.polyline(
                &[
                    pos2(out.x + 6.0 * u, top),
                    pos2(out.x + 10.0 * u, top),
                    pos2(out.x + 10.0 * u, out.y),
                ],
                fine,
                print,
            );
            b.circle(back, 1.4 * u, Fill::Solid(print));
            // The output and the two diodes to ground.
            let ox = out.x + 22.0 * u;
            b.segment(out, pos2(r.right() - 6.0 * u, out.y), fine, print, false);
            b.circle(pos2(out.x + 10.0 * u, out.y), 1.4 * u, Fill::Solid(print));
            for (k, up) in [(0.0f32, true), (12.0, false)] {
                let x = ox + k * u;
                let (y0, y1) = (out.y, out.y + 26.0 * u);
                b.segment(pos2(x, y0), pos2(x, y1), fine, print, false);
                let m = out.y + 13.0 * u;
                let (tip, base) = if up {
                    (m - 4.0 * u, m + 4.0 * u)
                } else {
                    (m + 4.0 * u, m - 4.0 * u)
                };
                b.polygon(
                    &[
                        pos2(x - 4.5 * u, base),
                        pos2(x + 4.5 * u, base),
                        pos2(x, tip),
                    ],
                    print,
                );
                b.segment(
                    pos2(x - 4.5 * u, tip),
                    pos2(x + 4.5 * u, tip),
                    fine,
                    print,
                    false,
                );
            }
            let g = pos2(ox + 6.0 * u, out.y + 26.0 * u);
            b.segment(pos2(ox, g.y), pos2(ox + 12.0 * u, g.y), fine, print, false);
            for (i, w) in [9.0f32, 6.0, 3.0].iter().enumerate() {
                let y = g.y + 3.0 * u + i as f32 * 2.5 * u;
                b.segment(
                    pos2(g.x - w * u, y),
                    pos2(g.x + w * u, y),
                    fine,
                    print,
                    false,
                );
            }
        }
        Motif::Square => {
            // The lattice behind: fine hexagons.
            let s = 10.0 * u;
            let hgt = s * 0.866;
            let mut row = 0;
            let mut y = r.top() + hgt * 0.5;
            while y < r.bottom() {
                let off = if row % 2 == 1 { s * 0.75 } else { 0.0 };
                let mut x = r.left() + off;
                while x < r.right() {
                    let mut hex = [Pos2::ZERO; 7];
                    for (i, p) in hex.iter_mut().enumerate() {
                        let a = PI / 3.0 * i as f32;
                        *p = pos2(x + a.cos() * s * 0.5, y + a.sin() * s * 0.5);
                    }
                    b.polyline(&hex, fine * 0.7, with_alpha(print, 0.22));
                    x += s * 1.5;
                }
                y += hgt;
                row += 1;
            }
            // The bolt: a hard-clipped wave falling across the face.
            let bolt = [
                pos2(r.left() + r.width() * 0.62, r.top() + 2.0 * u),
                pos2(r.left() + r.width() * 0.34, r.top() + r.height() * 0.48),
                pos2(r.left() + r.width() * 0.56, r.top() + r.height() * 0.48),
                pos2(r.left() + r.width() * 0.30, r.bottom() - 2.0 * u),
                pos2(r.left() + r.width() * 0.70, r.top() + r.height() * 0.40),
                pos2(r.left() + r.width() * 0.48, r.top() + r.height() * 0.40),
            ];
            b.polygon(&bolt, with_alpha(accent, 0.92));
            let mut outline = bolt.to_vec();
            outline.push(bolt[0]);
            b.polyline(&outline, fine, print);
            let sq = Rect::from_center_size(c0, vec2(r.width(), r.height() * 0.30));
            sine_path(sq, 2.5, |s| (6.0 * s).clamp(-1.0, 1.0), &mut pts);
            b.polyline(&pts, 1.2 * u, print);
        }
        Motif::Burst => {
            let rad = r.height() * 0.30;
            for k in 0..48 {
                let a = TAU * k as f32 / 48.0;
                let d = vec2(a.cos(), a.sin());
                let len = if k % 4 == 0 {
                    2.1
                } else if k % 2 == 0 {
                    1.75
                } else {
                    1.5
                };
                b.segment(
                    c0 + d * rad * 1.18,
                    c0 + d * rad * len,
                    fine,
                    with_alpha(accent, 0.85),
                    false,
                );
            }
            for (k, s) in [1.0f32, 1.1].iter().enumerate() {
                b.circle_stroke(c0, rad * s, if k == 0 { 1.3 * u } else { fine }, print);
            }
            // Dots round the ring, like a dial's.
            for k in 0..24 {
                let a = TAU * k as f32 / 24.0;
                b.circle(
                    c0 + vec2(a.cos(), a.sin()) * rad * 0.88,
                    0.8 * u,
                    Fill::Solid(with_alpha(print, 0.7)),
                );
            }
            let (bx, top, bot) = (c0.x - rad * 0.22, c0.y - rad * 0.45, c0.y + rad * 0.45);
            b.segment(pos2(bx, top), pos2(bx, bot), 1.8 * u, print, false);
            b.segment(
                pos2(c0.x - rad * 0.8, c0.y),
                pos2(bx, c0.y),
                fine,
                print,
                false,
            );
            let col = pos2(c0.x + rad * 0.32, c0.y - rad * 0.7);
            let emi = pos2(c0.x + rad * 0.32, c0.y + rad * 0.7);
            b.segment(pos2(bx, c0.y - rad * 0.18), col, 1.1 * u, print, false);
            b.segment(pos2(bx, c0.y + rad * 0.18), emi, 1.1 * u, print, false);
            let dir = (emi - pos2(bx, c0.y + rad * 0.18)).normalized();
            let side = vec2(-dir.y, dir.x);
            b.polygon(
                &[
                    emi,
                    emi - dir * 6.0 * u + side * 3.0 * u,
                    emi - dir * 6.0 * u - side * 3.0 * u,
                ],
                print,
            );
        }
        Motif::Nested => {
            // A halftone rising to the apex.
            let step = 5.0 * u;
            let mut y = r.top() + step;
            while y < r.bottom() {
                let t = (y - r.top()) / r.height();
                let mut x = r.left() + step * 0.5;
                while x < r.right() {
                    let rad = (0.3 + 1.4 * t) * u;
                    b.circle(pos2(x, y), rad, Fill::Solid(with_alpha(accent, 0.18)));
                    x += step;
                }
                y += step;
            }
            for (k, s) in [1.0f32, 0.82, 0.64, 0.46, 0.28].iter().enumerate() {
                let hh = r.height() * 0.92 * s;
                let w = hh * 1.15;
                let base = r.top() + r.height() * 0.5 + r.height() * 0.42 * 0.92;
                let tri = [
                    pos2(c0.x, base - hh),
                    pos2(c0.x + w * 0.5, base),
                    pos2(c0.x - w * 0.5, base),
                    pos2(c0.x, base - hh),
                ];
                let col = if k == 0 {
                    print
                } else {
                    with_alpha(print, 0.75 - 0.1 * k as f32)
                };
                b.polyline(&tri, if k == 0 { 1.3 * u } else { fine }, col);
            }
            let cr = Rect::from_center_size(
                pos2(c0.x, c0.y + 4.0 * u),
                vec2(r.width(), r.height() * 0.45),
            );
            for (j, p) in pts.iter_mut().enumerate() {
                let x = j as f32 / 96.0;
                let y = 0.55 - 1.1 * (PI * x).sin();
                *p = pos2(
                    cr.left() + cr.width() * x,
                    cr.center().y - y * cr.height() * 0.5,
                );
            }
            b.polyline(&pts, 1.5 * u, accent);
        }
    }
}

/// How a pedal's name is lettered.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Letter {
    /// Capitals between two fine rules (art deco).
    Deco,
    /// Technical capitals in brackets, as on a drawing.
    Technical,
    /// Heavy capitals over a bar.
    Bold,
    /// Upper and lower case over a swash.
    Script,
    /// Wide capitals between two small triangles.
    Wide,
}

/// The name lettered in `r` in style `l`, on a plate of `ground` where the art is busy.
#[allow(clippy::too_many_arguments)]
pub fn lettering(
    b: &mut Brush,
    k: f32,
    u: f32,
    r: Rect,
    name: Src<'_>,
    l: Letter,
    print: Color32,
    accent: Color32,
    ground: Color32,
) {
    let (role, caps) = match l {
        Letter::Deco => (Role::Title, true),
        Letter::Technical => (Role::Value, true),
        Letter::Bold => (Role::Hero, true),
        Letter::Script => (Role::Title, false),
        Letter::Wide => (Role::Section, true),
    };
    let t = Text {
        src: name,
        role,
        caps,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: print,
    };
    let inner = r.shrink2(vec2(18.0 * u, 0.0));
    // Never elided: the pedal's own cut if it fits, a smaller one if not, and on two lines
    // (split at a space) if even that is too wide — a small pedal sets its name smaller.
    let fits = |t: &Text<'_>| ktext::measure(b.ctx(), t, k).x <= inner.width();
    let mut one = t;
    let mut two: Option<[String; 2]> = None;
    if !fits(&one) {
        one.role = Role::Section;
        if !fits(&one) {
            one.role = Role::Label;
            if !fits(&one) {
                let words = match name {
                    Src::Key(key) => crate::i18n::t(key).to_string(),
                    Src::Str(s) => s.to_string(),
                };
                let words = if caps { words.to_uppercase() } else { words };
                let mid = words.len() / 2;
                let cut = words
                    .char_indices()
                    .filter(|(_, ch)| *ch == ' ')
                    .map(|(i, _)| i)
                    .min_by_key(|i| (*i as isize - mid as isize).abs());
                if let Some(c) = cut {
                    two = Some([words[..c].to_string(), words[c + 1..].to_string()]);
                }
            }
        }
    }
    let (w, lines_h) = match &two {
        Some(l) => {
            let m = |x: &str| {
                ktext::measure(
                    b.ctx(),
                    &Text {
                        src: Src::Str(x),
                        ..one
                    },
                    k,
                )
            };
            let (a, c) = (m(&l[0]), m(&l[1]));
            (a.x.max(c.x), a.y + c.y)
        }
        None => (
            ktext::measure(b.ctx(), &one, k).x.min(inner.width()),
            r.height(),
        ),
    };
    let r = if two.is_some() {
        Rect::from_center_size(
            r.center() - vec2(0.0, (lines_h - r.height()) * 0.5),
            vec2(r.width(), lines_h),
        )
    } else {
        r
    };
    let (x0, x1) = (r.center().x - w * 0.5, r.center().x + w * 0.5);
    // A quiet plate of the paint behind the words, so the art never fights them.
    let plate = Rect::from_min_max(
        pos2(x0 - 10.0 * u, r.top()),
        pos2(x1 + 10.0 * u, r.bottom()),
    );
    b.rrect(plate, 3.0 * u, Fill::Solid(with_alpha(ground, 0.82)));
    match &two {
        Some(l) => {
            let half = r.height() * 0.5;
            for (i, line) in l.iter().enumerate() {
                let lr = Rect::from_min_size(
                    pos2(r.left(), r.top() + half * i as f32),
                    vec2(r.width(), half),
                );
                ktext::paint(
                    b,
                    lr,
                    &Text {
                        src: Src::Str(line),
                        caps: false,
                        ..one
                    },
                    k,
                );
            }
        }
        None => {
            ktext::paint(b, inner, &one, k);
        }
    }
    let fine = 0.8 * u;
    match l {
        Letter::Deco => {
            for dy in [-1.0f32, 1.0] {
                let y = r.center().y + dy * (r.height() * 0.5 - 1.0 * u);
                b.hline(x0 - 8.0 * u, x1 + 8.0 * u, y, fine, with_alpha(print, 0.85));
                b.hline(x0, x1, y - dy * 2.0 * u, fine, accent);
                b.circle(pos2(x0 - 10.0 * u, y), 1.2 * u, Fill::Solid(accent));
                b.circle(pos2(x1 + 10.0 * u, y), 1.2 * u, Fill::Solid(accent));
            }
        }
        Letter::Technical => {
            for (x, s) in [(x0 - 8.0 * u, 1.0f32), (x1 + 8.0 * u, -1.0)] {
                let (t, bt) = (r.top() + 2.0 * u, r.bottom() - 2.0 * u);
                b.polyline(
                    &[
                        pos2(x + s * 5.0 * u, t),
                        pos2(x, t),
                        pos2(x, bt),
                        pos2(x + s * 5.0 * u, bt),
                    ],
                    fine,
                    print,
                );
            }
            b.hline(
                x0,
                x1,
                r.bottom() - 1.0 * u,
                fine * 0.7,
                with_alpha(print, 0.5),
            );
        }
        Letter::Bold => {
            b.rect(
                Rect::from_min_max(
                    pos2(x0, r.bottom() - 1.5 * u),
                    pos2(x1, r.bottom() + 1.0 * u),
                ),
                Fill::Solid(accent),
            );
        }
        Letter::Script => {
            let mut pts = [Pos2::ZERO; 33];
            for (j, p) in pts.iter_mut().enumerate() {
                let x = j as f32 / 32.0;
                let y = r.bottom() - 1.0 * u + (PI * x).sin() * 3.5 * u
                    - (FRAC_PI_2 * x).sin() * 1.5 * u;
                *p = pos2(x0 - 6.0 * u + (w + 12.0 * u) * x, y);
            }
            b.polyline(&pts, 1.1 * u, accent);
        }
        Letter::Wide => {
            for (x, s) in [(x0 - 10.0 * u, 1.0f32), (x1 + 10.0 * u, -1.0)] {
                let y = r.center().y;
                b.polygon(
                    &[
                        pos2(x - s * 4.0 * u, y - 4.0 * u),
                        pos2(x + s * 3.0 * u, y),
                        pos2(x - s * 4.0 * u, y + 4.0 * u),
                    ],
                    accent,
                );
            }
        }
    }
}

/// A border round the face, of the motif's family.
pub fn border(b: &mut Brush, u: f32, r: Rect, m: Motif, print: Color32, accent: Color32) {
    let fine = 0.7 * u;
    let inset = r.shrink(8.0 * u);
    match m {
        Motif::Plain => {}
        Motif::Waves => {
            b.rrect_stroke(inset, 8.0 * u, fine, with_alpha(print, 0.4));
            b.rrect_stroke(
                inset.shrink(3.0 * u),
                6.0 * u,
                fine,
                with_alpha(accent, 0.55),
            );
            for (cx, cy, sx, sy) in [
                (inset.left(), inset.top(), 1.0f32, 1.0f32),
                (inset.right(), inset.top(), -1.0, 1.0),
                (inset.left(), inset.bottom(), 1.0, -1.0),
                (inset.right(), inset.bottom(), -1.0, -1.0),
            ] {
                let o = pos2(cx + sx * 3.0 * u, cy + sy * 3.0 * u);
                for i in 0..6 {
                    let a = FRAC_PI_2 * i as f32 / 5.0;
                    let d = vec2(a.cos() * sx, a.sin() * sy);
                    b.segment(o, o + d * 14.0 * u, fine, with_alpha(accent, 0.55), false);
                }
            }
        }
        Motif::Circuit => {
            b.rrect_stroke(inset, 3.0 * u, fine, with_alpha(print, 0.35));
            for i in 0..=8 {
                let x = inset.left() + inset.width() * i as f32 / 8.0;
                b.vline(
                    x,
                    inset.bottom() - 3.0 * u,
                    inset.bottom(),
                    fine,
                    with_alpha(print, 0.4),
                );
            }
        }
        Motif::Square => {
            b.rrect_stroke(inset, 2.0 * u, fine, with_alpha(print, 0.35));
            for (cx, cy) in [
                (inset.left(), inset.top()),
                (inset.right(), inset.top()),
                (inset.left(), inset.bottom()),
                (inset.right(), inset.bottom()),
            ] {
                b.rect(
                    Rect::from_center_size(pos2(cx, cy), vec2(5.0, 5.0) * u),
                    Fill::Solid(with_alpha(accent, 0.8)),
                );
            }
        }
        Motif::Burst => {
            let n = 15;
            for side in 0..2 {
                let y = if side == 0 {
                    inset.top()
                } else {
                    inset.bottom()
                };
                for i in 0..n {
                    let x = inset.left() + inset.width() * (i as f32 + 0.5) / n as f32;
                    b.circle_stroke(
                        pos2(x, y),
                        inset.width() / n as f32 * 0.5,
                        fine,
                        with_alpha(accent, 0.5),
                    );
                }
            }
            b.vline(
                inset.left(),
                inset.top(),
                inset.bottom(),
                fine,
                with_alpha(accent, 0.5),
            );
            b.vline(
                inset.right(),
                inset.top(),
                inset.bottom(),
                fine,
                with_alpha(accent, 0.5),
            );
        }
        Motif::Nested => {
            for (y, dir) in [(inset.top(), 1.0f32), (inset.bottom(), -1.0)] {
                let mut pts = [Pos2::ZERO; 37];
                for (i, p) in pts.iter_mut().enumerate() {
                    let x = inset.left() + inset.width() * i as f32 / 36.0;
                    let up = if i % 2 == 0 { 0.0 } else { 5.0 * u };
                    *p = pos2(x, y + dir * up);
                }
                b.polyline(&pts, fine, with_alpha(accent, 0.7));
            }
        }
    }
}
