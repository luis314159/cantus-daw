//! Our own image rasteriser (ADR 0089 §3): textured triangles onto a stage's pixmap.
//!
//! Sprite frames, DragonBones images and meshes and Lottie image layers draw through it.
//! tiny-skia has no textured-triangle call — a mesh drawn as one anti-aliased path per
//! triangle shows a seam along every shared edge, and its pattern shader runs on its slow
//! pipeline (measured: `docs/round35/import/`). Here:
//!
//! - one triangle at a time; each row's span is solved from the three edges, and u/v step
//!   along it (no per-pixel division);
//! - a **top-left rule**: a pixel centre on an edge shared by two triangles belongs to
//!   exactly one of them — no seam, no double blend;
//! - **nearest** (pixel art) or **bilinear** sampling, in 24.8 fixed point; a tap outside
//!   the source region is transparent, so a bilinear image fades over its last pixel
//!   (its edge is smooth) and never bleeds a neighbour of the atlas;
//! - premultiplied RGBA (tiny-skia's), **source-over**, with a tint (the slot's colour,
//!   the layer's opacity) multiplied in;
//! - only `+ − × ÷` and integer arithmetic: the same bytes on every system.

use tiny_skia::{Pixmap, Transform};

/// How the source is sampled.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Filter {
    /// Sharp pixels (pixel art).
    Nearest,
    /// Smooth (painted art, rotation, scaling down).
    Bilinear,
}

/// What a drawn image is multiplied and turned by: straight multipliers 0…1 (red, green,
/// blue, alpha — a slot's colour, an opacity) and a hue turn in degrees (`Pose::hue`),
/// applied per pixel as it is drawn — no turned copy of an atlas is ever made.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tint {
    pub rgba: [f32; 4],
    pub hue: f32,
}

impl Tint {
    pub const NONE: Tint = Tint {
        rgba: [1.0, 1.0, 1.0, 1.0],
        hue: 0.0,
    };

    pub fn rgba(rgba: [f32; 4]) -> Tint {
        Tint { rgba, hue: 0.0 }
    }

    pub fn opacity(a: f32) -> Tint {
        Tint::rgba([1.0, 1.0, 1.0, a])
    }

    /// The same, its colours turned by `deg` (whole degrees, as the hue is cached nowhere).
    pub fn hue(self, deg: f32) -> Tint {
        Tint { hue: deg, ..self }
    }

    /// As 0…256 integer factors for premultiplied channels (colour × alpha, alpha).
    fn factors(self) -> [u32; 4] {
        let [r, g, b, a] = self.rgba.map(|v| v.clamp(0.0, 1.0));
        let f = |v: f32| (v * 256.0 + 0.5) as u32;
        [f(r * a), f(g * a), f(b * a), f(a)]
    }

    fn multiplies(self) -> bool {
        self.rgba != [1.0, 1.0, 1.0, 1.0]
    }

    /// The hue turn as an 8.8 fixed-point matrix (the luminance-preserving one of
    /// `render::hue_rotate`), `None` for no turn.
    fn hue_matrix(self) -> Option<[i32; 9]> {
        let step = super::image::hue_step(self.hue);
        if step == 0 {
            return None;
        }
        let (s, c) = cantus_dsp::math::sin_cos(step as f32 * std::f32::consts::PI / 180.0);
        let m = [
            0.213 + c * 0.787 - s * 0.213,
            0.715 - c * 0.715 - s * 0.715,
            0.072 - c * 0.072 + s * 0.928,
            0.213 - c * 0.213 + s * 0.143,
            0.715 + c * 0.285 + s * 0.140,
            0.072 - c * 0.072 - s * 0.283,
            0.213 - c * 0.213 - s * 0.787,
            0.715 - c * 0.715 + s * 0.715,
            0.072 + c * 0.928 + s * 0.072,
        ];
        Some(m.map(|v| (v * 256.0).round() as i32))
    }
}

/// A vertex: where it lands on the destination (pixels) and where it samples the source
/// (pixels).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
}

/// The part of the source a triangle may sample: `[x0, y0, x1, y1)` in source pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Region {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Region {
    pub fn of(src: &Pixmap) -> Region {
        Region {
            x0: 0,
            y0: 0,
            x1: src.width() as i32,
            y1: src.height() as i32,
        }
    }

    /// Clamped to the source.
    pub fn within(self, src: &Pixmap) -> Region {
        let (w, h) = (src.width() as i32, src.height() as i32);
        Region {
            x0: self.x0.clamp(0, w),
            y0: self.y0.clamp(0, h),
            x1: self.x1.clamp(0, w),
            y1: self.y1.clamp(0, h),
        }
    }

    fn is_empty(self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
}

