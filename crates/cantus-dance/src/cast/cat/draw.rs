//! Drawing the cat from a [`Body`]: its shadow, the tail (behind unless it turns away), a
//! far arm, the legs, the body with its white chest, the scarf, the near arms, the ears and
//! the head (it turns before the body), the face, the beat's extras.

use super::{pal, Cat};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, ribbon_into, Sink};
use crate::rig::face::{self, FaceStyle, MouthStyle};
use crate::rig::{self, Ball, P};
use tiny_skia::Transform;

const ANCHOR_Y: f32 = -0.07;
const BODY: (P, f32, f32) = ((0.0, -0.28), 0.225, 0.215);
const HEAD: (P, f32, f32) = ((0.0, -0.72), 0.3, 0.27);

const FACE: FaceStyle = FaceStyle {
    eye_lon: 30.0,
    eye_lat: 8.0,
    eye_rx: 0.074,
    eye_ry: 0.09,
    white: pal::EYE_WHITE,
    pupil: pal::PUPIL,
    iris: pal::IRIS,
    lid: pal::FUR.base,
    ink: pal::OUTLINE,
    cheek_lon: 46.0,
    cheek_lat: -16.0,
    cheek_rx: 0.06,
    cheek: pal::CHEEK,
    skin: pal::FUR.base,
};

