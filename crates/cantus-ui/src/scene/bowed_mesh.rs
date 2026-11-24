//! r42:arcus-look — a bowed instrument as surfaces, in the world (cm), for both of the scene
//! engine's pictures: grids of points with their normals and grain coordinates (the plates,
//! the ribs, the neck, the fingerboard, the scroll's volute), flat polygons (the f-holes,
//! the bridge, the purfling), rods (the endpin, the pegs' shafts) and wires (the strings).
//! The Drawn picture (`bowed_draw`) shades each grid cell as a facet; the Photographic one
//! (`views/inst/chorda_scene_px_bowed.rs`) bakes the same cells as smooth triangles.
//! Built once per instrument and kept (`mesh`): the picture is rebuilt only when the
//! scene's size, theme or look changes, never per frame.

use super::bowed::{arch, dist, outline, reach, rib, Bowed, Dims, Pose};
use super::bowed_hold::{Hold, Holder}; // r42:arcus-upper
use super::geo::V3;
use std::sync::{Arc, Mutex, OnceLock};

/// What a surface is made of.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The varnished spruce top.
    Top,
    /// The varnished flamed maple: back, ribs, neck, pegbox, scroll.
    Maple,
    /// The maple inside the pegbox, in its own shade.
    Inside,
    /// Ebony: fingerboard, nut, pegs, tailpiece, saddle, endpin collar.
    Ebony,
    /// The bridge's pale maple.
    Bridge,
    /// Through the bridge's cut-outs: the top behind it, in the bridge's shade.
    BridgeCut,
    /// Nickel: fine tuners, the endpin, a bass's machines.
    Nickel,
    /// A bass's machines' brass plates.
    Brass,
    Rubber,
    /// Through an f-hole: inside the box.
    Hole,
    /// The f-holes' pale cut edge.
    Cut,
    Purfling,
    /// A string's silk wrapping (by string).
    Silk(u8),
    /// A plain steel string, a wound one.
    Steel,
    Wound,
    /// The player's chair or stool: its stained wood, its seat's cloth.
    Chair,
    Seat,
    /// What holds it (`bowed_hold`, r42:arcus-upper): a stand's steel and pads; the
    /// musician's skin, clothes (by tone), trousers, shoes, hair, the face's strokes and
    /// cheeks; the wooden mannequin and its joints (by tone).
    Stand,
    Pad,
    Skin(u8),
    /// Skin in its own shade (the neck under the jaw).
    SkinShade(u8),
    Cloth(u8),
    Trousers,
    Shoes,
    Hair,
    Feature,
    Blush,
    Wood(u8),
    Joint(u8),
}

/// A grid of points (`rows` × `cols`, row-major) with normals and grain coordinates (cm:
/// along the grain, across it).
#[derive(Clone, Debug, Default)]
pub struct Patch {
    pub pts: Vec<V3>,
    pub nrm: Vec<V3>,
    pub uv: Vec<[f32; 2]>,
    pub rows: usize,
    pub cols: usize,
    /// The last column joins the first (a closed ring).
    pub closed: bool,
}

/// A flat polygon (possibly concave) lying on or just over a surface.
#[derive(Clone, Debug)]
pub struct Poly {
    pub pts: Vec<V3>,
    pub n: V3,
}

/// A round rod from `a` to `b` of radius `r`.
#[derive(Copy, Clone, Debug)]
pub struct Rod {
    pub a: V3,
    pub b: V3,
    pub r: f32,
}

/// The instrument's surfaces.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub patches: Vec<(Role, Patch)>,
    pub polys: Vec<(Role, Poly)>,
    pub rods: Vec<(Role, Rod)>,
    /// Points whose floor shadow is the body's (their hull), the endpin's foot.
    pub shadow: Vec<V3>,
    pub foot: V3,
    /// The spruce's grain over the top (the Drawn picture's fine lines; the photographic
    /// one has the grain in its texture), and the top's normal.
    pub grain: Vec<Vec<V3>>,
    pub normal: V3,
    /// What else stands in the room (the chair, the stool): each piece's points, whose
    /// floor shadow is their hull.
    pub shadows: Vec<Vec<V3>>,
}

