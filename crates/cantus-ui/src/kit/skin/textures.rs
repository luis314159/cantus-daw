//! Seamless tiles for the materials' and the displays' fine textures (round 32, ADR 0074):
//! brushed grain, vinyl amp covering's pebbles, carbon weave, paper fibre, wood grain, a CRT's scan lines,
//! a VFD's mesh.
//!
//! Each tile is computed once, procedurally, at the screen's own pixels (one texel per
//! physical pixel, so it is crisp at every window scale and every display density) into
//! the kit's layer atlas, and laid over a surface as a grid of quads. A tile holds only
//! light and shade — white or black, by alpha — so one tile serves every colour: the
//! light is tinted by the theme's `light` token where it is drawn.

use super::super::brush::Brush;
use super::super::cache;
use egui::{Color32, Pos2, Rect, Vec2};
use std::f32::consts::TAU;

/// A texture.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Tile {
    Brushed,
    Tolex,
    Carbon,
    Paper,
    Wood,
    /// Wood with paper's fibre in it, a fifth as strong: the wood material's two tiles as
    /// one, so its surfaces lay one grid of quads.
    Varnished,
    /// A CRT's scan lines.
    Scan,
    /// A vacuum-fluorescent display's mesh.
    Mesh,
}

/// The tile's size in texels (physical pixels).
fn size(t: Tile) -> (usize, usize) {
    match t {
        Tile::Wood | Tile::Varnished => (256, 128),
        _ => (128, 128),
    }
}

/// A hash of a lattice point to 0…1.
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((y as u32).wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise that repeats every `px` × `py` lattice cells, at (x, y) in cells.
fn noise(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let at = |i: i32, j: i32| hash(i.rem_euclid(px), j.rem_euclid(py), seed);
    let a = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * fx;
    let b = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * fx;
    a + (b - a) * fy
}

/// The tile's light (> 0) or shade (< 0) at texel (x, y), −1…1.
fn texel(t: Tile, x: usize, y: usize, w: usize, h: usize) -> f32 {
    let (xf, yf) = (x as f32, y as f32);
    match t {
        Tile::Varnished => {
            let (pw, ph) = size(Tile::Paper);
            texel(Tile::Wood, x, y, w, h) + 0.2 * texel(Tile::Paper, x % pw, y % ph, pw, ph)
        }
        Tile::Brushed => {
            // Each row a streak of its own, varying slowly along it.
            let row = hash(0, y as i32, 11) * 2.0 - 1.0;
            let along = noise(xf / 16.0, yf * 0.5, (w / 16) as i32, h as i32, 12) * 2.0 - 1.0;
            let fine = hash(x as i32, y as i32, 13) * 2.0 - 1.0;
            0.55 * row + 0.30 * along * row.abs().max(0.3) + 0.15 * fine
        }
        Tile::Tolex => {
            // Pebbles: two scales of noise, lit from the top left.
            // Lattices that divide the tile, so it repeats exactly.
            let hgt = |x: f32, y: f32| {
                0.45 * noise(x / 4.0, y / 4.0, (w / 4) as i32, (h / 4) as i32, 21)
                    + 0.55 * noise(x / 2.0, y / 2.0, (w / 2) as i32, (h / 2) as i32, 22)
            };
            let wrap = |v: f32, n: usize| v.rem_euclid(n as f32);
            let a = hgt(xf, yf);
            let b = hgt(wrap(xf + 1.0, w), wrap(yf + 1.0, h));
            ((a - b) * 6.0).clamp(-1.0, 1.0)
        }
        Tile::Carbon => {
            // A 2/2 twill: tows 8 texels wide, over two and under two, each lit across.
            let (cx, cy) = (x / 8, y / 8);
            let (fx, fy) = ((x % 8) as f32 / 8.0, (y % 8) as f32 / 8.0);
            let across = if (cx + cy) % 4 < 2 { fy } else { fx };
            let bulge = 1.0 - ((across - 0.5).abs() * 2.0).powi(2);
            let edge = if (cx + cy) % 4 < 2 { 0.0 } else { -0.15 };
            (bulge - 0.55) * 0.9 + edge
        }
        Tile::Paper => {
            let n = noise(xf / 4.0, yf / 4.0, (w / 4) as i32, (h / 4) as i32, 31) * 2.0 - 1.0;
            let f = noise(xf / 32.0, yf / 8.0, (w / 32) as i32, (h / 8) as i32, 32) * 2.0 - 1.0;
            0.6 * n + 0.4 * f
        }
        Tile::Wood => {
            // Grain lines that wander with x (whole periods across the tile) and bunch
            // and spread down it (whole periods down it): seamless both ways.
            let u = xf / w as f32;
            let v = yf / h as f32;
            let wander = 5.0 * (TAU * u).sin()
                + 2.5 * (TAU * 3.0 * u + 1.3).sin()
                + 1.2 * (TAU * 7.0 * u + 0.4).sin();
            let bunch = 4.0 * (TAU * 2.0 * v).sin();
            let pos = (yf + wander + bunch) / 8.0;
            let d = (pos - pos.floor() - 0.5).abs() * 2.0;
            let line = 1.0 - smooth((d / 0.35).min(1.0));
            let fleck = noise(xf / 32.0, yf / 4.0, (w / 32) as i32, (h / 4) as i32, 41);
            -0.75 * line + 0.25 * (fleck - 0.5)
        }
        Tile::Scan => {
            if y % 2 == 1 {
                -0.55
            } else {
                0.0
            }
        }
        Tile::Mesh => {
            if x.is_multiple_of(3) || y.is_multiple_of(3) {
                -0.7
            } else {
                0.0
            }
        }
    }
}

/// The most light and the most shade any texel of `t` holds (0…1 each): what a tile can
/// do to the colour under it at full strength (`materials::extremes`). Computed once.
pub fn reach(t: Tile) -> (f32, f32) {
    use std::sync::OnceLock;
    static REACH: OnceLock<[(f32, f32); 8]> = OnceLock::new();
    let all = REACH.get_or_init(|| {
        ALL.map(|t| {
            let (w, h) = size(t);
            let (mut hi, mut lo) = (0.0f32, 0.0f32);
            for y in 0..h {
                for x in 0..w {
                    let v = texel(t, x, y, w, h).clamp(-1.0, 1.0);
                    hi = hi.max(v);
                    lo = lo.min(v);
                }
            }
            (hi, -lo)
        })
    });
    all[ALL.iter().position(|a| *a == t).unwrap_or(0)]
}

/// Every tile.
pub const ALL: [Tile; 8] = [
    Tile::Brushed,
    Tile::Tolex,
    Tile::Carbon,
    Tile::Paper,
    Tile::Wood,
    Tile::Varnished,
    Tile::Scan,
    Tile::Mesh,
];

/// The side of the patch a tile is laid in, at least: the tile repeated to fill it, so a
/// large surface lays a quarter (or less) of the quads a tile alone would take.
const PATCH: usize = 256;

/// The patch of `t` kept in the atlas: whole tiles, so it repeats as they do.
fn patch(t: Tile) -> (usize, usize) {
    let (w, h) = size(t);
    (w * PATCH.div_ceil(w), h * PATCH.div_ceil(h))
}

/// The tile's patch's uv in the atlas, rasterised the first time.
fn tile_uv(b: &mut Brush, t: Tile) -> Option<(Rect, Vec2)> {
    let (tw, th) = size(t);
    let (w, h) = patch(t);
    let key = cache::key(("skin-tile", t, w, h));
    let uv = b.layer(key, w, h, |pix| {
        let data = pix.pixels_mut();
        for y in 0..h {
            for x in 0..w {
                let v = texel(t, x % tw, y % th, tw, th).clamp(-1.0, 1.0);
                let a = (v.abs() * 255.0).round() as u8;
                let c = if v >= 0.0 { a } else { 0 };
                if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(c, c, c, a) {
                    data[y * w + x] = p;
                }
            }
        }
    })?;
    Some((uv, Vec2::new(w as f32, h as f32)))
}

/// Lay tile `t` over `rect`, its light tinted `light` and its shade black, at `alpha`.
/// The grid starts at the rect's top left; partial tiles at its right and bottom take the
/// matching part of the tile.
pub fn lay(b: &mut Brush, rect: Rect, t: Tile, light: Color32, alpha: f32) {
    if alpha <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let Some((uv, texels)) = tile_uv(b, t) else {
        return;
    };
    let side = texels / b.ppp();
    let tint = super::super::color::with_alpha(light, alpha);
    let rect = b.snap_rect(rect);
    let mut y = rect.top();
    while y < rect.bottom() - 0.01 {
        let hgt = side.y.min(rect.bottom() - y);
        let mut x = rect.left();
        while x < rect.right() - 0.01 {
            let wid = side.x.min(rect.right() - x);
            let part = Rect::from_min_size(
                uv.min,
                Vec2::new(uv.width() * wid / side.x, uv.height() * hgt / side.y),
            );
            b.image(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(wid, hgt)),
                part,
                tint,
            );
            x += wid;
        }
        y += hgt;
    }
}