#[inline(always)]
fn texel(s: &[u8], sw: i32, r: Region, x: i32, y: i32) -> u32 {
    if x < r.x0 || y < r.y0 || x >= r.x1 || y >= r.y1 {
        return 0;
    }
    let i = ((y * sw + x) * 4) as usize;
    match s.get(i..i + 4) {
        Some(p) => u32::from_le_bytes([p[0], p[1], p[2], p[3]]),
        None => 0,
    }
}

/// `a` to `b` by `f` (0…256), two channels per lane.
#[inline(always)]
fn lerp_packed(a: u32, b: u32, f: u32) -> u32 {
    let g = 256 - f;
    let rb = (((a & 0x00FF_00FF) * g + (b & 0x00FF_00FF) * f) >> 8) & 0x00FF_00FF;
    let ag = ((((a >> 8) & 0x00FF_00FF) * g + ((b >> 8) & 0x00FF_00FF) * f) >> 8) & 0x00FF_00FF;
    rb | (ag << 8)
}

/// 24.8 fixed point, rounded towards −∞ (no `floor` call; the cast saturates).
#[inline(always)]
fn fixed(v: f32) -> i32 {
    let s = v * 256.0;
    let i = s as i32;
    if (i as f32) > s {
        i - 1
    } else {
        i
    }
}

#[inline(always)]
fn sample(s: &[u8], sw: i32, r: Region, u: f32, v: f32, filter: Filter) -> u32 {
    match filter {
        Filter::Nearest => {
            let (x, y) = (fixed(u) >> 8, fixed(v) >> 8);
            texel(s, sw, r, x, y)
        }
        Filter::Bilinear => {
            let (fu, fv) = (fixed(u) - 128, fixed(v) - 128);
            let (x, y) = (fu >> 8, fv >> 8);
            let (fx, fy) = ((fu & 255) as u32, (fv & 255) as u32);
            let top = lerp_packed(texel(s, sw, r, x, y), texel(s, sw, r, x + 1, y), fx);
            let bot = lerp_packed(texel(s, sw, r, x, y + 1), texel(s, sw, r, x + 1, y + 1), fx);
            lerp_packed(top, bot, fy)
        }
    }
}

/// `x * y / 255`, rounded (exact for 8-bit operands).
#[inline(always)]
fn mul255(x: u32, y: u32) -> u32 {
    let t = x * y + 128;
    (t + (t >> 8)) >> 8
}

#[inline(always)]
fn over(dst: &mut [u8], p: u32, tint: Option<[u32; 4]>, hue: Option<&[i32; 9]>) {
    let mut c = p.to_le_bytes().map(u32::from);
    if let Some(m) = hue {
        // Premultiplied: the matrix is linear, so it turns colour × alpha alike; each
        // channel stays within its alpha.
        let (r, g, b, a) = (c[0] as i32, c[1] as i32, c[2] as i32, c[3] as i32);
        let turn =
            |k: usize| ((m[k] * r + m[k + 1] * g + m[k + 2] * b + 128) >> 8).clamp(0, a) as u32;
        c = [turn(0), turn(3), turn(6), c[3]];
    }
    if let Some(f) = tint {
        for k in 0..4 {
            c[k] = (c[k] * f[k] + 128) >> 8;
        }
    }
    let sa = c[3];
    if sa == 0 {
        return;
    }
    if sa >= 255 {
        dst[0] = c[0].min(255) as u8;
        dst[1] = c[1].min(255) as u8;
        dst[2] = c[2].min(255) as u8;
        dst[3] = 255;
        return;
    }
    let inv = 255 - sa;
    for k in 0..4 {
        dst[k] = (c[k] + mul255(u32::from(dst[k]), inv)).min(255) as u8;
    }
}

