//! r42:arcus-upper — a realistic figure's live arms (`scene::bowed_human`): the arm's own
//! surfaces (the sleeve's velvet or wool, the shirt's cuff, the hand's skin with its detail)
//! skinned to the joints the playing puts them at — the left hand's fingers on the stop
//! (`hand::left_pose`), the bow hand on the frog (`arm::bow_hand`) — and painted as smooth
//! triangles in depth order, the outline feathered by a pixel. Lit as the stylised figure's
//! arms are (`lit`). The left arm is painted in two passes: what the neck hides from the
//! eye before the neck is stamped back, the rest after.
//!
//! Per frame: one arm's few thousand vertices skinned (four bones each), its triangles
//! sorted; the buffers are kept (a thread's scratch), so a frame allocates nothing once
//! they have grown.

use super::body::View;
use super::lit::Lit;
use crate::kit::skin::scene::hold as hp;
use crate::kit::Brush;
use crate::scene::bowed_human::{self as hu, ArmAim, Figure, Posed, Stuff, Who};
use crate::scene::bowed_mesh::Role;
use crate::scene::geo::V3;
use egui::{Color32, Pos2};
use std::cell::RefCell;
use std::sync::OnceLock;

/// One arm's part of a figure: its vertices (the figure's numbering), its triangles in
/// its own, each one's stuff; its edges with the triangles either side (none: `u32::MAX`).
struct ArmMesh {
    verts: Vec<u32>,
    tris: Vec<[u16; 3]>,
    stuff: Vec<Stuff>,
    /// The upper arm's triangles (the left one's always paint behind the instrument: its
    /// body's stamp covers what the body hides).
    upper: Vec<bool>,
    /// Each vertex's opacity: clear where the arm's part ends inside the body, rising with
    /// the arm's share of its weights over the band round the shoulder, so the live arm
    /// fades into the baked body without a seam.
    alpha: Vec<f32>,
    edges: Vec<(u16, u16, u32, u32)>,
    /// r42:arcus-hand — which open edges are a texture seam's two sides drawn apart by the
    /// thinning (each side's copy collapsed on its own): painted solid across the crack,
    /// not feathered (the thin light line along the sleeve the review found).
    seam: Vec<bool>,
    /// r42:arcus-upper — each vertex's first copy at its place (a texture seam's copies
    /// share one normal).
    weld: Vec<u16>,
    /// r42:arcus-hand — High's hand: whether each triangle is the hand's skin or a nail
    /// (every corner on the hand's or the fingers' bones), drawn as a curved patch.
    fine: Vec<bool>,
    /// r42:arcus-polish — per triangle, a bit for each edge (corner k to k + 1) that a fine
    /// patch draws curved: only where the triangle beyond it is a fine patch too (a curved
    /// edge against a flat neighbour opened pixel cracks the jacket or the instrument showed
    /// through: dark dots across the back of his hand).
    curved: Vec<u8>,
    /// r42:arcus-hand — each vertex's detail (luminance, red) for the hand: the figure's,
    /// with the nails' free edges paler and the knuckles a little redder.
    vdet: Vec<[f32; 2]>,
    /// r42:arcus-hands — the hand's fine marks (the joints' creases) on this mesh; none on
    /// Light's.
    marks: Vec<super::marks::Line>,
    /// r42:arcus-hands — each triangle's edges (corner k to k + 1) as indices into the
    /// mesh's distinct edges (by vertex), and how many there are: a patch's edge points are
    /// worked out once a frame and shared by the two patches on that edge.
    tri_edges: Vec<[u32; 3]>,
    n_edges: usize,
    /// r42:arcus-hands — each vertex's skin texture and the back of the hand's relief, as
    /// the patches take them per point: the flat triangles round the wrist lit without
    /// them showed a lighter band across the back of his hand where the patches end.
    vtex: Vec<f32>,
    /// r42:arcus-polish2 — how much of each vertex follows the live arm (1) rather than
    /// staying where the bake put it (0, the arm at rest): the bow arm's band round the
    /// shoulder, where the live arm fades into the baked body, is the baked body's own surface
    /// until the arm is drawn opaque, then turns with the arm. Faded over the body in the
    /// live pose, the band's triangles stood off the baked shoulder (the arm at rest there)
    /// as dark and grey shards round her shoulder's seam (the owner's review of sheet v11).
    live: Vec<f32>,
    /// r42:arcus-polish2 — the sleeve's vertices (only a sleeve's cloth touches them): they
    /// follow the forearm, never the hand.
    sleeve: Vec<bool>,
}

/// r42:arcus-upper — the arm at a level of detail: 0 the full one (High), 1 and 2 Light's
/// (its vertices merged per bone on a grid of [`LOD_CELL`] cm: 1 for a hand drawn large,
/// 2 for one drawn small). Made once, the first time it is drawn, and kept.
fn arm_mesh_at(who: Who, s: usize, level: usize) -> &'static ArmMesh {
    const N: usize = 12;
    static M: [OnceLock<ArmMesh>; N] = [const { OnceLock::new() }; N];
    let level = level.min(2);
    let k = ((who == Who::Man) as usize * 2 + s.min(1)) * 3 + level;
    M[k].get_or_init(|| {
        let fig = hu::figure(who);
        if level == 0 {
            build_arm(fig, s)
        } else {
            lod(fig, s, arm_mesh_at(who, s, 0), LOD_CELL[level - 1])
        }
    })
}

/// Light's grid (cm): the hand's and the sleeve's cells for a hand drawn large, small (the
/// release profile's measure: the man's suit at 0.95/1.8 cm still cost +1.0 ms).
const LOD_CELL: [(f32, f32); 2] = [(1.15, 2.6), (1.6, 3.6)];

/// The px a cm takes on screen over which Light draws its finer level.
const LOD_FINE_PX: f32 = 2.6;

/// `full` with its vertices merged: each vertex goes to the cell of a grid (`cell.0` cm on
/// the hand, `cell.1` on the arm) its main bone, the stuffs it touches and its place at rest
/// name — so two fingers, a nail and its finger, a cuff and its sleeve never merge — and
/// stands for the cell by the vertex nearest the cell's middle; triangles that fold to an
/// edge go. A finger keeps its own bones, so it bends as High's does.
fn lod(fig: &Figure, s: usize, full: &ArmMesh, cell: (f32, f32)) -> ArmMesh {
    use std::collections::HashMap;
    let o = hu::side(s);
    let n = full.verts.len();
    let mut touch = vec![0u32; n];
    for (k, t) in full.tris.iter().enumerate() {
        for &i in t {
            touch[i as usize] |= 1 << stuff_slot(full.stuff[k]);
        }
    }
    let main = |g: usize| {
        fig.bw[g]
            .iter()
            .fold(
                (0u8, -1.0f32),
                |b, &(bone, w)| if w > b.1 { (bone, w) } else { b },
            )
            .0
    };
    let mut cells: HashMap<(u8, u32, [i32; 3]), Vec<usize>> = HashMap::new();
    for (i, &g) in full.verts.iter().enumerate() {
        let g = g as usize;
        let bone = main(g);
        let handy = bone as usize >= o + hu::HAND && (bone as usize) <= o + hu::finger(4, 2);
        let c = if handy { cell.0 } else { cell.1 };
        let p = fig.pos[g];
        let key = [p.x, p.y, p.z].map(|x| (x / c).floor() as i32);
        cells.entry((bone, touch[i], key)).or_default().push(i);
    }
    let mut rep = vec![0usize; n];
    for members in cells.values() {
        let m = members
            .iter()
            .fold(V3::default(), |a, &i| a + fig.pos[full.verts[i] as usize])
            * (1.0 / members.len() as f32);
        let best = members
            .iter()
            .copied()
            .min_by(|&a, &b| {
                let da = (fig.pos[full.verts[a] as usize] - m).len();
                let db = (fig.pos[full.verts[b] as usize] - m).len();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(members[0]);
        for &i in members {
            rep[i] = best;
        }
    }
    // The kept vertices renumbered.
    let mut map = vec![u16::MAX; n];
    let mut verts = Vec::new();
    let mut alpha = Vec::new();
    let mut live = Vec::new();
    let mut sleeve = Vec::new();
    let mut tris = Vec::new();
    let mut stuff = Vec::new();
    let mut upper = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (k, t) in full.tris.iter().enumerate() {
        let r = t.map(|i| rep[i as usize]);
        if r[0] == r[1] || r[1] == r[2] || r[0] == r[2] {
            continue;
        }
        let mut sorted = r;
        sorted.sort_unstable();
        if !seen.insert((sorted, full.stuff[k] as u8)) {
            continue;
        }
        let local = r.map(|i| {
            if map[i] == u16::MAX {
                map[i] = verts.len() as u16;
                verts.push(full.verts[i]);
                alpha.push(full.alpha[i]);
                live.push(full.live[i]);
                sleeve.push(full.sleeve[i]);
            }
            map[i]
        });
        tris.push(local);
        stuff.push(full.stuff[k]);
        upper.push(full.upper[k]);
    }
    let mut e: HashMap<(u16, u16), (u32, u32)> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let slot = e
                .entry((a.min(b), a.max(b)))
                .or_insert((u32::MAX, u32::MAX));
            if slot.0 == u32::MAX {
                slot.0 = ti as u32;
            } else {
                slot.1 = ti as u32;
            }
        }
    }
    // An open edge whose ends are both clear (where the arm's part was cut inside the body)
    // draws no outline, as High's does not.
    let mut edges: Vec<(u16, u16, u32, u32)> = e
        .into_iter()
        .filter(|&((a, b), (_, t1))| {
            t1 != u32::MAX || alpha[a as usize] > 0.0 || alpha[b as usize] > 0.0
        })
        .map(|((a, b), (t0, t1))| (a, b, t0, t1))
        .collect();
    edges.sort_unstable();
    let weld = weld_of(fig, &verts);
    let fine = vec![false; tris.len()];
    let vdet = verts.iter().map(|&g| fig.detail[g as usize]).collect();
    let seam = seams(fig, &verts, &edges);
    let curved = vec![0; tris.len()];
    let (tri_edges, n_edges) = edge_index(&tris);
    let vtex = vertex_tex(fig, s, &verts);
    ArmMesh {
        verts,
        tris,
        stuff,
        upper,
        alpha,
        edges,
        seam,
        weld,
        fine,
        curved,
        vdet,
        marks: Vec::new(),
        tri_edges,
        n_edges,
        vtex,
        live,
        sleeve,
    }
}

/// For each of an arm's vertices, the arm's own index of the first at its place.
fn weld_of(fig: &Figure, verts: &[u32]) -> Vec<u16> {
    let mut first: std::collections::HashMap<u32, u16> = Default::default();
    verts
        .iter()
        .enumerate()
        .map(|(i, &g)| {
            let w = fig.weld.get(g as usize).copied().unwrap_or(g);
            *first.entry(w).or_insert(i as u16)
        })
        .collect()
}

