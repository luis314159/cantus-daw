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
pub fn skin_at(base: &Mat, d: [f32; 2]) -> Mat {
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
fn hair_at(base: &Mat, d: [f32; 2], scalp: C3, short: bool) -> Mat {
    let across = d[0];
    // r42:arcus-polish4 — his (`short`): broader value bands along the combing (the
    // coordinator: one smooth shell), and his short sides' roots with more of the skin.
    let bands = if short { 2.0 } else { 1.0 };
    let k = 0.9
        + 0.07 * bands * (across * 2.1).sin()
        + 0.05 * bands * (across * 5.3 + 1.7).sin()
        + 0.04 * (across * 13.0 + 0.4).sin();
    // r42:arcus-polish2 — the edge blends over the first third of the band only, and into
    // the hair's own deeper roots with a little of the skin (blended into the skin over the
    // whole band, her hairline read as a pale pink stripe across the temple — the owner's
    // review of the Drawn picture).
    let e = (d[1] / 0.35).clamp(0.0, 1.0).powf(0.6);
    let root: C3 = if short {
        std::array::from_fn(|i| base.albedo[i] * 0.45 + scalp[i] * 0.5)
    } else {
        std::array::from_fn(|i| base.albedo[i] * 0.62 * 0.7 + scalp[i] * 0.3)
    };
    // The roots a little deeper for a while in from the edge.
    let deep = 1.0 - 0.16 * (1.0 - (d[1] / 0.6).clamp(0.0, 1.0));
    Mat {
        albedo: std::array::from_fn(|i| {
            (root[i] + (base.albedo[i] * k * deep - root[i]) * e).clamp(0.0, 1.0)
        }),
        ..*base
    }
}

/// Skin lit: the light's brightness on it in the skin's own hue (a light theme's cool
/// ambient turned a face grey-green), a warm glow of light through it (more where it turns
/// from the eye), the brightest eased so a face full in the key keeps its modelling, and
/// never past white in one channel (a clipped red turned it yellow).
pub fn skin_tone(mat: &Mat, shaded: C3, ndv: f32) -> C3 {
    let lum = |c: C3| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let a = mat.albedo;
    let k = lum(shaded) / lum(a).max(1e-3);
    let glow = 0.05 + (1.0 - ndv).powi(2) * 0.12;
    let mut c = [
        a[0] * (k + glow),
        a[1] * (k + glow * 0.5),
        a[2] * (k + glow * 0.3),
    ];
    let l = lum(c);
    if l > 0.5 {
        let e = (0.5 + (l - 0.5) * 0.4) / l;
        c = c.map(|x| x * e);
    }
    let top = c[0].max(c[1]).max(c[2]);
    if top > 0.97 {
        c = c.map(|x| x * 0.97 / top);
    }
    c
}

/// r42:arcus-polish3 — skin under the concert stage's spot: as [`skin_tone`], but only the
/// very brightest eased (eased from the middle up, a face full in the spot read flat and
/// pale: the modelling is the spot's, its lit side must stay bright).
pub fn stage_skin(mat: &Mat, shaded: C3, ndv: f32) -> C3 {
    let lum = |c: C3| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let a = mat.albedo;
    let k = lum(shaded) / lum(a).max(1e-3);
    // r42:arcus-polish4 — the stage's skin in its own hue (`Concert::d_skin`, its value
    // kept): the drawn face read a pale beige beside the photographs' warm peach.
    let g = crate::scene::figure::concert::get().d_skin;
    let ga = [a[0] * g[0], a[1] * g[1], a[2] * g[2]];
    let a = ga.map(|x| x * lum(a) / lum(ga).max(1e-4));
    let glow = 0.02 + (1.0 - ndv).powi(2) * 0.08;
    let mut c = [
        a[0] * (k + glow),
        a[1] * (k + glow * 0.45),
        a[2] * (k + glow * 0.25),
    ];
    let l = lum(c);
    if l > 0.78 {
        let e = (0.78 + (l - 0.78) * 0.3) / l;
        c = c.map(|x| x * e);
    }
    let top = c[0].max(c[1]).max(c[2]);
    if top > 0.97 {
        c = c.map(|x| x * 0.97 / top);
    }
    c
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
        // r42:arcus-upper — the cards are the Photographic picture's (this one draws no
        // texture; the hair's shell and the painted brows stand for them).
        if matches!(role, Role::Card(_) | Role::Scalp(_)) {
            continue;
        }
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
                    Role::Hair | Role::HairFair | Role::Shell(_) => {
                        hair_at(&base, t.detail[i], scalp, *role == Role::Shell(1))
                    }
                    _ => base,
                };
                let mut col = c.light.shade(&mat, n, vv, p, None, 1.0);
                let ndv = n.dot(vv).clamp(0.0, 1.0);
                if matches!(role, Role::HairFair | Role::Shell(_)) {
                    // r42:arcus-polish2 — the underlayer: hair turned down lies in its own
                    // shade (her Drawn hair read as one flat yellow cap).
                    let under = 1.0 - 0.3 * ((0.25 - n.y) / 0.9).clamp(0.0, 1.0);
                    col = col.map(|x| x * under);
                }
                if warm {
                    col = if c.light.concert.is_some() {
                        stage_skin(&mat, col, ndv) // r42:arcus-polish3
                    } else {
                        skin_tone(&mat, col, ndv)
                    };
                }
                if inside {
                    col = col.map(|x| x * 0.6);
                }
                if matches!(role, Role::Shirt | Role::Tie) {
                    // r42:arcus-polish2 — the shade under the collar's and lapels' edges.
                    let k = t.detail[i][0].clamp(0.3, 1.2);
                    col = col.map(|x| x * k);
                }
                if *role == Role::Velvet {
                    // Velvet: its pile catches the light where it turns away.
                    col = add(col, hp::VELVET_SHEEN, (1.0 - ndv).powi(2) * 0.55);
                }
                // r42:arcus-polish3 — the concert's back lights: a clean warm line along
                // the edges that turn toward them (the hair, the shoulders, the face's
                // edge), as a stage photograph's rim.
                let rim = c.light.rim(n, vv);
                if rim > 0.0 && !inside {
                    let k = match role {
                        Role::Skin(_) => 0.35,
                        Role::Velvet | Role::Wool => 0.5,
                        // r42:arcus-polish4 — his dark hair takes the varnish's rim already
                        // (`Light::shade`): more read as a bright cap's rim over the crown.
                        Role::Shell(1) => 0.25,
                        _ => 1.0,
                    };
                    col = add(col, [1.0, 0.88, 0.72], rim * k);
                }
                cols[k] = col;
            }
            // r42:arcus-polish4 — the hair's shell a little nearer than the skin it lies on:
            // at its thin edge the two lie within a millimetre and, sorted by their middles,
            // the skin's triangles painted over the hair in places (his crown's and temples'
            // light flecks in the Drawn picture).
            // (And his shirt and tie a little nearer: the collar's far point lies over the
            // violin's edge, and sorted by their middles its triangles and the violin's cells
            // interleaved — white shards over the violin, the coordinator's review.)
            let bias = match role {
                Role::Shell(_) => SHELL_BIAS,
                Role::Shirt | Role::Tie => SHIRT_NEAR,
                _ => 0.0,
            };
            c.p.at(cam.depth(ctr) - bias, Part::Bowed);
            let sp = c.sps(&q);
            // Overlapping a little more than a pixel: no seam grid on a light theme.
            c.p.poly(sp.clone(), gradient(&sp, &cols), 1.0, 1.0);
            // (Not over the hair's edge band, where it blends into its roots: light strands
            // there read as white dashes along the hairline.)
            let edge = [i0, i1, i2].iter().any(|&i| t.detail[i][1] < 0.45);
            // (r42:arcus-polish4: his too — combed straight back, the strands' lines run
            // from the hairline over the crown.)
            if !inside && !edge && matches!(role, Role::HairFair | Role::Shell(_)) {
                strand_lines(c, &sp, [i0, i1, i2].map(|i| t.detail[i][0]), &cols);
            }
        }
    }
}

