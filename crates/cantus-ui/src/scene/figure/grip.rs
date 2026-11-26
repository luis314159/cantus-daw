//! r42:arcus-upper — the grip library (ADR 0159): where a hand's joints go to hold a thing,
//! as data a figure's arm is then posed to (the wrist by the arm's two bones; the fingers by
//! `bowed_human::arm_aims`, each phalanx turned the shortest way, the non-playing fingers
//! relaxed to their own length). Each grip is designed against photographs of real hands
//! (`tools/arcus-figures/hand_design.py` renders one in seconds).
//!
//! - **BowFrog** — a bow held at its frog, as the owner's reference and the method books
//!   show it (sheet v7).
//! - **Rod** — a plain power grip round a rod (a stick, a mallet's handle, a drumstick's):
//!   the second client's, which keeps the contract from being a bow's.
//!
//! The necks' grips (a violin's, a cello's) are worked out by Arcus from its strings
//! (`views/inst/arcus_draw_hand::left_pose`), the fingertips on their stops.

use crate::scene::geo::V3;

/// The woman's hand from the wrist to its knuckles' middle (cm): the grips' unit.
pub const WOMAN_REACH: f32 = 8.64;

/// A hand's joints: the wrist, the palm's middle, each finger's knuckle, middle joint, last
/// joint and tip (index … little finger), the thumb's base, knuckle, joint and tip.
#[derive(Copy, Clone, Debug)]
pub struct Joints {
    pub wrist: V3,
    pub palm: V3,
    pub fingers: [[V3; 4]; 4],
    pub thumb: [V3; 4],
}

/// What a hand holds.
#[derive(Copy, Clone, Debug)]
pub enum Grip {
    /// A bow at its `frog` (the hair's point there; `dir` frog to tip; `n` the side the
    /// stick is on; `facing` the player's, the side away from the player is the far side;
    /// `held` a violin's or a viola's bow: the little finger curved on top).
    BowFrog {
        frog: V3,
        dir: V3,
        n: V3,
        facing: V3,
        held: bool,
    },
    /// A rod from `a` to `b` (`r` thick) held at `at` (0…1 along it), the hand's back
    /// toward `back`, the fingers wrapping over toward `wrap`.
    Rod {
        a: V3,
        b: V3,
        at: f32,
        r: f32,
        back: V3,
        wrap: V3,
    },
}

/// The joints for grip `g`, the hand `k` times the woman's (`side` 0 the left hand, 1 the
/// right).
pub fn joints(g: &Grip, k: f32, side: usize) -> Joints {
    match *g {
        Grip::BowFrog {
            frog,
            dir,
            n,
            facing,
            held,
        } => bow_frog(frog, dir, n, facing, held, k),
        Grip::Rod {
            a,
            b,
            at,
            r,
            back,
            wrap,
        } => rod(a, b, at, r, back, wrap, k, side),
    }
}