fn build_arm(fig: &Figure, s: usize) -> ArmMesh {
    let bit = if s == 0 { 2 } else { 4 };
    let mut map = vec![u32::MAX; fig.pos.len()];
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    let mut stuff = Vec::new();
    let mut upper = Vec::new();
    let o = hu::side(s);
    let up_share = |i: usize| {
        fig.bw[i]
            .iter()
            .filter(|(b, _)| {
                let b = *b as usize;
                b == o + hu::CLAV || b == o + hu::UPPER1 || b == o + hu::UPPER2
            })
            .map(|x| x.1)
            .sum::<f32>()
    };
    for (k, t) in fig.tris.iter().enumerate() {
        if fig.part[k] & bit == 0 {
            continue;
        }
        let local = t.map(|i| {
            let i = i as usize;
            if map[i] == u32::MAX {
                map[i] = verts.len() as u32;
                verts.push(i as u32);
            }
            map[i] as u16
        });
        tris.push(local);
        stuff.push(fig.stuff[k]);
        upper.push(t.iter().map(|&i| up_share(i as usize)).sum::<f32>() / 3.0 > 0.5);
    }
    close_cloth_seams(fig, &verts, &mut tris, &mut stuff, &mut upper);
    // (Edges by place: a texture seam's copies are one edge, not an outline.)
    let wl = weld_of(fig, &verts);
    let mut e: std::collections::HashMap<(u16, u16), (u32, u32)> = Default::default();
    for (ti, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (wl[a as usize], wl[b as usize]);
            let key = (a.min(b), a.max(b));
            let slot = e.entry(key).or_insert((u32::MAX, u32::MAX));
            if slot.0 == u32::MAX {
                slot.0 = ti as u32;
            } else {
                slot.1 = ti as u32;
            }
        }
    }
    // An edge with one triangle in the arm but two in the figure is where the part was cut.
    let mut whole: std::collections::HashMap<(u32, u32), u8> = Default::default();
    let gw = |i: u16| fig.weld.get(i as usize).copied().unwrap_or(i as u32);
    for t in &fig.tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (gw(a), gw(b));
            let key = (a.min(b), a.max(b));
            *whole.entry(key).or_insert(0) += 1;
        }
    }
    let side_share = |i: usize| {
        fig.bw[i]
            .iter()
            .filter(|(b, _)| {
                let b = *b as usize;
                b >= o + hu::UPPER1 && b <= o + hu::finger(4, 2)
            })
            .map(|x| x.1)
            .sum::<f32>()
    };
    let mut alpha: Vec<f32> = verts
        .iter()
        .map(|&g| {
            let t = ((side_share(g as usize) - 0.15) / 0.23).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect();
    let edges: Vec<(u16, u16, u32, u32)> = e
        .into_iter()
        .filter_map(|((a, b), (t0, t1))| {
            if t1 == u32::MAX {
                let (ga, gb) = (gw(verts[a as usize] as u16), gw(verts[b as usize] as u16));
                if whole.get(&(ga.min(gb), ga.max(gb))).copied().unwrap_or(0) > 1 {
                    alpha[a as usize] = 0.0;
                    alpha[b as usize] = 0.0;
                    return None;
                }
            }
            Some((a, b, t0, t1))
        })
        .collect();
    let weld = weld_of(fig, &verts);
    // (The band round each shoulder: the bake's surface below the arm's share where the live
    // arm turns opaque — `bowed_human::ARM_COVERS`, where the bake leaves the bow arm's out —,
    // turning with the arm over [`BAND_TURN`] of share above it. The left arm's too: its
    // armpit's corners, pulled between the chest and the raised arm, stood out as spikes
    // under his upper arm beside the frog, the owner's review of sheet v11.)
    let live: Vec<f32> = verts
        .iter()
        .map(|&g| {
            let t = ((side_share(g as usize) - hu::ARM_COVERS) / BAND_TURN).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect();
    // (A sleeve's vertex: every triangle round it is the sleeve's cloth.)
    let mut sleeve = vec![true; verts.len()];
    for (t, st) in tris.iter().zip(&stuff) {
        if !matches!(st, Stuff::Velvet | Stuff::Wool) {
            for &i in t {
                sleeve[i as usize] = false;
            }
        }
    }
    let (fine, vdet) = hand_detail(fig, s, &verts, &tris, &stuff);
    let seam = seams(fig, &verts, &edges);
    let curved = curved_edges(&tris, &fine, &weld);
    let marks = super::marks::build(fig, s, &verts, &tris, &stuff);
    let (tri_edges, n_edges) = edge_index(&tris);
    let vtex = vertex_tex(fig, s, &verts);
    ArmMesh {
        verts,
        tris,
        stuff,
        upper,
        alpha,
        edges,
        seam,
        weld,
        fine,
        curved,
        vdet,
        marks,
        tri_edges,
        n_edges,
        vtex,
        live,
        sleeve,
    }
}

/// r42:arcus-polish2 — how far a sleeve's triangle may be drawn out past its size at rest
/// (an edge's length) before it is taken for a fold's inside.
const STRETCH_MAX: f32 = 1.9;

/// r42:arcus-polish2 — over how much of the arm's share (above where it turns opaque) the
/// bow arm's band turns from the bake's place to the arm's.
const BAND_TURN: f32 = 0.4;

/// r42:arcus-hand — a sleeve's seam closed. The cloth's texture seam split its vertices in
/// two copies, and the offline thinning collapsed each side on its own, so the two sides of
/// the seam no longer meet: a crack down the sleeve the wall showed through (the review's
/// "tan stripe"). Each cloth vertex on an edge open in the whole figure is joined to the
/// nearest such vertex on the seam's other side (not one it shares a triangle with) within
/// [`SEAM_NEAR`] cm; triangles that fold to an edge go. Cloth only: the skin's seams keep
/// their copies (their texture coordinates differ).
fn close_cloth_seams(
    fig: &Figure,
    verts: &[u32],
    tris: &mut Vec<[u16; 3]>,
    stuff: &mut Vec<Stuff>,
    upper: &mut Vec<bool>,
) {
    use std::collections::{HashMap, HashSet};
    let cloth = |st: Stuff| matches!(st, Stuff::Velvet | Stuff::Wool | Stuff::Shirt);
    let gw = |g: u32| fig.weld.get(g as usize).copied().unwrap_or(g);
    let mut whole: HashMap<(u32, u32), u8> = HashMap::new();
    for t in &fig.tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (gw(a as u32), gw(b as u32));
            *whole.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let mut open: Vec<u16> = Vec::new();
    let mut near: HashSet<(u16, u16)> = HashSet::new();
    for (k, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            near.insert((a.min(b), a.max(b)));
            let (ga, gb) = (gw(verts[a as usize]), gw(verts[b as usize]));
            if cloth(stuff[k]) && whole.get(&(ga.min(gb), ga.max(gb))).copied() == Some(1) {
                open.push(a);
                open.push(b);
            }
        }
        for &i in t {
            for &j in t {
                if i < j {
                    near.insert((i, j));
                }
            }
        }
    }
    open.sort_unstable();
    open.dedup();
    let pos = |i: u16| fig.pos[verts[i as usize] as usize];
    let mut map: Vec<u16> = (0..verts.len() as u16).collect();
    let root = |m: &Vec<u16>, mut i: u16| {
        while m[i as usize] != i {
            i = m[i as usize];
        }
        i
    };
    for &i in &open {
        let best = open
            .iter()
            .copied()
            .filter(|&j| j != i && !near.contains(&(i.min(j), i.max(j))))
            .map(|j| (j, (pos(i) - pos(j)).len()))
            .filter(|&(_, d)| d < SEAM_NEAR)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((j, _)) = best {
            let (ri, rj) = (root(&map, i), root(&map, j));
            if ri != rj {
                map[ri.max(rj) as usize] = ri.min(rj);
            }
        }
    }
    let mut k = 0;
    while k < tris.len() {
        let t = tris[k].map(|i| root(&map, i));
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            tris.swap_remove(k);
            stuff.swap_remove(k);
            upper.swap_remove(k);
        } else {
            tris[k] = t;
            k += 1;
        }
    }
}

/// r42:arcus-hand — the open edges that are a seam's sides: each open edge whose middle
/// lies within [`SEAM_NEAR`] cm (at rest) of another open edge's middle.
fn seams(fig: &Figure, verts: &[u32], edges: &[(u16, u16, u32, u32)]) -> Vec<bool> {
    let mid = |e: &(u16, u16, u32, u32)| {
        (fig.pos[verts[e.0 as usize] as usize] + fig.pos[verts[e.1 as usize] as usize]) * 0.5
    };
    let open: Vec<(usize, V3)> = edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.3 == u32::MAX)
        .map(|(i, e)| (i, mid(e)))
        .collect();
    let mut seam = vec![false; edges.len()];
    for (k, &(i, m)) in open.iter().enumerate() {
        if open
            .iter()
            .enumerate()
            .any(|(j, &(_, n))| j != k && (m - n).len() < SEAM_NEAR)
        {
            seam[i] = true;
        }
    }
    seam
}

/// r42:arcus-hands, pass 2 — the skin's fine variation per vertex (its luminance's share,
/// its warmth's), and the back's extensor tendons (how much lighter at their ridge, their
/// half-width in cm).
const SKIN_VARY: (f32, f32) = (0.05, 0.12);
/// r42:arcus-hands, pass 2 — a nail's broad gloss (its power and strength) and its sheen at
/// a glance.
const NAIL_GLOSS: (i32, f32, f32) = (14, 0.1, 0.08);
const TENDON: (f32, f32) = (0.045, 0.22);

/// How near two open edges lie to be one seam's sides (cm; the thinning's shortest edge).
const SEAM_NEAR: f32 = 0.62;

