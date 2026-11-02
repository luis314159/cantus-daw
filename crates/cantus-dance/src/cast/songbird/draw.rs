//! Drawing the songbird from a [`Body`]: back to front — its shadow, what is behind the
//! ball (the tail, a far wing, a far ear cup), its legs, the ball with its belly and back,
//! the face, the headphones and crest, the near wings, and what the beat adds.

use super::{face, pal, Songbird};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{self, Anchor, Body, Fx};
use crate::rig::draw::{self as d, ellipse_into, limb_into, ribbon_into, Sink};
use crate::rig::{self, fx, sin_deg, Ball, P};
use cantus_core::inst::dancer::Mood;
use tiny_skia::Transform;

/// The ball: its centre and radii, standing.
pub(super) const CENTER: P = (0.0, -0.53);
pub(super) const RX: f32 = 0.37;
pub(super) const RY: f32 = 0.39;
/// Where it squashes from: the bottom of the ball.
const ANCHOR_Y: f32 = -0.14;
/// The tip of a wing (its middle primaries), in the wing's own units.
const WING_TIP: P = (-0.035, 0.33);
/// Where the wings join the ball (latitude).
const SHOULDER_LAT: f32 = -18.0;

/// The body's transform in the character's units: offset, lean about the feet, squash
/// from the bottom of the ball.
pub(super) fn torso(b: &Body) -> Transform {
    let (sx, sy) = rig::squash(b.squash);
    Transform::from_translate(b.x, b.y)
        .pre_concat(crate::rig::rotation(b.lean))
        .pre_translate(0.0, ANCHOR_Y)
        .pre_scale(sx, sy)
        .pre_translate(0.0, -ANCHOR_Y)
}

pub(super) fn map(t: &Transform, p: P) -> P {
    (
        t.sx * p.0 + t.kx * p.1 + t.tx,
        t.ky * p.0 + t.sy * p.1 + t.ty,
    )
}

fn with(c: &mut Canvas, t: Transform, f: impl FnOnce(&mut Canvas)) {
    let saved = c.transform;
    c.transform = t;
    f(c);
    c.transform = saved;
}

/// A wing's transform (in the torso's units): from its own (pointing down, its outer side
/// to the left) to its shoulder, turned by the arm's angle; the right wing mirrored.
fn wing_transform(ball: &Ball, i: usize, b: &Body) -> Transform {
    let lon = if i == 0 { -80.0 } else { 80.0 };
    let sh = ball.at(lon, SHOULDER_LAT, 0.96).at;
    if i == 0 {
        Transform::from_translate(sh.0, sh.1).pre_concat(crate::rig::rotation(b.arm[0]))
    } else {
        Transform::from_translate(sh.0, sh.1)
            .pre_concat(crate::rig::rotation(-b.arm[1]))
            .pre_scale(-1.0, 1.0)
    }
}

/// The coverts: the arm of the wing from the shoulder to the wrist, its outer side the
/// leading edge.
fn coverts(s: &mut dyn Sink, g: f32) {
    d::blob_grown_into(
        s,
        &[
            (0.042, -0.05),
            (-0.05, -0.056),
            (-0.1, 0.02),
            (-0.106, 0.11),
            (-0.075, 0.17),
            (0.0, 0.18),
            (0.05, 0.1),
            (0.056, 0.02),
        ],
        g,
    );
}

/// The four primaries from the wrist, the outermost longest (it carries the leading edge
/// on to the tip), fanned by `spread` and curled by `bend`.
fn primaries(s: &mut dyn Sink, g: f32, bend: f32, spread: f32) {
    let sp = 4.0 + 11.0 * spread.clamp(0.0, 1.0);
    for k in 0..4 {
        let kf = k as f32;
        let root = (-0.066 + 0.03 * kf, 0.1);
        let deg = -10.0 - 1.5 * sp + sp * kf - bend;
        let len = [0.205, 0.185, 0.162, 0.138][k];
        let tip = rig::reach(root, len, deg);
        let mid = rig::reach(root, len * 0.55, deg + 4.0);
        limb_into(s, root, mid, tip, 0.072 + 2.0 * g, 0.062 + 2.0 * g);
    }
}

