//! The rig and drawing helpers the cast is built with (round 35, ADR 0090).
//!
//! - this file: the maths of motion — easing, arcs, pulses, follow-through, squash and
//!   stretch, a point on a turning ball (the pseudo-3D every character's head and body
//!   use), and the **ground**: the transform of the floor under a character, which the
//!   renderer's lift and spin do not move (its shadow, a dust puff, confetti fall there).
//! - [`draw`]: shapes (ellipses, smooth blobs, tapered limbs along a curve) and the one way
//!   the family paints a part — an outline of varying weight, a soft shade, a light, a rim.
//! - [`fx`]: what the beat adds — sparkles, music notes, dust, confetti, a sleepy *z*.
//!
//! Everything is a pure function of its arguments and calls `cantus_dsp::math`, so the
//! same pose gives the same pixels on every system (ADR 0031 R9).

#[path = "rig_dance.rs"]
pub mod dance;
#[path = "rig_draw.rs"]
pub mod draw;
#[path = "rig_face.rs"]
pub mod face;
#[path = "rig_fx.rs"]
pub mod fx;

use crate::pose::Pose;
use crate::render::Canvas;
use cantus_dsp::math;
use tiny_skia::Transform;

pub use std::f32::consts::{PI, TAU};

/// A point in a character's units.
pub type P = (f32, f32);

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp_p(a: P, b: P, t: f32) -> P {
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t))
}

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Degrees to radians (no transcendental: allowed).
pub fn rad(deg: f32) -> f32 {
    deg * (PI / 180.0)
}

pub fn sin_deg(deg: f32) -> f32 {
    math::sin(rad(deg))
}

pub fn cos_deg(deg: f32) -> f32 {
    math::cos(rad(deg))
}

/// `sin(τ·t)`: one swing per cycle.
pub fn wave(t: f32) -> f32 {
    math::sin(t * TAU)
}

/// `cos(τ·t)`.
pub fn wave_c(t: f32) -> f32 {
    math::cos(t * TAU)
}

/// Smoothstep.
pub fn smooth(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_in(t: f32) -> f32 {
    let t = clamp01(t);
    t * t
}

pub fn ease_out(t: f32) -> f32 {
    let t = clamp01(t);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Cubic in-out.
pub fn ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let u = -2.0 * t + 2.0;
        1.0 - u * u * u / 2.0
    }
}

/// Overshoots its target a little and settles: a pose that *arrives*.
pub fn ease_out_back(t: f32) -> f32 {
    let t = clamp01(t);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    let u = t - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

/// A damped wobble after a hit at t = 0: 1 → 0 with a few swings (follow-through).
pub fn wobble(t: f32, swings: f32) -> f32 {
    if t <= 0.0 {
        return 1.0;
    }
    let decay = math::exp(-4.5 * t);
    decay * math::cos(t * swings * TAU)
}

/// A jump's arc: 0 at both ends, 1 in the middle (a parabola).
pub fn arc(t: f32) -> f32 {
    let t = clamp01(t);
    4.0 * t * (1.0 - t)
}

/// How far `t` is through the window `a…b` (0 before, 1 after).
pub fn window(t: f32, a: f32, b: f32) -> f32 {
    if b <= a {
        return if t >= b { 1.0 } else { 0.0 };
    }
    clamp01((t - a) / (b - a))
}

/// A smooth bump 0 → 1 → 0 centred on `at`, `width` wide, around a cycle (wraps 0…1).
pub fn pulse(t: f32, at: f32, width: f32) -> f32 {
    let mut d = (t - at).rem_euclid(1.0);
    if d > 0.5 {
        d -= 1.0;
    }
    let x = d.abs() / (width * 0.5).max(1e-4);
    if x >= 1.0 {
        0.0
    } else {
        let u = 1.0 - x * x;
        u * u
    }
}

/// A phase a little behind `t` (follow-through: what hangs on a part arrives later).
pub fn lag(t: f32, by: f32) -> f32 {
    (t - by).rem_euclid(1.0)
}

/// ±1 by a cycle's parity: what alternates left and right.
pub fn side(cycle: u32) -> f32 {
    if cycle.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// Squash (`amount` > 0, wider and shorter) or stretch (< 0), keeping the area.
pub fn squash(amount: f32) -> (f32, f32) {
    // Held to what still reads as the same body (and keeps it inside its bounds).
    let sy = (1.0 - amount).clamp(0.76, 1.3);
    (1.0 / sy, sy)
}

/// Rotate `p` about `about` by `deg` (clockwise on screen, y down).
pub fn rotate(p: P, about: P, deg: f32) -> P {
    let (s, c) = math::sin_cos(rad(deg));
    let (x, y) = (p.0 - about.0, p.1 - about.1);
    (about.0 + x * c - y * s, about.1 + x * s + y * c)
}

/// A rotation by `deg` (clockwise on screen, y down) as a transform, built from
/// `cantus_dsp::math::sin_cos`: tiny-skia's own rotation constructors call the platform's
/// sine and cosine, whose last bits differ between systems (ADR 0031 R9), and a pose must
/// give the same pixels everywhere. Every rotation in this crate goes through here
/// (`tests/determinism.rs` holds it).
pub fn rotation(deg: f32) -> Transform {
    let (s, c) = math::sin_cos(rad(deg));
    Transform::from_row(c, s, -s, c, 0.0, 0.0)
}

/// [`rotation`] about the point (`cx`, `cy`).
pub fn rotation_at(deg: f32, cx: f32, cy: f32) -> Transform {
    Transform::from_translate(cx, cy)
        .pre_concat(rotation(deg))
        .pre_translate(-cx, -cy)
}

/// The point `len` from `from` in the direction `deg` (0 = straight down, 90 = to the
/// right, 180 = up, −90 = to the left).
pub fn reach(from: P, len: f32, deg: f32) -> P {
    let (s, c) = math::sin_cos(rad(deg));
    (from.0 + s * len, from.1 + c * len)
}

/// A number in 0…1 from an integer: the same everywhere (`pose::hash01`).
pub fn hash(n: i64) -> f32 {
    crate::pose::hash01(n) as f32
}

/// A point on a ball that turns: the pseudo-3D of every head and body in the family.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ball {
    pub center: P,
    pub rx: f32,
    pub ry: f32,
    /// Degrees it has turned about its vertical axis (0 = facing us, 90 = its right side
    /// toward us… shown on screen as turning to the viewer's right).
    pub yaw: f32,
}

/// A point projected from a [`Ball`]: where it shows, how much it faces us (−1 away … 1
/// toward us), and how wide something lying on the surface there looks (its foreshortening).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OnBall {
    pub at: P,
    pub z: f32,
    pub squeeze: f32,
}