/// r42:arcus-hand — which of an arm's triangles are its hand's skin or nails (`fine`), and
/// each vertex's detail for the hand: the figure's own, a nail paler toward its free edge
/// (the last fifth of its finger's last bone) and pink over its bed, the skin over the
/// middle and base knuckles on the back a little redder and darker (a hand's knuckles,
/// what a photograph of a violinist's left hand shows first).
fn hand_detail(
    fig: &Figure,
    s: usize,
    verts: &[u32],
    tris: &[[u16; 3]],
    stuff: &[Stuff],
) -> (Vec<bool>, Vec<[f32; 2]>) {
    let o = hu::side(s);
    let handy = |g: usize| {
        fig.bw[g]
            .iter()
            .filter(|(b, _)| (*b as usize) >= o + hu::HAND && (*b as usize) <= o + hu::finger(4, 2))
            .map(|x| x.1)
            .sum::<f32>()
            > 0.98
    };
    let fine = tris
        .iter()
        .zip(stuff)
        .map(|(t, st)| {
            matches!(st, Stuff::Skin | Stuff::Nail)
                && t.iter().all(|&i| handy(verts[i as usize] as usize))
        })
        .collect();
    let mut nail = vec![false; verts.len()];
    for (t, st) in tris.iter().zip(stuff) {
        if *st == Stuff::Nail {
            for &i in t {
                nail[i as usize] = true;
            }
        }
    }
    // The back of the hand at rest (away from the palm): the knuckles' side.
    let kn = |f: usize| fig.bones[o + hu::finger(f, 0)].head;
    let wr = fig.bones[o + hu::HAND].head;
    let mut back = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
    if (back.x < 0.0) == (s == 0) {
        back = back * -1.0;
    }
    let vdet = verts
        .iter()
        .enumerate()
        .map(|(i, &g)| {
            let g = g as usize;
            let mut d = fig.detail[g];
            let p = fig.pos[g];
            // The finger this vertex mostly follows, and its bone.
            let (bone, _) = fig.bw[g].iter().fold(
                (0u8, -1.0f32),
                |b, &(x, w)| {
                    if w > b.1 {
                        (x, w)
                    } else {
                        b
                    }
                },
            );
            let bone = bone as usize;
            // r42:arcus-hands, pass 2 — the skin's fine variation (the coordinator: "a
            // uniform beige"): each vertex a little lighter or darker, warmer or cooler, from
            // a hash of its index (the same every frame and every run).
            let hsh = |k: u32| {
                let mut x = (g as u32).wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
                x ^= x >> 15;
                x = x.wrapping_mul(0x2C1B_3C6D);
                x ^= x >> 12;
                (x & 0xFFFF) as f32 / 65535.0 - 0.5
            };
            d = [
                d[0] * (1.0 + SKIN_VARY.0 * hsh(1)),
                d[1] + SKIN_VARY.1 * hsh(2),
            ];
            // r42:arcus-polish — the back of the hand's faint veins: three soft lines from
            // the wrist's middle toward the gaps between the knuckles, a little blue under
            // the skin (the owner's small details, 2026-11-25).
            if bone == o + hu::HAND {
                let on_back = (p - wr).norm().dot(back).max(0.0);
                let mut vein = 0.0f32;
                for (f1, f2) in [(1usize, 2usize), (2, 3), (3, 4)] {
                    let a = wr.lerp((kn(f1) + kn(f2)) * 0.5, 0.18);
                    let b = wr.lerp((kn(f1) + kn(f2)) * 0.5, 0.82);
                    let ab = b - a;
                    let tt = ((p - a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
                    let dd = (p - (a + ab * tt)).len();
                    vein = vein.max((-(dd / 0.32) * (dd / 0.32)).exp() * (1.0 - tt * 0.5));
                }
                let w = vein * on_back.powf(0.5) * VEIN;
                d = [d[0] * (1.0 - 0.05 * w), d[1] - 0.3 * w];
                // r42:arcus-hands, pass 2 — the extensor tendons (crop-08): a faint lighter
                // ridge from the wrist's middle to each base knuckle, clearest over the
                // back's middle, gone at both ends.
                let mut ten = 0.0f32;
                for f in 1..5usize {
                    let a = wr.lerp(kn(f), 0.22);
                    let b = wr.lerp(kn(f), 0.92);
                    let ab = b - a;
                    let tt = ((p - a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
                    let dd = (p - (a + ab * tt)).len();
                    let along = (tt * std::f32::consts::PI).sin();
                    ten = ten.max((-(dd / TENDON.1) * (dd / TENDON.1)).exp() * along);
                }
                let w = ten * on_back.powf(0.5) * TENDON.0;
                d = [d[0] * (1.0 + w), d[1] - 0.1 * w];
                return d;
            }
            if !(o + hu::finger(0, 0)..=o + hu::finger(4, 2)).contains(&bone) {
                return d;
            }
            let bn = &fig.bones[bone];
            let ax = bn.tail - bn.head;
            let t = (p - bn.head).dot(ax) / ax.dot(ax).max(1e-6);
            if nail[i] {
                // The free edge pale, the bed pink.
                let edge = ((t - 0.78) / 0.12).clamp(0.0, 1.0);
                // r42:arcus-polish — and the lunula at its root, a paler crescent.
                let lunula = (1.0 - ((t - 0.45) / 0.1).clamp(0.0, 1.0)) * (1.0 - edge);
                d = [
                    d[0] * (1.0 + 0.16 * edge + 0.1 * lunula),
                    d[1] + 0.25 * (1.0 - edge - lunula) - 0.45 * edge - 0.2 * lunula,
                ];
                return d;
            }
            // Over a joint on the back: the base and middle knuckles.
            let k = (bone - o - hu::finger(0, 0)) % 3;
            // The fingertips' pads warmer (blood near the skin).
            if k == 2 {
                let tipw = ((t - 0.5) / 0.5).clamp(0.0, 1.0);
                d = [d[0] * (1.0 - 0.04 * tipw), d[1] + 0.42 * tipw];
            }
            let near_head = (1.0 - (t / 0.22).abs()).max(0.0);
            let on_back = (p - (bn.head + ax * t)).norm().dot(back).max(0.0);
            if bone >= o + hu::finger(1, 0) {
                // (r42:arcus-hands, pass 2: the last knuckles too, fainter; the base
                // knuckles' tops a little lighter, where their bumps catch the light, the
                // skin round them redder.)
                let w = near_head * on_back * [1.0, 1.0, 0.6][k];
                let top = near_head.powi(3) * on_back.powi(4) * (k == 0) as u8 as f32;
                d = [d[0] * (1.0 - 0.05 * w + 0.05 * top), d[1] + 0.4 * w];
            }
            // (r42:arcus-hands: the joints' creases are the marks' — `marks`, soft lines a
            // pixel and a half wide on the skinned surface; a band per vertex here, narrower
            // than the patches' points, broke into dark 1-3 px dashes on the fingers.)
            d
        })
        .collect();
    (fine, vdet)
}

#[derive(Default)]
struct Scratch {
    pts: Vec<V3>,
    nrm: Vec<V3>,
    sp: Vec<Pos2>,
    front: Vec<bool>,
    /// r42:arcus-hands — each triangle drawn split between the passes (a hand patch across
    /// the neck's edge).
    split: Vec<bool>,
    /// r42:arcus-hands — the patches' shared points (kept while the skinning is).
    fine: RefCell<FineCache>,
    /// r42:arcus-hands — High's samples with their depths (`raster`), and what each pass's
    /// last picture was filled for (the skinning's key and its rectangle).
    ras: RefCell<super::raster::Raster>,
    ras_key: [Option<([V3; 8], egui::Rect)>; 4],
    col: Vec<[Color32; 3]>,
    order: Vec<(f32, u32)>,
    ao: Vec<f32>,
    caps: Vec<Capsule>,
    /// Each vertex's colour this frame and the stuff it was lit as.
    vcol: Vec<(Option<Stuff>, Color32)>,
    /// What the kept skinning was made for, and the arm's middle.
    key: Option<[V3; 8]>,
    mid: V3,
    /// r42:arcus-polish2 — each triangle's class this frame ([`CLS_FACING`] …), its
    /// middle's depth, whether the neck hides each corner, and whether the classes were
    /// made with the neck's test.
    cls: Vec<u8>,
    depth: Vec<f32>,
    hidv: Vec<bool>,
    cls_hid: bool,
    /// r42:arcus-polish2 — the outline's way out at each corner (summed over its edges) and
    /// along each edge (unit, on screen).
    vout: Vec<egui::Vec2>,
    eout: Vec<egui::Vec2>,
    /// r42:arcus-polish2 — each bone as a dual quaternion this frame (none: stretched).
    dq: Vec<Option<Dq>>,
    /// r42:arcus-polish2 — the cuffs' edge corners posed, and which corners are skin.
    ring: Vec<V3>,
    skin_v: Vec<bool>,
}

/// A rigid turn and move as a unit dual quaternion: the real part's (w, x, y, z) and the
/// dual part's.
type Dq = ([f32; 4], [f32; 4]);

/// Bone transform `x` as a dual quaternion, unless it stretches its bone.
fn dual(x: &hu::Xf) -> Option<Dq> {
    if (x.k - 1.0).abs() > 1e-3 {
        return None;
    }
    let r = &x.rot;
    // The rotation's matrix: its columns are the images of the axes.
    let (m00, m01, m02) = (r.x.x, r.y.x, r.z.x);
    let (m10, m11, m12) = (r.x.y, r.y.y, r.z.y);
    let (m20, m21, m22) = (r.x.z, r.y.z, r.z.z);
    let tr = m00 + m11 + m22;
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [0.25 * s, (m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [(m21 - m12) / s, 0.25 * s, (m01 + m10) / s, (m02 + m20) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m02 - m20) / s, (m01 + m10) / s, 0.25 * s, (m12 + m21) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m10 - m01) / s, (m02 + m20) / s, (m12 + m21) / s, 0.25 * s]
    };
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1e-6 {
        return None;
    }
    let q = q.map(|c| c / n);
    // p' = at + R(p − head) = R p + t, t = at − R head.
    let t = x.at - r.apply(x.head);
    // The dual part: ½ (0, t) q.
    let d = [
        -0.5 * (t.x * q[1] + t.y * q[2] + t.z * q[3]),
        0.5 * (t.x * q[0] + t.y * q[3] - t.z * q[2]),
        0.5 * (-t.x * q[3] + t.y * q[0] + t.z * q[1]),
        0.5 * (t.x * q[2] - t.y * q[1] + t.z * q[0]),
    ];
    Some((q, d))
}

/// Vertex `i` of `fig` skinned by its bones' dual quaternions blended (none if a bone it
/// follows has none).
fn dq_vertex(fig: &Figure, posed: &Posed, dq: &[Option<Dq>], i: usize) -> Option<V3> {
    let w = &fig.bw[i];
    let first = dq.get(w.first()?.0 as usize).copied().flatten()?;
    let (mut r, mut d) = ([0.0f32; 4], [0.0f32; 4]);
    for &(b, x) in w {
        if x <= 0.0 {
            continue;
        }
        let (q, e) = dq.get(b as usize).copied().flatten()?;
        // The shorter way round: each turn on the first one's side.
        let s = if q.iter().zip(first.0).map(|(a, b)| a * b).sum::<f32>() < 0.0 {
            -x
        } else {
            x
        };
        for k in 0..4 {
            r[k] += q[k] * s;
            d[k] += e[k] * s;
        }
    }
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2] + r[3] * r[3]).sqrt();
    if n < 1e-6 {
        return None;
    }
    let (r, d) = (r.map(|c| c / n), d.map(|c| c / n));
    let p = fig.pos[i];
    // Rotate p by r, then move by t = 2 d r*.
    let (w0, v) = (
        r[0],
        V3 {
            x: r[1],
            y: r[2],
            z: r[3],
        },
    );
    let rot = p + v.cross(v.cross(p) + p * w0) * 2.0;
    let t = V3 {
        x: 2.0 * (-d[0] * r[1] + d[1] * r[0] - d[2] * r[3] + d[3] * r[2]),
        y: 2.0 * (-d[0] * r[2] + d[1] * r[3] + d[2] * r[0] - d[3] * r[1]),
        z: 2.0 * (-d[0] * r[3] - d[1] * r[2] + d[2] * r[1] + d[3] * r[0]),
    };
    let _ = posed;
    Some(rot + t)
}

/// A triangle faces the eye (and is drawn: not a shirt's cuff), is a sleeve's inside, lies
/// behind the neck.
const CLS_FACING: u8 = 1;
const CLS_INSIDE: u8 = 2;
const CLS_BEHIND: u8 = 4;
/// r42:arcus-hands — a hand patch across the neck's edge (drawn split between the passes).
const CLS_SPLIT: u8 = 8;

/// The occluding capsules (ADR 0159: shared by every figure's live limbs).
pub use crate::scene::figure::live::Capsule;

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// How the live arm's corners are lit: by the picture's own light, so the arm is the baked
/// body's (the owner, 2026-11-24: grey sleeves and chalk hands beside a warm face). The
/// Drawn picture's `Light` and its skin's tone as `bowed_draw_tris` shades the body; the
/// Photographic picture's soft key (wrapped on skin, its light through skin warm in the
/// terminator), its fill from the camera's side and its ambient, fitted to its bake, through
/// its exposure and filmic curve.
pub struct Shading {
    drawn: Option<crate::scene::light::Light>,
    eye: V3,
    key: V3,
    dark: bool,
}

impl Shading {
    pub fn of(lit: &Lit) -> Shading {
        let photo =
            crate::scene::backend::current() == crate::scene::backend::SceneBackend::PerPixel;
        let drawn = (!photo).then(|| {
            crate::scene::light::Light::new(
                lit.dark,
                false,
                crate::scene::geo::v(-90.0, 380.0, 160.0),
                crate::scene::geo::v(lit.mid.x, 0.0, lit.mid.z),
            )
        });
        Shading {
            drawn,
            eye: lit.eye,
            key: lit.key,
            dark: lit.dark,
        }
    }
}

/// The light by the normal's way, worked out once for a stuff and kept (an octahedral map,
/// `LUT`² ways): a frame looks a corner's colour up instead of lighting it (the frame
/// budget: lit per corner, the realistic arms cost 3–4 ms a frame). Taken at the arm's
/// middle and with the skin's plain albedo; the skin's detail and the occlusion scale it.
const LUT: usize = 40;

#[derive(Clone, PartialEq)]
struct LutKey {
    photo: bool,
    dark: bool,
    tone: u8,
    eye: [i32; 3],
    key: [i32; 3],
    mid: [i32; 3],
}

#[derive(Default)]
struct Luts {
    key: Option<LutKey>,
    /// Per stuff (by `stuff_slot`): its colours, display-referred 0…1.
    maps: [Vec<[f32; 3]>; 8],
}

thread_local! {
    /// One set per arm (their middles differ: one set for both was remade twice a frame).
    static LUTS: RefCell<[Luts; 2]> = RefCell::new([Luts::default(), Luts::default()]);
}

fn stuff_slot(st: Stuff) -> usize {
    match st {
        Stuff::Skin | Stuff::Lips => 0,
        Stuff::Nail => 1,
        Stuff::Velvet => 2,
        Stuff::Wool => 3,
        Stuff::Shirt => 4,
        Stuff::Tie => 5,
        Stuff::Hair => 6,
        _ => 7,
    }
}

/// The octahedral map's place for a unit vector, and back.
fn oct_uv(n: V3) -> (f32, f32) {
    let s = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / s, n.y / s);
    if n.z < 0.0 {
        let (ox, oy) = (x, y);
        x = (1.0 - oy.abs()) * ox.signum();
        y = (1.0 - ox.abs()) * oy.signum();
    }
    (x * 0.5 + 0.5, y * 0.5 + 0.5)
}

fn oct_dir(u: f32, v: f32) -> V3 {
    let (x, y) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
    let z = 1.0 - x.abs() - y.abs();
    let (x, y) = if z < 0.0 {
        ((1.0 - y.abs()) * x.signum(), (1.0 - x.abs()) * y.signum())
    } else {
        (x, y)
    };
    v3(x, y, z).norm()
}

/// A corner's colour from the kept maps (made again when the light or the view changes).
#[allow(clippy::too_many_arguments)]
fn lut_colour(
    sh: &Shading,
    side: usize,
    st: Stuff,
    tone: u8,
    detail: [f32; 2],
    mid: V3,
    n: V3,
    ao: f32,
) -> Color32 {
    with_lut(sh, side, tone, mid, |l| l.colour(st, detail, n, ao))
}

/// r42:arcus-polish2 — the kept maps borrowed once for many corners (a frame's few thousand
/// look-ups each took the thread's table, checked its key and let it go: a fifth of the
/// per-triangle loop).
struct LutAt<'a> {
    l: &'a mut Luts,
    sh: &'a Shading,
    tone: u8,
    mid: V3,
}

/// `f` with arm `side`'s maps for this light, view and middle (emptied when they change).
fn with_lut<R>(sh: &Shading, side: usize, tone: u8, mid: V3, f: impl FnOnce(&mut LutAt) -> R) -> R {
    let q = |p: V3| [p.x.round() as i32, p.y.round() as i32, p.z.round() as i32];
    let key = LutKey {
        photo: sh.drawn.is_none(),
        dark: sh.dark,
        tone,
        eye: q(sh.eye),
        key: q(sh.key),
        // (The arm's middle to 8 cm: to the cm, a hand trembling in vibrato across a
        // centimetre's edge remade the maps — a slow frame now and then; at 4 cm the man's
        // bowing arm still crossed one.)
        mid: q(mid * 0.125),
    };
    LUTS.with(|l| {
        let mut all = l.borrow_mut();
        let l = &mut all[side.min(1)];
        if l.key.as_ref() != Some(&key) {
            l.key = Some(key);
            for m in l.maps.iter_mut() {
                m.clear();
            }
        }
        f(&mut LutAt { l, sh, tone, mid })
    })
}

impl LutAt<'_> {
    /// A corner's colour: its stuff by its normal from the map, its detail and occlusion.
    fn colour(&mut self, st: Stuff, detail: [f32; 2], n: V3, ao: f32) -> Color32 {
        let slot = stuff_slot(st);
        if self.l.maps[slot].is_empty() {
            let plain = [1.0, 0.0];
            let mut map = Vec::with_capacity(LUT * LUT);
            for j in 0..LUT {
                for i in 0..LUT {
                    let d = oct_dir((i as f32 + 0.5) / LUT as f32, (j as f32 + 0.5) / LUT as f32);
                    let c = colour(self.sh, st, self.tone, plain, self.mid, d, 1.0);
                    map.push([c.r(), c.g(), c.b()].map(|x| x as f32 / 255.0));
                }
            }
            self.l.maps[slot] = map;
        }
        let map = &self.l.maps[slot];
        let (u, v) = oct_uv(n);
        let (fx, fy) = (u * LUT as f32 - 0.5, v * LUT as f32 - 0.5);
        let (x0, y0) = (fx.floor().max(0.0), fy.floor().max(0.0));
        let (tx, ty) = ((fx - x0).clamp(0.0, 1.0), (fy - y0).clamp(0.0, 1.0));
        let (x0, y0) = ((x0 as usize).min(LUT - 1), (y0 as usize).min(LUT - 1));
        let (x1, y1) = ((x0 + 1).min(LUT - 1), (y0 + 1).min(LUT - 1));
        let at = |x: usize, y: usize| map[y * LUT + x];
        let mut c = [0.0f32; 3];
        for (k, ck) in c.iter_mut().enumerate() {
            let a = at(x0, y0)[k] * (1.0 - tx) + at(x1, y0)[k] * tx;
            let b = at(x0, y1)[k] * (1.0 - tx) + at(x1, y1)[k] * tx;
            *ck = a * (1.0 - ty) + b * ty;
        }
        // The skin's detail (brows, lips, the knuckles' creases, the nails) and the
        // occlusion.
        if matches!(st, Stuff::Skin | Stuff::Lips) {
            let (lum, red) = (detail[0].min(1.1), detail[1]);
            let f = [1.0 + 0.22 * red, 1.0 - 0.12 * red, 1.0 - 0.18 * red];
            for (ck, fk) in c.iter_mut().zip(f) {
                *ck *= lum * fk;
            }
        }
        let k = 0.38 + 0.62 * ao;
        let b = |x: f32| ((x * k).clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
    }
}

