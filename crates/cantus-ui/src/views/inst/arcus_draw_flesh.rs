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
use crate::kit::appearance::rgb;
use crate::kit::skin::scene::hold as hp;
use crate::kit::Brush;
use crate::scene::bowed_human::{self as hu, ArmAim, Figure, Posed, Stuff, Who};
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
}

fn arm_mesh(who: Who, s: usize) -> &'static ArmMesh {
    static M: [OnceLock<ArmMesh>; 4] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    let k = (who == Who::Man) as usize * 2 + s.min(1);
    M[k].get_or_init(|| build_arm(hu::figure(who), s))
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
    let mut e: std::collections::HashMap<(u16, u16), (u32, u32)> = Default::default();
    for (ti, t) in tris.iter().enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
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
    for t in &fig.tris {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = (a.min(b) as u32, a.max(b) as u32);
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
                let (ga, gb) = (verts[a as usize], verts[b as usize]);
                if whole.get(&(ga.min(gb), ga.max(gb))).copied().unwrap_or(0) > 1 {
                    alpha[a as usize] = 0.0;
                    alpha[b as usize] = 0.0;
                    return None;
                }
            }
            Some((a, b, t0, t1))
        })
        .collect();
    ArmMesh {
        verts,
        tris,
        stuff,
        upper,
        alpha,
        edges,
    }
}