const MOUTH: MouthStyle = MouthStyle {
    width: 0.045,
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

/// The tail: a long curve from the back of the body, its curl following the body late.
fn tail_points(body: &Ball, b: &Body) -> ([P; 6], bool) {
    let on = body.at(155.0, -45.0, 0.85);
    let dx = rig::sin_deg(155.0 + body.yaw);
    let away = on.z > 0.0;
    let side = if dx.abs() < 0.05 { 1.0 } else { dx.signum() };
    // The tail keeps up against the body's lean (it would sweep the floor otherwise).
    let mut deg = 180.0 - side * (55.0 + 30.0 * dx.abs()) + b.tail * 0.6 + b.lean * 1.3;
    let mut p = on.at;
    let mut pts = [p; 6];
    for (k, q) in pts.iter_mut().enumerate().skip(1) {
        // Each segment curls a little further: an S that the swing passes along.
        deg += side * (8.0 - 9.0 * k as f32) + b.tail * 0.35;
        p = rig::reach(p, 0.1, deg);
        // Never through the floor: a tail that would dip below it runs along it.
        p.1 = p.1.min(-0.05);
        *q = p;
    }
    (pts, away)
}

fn draw_tail(c: &mut Canvas, pts: &[P; 6]) {
    let tail = |s: &mut dyn Sink, g: f32| ribbon_into(s, pts, 0.07 + 2.0 * g);
    d::part(c, &tail, &pal::FUR, &pal::OUTLINE, 2);
    // Two darker rings and a white tip.
    let rings = |s: &mut dyn Sink, _: f32| {
        for k in [1usize, 3] {
            ribbon_into(
                s,
                &[
                    rig::lerp_p(pts[k], pts[k + 1], 0.3),
                    rig::lerp_p(pts[k], pts[k + 1], 0.55),
                ],
                0.066,
            );
        }
    };
    d::flat(c, &rings, pal::STRIPE);
    let tip = |s: &mut dyn Sink, _: f32| {
        ribbon_into(s, &[rig::lerp_p(pts[4], pts[5], 0.25), pts[5]], 0.066)
    };
    d::flat(c, &tip, pal::WHITE.base);
}

/// The ears: rounded triangles on top of the head, flicking with `top`.
fn ear_points(head: &Ball, b: &Body, i: usize) -> Option<[P; 3]> {
    let side = if i == 0 { -1.0 } else { 1.0 };
    let root = head.at(side * 40.0, 50.0, 0.92);
    if root.z < -0.6 {
        return None;
    }
    let w = 0.1 * (0.45 + 0.55 * rig::cos_deg(side * 40.0 + head.yaw).abs());
    let deg =
        180.0 - side * (24.0 + 6.0 * (1.0 - root.squeeze)) - b.top * 0.7 + side * b.top.abs() * 0.2;
    let tip = rig::reach(root.at, 0.24, deg);
    let along = rig::reach((0.0, 0.0), 1.0, deg + 90.0);
    let a = (root.at.0 - along.0 * w, root.at.1 - along.1 * w);
    let z = (root.at.0 + along.0 * w, root.at.1 + along.1 * w);
    Some([a, tip, z])
}

fn ears_into(s: &mut dyn Sink, ears: &[Option<[P; 3]>; 2], g: f32, inner: bool) {
    for [a, tip, z] in ears.iter().flatten() {
        if inner {
            let m = ((a.0 + z.0) * 0.5, (a.1 + z.1) * 0.5);
            let pts = [
                rig::lerp_p(*a, m, 0.4),
                rig::lerp_p(m, *tip, 0.72),
                rig::lerp_p(*z, m, 0.4),
            ];
            d::blob_into(s, &pts);
        } else {
            let tip = rig::lerp_p(*tip, ((a.0 + z.0) * 0.5, (a.1 + z.1) * 0.5), -g * 2.0);
            d::blob_grown_into(s, &[*a, tip, *z], g);
        }
    }
}

/// The scarf round the neck: its band over the front of the body and an end that swings.
fn draw_scarf(c: &mut Canvas, body: &Ball, b: &Body) {
    let ring: [P; 7] = std::array::from_fn(|k| {
        let lon = -100.0 + 200.0 * k as f32 / 6.0;
        let on = body.at(lon, 48.0, 1.06);
        on.at
    });
    let end_root = body.at(32.0, 42.0, 1.05);
    let end = |s: &mut dyn Sink, g: f32| {
        if end_root.z > -0.2 {
            let deg = 8.0 + b.tail * 0.3 - b.lean * 0.5;
            let tip = rig::reach(end_root.at, 0.17, deg);
            let mid = rig::reach(end_root.at, 0.09, deg - 6.0);
            d::limb_into(s, end_root.at, mid, tip, 0.07 + 2.0 * g, 0.085 + 2.0 * g);
        }
    };
    let band = |s: &mut dyn Sink, g: f32| ribbon_into(s, &ring, 0.1 + 2.0 * g);
    d::part(c, &end, &pal::SCARF, &pal::OUTLINE, 2);
    d::part(c, &band, &pal::SCARF, &pal::OUTLINE, 2);
    let stripe = |s: &mut dyn Sink, _: f32| ribbon_into(s, &ring, 0.022);
    d::flat(c, &stripe, pal::SCARF_STRIPE);
}

/// Arms (the ones `which`): fur from the shoulder, a white paw.
fn draw_arms(c: &mut Canvas, body: &Ball, b: &Body, which: &[usize]) {
    let arm = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = body.at(side * 74.0, 30.0, 0.92).at;
        let pts = kit::arm_points(sh, b.arm[i], b.bend[i] + 10.0, 0.13, 0.12, i);
        let deg = kit::forearm_deg(b.arm[i], b.bend[i] + 10.0, i);
        (pts, deg)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            kit::hose_into(s, arm(i).0, 0.07, 0.06, g);
        }
    };
    let paws = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, deg) = arm(i);
            kit::mitten_into(s, pts[2], deg, 0.06, b.spread[i], i, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        paws(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::FUR, 2);
    d::bands(c, &paws, &pal::WHITE, 2);
}

/// Legs from the hips to white paws on the ground.
fn draw_legs(c: &mut Canvas, body: &Ball, t: &Transform, b: &Body) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, body.at(side * 34.0, -48.0, 0.9).at);
        let f = b.foot[i];
        let foot = (side * 0.105 + f.0, f.1);
        let knee = ((hip.0 + foot.0) * 0.5 + side * 0.02, (hip.1 + foot.1) * 0.5);
        ([hip, knee, (foot.0, foot.1 - 0.03)], foot, side, f.1)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            kit::hose_into(s, leg(i).0, 0.075, 0.065, g);
        }
    };
    let paws = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (_, foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.13, 0.065, side, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        paws(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::FUR, 2);
    d::bands(c, &paws, &pal::WHITE, 2);
}