/// A corner's colour: its stuff lit (`ao`: how much of the light round it reaches, the
/// fingers' and the stick's shade on each other).
fn colour(sh: &Shading, st: Stuff, tone: u8, detail: [f32; 2], p: V3, n: V3, ao: f32) -> Color32 {
    let tone = tone.min(4);
    let e = (sh.eye - p).norm();
    let ndv = n.dot(e).clamp(0.0, 1.0);
    // The nail: the skin's own colour paler and a little pink, glossier.
    let skin_detail = match st {
        // (A lighter, glossier cap than the skin round it: the owner's review.)
        Stuff::Nail => [detail[0].clamp(0.9, 1.1) * 1.32, detail[1] * 0.3 - 0.2],
        _ => detail,
    };
    if let Some(light) = &sh.drawn {
        use crate::scene::bowed_draw_tris::{skin_at, skin_tone};
        let mat = match st {
            Stuff::Skin | Stuff::Lips | Stuff::Nail => skin_at(&hp::real_skin(tone), skin_detail),
            Stuff::Velvet => {
                crate::scene::bowed_draw::material(crate::scene::bowed::Bowed::Cello, Role::Velvet)
            }
            Stuff::Wool => {
                crate::scene::bowed_draw::material(crate::scene::bowed::Bowed::Cello, Role::Wool)
            }
            Stuff::Shirt => {
                crate::scene::bowed_draw::material(crate::scene::bowed::Bowed::Cello, Role::Shirt)
            }
            Stuff::Tie => {
                crate::scene::bowed_draw::material(crate::scene::bowed::Bowed::Cello, Role::Tie)
            }
            Stuff::Hair => hp::hair(hp::HAIR),
            _ => crate::scene::bowed_draw::material(crate::scene::bowed::Bowed::Cello, Role::Shoes),
        };
        let mut c = light.shade(&mat, n, e, p, None, 1.0);
        if matches!(st, Stuff::Skin | Stuff::Lips | Stuff::Nail) {
            c = skin_tone(&mat, c, ndv);
        }
        if st == Stuff::Velvet {
            let s = hp::VELVET_SHEEN;
            let g = 1.0 - ndv;
            let k = g * g * 0.55 + g.powi(5) * 0.6; // r42:arcus-polish
            c = [c[0] + s[0] * k, c[1] + s[1] * k, c[2] + s[2] * k];
        }
        if matches!(st, Stuff::Velvet | Stuff::Wool) {
            // r42:arcus-hand — the sleeve's rim (as the Photographic picture's).
            let (rim, lift) = cloth_rim(n, e);
            let s = hp::CLOTH_RIM;
            c = [0, 1, 2].map(|i| c[i] * lift + s[i] * rim * 0.8);
        }
        let k = 0.35 + 0.65 * ao;
        return c3(c.map(|x| x * k));
    }
    // The Photographic picture's light, in linear light.
    let lin = |c: crate::kit::skin::scene::C3| {
        c.map(|x| egui::ecolor::linear_f32_from_gamma_u8((x.clamp(0.0, 1.0) * 255.0) as u8))
    };
    let l = (sh.key - p).norm();
    let ndl = n.dot(l);
    let skin = matches!(st, Stuff::Skin | Stuff::Lips | Stuff::Nail);
    let (key_k, fill_k, amb_k, ex) = if sh.dark {
        (1.0, 0.22, 0.16, 1.25)
    } else {
        (1.0, 0.42, 0.24, 1.15)
    };
    // (r42:arcus-polish: a narrower wrap on the live skin — the back of the hand lit flat hid
    // its tendons and knuckles, the coordinator against crop-08's raking light.)
    let wrap = if skin { 0.25 } else { 0.1 };
    let dif = ((ndl + wrap) / (1.0 + wrap)).max(0.0);
    let fill = (n.dot((e + v3(0.0, 0.25, 0.0)).norm())).max(0.0) * fill_k;
    let amb = amb_k * (0.8 + 0.2 * n.y);
    let albedo: [f32; 3] = match st {
        Stuff::Skin | Stuff::Lips | Stuff::Nail => {
            // (r42:arcus-hand: warmer, as her face in the same picture is — the review found
            // the hand clay-grey beside it: the face's red over blue 1.7, the hand's 1.4.)
            let a = lin(rgbf(hp::REAL_SKINS[tone as usize]));
            let a = [0, 1, 2].map(|i| a[i] * 0.48 * SKIN_WARM[i]);
            let (lum, red) = (skin_detail[0].min(1.25), skin_detail[1]);
            [
                (a[0] * lum * (1.0 + 0.22 * red)).max(0.0),
                (a[1] * lum * (1.0 - 0.12 * red)).max(0.0),
                (a[2] * lum * (1.0 - 0.18 * red)).max(0.0),
            ]
        }
        Stuff::Velvet => lin(hp::VELVET),
        Stuff::Wool => lin(hp::WOOL),
        Stuff::Shirt => lin(hp::SHIRT),
        Stuff::Tie => lin(hp::TIE),
        Stuff::Hair => lin(hp::HAIR),
        _ => lin(hp::SHOES),
    };
    let mut rad = [0.0f32; 3];
    for (i, r) in rad.iter_mut().enumerate() {
        *r = albedo[i] * (key_k * dif * ao + fill + amb * ao);
    }
    if skin {
        // Light through skin: warm where the key falls off.
        // (r42:arcus-upper: stronger, with the picture's: a red share in the shade and the
        // portrait's soft fill from the camera's side.)
        let t = (dif - ndl.max(0.0)).max(0.0) * key_k * ao;
        rad[0] += albedo[0] * (t * 0.75 + amb * ao * 0.06);
        rad[1] += albedo[1] * (t * 0.16 + amb * ao * 0.015);
        let front = ndv.powf(2.0) * 0.035 * ao;
        for (i, r) in rad.iter_mut().enumerate() {
            *r += albedo[i] * (key_k + amb) * front;
        }
        // A soft sheen (a nail's sharper).
        let h = (l + e).norm();
        let (pw, k) = if st == Stuff::Nail {
            (60, 0.34)
        } else {
            (18, 0.035)
        };
        let mut sp = n.dot(h).max(0.0).powi(pw) * k * key_k * ao * (ndl > 0.0) as u8 as f32;
        if st == Stuff::Nail {
            // r42:arcus-hands, pass 2 — the nail plate's broad gloss (the coordinator: the
            // plate's sheen must read from the picture's camera; the sharp lobe alone fell
            // between the few corners a nail has) and its reflection at a glance.
            let broad = n.dot(h).max(0.0).powi(NAIL_GLOSS.0) * NAIL_GLOSS.1 * key_k * ao;
            let glance = (1.0 - ndv).powi(4) * NAIL_GLOSS.2 * (key_k + amb) * ao;
            sp += broad * (ndl > 0.0) as u8 as f32 + glance;
        }
        rad = rad.map(|x| x + sp);
    }
    if st == Stuff::Velvet {
        // Velvet's pile catches the light where it turns away.
        // (r42:arcus-polish: and a bright rim where it turns fully away — the bake's.)
        let g = 1.0 - ndv;
        let pile = g * g * 0.28 + g.powi(5) * 0.42;
        let s = lin(hp::VELVET_SHEEN);
        for (i, r) in rad.iter_mut().enumerate() {
            *r += s[i] * pile * (key_k * dif.max(0.35) + amb);
        }
    }
    if matches!(st, Stuff::Velvet | Stuff::Wool) {
        // r42:arcus-hand — the sleeve against the dark body behind it: a back light along
        // its edge and its own value a little lifted, so the arm reads as an arm in front.
        let (rim, lift) = cloth_rim(n, e);
        let s = lin(hp::CLOTH_RIM);
        for (i, r) in rad.iter_mut().enumerate() {
            *r = *r * lift + s[i] * rim;
        }
    }
    let t = crate::views::inst::chorda::scene::hybrid::raster::tone(rad.map(|x| x * ex), 0.85);
    let g = |x: f32| egui::ecolor::gamma_u8_from_linear_f32(x);
    Color32::from_rgb(g(t[0]), g(t[1]), g(t[2]))
}

/// r42:arcus-hand — a live sleeve's back light: its share where the cloth turns away from
/// the eye toward a light behind the figure and a little over it (`n` the normal, `e` the
/// way to the eye), and how much its own value is lifted (the arm a shade lighter than the
/// dress or the jacket behind it).
fn cloth_rim(n: V3, e: V3) -> (f32, f32) {
    let back = (v3(0.0, 0.6, 0.0) - e).norm();
    let edge = (1.0 - n.dot(e).clamp(0.0, 1.0)).powi(3);
    (edge * n.dot(back).max(0.0).sqrt() * CLOTH_RIM_K, CLOTH_LIFT)
}

/// r42:arcus-hand — the live hand's skin warmed to the baked face's (linear gains).
const SKIN_WARM: [f32; 3] = [1.1, 0.95, 0.78];

/// How strong the sleeve's rim is and how much its value is lifted.
const CLOTH_RIM_K: f32 = 0.34;
const CLOTH_LIFT: f32 = 1.3;

fn c3(c: crate::kit::skin::scene::C3) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(b(c[0]), b(c[1]), b(c[2]))
}

fn rgbf(c: [u8; 3]) -> crate::kit::skin::scene::C3 {
    c.map(|x| x as f32 / 255.0)
}

fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

