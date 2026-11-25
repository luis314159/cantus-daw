//! r42:arcus-upper — the realistic musicians that may hold an Arcus instrument (the owner,
//! 2026-11-24): a woman in a black velvet long-sleeved concert dress and a man in a dark
//! suit, made from MakeHuman's CC0 assets offline (`tools/arcus-figures/build_figures.py`)
//! into `assets/figures/{woman,man}.fig`, posed here from the same skeleton the stylised
//! figure is posed from (`bowed_figure::skeleton`), skinned (four bones a vertex) and
//! handed to both pictures as smooth triangles (`Mesh::tris`).
//!
//! **Not uncanny** (the owner asked twice): realistic adult proportions, the eyelids lowered
//! over the eyes (a player's eyes on the instrument), small calm features, soft skin. **The
//! faces are original**: MakeHuman's generic adult averages, nothing fitted to a photograph
//! (the owner's reference shows a real person; only her posture, dress and light are taken).
//!
//! The arms are the live picture's, as the stylised figure's are: each triangle carries its
//! part (the body, baked; the left or the right arm, drawn live by
//! `views/inst/arcus_draw_flesh.rs` from the same vertices), a band round each shoulder in
//! both, so no seam opens when an arm moves.

use super::geo::{v, V3};
use std::sync::OnceLock;

/// The scene's bones, as `build_figures.py` lists them: the trunk, then each side's 24
/// (left first).
pub const HIPS: usize = 0;
pub const SPINE: usize = 1;
pub const CHEST: usize = 2;
pub const NECK: usize = 3;
pub const HEAD: usize = 4;
/// A side's bones (`side` 0 the left, 1 the right): its first bone's index.
pub const fn side(s: usize) -> usize {
    5 + 24 * s
}
pub const CLAV: usize = 0;
pub const UPPER1: usize = 1;
pub const UPPER2: usize = 2;
pub const FORE1: usize = 3;
pub const FORE2: usize = 4;
pub const HAND: usize = 5;
/// Finger `f` (0 the thumb … 4 the little finger), phalanx `k` (0…2).
pub const fn finger(f: usize, k: usize) -> usize {
    6 + f * 3 + k
}
pub const THIGH: usize = 21;
pub const SHIN: usize = 22;
pub const FOOT: usize = 23;
pub const BONES: usize = 53;

/// What a triangle is made of (`build_figures.py`'s numbers).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stuff {
    Skin,
    Lips,
    Hair,
    Velvet,
    Wool,
    Shirt,
    Tie,
    Shoes,
    /// The eyes' whites and irises under the lowered lids, the lashes, the nails (sheet v7).
    Eye,
    Iris,
    Lash,
    Nail,
    /// r42:arcus-upper — her small drop earrings (gold).
    Jewel,
    /// r42:arcus-upper — MakeHuman's CC0 hair, brow and lash proxies: alpha-tested cards
    /// in the Photographic bake (their textures `assets/figures/<who>_<kind>.png`), left out
    /// of the Drawn picture; their vertices carry the cards' texture coordinates.
    CardHair,
    CardBrow,
    CardLash,
    /// r42:arcus-upper — the hair as a thick shell (the Drawn picture's) and a thin scalp
    /// under the cards (the Photographic picture's).
    Shell,
    Scalp,
}

impl Stuff {
    fn of(n: u8) -> Stuff {
        match n {
            0 => Stuff::Skin,
            1 => Stuff::Lips,
            2 => Stuff::Hair,
            3 => Stuff::Velvet,
            4 => Stuff::Wool,
            5 => Stuff::Shirt,
            6 => Stuff::Tie,
            8 => Stuff::Eye,
            9 => Stuff::Iris,
            10 => Stuff::Lash,
            11 => Stuff::Nail,
            12 => Stuff::Jewel,
            13 => Stuff::CardHair,
            14 => Stuff::CardBrow,
            15 => Stuff::CardLash,
            16 => Stuff::Shell,
            17 => Stuff::Scalp,
            _ => Stuff::Shoes,
        }
    }
}

