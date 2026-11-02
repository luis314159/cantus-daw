//! Drawing the note from a [`Body`]: shadow, the stem and its flag (behind the head), a
//! far arm, the legs and gold shoes, the note head with its face and bow tie, the near
//! arms in white gloves, the beat's extras.

use super::{pal, Note};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, limb_into, ribbon_into, Sink};
use crate::rig::face::{self, FaceStyle, MouthStyle};
use crate::rig::{self, Ball, P};
use tiny_skia::Transform;

const ANCHOR_Y: f32 = -0.18;
const HEAD: (P, f32, f32) = ((0.0, -0.46), 0.31, 0.25);
/// A note head leans back a little, always.
const LEAN: f32 = -14.0;

const FACE: FaceStyle = FaceStyle {
    eye_lon: 26.0,
    eye_lat: 16.0,
    eye_rx: 0.078,
    eye_ry: 0.094,
    white: pal::EYE_WHITE,
    pupil: pal::PUPIL,
    iris: pal::IRIS,
    lid: pal::PLUM.base,
    ink: pal::OUTLINE,
    cheek_lon: 46.0,
    cheek_lat: -12.0,
    cheek_rx: 0.06,
    cheek: pal::CHEEK,
    skin: pal::PLUM.base,
};

const MOUTH: MouthStyle = MouthStyle {
    width: 0.062,
    inside: pal::MOUTH,
    tongue: pal::TONGUE,
    ink: d::Ink {
        color: pal::LIP,
        width: 0.02,
    },
};

fn head_ball(yaw: f32) -> Ball {
    Ball {
        center: HEAD.0,
        rx: HEAD.1,
        ry: HEAD.2,
        yaw,
    }
}

/// The stem from the right side of the head up, and its flag curling out, waving late.
fn draw_stem(c: &mut Canvas, head: &Ball, b: &Body) {
    let root = head.at(80.0, 6.0, 0.9).at;
    let top = (root.0 + 0.02, -1.2);
    let side = if rig::sin_deg(80.0 + head.yaw) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let wave = b.top * 0.004;
    let f0 = (top.0, top.1 + 0.02);
    let f1 = (top.0 + side * (0.28 + wave), top.1 + 0.15);
    let f2 = (top.0 + side * (0.17 + wave * 0.5), top.1 + 0.4);
    let stem = |s: &mut dyn Sink, g: f32| {
        ribbon_into(s, &[root, top], 0.08 + 2.0 * g);
        limb_into(s, f0, f1, f2, 0.13 + 2.0 * g, 0.04 + 2.0 * g);
    };
    d::part(c, &stem, &pal::PLUM, &pal::OUTLINE, 2);
    let trim = [
        rig::lerp_p(f0, f1, 0.35),
        rig::lerp_p(f1, f2, 0.2),
        rig::lerp_p(f1, f2, 0.75),
    ];
    d::flat(c, &|s, _| ribbon_into(s, &trim, 0.018), pal::GOLD.base);
}

/// Arms (the ones `which`): plum from the sides of the head, white gloves.
fn draw_arms(c: &mut Canvas, head: &Ball, b: &Body, which: &[usize]) {
    let arm = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = head.at(side * 84.0, -18.0, 0.95).at;
        let pts = kit::arm_points(sh, b.arm[i], b.bend[i], 0.14, 0.13, i);
        (pts, kit::forearm_deg(b.arm[i], b.bend[i], i))
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            kit::hose_into(s, arm(i).0, 0.05, 0.045, g);
        }
    };
    let gloves = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, deg) = arm(i);
            kit::mitten_into(s, pts[2], deg, 0.07, b.spread[i], i, g);
            // The cuff.
            ellipse_into(s, pts[2], 0.04 + g, 0.04 + g, 0.0);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        gloves(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::PLUM, 2);
    d::bands(c, &gloves, &pal::GLOVE, 2);
}

/// Legs from under the head to gold shoes.
fn draw_legs(c: &mut Canvas, head: &Ball, t: &Transform, b: &Body) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, head.at(side * 30.0, -70.0, 0.9).at);
        let f = b.foot[i];
        let foot = (side * 0.11 + f.0, f.1);
        let knee = ((hip.0 + foot.0) * 0.5 + side * 0.02, (hip.1 + foot.1) * 0.5);
        ([hip, knee, (foot.0, foot.1 - 0.03)], foot, side, f.1)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            kit::hose_into(s, leg(i).0, 0.05, 0.045, g);
        }
    };
    let shoes = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (_, foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.15, 0.07, side, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        shoes(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::PLUM, 2);
    d::bands(c, &shoes, &pal::GOLD, 3);
}