/// The underside: the inner half of the coverts, lighter.
fn underside(s: &mut dyn Sink, _: f32) {
    d::blob_into(
        s,
        &[
            (0.03, -0.01),
            (0.046, 0.08),
            (0.0, 0.155),
            (-0.03, 0.12),
            (-0.012, 0.03),
        ],
    );
}

/// The wings `which` (by index), drawn as one part: each layer is one fill covering all of
/// them, every wing mapped by its own placement.
fn draw_wings(c: &mut Canvas, ball: &Ball, b: &Body, which: &[usize]) {
    if which.is_empty() {
        return;
    }
    let place = |i: usize| wing_transform(ball, i, b);
    // Each wing in its own placement, `local` applied first (in the wing's own units).
    let each =
        |s: &mut dyn Sink, g: f32, local: Transform, part: &dyn Fn(&mut dyn Sink, f32, usize)| {
            for &i in which {
                let mut m = d::Mapped {
                    inner: &mut *s,
                    t: place(i).pre_concat(local),
                };
                part(&mut m, g, i);
            }
        };
    let id = Transform::identity();
    let all = |s: &mut dyn Sink, g: f32| {
        each(s, g, id, &|m, g, i| {
            coverts(m, g);
            primaries(m, g, b.bend[i], b.spread[i]);
        })
    };
    d::outline(c, &all, &pal::OUTLINE);
    let quills = d::mix(pal::WING.base, pal::WING.shade, 0.5);
    d::flat(
        c,
        &|s, g| each(s, g, id, &|m, g, i| primaries(m, g, b.bend[i], b.spread[i])),
        quills,
    );
    // Thin dark lines between the primaries.
    let lines = |m: &mut dyn Sink, _: f32, i: usize| {
        let sp = 4.0 + 11.0 * b.spread[i].clamp(0.0, 1.0);
        for k in 1..4 {
            let kf = k as f32;
            let root = (-0.081 + 0.03 * kf, 0.12);
            let deg = -10.0 - 1.5 * sp + sp * (kf - 0.5) - b.bend[i];
            let len = [0.205, 0.185, 0.162, 0.138][k];
            let a = rig::reach(root, len * 0.45, deg);
            let z = rig::reach(root, len * 0.95, deg);
            ribbon_into(m, &[a, z], 0.009);
        }
    };
    d::fixed(
        c,
        &|s, g| each(s, g, id, &lines),
        d::mix(pal::INK, pal::WING.shade, 0.4),
    );
    // The coverts: their shade, then the base drawn smaller toward the shoulder.
    d::flat(
        c,
        &|s, g| each(s, g, id, &|m, g, _| coverts(m, g)),
        pal::WING.shade,
    );
    let toward = Transform::from_translate(-0.01, -0.02)
        .pre_scale(0.84, 0.84)
        .pre_translate(0.01, 0.02);
    d::flat(
        c,
        &|s, g| each(s, g, toward, &|m, g, _| coverts(m, g)),
        pal::WING.base,
    );
    // The lighter underside and a lit line along the leading edge: one fill.
    let light = |m: &mut dyn Sink, g: f32, _: usize| {
        underside(m, g);
        ribbon_into(
            m,
            &[
                (-0.03, -0.035),
                (-0.075, -0.005),
                (-0.094, 0.06),
                (-0.094, 0.12),
            ],
            0.014,
        );
    };
    d::flat(
        c,
        &|s, g| each(s, g, id, &light),
        d::mix(pal::WING.light, pal::BELLY.base, 0.35),
    );
}

