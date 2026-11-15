//! r40:scene-px, round 40 (visual) — the upright for the photographic renderer: a modern
//! studio upright (`scene::piano_geo`'s): side panels stepped as a real one's (full depth
//! below the keys, the arm round its front, shallow above), the key blocks, the key slip and
//! keys, the fallboard open over the key backs with its rounded lip and nameplate, a music
//! shelf with a sheet of music leaning on the upper panel's raised field, the bottom board
//! with its field and the pedals, the plinth; the top in two leaves on a piano hinge — the
//! front leaf folds back over the rest, showing the frame's top bar, its tuning pins and
//! the strings' tops in the well.

use super::geo::{cm, Geo};
use super::mat::MatId;
use super::mesh::{v3, V3};
use super::piano::Mats;
use crate::scene::piano_geo::{
    upright_leaf, UP_BENCH_DZ, UP_BLOCK, UP_D, UP_H, UP_LEAF_HINGE, UP_LEG, UP_LEG_X, UP_SHELF_Y,
    UP_SIDE, UP_TOE, UP_TOP_TH, UP_W,
};

/// A profile in the (z, y) plane (cm) extruded along x from `xa` to `xb`, every edge round
/// its two faces at `xa` and `xb` rounded to `r` cm (a
/// bullnose, as a real case's edges are): the faces' outlines inset by `r`, each side of
/// the profile meeting them in a quarter round — the edges catch the light as one soft
/// line, never a box's sharp corner (round 40 cp3). The rounds' points are shared at the
/// profile's corners (a closed solid); their normals are each side's own.
fn rounded_prism_x(g: &mut Geo, prof: &[(f32, f32)], xa: f32, xb: f32, r: f32, m: MatId) {
    let pm: Vec<(f32, f32)> = prof.iter().map(|&(z, y)| (z / 100.0, y / 100.0)).collect();
    let (xa, xb, r) = (
        xa / 100.0,
        xb / 100.0,
        (r / 100.0).min((xb - xa) / 200.0 * 0.95),
    );
    let n = pm.len();
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (pm[i], pm[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    let s = if area > 0.0 { 1.0 } else { -1.0 };
    // Each side's outward normal in (z, y), and each corner's mitre (the inset's offset).
    let out: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let (a, b) = (pm[i], pm[(i + 1) % n]);
            let (dz, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dz.hypot(dy).max(1e-9);
            (dy * s / l, -dz * s / l)
        })
        .collect();
    let mitre: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let (o1, o2) = (out[(i + n - 1) % n], out[i]);
            let k = (1.0 + o1.0 * o2.0 + o1.1 * o2.1).max(0.3);
            ((o1.0 + o2.0) / k, (o1.1 + o2.1) / k)
        })
        .collect();
    let inset: Vec<(f32, f32)> = (0..n)
        .map(|i| (pm[i].0 - mitre[i].0 * r, pm[i].1 - mitre[i].1 * r))
        .collect();
    g.plate_map(&inset, |z, y| v3(xa, y, z), v3(-1.0, 0.0, 0.0), m);
    g.plate_map(&inset, |z, y| v3(xb, y, z), v3(1.0, 0.0, 0.0), m);
    const STEPS: usize = 4;
    for i in 0..n {
        let j = (i + 1) % n;
        let o = out[i];
        // θ from the face (0) to the side (π/2), on the face at `x0` leaving along `dx`.
        let at = |k: usize, x0: f32, dx: f32| -> [(V3, V3); 2] {
            let t = std::f32::consts::FRAC_PI_2 * k as f32 / STEPS as f32;
            let (sn, cs) = (t.sin(), t.cos());
            let x = x0 + dx * r * (1.0 - cs);
            let nn = v3(-dx * cs, o.1 * sn, o.0 * sn).norm();
            let pt = |q: usize| {
                let (z, y) = (
                    inset[q].0 + mitre[q].0 * r * sn,
                    inset[q].1 + mitre[q].1 * r * sn,
                );
                (v3(x, y, z), nn)
            };
            [pt(i), pt(j)]
        };
        for k in 0..STEPS {
            g.strip(&at(k, xa, 1.0), &at(k + 1, xa, 1.0), 0.0, r, m);
            g.strip(&at(k + 1, xb, -1.0), &at(k, xb, -1.0), 0.0, r, m);
        }
        let side = v3(0.0, o.1, o.0);
        let (a, b) = (pm[i], pm[j]);
        g.quad_uv(
            [
                v3(xa + r, a.1, a.0),
                v3(xb - r, a.1, a.0),
                v3(xb - r, b.1, b.0),
                v3(xa + r, b.1, b.0),
            ],
            side,
            m,
        );
    }
}

