//! Lottie's geometry as cubic contours: the shapes' outlines, their transforms, trim paths
//! (cut on the curves themselves) and rounded corners — before anything becomes pixels.

use super::model::Xform;
use super::prop::Bezier;
use cantus_dsp::math;
use tiny_skia::{PathBuilder, Transform};

/// A shape's outlines, in the coordinates of the group they were drawn in, and that
/// group's transform onto the target.
#[derive(Clone, Debug)]
pub struct Geo {
    pub contours: Vec<Bezier>,
    pub t: Transform,
}

/// A rotation by `deg` with the deterministic maths.
pub fn rotation(deg: f32) -> Transform {
    if deg == 0.0 {
        return Transform::identity();
    }
    let (s, c) = math::sin_cos(deg * std::f32::consts::PI / 180.0);
    Transform::from_row(c, s, -s, c, 0.0, 0.0)
}

/// A Lottie transform at frame `f`: T(p) · R(r) · skew · S(s) · T(−a).
pub fn matrix(x: &Xform, f: f32) -> Transform {
    let [ax, ay] = x.anchor.at2(f, [0.0, 0.0]);
    let [px, py] = match &x.split {
        Some((sx, sy)) => [sx.at1(f, 0.0), sy.at1(f, 0.0)],
        None => x.position.at2(f, [0.0, 0.0]),
    };
    let [sx, sy] = x.scale.at2(f, [100.0, 100.0]);
    let mut t = Transform::from_translate(px, py).pre_concat(rotation(x.rotation.at1(f, 0.0)));
    let sk = x.skew.at1(f, 0.0);
    if sk != 0.0 {
        let sa = x.skew_axis.at1(f, 0.0);
        let shear = math::tan((-sk).clamp(-85.0, 85.0) * std::f32::consts::PI / 180.0);
        t = t
            .pre_concat(rotation(sa))
            .pre_concat(Transform::from_row(1.0, 0.0, shear, 1.0, 0.0, 0.0))
            .pre_concat(rotation(-sa));
    }
    t.pre_scale(sx / 100.0, sy / 100.0).pre_translate(-ax, -ay)
}

/// A repeater's k-th copy: x' = a + p·k + R(r·k) · S(s^k) · (x − a).
pub fn repeat_matrix(x: &Xform, f: f32, k: f32) -> Transform {
    let [ax, ay] = x.anchor.at2(f, [0.0, 0.0]);
    let [px, py] = x.position.at2(f, [0.0, 0.0]);
    let [sx, sy] = x.scale.at2(f, [100.0, 100.0]);
    let r = x.rotation.at1(f, 0.0);
    let pow = |s: f32| math::powf((s / 100.0).max(0.0), k);
    Transform::from_translate(ax + px * k, ay + py * k)
        .pre_concat(rotation(r * k))
        .pre_scale(pow(sx), pow(sy))
        .pre_translate(-ax, -ay)
}

/// κ: a quarter circle's control distance.
const KAPPA: f32 = 0.552_284_8;

pub fn ellipse(c: [f32; 2], s: [f32; 2]) -> Bezier {
    let (rx, ry) = (s[0] * 0.5, s[1] * 0.5);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    Bezier {
        closed: true,
        v: vec![
            [c[0], c[1] - ry],
            [c[0] + rx, c[1]],
            [c[0], c[1] + ry],
            [c[0] - rx, c[1]],
        ],
        i: vec![[-kx, 0.0], [0.0, -ky], [kx, 0.0], [0.0, ky]],
        o: vec![[kx, 0.0], [0.0, ky], [-kx, 0.0], [0.0, -ky]],
    }
}