/// The tail: three paddle feathers fanned from the back of the ball.
fn draw_tail(c: &mut Canvas, ball: &Ball, b: &Body) {
    let on = ball.at(150.0, -40.0, 0.82);
    let root = on.at;
    let dx = sin_deg(150.0 + ball.yaw);
    // Behind, it fans up past the side of the ball; turned toward us, it hangs down.
    let toward = rig::smooth(on.z * 2.5);
    let dir = rig::lerp(180.0 - 110.0 * dx, 15.0 * dx.signum() + 25.0 * dx, toward) + b.tail;
    let feathers: [(P, P, P, f32); 3] = std::array::from_fn(|k| {
        let (off, len) = [(-17.0, 0.29), (0.0, 0.34), (17.0, 0.28)][k];
        let deg = dir + off * (0.6 + 0.4 * dx.abs());
        let tip = rig::reach(root, len, deg);
        let ctrl = rig::reach(root, len * 0.5, deg + 8.0 * dx.signum());
        (root, ctrl, tip, 0.092 - 0.006 * k as f32)
    });
    let all = |s: &mut dyn Sink, g: f32| {
        for (a, m, z, w) in feathers {
            limb_into(s, a, m, z, 0.055 + 2.0 * g, w + 2.0 * g);
        }
    };
    d::part(c, &all, &pal::WING, &pal::OUTLINE, 1);
    let tips = |s: &mut dyn Sink, _: f32| {
        for (a, _, z, _) in feathers {
            let u = rig::lerp_p(a, z, 0.72);
            let m = rig::lerp_p(a, z, 0.86);
            limb_into(s, u, m, z, 0.05, 0.07);
        }
    };
    d::flat(c, &tips, pal::TIP);
}

/// The crest: three feathers from the top of the head, curling back.
fn draw_crest(c: &mut Canvas, ball: &Ball, b: &Body, mood: Mood) {
    let root = ball.at(0.0, 80.0, 0.96).at;
    let lift = match mood {
        Mood::Surprised | Mood::Excited => 1.15,
        Mood::Sleepy => 0.8,
        _ => 1.0,
    };
    let droop = if mood == Mood::Sleepy { 22.0 } else { 0.0 };
    let feathers: [(P, P, P); 3] = std::array::from_fn(|k| {
        let (off, len) = [(30.0, 0.17), (2.0, 0.25), (-30.0, 0.16)][k];
        let deg = 180.0 - b.top + off * lift - droop * (off / 30.0 + 1.0) * 0.6;
        let tip = rig::reach(root, len * lift, deg);
        let ctrl = rig::reach(root, len * 0.62 * lift, deg - 24.0);
        (root, ctrl, tip)
    });
    let all = |s: &mut dyn Sink, g: f32| {
        for (a, m, z) in feathers {
            limb_into(s, a, m, z, 0.07 + 2.0 * g, 0.03 + 2.0 * g);
        }
    };
    d::part(c, &all, &pal::BODY, &pal::OUTLINE, 1);
    let tips = |s: &mut dyn Sink, _: f32| {
        for (_, m, z) in feathers {
            let u = rig::lerp_p(m, z, 0.25);
            let mid = rig::lerp_p(u, z, 0.5);
            limb_into(s, u, mid, z, 0.034, 0.024);
        }
    };
    d::flat(c, &tips, pal::TIP);
}

/// The ear cups of the headphones at the longitudes `lons` (one part for all of them).
fn draw_cups(c: &mut Canvas, ball: &Ball, lons: &[f32]) {
    if lons.is_empty() {
        return;
    }
    let cup = |lon: f32| {
        let on = ball.at(lon, 16.0, 1.03);
        let face_on = rig::cos_deg(lon + ball.yaw).abs();
        let rx = 0.05 + 0.035 * face_on;
        let tilt = -lon.signum() * 8.0 * (1.0 - face_on);
        (on.at, rx, tilt, face_on)
    };
    let all = |s: &mut dyn Sink, g: f32| {
        for lon in lons {
            let (at, rx, tilt, _) = cup(*lon);
            ellipse_into(s, at, rx + g, 0.1 + g, tilt);
        }
    };
    d::part(c, &all, &pal::PHONES, &pal::OUTLINE, 2);
    let caps = |s: &mut dyn Sink, _: f32| {
        for lon in lons {
            let (at, rx, tilt, face_on) = cup(*lon);
            if face_on > 0.25 {
                ellipse_into(s, at, rx * 0.55, 0.06, tilt);
            }
        }
    };
    d::flat(c, &caps, d::mix(pal::PHONES.base, pal::PHONES.light, 0.5));
}