/// The bow hold (sheet v7, designed against the Commons photographs "Bow hand
/// Violoncello.jpg" and "Right hand position on bow.jpg"):
///
/// - the hand **on top of the stick**, its knuckles over it and a little toward the player,
///   the knuckles' line along the stick, rolled onto the index (over is up in the room made
///   square to the bow, not the string's normal: a cellist's hand hung under the stick from
///   the camera's side);
/// - the wrist relaxed, behind and over the knuckles, the forearm coming from the player's
///   side;
/// - the **fingers draped over the stick** to its far side, spread a little, the stick
///   across them at the middle phalanx (the index resting on it, toward the tip), the middle
///   finger opposite the thumb, the ring finger over the frog;
/// - the little finger draped as the others on a cello or a bass, **curved on top of the
///   stick** on a violin or a viola, its tip standing on it;
/// - the **thumb bent**, its tip touching the stick at the frog's front corner from the near
///   side underneath, its joint bowed out.
pub fn bow_frog(frog: V3, dir: V3, n: V3, facing: V3, held: bool, k: f32) -> Joints {
    let up = V3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    let lift = (up - dir * up.dot(dir)).norm();
    let mut over = (lift + n * 0.35).norm();
    over = (over - dir * over.dot(dir)).norm();
    // The far side of the stick (away from the player): the fingers drape over to it.
    let mut far = over.cross(dir).norm();
    if far.dot(facing) < 0.0 {
        far = -far;
    }
    // The stick's middle at the frog's front corner (Arcus's stick: 2.1 cm over the hair).
    let s = frog + n * 1.95;
    let at = |u: f32, o: f32, f: f32| s + (dir * u + over * o + far * f) * k;
    let g = bow_design();
    // (r42:arcus-hands: the knuckles high and round over the stick, the hand leant toward
    // the index (its knuckles' line falling toward the tip), the fingers curled round the
    // stick and slanting toward the tip — `figure::fingers::curl` bends their three joints
    // together; against the Commons photograph "Right hand position on bow.jpg".)
    let kc = at(g.kc[0], g.kc[1], g.kc[2]);
    let hd = (far * g.hd[0] - over * g.hd[1] + dir * g.hd[2]).norm();
    let kx = (dir - over * g.lean).norm();
    let wrist = kc - hd * (8.6 * k);
    let palm = kc - hd * (3.6 * k);
    let mut fingers = [[V3::default(); 4]; 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let knuckle = kc + kx * ((1.5 - j as f32) * 1.9 * k);
        let u = g.along[j];
        let (mid, tip) = if j == 3 && held {
            // Curved on top: its middle joint arched up, its tip standing on the stick.
            (
                at(g.little_mid[0], g.little_mid[1], g.little_mid[2]),
                at(g.little_tip[0], g.little_tip[1], g.little_tip[2]),
            )
        } else if j == 3 {
            (at(u, 1.3, 0.55), at(u - 0.2, -0.2, 0.95))
        } else {
            (
                at(u, g.mid[0], g.mid[1]),
                at(u + g.slant[j], -g.hang[j], g.tip_far),
            )
        };
        *f = [knuckle, mid, mid.lerp(tip, 0.55), tip];
    }
    // The thumb: bent, its tip at the frog's front corner under the stick's near side.
    let t3 = at(g.thumb_tip[0], g.thumb_tip[1], g.thumb_tip[2]);
    let t2 = at(g.thumb_ip[0], g.thumb_ip[1], g.thumb_ip[2]);
    let t1 = palm.lerp(t2, 0.45) - far * 0.6 - over * 0.4;
    let t0 = wrist.lerp(kc, 0.35) - far * 1.0 - over * 0.6;
    Joints {
        wrist,
        palm,
        fingers,
        thumb: [t0, t1, t2, t3],
    }
}

/// r42:arcus-hands — the bow hold's numbers (cm in the stick's frame at the frog's front
/// corner: along it toward the tip, over it, to its far side; the woman's hand).
#[derive(Copy, Clone, Debug)]
struct BowDesign {
    /// The knuckles' middle.
    kc: [f32; 3],
    /// The hand's way from the wrist (far, down, toward the tip).
    hd: [f32; 3],
    /// How far the knuckles' line falls toward the tip (the hand leant to the index).
    lean: f32,
    /// Each finger's middle joint along the stick.
    along: [f32; 4],
    /// The middle joints over the stick and to its far side.
    mid: [f32; 2],
    /// Each tip under the stick, slanted toward the tip, and to the far side.
    hang: [f32; 3],
    slant: [f32; 3],
    tip_far: f32,
    little_mid: [f32; 3],
    little_tip: [f32; 3],
    thumb_tip: [f32; 3],
    thumb_ip: [f32; 3],
}

const BOW: BowDesign = BowDesign {
    kc: [0.6, 4.4, -1.2],
    hd: [0.8, 0.45, 0.2],
    lean: 0.6,
    along: [2.0, 0.5, -1.1, -2.6],
    mid: [2.0, 1.1],
    hang: [0.7, 0.6, 0.5],
    slant: [1.1, 0.6, 0.2],
    tip_far: 0.1,
    little_mid: [-2.2, 2.8, -0.6],
    little_tip: [-2.9, 0.55, 0.0],
    thumb_tip: [0.3, -0.35, -0.45],
    thumb_ip: [-0.5, -1.0, -1.45],
};