/// r42:arcus-polish2 — her hair's drawn strands (the owner: "a few drawn strand lines,
/// hatches … in the Drawn look's own style: clean vector shapes"): over each triangle of
/// her hair, fine lines where its strands' across coordinate passes a strand's (every
/// [`STRAND_STEP`], unevenly), so they run as the hair is combed — back from the hairline
/// over the head, wound round the bun — each a shade deeper or lighter than the hair under
/// it, half seen.
fn strand_lines(c: &mut Ctx, sp: &[egui::Pos2], across: [f32; 3], cols: &[C3; 3]) {
    use super::display::Cap;
    let (lo, hi) = (
        across.iter().copied().fold(f32::MAX, f32::min),
        across.iter().copied().fold(f32::MIN, f32::max),
    );
    if hi - lo < 1e-4 || hi - lo > 40.0 * STRAND_STEP {
        return;
    }
    let mean: C3 = std::array::from_fn(|k| (cols[0][k] + cols[1][k] + cols[2][k]) / 3.0);
    let mut s = (lo / STRAND_STEP).ceil();
    while s * STRAND_STEP <= hi {
        let a = s * STRAND_STEP;
        // Its crossings of the triangle's three edges.
        let mut ends = [egui::Pos2::ZERO; 2];
        let mut n = 0;
        for e in 0..3 {
            let (i, j) = (e, (e + 1) % 3);
            let (u, v) = (across[i], across[j]);
            if (u - a) * (v - a) < 0.0 && n < 2 {
                let t = (a - u) / (v - u);
                ends[n] = sp[i] + (sp[j] - sp[i]) * t;
                n += 1;
            }
        }
        if n == 2 {
            // Each strand its own: deeper or lighter, some faint (a hash of its number).
            let h = ((s as i32).wrapping_mul(1_103_515_245).wrapping_add(12_345) >> 8) & 255;
            let light = h % 3 == 0;
            let k = if light { 1.22 } else { 0.66 };
            let col: C3 = mean.map(|x| (x * k).min(1.0));
            let op = 0.4 + 0.3 * ((h >> 2) % 4) as f32 / 3.0;
            let w = if light { 0.45 } else { 0.6 };
            c.p.line(ends[0], ends[1], w, Paint::Solid(col), Cap::Butt, op);
        }
        s += 1.0;
    }
}

/// r42:arcus-polish4 — how much nearer the hair's shell sorts than its own depth (cm).
const SHELL_BIAS: f32 = 0.6;
/// r42:arcus-polish4 — how much nearer his shirt and tie sort (cm).
const SHIRT_NEAR: f32 = 1.5;

/// How far apart (in the hair's across coordinate: about three degrees round the
/// head) her drawn strands lie.
const STRAND_STEP: f32 = 0.5;

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