/// The band of the headphones over the head, from cup to cup.
fn draw_band(c: &mut Canvas, ball: &Ball) {
    let half = (ball.rx * 1.03 * rig::cos_deg(ball.yaw)).abs().max(0.02);
    let cy = ball.center.1 - ball.ry * rig::sin_deg(16.0);
    let up = ball.center.1 - ball.ry * 1.1;
    let pts: [P; 7] = std::array::from_fn(|k| {
        let a = rig::PI * k as f32 / 6.0;
        (
            ball.center.0 - half * cantus_dsp::math::cos(a),
            cy - (cy - up) * cantus_dsp::math::sin(a),
        )
    });
    d::ribbon(c, &pts, 0.062, pal::INK, false);
    d::ribbon(c, &pts, 0.036, pal::BAND, true);
}

/// Legs and feet: from the hips (moving with the body) to the feet (on the ground, or
/// lifted by a step or a kick).
fn draw_legs(c: &mut Canvas, ball: &Ball, t: &Transform, b: &Body) {
    let mut legs = [((0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0), 0.0f32); 2];
    for (i, leg) in legs.iter_mut().enumerate() {
        let sgn = if i == 0 { -1.0 } else { 1.0 };
        let hip = map(t, ball.at(32.0 * sgn, -58.0, 0.92).at);
        let rest_x = ball.at(32.0 * sgn, -58.0, 0.92).at.0 * 1.12;
        let f = b.foot[i];
        let foot = (rest_x + f.0, -0.03 + f.1);
        let ankle = (foot.0, foot.1 - 0.012);
        let mid = (
            (hip.0 + ankle.0) * 0.5 + sgn * 0.02,
            (hip.1 + ankle.1) * 0.5,
        );
        // A foot turns out a little when lifted.
        let turn = sgn * (8.0 + 40.0 * (-f.1).clamp(0.0, 0.2) / 0.2);
        *leg = (hip, mid, ankle, foot, turn);
    }
    let all = |s: &mut dyn Sink, g: f32| {
        for (hip, mid, ankle, foot, turn) in legs {
            limb_into(s, hip, mid, ankle, 0.05 + 2.0 * g, 0.042 + 2.0 * g);
            // A rounded sole and three toes.
            ellipse_into(s, foot, 0.062 + g, 0.03 + g, turn);
            for k in [-1.0, 0.0, 1.0] {
                let toe = rig::rotate((foot.0 + k * 0.046, foot.1 + 0.008), foot, turn);
                ellipse_into(s, toe, 0.03 + g, 0.026 + g, turn + k * 18.0);
            }
        }
    };
    d::part(c, &all, &pal::LEG, &pal::OUTLINE, 1);
}

/// A patch on the ball (a belly, a mantle): an ellipse at `at`, its points pulled inside
/// the silhouette of the ball, so a patch turned toward the edge never pokes out of it.
fn decal(at: P, rx: f32, ry: f32) -> [P; 12] {
    std::array::from_fn(|k| {
        let a = rig::TAU * k as f32 / 12.0;
        let (sn, cs) = cantus_dsp::math::sin_cos(a);
        let p = (at.0 + rx * cs, at.1 + ry * sn);
        let (dx, dy) = (
            (p.0 - CENTER.0) / (RX - 0.012),
            (p.1 - CENTER.1) / (RY - 0.012),
        );
        let r = (dx * dx + dy * dy).sqrt();
        if r > 1.0 {
            (
                CENTER.0 + (p.0 - CENTER.0) / r,
                CENTER.1 + (p.1 - CENTER.1) / r,
            )
        } else {
            p
        }
    })
}