pub fn rect(c: [f32; 2], s: [f32; 2], r: f32) -> Bezier {
    let (hw, hh) = (s[0].abs() * 0.5, s[1].abs() * 0.5);
    let r = r.max(0.0).min(hw).min(hh);
    let (l, t, rt, b) = (c[0] - hw, c[1] - hh, c[0] + hw, c[1] + hh);
    if r <= 0.0 {
        return Bezier {
            closed: true,
            v: vec![[rt, t], [rt, b], [l, b], [l, t]],
            i: vec![[0.0; 2]; 4],
            o: vec![[0.0; 2]; 4],
        };
    }
    let k = r * KAPPA;
    Bezier {
        closed: true,
        v: vec![
            [rt - r, t],
            [rt, t + r],
            [rt, b - r],
            [rt - r, b],
            [l + r, b],
            [l, b - r],
            [l, t + r],
            [l + r, t],
        ],
        i: vec![
            [-0.0, 0.0],
            [0.0, -k],
            [0.0, 0.0],
            [k, 0.0],
            [0.0, 0.0],
            [0.0, k],
            [0.0, 0.0],
            [-k, 0.0],
        ],
        o: vec![
            [k, 0.0],
            [0.0, 0.0],
            [0.0, k],
            [0.0, 0.0],
            [-k, 0.0],
            [0.0, 0.0],
            [0.0, -k],
            [0.0, 0.0],
        ],
    }
}

/// A star or a polygon (straight edges).
pub fn star(star: bool, c: [f32; 2], points: f32, rot: f32, outer: f32, inner: f32) -> Bezier {
    let n = (points.round() as i32).clamp(3, 100) as usize;
    let count = if star { n * 2 } else { n };
    let mut v = Vec::with_capacity(count);
    for k in 0..count {
        let r = if star && k % 2 == 1 { inner } else { outer };
        let a = (rot - 90.0) * std::f32::consts::PI / 180.0
            + k as f32 * std::f32::consts::TAU / count as f32;
        let (s, co) = math::sin_cos(a);
        v.push([c[0] + r * co, c[1] + r * s]);
    }
    Bezier {
        closed: true,
        i: vec![[0.0; 2]; v.len()],
        o: vec![[0.0; 2]; v.len()],
        v,
    }
}

/// The cubic segments of a contour: (p0, p1, p2, p3).
pub fn segments(b: &Bezier) -> Vec<[[f32; 2]; 4]> {
    let n = b.v.len();
    if n < 2 {
        return Vec::new();
    }
    let count = if b.closed { n } else { n - 1 };
    (0..count)
        .map(|k| {
            let (a, c) = (k, (k + 1) % n);
            [
                b.v[a],
                [b.v[a][0] + b.o[a][0], b.v[a][1] + b.o[a][1]],
                [b.v[c][0] + b.i[c][0], b.v[c][1] + b.i[c][1]],
                b.v[c],
            ]
        })
        .collect()
}

