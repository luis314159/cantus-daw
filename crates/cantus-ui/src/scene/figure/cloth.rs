//! r42:arcus-polish — where a realistic figure's clothes meet its skin (ADR 0159, the owner's
//! polish list of 2026-11-25: "the dress's V edge sits on the skin with a contact shadow and
//! the velvet's edge sheen"; his collar's shadow on his neck; the cuffs on the wrists). Worked
//! out once when a figure is read, from the mesh at rest:
//!
//! - **each cloth vertex's distance to its cloth's edge** (cm along the mesh: the neckline,
//!   a cuff, a lapel's edge — wherever the cloth's triangles meet another stuff's or end),
//!   which the picture turns into the roll of an edge catching the light;
//! - **the skin's contact shadow**: a skin vertex near a cloth (within a centimetre or so)
//!   darkened by how near — the soft shade a neckline, a collar or a cuff lays on the skin.
//!
//! And how a cloth vertex's folds are carried (`build_figures.py::cloth_folds`): its texture
//! coordinates `[10 + depth, along]` (a run of soft folds across `along`, cm) or
//! `[20 + depth, across]` (a seam `across` cm away), read by [`fold`].

use super::super::bowed_human::{Figure, Stuff};
use super::super::geo::V3;

/// Whether `s` is a cloth (its edges roll; it shades the skin it lies on).
pub fn is_cloth(s: Stuff) -> bool {
    matches!(s, Stuff::Velvet | Stuff::Wool | Stuff::Shirt | Stuff::Tie)
}

/// How far (cm) an edge is looked for; past it a vertex is "far".
pub const EDGE_FAR: f32 = 4.0;

/// Each vertex's distance (cm, along its cloth's mesh) to that cloth's edge; [`EDGE_FAR`]
/// for a vertex of no cloth or further.
pub fn edges(fig: &Figure) -> Vec<f32> {
    let n = fig.pos.len();
    let w = |i: u16| fig.weld[i as usize] as usize;
    // Per welded vertex: the stuffs its triangles are (a bit each), and its cloth.
    let mut mask = vec![0u32; n];
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        for &i in t {
            mask[w(i)] |= 1 << (*s as u32);
        }
    }
    // Edges of each cloth's triangles (welded): those used once are its open boundary.
    let mut count: std::collections::HashMap<(usize, usize, u32), u8> = Default::default();
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        if !is_cloth(*s) {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (w(t[k]), w(t[(k + 1) % 3]));
            if a != b {
                *count.entry((a.min(b), a.max(b), *s as u32)).or_insert(0) += 1;
            }
        }
    }
    let mut dist = vec![EDGE_FAR; n];
    let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    let mut heap = std::collections::BinaryHeap::new();
    for (&(a, b, _), &c) in &count {
        let l = (fig.pos[a] - fig.pos[b]).len();
        adj[a].push((b, l));
        adj[b].push((a, l));
        if c == 1 {
            for x in [a, b] {
                dist[x] = 0.0;
            }
        }
    }
    for (i, &m) in mask.iter().enumerate() {
        // A cloth vertex also on another stuff's triangle (the dress's skin, a cuff on its
        // sleeve) is on an edge.
        let cloth = (0..32).any(|k| m & (1 << k) != 0 && is_cloth(stuff_of(k)));
        if cloth && m.count_ones() > 1 {
            dist[i] = 0.0;
        }
    }
    for (i, d) in dist.iter().enumerate() {
        if *d == 0.0 {
            heap.push(Near(0.0, i));
        }
    }
    while let Some(Near(d, i)) = heap.pop() {
        if d > dist[i] {
            continue;
        }
        for &(j, l) in &adj[i] {
            let nd = d + l;
            if nd < dist[j] {
                dist[j] = nd;
                heap.push(Near(nd, j));
            }
        }
    }
    (0..n).map(|i| dist[fig.weld[i] as usize]).collect()
}

/// The stuff numbered `k` (the enum's order).
fn stuff_of(k: u32) -> Stuff {
    const ALL: [Stuff; 18] = [
        Stuff::Skin,
        Stuff::Lips,
        Stuff::Hair,
        Stuff::Velvet,
        Stuff::Wool,
        Stuff::Shirt,
        Stuff::Tie,
        Stuff::Shoes,
        Stuff::Eye,
        Stuff::Iris,
        Stuff::Lash,
        Stuff::Nail,
        Stuff::Jewel,
        Stuff::CardHair,
        Stuff::CardBrow,
        Stuff::CardLash,
        Stuff::Shell,
        Stuff::Scalp,
    ];
    ALL.get(k as usize).copied().unwrap_or(Stuff::Shoes)
}

