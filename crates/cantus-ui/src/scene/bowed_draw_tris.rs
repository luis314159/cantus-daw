//! r42:arcus-upper — the realistic figures' triangles (`Mesh::tris`) in the Drawn picture:
//! each shaded at its three corners (smooth normals) and painted with the linear gradient
//! along which its shading changes most, so a cheek or a sleeve reads round, not faceted.
//! The skin takes its albedo's detail (the brows, the lips, the lashes' line) per corner and
//! a warm light in its shade (light through skin); the velvet its pale sheen where it turns
//! away; the hair a streak along its strands.

use super::bowed::Bowed;
use super::bowed_mesh::{Mesh, Role};
use super::display::{Paint, Part};
use super::prims::Ctx;
use crate::kit::skin::scene::{hold as hp, Mat, C3};

/// A corner's material: the skin's albedo with its detail (`lum`, `red`).
fn skin_at(base: &Mat, d: [f32; 2]) -> Mat {
    let (lum, red) = (d[0].min(1.1), d[1]);
    let a = base.albedo;
    let albedo = [
        (a[0] * lum * (1.0 + 0.22 * red)).clamp(0.0, 1.0),
        (a[1] * lum * (1.0 - 0.12 * red)).clamp(0.0, 1.0),
        (a[2] * lum * (1.0 - 0.18 * red)).clamp(0.0, 1.0),
    ];
    Mat { albedo, ..*base }
}

/// A hair corner's albedo: soft streaks across its strands, blended into the skin's
/// `scalp` colour at the hairline (`d[0]`: 0 at the hairline, 1 inside it).
fn hair_at(base: &Mat, d: [f32; 2], scalp: C3) -> Mat {
    let across = d[1];
    let k = 0.92 + 0.06 * (across * 2.1).sin() + 0.04 * (across * 5.3 + 1.7).sin();
    let e = d[0].clamp(0.0, 1.0).powf(0.6);
    Mat {
        albedo: std::array::from_fn(|i| {
            (scalp[i] + (base.albedo[i] * k - scalp[i]) * e).clamp(0.0, 1.0)
        }),
        ..*base
    }
}

fn add(c: C3, d: C3, k: f32) -> C3 {
    [
        (c[0] + d[0] * k).min(1.0),
        (c[1] + d[1] * k).min(1.0),
        (c[2] + d[2] * k).min(1.0),
    ]
}

