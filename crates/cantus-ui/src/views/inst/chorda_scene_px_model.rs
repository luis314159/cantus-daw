//! r40:scene-px — the scene module's [`Model`] (its captured 3-D primitives: the facets,
//! tubes and flat paints the vector picture is drawn from) as closed triangles for the
//! photographic renderer: each facet fanned, its corners welded with their neighbours of
//! the same material so a curved rim built of facets shades smooth (a long highlight runs
//! unbroken along it), each tube a round rod, each paint its own light or matte colour;
//! the flat shadows the vector picture paints are left out (the bake casts its own).

use super::geo::{of, vt, Geo, Vert};
use super::mat::{finish, lin3, MatId, PMat};
use super::mesh::{v3, V3};
use crate::kit::skin::scene::Mat;
use crate::scene::display::Part;
use crate::scene::{Model, Prim};
use std::collections::HashMap;

/// Normals closer than this (cos) are averaged where facets meet.
const SMOOTH: f32 = 0.82;

struct Mats {
    known: Vec<(Mat, bool, MatId)>,
}

impl Mats {
    fn of(&mut self, g: &mut Geo, m: &Mat, under: bool) -> MatId {
        if let Some(&(_, _, id)) = self.known.iter().find(|(k, u, _)| k == m && *u == under) {
            return id;
        }
        let mut p = finish(m, 17 + self.known.len() as u32);
        p.lid_under = under;
        let id = g.mat(p);
        self.known.push((*m, under, id));
        id
    }
}

/// An axis pair in the plane of `n` (for a facet's uv, in metres).
fn plane_axes(n: V3) -> (V3, V3) {
    let a = super::geo::perp(n);
    (a, n.cross(a).norm())
}

/// The instrument of `model` added to `g` (its triangles after the room's).
pub fn build(g: &mut Geo, model: &Model) {
    let mut mats = Mats { known: Vec::new() };
    let start = g.tris.len();
    for p in &model.prims {
        match p {
            Prim::Facet {
                pts, n, mat, part, ..
            } => {
                if pts.len() < 3 {
                    continue;
                }
                let n = of(*n);
                let n = if n.len() > 1e-6 {
                    n.norm()
                } else {
                    v3(0.0, 1.0, 0.0)
                };
                let under = *part == Part::Lid && n.y < -0.2;
                let m = mats.of(g, mat, under);
                let (a, b) = plane_axes(n);
                let w: Vec<Vert> = pts
                    .iter()
                    .map(|&q| {
                        let q = of(q);
                        vt(q, n, [q.dot(a), q.dot(b)])
                    })
                    .collect();
                for k in 1..w.len() - 1 {
                    g.tri(w[0], w[k], w[k + 1], m);
                }
            }
            Prim::Tube { a, b, r, mat, .. } => {
                let m = mats.of(g, mat, false);
                let r = r / 100.0;
                let sides = if r > 0.02 { 20 } else { 12 };
                let first = g.tris.len() as u32;
                g.tube(of(*a), of(*b), r, r, sides, true, m);
                // A rod like the photographic builders' own (a thin one is drawn as a
                // tube stroke).
                g.rods.push(super::geo::Rod {
                    a: of(*a),
                    b: of(*b),
                    r,
                    m,
                    tris: (first, g.tris.len() as u32),
                });
            }
            Prim::Flat {
                pts, col, alpha, ..
            } => {
                // The vector picture's painted shadows and glazes: the bake makes its own.
                if *alpha < 0.95 || pts.len() < 3 {
                    continue;
                }
                let c = lin3(*col);
                let bright = c[0].max(c[1]).max(c[2]);
                let m = if bright > 0.6 {
                    g.mat(PMat::emit(c.map(|v| v * 2.0)))
                } else {
                    g.mat(PMat::matte(c))
                };
                let q: Vec<V3> = pts.iter().map(|&q| of(q)).collect();
                let n = (q[1] - q[0]).cross(q[2] - q[0]).norm();
                let (a, b) = plane_axes(n);
                let w: Vec<Vert> = q.iter().map(|&q| vt(q, n, [q.dot(a), q.dot(b)])).collect();
                for k in 1..w.len() - 1 {
                    g.tri(w[0], w[k], w[k + 1], m);
                }
            }
        }
    }
    weld(g, start);
}

/// Average the normals of corners that meet (same place, same material, gently bent).
fn weld(g: &mut Geo, start: usize) {
    let key = |p: V3, m: MatId| {
        (
            (p.x * 2000.0).round() as i32,
            (p.y * 2000.0).round() as i32,
            (p.z * 2000.0).round() as i32,
            m,
        )
    };
    let mut at: HashMap<(i32, i32, i32, MatId), Vec<V3>> = HashMap::new();
    for t in &g.tris[start..] {
        for v in &t.v {
            let e = at.entry(key(v.p, t.m)).or_default();
            if !e.iter().any(|n| n.dot(v.n) > 0.9999) {
                e.push(v.n);
            }
        }
    }
    for t in &mut g.tris[start..] {
        let m = t.m;
        for v in &mut t.v {
            if let Some(ns) = at.get(&key(v.p, m)) {
                let mut s = V3::default();
                for &n in ns {
                    if n.dot(v.n) > SMOOTH {
                        s = s + n;
                    }
                }
                if s.len() > 1e-6 {
                    v.n = s.norm();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::{build, Room, SceneBackend, Subject, Want};
    use cantus_core::inst::chorda_look::{Body, Finish, Look};
    use cantus_core::inst::room_look::RoomLook;

    /// A model with no subject (a scene a later version of the module describes only by its
    /// primitives) still makes a picture: its facets welded, its tubes round, its own room.
    #[test]
    fn a_model_of_primitives_alone_is_baked() {
        let want = Want {
            subject: Subject::Piano {
                look: Look::new(Body::Grand, Finish::Black),
                lid: 1.0,
                persp: 0.35,
                width: 0.7,
                room_level: 0.35,
                mics: true,
            },
            room: Room::Stage,
            look: RoomLook::default(),
            dark: true,
            size: egui::vec2(240.0, 144.0),
            ppp: 1.0,
            backend: SceneBackend::PerPixel,
            frame: None,
            quick: false,
        };
        let mut model = build(&want)
            .model
            .expect("a per-pixel build keeps its model");
        assert!(!model.prims.is_empty());
        model.subject = None;
        let q = super::super::Quality {
            ss: 1,
            full: true,
            shadow: 512,
            trace: true,
        };
        let (img, st) = super::super::render_of(&model, 1.0, q, 2);
        assert_eq!(img.size, [240, 144]);
        assert!(
            st.tris > 1000,
            "the primitives became triangles ({})",
            st.tris
        );
        let lum: Vec<f32> = img
            .pixels
            .iter()
            .map(|c| c.r() as f32 * 0.3 + c.g() as f32 * 0.6 + c.b() as f32 * 0.1)
            .collect();
        let mean = lum.iter().sum::<f32>() / lum.len() as f32;
        let sd = (lum.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / lum.len() as f32).sqrt();
        assert!(sd > 4.0, "a picture, not a flat fill (spread {sd})");
    }
}