/// A bone at rest: its parent, head, tail and its frame's x (across it).
#[derive(Copy, Clone, Debug, Default)]
pub struct Bone {
    pub parent: Option<usize>,
    pub head: V3,
    pub tail: V3,
    pub x: V3,
}

/// A figure at rest (cm, y up, facing +z, its left on +x).
#[derive(Debug, Default)]
pub struct Figure {
    pub bones: Vec<Bone>,
    pub marks: Vec<(String, V3)>,
    pub pos: Vec<V3>,
    /// Four bones and their weights (0…1) a vertex.
    pub bw: Vec<[(u8, f32); 4]>,
    /// The skin's albedo detail (a luminance factor, a red shift) or the hair's strand
    /// coordinates (cm along, across).
    pub detail: Vec<[f32; 2]>,
    pub tris: Vec<[u16; 3]>,
    pub stuff: Vec<Stuff>,
    /// A bit each: 1 the body, 2 the left arm, 4 the right.
    pub part: Vec<u8>,
}

/// Which figure.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    Woman,
    Man,
}

static WOMAN: &[u8] = include_bytes!("../../assets/figures/woman.fig");
static MAN: &[u8] = include_bytes!("../../assets/figures/man.fig");

/// The figure, read once.
pub fn figure(w: Who) -> &'static Figure {
    static W: OnceLock<Figure> = OnceLock::new();
    static M: OnceLock<Figure> = OnceLock::new();
    match w {
        Who::Woman => W.get_or_init(|| parse(WOMAN).unwrap_or_default()),
        Who::Man => M.get_or_init(|| parse(MAN).unwrap_or_default()),
    }
}

struct Rd<'a> {
    b: &'a [u8],
    o: usize,
}

impl Rd<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.b.get(self.o..self.o + N)?;
        self.o += N;
        s.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take()?))
    }
    fn f16(&mut self) -> Option<f32> {
        Some(half(u16::from_le_bytes(self.take()?)))
    }
    fn v3(&mut self) -> Option<V3> {
        Some(v(self.f32()?, self.f32()?, self.f32()?))
    }
}

/// An IEEE half float.
fn half(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    match e {
        0 => s * m * (2.0f32).powi(-24),
        31 => s * f32::MAX,
        _ => s * (1.0 + m / 1024.0) * (2.0f32).powi(e - 15),
    }
}

/// A `.fig` file (`build_figures.py::write_fig`).
pub fn parse(b: &[u8]) -> Option<Figure> {
    let mut r = Rd { b, o: 0 };
    if r.take::<4>()? != *b"CFIG" || r.u16()? != 1 {
        return None;
    }
    let mut f = Figure::default();
    let nb = r.u16()? as usize;
    for _ in 0..nb {
        let p = r.i16()?;
        f.bones.push(Bone {
            parent: (p >= 0).then_some(p as usize),
            head: r.v3()?,
            tail: r.v3()?,
            x: r.v3()?,
        });
    }
    let nm = r.u16()? as usize;
    for _ in 0..nm {
        let l = r.u8()? as usize;
        let name = std::str::from_utf8(r.b.get(r.o..r.o + l)?)
            .ok()?
            .to_string();
        r.o += l;
        f.marks.push((name, r.v3()?));
    }
    let nv = r.u32()? as usize;
    let mut strand = Vec::with_capacity(nv);
    for _ in 0..nv {
        let p = v(
            r.i16()? as f32 * 0.01,
            r.i16()? as f32 * 0.01,
            r.i16()? as f32 * 0.01,
        );
        let ids = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
        let ws = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
        let lum = 0.35 + r.u8()? as f32 / 255.0;
        let red = r.u8()? as i8 as f32 / 100.0;
        let uv = [r.f16()?, r.f16()?];
        f.pos.push(p);
        f.bw.push(std::array::from_fn(|k| (ids[k], ws[k] as f32 / 255.0)));
        f.detail.push([lum, red]);
        strand.push(uv);
    }
    let nt = r.u32()? as usize;
    for _ in 0..nt {
        let t = [r.u16()?, r.u16()?, r.u16()?];
        if t.iter().any(|&i| i as usize >= nv) {
            return None;
        }
        f.tris.push(t);
        f.stuff.push(Stuff::of(r.u8()?));
        f.part.push(r.u8()?);
    }
    // A hair vertex carries its strand coordinates instead of the skin's detail.
    for (t, s) in f.tris.iter().zip(&f.stuff) {
        if matches!(
            *s,
            Stuff::Hair
                | Stuff::CardHair
                | Stuff::CardBrow
                | Stuff::CardLash
                | Stuff::Shell
                | Stuff::Scalp
        ) {
            for &i in t {
                f.detail[i as usize] = strand[i as usize];
            }
        }
    }
    Some(f)
}

