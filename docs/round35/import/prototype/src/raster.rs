//! Our own textured-triangle rasteriser (prototype): affine UVs, nearest or bilinear,
//! premultiplied source-over, top-left fill rule (shared edges drawn once: no seams).
use tiny_skia::Pixmap;

#[derive(Copy, Clone)]
pub struct V {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
}

fn sample_nearest(src: &[u8], sw: i32, sh: i32, u: f32, v: f32) -> [u32; 4] {
    let x = (u as i32).clamp(0, sw - 1);
    let y = (v as i32).clamp(0, sh - 1);
    let i = ((y * sw + x) * 4) as usize;
    [src[i] as u32, src[i + 1] as u32, src[i + 2] as u32, src[i + 3] as u32]
}

fn sample_bilinear(src: &[u8], sw: i32, sh: i32, u: f32, v: f32) -> [u32; 4] {
    let (u, v) = (u - 0.5, v - 0.5);
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (((u - x0) * 256.0) as u32, ((v - y0) * 256.0) as u32);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let px = |x: i32, y: i32| {
        let x = x.clamp(0, sw - 1);
        let y = y.clamp(0, sh - 1);
        ((y * sw + x) * 4) as usize
    };
    let (a, b, c, d) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
    let mut o = [0u32; 4];
    for k in 0..4 {
        let top = src[a + k] as u32 * (256 - fx) + src[b + k] as u32 * fx;
        let bot = src[c + k] as u32 * (256 - fx) + src[d + k] as u32 * fx;
        o[k] = (top * (256 - fy) + bot * fy) >> 16;
    }
    o
}