fn lerp(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// A cubic cut to its part between parameters `t0` and `t1`.
pub fn sub_cubic(s: [[f32; 2]; 4], t0: f32, t1: f32) -> [[f32; 2]; 4] {
    // Split at t1, keep the first part; split that at t0/t1, keep the second.
    let split = |s: [[f32; 2]; 4], t: f32| {
        let ab = lerp(s[0], s[1], t);
        let bc = lerp(s[1], s[2], t);
        let cd = lerp(s[2], s[3], t);
        let abc = lerp(ab, bc, t);
        let bcd = lerp(bc, cd, t);
        let m = lerp(abc, bcd, t);
        ([s[0], ab, abc, m], [m, bcd, cd, s[3]])
    };
    let (first, _) = split(s, t1);
    if t1 <= 1e-9 {
        return [s[0]; 4];
    }
    let (_, second) = split(first, (t0 / t1).clamp(0.0, 1.0));
    second
}

/// A segment's length and its arc-length table (16 steps) for finding a parameter.
fn measure(s: &[[f32; 2]; 4]) -> (f32, [f32; 17]) {
    let mut table = [0.0f32; 17];
    let mut prev = s[0];
    let mut len = 0.0;
    for (k, slot) in table.iter_mut().enumerate().skip(1) {
        let t = k as f32 / 16.0;
        let v = 1.0 - t;
        let p = [
            v * v * v * s[0][0]
                + 3.0 * v * v * t * s[1][0]
                + 3.0 * v * t * t * s[2][0]
                + t * t * t * s[3][0],
            v * v * v * s[0][1]
                + 3.0 * v * v * t * s[1][1]
                + 3.0 * v * t * t * s[2][1]
                + t * t * t * s[3][1],
        ];
        let (dx, dy) = (p[0] - prev[0], p[1] - prev[1]);
        len += (dx * dx + dy * dy).sqrt();
        *slot = len;
        prev = p;
    }
    (len, table)
}

fn param_at(table: &[f32; 17], d: f32) -> f32 {
    let len = table[16];
    if len <= 0.0 {
        return 0.0;
    }
    let k = table.partition_point(|x| *x < d).clamp(1, 16);
    let (a, b) = (table[k - 1], table[k]);
    let f = if b > a { (d - a) / (b - a) } else { 0.0 };
    ((k - 1) as f32 + f.clamp(0.0, 1.0)) / 16.0
}

/// The contours' total length.
pub fn length(contours: &[Bezier]) -> f32 {
    contours
        .iter()
        .flat_map(segments)
        .map(|s| measure(&s).0)
        .sum()
}

/// The part of `contours` (taken as one path, end to end) from length `d0` to `d1`.
fn cut(contours: &[Bezier], d0: f32, d1: f32) -> Vec<Bezier> {
    let mut out = Vec::new();
    let mut at = 0.0f32;
    for c in contours {
        let mut piece: Option<Bezier> = None;
        for s in segments(c) {
            let (len, table) = measure(&s);
            let (a, b) = (at, at + len);
            at = b;
            if b <= d0 || a >= d1 || len <= 0.0 {
                if let Some(p) = piece.take() {
                    out.push(p);
                }
                continue;
            }
            let t0 = param_at(&table, (d0 - a).max(0.0));
            let t1 = param_at(&table, (d1 - a).min(len));
            let part = sub_cubic(s, t0, t1);
            let p = piece.get_or_insert_with(|| Bezier {
                closed: false,
                v: vec![part[0]],
                i: vec![[0.0, 0.0]],
                o: vec![[0.0, 0.0]],
            });
            if let Some(o) = p.o.last_mut() {
                *o = [part[1][0] - part[0][0], part[1][1] - part[0][1]];
            }
            p.v.push(part[3]);
            p.i.push([part[2][0] - part[3][0], part[2][1] - part[3][1]]);
            p.o.push([0.0, 0.0]);
        }
        if let Some(p) = piece {
            out.push(p);
        }
    }
    out
}

/// Trim `contours` to `start`…`end` (0…1) shifted by `offset` (turns), wrapping.
pub fn trim(contours: &[Bezier], start: f32, end: f32, offset: f32) -> Vec<Bezier> {
    let (mut s, mut e) = (start.min(end), start.max(end));
    if e - s >= 1.0 - 1e-6 {
        return contours.to_vec();
    }
    if e - s <= 1e-6 {
        return Vec::new();
    }
    let o = offset.rem_euclid(1.0);
    s += o;
    e += o;
    let total = length(contours);
    if total <= 0.0 {
        return Vec::new();
    }
    if e <= 1.0 {
        cut(contours, s * total, e * total)
    } else if s >= 1.0 {
        cut(contours, (s - 1.0) * total, (e - 1.0) * total)
    } else {
        let mut a = cut(contours, s * total, total);
        a.extend(cut(contours, 0.0, (e - 1.0) * total));
        a
    }
}

/// Corners rounded by `r`: each sharp vertex (no tangents) cut back and joined by a curve.
pub fn round_corners(b: &Bezier, r: f32) -> Bezier {
    let n = b.v.len();
    if r <= 0.0 || n < 3 {
        return b.clone();
    }
    let mut out = Bezier {
        closed: b.closed,
        ..Bezier::default()
    };
    for k in 0..n {
        let sharp = b.i[k] == [0.0, 0.0] && b.o[k] == [0.0, 0.0];
        let ends = !b.closed && (k == 0 || k == n - 1);
        if !sharp || ends {
            out.v.push(b.v[k]);
            out.i.push(b.i[k]);
            out.o.push(b.o[k]);
            continue;
        }
        let p = b.v[k];
        let prev = b.v[(k + n - 1) % n];
        let next = b.v[(k + 1) % n];
        let dist = |a: [f32; 2], c: [f32; 2]| {
            let (x, y) = (a[0] - c[0], a[1] - c[1]);
            (x * x + y * y).sqrt()
        };
        let (dp, dn) = (dist(p, prev), dist(p, next));
        let cut_p = r.min(dp * 0.5);
        let cut_n = r.min(dn * 0.5);
        let a = if dp > 0.0 {
            lerp(p, prev, cut_p / dp)
        } else {
            p
        };
        let c = if dn > 0.0 {
            lerp(p, next, cut_n / dn)
        } else {
            p
        };
        out.v.push(a);
        out.i.push([0.0, 0.0]);
        out.o.push([(p[0] - a[0]) * KAPPA, (p[1] - a[1]) * KAPPA]);
        out.v.push(c);
        out.i.push([(p[0] - c[0]) * KAPPA, (p[1] - c[1]) * KAPPA]);
        out.o.push([0.0, 0.0]);
    }
    out
}

/// The contours as a tiny-skia path, each point through `t`.
pub fn to_path(contours: &[Bezier], t: Transform, pb: &mut PathBuilder) {
    let m = |p: [f32; 2]| super::super::raster::map(t, p[0], p[1]);
    for c in contours {
        let segs = segments(c);
        if segs.is_empty() {
            continue;
        }
        let (x, y) = m(segs[0][0]);
        pb.move_to(x, y);
        for s in &segs {
            let (a, b, d) = (m(s[1]), m(s[2]), m(s[3]));
            pb.cubic_to(a.0, a.1, b.0, b.1, d.0, d.1);
        }
        if c.closed {
            pb.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trim_keeps_the_part_asked_for() {
        let sq = rect([0.0, 0.0], [10.0, 10.0], 0.0);
        assert!((length(std::slice::from_ref(&sq)) - 40.0).abs() < 1e-3);
        let half = trim(std::slice::from_ref(&sq), 0.0, 0.5, 0.0);
        assert!((length(&half) - 20.0).abs() < 1e-3);
        // Wrapping past the end.
        let wrap = trim(std::slice::from_ref(&sq), 0.75, 1.0, 0.5);
        assert!((length(&wrap) - 10.0).abs() < 1e-3);
        assert!(trim(std::slice::from_ref(&sq), 0.3, 0.3, 0.0).is_empty());
        let circle = ellipse([0.0, 0.0], [20.0, 20.0]);
        let l = length(std::slice::from_ref(&circle));
        assert!((l - std::f32::consts::PI * 20.0).abs() < 0.2, "{l}");
        let q = trim(std::slice::from_ref(&circle), 0.0, 0.25, 0.0);
        assert!((length(&q) - l / 4.0).abs() < 0.2);
    }

    #[test]
    fn a_sub_cubic_starts_and_ends_on_the_curve() {
        let s = [[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]];
        let p = sub_cubic(s, 0.0, 1.0);
        assert_eq!(p, s);
        let mid = sub_cubic(s, 0.5, 1.0);
        assert!((mid[0][0] - 5.0).abs() < 1e-5 && (mid[0][1] - 7.5).abs() < 1e-5);
    }

    #[test]
    fn rounded_corners_add_two_points_per_corner() {
        let sq = rect([0.0, 0.0], [10.0, 10.0], 0.0);
        let r = round_corners(&sq, 2.0);
        assert_eq!(r.v.len(), 8);
        assert!(length(std::slice::from_ref(&r)) < 40.0);
    }

    #[test]
    fn the_transform_order_is_lotties() {
        use super::super::prop::Prop;
        let s = |v: &[f32]| Prop::Static(v.to_vec());
        let x = Xform {
            anchor: s(&[10.0, 0.0]),
            position: s(&[100.0, 100.0]),
            split: None,
            scale: s(&[200.0, 200.0]),
            rotation: s(&[90.0]),
            skew: s(&[0.0]),
            skew_axis: s(&[0.0]),
            opacity: s(&[100.0]),
            start_opacity: s(&[100.0]),
            end_opacity: s(&[100.0]),
        };
        let t = matrix(&x, 0.0);
        // The anchor lands on the position; a point 1 to its right goes 2 down.
        let (ax, ay) = super::super::super::raster::map(t, 10.0, 0.0);
        assert!((ax - 100.0).abs() < 1e-4 && (ay - 100.0).abs() < 1e-4);
        let (bx, by) = super::super::super::raster::map(t, 11.0, 0.0);
        assert!((bx - 100.0).abs() < 1e-4 && (by - 102.0).abs() < 1e-4);
    }
}