/// A raised field on a front at `z`: a flat middle `rise` proud of it, its four bevels
/// sloping down to the outer rectangle — each catches the light its own way.
#[allow(clippy::too_many_arguments)]
fn field(g: &mut Geo, x0: f32, x1: f32, y0: f32, y1: f32, z: f32, bev: f32, rise: f32, m: MatId) {
    let o = |x: f32, y: f32| cm(x, y, z);
    let i = |x: f32, y: f32| cm(x, y, z + rise);
    let (ix0, ix1, iy0, iy1) = (x0 + bev, x1 - bev, y0 + bev, y1 - bev);
    g.quad_uv(
        [i(ix0, iy0), i(ix1, iy0), i(ix1, iy1), i(ix0, iy1)],
        v3(0.0, 0.0, 1.0),
        m,
    );
    let k = rise / bev;
    for (q, n) in [
        (
            [o(x0, y0), o(x1, y0), i(ix1, iy0), i(ix0, iy0)],
            v3(0.0, -k, 1.0),
        ),
        (
            [i(ix0, iy1), i(ix1, iy1), o(x1, y1), o(x0, y1)],
            v3(0.0, k, 1.0),
        ),
        (
            [o(x0, y0), i(ix0, iy0), i(ix0, iy1), o(x0, y1)],
            v3(-k, 0.0, 1.0),
        ),
        (
            [i(ix1, iy0), o(x1, y0), o(x1, y1), i(ix1, iy1)],
            v3(k, 0.0, 1.0),
        ),
    ] {
        g.quad_uv(q, n.norm(), m);
    }
}

pub fn build(g: &mut Geo, m: &Mats, lid: f32) {
    let (x0, x1, zb) = (-UP_W / 2.0, UP_W / 2.0, -UP_D);
    let (ix0, ix1) = (x0 + 3.0, x1 - 3.0);
    let well = UP_H - 22.0;
    let open = lid > 0.2;
    // The side panels.
    rounded_prism_x(g, &UP_SIDE, x0, ix0, 0.9, m.case);
    rounded_prism_x(g, &UP_SIDE, ix1, x1, 0.9, m.case);
    // The front legs under the arms, tapering to their toe blocks on casters.
    for (la, lb) in UP_LEG_X {
        rounded_prism_x(g, &UP_LEG, la, lb, 0.9, m.case);
        rounded_prism_x(g, &UP_TOE, la - 1.0, lb + 1.0, 0.6, m.case);
        g.tube(
            cm(la + 0.4, 1.2, 20.0),
            cm(lb - 0.4, 1.2, 20.0),
            0.012,
            0.012,
            16,
            true,
            m.rubber,
        );
    }
    // The carcass behind the fronts: solid below the well, the back block behind it.
    g.aabb(cm(ix0, 7.0, zb), cm(ix1, well, -2.0), m.case);
    g.aabb(cm(ix0, well, zb), cm(ix1, UP_H, UP_LEAF_HINGE), m.case);
    // The well's floor, the frame's top bar with its pins, the strings' tops.
    g.aabb(
        cm(ix0, well - 0.5, UP_LEAF_HINGE),
        cm(ix1, well, -2.0),
        m.keybed,
    );
    if open {
        well_inside(g, m, ix0, ix1, well);
    }
    // The upper panel and its raised field.
    g.aabb(cm(ix0, 84.0, -2.0), cm(ix1, UP_H, 0.0), m.case);
    field(g, -58.0, 58.0, 91.0, UP_H - 7.0, 0.0, 3.0, 0.7, m.case);
    // The top: its back leaf, the piano hinge, the front leaf at its fold.
    g.aabb(
        cm(x0 - 1.0, UP_H, zb - 0.5),
        cm(x1 + 1.0, UP_H + UP_TOP_TH, UP_LEAF_HINGE),
        m.case,
    );
    let to_m = |p: crate::scene::geo::V3| cm(p.x, p.y, p.z);
    let c = upright_leaf(lid).map(to_m);
    g.boxed(c[0], c[1] - c[0], c[3] - c[0], c[4] - c[0], m.case);
    // Its front edge a bullnose.
    g.tube(
        (c[3] + c[7]) * 0.5,
        (c[2] + c[6]) * 0.5,
        UP_TOP_TH / 200.0,
        UP_TOP_TH / 200.0,
        16,
        true,
        m.case,
    );
    g.tube(
        cm(x0 + 2.0, UP_H + UP_TOP_TH, UP_LEAF_HINGE),
        cm(x1 - 2.0, UP_H + UP_TOP_TH, UP_LEAF_HINGE),
        0.0035,
        0.0035,
        8,
        true,
        m.brass,
    );
    // The bottom board with its field, the plinth.
    g.aabb(cm(ix0, 7.0, -2.0), cm(ix1, 64.0, 3.0), m.case);
    field(g, -54.0, 54.0, 14.0, 58.0, 3.0, 2.6, 0.6, m.case);
    g.aabb(cm(ix0, 0.0, -2.0), cm(ix1, 7.0, 5.0), m.case);
    // The keybed, the key slip, the key blocks.
    g.aabb(cm(ix0, 64.0, -2.0), cm(ix1, 70.6, 23.4), m.case);
    g.aabb(cm(x0 + 15.0, 65.6, 22.6), cm(x1 - 15.0, 72.4, 24.2), m.case);
    rounded_prism_x(g, &UP_BLOCK, ix0, x0 + 15.0, 0.8, m.case);
    rounded_prism_x(g, &UP_BLOCK, x1 - 15.0, ix1, 0.8, m.case);
    keys(g, m, x0 + 15.0, x1 - 15.0);
    fallboard(g, m, x0 + 15.0, x1 - 15.0);
    shelf(g, m);
    // The bench in the piano's finish, where the player sits.
    let (first, first_rod) = (g.tris.len(), g.rods.len());
    super::piano::bench(g, m);
    let dz = v3(0.0, 0.0, UP_BENCH_DZ / 100.0);
    for t in &mut g.tris[first..] {
        for v in &mut t.v {
            v.p = v.p + dz;
        }
    }
    for r in &mut g.rods[first_rod..] {
        (r.a, r.b) = (r.a + dz, r.b + dz);
    }
    // The pedals out of the bottom board, on their trim.
    g.aabb(cm(-13.0, 2.6, 3.0), cm(13.0, 7.4, 3.6), m.keybed);
    for px in [-8.0, 0.0, 8.0] {
        g.tube(
            cm(px, 5.0, 3.4),
            cm(px, 4.2, 12.5),
            0.011,
            0.009,
            12,
            true,
            m.brass,
        );
    }
}

