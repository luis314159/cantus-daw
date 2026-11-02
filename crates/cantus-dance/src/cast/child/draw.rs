//! Drawing the child from a [`Body`]: shadow, a far arm, legs and sneakers, shorts, the
//! T-shirt with its star, the near arms, the head (ears, face, hair, clip), the extras.

use super::{pal, Child};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, ribbon_into, Sink};
use crate::rig::face::{self, FaceStyle, MouthStyle};
use crate::rig::{self, Ball, P};
use tiny_skia::Transform;

const ANCHOR_Y: f32 = -0.2;
const BODY: (P, f32, f32) = ((0.0, -0.37), 0.17, 0.17);
const HEAD: (P, f32, f32) = ((0.0, -0.76), 0.245, 0.24);

const FACE: FaceStyle = FaceStyle {
    eye_lon: 28.0,
    eye_lat: 2.0,
    eye_rx: 0.064,
    eye_ry: 0.08,
    white: pal::EYE_WHITE,
    pupil: pal::PUPIL,
    iris: pal::IRIS,
    lid: pal::SKIN.base,
    ink: pal::OUTLINE,
    cheek_lon: 44.0,
    cheek_lat: -22.0,
    cheek_rx: 0.055,
    cheek: pal::CHEEK,
    skin: pal::SKIN.base,
};

const MOUTH: MouthStyle = MouthStyle {
    width: 0.055,
    inside: pal::MOUTH,
    tongue: pal::TONGUE,
    ink: pal::OUTLINE,
};

fn ball(c: (P, f32, f32), yaw: f32) -> Ball {
    Ball {
        center: c.0,
        rx: c.1,
        ry: c.2,
        yaw,
    }
}

/// A five-pointed star at `c`, `r` across, turned by `deg`.
fn star_into(s: &mut dyn Sink, c: P, r: f32, sx: f32, deg: f32) {
    let pts: [P; 10] = std::array::from_fn(|k| {
        let rr = if k % 2 == 0 { r } else { r * 0.45 };
        let a = rig::rad(deg + 36.0 * k as f32 - 90.0);
        let (sn, cs) = cantus_dsp::math::sin_cos(a);
        (c.0 + rr * cs * sx, c.1 + rr * sn)
    });
    d::polygon_into(s, &pts);
}

/// Curly hair: bumps around the top of the head (and the back when it turns away).
fn hair_into(s: &mut dyn Sink, head: &Ball, g: f32) {
    // Curls framing the face down to the ears, and a crown of them on top.
    for k in 0..11 {
        let lon = -112.0 + 22.4 * k as f32;
        let lat = 20.0 + 42.0 * rig::cos_deg(lon * 0.8).max(0.0);
        let on = head.at(lon, lat, 1.04);
        if on.z < -0.4 {
            continue;
        }
        ellipse_into(s, on.at, 0.075 + g, 0.07 + g, 0.0);
    }
    // Round the back, down to the nape.
    for k in 0..5 {
        let lon = 135.0 + 22.5 * k as f32;
        for lat in [8.0, 38.0] {
            let on = head.at(lon, lat, 1.04);
            if on.z > -0.2 {
                ellipse_into(s, on.at, 0.08 + g, 0.075 + g, 0.0);
            }
        }
    }
    for (lon, lat) in [(-40.0, 78.0), (0.0, 84.0), (40.0, 78.0)] {
        let on = head.at(lon, lat, 1.12);
        ellipse_into(s, on.at, 0.09 + g, 0.085 + g, 0.0);
    }
    let crown = head.at(0.0, 75.0, 1.0);
    ellipse_into(s, (crown.at.0, crown.at.1 + 0.02), 0.19 + g, 0.1 + g, 0.0);
}

/// Arms (the ones `which`): a sleeve, the arm, a hand.
fn draw_arms(c: &mut Canvas, body: &Ball, b: &Body, which: &[usize]) {
    let arm = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = body.at(side * 78.0, 38.0, 0.95).at;
        let pts = kit::arm_points(sh, b.arm[i], b.bend[i], 0.14, 0.13, i);
        (pts, kit::forearm_deg(b.arm[i], b.bend[i], i))
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            kit::hose_into(s, arm(i).0, 0.06, 0.052, g);
        }
    };
    let hands = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, deg) = arm(i);
            kit::mitten_into(s, pts[2], deg, 0.058, b.spread[i], i, g);
        }
    };
    let sleeves = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, _) = arm(i);
            let end = rig::lerp_p(pts[0], pts[1], 0.55);
            ribbon_into(s, &[pts[0], end], 0.095 + 2.0 * g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        hands(s, g);
        sleeves(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::SKIN, 2);
    d::bands(c, &hands, &pal::SKIN, 2);
    d::bands(c, &sleeves, &pal::SHIRT, 2);
}