impl Figure {
    /// A named point at rest.
    pub fn mark(&self, name: &str) -> V3 {
        self.marks
            .iter()
            .find(|(n, _)| n == name)
            .map(|m| m.1)
            .unwrap_or_default()
    }
}

/// A figure's rig: moved to `figure::rig` (ADR 0159), named here as before.
pub use super::figure::rig::{Aim, Rot, Xf};

/// Every bone posed.
#[derive(Clone, Debug)]
pub struct Posed {
    pub xf: [Xf; BONES],
}

impl Bone {
    pub fn len(&self) -> f32 {
        (self.tail - self.head).len()
    }
    fn frame(&self) -> Rot {
        Rot::frame(self.x, self.tail - self.head)
    }
}

/// The figure posed by `aims` (a bone without one follows its parent rigidly; the root
/// without one turns by `root`).
pub fn pose(fig: &Figure, aims: &[Option<Aim>; BONES], root: &Rot, off: V3) -> Posed {
    let mut xf = [Xf::default(); BONES];
    for b in 0..fig.bones.len().min(BONES) {
        xf[b] = step(fig, b, &xf, aims[b], root, off);
    }
    Posed { xf }
}

/// Bone `b` posed by `aim` (or carried by its parent, already posed in `xf`).
fn step(fig: &Figure, b: usize, xf: &[Xf; BONES], aim: Option<Aim>, root: &Rot, off: V3) -> Xf {
    let bone = &fig.bones[b];
    let parent = bone.parent.map(|p| xf[p]);
    let carried = parent.map_or(root.apply(bone.head) + off, |p| p.apply(bone.head));
    let prot = parent.map_or(*root, |p| p.rot);
    let y0 = (bone.tail - bone.head).norm();
    match aim {
        None => Xf {
            head: bone.head,
            y0,
            k: 1.0,
            rot: prot,
            at: carried,
        },
        Some(a) => {
            let x = a.x.unwrap_or_else(|| prot.apply(bone.x));
            let fp = Rot::frame(x, a.y);
            Xf {
                head: bone.head,
                y0,
                k: a.len.map_or(1.0, |l| l / bone.len().max(1e-3)),
                rot: fp.mul(&bone.frame().t()),
                at: a.head.unwrap_or(carried),
            }
        }
    }
}

/// `base` with one arm (`s` 0 left, 1 right) posed through `a` instead (the live arms:
/// the body stays as baked). Nothing allocated.
pub fn repose_arm(fig: &Figure, base: &Posed, s: usize, a: &ArmAim) -> Posed {
    let mut aims: [Option<Aim>; BONES] = [None; BONES];
    let sh = shoulder(fig, base, s);
    arm_aims(fig, &mut aims, s, a, sh);
    let mut out = base.clone();
    let o = side(s);
    #[allow(clippy::needless_range_loop)] // each bone posed after its parent, in order
    for b in o + UPPER1..=o + finger(4, 2) {
        out.xf[b] = step(fig, b, &out.xf, aims[b], &Rot::I, V3::default());
    }
    out
}

impl Posed {
    /// A rest point carried by bone `b`.
    pub fn at(&self, b: usize, p: V3) -> V3 {
        self.xf[b].apply(p)
    }

