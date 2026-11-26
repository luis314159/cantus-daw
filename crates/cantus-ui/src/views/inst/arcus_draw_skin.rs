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
const TIP_RED: f32 = 0.62;
const KNUCKLE_RED: f32 = 0.5;

/// r42:arcus-hands, pass 3 — `CANTUS_HAND_TUNE="a,b,_,d…"`: the hands' look's numbers
/// overridden while designing it in `shoot_arcus` (index `i`: 0–2 the gains, 3–5 the sheen,
/// 6–8 the blood, 9 the tips, 10 the knuckles, 11 the creases' floor; `_` keeps one).
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

/// The redness tint's gains (see [`BLOOD`]) for `red` units of redness.
pub fn blood(red: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| 1.0 + tune(6 + i, BLOOD[i]) * red)
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
    tune(11, 0.45)
}

/// The fingertips' pads' redness (see [`TIP_RED`]).
pub fn tip_red() -> f32 {
    tune(9, TIP_RED)
}

/// The knuckles' redness (see [`KNUCKLE_RED`]).
pub fn knuckle_red() -> f32 {
    tune(10, KNUCKLE_RED)
}