/// Legs from the shorts to white sneakers with a coral stripe.
fn draw_legs(c: &mut Canvas, body: &Ball, t: &Transform, b: &Body) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, (body.at(side * 30.0, -70.0, 0.9).at.0, -0.15));
        let f = b.foot[i];
        let foot = (side * 0.1 + f.0, f.1);
        let knee = (
            (hip.0 + foot.0) * 0.5 + side * 0.015,
            (hip.1 + foot.1) * 0.5,
        );
        ([hip, knee, (foot.0, foot.1 - 0.03)], foot, side, f.1)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            kit::hose_into(s, leg(i).0, 0.062, 0.056, g);
        }
    };
    let shoes = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (_, foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.15, 0.075, side, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        shoes(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::SKIN, 2);
    d::bands(c, &shoes, &pal::SHOE, 2);
    let stripes = |s: &mut dyn Sink, _: f32| {
        for i in 0..2 {
            let (_, foot, side, _) = leg(i);
            let a = (foot.0 - side * 0.02, foot.1 - 0.05);
            ribbon_into(s, &[a, (a.0 + side * 0.06, a.1 + 0.012)], 0.016);
        }
    };
    d::flat(c, &stripes, pal::STRIPE);
}

/// The head (in the head's frame): ears, the face, the hair and its clip, the features.
fn draw_head(c: &mut Canvas, head: &Ball, b: &Body, pose: &Pose) {
    let ears = |s: &mut dyn Sink, g: f32| {
        for side in [-1.0f32, 1.0] {
            let on = head.at(side * 90.0, -4.0, 1.0);
            if on.z > -0.5 {
                let w = 0.045 * (0.5 + 0.5 * (1.0 - on.squeeze));
                ellipse_into(s, on.at, w + g, 0.06 + g, 0.0);
            }
        }
    };
    d::part(c, &ears, &pal::SKIN, &pal::OUTLINE, 2);
    let face_shape =
        |s: &mut dyn Sink, g: f32| ellipse_into(s, head.center, head.rx + g, head.ry + g, 0.0);
    d::part(c, &face_shape, &pal::SKIN, &pal::OUTLINE, 3);
    let f = face::face_of(pose.mood);
    let lat = b.nod * 0.5;
    face::cheeks(c, head, &FACE, &f, b, pose);
    face::eyes(c, head, &FACE, &f, b, pose);
    let nose = head.at(0.0, -14.0 + lat, 1.0);
    if nose.z > 0.1 {
        let n = nose.at;
        d::flat(
            c,
            &|s, _| ellipse_into(s, n, 0.018, 0.013, 0.0),
            pal::SKIN.shade,
        );
    }
    let m = head.at(0.0, -32.0 + lat, 1.0);
    face::mouth(c, &m, &f, b.mouth, &MOUTH);
    // Curly hair over the top (and over the back, seen when turned away), bouncing late.
    let bob = Transform::from_translate(b.top * 0.0006, -b.top.abs() * 0.0004);
    let saved = c.transform;
    c.transform = saved.pre_concat(bob);
    d::part(
        c,
        &|s, g| hair_into(s, head, g),
        &pal::HAIR,
        &pal::OUTLINE,
        3,
    );
    // A gold star clip in the hair.
    let clip = head.at(-52.0, 58.0, 1.08);
    if clip.z > 0.0 {
        let star = |s: &mut dyn Sink, g: f32| {
            star_into(s, clip.at, 0.05 + g, 0.4 + 0.6 * clip.squeeze, -12.0)
        };
        d::outline(c, &star, &pal::OUTLINE.thin(0.7));
        d::flat(c, &star, pal::CLIP);
    }
    c.transform = saved;
}