    /// Everything moved by `d`.
    pub fn shift(&mut self, d: V3) {
        for x in &mut self.xf {
            x.at = x.at + d;
        }
    }

    /// Vertex `i` of `fig` posed.
    pub fn vertex(&self, fig: &Figure, i: usize) -> V3 {
        let p = fig.pos[i];
        let mut o = V3::default();
        for &(b, w) in &fig.bw[i] {
            if w > 0.0 {
                o = o + self.xf[(b as usize).min(BONES - 1)].apply(p) * w;
            }
        }
        o
    }
}

const UP: V3 = V3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};

/// How far back of the stylised figure's place a realistic cellist or bassist sits (cm).
const SIT_BACK: f32 = 4.0;

/// How far a realistic cellist's head comes forward of the stylised figure's, leaning into
/// the instrument (cm), and how far the chin lifts (radians).
const LEAN_IN: f32 = 2.5;
const CHIN_UP: f32 = 0.1;

/// How far a violinist's or a violist's trunk turns to the left (radians).
const TRUNK_TURN: f32 = 0.2;

/// The figure's own turn: its left (+x at rest) to the skeleton's left, its front to the
/// skeleton's front.
pub fn facing(s: &super::bowed_figure::Skel) -> Rot {
    Rot {
        x: -s.r,
        y: UP,
        z: s.f,
    }
}

/// The joints the arms are posed through (world cm): the shoulder is the body's.
#[derive(Copy, Clone, Debug)]
pub struct ArmAim {
    pub elbow: V3,
    pub wrist: V3,
    /// The knuckles (index … little finger) and the hand's palm side (a unit vector out of
    /// the palm), or none (the hand carried by the forearm).
    pub hand: Option<HandAim>,
}

#[derive(Copy, Clone, Debug)]
pub struct HandAim {
    /// Each finger's knuckle, middle joint, last joint, tip (index … little finger).
    pub fingers: [[V3; 4]; 4],
    /// The thumb's base, knuckle, joint, tip.
    pub thumb: [V3; 4],
    /// The finger (index … little) whose tip must land exactly (the left hand's playing
    /// finger on its stop); the others are relaxed: their tips reach the pose's way at
    /// 70–86 % of their own length, so a smaller or a larger hand curls as a hand does
    /// instead of crumpling (sheet v7).
    pub exact: Option<usize>,
    /// r42:arcus-upper — the hand's and the finger bones' turns given outright (the neck
    /// hand's authored table, `figure::neck_hand`): the bones are set as they say, from the
    /// joints above, with no reach worked out.
    pub rots: Option<super::figure::neck_hand::HandRots>,
}