/// Paint arm `s` (0 left, 1 right) of figure `who` posed through `aim` from the baked
/// body `base`. `hidden`: for the left arm, paint only what the neck hides from the eye
/// (`Some(true)`, before the neck is stamped back) or only what it does not (`Some(false)`);
/// `hide` tests a point.
#[allow(clippy::too_many_arguments)]
pub fn arm(
    b: &mut Brush,
    v: &View,
    lit: &Lit,
    who: Who,
    tone: u8,
    base: &Posed,
    s: usize,
    aim: &ArmAim,
    hidden: Option<(bool, &dyn Fn(V3) -> bool)>,
    held: &[Capsule],
    light: bool,
) {
    let fig = hu::figure(who);
    // r42:arcus-upper — Light: the merged mesh, finer while the hand is drawn large.
    let level = if !light {
        0
    } else if v.scale(aim.wrist) > LOD_FINE_PX {
        1
    } else {
        2
    };
    let am = arm_mesh_at(who, s, level);
    let shading = Shading::of(lit);
    // r42:arcus-hand — High's hand as curved patches cut to a few px a side (none when the
    // hand is small on screen).
    let fine_n = if light {
        1
    } else {
        ((v.scale(aim.wrist) * FINE_EDGE / FINE_PX).ceil() as usize).clamp(1, FINE_MAX)
    };
    SCRATCH.with(|sc| {
        let mut sc = sc.borrow_mut();
        let sc = &mut *sc;
        let n = am.verts.len();
        // The left arm is painted in two passes (behind the neck, then in front): the
        // second finds the arm skinned and lit by the first.
        let h = aim
            .hand
            .map_or([V3::default(); 2], |h| [h.fingers[1][3], h.thumb[3]]);
        let key = [
            aim.elbow,
            aim.wrist,
            h[0],
            h[1],
            v.cam.eye(),
            lit.key,
            base.xf[hu::side(s)].at,
            V3 {
                x: who as u8 as f32,
                y: s as f32 + tone as f32 * 2.0,
                z: shading.drawn.is_some() as u8 as f32 + v.origin.x * 3.0 + v.k.x,
            },
        ];
        let key = {
            let mut k = key;
            k[7].x += level as f32 * 16.0;
            k
        };
        let same = sc.key == Some(key) && sc.pts.len() == n;
        sc.key = Some(key);
        let tt = super::probe::t0();
        let mid = if same {
            sc.mid
        } else {
            let mut fc = sc.fine.borrow_mut();
            fc.v.clear();
            fc.v.resize(n, None);
            fc.e.clear();
            fc.e.resize(am.n_edges, None);
            drop(fc);
            arm_setup(fig, am, base, s, aim, v, held, sc, light)
        };
        super::probe::lap("setup", tt);
        // r42:arcus-polish — the tests' measure of the arm from the app's camera.
        if super::probe::on() {
            let o = hu::side(s);
            super::probe::keep(super::probe::Arm {
                who: who as u8,
                side: s,
                shoulder: base.at(o + hu::UPPER1, fig.bones[o + hu::UPPER1].head),
                elbow: aim.elbow,
                wrist: aim.wrist,
                pts: sc.pts.clone(),
                bone: am
                    .verts
                    .iter()
                    .map(|&g| {
                        fig.bw[g as usize]
                            .iter()
                            .fold(
                                (0u8, -1.0f32),
                                |b, &(x, w)| if w > b.1 { (x, w) } else { b },
                            )
                            .0
                    })
                    .collect(),
                screen: sc.sp.clone(),
                scale: sc.pts.iter().map(|&p| v.scale(p)).collect(),
                tris: am.tris.clone(),
                skin: {
                    let mut k = vec![false; am.verts.len()];
                    for (t, st) in am.tris.iter().zip(&am.stuff) {
                        if matches!(st, Stuff::Skin | Stuff::Nail) {
                            for &i in t {
                                k[i as usize] = true;
                            }
                        }
                    }
                    k
                },
                joints: {
                    let posed = hu::repose_arm(fig, base, s, aim);
                    (o + hu::UPPER1..=o + hu::finger(4, 2))
                        .map(|b| {
                            let w = posed.at(b, fig.bones[b].head);
                            (b, v.p(w), w)
                        })
                        .collect()
                },
            });
        }
        let eye = v.cam.eye();
        // r42:arcus-polish — the arm's soft shadow where it crosses in front of the trunk
        // (once: in the left arm's first pass).
        let tt = super::probe::t0();
        if hidden.is_none_or(|h| h.0) {
            trunk_shadow(b, v, lit, fig, base, aim, &shading, who, s, tone, mid);
        }
        super::probe::lap("trunk_shadow", tt);
        let tt = super::probe::t0();
        // r42:arcus-polish2 — each triangle's facing, whether it is a sleeve's inside and
        // whether the neck hides it, worked out once a frame: the left arm's second pass
        // finds them (and the neck's test is made per corner, not per corner of every
        // triangle).
        if !(same && sc.cls.len() == am.tris.len() && sc.cls_hid == hidden.is_some()) {
            sc.cls.clear();
            sc.depth.clear();
            sc.hidv.clear();
            if let Some((_, hid)) = hidden {
                sc.hidv.extend(sc.pts.iter().map(|&p| hid(p)));
            }
            sc.cls_hid = hidden.is_some();
            let near_side = {
                let o = hu::side(s);
                v.cam
                    .depth(base.at(o + hu::UPPER1, fig.bones[o + hu::UPPER1].head))
            };
            for (k, t) in am.tris.iter().enumerate() {
                let [a, bb, c] = t.map(|i| i as usize);
                let ctr = (sc.pts[a] + sc.pts[bb] + sc.pts[c]) * (1.0 / 3.0);
                sc.depth.push(v.cam.depth(ctr));
                // The figure's triangles are wound outward (MakeHuman's, kept by the
                // converter).
                let fnrm = (sc.pts[bb] - sc.pts[a]).cross(sc.pts[c] - sc.pts[a]);
                // r42:arcus-hand — the shirt's cuff stays under the jacket's sleeve: drawn
                // live its thinned band broke into white specks and jagged teeth against
                // the wrist (the review, both wrists, both cameras).
                let facing = fnrm.dot(eye - ctr) > 0.0 && am.stuff[k] != Stuff::Shirt;
                // r42:arcus-polish2 — a sleeve's triangle the pose drew out to over
                // [`STRETCH_MAX`] times its length at rest: the crook of a sharply bent
                // elbow, whose corners the two bones pull apart, poked out of the sleeve
                // as spikes (his left elbow beside the frog, the owner's review). It lies in
                // the fold; left out, the sleeve round it closes the view.
                // (Never in the band round the shoulder, which is drawn out on purpose.)
                let stretched = matches!(am.stuff[k], Stuff::Velvet | Stuff::Wool)
                    && [a, bb, c].iter().all(|&i| am.live[i] >= 1.0)
                    && {
                        let rest = |i: usize, j: usize| {
                            (fig.pos[am.verts[i] as usize] - fig.pos[am.verts[j] as usize]).len()
                        };
                        [(a, bb), (bb, c), (c, a)].iter().any(|&(i, j)| {
                            (sc.pts[i] - sc.pts[j]).len() > STRETCH_MAX * rest(i, j).max(0.05)
                        })
                    };
                // r42:arcus-polish2 — the band round the bow arm's shoulder turned away: drawn
                // as cloth's inside (deep in its shade). The bake leaves that band out where
                // the live arm is opaque; turned away in the arm's pose, the gap showed the
                // room as a bright pixel by the sleeve (the stray-pixel gate's).
                // (Only on the shoulder's near side: its far side lies behind the body.)
                // (The bow arm's only: the bake keeps the left arm's band.)
                let band_back = !facing
                    && s == 1
                    && matches!(am.stuff[k], Stuff::Velvet | Stuff::Wool)
                    && [a, bb, c].iter().any(|&i| am.live[i] < 1.0)
                    && sc.depth[k] < near_side;
                if band_back {
                    let behind = hidden.is_some_and(|(_, hid)| {
                        am.upper[k] || sc.hidv[a] || sc.hidv[bb] || sc.hidv[c] || hid(ctr)
                    });
                    sc.cls
                        .push(CLS_FACING | CLS_INSIDE | (CLS_BEHIND * behind as u8));
                    continue;
                }
                if !facing || stretched {
                    sc.cls.push(0);
                    continue;
                }
                // r42:arcus-polish — the inside of a sleeve's open end: near the cloth's
                // edge, a face turned in toward the forearm's axis is the hem's inside seen
                // through the opening (lit by the sleeve's rim it read as grey-blue points
                // beside his bow hand, the review): drawn deep in the sleeve's shade.
                let inside = matches!(am.stuff[k], Stuff::Velvet | Stuff::Wool)
                    && t.iter().all(|&i| {
                        fig.edge
                            .get(am.verts[i as usize] as usize)
                            .copied()
                            .unwrap_or(9.0)
                            < SLEEVE_INSIDE
                    })
                    && {
                        let ax = aim.wrist - aim.elbow;
                        let tt = ((ctr - aim.elbow).dot(ax) / ax.dot(ax).max(1e-6)).clamp(0.0, 1.2);
                        fnrm.dot(ctr - (aim.elbow + ax * tt)) < 0.0
                    };
                // (r42:arcus-hand: behind if any corner is — a triangle across the neck's
                // edge painted in front showed the hand over the neck; painted behind, the
                // neck's stamp covers just the part it hides.)
                let behind = hidden.is_some_and(|(_, hid)| {
                    am.upper[k] || sc.hidv[a] || sc.hidv[bb] || sc.hidv[c] || hid(ctr)
                });
                // r42:arcus-hands — a hand patch some of whose corners the neck hides and
                // some not.
                let split = behind
                    && am.fine[k]
                    && !am.upper[k]
                    && hidden.is_some_and(|(_, hid)| {
                        !(sc.hidv[a] && sc.hidv[bb] && sc.hidv[c] && hid(ctr))
                    });
                sc.cls.push(
                    CLS_FACING
                        | (CLS_INSIDE * inside as u8)
                        | (CLS_BEHIND * behind as u8)
                        | (CLS_SPLIT * split as u8),
                );
            }
        }
        sc.front.clear();
        sc.split.clear();
        sc.col.clear();
        sc.order.clear();
        with_lut(&shading, s, tone, mid, |lut| {
            for (k, t) in am.tris.iter().enumerate() {
                let [a, bb, c] = t.map(|i| i as usize);
                let cls = sc.cls[k];
                let mut show = cls & CLS_FACING != 0;
                let inside = cls & CLS_INSIDE != 0;
                // r42:arcus-hands — a hand patch across the neck's edge is drawn in both
                // passes, each of its small triangles in the pass its own middle belongs to
                // (painted behind whole, the neck's stamp cut it along its triangles: the
                // dark shards and stair-steps on the fingers at 100 % and 200 %).
                let split = show && fine_n > 1 && cls & CLS_SPLIT != 0;
                if show && !split {
                    if let Some((want, _)) = hidden {
                        show = (cls & CLS_BEHIND != 0) == want;
                    }
                }
                sc.front.push(show);
                sc.split.push(split);
                let st = am.stuff[k];
                let mut cc = [Color32::TRANSPARENT; 3];
                if show && inside {
                    // (r42:arcus-polish2: lit as the cloth's own outside a little in shade —
                    // drawn from the turned-away normal in deep shade, the cuff's inside read
                    // as a black ring at her bow wrist once the sleeve stopped turning with
                    // the hand.)
                    let c = lut.colour(st, [1.0, 0.0], sc.nrm[a], 0.6);
                    cc = [c; 3].map(|x| {
                        let k = INSIDE_SHADE;
                        Color32::from_rgb(
                            (x.r() as f32 * k) as u8,
                            (x.g() as f32 * k) as u8,
                            (x.b() as f32 * k) as u8,
                        )
                    });
                    sc.order.push((sc.depth[k] + 0.02, k as u32));
                } else if show {
                    for (j, &i) in [a, bb, c].iter().enumerate() {
                        // Each corner lit once a frame (a vertex where two stuffs meet, once
                        // each way).
                        let (kept_st, kept) = sc.vcol[i];
                        cc[j] = if kept_st == Some(st) {
                            kept
                        } else {
                            let mut d = am.vdet[i];
                            if st == Stuff::Skin {
                                d[0] *= am.vtex[i];
                            }
                            let mut cl = lut.colour(st, d, sc.nrm[i], sc.ao[i]);
                            if am.alpha[i] < 1.0 {
                                cl = cl.gamma_multiply(am.alpha[i]);
                            }
                            sc.vcol[i] = (Some(st), cl);
                            cl
                        };
                    }
                    // A shirt's cuff lies inside its sleeve: sorted a little behind it.
                    let bias = if st == Stuff::Shirt { 3.0 } else { 0.0 };
                    sc.order.push((sc.depth[k] + bias, k as u32));
                }
                // TEMP r42:arcus-polish2 debug: stretch (red) and slivers (blue).
                if std::env::var("CANTUS_ARM_DEBUG").is_ok_and(|v| v == "2") {
                    let rest = |i: usize, j: usize| {
                        (fig.pos[am.verts[i] as usize] - fig.pos[am.verts[j] as usize]).len()
                    };
                    let ratio = [(a, bb), (bb, c), (c, a)]
                        .iter()
                        .map(|&(i, j)| (sc.pts[i] - sc.pts[j]).len() / rest(i, j).max(0.05))
                        .fold(0.0f32, f32::max);
                    let (p0, p1, p2) = (sc.sp[a], sc.sp[bb], sc.sp[c]);
                    let area = ((p1 - p0).x * (p2 - p0).y - (p1 - p0).y * (p2 - p0).x).abs();
                    let long = (p1 - p0)
                        .length()
                        .max((p2 - p1).length())
                        .max((p0 - p2).length());
                    let thin = area / (long * long).max(1e-3);
                    let r = ((ratio - 1.0) * 180.0).clamp(0.0, 255.0) as u8;
                    let bl = if thin < 0.08 && long > 4.0 { 255 } else { 40 };
                    cc = [Color32::from_rgb(r, 40, bl); 3];
                } else if std::env::var_os("CANTUS_ARM_DEBUG").is_some() {
                    for (j, &i) in [a, bb, c].iter().enumerate() {
                        let x = cc[j];
                        let f = am.live[i];
                        cc[j] = if s == 0 {
                            Color32::from_rgb(
                                x.r().saturating_add(90),
                                (x.g() as f32 * f) as u8,
                                x.b(),
                            )
                        } else {
                            Color32::from_rgb(
                                (x.r() as f32 * f) as u8,
                                x.g().saturating_add(90),
                                x.b(),
                            )
                        };
                    }
                }
                sc.col.push(cc);
            }
        });
        super::probe::lap("tris", tt);
        let tt = super::probe::t0();
        // The outline: an edge between a shown triangle and one not shown (or none).
        let tri_n = am.tris.len() as u32;
        // r42:arcus-polish2 — each outline corner's way out, the mean of its edges' (by place:
        // a texture seam's copies are one corner), so neighbouring edges' soft strips meet
        // mitred: each edge's own strip overlapped its neighbour's at a turn (a brighter dot
        // on a sleeve's rim light, a darker one on a dark edge) and left a notch on the other
        // side (the room showing through) — the stray-pixel gate's dotted lines.
        sc.vout.clear();
        sc.vout.resize(am.verts.len(), egui::Vec2::ZERO);
        sc.eout.clear();
        sc.eout.resize(am.edges.len(), egui::Vec2::ZERO);
        for (e, &(a, bb, t0, t1)) in am.edges.iter().enumerate() {
            let f0 = t0 != u32::MAX && sc.front[t0 as usize];
            let f1 = t1 != u32::MAX && sc.front[t1 as usize];
            let seam = f0 && f1 && rank(am.stuff[t0 as usize]) != rank(am.stuff[t1 as usize]);
            if am.seam[e] || !(f0 != f1 || seam) {
                continue;
            }
            let o = outline_out(am, sc, e);
            sc.eout[e] = o;
            for i in [a, bb] {
                let w = am.weld[i as usize] as usize;
                sc.vout[w] += o;
            }
        }
        for (e, &(a, bb, t0, t1)) in am.edges.iter().enumerate() {
            let f0 = t0 != u32::MAX && sc.front[t0 as usize];
            let f1 = t1 != u32::MAX && sc.front[t1 as usize];
            // A seam's side: closed solid across the crack, then no outline (r42:arcus-hand).
            if am.seam[e] {
                if f0 {
                    sc.order
                        .push((v.cam.depth(sc.pts[a as usize]) + 0.01, tri_n + e as u32));
                }
                continue;
            }
            // r42:arcus-upper — and where two stuffs meet on the arm (a cuff on the wrist, a
            // shirt's cuff under a sleeve): the cloth's edge feathered over what is next to it,
            // so the cuff reads as a clean edge (triangles meet unsmoothed: a staircase).
            let seam = f0 && f1 && rank(am.stuff[t0 as usize]) != rank(am.stuff[t1 as usize]);
            if f0 != f1 || seam {
                let mid = (sc.pts[a as usize] + sc.pts[bb as usize]) * 0.5;
                // (r42:arcus-hands: a split patch's outline only in its middle's pass.)
                let split = [t0, t1]
                    .iter()
                    .any(|&t| t != u32::MAX && sc.split[t as usize]);
                if split && hidden.is_some_and(|(want, hid)| hid(mid) != want) {
                    continue;
                }
                // (r42:arcus-hand: a shirt's edge sorted with its cuff, behind the sleeve and
                // the wrist — drawn at its own depth it laid white threads over his wrist.)
                let shirt = [t0, t1]
                    .iter()
                    .any(|&t| t != u32::MAX && am.stuff[t as usize] == Stuff::Shirt);
                let bias = if shirt { 3.0 } else { 0.0 };
                sc.order
                    .push((v.cam.depth(mid) - 0.05 + bias, tri_n + e as u32));
            }
        }
        // r42:arcus-hands — the hand's creases, sorted a little in front of their skin.
        let marks_at = tri_n + am.edges.len() as u32;
        if fine_n > 1 {
            for (li, line) in am.marks.iter().enumerate() {
                let p = super::marks::middle(line, &am.tris, &sc.pts);
                if hidden.is_none_or(|(want, hid)| hid(p) == want) {
                    sc.order
                        .push((v.cam.depth(p) - MARK_AHEAD, marks_at + li as u32));
                }
            }
        }
        sc.order
            .sort_unstable_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
        super::probe::lap("edges+sort", tt);
        let tt = super::probe::t0();
        // r42:arcus-hands — High fills the arm's samples with their depths over its own
        // screen rectangle (`raster`); Light paints its triangles in depth order.
        let slot = s * 2 + matches!(hidden, Some((false, _))) as usize;
        let rast = !light;
        let ras_rect = if rast {
            let mut r = egui::Rect::NOTHING;
            for (k, t) in am.tris.iter().enumerate() {
                if sc.front[k] {
                    for &i in t {
                        r.extend_with(sc.sp[i as usize]);
                    }
                }
            }
            r.expand(3.0).intersect(b.clip)
        } else {
            egui::Rect::NOTHING
        };
        let fresh = rast
            && (sc.ras_key[slot] != Some((key, ras_rect)) || super::raster::stale(b.ctx(), slot));
        if rast && ras_rect.is_positive() && !fresh {
            // At rest: the last picture again (nothing filled, nothing uploaded).
            let rect = sc.ras.borrow().rect();
            let tex = super::raster::upload(b.ctx(), slot, &mut sc.ras.borrow_mut(), false);
            if let Some(tex) = tex {
                b.foreign(tex, rect);
            }
            return;
        }
        if rast {
            sc.ras_key[slot] = Some((key, ras_rect));
            let ss = if b.ppp() >= 1.5 { 2 } else { 3 };
            sc.ras.borrow_mut().begin(ras_rect, b.ppp(), ss);
        }
        let depth_of = |q: V3| v.cam.depth(q);
        for &(_, k) in &sc.order {
            if k >= marks_at {
                let line = &am.marks[(k - marks_at) as usize];
                if rast {
                    super::marks::stroke_into(
                        &mut sc.ras.borrow_mut(),
                        v,
                        line,
                        &am.tris,
                        &sc.pts,
                        &sc.nrm,
                        &sc.col,
                        eye,
                    );
                } else {
                    super::marks::stroke(b, v, line, &am.tris, &sc.pts, &sc.nrm, &sc.col, eye);
                }
                continue;
            }
            if k < tri_n {
                if fine_n > 1 && am.fine[k as usize] {
                    let fc = FineCtx {
                        v,
                        fig,
                        sh: &shading,
                        side: s,
                        tone,
                        mid,
                    };
                    let t1 = super::probe::t0();
                    let n = if sc.split[k as usize] {
                        SPLIT_N
                    } else {
                        fine_n
                    };
                    let only = if sc.split[k as usize] { hidden } else { None };
                    if rast {
                        let mut r = sc.ras.borrow_mut();
                        fine_tri(
                            &mut |p, q, c| r.tri(p, q.map(depth_of), c, None),
                            &fc,
                            sc,
                            am,
                            k as usize,
                            n,
                            only,
                        );
                    } else {
                        fine_tri(
                            &mut |p, _, c| b.shaded_tri(p, c),
                            &fc,
                            sc,
                            am,
                            k as usize,
                            n,
                            only,
                        );
                    }
                    super::probe::lap("fine_tri", t1);
                    continue;
                }
                let t = am.tris[k as usize];
                let p = t.map(|i| sc.sp[i as usize]);
                if rast {
                    let z = t.map(|i| depth_of(sc.pts[i as usize]));
                    sc.ras.borrow_mut().tri(p, z, sc.col[k as usize], None);
                } else {
                    b.shaded_tri(p, sc.col[k as usize]);
                }
            } else {
                let (a, bb, t0, t1) = am.edges[(k - tri_n) as usize];
                if am.seam[(k - tri_n) as usize] {
                    // Solid, a px and a half across the crack, the triangle's own colours.
                    let tri = am.tris[t0 as usize];
                    let corner = |i: u16| {
                        let j = tri.iter().position(|&x| x == i).unwrap_or(0);
                        sc.col[t0 as usize][j]
                    };
                    let (pa, pb) = (sc.sp[a as usize], sc.sp[bb as usize]);
                    let third = tri
                        .iter()
                        .find(|&&x| x != a && x != bb)
                        .copied()
                        .unwrap_or(a);
                    let d = pb - pa;
                    let mut out = egui::vec2(-d.y, d.x).normalized() * (1.5 * b.px());
                    if out.dot(sc.sp[third as usize] - pa) > 0.0 {
                        out = -out;
                    }
                    let (ca, cb) = (corner(a), corner(bb));
                    if rast {
                        let (za, zb) =
                            (depth_of(sc.pts[a as usize]), depth_of(sc.pts[bb as usize]));
                        let mut r = sc.ras.borrow_mut();
                        r.tri([pa, pb, pb + out], [za, zb, zb], [ca, cb, cb], None);
                        r.tri([pa, pb + out, pa + out], [za, zb, za], [ca, cb, ca], None);
                    } else {
                        b.shaded_tri([pa, pb, pb + out], [ca, cb, cb]);
                        b.shaded_tri([pa, pb + out, pa + out], [ca, cb, ca]);
                    }
                    continue;
                }
                // (r42:arcus-hands: High's samples cover the outline themselves.)
                if rast {
                    continue;
                }
                let both = t0 != u32::MAX
                    && t1 != u32::MAX
                    && sc.front[t0 as usize]
                    && sc.front[t1 as usize];
                let t = if both {
                    // A seam: from the one on top (the cloth over the skin, the sleeve
                    // over the shirt).
                    if rank(am.stuff[t0 as usize]) >= rank(am.stuff[t1 as usize]) {
                        t0
                    } else {
                        t1
                    }
                } else if t0 != u32::MAX && sc.front[t0 as usize] {
                    t0
                } else {
                    t1
                } as usize;
                let tri = am.tris[t];
                let corner = |i: u16| {
                    let j = tri.iter().position(|&x| x == i).unwrap_or(0);
                    sc.col[t][j]
                };
                let (pa, pb) = (sc.sp[a as usize], sc.sp[bb as usize]);
                let third = tri
                    .iter()
                    .find(|&&x| x != a && x != bb)
                    .copied()
                    .unwrap_or(a);
                let inside = sc.sp[third as usize];
                let d = pb - pa;
                let mut out = egui::vec2(-d.y, d.x);
                if out.dot(inside - pa) > 0.0 {
                    out = -out;
                }
                let e = (k - tri_n) as usize;
                // (r42:arcus-polish: curved only where the patch curves that edge.)
                let ka = tri.iter().position(|&x| x == a).unwrap_or(0);
                let kb = tri.iter().position(|&x| x == bb).unwrap_or(0);
                let ke = if (ka + 1) % 3 == kb { ka } else { kb };
                if out.length() > 1e-4 {
                    if fine_n > 1 && am.fine[t] && am.curved[t] & (1 << ke) != 0 {
                        // Along the patch's curved edge (r42:arcus-hand).
                        let (ia, ib) = (a as usize, bb as usize);
                        let (p0, p1, n0, n1) = (sc.pts[ia], sc.pts[ib], sc.nrm[ia], sc.nrm[ib]);
                        let c0 = (p0 * 2.0 + p1 - n0 * (p1 - p0).dot(n0)) * (1.0 / 3.0);
                        let c1 = (p1 * 2.0 + p0 - n1 * (p0 - p1).dot(n1)) * (1.0 / 3.0);
                        let at = |t: f32| {
                            let u = 1.0 - t;
                            p0 * (u * u * u)
                                + c0 * (3.0 * u * u * t)
                                + c1 * (3.0 * u * t * t)
                                + p1 * (t * t * t)
                        };
                        let (ca, cb) = (corner(a), corner(bb));
                        let mut prev = (pa, ca);
                        for j in 1..=fine_n {
                            let t = j as f32 / fine_n as f32;
                            let q = (v.p(at(t)), lerp_c(ca, cb, t));
                            b.edge(prev.0, q.0, prev.1, q.1, out);
                            prev = q;
                        }
                    } else {
                        // (r42:arcus-polish2: mitred with its neighbours.)
                        let o = sc.eout[e];
                        let join = |i: u16| {
                            let m = sc.vout[am.weld[i as usize] as usize];
                            if m.length() < 1e-3 || o == egui::Vec2::ZERO {
                                return o;
                            }
                            let m = m.normalized();
                            // (A sharp turn keeps the edge's own way: a long miter spikes.)
                            let c = m.dot(o);
                            if c < 0.5 {
                                o
                            } else {
                                m / c
                            }
                        };
                        let px = b.px();
                        // (r42:arcus-polish2: the strip in its corners' colours drawn halfway
                        // toward the triangle's inner corner's: a sleeve's rim light is
                        // brightest exactly on its outline, and its corners alternate there
                        // between more and less grazing — the strip in their own colours
                        // drew a dotted bright line a pixel wide along every sleeve.)
                        let ci = corner(third);
                        let (ca, cb) = (lerp_c(corner(a), ci, 0.5), lerp_c(corner(bb), ci, 0.5));
                        b.edge_joined(pa, pb, ca, cb, join(a) * px, join(bb) * px);
                    }
                }
            }
        }
        if rast && ras_rect.is_positive() {
            let rect = sc.ras.borrow().rect();
            let tex = super::raster::upload(b.ctx(), slot, &mut sc.ras.borrow_mut(), true);
            if let Some(tex) = tex {
                b.foreign(tex, rect);
            }
        }
        super::probe::lap("paint(incl fine)", tt);
    });
}