pub(super) fn draw(child: &Child, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = kit::torso(b, ANCHOR_Y);
    let tt = base.pre_concat(t);
    let body = ball(BODY, b.yaw * 0.6);
    let head = ball(HEAD, b.yaw);
    let head_local = t.pre_concat(crate::rig::rotation_at(b.tilt, HEAD.0 .0, HEAD.0 .1 + 0.15));
    let head_t = base.pre_concat(head_local);
    kit::shadow(c, ground, pose, b, 0.28, pal::SHADOW);
    let arm_z = |i: usize| body.at(if i == 0 { -78.0 } else { 78.0 }, 38.0, 1.0).z;
    let (mut far, mut near) = ([0usize; 2], [0usize; 2]);
    let (mut nf, mut nn) = (0, 0);
    for i in 0..2 {
        if arm_z(i) < -0.1 {
            far[nf] = i;
            nf += 1;
        } else {
            near[nn] = i;
            nn += 1;
        }
    }
    kit::with(c, tt, |c| draw_arms(c, &body, b, &far[..nf]));
    draw_legs(c, &body, &t, b);
    kit::with(c, tt, |c| {
        let ws = 0.8 + 0.2 * rig::cos_deg(body.yaw).abs();
        let shorts = |s: &mut dyn Sink, g: f32| {
            d::blob_grown_into(
                s,
                &[
                    (-0.17 * ws, -0.27),
                    (0.17 * ws, -0.27),
                    (0.19 * ws, -0.13),
                    (0.0, -0.115),
                    (-0.19 * ws, -0.13),
                ],
                g,
            );
        };
        d::part(c, &shorts, &pal::SHORTS, &pal::OUTLINE, 2);
        let neck =
            |s: &mut dyn Sink, g: f32| ellipse_into(s, (0.0, -0.53), 0.06 + g, 0.06 + g, 0.0);
        d::part(c, &neck, &pal::SKIN, &pal::OUTLINE, 1);
        let shirt = |s: &mut dyn Sink, g: f32| {
            d::blob_grown_into(
                s,
                &[
                    (-0.14 * ws, -0.52),
                    (0.0, -0.545),
                    (0.14 * ws, -0.52),
                    (0.185 * ws, -0.4),
                    (0.18 * ws, -0.25),
                    (0.0, -0.23),
                    (-0.18 * ws, -0.25),
                    (-0.185 * ws, -0.4),
                ],
                g,
            );
        };
        d::part(c, &shirt, &pal::SHIRT, &pal::OUTLINE, 3);
        let chest = body.at(0.0, 5.0, 1.0);
        if chest.z > 0.2 {
            let sx = 0.4 + 0.6 * chest.squeeze;
            d::flat(
                c,
                &|s, _| star_into(s, (chest.at.0 * 0.8, chest.at.1), 0.075, sx, 0.0),
                pal::STAR,
            );
        }
        draw_arms(c, &body, b, &near[..nn]);
    });
    kit::with(c, head_t, |c| draw_head(c, &head, b, pose));
    let hand = |m: &Body, mt: &Transform, i: usize| {
        let mb = ball(BODY, m.yaw * 0.6);
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = mb.at(side * 78.0, 38.0, 0.95).at;
        kit::map(
            mt,
            kit::arm_points(sh, m.arm[i], m.bend[i], 0.14, 0.18, i)[2],
        )
    };
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let m = kit::map(&head_local, head.at(0.0, -20.0, 1.0).at);
                (m.0 + side * 0.34, m.1 - 0.16)
            }
            Anchor::Top => kit::map(&head_local, head.at(0.0, 90.0, 1.6).at),
            Anchor::Hand(i) => hand(b, &t, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &t, 0), hand(b, &t, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let m = kit::body_at(pose, &child.style, 0.0, 40.0);
        let mt = kit::torso(&m, ANCHOR_Y);
        rig::lerp_p(hand(&m, &mt, 0), hand(&m, &mt, 1), 0.5)
    };
    let look = FxLook {
        ink: pal::OUTLINE,
        sparkle: pal::SPARKLE,
        note: pal::NOTE,
        dust: pal::DUST,
        heart: pal::CHEEK,
        confetti: pal::CONFETTI,
    };
    kit::extras(c, pose, b, ground, &look, &anchor, &met);
}