/// A distance and a vertex, nearest first.
#[derive(PartialEq)]
struct Near(f32, usize);
impl Eq for Near {}
impl PartialOrd for Near {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Near {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// How far under a cloth (cm) the skin is drawn in, at most, and from how near.
pub const SINK: (f32, f32) = (0.35, 0.6);

/// The skin under a cloth's edge drawn in a little (a skin vertex no cloth triangle uses,
/// within [`SINK`]'s reach of a cloth vertex, moved in along its normal at rest): where the
/// cloth lies on the skin the two were within a millimetre, and the skin showed through in
/// teeth (his collar's sawtooth on his neck, the review).
pub fn sink(fig: &mut Figure) {
    let (depth, reach) = SINK;
    let n = fig.pos.len();
    let mut skin = vec![false; n];
    let mut cloth = vec![false; n];
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        for &i in t {
            if matches!(s, Stuff::Skin | Stuff::Lips) {
                skin[i as usize] = true;
            } else if is_cloth(*s) {
                cloth[i as usize] = true;
            }
        }
    }
    let mut nrm = vec![V3::default(); n];
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        if matches!(s, Stuff::Skin | Stuff::Lips) {
            let [a, b, c] = t.map(|i| i as usize);
            let f = (fig.pos[b] - fig.pos[a]).cross(fig.pos[c] - fig.pos[a]);
            for i in [a, b, c] {
                let w = fig.weld[i] as usize;
                nrm[w] = nrm[w] + f;
            }
        }
    }
    let cell = |p: V3| [p.x, p.y, p.z].map(|c| (c / reach).floor() as i32);
    let mut grid: std::collections::HashMap<[i32; 3], Vec<u32>> = Default::default();
    for (i, &c) in cloth.iter().enumerate() {
        if c {
            grid.entry(cell(fig.pos[i])).or_default().push(i as u32);
        }
    }
    let mut moved = fig.pos.clone();
    for i in 0..n {
        if !skin[i] || cloth[i] {
            continue;
        }
        let p = fig.pos[i];
        let c = cell(p);
        let mut best = f32::MAX;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                        for &j in v {
                            best = best.min((fig.pos[j as usize] - p).len());
                        }
                    }
                }
            }
        }
        let nn = nrm[fig.weld[i] as usize];
        if best < reach && nn.len() > 1e-9 {
            moved[i] = p - nn.norm() * (depth * (1.0 - best / reach));
        }
    }
    fig.pos = moved;
}

/// How deep the contact shadow is where cloth touches the skin, and over how far (cm).
pub const CONTACT: (f32, f32) = (0.34, 0.75);

/// The skin's contact shadow: each skin vertex's luminance detail darkened by how near it
/// lies to a cloth vertex (at rest, cm), `1 - depth·e^{-(d/σ)²}`. Every vertex a skin triangle
/// uses; cloth vertices are left as they are.
pub fn contact(fig: &mut Figure) {
    let (depth, sigma) = CONTACT;
    let reach = sigma * 2.2;
    let mut skin = vec![false; fig.pos.len()];
    let mut cloth = vec![false; fig.pos.len()];
    for (t, s) in fig.tris.iter().zip(&fig.stuff) {
        for &i in t {
            if matches!(s, Stuff::Skin | Stuff::Lips) {
                skin[i as usize] = true;
            } else if is_cloth(*s) {
                cloth[i as usize] = true;
            }
        }
    }
    // The cloth's points in cells of `reach`.
    let cell = |p: V3| [p.x, p.y, p.z].map(|c| (c / reach).floor() as i32);
    let mut grid: std::collections::HashMap<[i32; 3], Vec<u32>> = Default::default();
    for (i, &c) in cloth.iter().enumerate() {
        if c {
            grid.entry(cell(fig.pos[i])).or_default().push(i as u32);
        }
    }
    for i in 0..fig.pos.len() {
        if !skin[i] {
            continue;
        }
        let p = fig.pos[i];
        let c = cell(p);
        let mut best = f32::MAX;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                        for &j in v {
                            if j as usize != i {
                                best = best.min((fig.pos[j as usize] - p).len());
                            }
                        }
                    }
                }
            }
        }
        if best < reach {
            let k = 1.0 - depth * (-(best / sigma) * (best / sigma)).exp();
            fig.detail[i][0] *= k;
        }
    }
}

/// A cloth's fold at a point, from its carried coordinates `[code, coord]`
/// (`build_figures.py::cloth_folds`): −1 the bottom of a crease … +1 a fold's crest; 0 none.
pub fn fold(code: f32, coord: f32) -> f32 {
    use std::f32::consts::TAU;
    if (10.0..11.5).contains(&code) {
        // A run of soft folds, unevenly spaced (about 3 cm), their creases narrower than
        // their crests.
        let a = (code - 10.0).min(1.0);
        let x = coord / 3.1;
        let s = (TAU * x + 0.9 * (TAU * x * 0.37 + 1.3).sin()).sin();
        a * if s < 0.0 {
            -(-s).powf(0.55)
        } else {
            s.powf(1.4)
        }
    } else if (20.0..21.5).contains(&code) {
        // A seam: a fine groove with the cloth raised a little either side.
        let a = (code - 20.0).min(1.0);
        let d = coord.abs();
        a * (-(-(d / 0.12) * (d / 0.12)).exp() + 0.45 * (-((d - 0.3) / 0.16).powi(2)).exp())
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_are_bounded_and_none_without_a_code() {
        for i in 0..400 {
            let x = i as f32 * 0.05 - 10.0;
            for code in [10.0, 10.5, 11.0, 20.0, 20.7, 21.0] {
                let f = fold(code, x);
                assert!((-1.01..=1.01).contains(&f), "{code} {x}: {f}");
            }
            assert_eq!(fold(0.4, x), 0.0);
            assert_eq!(fold(2.5, x), 0.0);
        }
        assert!(fold(21.0, 0.0) < -0.5, "a seam's groove");
    }

    #[test]
    fn the_dress_edge_is_found_and_shades_the_skin() {
        let fig = crate::scene::bowed_human::figure(crate::scene::bowed_human::Who::Woman);
        let e = edges(fig);
        let on_edge = fig
            .tris
            .iter()
            .zip(&fig.stuff)
            .filter(|(_, s)| **s == Stuff::Velvet)
            .flat_map(|(t, _)| t.iter())
            .filter(|&&i| e[i as usize] < 0.01)
            .count();
        assert!(
            on_edge > 50,
            "the neckline and the cuffs: {on_edge} edge corners"
        );
    }
}