pub(super) fn draw(note: &Note, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = kit::torso(b, ANCHOR_Y);
    let head = head_ball(b.yaw);
    let lean = crate::rig::rotation_at(LEAN + b.tilt * 0.5, HEAD.0 .0, HEAD.0 .1);
    let tl = t.pre_concat(lean);
    let tt = base.pre_concat(tl);
    kit::shadow(c, ground, pose, b, 0.32, pal::SHADOW);
    let arm_z = |i: usize| head.at(if i == 0 { -84.0 } else { 84.0 }, -18.0, 1.0).z;
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
        draw_stem(c, &head, b);
        draw_arms(c, &head, b, &far[..nf]);
    });
    draw_legs(c, &head, &tl, b);
    kit::with(c, tt, |c| {
        let shape =
            |s: &mut dyn Sink, g: f32| ellipse_into(s, head.center, head.rx + g, head.ry + g, 0.0);
        d::part(c, &shape, &pal::PLUM, &pal::OUTLINE, 3);
        d::rim_arc(
            c,
            head.center,
            head.rx,
            head.ry,
            -10.0,
            70.0,
            &pal::PLUM,
            pal::OUTLINE.width,
        );
        // The gloss of a fresh ink note: a soft highlight up on the left.
        let gloss = (
            head.center.0 - head.rx * 0.5,
            head.center.1 - head.ry * 0.52,
        );
        d::flat(
            c,
            &|s, _| ellipse_into(s, gloss, head.rx * 0.28, head.ry * 0.14, -24.0),
            d::mix(pal::PLUM.light, [255, 255, 255, 255], 0.45),
        );
        let f = face::face_of(pose.mood);
        let face_t =
            c.transform
                .pre_concat(crate::rig::rotation_at(-LEAN * 0.6, HEAD.0 .0, HEAD.0 .1));
        kit::with(c, face_t, |c| {
            face::cheeks(c, &head, &FACE, &f, b, pose);
            face::eyes(c, &head, &FACE, &f, b, pose);
            let m = head.at(0.0, -22.0 + b.nod * 0.5, 1.0);
            face::mouth(c, &m, &f, b.mouth, &MOUTH);
            // The gold bow tie under the smile.
            let tie = head.at(0.0, -62.0, 1.0);
            if tie.z > 0.2 {
                let w = 0.06 * (0.4 + 0.6 * tie.squeeze);
                let a = tie.at;
                let bow = |s: &mut dyn Sink, g: f32| {
                    d::blob_grown_into(
                        s,
                        &[(a.0, a.1), (a.0 - w, a.1 - 0.035), (a.0 - w, a.1 + 0.035)],
                        g,
                    );
                    d::blob_grown_into(
                        s,
                        &[(a.0, a.1), (a.0 + w, a.1 + 0.035), (a.0 + w, a.1 - 0.035)],
                        g,
                    );
                    ellipse_into(s, a, 0.018 + g, 0.018 + g, 0.0);
                };
                d::part(c, &bow, &pal::GOLD, &pal::OUTLINE.thin(0.8), 2);
            }
        });
        draw_arms(c, &head, b, &near[..nn]);
    });
    let hand = |m: &Body, mt: &Transform, i: usize| {
        let mh = head_ball(m.yaw);
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = mh.at(side * 84.0, -18.0, 0.95).at;
        kit::map(
            mt,
            kit::arm_points(sh, m.arm[i], m.bend[i], 0.14, 0.19, i)[2],
        )
    };
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let m = kit::map(&tl, head.at(0.0, -10.0, 1.0).at);
                (m.0 + side * 0.36, m.1 - 0.16)
            }
            Anchor::Top => kit::map(&tl, (0.0, -1.05)),
            Anchor::Hand(i) => hand(b, &tl, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &tl, 0), hand(b, &tl, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let m = kit::body_at(pose, &note.style, 0.0, 40.0);
        let mt = kit::torso(&m, ANCHOR_Y).pre_concat(crate::rig::rotation_at(
            LEAN + m.tilt * 0.5,
            HEAD.0 .0,
            HEAD.0 .1,
        ));
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
