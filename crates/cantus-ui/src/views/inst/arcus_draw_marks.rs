//! r42:arcus-hands — the fine marks on a realistic figure's hands that its mesh is too coarse
//! to carry (the owner, 2026-11-25: "find the details and improve them, especially the
//! fingers of the hand"; the reference is photographs of professional violinists): the
//! dorsal creases over each finger's middle joint (three, curved round the finger) and last
//! joint (two) and over the thumb's joint, drawn as thin soft lines **on the skinned
//! surface** — each line's points are anchored at rest to a triangle of the arm's mesh (its
//! barycentric place, found by casting from the joint's axis outward round the finger's
//! back), so a frame carries them with the skin, sorts them with the triangles and hides
//! them where their skin turns away or the neck stands in front.
//!
//! Made once per arm mesh (High's only; Light draws none) and kept; a frame projects a few
//! hundred points and strokes some fifty polylines.

use crate::scene::bowed_human::{self as hu, Figure, Stuff};
use crate::scene::geo::V3;

/// A point on the arm's surface: a triangle of the arm's mesh and the point's weights.
#[derive(Copy, Clone, Debug, Default)]
pub struct Anchor {
    pub tri: u32,
    pub w: [f32; 3],
}

/// One crease: a line of anchors (at most [`LINE_PTS`]), how strong it is and its width (cm).
#[derive(Copy, Clone, Debug)]
pub struct Line {
    pub pts: [Anchor; LINE_PTS],
    pub n: u8,
    pub strength: f32,
    pub width: f32,
}

pub const LINE_PTS: usize = 9;

/// The creases a joint carries: (the joint's finger, its bone whose head is the joint, the
/// lines' offsets along the finger from the joint (cm, toward the tip positive, scaled by
/// the hand), each line's half-span round the finger (deg), its strength).
const CREASES: [(usize, usize, &[(f32, f32, f32)]); 9] = [
    // The middle joints (PIP): three, the middle one the longest and darkest.
    (1, 1, PIP),
    (2, 1, PIP),
    (3, 1, PIP),
    (4, 1, PIP),
    // The last joints (DIP): two, fainter.
    (1, 2, DIP),
    (2, 2, DIP),
    (3, 2, DIP),
    (4, 2, DIP),
    // The thumb's joint (IP).
    (0, 2, DIP),
];
const PIP: &[(f32, f32, f32)] = &[(-0.13, 38.0, 0.55), (0.0, 52.0, 0.85), (0.12, 42.0, 0.65)];
const DIP: &[(f32, f32, f32)] = &[(-0.05, 40.0, 0.6), (0.07, 34.0, 0.45)];
/// How far a crease's ends bow back toward the hand (cm at its half-span): the dorsal
/// creases are arcs round the knuckle, not rings.
const BOW: f32 = 0.05;
/// A crease's width (cm) on her hand.
const CREASE_W: f32 = 0.028;

/// The hand's back at rest (away from the palm) for side `s`.
fn back_of(fig: &Figure, s: usize) -> V3 {
    let o = hu::side(s);
    let kn = |f: usize| fig.bones[o + hu::finger(f, 0)].head;
    let wr = fig.bones[o + hu::HAND].head;
    let mut back = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
    if (back.x < 0.0) == (s == 0) {
        back = back * -1.0;
    }
    back
}

/// Where a ray from `o` along `d` meets triangle `p` (Möller–Trumbore): its distance and
/// weights.
fn hit(o: V3, d: V3, p: [V3; 3]) -> Option<(f32, [f32; 3])> {
    let e1 = p[1] - p[0];
    let e2 = p[2] - p[0];
    let h = d.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-9 {
        return None;
    }
    let f = 1.0 / a;
    let s = o - p[0];
    let u = f * s.dot(h);
    if !(-1e-4..=1.0001).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = f * d.dot(q);
    if v < -1e-4 || u + v > 1.0001 {
        return None;
    }
    let t = f * e2.dot(q);
    (t > 0.0).then_some((t, [1.0 - u - v, u, v]))
}

