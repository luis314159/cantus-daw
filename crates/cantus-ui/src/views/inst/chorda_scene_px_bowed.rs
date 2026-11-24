//! r42:arcus-look — Arcus's cello and double bass for the photographic renderer: the scene
//! module's surfaces (`scene::bowed_mesh`, the same the Drawn picture is built from) as
//! smooth triangles — each grid cell two, every corner its own normal, so the arching and
//! the scroll's turns shade round — in their materials: the spruce top's quartered grain
//! and the maple's flame (`Tex::Tonewood`) under the varnish's clear coat, ebony, the
//! bridge's pale maple, nickel, brass, the strings' steel and the silks; the f-holes and
//! the bridge's cut-outs dark, the strings and rods round.

use super::geo::{ear_clip, of, perp, vt, Geo};
use super::mat::{finish, lin3, parts, MatId, PMat, Tex};
use super::mesh::V3;
use crate::kit::skin::scene::bowed as pal;
use crate::scene::bowed::Bowed;
use crate::scene::bowed_mesh::{mesh, Role};
use std::collections::HashMap;

/// The photographic material of a role.
fn pmat(b: Bowed, r: Role) -> PMat {
    // The varnish seen per pixel: its colour deeper than the Drawn picture's flat facets
    // need (the clear coat over it adds its own light).
    let tone = |m: &crate::kit::skin::scene::Mat, lines: f32, flame: f32, seed: u32| {
        let f = finish(m, seed);
        // Deeper and more saturated (red kept, blue taken): under the clear coat's white
        // reflection a varnish's orange must still read orange, not brown or salmon.
        let k = |c: [f32; 3]| [c[0] * 0.56, c[1] * 0.40, c[2] * 0.26];
        PMat {
            tex: Tex::Tonewood { seed, lines, flame },
            base: k(f.base),
            dark: k(f.dark),
            light: k(f.light),
            coat: f.coat * 0.5,
            rough: (f.rough + 0.08).min(0.4),
            ..f
        }
    };
    let (top, maple) = match b {
        Bowed::Cello => (pal::CELLO_TOP, pal::CELLO_MAPLE),
        Bowed::Bass => (pal::BASS_TOP, pal::BASS_MAPLE),
        Bowed::Violin => (pal::VIOLIN_TOP, pal::VIOLIN_MAPLE), // r42:arcus-upper
        Bowed::Viola => (pal::VIOLA_TOP, pal::VIOLA_MAPLE),    // r42:arcus-upper
    };
    match r {
        Role::Top => tone(&top, 520.0, 0.0, 71),
        Role::Maple => tone(&maple, 260.0, 1.0, 73),
        Role::Inside => {
            let mut m = tone(&maple, 260.0, 1.0, 73);
            m.base = m.base.map(|c| c * 0.4);
            m.dark = m.dark.map(|c| c * 0.4);
            m.light = m.light.map(|c| c * 0.4);
            m
        }
        Role::Ebony | Role::Purfling => PMat::lacquer(lin3(pal::EBONY.albedo), 0.75, 0.10),
        Role::Bridge => PMat {
            tex: Tex::Tonewood {
                seed: 77,
                lines: 380.0,
                flame: 0.3,
            },
            ..finish(&pal::BRIDGE, 77)
        },
        Role::BridgeCut => PMat::matte(lin3(top.dark).map(|c| c * 0.35)),
        Role::Nickel => parts::nickel(),
        Role::Brass => parts::brass(),
        Role::Rubber => parts::rubber(),
        Role::Hole => PMat::matte(lin3(pal::HOLE)),
        Role::Cut => PMat::matte(lin3(pal::CUT)),
        Role::Silk(i) => PMat::matte(lin3(pal::SILK[(i as usize).min(3)])).with(Tex::Fabric),
        Role::Steel => parts::steel(),
        Role::Wound => PMat::metal(lin3(pal::TUNGSTEN.tint), 0.22),
        Role::Chair => finish(&crate::kit::skin::scene::FINISH_WALNUT, 91),
        Role::Seat => PMat::matte(lin3(pal::SEAT.albedo)).with(Tex::Fabric),
    }
}

/// `b` added to `g` (its triangles after the room's).
pub fn build(g: &mut Geo, b: Bowed) {
    let m = mesh(b);
    let mut ids: HashMap<Role, MatId> = HashMap::new();
    let mut id = |g: &mut Geo, r: Role| *ids.entry(r).or_insert_with(|| g.mat(pmat(b, r)));
    let uvm = |uv: [f32; 2]| [uv[0] / 100.0, uv[1] / 100.0];
    for (role, p) in &m.patches {
        let mi = id(g, *role);
        for cell in p.cells() {
            let v: [_; 4] = std::array::from_fn(|k| {
                vt(of(p.pts[cell[k]]), of(p.nrm[cell[k]]), uvm(p.uv[cell[k]]))
            });
            let area = |a: V3, b: V3, c: V3| (b - a).cross(c - a).len();
            if area(v[0].p, v[1].p, v[2].p) > 1e-10 {
                g.tri(v[0], v[1], v[2], mi);
            }
            if area(v[0].p, v[2].p, v[3].p) > 1e-10 {
                g.tri(v[0], v[2], v[3], mi);
            }
        }
    }
    for (role, poly) in &m.polys {
        let mi = id(g, *role);
        let n = of(poly.n).norm();
        let a = perp(n);
        let c = n.cross(a).norm();
        let pts: Vec<V3> = poly.pts.iter().map(|&q| of(q)).collect();
        let flat: Vec<(f32, f32)> = pts.iter().map(|q| (q.dot(a), q.dot(c))).collect();
        for t in ear_clip(&flat) {
            let w = t.map(|i| vt(pts[i], n, [flat[i].0, flat[i].1]));
            g.tri(w[0], w[1], w[2], mi);
        }
    }
    for (role, r) in &m.rods {
        let mi = id(g, *role);
        g.rod(of(r.a), of(r.b), r.r / 100.0, mi);
    }
}
