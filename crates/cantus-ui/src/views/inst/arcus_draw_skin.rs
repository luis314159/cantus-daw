//! r42:arcus-hands, pass 3 — the live hands' own skin under the light (the coordinator at
//! 400 % against crop-08, crop-02 and crop-11: "flat orange-yellow: no specular sheen, no
//! subsurface redness at the joints and tips, no value changes over the back of the hand").
//! Kept apart from the arms' shading (`flesh::colour`, `flesh::concert_colour`, the face's
//! and the clothes' — arcus-polish's) so the hands' look is changed here alone.
//!
//! Measured on the photographs (the mean of a patch, display-referred): the back and the
//! fingers' hue 11–24° (crop-11's 11, crop-02's 18–24, crop-08's 21–24), green over red
//! 0.69–0.80 and blue over red 0.48–0.68; the hands drawn before this pass 26–28°, 0.71 and
//! 0.47 — more yellow and more saturated than any of them, and than the face beside them.

use crate::scene::geo::V3;
use std::sync::OnceLock;

/// The hands' skin's gains over the skin's albedo (linear, red, green, blue) under the
/// concert's light — was the arms' `SKIN_WARM` (1.1, 0.95, 0.78) on top of the concert's
/// own warm skin gain: blue lifted so the hue comes to the photographs' ~20° and the
/// lit fingers to their range (measured after: hue 23–25°, green over red 0.69–0.73,
/// blue over red 0.51–0.56 — crop-02's and crop-08's family; the light itself is warm).
const GAIN: [f32; 3] = [1.05, 0.95, 1.02];

/// Skin's soft sheen (the knuckles' and the back's soft highlights in crop-08 and crop-02):
/// a broad lobe's strength and power, and the sheen at a glance (skin's oil layer reflects
/// the room as it turns away).
const SHEEN: (f32, i32, f32) = (0.055, 7, 0.03);

/// How the skin's redness detail tints (red, green, blue gains a unit of redness): blood
/// under the skin reads pink-red — the green falls most, the blue less (was 0.22, −0.12,
/// −0.18 on red, green, blue: toward orange).
const BLOOD: [f32; 3] = [0.14, -0.17, -0.07];

/// The fingertips' and the knuckles' redness (the detail's red at a tip's pad and over a
/// joint's back; was 0.42 and 0.4): crop-02's pinker tips and joints.
const TIP_RED: f32 = 0.5;
const KNUCKLE_RED: f32 = 0.5;