#[derive(Default)]
struct Scratch {
    pts: Vec<V3>,
    nrm: Vec<V3>,
    sp: Vec<Pos2>,
    front: Vec<bool>,
    col: Vec<[Color32; 3]>,
    order: Vec<(f32, u32)>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// A corner's colour: its stuff lit, the skin with its detail and the warmth of light
/// through it, the velvet with its pile's sheen.
fn colour(lit: &Lit, st: Stuff, tone: u8, detail: [f32; 2], p: V3, n: V3) -> Color32 {
    let skin = hp::REAL_SKINS[(tone as usize).min(4)];
    let base: [u8; 3] = match st {
        Stuff::Skin | Stuff::Lips => {
            let (lum, red) = (detail[0].min(1.1), detail[1]);
            let f = |c: u8, k: f32| ((c as f32) * lum * k).clamp(0.0, 255.0) as u8;
            [
                f(skin[0], 1.0 + 0.22 * red),
                f(skin[1], 1.0 - 0.12 * red),
                f(skin[2], 1.0 - 0.18 * red),
            ]
        }
        Stuff::Velvet => c8(hp::VELVET),
        Stuff::Wool => c8(hp::WOOL),
        Stuff::Shirt => c8(hp::SHIRT),
        Stuff::Tie => c8(hp::TIE),
        Stuff::Shoes => c8(hp::SHOES),
        Stuff::Hair => c8(hp::HAIR),
    };
    let c = lit.shade(base, p, n);
    let e = (lit.eye - p).norm();
    let turn = 1.0 - n.dot(e).clamp(0.0, 1.0);
    let add = match st {
        Stuff::Velvet => {
            let s = c8(hp::VELVET_SHEEN);
            let k = turn * turn * 0.55;
            [s[0] as f32 * k, s[1] as f32 * k, s[2] as f32 * k]
        }
        Stuff::Skin | Stuff::Lips => {
            let k = turn * turn * 0.10;
            [
                base[0] as f32 * k,
                base[1] as f32 * 0.55 * k,
                base[2] as f32 * 0.4 * k,
            ]
        }
        _ => [0.0; 3],
    };
    let ch = |x: u8, a: f32| (x as f32 + a).min(255.0) as u8;
    rgb([ch(c.r(), add[0]), ch(c.g(), add[1]), ch(c.b(), add[2])])
}

fn c8(c: crate::kit::skin::scene::C3) -> [u8; 3] {
    let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(c[0]), f(c[1]), f(c[2])]
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
) {
    let fig = hu::figure(who);
    let am = arm_mesh(who, s);
    let posed = hu::repose_arm(fig, base, s, aim);
    SCRATCH.with(|sc| {
        let mut sc = sc.borrow_mut();
        let sc = &mut *sc;
        let n = am.verts.len();
        sc.pts.clear();
        sc.nrm.clear();
        sc.sp.clear();
        for &i in &am.verts {
            let p = posed.vertex(fig, i as usize);
            sc.pts.push(p);
            sc.sp.push(v.p(p));
        }
        sc.nrm.resize(n, V3::default());
        for t in &am.tris {
            let [a, bb, c] = t.map(|i| i as usize);
            let f = (sc.pts[bb] - sc.pts[a]).cross(sc.pts[c] - sc.pts[a]);
            for i in [a, bb, c] {
                sc.nrm[i] = sc.nrm[i] + f;
            }
        }
        for q in &mut sc.nrm {
            *q = q.norm();
        }
        let eye = v.cam.eye();
        sc.front.clear();
        sc.col.clear();
        sc.order.clear();
        for (k, t) in am.tris.iter().enumerate() {
            let [a, bb, c] = t.map(|i| i as usize);
            let ctr = (sc.pts[a] + sc.pts[bb] + sc.pts[c]) * (1.0 / 3.0);
            // The figure's triangles are wound outward (MakeHuman's, kept by the converter).
            let fnrm = (sc.pts[bb] - sc.pts[a]).cross(sc.pts[c] - sc.pts[a]);
            let mut show = fnrm.dot(eye - ctr) > 0.0;
            if show {
                if let Some((want, hid)) = hidden {
                    show = (am.upper[k] || hid(ctr)) == want;
                }
            }
            sc.front.push(show);
            let st = am.stuff[k];
            let mut cc = [Color32::TRANSPARENT; 3];
            if show {
                for (j, &i) in [a, bb, c].iter().enumerate() {
                    let d = fig.detail[am.verts[i] as usize];
                    cc[j] = colour(lit, st, tone, d, sc.pts[i], sc.nrm[i]);
                    if am.alpha[i] < 1.0 {
                        cc[j] = cc[j].gamma_multiply(am.alpha[i]);
                    }
                }
                // A shirt's cuff lies inside its sleeve: sorted a little behind it.
                let bias = if st == Stuff::Shirt { 1.5 } else { 0.0 };
                sc.order.push((v.cam.depth(ctr) + bias, k as u32));
            }
            sc.col.push(cc);
        }
        // The outline: an edge between a shown triangle and one not shown (or none).
        let tri_n = am.tris.len() as u32;
        for (e, &(a, bb, t0, t1)) in am.edges.iter().enumerate() {
            let f0 = t0 != u32::MAX && sc.front[t0 as usize];
            let f1 = t1 != u32::MAX && sc.front[t1 as usize];
            if f0 != f1 {
                let mid = (sc.pts[a as usize] + sc.pts[bb as usize]) * 0.5;
                sc.order.push((v.cam.depth(mid) - 0.05, tri_n + e as u32));
            }
        }
        sc.order
            .sort_unstable_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
        for &(_, k) in &sc.order {
            if k < tri_n {
                let t = am.tris[k as usize];
                let p = t.map(|i| sc.sp[i as usize]);
                b.shaded_tri(p, sc.col[k as usize]);
            } else {
                let (a, bb, t0, t1) = am.edges[(k - tri_n) as usize];
                let t = if t0 != u32::MAX && sc.front[t0 as usize] {
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
                if out.length() > 1e-4 {
                    b.edge(pa, pb, corner(a), corner(bb), out);
                }
            }
        }
    });
}

/// The left arm's aim from the hand's pose.
pub fn left_aim(lp: &super::hand::LeftPose) -> ArmAim {
    ArmAim {
        elbow: lp.elbow,
        wrist: lp.wrist,
        hand: Some(hu::HandAim {
            fingers: lp.fingers,
            thumb: lp.thumb,
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
        }),
    }
}