impl Patch {
    /// A grid from its points, normals computed from its neighbours and turned toward
    /// `out(point)`; grain coordinates from `uv`.
    pub fn of(
        pts: Vec<V3>,
        rows: usize,
        cols: usize,
        closed: bool,
        out: impl Fn(V3) -> V3,
        uv: impl Fn(usize, usize) -> [f32; 2],
    ) -> Patch {
        let at = |r: usize, c: usize| pts[r * cols + c];
        let mut nrm = Vec::with_capacity(pts.len());
        let mut uvs = Vec::with_capacity(pts.len());
        for r in 0..rows {
            for c in 0..cols {
                let (c0, c1) = if closed {
                    ((c + cols - 1) % cols, (c + 1) % cols)
                } else {
                    (c.saturating_sub(1), (c + 1).min(cols - 1))
                };
                let (r0, r1) = (r.saturating_sub(1), (r + 1).min(rows - 1));
                let du = at(r, c1) - at(r, c0);
                let mut dv = at(r1, c) - at(r0, c);
                if dv.len() < 1e-5 {
                    // A collapsed row (a ring's centre): across the neighbouring column.
                    dv = at(r1.min(rows - 1), c1) - at(r0, c0);
                }
                let mut n = du.cross(dv).norm();
                let p = at(r, c);
                if n.len() < 0.5 {
                    n = out(p).norm();
                }
                if n.dot(out(p)) < 0.0 {
                    n = -n;
                }
                nrm.push(n);
                uvs.push(uv(r, c));
            }
        }
        Patch {
            pts,
            nrm,
            uv: uvs,
            rows,
            cols,
            closed,
        }
    }

    /// The cells: each its four corners' indices.
    pub fn cells(&self) -> impl Iterator<Item = [usize; 4]> + '_ {
        let cols = self.cols;
        let span = if self.closed { cols } else { cols - 1 };
        (0..self.rows.saturating_sub(1)).flat_map(move |r| {
            (0..span).map(move |c| {
                let c1 = (c + 1) % cols;
                [
                    r * cols + c,
                    r * cols + c1,
                    (r + 1) * cols + c1,
                    (r + 1) * cols + c,
                ]
            })
        })
    }
}

/// `b`'s surfaces, built once.
pub fn mesh(b: Bowed) -> Arc<Mesh> {
    static CELLO: OnceLock<Arc<Mesh>> = OnceLock::new();
    static BASS: OnceLock<Arc<Mesh>> = OnceLock::new();
    static VIOLIN: OnceLock<Arc<Mesh>> = OnceLock::new(); // r42:arcus-upper
    static VIOLA: OnceLock<Arc<Mesh>> = OnceLock::new(); // r42:arcus-upper
    let cell = match b {
        Bowed::Cello => &CELLO,
        Bowed::Bass => &BASS,
        Bowed::Violin => &VIOLIN, // r42:arcus-upper
        Bowed::Viola => &VIOLA,   // r42:arcus-upper
    };
    cell.get_or_init(|| Arc::new(build(b, Holder::default())))
        .clone()
}

/// `b`'s surfaces as `h` holds it (r42:arcus-upper): the default from [`mesh`], the others
/// built on first use and kept (a few at a time).
pub fn mesh_for(b: Bowed, h: Holder) -> Arc<Mesh> {
    if h == Holder::default() {
        return mesh(b);
    }
    static KEPT: Mutex<Vec<((Bowed, Holder), Arc<Mesh>)>> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(i) = kept.iter().position(|(k, _)| *k == (b, h)) {
        let e = kept.remove(i);
        let m = e.1.clone();
        kept.push(e);
        return m;
    }
    let m = Arc::new(build(b, h));
    if kept.len() >= 8 {
        kept.remove(0);
    }
    kept.push(((b, h), m.clone()));
    m
}

fn build(b: Bowed, h: Holder) -> Mesh {
    let d = b.dims();
    let pose = super::bowed_hold::pose(b, h); // r42:arcus-upper
    let mut m = Mesh::default();
    let o = outline(b, 8);
    body(&mut m, d, &pose, &o);
    super::bowed_head::neck(&mut m, d, &pose);
    super::bowed_fit::fittings(&mut m, d, &pose, &o);
    if h.hold != Hold::Stand {
        // r42:arcus-upper
        super::bowed_props::furniture(&mut m, b, &pose);
    } // r42:arcus-upper
    super::bowed_hold::build(&mut m, b, &pose, h); // r42:arcus-upper
    m
}