/// r42:arcus-hands, pass 3 — `CANTUS_HAND_TUNE="a,b,_,d…"`: the hands' look's numbers
/// overridden while designing it in `shoot_arcus` (index `i`: 0–2 the gains, 3–5 the sheen,
/// 6–8 the blood, 9 the tips, 10 the knuckles, 11 the creases' floor, 12–14 the nail's bed
/// and free edge, 15 its folds, 16–17 the grain, 18–19 the wrinkles; `_` keeps one).
/// Read once; unset in the app.
pub fn tune(i: usize, d: f32) -> f32 {
    static T: OnceLock<Vec<Option<f32>>> = OnceLock::new();
    T.get_or_init(|| {
        std::env::var("CANTUS_HAND_TUNE")
            .map(|s| s.split(',').map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_default()
    })
    .get(i)
    .copied()
    .flatten()
    .unwrap_or(d)
}

/// The hands' skin's gains (see [`GAIN`]).
pub fn gain() -> [f32; 3] {
    [0, 1, 2].map(|i| tune(i, GAIN[i]))
}

/// The redness tint's gains (see [`BLOOD`]) for `red` units of redness, on skin `hand` the
/// hand's (the arm's own, as before, elsewhere).
pub fn blood(red: f32, hand: f32) -> [f32; 3] {
    const ARM: [f32; 3] = [0.22, -0.12, -0.18];
    [0, 1, 2].map(|i| 1.0 + (ARM[i] + (tune(6 + i, BLOOD[i]) - ARM[i]) * hand) * red)
}

/// The soft sheen's radiance share at a point with normal `n`, the way to the eye `e` and
/// to the key `l` (`ao`: its occlusion): the broad lobe where the key lights it, and the
/// glance's whatever the key.
pub fn sheen(n: V3, e: V3, l: V3, ao: f32) -> (f32, f32) {
    let h = (l + e).norm();
    let ndl = n.dot(l);
    let ndv = n.dot(e).clamp(0.0, 1.0);
    let lobe = n.dot(h).max(0.0).powi(tune(4, SHEEN.1 as f32) as i32)
        * tune(3, SHEEN.0)
        * ao
        * (ndl > 0.0) as u8 as f32;
    let glance = (1.0 - ndv).powi(4) * tune(5, SHEEN.2) * ao;
    (lobe, glance)
}

/// How much of a crease shows at its joint's full bend (it fades from 1 as the joint
/// bends): crop-02's and crop-08's bent middle joints still show their folds round the
/// stretched knuckle (was 0).
pub fn crease_floor() -> f32 {
    tune(11, 0.6)
}

/// The fingertips' pads' redness (see [`TIP_RED`]).
pub fn tip_red() -> f32 {
    tune(9, TIP_RED)
}

/// The knuckles' redness (see [`KNUCKLE_RED`]).
pub fn knuckle_red() -> f32 {
    tune(10, KNUCKLE_RED)
}

thread_local! {
    static HAND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `f` with the light worked out as the hands' own skin (`hand`) or the arms' (the arms'
/// maps and the hands' are kept apart; a vertex blends them by its share of the hand).
pub fn as_hand<R>(hand: bool, f: impl FnOnce() -> R) -> R {
    let was = HAND.with(|h| h.replace(hand));
    let r = f();
    HAND.with(|h| h.set(was));
    r
}

/// Whether the light is being worked out as the hands' skin (see [`as_hand`]).
pub fn hand_now() -> bool {
    HAND.with(|h| h.get())
}

/// r42:arcus-hands, pass 4 — a nail's detail (luminance, redness) at `t` along its finger's
/// last bone: the bed pink through the plate, the lunula a paler crescent at its root, the
/// free edge white past the tip's pad (crop-08's raised index: a pink plate, a white rim).
pub fn nail(t: f32) -> [f32; 2] {
    let edge = ((t - 0.8) / 0.1).clamp(0.0, 1.0);
    let lunula = (1.0 - ((t - 0.5) / 0.1).clamp(0.0, 1.0)) * (1.0 - edge);
    let bed = 1.0 - edge - lunula;
    [
        tune(12, 0.96) * bed + 1.06 * lunula + tune(13, 1.2) * edge,
        tune(14, 0.5) * bed + 0.1 * lunula - 0.6 * edge,
    ]
}

/// r42:arcus-hands, pass 4 — the skin of a nail's folds (its detail `d`, `t` along the last
/// bone): the cuticle's line at the root darker and redder, the side folds less so.
pub fn fold(d: [f32; 2], t: f32) -> [f32; 2] {
    let root = 1.0 - ((t - 0.55) / 0.15).clamp(0.0, 1.0);
    let k = tune(15, 0.14) * (0.5 + 0.5 * root);
    [d[0] * (1.0 - k), d[1] + 0.3 + 0.2 * root]
}

/// r42:arcus-hands, pass 5 — the skin's fine grain at rest (the coordinator: "smooth and
/// young"; crop-08's back of the hand): a pore-scale value noise (two octaves, cells of
/// `GRAIN.0` and half that, `GRAIN.1` of the luminance) and, over a finger joint's back, its
/// dorsal wrinkles — fine arcs across the finger `WRINKLE.0` cm apart, `WRINKLE.1` darker at
/// their deepest, over `WRINKLE.2` cm either side of the joint.
const GRAIN: (f32, f32) = (0.07, 0.05);
const WRINKLE: (f32, f32, f32) = (0.11, 0.16, 0.32);

/// The grain of one hand triangle: the finger joint nearest it, if any (its place, the
/// finger's way and the back's, at rest).
pub struct Grain {
    joint: Option<(V3, V3, V3)>,
    // (r42:arcus-hands, pass 6: the numbers read once a triangle, not once a sample — the
    // inverse cell, the noise's depth, the wrinkles' spacing and depth.)
    k: [f32; 4],
}

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0 - 0.5
}

/// Trilinear value noise at `p` (cells of 1), −0.5 … 0.5.
fn noise(p: V3) -> f32 {
    let (fx, fy, fz) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (x, y, z) = (fx as i32, fy as i32, fz as i32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (s(p.x - fx), s(p.y - fy), s(p.z - fz));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx: i32, dy: i32, dz: i32| hash3(x + dx, y + dy, z + dz);
    l(
        l(
            l(c(0, 0, 0), c(1, 0, 0), u),
            l(c(0, 1, 0), c(1, 1, 0), u),
            v,
        ),
        l(
            l(c(0, 0, 1), c(1, 0, 1), u),
            l(c(0, 1, 1), c(1, 1, 1), u),
            v,
        ),
        w,
    )
}

impl Grain {
    /// Triangle `tri` (the arm's vertices `verts`) of figure `fig`'s arm `s`.
    pub fn of(
        fig: &crate::scene::bowed_human::Figure,
        s: usize,
        verts: &[u32],
        tri: &[u16; 3],
    ) -> Grain {
        use crate::scene::bowed_human as hu;
        let o = hu::side(s);
        let g = verts[tri[0] as usize] as usize;
        let c = tri
            .iter()
            .map(|&i| fig.pos[verts[i as usize] as usize])
            .fold(V3::default(), |a, b| a + b)
            * (1.0 / 3.0);
        let bone = fig.bw[g]
            .iter()
            .fold(
                (0u8, -1.0f32),
                |b, &(x, w)| if w > b.1 { (x, w) } else { b },
            )
            .0 as usize;
        let first = o + hu::finger(1, 0);
        if !(first..=o + hu::finger(4, 2)).contains(&bone) {
            return Grain {
                joint: None,
                k: Self::numbers(),
            };
        }
        let k = (bone - o - hu::finger(0, 0)) % 3;
        let bn = &fig.bones[bone];
        let mut best: Option<(f32, V3, V3)> = None;
        if k >= 1 {
            let prev = &fig.bones[bone - 1];
            best = Some(((c - bn.head).len(), bn.head, (bn.tail - prev.head).norm()));
        }
        if k <= 1 {
            let next = &fig.bones[bone + 1];
            let d = (c - bn.tail).len();
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, bn.tail, (next.tail - bn.head).norm()));
            }
        }
        let kn = |f: usize| fig.bones[o + hu::finger(f, 0)].head;
        let wr = fig.bones[o + hu::HAND].head;
        let mut back = (kn(1) - kn(4)).cross((kn(2) + kn(3)) * 0.5 - wr).norm();
        if (back.x < 0.0) == (s == 0) {
            back = back * -1.0;
        }
        Grain {
            joint: best.map(|(_, j, ax)| (j, ax, (back - ax * back.dot(ax)).norm())),
            k: Self::numbers(),
        }
    }

    fn numbers() -> [f32; 4] {
        [
            1.0 / tune(16, GRAIN.0),
            tune(17, GRAIN.1),
            1.0 / tune(18, WRINKLE.0),
            tune(19, WRINKLE.1),
        ]
    }

    /// The luminance factor at `q` (rest, cm).
    pub fn at(&self, q: V3) -> f32 {
        let [f, amp, per, deep] = self.k;
        let n = noise(q * f) * 0.65 + noise(q * (2.0 * f)) * 0.35;
        let mut k = 1.0 + amp * n;
        if let Some((j, ax, back)) = self.joint {
            let r = q - j;
            let s = r.dot(ax);
            let up = (r - ax * s).norm().dot(back);
            if up > 0.2 && s.abs() < WRINKLE.2 {
                let fade = (1.0 - (s / WRINKLE.2).powi(2)) * ((up - 0.2) / 0.5).clamp(0.0, 1.0);
                // (Arcs: the lines bow back toward the hand away from the knuckle's top.)
                let side = (r - ax * s - back * r.dot(back)).len();
                let ph = (s + 0.08 * side * side) * per;
                let line = (0.5 + 0.5 * (ph * std::f32::consts::TAU).cos()).powi(6);
                k *= 1.0 - deep * fade * line;
            }
        }
        k
    }
}