/// The ball, its belly and its back.
fn draw_ball(c: &mut Canvas, ball: &Ball) {
    let body = |s: &mut dyn Sink, g: f32| ellipse_into(s, CENTER, RX + g, RY + g, 0.0);
    d::part(c, &body, &pal::BODY, &pal::OUTLINE, 3);
    let belly = ball.at(0.0, -36.0, 1.0);
    if belly.z > 0.08 {
        let w = 0.25 * (0.3 + 0.7 * belly.squeeze);
        let pts = decal((belly.at.0, belly.at.1 + 0.03), w, 0.19);
        d::flat(c, &|s, _| d::blob_into(s, &pts), pal::BELLY.base);
    }
    // Its back: a darker mantle of feathers, seen when it turns away.
    let back = ball.at(180.0, 8.0, 1.0);
    if back.z > 0.08 {
        let w = 0.25 * (0.3 + 0.7 * back.squeeze);
        let x = back.at.0 * 0.85;
        let y = back.at.1 + 0.02;
        let mantle = decal((x, y), w, 0.17);
        d::flat(c, &|s, _| d::blob_into(s, &mantle), pal::WING.base);
        let marks = |s: &mut dyn Sink, _: f32| {
            for k in 0..2 {
                let yy = y + 0.02 + 0.07 * k as f32;
                let ww = w * (0.62 - 0.18 * k as f32);
                ribbon_into(
                    s,
                    &[
                        (x - ww, yy),
                        (x - ww * 0.5, yy + 0.03),
                        (x, yy),
                        (x + ww * 0.5, yy + 0.03),
                        (x + ww, yy),
                    ],
                    0.012,
                );
            }
        };
        d::flat(c, &marks, pal::WING.shade);
    }
    d::rim_arc(
        c,
        CENTER,
        RX,
        RY,
        -10.0,
        75.0,
        &pal::BODY,
        pal::OUTLINE.width,
    );
}

/// Where an extra's anchor is, in the character's units (the canvas's own transform).
fn anchor(at: Anchor, ball: &Ball, t: &Transform, b: &Body) -> P {
    match at {
        Anchor::Mouth(side) => {
            let beak = map(t, ball.at(0.0, 2.0, 1.0).at);
            (beak.0 + side * 0.3, beak.1 - 0.12)
        }
        Anchor::Hands => {
            let a = hand(ball, t, b, 0);
            let z = hand(ball, t, b, 1);
            ((a.0 + z.0) * 0.5, (a.1 + z.1) * 0.5)
        }
        Anchor::Hand(i) => hand(ball, t, b, i.min(1)),
        Anchor::Top => map(t, ball.at(0.0, 90.0, 1.4).at),
        Anchor::Feet => (b.x, 0.0),
    }
}

fn hand(ball: &Ball, t: &Transform, b: &Body, i: usize) -> P {
    map(&t.pre_concat(wing_transform(ball, i, b)), WING_TIP)
}