/// r42:arcus-hands — each vertex's skin texture (`cards::skin_detail` at its place) and
/// the back of the hand's relief there (`Relief`): what a patch's corner is lit with.
fn vertex_tex(fig: &Figure, s: usize, verts: &[u32]) -> Vec<f32> {
    let relief = Relief::of(fig, s);
    verts
        .iter()
        .map(|&g| {
            let g = g as usize;
            let t = match fig.uv.get(g) {
                Some(uv) if uv[0] >= 0.0 => crate::scene::figure::cards::skin_detail(uv[0], uv[1]),
                _ => 1.0,
            };
            t * relief.at(fig.pos[g])
        })
        .collect()
}

/// r42:arcus-hands — each triangle's three edges as indices into the mesh's distinct edges
/// (by its own vertices: a texture seam's two copies are two edges, as their colours
/// differ), and how many distinct edges there are.
fn edge_index(tris: &[[u16; 3]]) -> (Vec<[u32; 3]>, usize) {
    let mut seen: std::collections::HashMap<(u16, u16), u32> = Default::default();
    let out = tris
        .iter()
        .map(|t| {
            [0, 1, 2].map(|k| {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let n = seen.len() as u32;
                *seen.entry((a.min(b), a.max(b))).or_insert(n)
            })
        })
        .collect();
    (out, seen.len())
}

/// r42:arcus-hands — the hand patches' shared points this frame: each corner's and each
/// edge midpoint's place on screen, place in the room and colour, with what it was lit as
/// (the stuff and whether textured): two patches on one edge work it out once (the
/// budget: every point of 1 850 patches lit on its own cost 2.7–3.3 ms at 200 %).
#[derive(Default)]
struct FineCache {
    v: Vec<Option<(u8, Pos2, V3, Color32)>>,
    e: Vec<Option<(u8, Pos2, V3, Color32)>>,
}

/// r42:arcus-polish2 — outline edge `e`'s way out on screen (unit), away from the shown
/// triangle it belongs to (the one on top where two stuffs meet), as the outline is drawn.
fn outline_out(am: &ArmMesh, sc: &Scratch, e: usize) -> egui::Vec2 {
    let (a, bb, t0, t1) = am.edges[e];
    let shown = |t: u32| t != u32::MAX && sc.front[t as usize];
    let t = if shown(t0) && shown(t1) {
        if rank(am.stuff[t0 as usize]) >= rank(am.stuff[t1 as usize]) {
            t0
        } else {
            t1
        }
    } else if shown(t0) {
        t0
    } else {
        t1
    } as usize;
    let tri = am.tris[t];
    let third = tri
        .iter()
        .find(|&&x| x != a && x != bb)
        .copied()
        .unwrap_or(a);
    let (pa, pb) = (sc.sp[a as usize], sc.sp[bb as usize]);
    let d = pb - pa;
    let mut out = egui::vec2(-d.y, d.x);
    if out.dot(sc.sp[third as usize] - pa) > 0.0 {
        out = -out;
    }
    if out.length() > 1e-4 {
        out.normalized()
    } else {
        egui::Vec2::ZERO
    }
}

/// r42:arcus-polish — which of each fine triangle's edges are shared with another fine
/// triangle (by place: a texture seam's copies count as one), so the patch may curve them.
fn curved_edges(tris: &[[u16; 3]], fine: &[bool], weld: &[u16]) -> Vec<u8> {
    let key = |a: u16, b: u16| {
        let (a, b) = (weld[a as usize], weld[b as usize]);
        (a.min(b), a.max(b))
    };
    let mut count: std::collections::HashMap<(u16, u16), u8> = Default::default();
    for (t, &f) in tris.iter().zip(fine) {
        if f {
            for k in 0..3 {
                *count.entry(key(t[k], t[(k + 1) % 3])).or_insert(0) += 1;
            }
        }
    }
    tris.iter()
        .zip(fine)
        .map(|(t, &f)| {
            if !f {
                return 0;
            }
            (0..3).fold(0u8, |m, k| {
                if count.get(&key(t[k], t[(k + 1) % 3])).copied().unwrap_or(0) >= 2 {
                    m | (1 << k)
                } else {
                    m
                }
            })
        })
        .collect()
}

/// r42:arcus-polish — the back of a hand's forms at rest, for the tone per point of High's
/// patches: the extensor tendons from the wrist's middle to each knuckle a little lighter,
/// the valleys between the metacarpals a little darker, each base knuckle's top lighter with
/// a soft shade round it — light and shade only (the mesh, a centimetre a vertex there, holds
/// no such relief; the light comes from the upper front, so ridges read lighter).
struct Relief {
    wr: V3,
    heads: [V3; 4],
    back: V3,
}

impl Relief {
    fn of(fig: &Figure, s: usize) -> Relief {
        let o = hu::side(s);
        let kn = |f: usize| fig.bones[o + hu::finger(f, 0)].head;
        let wr = fig.bones[o + hu::HAND].head;
        let mut back = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
        if (back.x < 0.0) == (s == 0) {
            back = back * -1.0;
        }
        Relief {
            wr,
            heads: [kn(1), kn(2), kn(3), kn(4)],
            back,
        }
    }

    /// The luminance factor at `p` (rest, cm).
    fn at(&self, p: V3) -> f32 {
        let mut near = f32::MAX;
        let mut knuckle = 0.0f32;
        let mut on_back = 0.0f32;
        let mut along = 0.0f32;
        for &h in &self.heads {
            let a = self.wr.lerp(h, 0.2);
            let ab = h - a;
            let t = ((p - a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
            let q = p - (a + ab * t);
            let up = q.dot(self.back);
            let lat = (q - self.back * up).len();
            if lat < near {
                near = lat;
                on_back = (up / 0.6).clamp(0.0, 1.0);
                along = t;
            }
            let dk = (p - h).len();
            knuckle = knuckle
                .max((-((dk - 0.2) / 0.45).powi(2)).exp() * (q.dot(self.back) > 0.0) as u8 as f32);
        }
        if on_back <= 0.0 {
            return 1.0;
        }
        let ridge = (-(near / 0.22) * (near / 0.22)).exp();
        let valley = 1.0 - (-(near / 0.5) * (near / 0.5)).exp();
        let span = smooth01((along - 0.05) / 0.25) * (1.0 - smooth01((along - 0.9) / 0.15));
        1.0 + on_back * (span * (RELIEF.0 * ridge - RELIEF.1 * valley) + RELIEF.2 * knuckle)
    }
}

/// The tendons' lift, the valleys' shade, the knuckles' lift (luminance shares).
const RELIEF: (f32, f32, f32) = (0.1, 0.13, 0.08);

/// r42:arcus-hand — High's hand patches: a thinned triangle's edge (cm) and the px a
/// patch's small triangle may take on screen; at most this many cuts a side.
const FINE_EDGE: f32 = 0.62;
const FINE_PX: f32 = 4.0;
const FINE_MAX: usize = 2;
/// r42:arcus-hands — the cuts a side of a hand patch split between the passes (its small
/// triangles follow the neck's edge to about a pixel).
const SPLIT_N: usize = 4;
/// r42:arcus-hands — the hand's own occlusion: its gain and the most it darkens.
const AO_K: f32 = 0.45;
const AO_MOST: f32 = 0.4;
/// r42:arcus-hands — how far (cm) a crease is sorted in front of the skin it lies on.
const MARK_AHEAD: f32 = 0.12;

/// What a hand patch is lit and placed by.
struct FineCtx<'a> {
    v: &'a View,
    fig: &'a Figure,
    sh: &'a Shading,
    side: usize,
    tone: u8,
    mid: V3,
}

fn lerp_c(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        m(a.r(), b.r()),
        m(a.g(), b.g()),
        m(a.b(), b.b()),
        m(a.a(), b.a()),
    )
}