/// Each outline point's inward normal (the outline counter-clockwise), from a smoothed
/// copy so a corner's tip turns its normal gradually.
pub fn inward(o: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = o.len();
    let sm = smooth(o, 3);
    (0..n)
        .map(|i| {
            let (a, c) = (sm[(i + n - 1) % n], sm[(i + 1) % n]);
            let (tx, ty) = (c.0 - a.0, c.1 - a.1);
            let l = tx.hypot(ty).max(1e-6);
            (-ty / l, tx / l)
        })
        .collect()
}

/// `iters` passes of neighbour averaging over a closed polyline.
pub fn smooth(o: &[(f32, f32)], iters: usize) -> Vec<(f32, f32)> {
    let n = o.len();
    let mut a = o.to_vec();
    for _ in 0..iters {
        let b = a.clone();
        for i in 0..n {
            let (p, q, r) = (b[(i + n - 1) % n], b[i], b[(i + 1) % n]);
            a[i] = (
                q.0 * 0.5 + (p.0 + r.0) * 0.25,
                q.1 * 0.5 + (p.1 + r.1) * 0.25,
            );
        }
    }
    a
}

/// The outline moved `d` cm inward (smoothed as it goes in, so no corner folds over).
pub fn offset(o: &[(f32, f32)], nin: &[(f32, f32)], d: f32) -> Vec<(f32, f32)> {
    let moved: Vec<(f32, f32)> = o
        .iter()
        .zip(nin)
        .map(|(&(x, y), &(nx, ny))| (x + nx * d, y + ny * d))
        .collect();
    smooth(&moved, (d * 3.0).ceil() as usize)
}

/// The rings (local x, y) a plate is gridded with: the edge, rings close in over the edge's
/// detail (overhang, channel, purfling), then rings scaled toward the middle.
fn rings(d: &Dims, o: &[(f32, f32)]) -> Vec<Vec<(f32, f32)>> {
    let nin = inward(o);
    let mut out = vec![o.to_vec()];
    for k in [0.35, 0.8, 1.4, 2.2, 3.2] {
        out.push(offset(o, &nin, k));
    }
    let last = out.last().cloned().unwrap_or_default();
    let c = (0.0, d.body * 0.5);
    for s in [0.86, 0.72, 0.58, 0.45, 0.33, 0.22, 0.12, 0.04] {
        out.push(
            last.iter()
                .map(|&(x, y)| (c.0 + (x - c.0) * s, c.1 + (y - c.1) * s))
                .collect(),
        );
    }
    out
}