/// Set one arm's bones (`s` 0 left, 1 right) in `aims` through `a`, the shoulder where
/// the body carries it.
pub fn arm_aims(fig: &Figure, aims: &mut [Option<Aim>; BONES], s: usize, a: &ArmAim, sh: V3) {
    let o = side(s);
    let u = (a.elbow - sh).norm();
    let w = (a.wrist - a.elbow).norm();
    let mut zz = w - u * w.dot(u);
    if zz.len() < 1e-3 {
        zz = super::geo::basis(u).0;
    }
    let bend = u.cross(zz.norm()).norm();
    let up_len = (a.elbow - sh).len();
    let fo_len = (a.wrist - a.elbow).len();
    let rest_up = fig.bones[o + UPPER1].len() + fig.bones[o + UPPER2].len();
    let rest_fo = fig.bones[o + FORE1].len() + fig.bones[o + FORE2].len();
    let (ku, kf) = (up_len / rest_up.max(1e-3), fo_len / rest_fo.max(1e-3));
    aims[o + UPPER1] = Some(Aim {
        head: Some(sh),
        y: u,
        x: Some(bend),
        len: Some(fig.bones[o + UPPER1].len() * ku),
    });
    aims[o + UPPER2] = Some(Aim {
        head: Some(sh + u * (fig.bones[o + UPPER1].len() * ku)),
        y: u,
        x: Some(bend),
        len: Some(fig.bones[o + UPPER2].len() * ku),
    });
    // The hand's turn, and the forearm's twist toward it (shared out along the forearm).
    let hand_rot = a.hand.map(|h| {
        h.rots
            .map_or_else(|| hand_turn(fig, s, &h, a.wrist), |r| r.hand)
    });
    let twist = hand_rot.map_or(0.0, |r| {
        let q = r.apply(fig.bones[o + FORE2].x);
        let q = q - w * q.dot(w);
        if q.len() < 1e-4 {
            0.0
        } else {
            bend.cross(q).dot(w).atan2(bend.dot(q))
        }
    });
    let f1 = fig.bones[o + FORE1].len() * kf;
    for (k, b, head, share) in [(0, FORE1, a.elbow, 0.25), (1, FORE2, a.elbow + w * f1, 0.7)] {
        let _ = k;
        aims[o + b] = Some(Aim {
            head: Some(head),
            y: w,
            x: Some(Rot::about(w, twist * share).apply(bend)),
            len: Some(fig.bones[o + b].len() * kf),
        });
    }
    if let (Some(h), Some(r)) = (a.hand, hand_rot) {
        let hb = &fig.bones[o + HAND];
        aims[o + HAND] = Some(Aim {
            head: Some(a.wrist),
            y: r.apply((hb.tail - hb.head).norm()),
            x: Some(r.apply(hb.x)),
            len: None,
        });
        // Each finger from its own knuckle where the turned hand carries it (the figure's
        // fingers keep their width and spacing) to the pose's fingertip, bent through a
        // two-bone reach toward the pose's middle joint; stretched only when the tip is out
        // of reach (the playing finger lands on its stop exactly). Each phalanx turned from
        // the hand's turn by the shortest way to its own (sheet v7: no twist — a finger's
        // roll left to the parent's turn crumpled the skin at the knuckles).
        if let Some(rots) = h.rots {
            // Every bone as the table turns it, its head where the chain carries it.
            for f in 0..5 {
                let j = if f == 0 { h.thumb } else { h.fingers[f - 1] };
                for k in 0..3 {
                    let bone = &fig.bones[o + finger(f, k)];
                    let rb = rots.bones[f * 3 + k];
                    aims[o + finger(f, k)] = Some(Aim {
                        head: Some(j[k]),
                        y: rb.apply((bone.tail - bone.head).norm()),
                        x: Some(rb.apply(bone.x)),
                        len: None,
                    });
                }
            }
            return;
        }
        let wr = hb.head;
        for f in 0..5 {
            let j = if f == 0 { h.thumb } else { h.fingers[f - 1] };
            let b0 = &fig.bones[o + finger(f, 0)];
            let (l0, l1, l2) = (
                b0.len(),
                fig.bones[o + finger(f, 1)].len(),
                fig.bones[o + finger(f, 2)].len(),
            );
            let k0 = a.wrist + r.apply(b0.head - wr);
            let tip = if f > 0 && h.exact != Some(f - 1) {
                let d = j[3] - k0;
                let full = l0 + l1 + l2;
                k0 + d.norm() * d.len().clamp(0.7 * full, 0.86 * full)
            } else {
                j[3]
            };
            let reach = (tip - k0).len();
            let sc = (reach / ((l0 + l1 + l2) * 0.97)).max(1.0);
            let (l0, l1, l2) = (l0 * sc, l1 * sc, l2 * sc);
            let pole = j[1] - (k0 + tip) * 0.5;
            let pip = knee(k0, tip, l0, l1 + l2, pole);
            let along = (tip - pip).norm();
            let bend = (pole - along * pole.dot(along)).norm();
            let dip = pip + along * ((tip - pip).len() * l1 / (l1 + l2)) + bend * (0.12 * sc);
            for (k, (from, to)) in [(k0, pip), (pip, dip), (dip, tip)].into_iter().enumerate() {
                let d = to - from;
                let bone = &fig.bones[o + finger(f, k)];
                let rest = r.apply((bone.tail - bone.head).norm());
                aims[o + finger(f, k)] = Some(Aim {
                    head: Some(from),
                    y: d.norm(),
                    x: Some(Rot::between(rest, d.norm()).mul(&r).apply(bone.x)),
                    len: Some(d.len().max(0.2)),
                });
            }
        }
    }
}