/// Draw `m`'s triangles through `c`.
pub fn draw(c: &mut Ctx, b: Bowed, m: &Mesh) {
    let cam = c.cam;
    // The skin's colour, which the hair's edge blends into.
    let scalp = m
        .tris
        .iter()
        .find_map(|(r, _)| match r {
            Role::Skin(i) => Some(hp::real_skin(*i).albedo),
            _ => None,
        })
        .unwrap_or(hp::real_skin(0).albedo);
    for (role, t) in &m.tris {
        let base = match role {
            Role::Skin(i) => hp::real_skin(*i),
            Role::Hair => hp::hair(hp::HAIR),
            r => super::bowed_draw::material(b, *r),
        };
        let warm = matches!(role, Role::Skin(_));
        // Cloth is seen from inside too (a jacket's open front, a collar's turn): its back
        // drawn as its lining, a shade darker.
        let two_sided = matches!(role, Role::Velvet | Role::Wool | Role::Shirt | Role::Tie);
        for tri in &t.idx {
            let [i0, i1, i2] = tri.map(|i| i as usize);
            let q = [t.pts[i0], t.pts[i1], t.pts[i2]];
            let fnrm = (q[1] - q[0]).cross(q[2] - q[0]);
            if fnrm.len() < 1e-8 {
                continue;
            }
            let ctr = (q[0] + q[1] + q[2]) * (1.0 / 3.0);
            let fnrm = fnrm.norm();
            // The smooth normals' mean says which way the triangle faces (its winding may
            // be either after the converter's cuts).
            let sn = (t.nrm[i0] + t.nrm[i1] + t.nrm[i2]).norm();
            let fnrm = if fnrm.dot(sn) < 0.0 { -fnrm } else { fnrm };
            let inside = !cam.facing(ctr, fnrm);
            if inside && !two_sided {
                continue;
            }
            c.keep_facet(&q, fnrm, &base, 1.0);
            let mut cols = [[0.0f32; 3]; 3];
            for (k, &i) in [i0, i1, i2].iter().enumerate() {
                let p = t.pts[i];
                let n = if inside { -t.nrm[i] } else { t.nrm[i] };
                let vv = c.view(p);
                let mat = match role {
                    Role::Skin(_) => skin_at(&base, t.detail[i]),
                    Role::Hair | Role::HairFair => hair_at(&base, t.detail[i], scalp),
                    _ => base,
                };
                let mut col = c.light.shade(&mat, n, vv, p, None, 1.0);
                let ndv = n.dot(vv).clamp(0.0, 1.0);
                if warm {
                    // Light through the skin: a warm glow, more where it turns from the eye.
                    let k = 0.05 + (1.0 - ndv).powi(2) * 0.12;
                    col = add(
                        col,
                        [mat.albedo[0], mat.albedo[1] * 0.5, mat.albedo[2] * 0.3],
                        k,
                    );
                }
                if inside {
                    col = col.map(|x| x * 0.6);
                }
                if warm {
                    // A face full in the key keeps its modelling: the brightest tones eased
                    // (a light theme's key read a fair face as white).
                    col = col.map(|x| if x > 0.62 { 0.62 + (x - 0.62) * 0.45 } else { x });
                }
                if *role == Role::Velvet {
                    // Velvet: its pile catches the light where it turns away.
                    col = add(col, hp::VELVET_SHEEN, (1.0 - ndv).powi(2) * 0.55);
                }
                cols[k] = col;
            }
            c.p.at(cam.depth(ctr), Part::Bowed);
            let sp = c.sps(&q);
            // Overlapping a little more than a pixel: no seam grid on a light theme.
            c.p.poly(sp.clone(), gradient(&sp, &cols), 1.0, 1.0);
        }
    }
}

/// The linear gradient that matches three corner colours best: along the way the
/// brightness changes, from the darkest-projecting corner's colour to the brightest's.
fn gradient(sp: &[egui::Pos2], cols: &[C3; 3]) -> Paint {
    let lum = |c: &C3| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let l = [lum(&cols[0]), lum(&cols[1]), lum(&cols[2])];
    let (e1, e2) = (sp[1] - sp[0], sp[2] - sp[0]);
    let det = e1.x * e2.y - e1.y * e2.x;
    if det.abs() < 1e-4 {
        let m = |k: usize| (cols[0][k] + cols[1][k] + cols[2][k]) / 3.0;
        return Paint::Solid([m(0), m(1), m(2)]);
    }
    // The brightness's gradient over the screen: g·e1 = l1 − l0, g·e2 = l2 − l0.
    let (d1, d2) = (l[1] - l[0], l[2] - l[0]);
    let gx = (d1 * e2.y - d2 * e1.y) / det;
    let gy = (e1.x * d2 - e2.x * d1) / det;
    let g = egui::vec2(gx, gy);
    if g.length() < 1e-5 {
        return Paint::Solid(cols[0]);
    }
    let gn = g.normalized();
    let proj = |p: egui::Pos2| (p.to_vec2()).dot(gn);
    let (mut lo, mut hi) = (0, 0);
    for k in 1..3 {
        if proj(sp[k]) < proj(sp[lo]) {
            lo = k;
        }
        if proj(sp[k]) > proj(sp[hi]) {
            hi = k;
        }
    }
    let a = sp[lo];
    let b = a + gn * (proj(sp[hi]) - proj(sp[lo])).max(0.01);
    super::display::lin(a, b, &[(0.0, cols[lo]), (1.0, cols[hi])])
}