/// The marks of arm `s`'s hand on the mesh given by the figure's vertices `verts` and the
/// triangles `tris` (the arm's own numbering) with their `stuff`.
pub fn build(fig: &Figure, s: usize, verts: &[u32], tris: &[[u16; 3]], stuff: &[Stuff]) -> Vec<Line> {
    let o = hu::side(s);
    if fig.bones.len() < hu::BONES {
        return Vec::new();
    }
    let back = back_of(fig, s);
    let wr = fig.bones[o + hu::HAND].head;
    // The hand's size against hers (the woman's wrist to middle knuckle, 7.7 cm).
    let k = ((fig.bones[o + hu::finger(2, 0)].head - wr).len() / 7.7).clamp(0.7, 1.5);
    let main_bone = |g: usize| {
        fig.bw[g]
            .iter()
            .fold((0u8, -1.0f32), |b, &(x, w)| if w > b.1 { (x, w) } else { b })
            .0 as usize
    };
    let mut out = Vec::new();
    for &(f, bone_k, lines) in &CREASES {
        let b_in = o + hu::finger(f, bone_k - 1);
        let b_out = o + hu::finger(f, bone_k);
        let joint = fig.bones[b_out].head;
        let a_in = (joint - fig.bones[b_in].head).norm();
        let a_out = (fig.bones[b_out].tail - joint).norm();
        let ax = (a_in + a_out).norm();
        // The joint's back: the hand's back square to the finger (the thumb's: its nail's
        // side, away from the palm's middle).
        let up = if f == 0 {
            let mid = (fig.bones[o + hu::finger(2, 0)].head + wr) * 0.5;
            let x = joint - mid;
            let x = x - ax * x.dot(ax);
            (x.norm() + back).norm()
        } else {
            back
        };
        let up = (up - ax * up.dot(ax)).norm();
        let side = ax.cross(up).norm();
        // The finger's own skin triangles round the joint.
        let near: Vec<usize> = tris
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                stuff[*i] == Stuff::Skin
                    && t.iter().any(|&j| {
                        let g = verts[j as usize] as usize;
                        let mb = main_bone(g);
                        (mb == b_in || mb == b_out) && (fig.pos[g] - joint).len() < 1.6 * k
                    })
            })
            .map(|(i, _)| i)
            .collect();
        for &(u, half, strength) in lines {
            let mut line = Line {
                pts: [Anchor::default(); LINE_PTS],
                n: 0,
                strength,
                width: CREASE_W * k,
            };
            for j in 0..LINE_PTS {
                let x = j as f32 / (LINE_PTS - 1) as f32 * 2.0 - 1.0;
                let th = (x * half).to_radians();
                let c = joint + ax * ((u - BOW * x * x) * k);
                let d = up * th.cos() + side * th.sin();
                let mut best: Option<(f32, Anchor)> = None;
                for &ti in &near {
                    let p = tris[ti].map(|q| fig.pos[verts[q as usize] as usize]);
                    if let Some((t, w)) = hit(c, d, p) {
                        // (The first skin along the ray from inside the finger.)
                        if t < 2.0 * k && best.is_none_or(|b| t < b.0) {
                            best = Some((
                                t,
                                Anchor {
                                    tri: ti as u32,
                                    w,
                                },
                            ));
                        }
                    }
                }
                if let Some((_, a)) = best {
                    if (line.n as usize) < LINE_PTS {
                        line.pts[line.n as usize] = a;
                        line.n += 1;
                    }
                }
            }
            if line.n >= 3 {
                out.push(line);
            }
        }
    }
    out
}

/// Where a line's middle is in the room this frame (the triangles' skinned corners `pts`):
/// the line is sorted among the triangles by it.
pub fn middle(line: &Line, tris: &[[u16; 3]], pts: &[V3]) -> V3 {
    at(&line.pts[line.n as usize / 2], tris, pts)
}