impl Ball {
    /// The point at longitude `lon` (degrees, 0 = the front, positive to the viewer's
    /// right) and latitude `lat` (degrees, positive up), `out` × the radius from the centre
    /// (1 = on the surface).
    pub fn at(&self, lon: f32, lat: f32, out: f32) -> OnBall {
        let (sl, cl) = math::sin_cos(rad(lon + self.yaw));
        let (sa, ca) = math::sin_cos(rad(lat));
        OnBall {
            at: (
                self.center.0 + self.rx * out * ca * sl,
                self.center.1 - self.ry * out * sa,
            ),
            z: ca * cl,
            squeeze: cl.max(0.0),
        }
    }
}

/// The transform of the ground under a character on `canvas` (its feet's spot, in its own
/// units, unlifted and unspun): where a contact shadow, dust and falling confetti belong.
/// The inverse of `render::placement`'s lift and spin (a test holds them together).
pub fn ground(canvas: &Canvas, pose: &Pose, faces_itself: bool) -> Transform {
    let px = canvas.px_per_unit;
    let m = if !faces_itself && pose.turn < 0.0 {
        -1.0
    } else {
        1.0
    };
    let t = canvas.transform;
    // placement = T(feet) · S(px·m, px) · T(0, −½) · R(spin) · T(0, ½); the origin lands at
    // feet + S·(−½·sin, ½·cos − ½).
    let (s, c) = math::sin_cos(rad(pose.spin));
    let feet_x = t.tx - px * m * (-0.5 * s);
    let feet_y = t.ty - px * (0.5 * c - 0.5);
    Transform::from_row(px * m, 0.0, 0.0, px, feet_x, feet_y + pose.lift * 0.6 * px)
}

/// The energy a move is drawn with: never quite still, never wild.
pub fn vigour(pose: &Pose) -> f32 {
    0.35 + 0.65 * clamp01(pose.energy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{placement, Background, Stage};
    use crate::{cast, pose_at, DanceFrame};

    #[test]
    fn easing_ends_where_it_should() {
        for f in [smooth, ease_in, ease_out, ease_in_out, ease_out_back] {
            assert!(f(0.0).abs() < 1e-6);
            assert!((f(1.0) - 1.0).abs() < 1e-5);
        }
        assert!(ease_out_back(0.7) > 1.0, "a back ease overshoots");
        assert_eq!(arc(0.5), 1.0);
        assert!(
            pulse(0.0, 0.98, 0.1) > 0.5,
            "a pulse wraps around the cycle"
        );
        assert_eq!(pulse(0.5, 0.0, 0.2), 0.0);
        let (sx, sy) = squash(0.2);
        assert!((sx * sy - 1.0).abs() < 1e-5, "squash keeps the area");
    }

    #[test]
    fn a_ball_faces_us_at_its_front() {
        let b = Ball {
            center: (0.0, -0.5),
            rx: 0.4,
            ry: 0.4,
            yaw: 0.0,
        };
        let f = b.at(0.0, 0.0, 1.0);
        assert!((f.z - 1.0).abs() < 1e-6 && f.at.0.abs() < 1e-6);
        let turned = Ball { yaw: 90.0, ..b }.at(0.0, 0.0, 1.0);
        assert!(turned.at.0 > 0.39 && turned.z.abs() < 1e-5);
    }

    #[test]
    fn the_ground_undoes_lift_and_spin() {
        let ch = cast::default_character();
        let stage = Stage::new(400, 300, Background::Transparent);
        let frame = DanceFrame {
            playing: true,
            beats: 2.25,
            ..DanceFrame::default()
        };
        let base = pose_at(&frame, ch.as_ref(), 0.0);
        for (lift, spin, turn) in [
            (0.0, 0.0, 0.0),
            (0.4, 0.0, 1.0),
            (0.3, 75.0, -1.0),
            (0.9, 300.0, 0.5),
        ] {
            let pose = Pose {
                lift,
                spin,
                turn,
                ..base
            };
            let (t, px) = placement(&stage, ch.as_ref(), &pose);
            let mut pixmap = tiny_skia::Pixmap::new(4, 4).unwrap();
            let canvas = Canvas {
                pixmap: &mut pixmap,
                transform: t,
                px_per_unit: px,
                hue: 0.0,
                shadow: true,
            };
            let g = ground(&canvas, &pose, ch.faces_itself());
            let flat = Pose {
                lift: 0.0,
                spin: 0.0,
                ..pose
            };
            let (want, _) = placement(&stage, ch.as_ref(), &flat);
            for (a, b) in [
                (g.tx, want.tx),
                (g.ty, want.ty),
                (g.sx, want.sx),
                (g.sy, want.sy),
            ] {
                assert!((a - b).abs() < 1e-3, "lift {lift} spin {spin}: {a} vs {b}");
            }
        }
    }
}