/// The head on the canvas (already in the head's frame): ears, the ball, stripes, muzzle,
/// nose, whiskers, mouth, cheeks, eyes.
fn draw_head(c: &mut Canvas, head: &Ball, b: &Body, pose: &Pose) {
    let ears = [ear_points(head, b, 0), ear_points(head, b, 1)];
    d::part(
        c,
        &|s, g| ears_into(s, &ears, g, false),
        &pal::FUR,
        &pal::OUTLINE,
        2,
    );
    d::flat(c, &|s, g| ears_into(s, &ears, g, true), pal::PINK);
    let skull =
        |s: &mut dyn Sink, g: f32| ellipse_into(s, head.center, head.rx + g, head.ry + g, 0.0);
    d::part(c, &skull, &pal::FUR, &pal::OUTLINE, 3);
    let f = face::face_of(pose.mood);
    let lat = b.nod * 0.5;
    // Tabby stripes on the forehead.
    let stripes = |s: &mut dyn Sink, _: f32| {
        for (lon, top) in [(-15.0, 74.0), (0.0, 80.0), (15.0, 74.0)] {
            let a = head.at(lon, top, 1.0);
            let z = head.at(lon, 60.0 + lat * 0.5, 1.0);
            if a.z > 0.15 && z.z > 0.15 {
                ribbon_into(s, &[a.at, z.at], 0.026 * z.squeeze.max(0.4));
            }
        }
    };
    d::flat(c, &stripes, pal::STRIPE);
    // From behind: tabby stripes down the back of the head.
    let back = |s: &mut dyn Sink, _: f32| {
        for lon in [160.0, 180.0, 200.0] {
            let a = head.at(lon, 60.0, 1.0);
            let z = head.at(lon, 10.0, 1.0);
            if a.z > 0.15 && z.z > 0.15 {
                ribbon_into(
                    s,
                    &[a.at, head.at(lon, 35.0, 1.0).at, z.at],
                    0.034 * z.squeeze.max(0.4),
                );
            }
        }
    };
    d::flat(c, &back, pal::STRIPE);
    face::cheeks(c, head, &FACE, &f, b, pose);
    // The muzzle: two white lobes.
    let lobes = [
        head.at(-11.0, -24.0 + lat, 1.0),
        head.at(11.0, -24.0 + lat, 1.0),
    ];
    if lobes.iter().any(|o| o.z > 0.1) {
        let muzzle = |s: &mut dyn Sink, g: f32| {
            for o in lobes.iter().filter(|o| o.z > 0.0) {
                ellipse_into(s, o.at, 0.075 * (0.4 + 0.6 * o.squeeze) + g, 0.06 + g, 0.0);
            }
        };
        d::part(c, &muzzle, &pal::WHITE, &pal::OUTLINE.thin(0.6), 1);
        // Whiskers, three a side.
        let whiskers = |s: &mut dyn Sink, _: f32| {
            for (k, o) in lobes.iter().enumerate() {
                if o.z <= 0.2 {
                    continue;
                }
                let side = if k == 0 { -1.0 } else { 1.0 };
                for (j, dy) in [-0.022f32, 0.0, 0.022].iter().enumerate() {
                    let a = (o.at.0 + side * 0.055 * o.squeeze, o.at.1 + dy);
                    let deg =
                        side * (90.0 + (j as f32 - 1.0) * 14.0) + side * 4.0 * b.tilt.signum();
                    let z = rig::reach(a, 0.13 * o.squeeze.max(0.3), deg);
                    ribbon_into(s, &[a, rig::lerp_p(a, z, 0.5), z], 0.009);
                }
            }
        };
        d::fixed(c, &whiskers, d::mix(pal::INK, pal::FUR.shade, 0.3));
    }
    let nose = head.at(0.0, -10.0 + lat, 1.02);
    if nose.z > 0.1 {
        let w = 0.032 * (0.5 + 0.5 * nose.squeeze);
        let tri = [
            (nose.at.0 - w, nose.at.1 - 0.012),
            (nose.at.0 + w, nose.at.1 - 0.012),
            (nose.at.0, nose.at.1 + 0.02),
        ];
        d::part(
            c,
            &|s, g| d::blob_grown_into(s, &tri, g),
            &d::Look {
                base: pal::NOSE,
                shade: pal::NOSE,
                light: pal::PINK,
                rim: [0, 0, 0, 0],
            },
            &pal::OUTLINE.thin(0.6),
            1,
        );
    }
    let m = head.at(0.0, -27.0 + lat, 1.0);
    face::mouth(c, &m, &f, b.mouth, &MOUTH);
    face::eyes(c, head, &FACE, &f, b, pose);
}