pub(super) fn draw(bird: &Songbird, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = torso(b);
    let tt = base.pre_concat(t);
    let ball = Ball {
        center: CENTER,
        rx: RX,
        ry: RY,
        yaw: b.yaw,
    };
    let strength = dance::fx_strength(pose);

    // The contact shadow, smaller and fainter the higher it is (none over a chroma key).
    let lifted = (-b.foot[0].1.min(b.foot[1].1)).max(0.0) * 0.3;
    let high = rig::clamp01((pose.lift * 0.6 + (-b.y).max(0.0) + lifted) / 0.5);
    if c.shadow {
        with(c, ground, |c| {
            d::glow(
                c,
                (b.x * 0.8, -0.005),
                0.36 * (1.0 - 0.45 * high),
                0.055 * (1.0 - 0.3 * high),
                d::alpha(pal::SHADOW, 1.0 - 0.55 * high),
                false,
            );
        });
    }

    let wing_z = |i: usize| {
        ball.at(if i == 0 { -80.0 } else { 80.0 }, SHOULDER_LAT, 1.0)
            .z
    };
    let cup_z = |lon: f32| ball.at(lon, 16.0, 1.0).z;
    let tail_z = ball.at(150.0, -40.0, 1.0).z;
    let mut back = [0.0f32; 2];
    let mut front = [0.0f32; 2];
    let (mut nb, mut nf) = (0, 0);
    for lon in [-90.0, 90.0] {
        if cup_z(lon) < -0.08 {
            back[nb] = lon;
            nb += 1;
        } else {
            front[nf] = lon;
            nf += 1;
        }
    }

    with(c, tt, |c| {
        if tail_z <= 0.0 {
            draw_tail(c, &ball, b);
        }
        let behind = [0usize, 1].map(|i| wing_z(i) < -0.05);
        match behind {
            [true, true] => draw_wings(c, &ball, b, &[0, 1]),
            [true, false] => draw_wings(c, &ball, b, &[0]),
            [false, true] => draw_wings(c, &ball, b, &[1]),
            _ => {}
        }
        draw_cups(c, &ball, &back[..nb]);
    });
    draw_legs(c, &ball, &t, b);
    with(c, tt, |c| {
        draw_ball(c, &ball);
        if tail_z > 0.0 {
            draw_tail(c, &ball, b);
        }
    });
    let face_t = tt.pre_concat(crate::rig::rotation_at(b.tilt, CENTER.0, CENTER.1));
    with(c, face_t, |c| face::draw(c, &ball, b, pose));
    with(c, tt, |c| {
        draw_band(c, &ball);
        draw_crest(c, &ball, b, pose.mood);
        draw_cups(c, &ball, &front[..nf]);
        let ahead = [0usize, 1].map(|i| wing_z(i) >= -0.05);
        match ahead {
            [true, true] => draw_wings(c, &ball, b, &[0, 1]),
            [true, false] => draw_wings(c, &ball, b, &[0]),
            [false, true] => draw_wings(c, &ball, b, &[1]),
            _ => {}
        }
    });

    // What the beat adds (at most four at once; no allocation).
    let ink = pal::OUTLINE;
    let mut extras = [None; 4];
    let mut n = 0;
    dance::extras(pose.mv, pose.phase, pose.cycle, pose, &mut |f| {
        if n < extras.len() {
            extras[n] = Some(f);
            n += 1;
        }
    });
    for f in extras.into_iter().flatten() {
        match f {
            Fx::Sparkle { at, t: age } => {
                // Where the hands met (on the beat), not where they have gone since.
                let p = if at == Anchor::Hands {
                    let met = bird.body_at(pose, 0.0);
                    let mt = torso(&met);
                    let mball = Ball {
                        yaw: met.yaw,
                        ..ball
                    };
                    anchor(at, &mball, &mt, &met)
                } else {
                    anchor(at, &ball, &t, b)
                };
                fx::sparkle(c, p, 0.17 * (0.6 + 0.4 * strength), age, pal::SPARKLE, &ink);
            }
            Fx::Note { at, t: age, kind } => {
                let p = anchor(at, &ball, &t, b);
                let fill = d::alpha(pal::NOTE, 0.5 + 0.5 * strength);
                fx::note(c, p, 0.12 * (0.75 + 0.25 * strength), age, fill, &ink, kind);
            }
            Fx::Dust { t: age } => with(c, ground, |c| {
                let seed = i64::from(pose.cycle);
                fx::dust(
                    c,
                    (b.x, 0.0),
                    0.5 * (0.5 + 0.5 * strength),
                    age,
                    pal::DUST,
                    seed,
                );
            }),
            Fx::Confetti { t: age } => with(c, ground, |c| {
                let n = (6.0 + 12.0 * strength) as usize;
                let seed = i64::from(pose.cycle);
                fx::confetti(c, (0.0, -1.05), 0.35, age, n, &pal::CONFETTI, seed);
            }),
        }
    }
    // A mood's own extra.
    let secs = pose.seconds as f32;
    match pose.mood {
        Mood::Sleepy => {
            let age = (secs / 2.2).rem_euclid(1.0);
            let p = anchor(Anchor::Top, &ball, &t, b);
            fx::zzz(c, (p.0 + 0.2, p.1 + 0.15), 0.09, age, pal::NOTE, &ink);
        }
        Mood::Excited => {
            let age = (pose.beat_phase * 1.4).min(1.0);
            let p = anchor(Anchor::Top, &ball, &t, b);
            fx::sparkle(c, (p.0 + 0.32, p.1 + 0.2), 0.11, age, pal::SPARKLE, &ink);
        }
        Mood::Shy => {
            let age = (secs / 3.1).rem_euclid(1.0);
            let p = anchor(Anchor::Top, &ball, &t, b);
            fx::heart(c, (p.0 - 0.3, p.1 + 0.25), 0.1, age, pal::CHEEK, &ink);
        }
        _ => {}
    }
}