/// The keys between the blocks: white, then black with their sloped fronts; the red felt.
fn keys(g: &mut Geo, m: &Mats, xa: f32, xb: f32) {
    let wk = (xb - xa) / 52.0;
    for i in 0..52 {
        let x = xa + i as f32 * wk;
        g.aabb(
            cm(x + 0.11, 70.8, 6.0),
            cm(x + wk - 0.11, 72.6, 22.2),
            m.ivory,
        );
    }
    let pat = [true, false, true, true, false, true, true];
    for i in 0..51 {
        if !pat[i % 7] {
            continue;
        }
        let x = xa + (i + 1) as f32 * wk - wk * 0.3;
        g.aabb(cm(x, 72.6, 6.0), cm(x + wk * 0.6, 73.9, 15.0), m.ebony);
        g.boxed(
            cm(x + 0.1, 73.9, 15.0),
            cm(wk * 0.6 - 0.2, 0.0, 0.0),
            cm(0.0, -1.3, 0.5),
            cm(0.0, 0.0, -0.6),
            m.ebony,
        );
    }
    g.tube(
        cm(xa, 72.9, 5.7),
        cm(xb, 72.9, 5.7),
        0.004,
        0.004,
        8,
        false,
        m.red,
    );
}

/// The fallboard open over the key backs: a board in a gentle curve from its rounded lip
/// up to the shelf, a small brass nameplate on it.
fn fallboard(g: &mut Geo, m: &Mats, xa: f32, xb: f32) {
    let along = cm(xb - xa, 0.0, 0.0);
    let pts = [cm(xa, 73.4, 6.0), cm(xa, 78.8, 5.1), cm(xa, 84.0, 2.2)];
    for w in pts.windows(2) {
        let up = w[1] - w[0];
        let n = up.cross(along).norm();
        g.boxed(w[0], along, up, n * 0.016, m.case);
    }
    g.tube(
        cm(xa, 73.6, 5.6),
        cm(xb, 73.6, 5.6),
        0.0095,
        0.0095,
        16,
        true,
        m.case,
    );
    // Its top edge a bullnose the thickness of the board (a box's corner read sharp).
    {
        let up = pts[2] - pts[1];
        let n = up.cross(along).norm();
        let e = pts[2] + n * 0.008;
        g.tube(e, e + along, 0.0082, 0.0082, 16, true, m.case);
    }
    let (lo, hi) = (pts[0], pts[1]);
    let up = hi - lo;
    let n = up.cross(along).norm();
    let mid = lo + along * 0.5 + up * 0.62 - n * 0.0008;
    let e = along.norm();
    let u = up.norm();
    g.boxed(
        mid - e * 0.05 - u * 0.004,
        e * 0.10,
        u * 0.008,
        n * -0.0012,
        m.brass,
    );
}