/// The hand's turn from rest: its wrist-to-knuckles way and the knuckles' line (index to
/// little finger) brought to the pose's.
fn hand_turn(fig: &Figure, s: usize, h: &HandAim, wrist: V3) -> Rot {
    let o = side(s);
    let kn = |f: usize| fig.bones[o + finger(f, 0)].head;
    let rest_mid = (kn(2) + kn(3)) * 0.5;
    let wr = fig.bones[o + HAND].head;
    let ry = (rest_mid - wr).norm();
    let rx = kn(1) - kn(4);
    let mid = (h.fingers[1][0] + h.fingers[2][0]) * 0.5;
    let py = (mid - wrist).norm();
    let px = h.fingers[0][0] - h.fingers[3][0];
    Rot::frame(px, py).mul(&Rot::frame(rx, ry).t())
}

/// A two-bone limb's middle joint (`elbow` with the realistic lengths).
fn knee(hip: V3, ankle: V3, a: f32, b: f32, pole: V3) -> V3 {
    super::figure::rig::two_bone(hip, ankle, a, b, pole)
}

/// Arcus's rig for the stylised skeleton `s` (ADR 0159: Arcus is the PlayRig's first
/// client): under a chin the body hangs from the jaw on the chinrest (leant 6° forward,
/// turned toward the scroll; the chair's height follows); behind a cello or a bass the
/// figure sits on the instrument's seat a little behind the stylised figure's place, the
/// head where the stylised head is, the knees and the ankles where it puts them; each arm
/// through `arms` (the live arms' pose) or resting (the left hand at the skeleton's wrist,
/// the bow hand in front of the chest to the right).
pub fn arcus_rig(
    s: &super::bowed_figure::Skel,
    held: bool,
    arms: [Option<ArmAim>; 2],
) -> super::figure::play::PlayRig {
    use super::figure::play::{Hand, Head, HeadAt, Legs, PlayRig, Seat};
    let (at, seat, legs) = if held {
        (
            HeadAt::Jaw(s.chin),
            Seat::FromHead {
                lean: 6.0f32.to_radians(),
                turn: TRUNK_TURN,
            },
            Legs::Forward,
        )
    } else {
        // Sat a little further back than the stylised figure (sheet v7: a realistic hand
        // is longer; with the cello's neck at the shoulder the elbow folded under 30°).
        let back = s.f * SIT_BACK;
        // Upright, leaning into the cello, the chin a little lifted (the owner's reference's
        // posture, 2026-11-24).
        (
            HeadAt::Eyes(s.head - back + s.f * LEAN_IN),
            Seat::Chair {
                at: s.pelvis - back,
                top: s.pelvis.y - 10.0 - back.y,
            },
            Legs::To {
                knees: s.knees,
                ankles: s.ankles,
            },
        )
    };
    let hand = |sd: usize| match arms[sd] {
        Some(a) => Hand::Aim(a),
        None => {
            let (wrist, pole) = match (sd, held) {
                (0, true) => (s.lw, s.down + s.r * 0.45 + s.f * 0.15),
                (0, false) => (s.lw, -s.r + s.down * 1.3),
                (_, true) => (s.chest + s.r * 14.0 + s.f * 24.0, s.down + s.r * 1.5),
                (_, false) => (
                    s.chest + s.r * 24.0 + s.f * 22.0 + s.down * 10.0,
                    s.r + s.down * 1.6,
                ),
            };
            Hand::Rest { wrist, pole }
        }
    };
    let (up, face) = if held {
        (s.hu, s.hf)
    } else {
        let (sn, cs) = CHIN_UP.sin_cos();
        (s.hu * cs - s.hf * sn, s.hf * cs + s.hu * sn)
    };
    PlayRig {
        facing: s.f,
        right: s.r,
        head: Head { at, up, face },
        seat,
        legs,
        hands: [hand(0), hand(1)],
        occluders: Vec::new(),
    }
}

