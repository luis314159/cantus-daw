//! Drawing the lamb from a [`Body`]: shadow, tail puff (behind unless she turns away), a
//! far front leg, the hind legs, the wool, the collar and its bell, the near front legs,
//! the ears, the face under its tuft and bow, the beat's extras.

use super::{pal, Lamb};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, ribbon_into, Sink};
use crate::rig::face::{self, FaceStyle, MouthStyle};
use crate::rig::{self, Ball, P};
use tiny_skia::Transform;

const ANCHOR_Y: f32 = -0.1;
const BODY: (P, f32, f32) = ((0.0, -0.36), 0.3, 0.27);
const HEAD: (P, f32, f32) = ((0.0, -0.78), 0.215, 0.225);

const FACE: FaceStyle = FaceStyle {
    eye_lon: 32.0,
    eye_lat: 10.0,
    eye_rx: 0.068,
    eye_ry: 0.082,
    white: pal::EYE_WHITE,
    pupil: pal::PUPIL,
    iris: pal::IRIS,
    lid: pal::FACE.base,
    ink: pal::OUTLINE,
    cheek_lon: 50.0,
    cheek_lat: -20.0,
    cheek_rx: 0.05,
    cheek: pal::CHEEK,
    skin: pal::FACE.base,
};

const MOUTH: MouthStyle = MouthStyle {
    width: 0.04,
    inside: pal::MOUTH,
    tongue: pal::TONGUE,
    ink: d::Ink {
        color: pal::LIP,
        width: 0.02,
    },
};

fn ball(c: (P, f32, f32), yaw: f32) -> Ball {
    Ball {
        center: c.0,
        rx: c.1,
        ry: c.2,
        yaw,
    }
}

/// A cloud of wool: an ellipse whose edge rises and falls in soft bumps, the bumps
/// turning with the ball (`turn` degrees).
fn cloud_into(s: &mut dyn Sink, c: P, rx: f32, ry: f32, bumps: usize, turn: f32, g: f32) {
    let n = (bumps * 2).min(24);
    let mut pts = [(0.0, 0.0); 24];
    for (k, p) in pts.iter_mut().enumerate().take(n) {
        let a = rig::TAU * k as f32 / n as f32 + rig::rad(turn);
        let r = if k % 2 == 0 { 1.0 } else { 0.84 };
        let (sn, cs) = cantus_dsp::math::sin_cos(a);
        *p = (c.0 + (rx * r + g) * cs, c.1 + (ry * r + g) * sn);
    }
    d::blob_into(s, &pts[..n]);
}

/// The ears: floppy ovals at the sides of the head, flopping late with `top`.
fn ears_into(s: &mut dyn Sink, head: &Ball, b: &Body, g: f32, inner: bool) {
    for side in [-1.0f32, 1.0] {
        let root = head.at(side * 84.0, 22.0, 0.95);
        if root.z < -0.5 {
            continue;
        }
        let deg = side * (104.0 + b.top * 0.8 * side) + 8.0 * b.squash.signum() * side;
        let len = 0.1 * (0.6 + 0.4 * (1.0 - root.squeeze));
        let c = rig::reach(root.at, len, deg);
        let tilt = 90.0 - deg;
        if inner {
            let c2 = rig::reach(root.at, len * 1.05, deg);
            ellipse_into(s, c2, len * 0.62, 0.024, tilt);
        } else {
            ellipse_into(s, c, len + 0.01 + g, 0.046 + g, tilt);
        }
    }
}

/// Front legs (the ones `which`): cocoa, ending in hooves.
fn draw_front_legs(c: &mut Canvas, body: &Ball, b: &Body, which: &[usize]) {
    let arm = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = body.at(side * 64.0, 6.0, 0.88).at;
        let pts = kit::arm_points(sh, b.arm[i], b.bend[i] * 0.7, 0.13, 0.11, i);
        let deg = kit::forearm_deg(b.arm[i], b.bend[i] * 0.7, i);
        (pts, deg)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            kit::hose_into(s, arm(i).0, 0.068, 0.06, g);
        }
    };
    let hooves = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, deg) = arm(i);
            let tip = rig::reach(pts[2], 0.02, deg);
            ellipse_into(s, tip, 0.034 + g, 0.03 + g, -deg);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        hooves(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::FACE, 2);
    d::flat(c, &hooves, pal::HOOF);
}