/// Draw one textured triangle of `src` onto `dst` (both premultiplied RGBA).
pub fn triangle(
    dst: &mut Pixmap,
    src: &Pixmap,
    tri: [Vertex; 3],
    region: Region,
    filter: Filter,
    tint: Tint,
) {
    let region = region.within(src);
    if region.is_empty() || tint.rgba[3] <= 0.0 {
        return;
    }
    let [a, mut b, mut c] = tri;
    if [a, b, c]
        .iter()
        .any(|p| !(p.x.is_finite() && p.y.is_finite() && p.u.is_finite() && p.v.is_finite()))
    {
        return;
    }
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-6 {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
    }
    let inv_area = 1.0 / area.abs();
    // u and v as affine functions of the destination's x and y.
    let grad = |fa: f32, fb: f32, fc: f32| {
        let dx = ((fb - fa) * (c.y - a.y) - (fc - fa) * (b.y - a.y)) * inv_area;
        let dy = ((fc - fa) * (b.x - a.x) - (fb - fa) * (c.x - a.x)) * inv_area;
        (dx, dy, fa - dx * a.x - dy * a.y)
    };
    let (udx, udy, u0) = grad(a.u, b.u, c.u);
    let (vdx, vdy, v0) = grad(a.v, b.v, c.v);
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let ymin = a.y.min(b.y).min(c.y);
    let ymax = a.y.max(b.y).max(c.y);
    // Rows whose centre (y + 0.5) lies in [ymin, ymax].
    let y_first = ((ymin - 0.5).ceil().max(0.0)) as i32;
    let y_end = (((ymax - 0.5).floor() + 1.0).min(dh as f32)) as i32;
    if y_first >= y_end {
        return;
    }
    let hue = tint.hue_matrix();
    let tint = tint.multiplies().then(|| tint.factors());
    let (sw, sdata) = (src.width() as i32, src.data());
    let edges = [(b, c), (c, a), (a, b)];
    let data = dst.data_mut();
    for y in y_first..y_end {
        let py = y as f32 + 0.5;
        // e(px) = ex·(py − q0.y) − ey·(px − q0.x) ≥ 0 inside (> 0 on a non-top-left edge).
        let (mut lo, mut hi) = (f32::NEG_INFINITY, f32::INFINITY);
        let (mut lo_open, mut hi_open) = (false, false);
        let mut empty = false;
        for (q0, q1) in edges {
            let (ex, ey) = (q1.x - q0.x, q1.y - q0.y);
            let top_left = ey < 0.0 || (ey == 0.0 && ex > 0.0);
            let k = ex * (py - q0.y) + ey * q0.x; // e = k − ey·px
            if ey == 0.0 {
                if k < 0.0 || (k == 0.0 && !top_left) {
                    empty = true;
                }
            } else if ey < 0.0 {
                // e grows with px: px ≥ k / ey
                let bound = k / ey;
                if bound > lo || (bound == lo && !top_left) {
                    lo = bound;
                    lo_open = !top_left;
                }
            } else {
                // e falls with px: px ≤ k / ey
                let bound = k / ey;
                if bound < hi || (bound == hi && !top_left) {
                    hi = bound;
                    hi_open = !top_left;
                }
            }
        }
        if empty {
            continue;
        }
        // Pixel x has its centre at x + 0.5.
        let mut x0 = (lo - 0.5).ceil();
        if lo_open && x0 + 0.5 == lo {
            x0 += 1.0;
        }
        let mut x1 = (hi - 0.5).floor() + 1.0;
        if hi_open && x1 - 0.5 == hi {
            x1 -= 1.0;
        }
        let x0 = (x0.max(0.0) as i32).min(dw);
        let x1 = (x1.min(dw as f32).max(0.0)) as i32;
        if x1 <= x0 {
            continue;
        }
        let px0 = x0 as f32 + 0.5;
        let mut u = u0 + udx * px0 + udy * py;
        let mut v = v0 + vdx * px0 + vdy * py;
        let start = ((y * dw + x0) * 4) as usize;
        let end = ((y * dw + x1) * 4) as usize;
        let Some(row) = data.get_mut(start..end) else {
            continue;
        };
        for px in row.chunks_exact_mut(4) {
            let p = sample(sdata, sw, region, u, v, filter);
            if p != 0 {
                over(px, p, tint, hue.as_ref());
            }
            u += udx;
            v += vdx;
        }
    }
}

/// Draw the source rectangle `region` with `t` mapping its own pixels (0…w, 0…h) onto
/// the destination's. A bilinear image is grown by one source pixel all round (sampled
/// transparent), so its outline fades instead of stepping.
pub fn image(
    dst: &mut Pixmap,
    src: &Pixmap,
    region: Region,
    t: Transform,
    filter: Filter,
    tint: Tint,
) {
    let region = region.within(src);
    if region.is_empty() {
        return;
    }
    let (w, h) = (
        (region.x1 - region.x0) as f32,
        (region.y1 - region.y0) as f32,
    );
    let g = if filter == Filter::Bilinear { 1.0 } else { 0.0 };
    let corner = |x: f32, y: f32| {
        let (px, py) = map(t, x, y);
        Vertex {
            x: px,
            y: py,
            u: region.x0 as f32 + x,
            v: region.y0 as f32 + y,
        }
    };
    let q = [
        corner(-g, -g),
        corner(w + g, -g),
        corner(w + g, h + g),
        corner(-g, h + g),
    ];
    triangle(dst, src, [q[0], q[1], q[2]], region, filter, tint);
    triangle(dst, src, [q[0], q[2], q[3]], region, filter, tint);
}