/// The figure posed for the skeleton `s` (the stylised figure's, posed from the
/// instrument) through Arcus's rig: every bone and the seat's height.
pub fn body(
    fig: &Figure,
    s: &super::bowed_figure::Skel,
    held: bool,
    arms: [Option<ArmAim>; 2],
) -> (Posed, f32) {
    super::figure::play::pose(fig, &arcus_rig(s, held, arms))
}

/// The shoulder (the upper arm's head) where the body carries it.
pub fn shoulder(fig: &Figure, p: &Posed, s: usize) -> V3 {
    p.at(side(s) + CLAV, fig.bones[side(s) + UPPER1].head)
}

/// The hand's length from the wrist to its knuckles' middle (side `s`), cm.
pub fn hand_reach(fig: &Figure, s: usize) -> f32 {
    let o = side(s);
    let b = &fig.bones;
    let mid = (b[o + finger(2, 0)].head + b[o + finger(3, 0)].head) * 0.5;
    (mid - b[o + HAND].head).len()
}

/// The rest lengths of an arm (upper, fore), cm.
pub fn arm_lengths(fig: &Figure, s: usize) -> (f32, f32) {
    let o = side(s);
    let b = &fig.bones;
    (
        b[o + UPPER1].len() + b[o + UPPER2].len(),
        b[o + FORE1].len() + b[o + FORE2].len(),
    )
}

/// Every vertex of `fig` posed, and smooth normals over all its triangles.
pub fn skin(fig: &Figure, p: &Posed) -> (Vec<V3>, Vec<V3>) {
    let pts: Vec<V3> = (0..fig.pos.len()).map(|i| p.vertex(fig, i)).collect();
    let mut nrm = vec![V3::default(); pts.len()];
    for t in &fig.tris {
        let [a, b, c] = t.map(|i| i as usize);
        let n = (pts[b] - pts[a]).cross(pts[c] - pts[a]);
        for i in [a, b, c] {
            nrm[i] = nrm[i] + n;
        }
    }
    for n in &mut nrm {
        *n = n.norm();
    }
    (pts, nrm)
}

/// Which realistic figure a holder names, if one.
pub fn who(h: super::bowed_hold::Hold) -> Option<Who> {
    match h {
        super::bowed_hold::Hold::Woman => Some(Who::Woman),
        super::bowed_hold::Hold::Man => Some(Who::Man),
        _ => None,
    }
}

/// The scene's material for a figure's stuff (`tone` the skin's).
pub fn role(st: Stuff, who: Who, tone: u8) -> super::bowed_mesh::Role {
    use super::bowed_mesh::Role;
    match st {
        Stuff::Skin | Stuff::Lips => Role::Skin(tone),
        Stuff::Hair if who == Who::Woman => Role::HairFair,
        Stuff::Hair => Role::Hair,
        Stuff::Velvet => Role::Velvet,
        Stuff::Wool => Role::Wool,
        Stuff::Shirt => Role::Shirt,
        Stuff::Tie => Role::Tie,
        Stuff::Shoes => Role::Shoes,
        Stuff::Eye => Role::Eye,
        Stuff::Iris => Role::Iris,
        Stuff::Lash => Role::Lash,
        Stuff::Nail => Role::Nail(tone),
        Stuff::Jewel => Role::Brass,
        Stuff::CardHair => Role::Card(who as u8 * 3),
        Stuff::CardBrow => Role::Card(who as u8 * 3 + 1),
        Stuff::CardLash => Role::Card(who as u8 * 3 + 2),
        Stuff::Shell => Role::Shell(who as u8),
        Stuff::Scalp => Role::Scalp(who as u8),
    }
}