/// Draw one triangle of `src` onto `dst` (both premultiplied RGBA8).
pub fn tri(dst: &mut Pixmap, src: &Pixmap, t: [V; 3], bilinear: bool, region: [i32; 4]) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let s = src.data();
    let sw = src.width() as i32;
    let sh = src.height() as i32;
    let [a, mut b, mut c] = t;
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-6 {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
    }
    let area = area.abs();
    let minx = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let maxx = a.x.max(b.x).max(c.x).ceil().min(dw as f32) as i32;
    let miny = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let maxy = a.y.max(b.y).max(c.y).ceil().min(dh as f32) as i32;
    // Edge e(p) = (q1 - q0) x (p - q0); inside when all >= 0 (top-left rule by bias).
    let edge = |q0: V, q1: V| {
        let (ex, ey) = (q1.x - q0.x, q1.y - q0.y);
        let top_left = (ey == 0.0 && ex < 0.0) || ey > 0.0;
        (ex, ey, q0, if top_left { 0.0 } else { -1e-4 })
    };
    let es = [edge(b, c), edge(c, a), edge(a, b)];
    let d = dst.data_mut();
    let [rx0, ry0, rx1, ry1] = region;
    for y in miny..maxy {
        let py = y as f32 + 0.5;
        for x in minx..maxx {
            let px = x as f32 + 0.5;
            let mut w = [0.0f32; 3];
            let mut inside = true;
            for (k, (ex, ey, q0, bias)) in es.iter().enumerate() {
                let e = ex * (py - q0.y) - ey * (px - q0.x);
                if e + bias < 0.0 {
                    inside = false;
                    break;
                }
                w[k] = e;
            }
            if !inside {
                continue;
            }
            let (l0, l1, l2) = (w[0] / area, w[1] / area, w[2] / area);
            let u = (l0 * a.u + l1 * b.u + l2 * c.u).clamp(rx0 as f32, rx1 as f32 - 0.001);
            let v = (l0 * a.v + l1 * b.v + l2 * c.v).clamp(ry0 as f32, ry1 as f32 - 0.001);
            let p = if bilinear { sample_bilinear(s, sw, sh, u, v) } else { sample_nearest(s, sw, sh, u, v) };
            if p[3] == 0 {
                continue;
            }
            let i = ((y * dw + x) * 4) as usize;
            let inv = 255 - p[3];
            for k in 0..4 {
                d[i + k] = (p[k] + (d[i + k] as u32 * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// An axis-aligned or rotated quad: corners in stage pixels, its source rectangle.
pub fn quad(dst: &mut Pixmap, src: &Pixmap, corners: [(f32, f32); 4], r: [i32; 4], bilinear: bool) {
    let uv = [(r[0], r[1]), (r[2], r[1]), (r[2], r[3]), (r[0], r[3])];
    let v = |k: usize| V { x: corners[k].0, y: corners[k].1, u: uv[k].0 as f32, v: uv[k].1 as f32 };
    tri_fast(dst, src, [v(0), v(1), v(2)], bilinear, r);
    tri_fast(dst, src, [v(0), v(2), v(3)], bilinear, r);
}

/// The fast one: per row, the span inside all three edges is solved analytically, then
/// u/v advance incrementally; no per-pixel division.
pub fn tri_fast(dst: &mut Pixmap, src: &Pixmap, t: [V; 3], bilinear: bool, region: [i32; 4]) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let s = src.data();
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let [a, mut b, mut c] = t;
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-6 {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
    }
    let inv_area = 1.0 / area.abs();
    // u, v as affine functions of (x, y): solve from the three vertices.
    let grad = |fa: f32, fb: f32, fc: f32| {
        let dx = ((fb - fa) * (c.y - a.y) - (fc - fa) * (b.y - a.y)) * inv_area;
        let dy = ((fc - fa) * (b.x - a.x) - (fb - fa) * (c.x - a.x)) * inv_area;
        (dx, dy, fa - dx * a.x - dy * a.y)
    };
    let (udx, udy, u0) = grad(a.u, b.u, c.u);
    let (vdx, vdy, v0) = grad(a.v, b.v, c.v);
    let miny = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let maxy = a.y.max(b.y).max(c.y).ceil().min(dh as f32) as i32;
    let edges = [(b, c), (c, a), (a, b)];
    let [rx0, ry0, rx1, ry1] = region;
    let (ulo, uhi, vlo, vhi) = (rx0 as f32, rx1 as f32 - 0.001, ry0 as f32, ry1 as f32 - 0.001);
    let d = dst.data_mut();
    for y in miny..maxy {
        let py = y as f32 + 0.5;
        // x span (pixel centres px = x + 0.5) inside every edge: e(px) = k + m * px >= 0.
        let (mut xl, mut xr) = (0.0f32, dw as f32);
        let mut empty = false;
        for (q0, q1) in edges {
            let (ex, ey) = (q1.x - q0.x, q1.y - q0.y);
            let k = ex * (py - q0.y) + ey * q0.x;
            let m = -ey;
            if m == 0.0 {
                if k < 0.0 {
                    empty = true;
                }
            } else if m > 0.0 {
                xl = xl.max(-k / m - 0.5);
            } else {
                xr = xr.min(-k / m - 0.5);
            }
        }
        if empty {
            continue;
        }
        let x0 = xl.ceil().max(0.0) as i32;
        let x1 = (xr.ceil() as i32).min(dw);
        if x1 <= x0 {
            continue;
        }
        let mut u = u0 + udx * (x0 as f32 + 0.5) + udy * py;
        let mut v = v0 + vdx * (x0 as f32 + 0.5) + vdy * py;
        let row = &mut d[((y * dw + x0) * 4) as usize..((y * dw + x1) * 4) as usize];
        for px in row.chunks_exact_mut(4) {
            let (uu, vv) = (u.clamp(ulo, uhi), v.clamp(vlo, vhi));
            let p = if bilinear {
                let w = bilinear_packed(s, sw, sh, uu, vv).to_le_bytes();
                [w[0] as u32, w[1] as u32, w[2] as u32, w[3] as u32]
            } else {
                sample_nearest(s, sw, sh, uu, vv)
            };
            u += udx;
            v += vdx;
            let sa = p[3];
            if sa == 0 {
                continue;
            }
            if sa == 255 {
                px.copy_from_slice(&[p[0] as u8, p[1] as u8, p[2] as u8, 255]);
                continue;
            }
            let inv = 255 - sa;
            for k in 0..4 {
                let t = px[k] as u32 * inv + 128;
                px[k] = (p[k] + ((t + (t >> 8)) >> 8)).min(255) as u8;
            }
        }
    }
}

#[inline(always)]
fn lerp_packed(a: u32, b: u32, f: u32) -> u32 {
    // f in 0..=256; two channels per lane.
    let (a_rb, a_ga) = (a & 0x00FF_00FF, (a >> 8) & 0x00FF_00FF);
    let (b_rb, b_ga) = (b & 0x00FF_00FF, (b >> 8) & 0x00FF_00FF);
    let rb = ((a_rb * (256 - f) + b_rb * f) >> 8) & 0x00FF_00FF;
    let ga = ((a_ga * (256 - f) + b_ga * f) >> 8) & 0x00FF_00FF;
    rb | (ga << 8)
}

#[inline(always)]
pub fn bilinear_packed(s: &[u8], sw: i32, sh: i32, u: f32, v: f32) -> u32 {
    // 24.8 fixed point; +256 keeps it positive so truncation is floor (no libm call).
    let ufx = ((u - 0.5) * 256.0) as i32 + 65536;
    let vfx = ((v - 0.5) * 256.0) as i32 + 65536;
    let (fx, fy) = ((ufx & 255) as u32, (vfx & 255) as u32);
    let (x0, y0) = ((ufx >> 8) - 256, (vfx >> 8) - 256);
    let (xa, xb) = (x0.clamp(0, sw - 1), (x0 + 1).clamp(0, sw - 1));
    let (ya, yb) = (y0.clamp(0, sh - 1), (y0 + 1).clamp(0, sh - 1));
    let g = |x: i32, y: i32| {
        let i = ((y * sw + x) * 4) as usize;
        u32::from_le_bytes([s[i], s[i + 1], s[i + 2], s[i + 3]])
    };
    let top = lerp_packed(g(xa, ya), g(xb, ya), fx);
    let bot = lerp_packed(g(xa, yb), g(xb, yb), fx);
    lerp_packed(top, bot, fy)
}