/// r42:arcus-hand — a hand triangle as a curved patch (PN triangles, Vlachos, Peters, Boyd
/// and Mitchell 2001: the cubic through its corners that meets their normals, so the thinned
/// hand's facets round off and its outline is smooth), cut `n` times a side, each point lit
/// from its own normal with the skin's own texture at its place (MakeHuman's knuckle
/// creases, the lines of the fingers' joints, the pores; as the bake does the face), the
/// corners' detail and occlusion between.
fn fine_tri(
    out: &mut dyn FnMut([Pos2; 3], [V3; 3], [Color32; 3]),
    fc: &FineCtx,
    sc: &Scratch,
    am: &ArmMesh,
    k: usize,
    n: usize,
    only: Option<(bool, &dyn Fn(V3) -> bool)>,
) {
    let t = am.tris[k].map(|i| i as usize);
    let st = am.stuff[k];
    let p = t.map(|i| sc.pts[i]);
    let nn = t.map(|i| sc.nrm[i]);
    let e =
        |i: usize, j: usize| (p[i] * 2.0 + p[j] - nn[i] * (p[j] - p[i]).dot(nn[i])) * (1.0 / 3.0);
    let (b210, b120, b021, b012, b102, b201) =
        (e(0, 1), e(1, 0), e(1, 2), e(2, 1), e(2, 0), e(0, 2));
    let ee = (b210 + b120 + b021 + b012 + b102 + b201) * (1.0 / 6.0);
    let vv = (p[0] + p[1] + p[2]) * (1.0 / 3.0);
    let b111 = ee + (ee - vv) * 0.5;
    let uv = t.map(|i| {
        fc.fig
            .uv
            .get(am.verts[i] as usize)
            .copied()
            .unwrap_or([-1.0, -1.0])
    });
    let textured = st == Stuff::Skin && uv.iter().all(|u| u[0] >= 0.0);
    let det = t.map(|i| am.vdet[i]);
    // r42:arcus-polish — the back of the hand's forms (the owner: "a puffy, featureless
    // dome… no tendon, metacarpal or knuckle definition"), per point from where it lies at
    // rest.
    let rest = t.map(|i| fc.fig.pos[am.verts[i] as usize]);
    let relief = (st == Stuff::Skin).then(|| Relief::of(fc.fig, fc.side));
    let ao = t.map(|i| sc.ao[i]);
    let mut pts = [Pos2::ZERO; 15];
    let mut room = [V3::default(); 15];
    let mut cols = [Color32::TRANSPARENT; 15];
    let row = |i: usize| i * (n + 1) - i * i.saturating_sub(1) / 2;
    let lit_as = stuff_slot(st) as u8 * 2 + textured as u8;
    let mut cache = sc.fine.borrow_mut();
    for i in 0..=n {
        for j in 0..=(n - i) {
            // r42:arcus-hands — a corner or an edge's midpoint once a frame (two cuts a
            // side: every point is one of them).
            let shared = (n == 2).then(|| match (i, j) {
                (0, 0) => (true, t[0]),
                (0, 2) => (true, t[1]),
                (2, 0) => (true, t[2]),
                (0, 1) => (false, am.tri_edges[k][0] as usize),
                (1, 1) => (false, am.tri_edges[k][1] as usize),
                _ => (false, am.tri_edges[k][2] as usize),
            });
            let at = row(i) + j;
            if let Some((corner, ix)) = shared {
                let slot = if corner {
                    cache.v.get(ix)
                } else {
                    cache.e.get(ix)
                };
                if let Some(Some((key, sp, q, c))) = slot {
                    if *key == lit_as {
                        pts[at] = *sp;
                        room[at] = *q;
                        cols[at] = *c;
                        continue;
                    }
                }
            }
            let (w1, w2) = (j as f32 / n as f32, i as f32 / n as f32);
            let w0 = 1.0 - w1 - w2;
            // r42:arcus-polish — a point on an edge is worked out the same way from either
            // triangle (from the edge's lower-numbered end; straight against a flat
            // neighbour): two ways of summing it differed in the last bit and left pixel
            // cracks between the patches.
            let on_edge = if i == 0 {
                Some((0usize, 1usize, w1))
            } else if i + j == n {
                Some((1, 2, w2))
            } else if j == 0 {
                Some((2, 0, w0))
            } else {
                None
            };
            let q = if let Some((ka, kb, sa)) = on_edge {
                let curved = am.curved[k] & (1 << ka) != 0;
                let (ka, kb, s_) = if am.weld[t[ka]] > am.weld[t[kb]]
                    || (am.weld[t[ka]] == am.weld[t[kb]] && t[ka] > t[kb])
                {
                    (kb, ka, 1.0 - sa)
                } else {
                    (ka, kb, sa)
                };
                let (pa, pb) = (p[ka], p[kb]);
                if curved {
                    let (ca, cb) = (e(ka, kb), e(kb, ka));
                    let u = 1.0 - s_;
                    pa * (u * u * u)
                        + ca * (3.0 * u * u * s_)
                        + cb * (3.0 * u * s_ * s_)
                        + pb * (s_ * s_ * s_)
                } else {
                    pa * (1.0 - s_) + pb * s_
                }
            } else {
                p[0] * (w0 * w0 * w0)
                    + p[1] * (w1 * w1 * w1)
                    + p[2] * (w2 * w2 * w2)
                    + (b210 * (w0 * w0 * w1)
                        + b120 * (w0 * w1 * w1)
                        + b201 * (w0 * w0 * w2)
                        + b021 * (w1 * w1 * w2)
                        + b102 * (w0 * w2 * w2)
                        + b012 * (w1 * w2 * w2))
                        * 3.0
                    + b111 * (6.0 * w0 * w1 * w2)
            };
            let nq = (nn[0] * w0 + nn[1] * w1 + nn[2] * w2).norm();
            let mut d = [
                det[0][0] * w0 + det[1][0] * w1 + det[2][0] * w2,
                det[0][1] * w0 + det[1][1] * w1 + det[2][1] * w2,
            ];
            if textured {
                let u = uv[0][0] * w0 + uv[1][0] * w1 + uv[2][0] * w2;
                let vt = uv[0][1] * w0 + uv[1][1] * w1 + uv[2][1] * w2;
                d[0] *= crate::scene::figure::cards::skin_detail(u, vt);
            }
            if let Some(r) = &relief {
                d[0] *= r.at(rest[0] * w0 + rest[1] * w1 + rest[2] * w2);
            }
            let a = ao[0] * w0 + ao[1] * w1 + ao[2] * w2;
            room[at] = q;
            pts[at] = fc.v.p(q);
            cols[at] = lut_colour(fc.sh, fc.side, st, fc.tone, d, fc.mid, nq, a);
            if let Some((corner, ix)) = shared {
                let slot = if corner {
                    cache.v.get_mut(ix)
                } else {
                    cache.e.get_mut(ix)
                };
                if let Some(x) = slot {
                    *x = Some((lit_as, pts[at], q, cols[at]));
                }
            }
        }
    }
    drop(cache);
    for i in 0..n {
        for j in 0..(n - i) {
            let (a, bb, c) = (row(i) + j, row(i) + j + 1, row(i + 1) + j);
            // (r42:arcus-hands: a split patch's small triangle only in its own pass.)
            let mine = |x: usize, y: usize, z: usize| {
                only.is_none_or(|(want, hid)| {
                    hid((room[x] + room[y] + room[z]) * (1.0 / 3.0)) == want
                })
            };
            if mine(a, bb, c) {
                out(
                    [pts[a], pts[bb], pts[c]],
                    [room[a], room[bb], room[c]],
                    [cols[a], cols[bb], cols[c]],
                );
            }
            if j + 1 < n - i {
                let d = row(i + 1) + j + 1;
                if mine(bb, d, c) {
                    out(
                        [pts[bb], pts[d], pts[c]],
                        [room[bb], room[d], room[c]],
                        [cols[bb], cols[d], cols[c]],
                    );
                }
            }
        }
    }
}

/// r42:arcus-upper — which stuff lies over which where two meet on an arm: the sleeve's
/// cloth over the shirt's cuff over the skin and the nails.
fn rank(st: Stuff) -> u8 {
    match st {
        Stuff::Velvet | Stuff::Wool => 2,
        Stuff::Shirt => 1,
        _ => 0,
    }
}

/// Each corner's share of the light round it (`sc.ao`): the hand's own pieces (the
/// palm, the fingers' phalanges as capsules) and what it holds shade the corners near
/// them that face them — the creases between fingers, a finger's side against the stick,
/// the palm under the fingers — as a real hand's are. A vertex is shaded by no piece of
/// its own finger next to its own bone. About 1 600 corners × 20 capsules a frame.
fn occlusion(
    fig: &hu::Figure,
    posed: &Posed,
    s: usize,
    am: &ArmMesh,
    held: &[Capsule],
    sc: &mut Scratch,
) {
    let o = hu::side(s);
    sc.caps.clear();
    for f in 0..5 {
        for k in 0..3 {
            let b = o + hu::finger(f, k);
            let bone = &fig.bones[b];
            let r = [0.95, 0.85, 0.85, 0.8, 0.68][f] * [1.0, 0.92, 0.82][k];
            sc.caps.push(Capsule {
                a: posed.at(b, bone.head),
                b: posed.at(b, bone.tail),
                r,
                bone: b,
            });
        }
    }
    // The palm: from the wrist toward the index's and the little finger's knuckles.
    let wr = posed.at(o + hu::HAND, fig.bones[o + hu::HAND].head);
    for f in [1usize, 4] {
        let kb = o + hu::finger(f, 0);
        let kn = posed.at(kb, fig.bones[kb].head);
        sc.caps.push(Capsule {
            a: wr.lerp(kn, 0.25),
            b: wr.lerp(kn, 0.85),
            r: 1.1,
            bone: o + hu::HAND,
        });
    }
    sc.caps.extend_from_slice(held);
    sc.ao.clear();
    for (j, &g) in am.verts.iter().enumerate() {
        let w = &fig.bw[g as usize];
        let (mut main, mut best) = (usize::MAX, 0.0);
        let mut hand = 0.0;
        for &(b, x) in w {
            let b = b as usize;
            if x > best {
                best = x;
                main = b;
            }
            if b >= o + hu::HAND && b <= o + hu::finger(4, 2) {
                hand += x;
            }
        }
        if hand < 0.05 {
            sc.ao.push(1.0);
            continue;
        }
        let (p, n) = (sc.pts[j], sc.nrm[j]);
        let near = |b: usize| {
            if b == usize::MAX || main == usize::MAX {
                return false;
            }
            if b == main {
                return true;
            }
            // The same finger's neighbouring phalanx, or the palm against a finger's base.
            let fb = |x: usize| (x >= o + hu::finger(0, 0)).then(|| (x - o - hu::finger(0, 0)) / 3);
            let kk = |x: usize| (x - o - hu::finger(0, 0)) % 3;
            match (fb(b), fb(main)) {
                (Some(f1), Some(f2)) => f1 == f2 && kk(b).abs_diff(kk(main)) <= 1,
                (None, Some(_)) => b == o + hu::HAND && kk(main) == 0,
                (Some(_), None) => main == o + hu::HAND && kk(b) == 0,
                (None, None) => true,
            }
        };
        let mut occ = 0.0f32;
        for c in &sc.caps {
            if near(c.bone) {
                continue;
            }
            occ += crate::scene::figure::live::shade(c, p, n);
        }
        // (r42:arcus-hands: softer — 0.62 at most left the slivers of a finger's shaded side
        // between two others as dark one-pixel dots and dashes, polish2's stray gate.)
        sc.ao.push(1.0 - (occ * AO_K * hand.min(1.0)).min(AO_MOST));
    }
}

/// r42:arcus-polish — the sleeve gathered in the elbow's crook (the owner, 2026-11-25:
/// folds at the inner elbow; "the arm must read as an arm, with a visible elbow"): round
/// the bent elbow, on the side the arm folds toward, the normals swung to and fro along the
/// arm (soft folds about [`FOLD_CM`] apart) and their creases shaded, as deep as the elbow
/// is bent. The hand's skin is never touched.
fn elbow_folds(fig: &Figure, posed: &Posed, s: usize, am: &ArmMesh, sc: &mut Scratch) {
    let o = hu::side(s);
    let at = |b: usize| posed.at(o + b, fig.bones[o + b].head);
    let (sh, el, wr) = (at(hu::UPPER1), at(hu::FORE1), at(hu::HAND));
    let (u, f) = ((sh - el).norm(), (wr - el).norm());
    // 0 straight … 1 bent square or more.
    let bend = ((u.dot(f) + 1.0) * 1.2).clamp(0.0, 1.0);
    if bend < 0.05 {
        return;
    }
    let crook = (u + f).norm();
    for (j, &g) in am.verts.iter().enumerate() {
        let hand: f32 = fig.bw[g as usize]
            .iter()
            .filter(|(b, _)| (*b as usize) >= o + hu::HAND && (*b as usize) <= o + hu::finger(4, 2))
            .map(|x| x.1)
            .sum();
        if hand > 0.3 {
            continue;
        }
        let d = sc.pts[j] - el;
        let (a, b) = (d.dot(u), d.dot(f));
        // Along the arm from the elbow: + up the upper arm, − down the forearm.
        let t = if a >= b { a } else { -b };
        let near = 1.0 - smooth01((t.abs() - 2.0) / 5.0);
        let n = sc.nrm[j];
        let inside = n.dot(crook).max(0.0).powf(0.7);
        let w = bend * near * inside * (1.0 - hand / 0.3);
        if w < 0.02 {
            continue;
        }
        let ph = t / FOLD_CM * std::f32::consts::TAU + 0.6 * (t * 0.9).sin();
        let ax = if t >= 0.0 { u } else { -f };
        sc.nrm[j] = (n + ax * (0.75 * w * ph.sin())).norm();
        sc.ao[j] *= 1.0 - 0.45 * w * (-ph.cos()).max(0.0).powi(2);
    }
}