pub(super) fn draw(cat: &Cat, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = kit::torso(b, ANCHOR_Y);
    let tt = base.pre_concat(t);
    let body = ball(BODY, b.yaw * 0.6);
    let head = ball(HEAD, b.yaw);
    // The head bobs a touch after the body and tilts on its own.
    let head_local = t.pre_concat(crate::rig::rotation_at(b.tilt, HEAD.0 .0, HEAD.0 .1 + 0.1));
    let head_t = base.pre_concat(head_local);
    kit::shadow(c, ground, pose, b, 0.3, pal::SHADOW);
    let (tail, away) = tail_points(&body, b);
    let arm_z = |i: usize| body.at(if i == 0 { -74.0 } else { 74.0 }, 30.0, 1.0).z;
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
    kit::with(c, tt, |c| {
        if !away {
            draw_tail(c, &tail);
        }
        draw_arms(c, &body, b, &far[..nf]);
    });
    draw_legs(c, &body, &t, b);
    kit::with(c, tt, |c| {
        let torso =
            |s: &mut dyn Sink, g: f32| ellipse_into(s, body.center, body.rx + g, body.ry + g, 0.0);
        d::part(c, &torso, &pal::FUR, &pal::OUTLINE, 2);
        let chest = body.at(0.0, -10.0, 1.0);
        if chest.z > 0.1 {
            let w = 0.12 * (0.3 + 0.7 * chest.squeeze);
            d::flat(
                c,
                &|s, _| ellipse_into(s, (chest.at.0 * 0.85, chest.at.1 + 0.02), w, 0.13, 0.0),
                pal::WHITE.base,
            );
        }
        draw_scarf(c, &body, b);
        draw_arms(c, &body, b, &near[..nn]);
    });
    kit::with(c, head_t, |c| draw_head(c, &head, b, pose));
    if away {
        kit::with(c, tt, |c| draw_tail(c, &tail));
    }
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let m = kit::map(&head_local, head.at(0.0, -20.0, 1.0).at);
                (m.0 + side * 0.34, m.1 - 0.16)
            }
            Anchor::Top => kit::map(&head_local, head.at(0.0, 90.0, 1.5).at),
            Anchor::Hands | Anchor::Hand(_) => {
                let hand = |i: usize| {
                    let side = if i == 0 { -1.0 } else { 1.0 };
                    let sh = body.at(side * 74.0, 30.0, 0.92).at;
                    kit::map(
                        &t,
                        kit::arm_points(sh, b.arm[i], b.bend[i] + 10.0, 0.13, 0.17, i)[2],
                    )
                };
                match at {
                    Anchor::Hand(i) => hand(i.min(1)),
                    _ => rig::lerp_p(hand(0), hand(1), 0.5),
                }
            }
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let m = kit::body_at(pose, &cat.style, 0.0, 40.0);
        let mt = kit::torso(&m, ANCHOR_Y);
        let mb = ball(BODY, m.yaw * 0.6);
        let hand = |i: usize| {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let sh = mb.at(side * 74.0, 30.0, 0.92).at;
            kit::map(
                &mt,
                kit::arm_points(sh, m.arm[i], m.bend[i] + 10.0, 0.13, 0.17, i)[2],
            )
        };
        rig::lerp_p(hand(0), hand(1), 0.5)
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