/// [`lay`] over a rounded rect: the corners' squares are left out, so nothing shows past
/// the curve.
pub fn lay_rounded(b: &mut Brush, rect: Rect, r: f32, t: Tile, light: Color32, alpha: f32) {
    let r = r.min(rect.width() * 0.5).min(rect.height() * 0.5).max(0.0);
    if r < 0.5 {
        lay(b, rect, t, light, alpha);
        return;
    }
    let r = b.snap(r);
    lay(
        b,
        Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + r),
            Pos2::new(rect.right(), rect.bottom() - r),
        ),
        t,
        light,
        alpha,
    );
    for (top, bottom) in [
        (rect.top(), rect.top() + r),
        (rect.bottom() - r, rect.bottom()),
    ] {
        lay(
            b,
            Rect::from_min_max(
                Pos2::new(rect.left() + r, top),
                Pos2::new(rect.right() - r, bottom),
            ),
            t,
            light,
            alpha,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_repeats_without_a_seam() {
        for t in ALL {
            let (w, h) = size(t);
            // Across the seam, neighbours differ no more than neighbours inside do.
            let mut inside = 0.0f32;
            let mut seam = 0.0f32;
            for y in 0..h {
                inside =
                    inside.max((texel(t, w / 2, y, w, h) - texel(t, w / 2 - 1, y, w, h)).abs());
                seam = seam.max((texel(t, 0, y, w, h) - texel(t, w - 1, y, w, h)).abs());
            }
            assert!(
                seam <= inside + 0.35,
                "{t:?}: a seam across x ({seam} vs {inside})"
            );
            let (mut inside, mut seam) = (0.0f32, 0.0f32);
            for x in 0..w {
                inside =
                    inside.max((texel(t, x, h / 2, w, h) - texel(t, x, h / 2 - 1, w, h)).abs());
                seam = seam.max((texel(t, x, 0, w, h) - texel(t, x, h - 1, w, h)).abs());
            }
            assert!(
                seam <= inside + 0.35,
                "{t:?}: a seam down y ({seam} vs {inside})"
            );
        }
    }
}