fn at(a: &Anchor, tris: &[[u16; 3]], pts: &[V3]) -> V3 {
    let t = tris[a.tri as usize];
    pts[t[0] as usize] * a.w[0] + pts[t[1] as usize] * a.w[1] + pts[t[2] as usize] * a.w[2]
}

/// How much darker a crease is than the skin under it, at its full strength.
const CREASE_DARK: f32 = 0.58;
/// A crease drawn no thinner than this share of a pixel (thinner, its colour is thinned).
const MIN_PX: f32 = 0.9;
/// The hand drawn smaller than this (points a centimetre): no creases (they would be dots).
const MIN_SCALE: f32 = 9.0;

/// Draws `line` from the arm's skinned corners (`pts`, `nrm`), the corners' colours this
/// frame (`col`, per triangle) and the eye: faded where its skin turns from the eye, its
/// colour the skin's under it, darker.
#[allow(clippy::too_many_arguments)]
pub fn stroke(
    b: &mut crate::kit::Brush,
    v: &super::body::View,
    line: &Line,
    tris: &[[u16; 3]],
    pts: &[V3],
    nrm: &[V3],
    col: &[[egui::Color32; 3]],
    eye: V3,
) {
    let n = line.n as usize;
    let mut prev: Option<(egui::Pos2, f32, [f32; 3], f32)> = None;
    for a in &line.pts[..n] {
        let t = tris[a.tri as usize].map(|i| i as usize);
        let p = at(a, tris, pts);
        let nn = (nrm[t[0]] * a.w[0] + nrm[t[1]] * a.w[1] + nrm[t[2]] * a.w[2]).norm();
        let facing = nn.dot((eye - p).norm());
        let c = col[a.tri as usize];
        // (A triangle not drawn this frame has no colour: its crease is not drawn either.)
        let fade = ((facing - 0.12) / 0.3).clamp(0.0, 1.0) * (c[0].a() > 0) as u8 as f32;
        let rgb = [0, 1, 2].map(|ch| {
            (0..3)
                .map(|k| {
                    let x = c[k];
                    [x.r(), x.g(), x.b()][ch] as f32 * a.w[k]
                })
                .sum::<f32>()
        });
        let scale = v.scale(p);
        let q = (v.p(p), fade, rgb, scale);
        if let Some(pr) = prev {
            let f = (pr.1 + q.1) * 0.5;
            let sc = (pr.3 + q.3) * 0.5;
            if f > 0.01 && sc > MIN_SCALE {
                let w = line.width * sc;
                let px = b.px() * MIN_PX;
                let thin = (w / px).min(1.0);
                // Fades in over the first scales a crease reads at.
                let zoom = ((sc - MIN_SCALE) / MIN_SCALE).clamp(0.0, 1.0);
                let alpha = line.strength * f * thin * zoom;
                let k = 1.0 - CREASE_DARK;
                let m = |ch: usize| ((pr.2[ch] + q.2[ch]) * 0.5 * k * alpha).round() as u8;
                let colour =
                    egui::Color32::from_rgba_premultiplied(m(0), m(1), m(2), (alpha * 255.0) as u8);
                b.segment(pr.0, q.0, w.max(px), colour, false);
            }
        }
        prev = Some(q);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::geo::v;

    #[test]
    fn a_ray_meets_a_triangle_at_its_weights() {
        let p = [v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(0.0, 1.0, 1.0)];
        let (t, w) = hit(v(0.25, 0.25, 0.0), v(0.0, 0.0, 1.0), p).expect("a hit");
        assert!((t - 1.0).abs() < 1e-5);
        assert!((w[1] - 0.25).abs() < 1e-5 && (w[2] - 0.25).abs() < 1e-5);
        assert!(hit(v(2.0, 2.0, 0.0), v(0.0, 0.0, 1.0), p).is_none());
    }
}