/// The music shelf, its lip, and a sheet of music leaning on the upper panel.
fn shelf(g: &mut Geo, m: &Mats) {
    g.aabb(cm(-50.0, 84.0, 0.0), cm(50.0, UP_SHELF_Y, 5.5), m.case);
    g.aabb(
        cm(-50.0, UP_SHELF_Y, 4.6),
        cm(50.0, UP_SHELF_Y + 1.5, 5.5),
        m.case,
    );
    let (b, t) = (cm(-22.0, UP_SHELF_Y + 0.1, 4.3), cm(-22.0, 113.0, 1.0));
    sheet(g, m, b, cm(44.0, 0.0, 0.0), t - b);
}

/// A sheet of music: two pages from `o` along `across` and `up`, the fold, five systems of
/// staves on each.
pub fn sheet(g: &mut Geo, m: &Mats, o: V3, across: V3, up: V3) {
    let n = across.cross(up).norm();
    let n = if n.z < 0.0 { -n } else { n };
    g.boxed(o, across, up, n * 0.0015, m.paper);
    let face = |a: f32, t: f32| o + across * a + up * t + n * 0.0018;
    // The staves printed on it: thin bands of ink (wires this fine vanish), the fold.
    let ink = |a0: f32, a1: f32, t: f32, h: f32, g: &mut Geo| {
        let o = face(a0, t);
        g.boxed(o, across * (a1 - a0), up * h, n * 0.0002, m.ink);
    };
    ink(0.497, 0.503, 0.02, 0.96, g);
    for (a0, a1) in [(0.06, 0.45), (0.55, 0.94)] {
        for s in 0..5 {
            let top = 0.9 - s as f32 * 0.17;
            for l in 0..5 {
                ink(a0, a1, top - l as f32 * 0.022, 0.004, g);
            }
            // A few notes on the staff.
            for k in 0..6 {
                let a = a0 + 0.04 + k as f32 * (a1 - a0 - 0.08) / 5.0;
                let t = top - 0.088 + ((k * 7 + s * 3) % 5) as f32 * 0.011;
                ink(a, a + 0.016, t, 0.012, g);
            }
        }
    }
}

/// What the open front leaf shows: the frame's top bar along the back of the well, two
/// rows of tuning pins out of it, the strings running down from them.
fn well_inside(g: &mut Geo, m: &Mats, ix0: f32, ix1: f32, well: f32) {
    let zw = UP_LEAF_HINGE;
    g.aabb(
        cm(ix0 + 5.0, UP_H - 13.0, zw),
        cm(ix1 - 5.0, UP_H - 1.0, zw + 2.5),
        m.gold,
    );
    let n = 64;
    for i in 0..n {
        let x = ix0 + 9.0 + i as f32 * ((ix1 - ix0 - 18.0) / (n - 1) as f32);
        for (row, y) in [(0, UP_H - 4.0), (1, UP_H - 7.5)] {
            let dx = if row == 1 { 0.9 } else { 0.0 };
            g.tube(
                cm(x + dx, y, zw + 2.5),
                cm(x + dx, y, zw + 4.6),
                0.0035,
                0.0035,
                8,
                true,
                m.nickel,
            );
        }
        let (mat, r) = if i < 16 {
            (m.copper, 0.0018)
        } else {
            (m.steel, 0.0008)
        };
        g.line(
            cm(x, UP_H - 9.5, zw + 3.0),
            cm(x, well + 0.2, zw + 3.4),
            r,
            mat,
            0.8,
        );
    }
}