/// The triangles of `fig` in any of the parts `mask` names, posed as `pts`/`nrm`, added
/// to `m` a role each.
pub fn emit(
    m: &mut super::bowed_mesh::Mesh,
    fig: &Figure,
    pts: &[V3],
    nrm: &[V3],
    who: Who,
    tone: u8,
    mask: u8,
) {
    use super::bowed_mesh::{Role, Tris};
    let mut out: Vec<(Role, Tris, Vec<u32>)> = Vec::new();
    for (k, t) in fig.tris.iter().enumerate() {
        if fig.part[k] & mask == 0 {
            continue;
        }
        let r = role(fig.stuff[k], who, tone);
        let e = match out.iter().position(|(x, _, _)| *x == r) {
            Some(i) => i,
            None => {
                out.push((r, Tris::default(), vec![u32::MAX; fig.pos.len()]));
                out.len() - 1
            }
        };
        let (_, tr, map) = &mut out[e];
        let idx = t.map(|i| {
            let i = i as usize;
            if map[i] == u32::MAX {
                map[i] = tr.pts.len() as u32;
                tr.pts.push(pts[i]);
                tr.nrm.push(nrm[i]);
                tr.detail.push(fig.detail[i]);
            }
            map[i]
        });
        tr.idx.push(idx);
    }
    m.tris.extend(out.into_iter().map(|(r, t, _)| (r, t)));
}

/// The realistic figure `who` posed for `b` in `pose`, its chair (a violinist's), its
/// body baked into `m` (its arms are the live picture's) and the woman's long skirt.
pub fn build(
    m: &mut super::bowed_mesh::Mesh,
    b: super::bowed::Bowed,
    pose: &super::bowed::Pose,
    who: Who,
    tone: u8,
) {
    let s = super::bowed_figure::skeleton(b, pose);
    let fig = figure(who);
    let held = b.dims().held;
    let (posed, seat) = body(fig, &s, held, [None, None]);
    if held {
        let hips = posed.at(HIPS, fig.bones[HIPS].head);
        let at = v(hips.x, 0.0, hips.z) - s.f * 6.0;
        super::bowed_props::chair_top(m, at, s.f.x.atan2(s.f.z), seat);
    }
    let (pts, nrm) = skin(fig, &posed);
    emit(m, fig, &pts, &nrm, who, tone, 1);
    // What the floor's shadow takes: the body's outline points.
    m.shadows.push(
        pts.iter()
            .step_by(97)
            .copied()
            .filter(|p| p.y < 60.0)
            .collect(),
    );
    if who == Who::Woman {
        super::bowed_human_skirt::skirt(m, fig, &posed, &s);
    }
}

/// The figure's baked pose for `b` (its arms at rest), kept: the live arms start from its
/// shoulders.
pub fn rest_body(b: super::bowed::Bowed, who: Who) -> std::sync::Arc<(Posed, f32)> {
    use std::sync::{Arc, Mutex};
    type Kept = Vec<((super::bowed::Bowed, Who), Arc<(Posed, f32)>)>;
    static KEPT: Mutex<Kept> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, p)) = kept.iter().find(|(k, _)| *k == (b, who)) {
        return p.clone();
    }
    let s = super::bowed_figure::skeleton(b, &super::bowed::Pose::of(b));
    let p = Arc::new(body(figure(who), &s, b.dims().held, [None, None]));
    kept.push(((b, who), p.clone()));
    p
}

/// The points the camera fits for the realistic figure holding `b`: its crown with room
/// over it, its head's sides.
pub fn fit(b: super::bowed::Bowed, who: Who) -> Vec<V3> {
    let fig = figure(who);
    let p = rest_body(b, who);
    let eyes = (fig.mark("eye.L") + fig.mark("eye.R")) * 0.5;
    let at = |q: V3| p.0.at(HEAD, q);
    vec![
        at(eyes + v(0.0, 17.0, -6.0)),
        at(eyes + v(9.5, 4.0, -6.0)),
        at(eyes + v(-9.5, 4.0, -6.0)),
    ]
}