/// Hind legs from under the wool to hooves on the ground.
fn draw_legs(c: &mut Canvas, body: &Ball, t: &Transform, b: &Body) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, body.at(side * 30.0, -62.0, 0.85).at);
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
            kit::hose_into(s, leg(i).0, 0.06, 0.055, g);
        }
    };
    let hooves = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (_, foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.085, 0.055, side * 0.4, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        hooves(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::FACE, 2);
    d::flat(c, &hooves, pal::HOOF);
}

/// The collar over the wool and its gold bell, swinging.
fn draw_bell(c: &mut Canvas, body: &Ball, b: &Body) {
    let ring: [P; 7] = std::array::from_fn(|k| {
        let lon = -70.0 + 140.0 * k as f32 / 6.0;
        body.at(lon, 52.0, 1.02).at
    });
    let front = body.at(0.0, 52.0, 1.02);
    let collar = |s: &mut dyn Sink, g: f32| ribbon_into(s, &ring, 0.04 + 2.0 * g);
    d::part(c, &collar, &pal::BOW, &pal::OUTLINE.thin(0.8), 1);
    if front.z > 0.2 {
        let swing = -b.tail * 0.6 - b.lean;
        let at = rig::reach(front.at, 0.05, swing);
        let bell = |s: &mut dyn Sink, g: f32| {
            ellipse_into(s, at, 0.042 + g, 0.04 + g, 0.0);
            ellipse_into(s, (at.0, at.1 + 0.03), 0.05 + g, 0.016 + g, 0.0);
        };
        d::part(c, &bell, &pal::BELL, &pal::OUTLINE.thin(0.8), 3);
        d::fixed(
            c,
            &|s, _| ellipse_into(s, (at.0, at.1 + 0.025), 0.01, 0.01, 0.0),
            pal::INK,
        );
    }
}

/// The head on the canvas (in the head's frame): ears, the face, the tuft and its bow,
/// cheeks, eyes, nose and mouth.
fn draw_head(c: &mut Canvas, head: &Ball, b: &Body, pose: &Pose) {
    d::part(
        c,
        &|s, g| ears_into(s, head, b, g, false),
        &pal::FACE,
        &pal::OUTLINE,
        2,
    );
    d::flat(c, &|s, g| ears_into(s, head, b, g, true), pal::PINK);
    let face_shape =
        |s: &mut dyn Sink, g: f32| ellipse_into(s, head.center, head.rx + g, head.ry + g, 0.0);
    d::part(c, &face_shape, &pal::FACE, &pal::OUTLINE, 3);
    // From behind, the head is wool too.
    let nape = head.at(180.0, 15.0, 1.0);
    if nape.z > 0.1 {
        let w = head.rx * (0.35 + 0.6 * nape.squeeze);
        let at = (nape.at.0 * 0.8, nape.at.1);
        d::flat(
            c,
            &|s, g| cloud_into(s, at, w, head.ry * 0.85, 5, 0.0, g),
            pal::WOOL.base,
        );
    }
    let f = face::face_of(pose.mood);
    let lat = b.nod * 0.5;
    face::cheeks(c, head, &FACE, &f, b, pose);
    face::eyes(c, head, &FACE, &f, b, pose);
    let nose = head.at(0.0, -28.0 + lat, 1.0);
    if nose.z > 0.1 {
        let w = 0.03 * (0.5 + 0.5 * nose.squeeze);
        let n = nose.at;
        d::fixed(
            c,
            &|s, _| {
                d::blob_into(
                    s,
                    &[
                        (n.0 - w, n.1 - 0.01),
                        (n.0 + w, n.1 - 0.01),
                        (n.0, n.1 + 0.018),
                    ],
                )
            },
            [62, 34, 40, 255],
        );
    }
    let m = head.at(0.0, -46.0 + lat, 1.0);
    face::mouth(c, &m, &f, b.mouth, &MOUTH);
    // The tuft of wool on top, bouncing late, and its bow.
    let tuft_at = head.at(0.0, 62.0, 1.0).at;
    let tuft_c = (tuft_at.0 + b.top * 0.002, tuft_at.1 - 0.03);
    let tuft = |s: &mut dyn Sink, g: f32| {
        for (dx, dy, r) in [
            (-0.085, 0.02, 0.07),
            (0.0, -0.03, 0.085),
            (0.085, 0.02, 0.07),
            (0.0, 0.04, 0.06),
        ] {
            ellipse_into(s, (tuft_c.0 + dx, tuft_c.1 + dy), r + g, r * 0.92 + g, 0.0);
        }
    };
    d::part(c, &tuft, &pal::WOOL, &pal::OUTLINE, 2);
    let bow_c = (tuft_c.0 + 0.1, tuft_c.1 - 0.02);
    let bow = |s: &mut dyn Sink, g: f32| {
        ellipse_into(
            s,
            (bow_c.0 - 0.045, bow_c.1 - 0.012),
            0.05 + g,
            0.032 + g,
            -25.0,
        );
        ellipse_into(
            s,
            (bow_c.0 + 0.045, bow_c.1 + 0.012),
            0.05 + g,
            0.032 + g,
            -25.0,
        );
    };
    d::part(c, &bow, &pal::BOW, &pal::OUTLINE.thin(0.8), 2);
    let knot = |s: &mut dyn Sink, g: f32| ellipse_into(s, bow_c, 0.022 + g, 0.022 + g, 0.0);
    d::part(c, &knot, &pal::BOW, &pal::OUTLINE.thin(0.8), 1);
}