/// r42:arcus-polish — the shadow an arm lays on the trunk it crosses in front of (the
/// owner, 2026-11-25: "wherever an arm crosses the torso, a contact shadow"): the trunk as
/// three capsules (the hips, the spine, the chest, posed as baked, a little inside the
/// figure's own surface). The arm is taken as its three pieces (the upper arm, the forearm,
/// the hand, each a capsule of the sleeve's or the hand's girth); at a few points along each
/// the trunk is looked for behind it along the eye's ray ([`SHADOW_REACH`] cm), and the
/// piece is cast along the key's way onto it as a soft band — darker the closer the arm
/// lies, wider and softer the farther (the key is a softbox: its penumbra grows with the
/// distance), clear at its sides and wherever the trunk is not behind (never over the room).
/// (r42:arcus-polish2: drawn from the pieces, not from every one of the arm's triangles —
/// the frame budget: the triangles' shadow cost the violinists about a millisecond.)
#[allow(clippy::too_many_arguments)]
fn trunk_shadow(
    b: &mut Brush,
    v: &View,
    lit: &Lit,
    fig: &Figure,
    base: &Posed,
    aim: &ArmAim,
    sh: &Shading,
    who: Who,
    s: usize,
    tone: u8,
    mid: V3,
) {
    if fig.bones.is_empty() {
        return;
    }
    let at = |k: usize| base.at(k, fig.bones[k].head);
    // The trunk's half width at the chest, from the shoulders' joints.
    let r = (at(hu::side(0) + hu::UPPER1) - at(hu::side(1) + hu::UPPER1)).len() * 0.29;
    let caps = [
        (at(hu::HIPS), at(hu::SPINE), r * 1.05),
        (at(hu::SPINE), at(hu::CHEST), r * 0.95),
        (at(hu::CHEST), at(hu::NECK), r),
    ];
    let inside = |p: V3| {
        caps.iter().any(|&(a, c, rr)| {
            let ab = c - a;
            let t = ((p - a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
            (p - (a + ab * t)).len() < rr
        })
    };
    let eye = v.cam.eye();
    // How far behind `p` (along the eye's ray) the trunk is, if within reach.
    let gap = |p: V3| {
        let ray = (p - eye).norm();
        (1..=SHADOW_STEPS)
            .map(|k| SHADOW_REACH * k as f32 / SHADOW_STEPS as f32)
            .find(|&d| inside(p + ray * d))
    };
    let deep = lut_colour(
        sh,
        s,
        Stuff::Velvet,
        tone,
        [1.0, 0.0],
        mid,
        v3(0.0, -1.0, 0.0),
        0.0,
    );
    let shoulder = at(hu::side(s) + hu::UPPER1);
    let knuckles = aim
        .hand
        .map_or(aim.wrist + (aim.wrist - aim.elbow) * 0.35, |h| {
            h.fingers[1][0]
        });
    // The pieces' girths (cm, as radii): a slim woman's velvet sleeve, a man's jacket.
    let girth = if who == Who::Man {
        [5.6, 4.6, 3.0]
    } else {
        [4.2, 3.4, 2.6]
    };
    // Each piece's strength along it: halved where two meet (their bands overlap at the
    // elbow and the wrist), the hand's fading out toward the knuckles.
    const MEET: [f32; 5] = [0.5, 1.0, 1.0, 1.0, 0.5];
    const HAND: [f32; 5] = [0.5, 1.0, 0.85, 0.45, 0.0];
    let pieces = [
        (shoulder, aim.elbow, girth[0], MEET),
        (aim.elbow, aim.wrist, girth[1], MEET),
        (aim.wrist, knuckles, girth[2], HAND),
    ];
    const N: usize = 5;
    for (p0, p1, rad, taper) in pieces {
        let ax = p1 - p0;
        if ax.len() < 0.5 {
            continue;
        }
        let mut rows = [[(Pos2::ZERO, Color32::TRANSPARENT); 3]; N];
        let mut any = false;
        for (j, row) in rows.iter_mut().enumerate() {
            let c = p0 + ax * (j as f32 / (N - 1) as f32);
            let l = (lit.key - c).norm();
            // Across the piece as the eye sees it.
            let side = ax.cross(c - eye).norm();
            let g = gap(c);
            let Some(g) = g else {
                let q = v.p(c - l * SHADOW_REACH * 0.5);
                let w = side * (rad * 1.2);
                *row = [
                    (v.p(c - w - l * SHADOW_REACH * 0.5), Color32::TRANSPARENT),
                    (q, Color32::TRANSPARENT),
                    (v.p(c + w - l * SHADOW_REACH * 0.5), Color32::TRANSPARENT),
                ];
                continue;
            };
            any = true;
            let k = (1.0 - g / SHADOW_REACH).clamp(0.0, 1.0) * SHADOW_DEPTH * taper[j];
            // The penumbra widens with the gap.
            let w = side * (rad * (0.9 + 0.06 * g));
            // A side whose ray finds no trunk is drawn in (the band ends at the trunk's edge,
            // not over the room).
            let cast = |o: V3| {
                let o = if gap(c + o).is_some() { o } else { o * 0.35 };
                v.p(c + o - l * g)
            };
            let (pl, pc, pr) = (cast(-w), v.p(c - l * g), cast(w));
            // The middle's strength, clear at both sides (a soft band).
            *row = [
                (pl, Color32::TRANSPARENT),
                (pc, deep.gamma_multiply(k)),
                (pr, Color32::TRANSPARENT),
            ];
        }
        if !any {
            continue;
        }
        for j in 1..N {
            let (a, c) = (rows[j - 1], rows[j]);
            for (x, y) in [(0usize, 1usize), (1, 2)] {
                b.shaded_tri([a[x].0, a[y].0, c[y].0], [a[x].1, a[y].1, c[y].1]);
                b.shaded_tri([a[x].0, c[y].0, c[x].0], [a[x].1, c[y].1, c[x].1]);
            }
        }
    }
}

/// How far behind an arm (cm) the trunk may be for its shadow to fall on it, in how many
/// steps that is looked along, and the shadow's opacity where the arm touches.
const SHADOW_REACH: f32 = 9.0;
const SHADOW_STEPS: usize = 3;
const SHADOW_DEPTH: f32 = 0.42;

/// r42:arcus-polish — how far in from a sleeve's edge (cm) its inside is drawn, and how
/// deep its shade is (of the cloth's own colour facing away).
const SLEEVE_INSIDE: f32 = 2.5;
const INSIDE_SHADE: f32 = 0.72;

/// r42:arcus-polish — how strongly the back of the hand's veins show.
const VEIN: f32 = 0.8;

/// How far apart the elbow's folds lie (cm).
const FOLD_CM: f32 = 2.4;

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Skin arm `s`, its normals, its occlusion; clear the kept colours. Returns the arm's
/// middle (where its light is taken).
#[allow(clippy::too_many_arguments)]
fn arm_setup(
    fig: &Figure,
    am: &ArmMesh,
    base: &Posed,
    s: usize,
    aim: &ArmAim,
    v: &View,
    held: &[Capsule],
    sc: &mut Scratch,
    light: bool,
) -> V3 {
    let tt = super::probe::t0();
    let posed = hu::repose_arm(fig, base, s, aim);
    let n = am.verts.len();
    sc.pts.clear();
    sc.nrm.clear();
    sc.sp.clear();
    // r42:arcus-polish2 — the arm's own bones as dual quaternions (none for a bone the pose
    // stretches: a finger's phalanx), so a vertex the shoulder's or the elbow's bones share
    // turns round the joint instead of being averaged through it: linear blending drew the
    // raised bow arm's shoulder in toward the joint on top and out underneath — broad, a
    // pad (the owner's review). Kavan, Collins, Žára and O'Sullivan 2007.
    sc.dq.clear();
    sc.dq.extend(posed.xf.iter().map(dual));
    let o = hu::side(s);
    let hand = o + hu::HAND..=o + hu::finger(4, 2);
    for (j, &i) in am.verts.iter().enumerate() {
        // r42:arcus-polish2 — a sleeve's cuff follows the forearm: the share of the hand's
        // bones in the hem's weights turned it with the wrist into spikes and a jagged end
        // (his left sleeve, the owner's review of sheet v11).
        let mut p = if am.sleeve[j]
            && fig.bw[i as usize]
                .iter()
                .any(|(b, _)| hand.contains(&(*b as usize)))
        {
            let q = fig.pos[i as usize];
            fig.bw[i as usize]
                .iter()
                .fold(V3::default(), |acc, &(b, w)| {
                    let b = if hand.contains(&(b as usize)) {
                        o + hu::FORE2
                    } else {
                        b as usize
                    };
                    acc + posed.xf[b.min(hu::BONES - 1)].apply(q) * w
                })
        } else {
            dq_vertex(fig, &posed, &sc.dq, i as usize)
                .unwrap_or_else(|| posed.vertex(fig, i as usize))
        };
        if am.live[j] < 1.0 {
            p = base.vertex(fig, i as usize).lerp(p, am.live[j]);
        }
        sc.pts.push(p);
        sc.sp.push(v.p(p));
    }
    sc.nrm.resize(n, V3::default());
    for t in &am.tris {
        let [a, bb, c] = t.map(|i| i as usize);
        let f = (sc.pts[bb] - sc.pts[a]).cross(sc.pts[c] - sc.pts[a]);
        for i in [a, bb, c] {
            let w = am.weld[i] as usize;
            sc.nrm[w] = sc.nrm[w] + f;
        }
    }
    for i in 0..n {
        sc.nrm[i] = sc.nrm[am.weld[i] as usize];
    }
    for q in &mut sc.nrm {
        *q = q.norm();
    }
    super::probe::lap("  skin+nrm", tt);
    let tt = super::probe::t0();
    if light {
        // Light: no creases' occlusion (the hand's own capsules a frame).
        sc.ao.clear();
        sc.ao.resize(n, 1.0);
    } else {
        occlusion(fig, &posed, s, am, held, sc);
    }
    super::probe::lap("  occl", tt);
    cuff_shadow(fig, am, sc);
    let tt = super::probe::t0();
    elbow_folds(fig, &posed, s, am, sc);
    super::probe::lap("  folds", tt);
    sc.vcol.clear();
    sc.vcol.resize(n, (None, Color32::TRANSPARENT));
    sc.mid = sc.pts.iter().fold(V3::default(), |a, &p| a + p) * (1.0 / n.max(1) as f32);
    sc.mid
}

/// r42:arcus-polish2 — the sleeve's end lays a soft shadow on the wrist under it (the
/// coordinator: "the sleeve's end meets the wrist softly … a contact shadow on the skin"):
/// each skin corner near the sleeve's edge (posed: the cuff's ring of cloth corners at its
/// edge) takes a little less of the light round it, the nearer the more.
fn cuff_shadow(fig: &Figure, am: &ArmMesh, sc: &mut Scratch) {
    sc.ring.clear();
    let mut skin = std::mem::take(&mut sc.skin_v);
    skin.clear();
    skin.resize(am.verts.len(), false);
    for (t, st) in am.tris.iter().zip(&am.stuff) {
        for &i in t {
            let i = i as usize;
            if matches!(st, Stuff::Velvet | Stuff::Wool) {
                if am.sleeve[i] && fig.edge.get(am.verts[i] as usize).is_some_and(|&e| e < 0.1) {
                    sc.ring.push(sc.pts[i]);
                }
            } else if *st == Stuff::Skin {
                skin[i] = true;
            }
        }
    }
    if !sc.ring.is_empty() {
        let (depth, reach) = CUFF_SHADOW;
        for (i, &k) in skin.iter().enumerate() {
            if !k {
                continue;
            }
            let p = sc.pts[i];
            let d = sc
                .ring
                .iter()
                .map(|&q| (q - p).len())
                .fold(f32::MAX, f32::min);
            if d < reach * 2.0 {
                sc.ao[i] *= 1.0 - depth * (-(d / reach) * (d / reach)).exp();
            }
        }
    }
    sc.skin_v = skin;
}

/// How much light the sleeve's end takes from the skin under it, and over how far (cm).
const CUFF_SHADOW: (f32, f32) = (0.32, 0.9);

/// The left arm's aim from the hand's pose.
pub fn left_aim(lp: &super::hand::LeftPose) -> ArmAim {
    ArmAim {
        elbow: lp.elbow,
        wrist: lp.wrist,
        hand: Some(hu::HandAim {
            fingers: lp.fingers,
            thumb: lp.thumb,
            exact: lp.playing,
            rots: lp.rots,
        }),
    }
}

/// The bow arm's aim from its hand on the frog.
pub fn right_aim(arm: &crate::scene::bowed_hold::Arm, h: &super::arm::BowHand) -> ArmAim {
    let el = crate::scene::bowed_hold::elbow(arm.shoulder, h.wrist, arm.upper, arm.fore, arm.pole);
    ArmAim {
        elbow: el,
        wrist: h.wrist,
        hand: Some(hu::HandAim {
            fingers: h.fingers,
            thumb: h.thumb,
            exact: None,
            rots: None,
        }),
    }
}
