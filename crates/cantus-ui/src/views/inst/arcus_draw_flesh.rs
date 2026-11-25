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
    ao: Vec<f32>,
    caps: Vec<Capsule>,
}

/// A limb's piece as an occluder: from `a` to `b`, `r` thick, the bone it is (none: an
/// object the hand holds) — what shades the corners near it.
#[derive(Copy, Clone, Debug)]
pub struct Capsule {
    pub a: V3,
    pub b: V3,
    pub r: f32,
    pub bone: usize,
}

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

/// A corner's colour: its stuff lit (`ao`: how much of the light round it reaches, the
/// fingers' and the stick's shade on each other).
fn colour(sh: &Shading, st: Stuff, tone: u8, detail: [f32; 2], p: V3, n: V3, ao: f32) -> Color32 {
    let tone = tone.min(4);
    let e = (sh.eye - p).norm();
    let ndv = n.dot(e).clamp(0.0, 1.0);
    // The nail: the skin's own colour paler and a little pink, glossier.
    let skin_detail = match st {
        Stuff::Nail => [detail[0].clamp(0.9, 1.1) * 1.2, detail[1] * 0.3 - 0.25],
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
            let k = (1.0 - ndv).powi(2) * 0.55;
            c = [c[0] + s[0] * k, c[1] + s[1] * k, c[2] + s[2] * k];
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
    let wrap = if skin { 0.45 } else { 0.1 };
    let dif = ((ndl + wrap) / (1.0 + wrap)).max(0.0);
    let fill = (n.dot((e + v3(0.0, 0.25, 0.0)).norm())).max(0.0) * fill_k;
    let amb = amb_k * (0.8 + 0.2 * n.y);
    let albedo: [f32; 3] = match st {
        Stuff::Skin | Stuff::Lips | Stuff::Nail => {
            let a = lin(rgbf(hp::REAL_SKINS[tone as usize])).map(|c| c * 0.48);
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
        let t = (dif - ndl.max(0.0)).max(0.0) * key_k * ao;
        rad[0] += albedo[0] * t * 0.55;
        rad[1] += albedo[1] * t * 0.12;
        // A soft sheen (a nail's sharper).
        let h = (l + e).norm();
        let (pw, k) = if st == Stuff::Nail {
            (60, 0.22)
        } else {
            (18, 0.035)
        };
        let sp = n.dot(h).max(0.0).powi(pw) * k * key_k * ao * (ndl > 0.0) as u8 as f32;
        rad = rad.map(|x| x + sp);
    }
    if st == Stuff::Velvet {
        // Velvet's pile catches the light where it turns away.
        let pile = (1.0 - ndv).powi(2) * 0.28;
        let s = lin(hp::VELVET_SHEEN);
        for (i, r) in rad.iter_mut().enumerate() {
            *r += s[i] * pile * (key_k * dif.max(0.35) + amb);
        }
    }
    let t = crate::views::inst::chorda::scene::hybrid::raster::tone(rad.map(|x| x * ex), 0.85);
    let g = |x: f32| egui::ecolor::gamma_u8_from_linear_f32(x);
    Color32::from_rgb(g(t[0]), g(t[1]), g(t[2]))
}

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
) {
    let fig = hu::figure(who);
    let am = arm_mesh(who, s);
    let posed = hu::repose_arm(fig, base, s, aim);
    let shading = Shading::of(lit);
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
        occlusion(fig, &posed, s, am, held, sc);
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
                    cc[j] = colour(&shading, st, tone, d, sc.pts[i], sc.nrm[i], sc.ao[i]);
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
            let ab = c.b - c.a;
            let t = ((p - c.a).dot(ab) / ab.dot(ab).max(1e-6)).clamp(0.0, 1.0);
            let q = c.a + ab * t;
            let d = q - p;
            let dist = d.len();
            let gap = (dist - c.r).max(0.0);
            if gap > 2.2 {
                continue;
            }
            let facing = (n.dot(d * (1.0 / dist.max(1e-4))) * 0.7 + 0.3).max(0.0);
            let f = 1.0 - gap / 2.2;
            occ += f * f * facing * (c.r / (c.r + gap)).min(1.0);
        }
        sc.ao.push(1.0 - (occ * 0.55 * hand.min(1.0)).min(0.62));
    }
}

/// The left arm's aim from the hand's pose.
pub fn left_aim(lp: &super::hand::LeftPose) -> ArmAim {
    ArmAim {
        elbow: lp.elbow,
        wrist: lp.wrist,
        hand: Some(hu::HandAim {
            fingers: lp.fingers,
            thumb: lp.thumb,
            exact: lp.playing,
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
        }),
    }
}