/// The body: the arched top, the back, the ribs, the plates' edges, the purfling.
fn body(m: &mut Mesh, d: &Dims, pose: &Pose, o: &[(f32, f32)]) {
    let rg = rings(d, o);
    let (rows, cols) = (rg.len(), o.len());
    let reach = reach(d);
    // The top over the edge plane, the back under the ribs.
    let mut top = Vec::with_capacity(rows * cols);
    let mut back = Vec::with_capacity(rows * cols);
    for ring in &rg {
        for &(x, y) in ring {
            let dd = dist(o, x, y);
            top.push(pose.at(x, y, arch(d.arch, dd, reach)));
            let mut zb = arch(d.arch_back, dd, reach);
            if d.flat_back {
                zb = 0.42 + (zb - 0.42) * 0.4;
            }
            back.push(pose.at(x, y, -rib(d, y) - zb));
        }
    }
    let ez = pose.ez;
    let uv_of = |ring: &Vec<Vec<(f32, f32)>>| {
        let flat: Vec<[f32; 2]> = ring.iter().flatten().map(|&(x, y)| [y, x]).collect();
        move |r: usize, c: usize| flat[r * cols + c]
    };
    m.patches.push((
        Role::Top,
        Patch::of(top, rows, cols, true, |_| ez, uv_of(&rg)),
    ));
    m.patches.push((
        Role::Maple,
        Patch::of(back, rows, cols, true, |_| -ez, uv_of(&rg)),
    ));
    // The plates' edges and the ribs (set in under the edge's overhang).
    let nin = inward(o);
    let ribs_o = offset(o, &nin, 0.3);
    let mid = pose.at(0.0, d.body * 0.5, -d.rib_lo * 0.5);
    let outward = move |p: V3| {
        let q = p - mid;
        q - ez * q.dot(ez)
    };
    let band = |pts: &[(f32, f32)], z: &dyn Fn(f32) -> [f32; 2]| -> Vec<V3> {
        let mut v2 = Vec::with_capacity(pts.len() * 2);
        for row in 0..2 {
            for &(x, y) in pts {
                v2.push(pose.at(x, y, z(y)[row]));
            }
        }
        v2
    };
    let edge_uv = |r: usize, c: usize| [c as f32, r as f32];
    let te = band(o, &|_| [0.42, -0.02]);
    m.patches
        .push((Role::Top, Patch::of(te, 2, cols, true, outward, edge_uv)));
    let be = band(o, &|y| [-rib(d, y) + 0.02, -rib(d, y) - 0.42]);
    m.patches
        .push((Role::Maple, Patch::of(be, 2, cols, true, outward, edge_uv)));
    let ri = band(&ribs_o, &|y| [0.0, -rib(d, y)]);
    // The ribs' flame runs across them: along the outline is across the grain.
    let mut arc = vec![0.0f32; cols];
    for i in 1..cols {
        let (a, b) = (ribs_o[i - 1], ribs_o[i]);
        arc[i] = arc[i - 1] + (b.0 - a.0).hypot(b.1 - a.1);
    }
    let rib_uv = move |r: usize, c: usize| [arc[c], r as f32 * 12.0];
    m.patches
        .push((Role::Maple, Patch::of(ri, 2, cols, true, outward, rib_uv)));
    // The purfling 4 mm in: at the picture's size its black-white-black reads as one dark
    // line.
    let reach_t = |x: f32, y: f32| arch(d.arch, dist(o, x, y), reach) + 0.015;
    for (a, b) in [(0.37, 0.56)] {
        let (oa, ob) = (offset(o, &nin, a), offset(o, &nin, b));
        let mut pts = Vec::with_capacity(cols * 2);
        for ring in [&oa, &ob] {
            for &(x, y) in ring.iter() {
                pts.push(pose.at(x, y, reach_t(x, y)));
            }
        }
        m.patches.push((
            Role::Purfling,
            Patch::of(pts, 2, cols, true, |_| ez, |r, c| [c as f32, r as f32]),
        ));
    }
    super::bowed_fit::f_holes(m, d, pose, o);
    grain(m, d, pose, o);
    for &(x, y) in o.iter().step_by(4) {
        m.shadow.push(pose.at(x, y, d.arch));
        m.shadow.push(pose.at(x, y, -rib(d, y)));
    }
}

/// Whether (x, y) is inside the closed polygon `o` (even-odd).
pub fn inside(o: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    let n = o.len();
    for i in 0..n {
        let (a, b) = (o[i], o[(i + n - 1) % n]);
        if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

/// The spruce's grain: lines along the top a little under a centimetre apart, wavering, each
/// run from purfling to purfling (the f-holes cut through them).
fn grain(m: &mut Mesh, d: &Dims, pose: &Pose, o: &[(f32, f32)]) {
    let rc = reach(d);
    let k = d.body / 75.7;
    let half = o.iter().fold(0.0f32, |a, p| a.max(p.0));
    let step = 0.85 * k;
    let n = (2.0 * half / step) as i32;
    for j in 0..=n {
        let x0 = -half + j as f32 * step + 0.3 * k * ((j as f32) * 1.7).sin();
        let mut run: Vec<V3> = Vec::new();
        let mut y = 0.0;
        while y <= d.body {
            let x = x0 + 0.12 * k * (y * 0.11 / k + j as f32).sin();
            let ok = inside(o, x, y) && dist(o, x, y) > 0.75;
            if ok {
                run.push(pose.at(x, y, arch(d.arch, dist(o, x, y), rc) + 0.01));
            }
            if (!ok || y + 1.0 * k > d.body) && run.len() >= 3 {
                m.grain.push(std::mem::take(&mut run));
            } else if !ok {
                run.clear();
            }
            y += 1.0 * k;
        }
    }
    m.normal = pose.ez;
}