/// Draw a mesh: `positions` on the destination (pixels), `uvs` in the source (pixels),
/// `triangles` as index triples. Indices out of range are skipped.
#[allow(clippy::too_many_arguments)]
pub fn mesh(
    dst: &mut Pixmap,
    src: &Pixmap,
    positions: &[(f32, f32)],
    uvs: &[(f32, f32)],
    triangles: &[[u32; 3]],
    region: Region,
    filter: Filter,
    tint: Tint,
) {
    let vtx = |i: u32| -> Option<Vertex> {
        let (x, y) = *positions.get(i as usize)?;
        let (u, v) = *uvs.get(i as usize)?;
        Some(Vertex { x, y, u, v })
    };
    for t in triangles {
        if let (Some(a), Some(b), Some(c)) = (vtx(t[0]), vtx(t[1]), vtx(t[2])) {
            triangle(dst, src, [a, b, c], region, filter, tint);
        }
    }
}

/// A point through a transform.
pub fn map(t: Transform, x: f32, y: f32) -> (f32, f32) {
    (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, Paint, Rect};

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
        p
    }

    #[test]
    fn compositing_matches_tiny_skia_within_one_step() {
        // The same colour over the same background: ours (an image) and tiny-skia's (a
        // rectangle), with and without an opacity.
        for (rgba, opacity) in [
            ([200u8, 40, 90, 128], 1.0f32),
            ([10, 220, 140, 255], 0.5),
            ([255, 255, 255, 37], 0.8),
        ] {
            let mut ours = solid(16, 16, [30, 60, 200, 180]);
            let mut theirs = ours.clone();
            let src = solid(8, 8, rgba);
            image(
                &mut ours,
                &src,
                Region::of(&src),
                Transform::from_translate(4.0, 4.0),
                Filter::Nearest,
                Tint::opacity(opacity),
            );
            let mut paint = Paint::default();
            let a = f32::from(rgba[3]) / 255.0 * opacity;
            paint.set_color(
                Color::from_rgba(
                    f32::from(rgba[0]) / 255.0,
                    f32::from(rgba[1]) / 255.0,
                    f32::from(rgba[2]) / 255.0,
                    a,
                )
                .unwrap(),
            );
            paint.anti_alias = false;
            theirs.fill_rect(
                Rect::from_xywh(4.0, 4.0, 8.0, 8.0).unwrap(),
                &paint,
                Transform::identity(),
                None,
            );
            for (o, t) in ours.data().iter().zip(theirs.data()) {
                assert!(
                    (i32::from(*o) - i32::from(*t)).abs() <= 1,
                    "ours {o} against tiny-skia's {t} for {rgba:?} at {opacity}"
                );
            }
        }
    }

    #[test]
    fn a_mesh_has_no_seam_and_no_double_blend() {
        // A half-transparent texture over a warped 8×8 grid: every pixel inside is drawn
        // exactly once (alpha exactly the texture's), none twice, none skipped.
        let src = solid(64, 64, [64, 64, 64, 128]);
        let mut dst = Pixmap::new(200, 200).unwrap();
        let n = 8usize;
        let mut pos = Vec::new();
        let mut uv = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let (fx, fy) = (i as f32 / n as f32, j as f32 / n as f32);
                // Interior vertices moved off the grid, the border kept straight.
                let wob = if i > 0 && i < n && j > 0 && j < n {
                    3.3 * ((i * 7 + j * 3) % 5) as f32 / 5.0
                } else {
                    0.0
                };
                pos.push((20.0 + fx * 160.0 + wob, 20.0 + fy * 160.0 - wob));
                uv.push((fx * 64.0, fy * 64.0));
            }
        }
        let mut tris = Vec::new();
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let k = j * (n as u32 + 1) + i;
                tris.push([k, k + 1, k + n as u32 + 2]);
                tris.push([k, k + n as u32 + 2, k + n as u32 + 1]);
            }
        }
        mesh(
            &mut dst,
            &src,
            &pos,
            &uv,
            &tris,
            Region::of(&src),
            Filter::Nearest,
            Tint::NONE,
        );
        for y in 20..180 {
            for x in 20..180 {
                let a = dst.pixel(x, y).unwrap().alpha();
                assert_eq!(a, 128, "pixel {x},{y}: alpha {a} (0 = a gap, more = twice)");
            }
        }
        assert_eq!(dst.pixel(19, 100).unwrap().alpha(), 0);
        assert_eq!(dst.pixel(180, 100).unwrap().alpha(), 0);
    }

    #[test]
    fn pixel_art_at_four_times_is_exact_blocks() {
        let mut src = Pixmap::new(4, 4).unwrap();
        for (i, p) in src.data_mut().chunks_exact_mut(4).enumerate() {
            let v = (i * 16) as u8;
            p.copy_from_slice(&[v, 255 - v, 0, 255]);
        }
        let mut dst = Pixmap::new(16, 16).unwrap();
        image(
            &mut dst,
            &src,
            Region::of(&src),
            Transform::from_scale(4.0, 4.0),
            Filter::Nearest,
            Tint::NONE,
        );
        for y in 0..16u32 {
            for x in 0..16u32 {
                assert_eq!(
                    dst.pixel(x, y).unwrap(),
                    src.pixel(x / 4, y / 4).unwrap(),
                    "{x},{y}"
                );
            }
        }
    }

    #[test]
    fn a_bilinear_image_has_a_soft_outline_and_no_bleed() {
        // The atlas holds a red cell beside a green one; drawing the red one never shows
        // green, and its outline steps through partial alpha.
        let mut src = solid(8, 4, [0, 255, 0, 255]);
        let red = solid(4, 4, [255, 0, 0, 255]);
        src.draw_pixmap(
            0,
            0,
            red.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        let mut dst = Pixmap::new(60, 60).unwrap();
        let cell = Region {
            x0: 0,
            y0: 0,
            x1: 4,
            y1: 4,
        };
        image(
            &mut dst,
            &src,
            cell,
            Transform::from_translate(10.0, 10.0).pre_scale(10.0, 10.0),
            Filter::Bilinear,
            Tint::NONE,
        );
        assert!(
            dst.pixels().iter().all(|p| p.green() == 0),
            "no green bleeds in"
        );
        let partial = dst
            .pixels()
            .iter()
            .filter(|p| p.alpha() > 0 && p.alpha() < 255)
            .count();
        assert!(partial > 40, "a soft outline ({partial} partial pixels)");
        assert_eq!(dst.pixel(30, 30).unwrap().alpha(), 255);
    }

    #[test]
    fn a_hue_turns_pixels_as_they_are_drawn_like_a_turned_copy() {
        let mut src = Pixmap::new(8, 8).unwrap();
        for (i, p) in src.data_mut().chunks_exact_mut(4).enumerate() {
            let a = 255 - (i * 3) as u8;
            let v = |x: u8| ((u32::from(x) * u32::from(a)) / 255) as u8;
            p.copy_from_slice(&[v(230), v((i * 4) as u8), v(40), a]);
        }
        for deg in [0.0f32, 37.0, 120.0, -90.0] {
            let mut ours = Pixmap::new(8, 8).unwrap();
            image(
                &mut ours,
                &src,
                Region::of(&src),
                Transform::identity(),
                Filter::Nearest,
                Tint::NONE.hue(deg),
            );
            let turned = super::super::image::hue_turned(&src, deg);
            for (o, t) in ours.data().iter().zip(turned.data()) {
                assert!(
                    (i32::from(*o) - i32::from(*t)).abs() <= 3,
                    "{deg}°: {o} against {t}"
                );
            }
        }
    }

    #[test]
    fn nonsense_vertices_draw_nothing() {
        let src = solid(4, 4, [255, 255, 255, 255]);
        let mut dst = Pixmap::new(10, 10).unwrap();
        let v = |x: f32, y: f32| Vertex {
            x,
            y,
            u: 0.0,
            v: 0.0,
        };
        for tri in [
            [v(f32::NAN, 0.0), v(5.0, 0.0), v(0.0, 5.0)],
            [v(0.0, 0.0), v(f32::INFINITY, 0.0), v(0.0, 5.0)],
            [v(0.0, 0.0), v(5.0, 5.0), v(10.0, 10.0)],
            [v(-1e30, -1e30), v(1e30, -1e30), v(0.0, 1e30)],
        ] {
            triangle(
                &mut dst,
                &src,
                tri,
                Region::of(&src),
                Filter::Bilinear,
                Tint::NONE,
            );
        }
        mesh(
            &mut dst,
            &src,
            &[(0.0, 0.0)],
            &[(0.0, 0.0)],
            &[[0, 7, 9]],
            Region::of(&src),
            Filter::Nearest,
            Tint::NONE,
        );
    }
}