/// [`BOW`], or while a bow hold is being designed in the app (`CANTUS_BOW_GRIP` =
/// `name=a,b,c;…` over its fields, read once — DEVELOPING §4; unset in the app).
fn bow_design() -> BowDesign {
    static D: std::sync::OnceLock<BowDesign> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        let mut g = BOW;
        let Ok(spec) = std::env::var("CANTUS_BOW_GRIP") else {
            return g;
        };
        for item in spec.split(';') {
            let Some((name, vals)) = item.split_once('=') else {
                continue;
            };
            let v: Vec<f32> = vals
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            let put = |dst: &mut [f32]| {
                for (d, x) in dst.iter_mut().zip(&v) {
                    *d = *x;
                }
            };
            match name.trim() {
                "kc" => put(&mut g.kc),
                "hd" => put(&mut g.hd),
                "lean" => put(std::slice::from_mut(&mut g.lean)),
                "along" => put(&mut g.along),
                "mid" => put(&mut g.mid),
                "hang" => put(&mut g.hang),
                "slant" => put(&mut g.slant),
                "tip_far" => put(std::slice::from_mut(&mut g.tip_far)),
                "little_mid" => put(&mut g.little_mid),
                "little_tip" => put(&mut g.little_tip),
                "thumb_tip" => put(&mut g.thumb_tip),
                "thumb_ip" => put(&mut g.thumb_ip),
                _ => {}
            }
        }
        g
    })
}

/// A power grip round a rod: the knuckles over it on the `back` side, the fingers wrapping
/// over toward `wrap` and round under it, the thumb round the other way; the index toward
/// the thumb's side of the hand (the rod's `a` end for a right hand, `b` for a left).
#[allow(clippy::too_many_arguments)]
pub fn rod(a: V3, b: V3, at: f32, r: f32, back: V3, wrap: V3, k: f32, side: usize) -> Joints {
    let ax = (b - a).norm();
    let back = (back - ax * back.dot(ax)).norm();
    let wrap = (wrap - ax * wrap.dot(ax) - back * wrap.dot(back)).norm();
    let p = a + (b - a) * at;
    // The index lies toward the thumb: a right hand's thumb toward the rod's `a` end.
    let toward = if side == 1 { -ax } else { ax };
    let kc = p + back * (r + 1.9 * k) - wrap * (0.6 * k);
    let hd = (wrap * 0.75 + back * -0.2 - toward * 0.15).norm();
    let wrist = kc - hd * (8.6 * k);
    let palm = kc - hd * (3.6 * k);
    let mut fingers = [[V3::default(); 4]; 4];
    for (j, f) in fingers.iter_mut().enumerate() {
        let u = (1.5 - j as f32) * 1.9 * k;
        let knuckle = kc + toward * u;
        let c = p + toward * (u * 0.95);
        let mid = c + wrap * (r + 1.2 * k) + back * (0.6 * k);
        // (r42:arcus-hands, pass 2: the tips curled under the rod and back toward the palm,
        // a power grip's — with the fingers lengthened and curled at all three joints the
        // middle joints had stood 4.1-4.2 cm off a stick, figure_contract's 4; now about
        // 3.1 / 3.5 cm hers / his, worked out on `fingers::curl`'s own reach.)
        let tip = c - wrap * (r * 0.4) - back * (r + 0.8 * k);
        *f = [knuckle, mid, mid.lerp(tip, 0.5), tip];
    }
    let c = p + toward * (2.6 * k);
    let t3 = c - back * (r + 0.6 * k) + wrap * (0.4 * k);
    let t2 = c - wrap * (r + 1.0 * k) - back * (0.3 * k);
    let t1 = palm.lerp(t2, 0.4) - back * (0.8 * k);
    let t0 = wrist.lerp(kc, 0.35) + toward * (1.5 * k) - back * (0.8 * k);
    Joints {
        wrist,
        palm,
        fingers,
        thumb: [t0, t1, t2, t3],
    }
}