pub(super) fn draw(lamb: &Lamb, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = kit::torso(b, ANCHOR_Y);
    let tt = base.pre_concat(t);
    let body = ball(BODY, b.yaw * 0.6);
    let head = ball(HEAD, b.yaw);
    let head_local = t.pre_concat(crate::rig::rotation_at(b.tilt, HEAD.0 .0, HEAD.0 .1 + 0.12));
    let head_t = base.pre_concat(head_local);
    kit::shadow(c, ground, pose, b, 0.34, pal::SHADOW);
    let puff = body.at(165.0, -15.0, 1.0);
    let puff_at = (puff.at.0 + b.tail * 0.0015, puff.at.1);
    let tail = |s: &mut dyn Sink, g: f32| cloud_into(s, puff_at, 0.075, 0.065, 4, b.tail, g);
    let arm_z = |i: usize| body.at(if i == 0 { -64.0 } else { 64.0 }, 6.0, 1.0).z;
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
        if puff.z <= 0.0 {
            d::part(c, &tail, &pal::WOOL, &pal::OUTLINE, 2);
        }
        draw_front_legs(c, &body, b, &far[..nf]);
    });
    draw_legs(c, &body, &t, b);
    kit::with(c, tt, |c| {
        let wool = |s: &mut dyn Sink, g: f32| {
            cloud_into(s, body.center, body.rx, body.ry, 9, body.yaw * 0.5, g)
        };
        d::part(c, &wool, &pal::WOOL, &pal::OUTLINE, 3);
        draw_bell(c, &body, b);
        draw_front_legs(c, &body, b, &near[..nn]);
        if puff.z > 0.0 {
            d::part(c, &tail, &pal::WOOL, &pal::OUTLINE, 2);
        }
    });
    kit::with(c, head_t, |c| draw_head(c, &head, b, pose));
    let hand = |m: &Body, mt: &Transform, i: usize| {
        let mb = ball(BODY, m.yaw * 0.6);
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = mb.at(side * 64.0, 6.0, 0.88).at;
        kit::map(
            mt,
            kit::arm_points(sh, m.arm[i], m.bend[i] * 0.7, 0.13, 0.13, i)[2],
        )
    };
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let m = kit::map(&head_local, head.at(0.0, -20.0, 1.0).at);
                (m.0 + side * 0.32, m.1 - 0.16)
            }
            Anchor::Top => kit::map(&head_local, head.at(0.0, 90.0, 1.7).at),
            Anchor::Hand(i) => hand(b, &t, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &t, 0), hand(b, &t, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let m = kit::body_at(pose, &lamb.style, 0.0, 40.0);
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
