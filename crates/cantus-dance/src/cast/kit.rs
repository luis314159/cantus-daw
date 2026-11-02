//! What the family's characters with arms and legs share (ADR 0090): the torso's
//! transform, rubber-hose arms and legs through shoulder, elbow and wrist, mitten hands,
//! shoes, the contact shadow, and the beat's extras at a character's own anchors. The
//! songbird has its own wings; the others are built from these.

use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{self, Anchor, Body, Fx};
use crate::rig::draw::{self as d, ellipse_into, limb_into, Ink, Sink};
use crate::rig::{self, fx, P};
use cantus_core::inst::dancer::Mood;
use tiny_skia::Transform;

/// The body's transform: offset, lean about the feet, squash from `anchor_y`.
pub fn torso(b: &Body, anchor_y: f32) -> Transform {
    let (sx, sy) = rig::squash(b.squash);
    Transform::from_translate(b.x, b.y)
        .pre_concat(crate::rig::rotation(b.lean))
        .pre_translate(0.0, anchor_y)
        .pre_scale(sx, sy)
        .pre_translate(0.0, -anchor_y)
}

pub fn map(t: &Transform, p: P) -> P {
    (
        t.sx * p.0 + t.kx * p.1 + t.tx,
        t.ky * p.0 + t.sy * p.1 + t.ty,
    )
}

pub fn with(c: &mut Canvas, t: Transform, f: impl FnOnce(&mut Canvas)) {
    let saved = c.transform;
    c.transform = t;
    f(c);
    c.transform = saved;
}

/// An arm: its shoulder, elbow and wrist from the body's numbers for arm `i` (0 on the
/// viewer's left): `angle` (0 hangs down, 90 out to its side, 180 up), `bend` (degrees
/// the forearm curls toward the middle).
pub fn arm_points(shoulder: P, angle: f32, bend: f32, upper: f32, fore: f32, i: usize) -> [P; 3] {
    let side = if i == 0 { -1.0 } else { 1.0 };
    let up = side * angle;
    let elbow = rig::reach(shoulder, upper, up);
    let wrist = rig::reach(elbow, fore, up - side * bend);
    [shoulder, elbow, wrist]
}

/// The direction (degrees, `reach`'s) the forearm points.
pub fn forearm_deg(angle: f32, bend: f32, i: usize) -> f32 {
    let side = if i == 0 { -1.0 } else { 1.0 };
    side * angle - side * bend
}

/// A rubber-hose limb through three points (the curve passes the middle one).
pub fn hose_into(s: &mut dyn Sink, pts: [P; 3], w0: f32, w1: f32, g: f32) {
    let [a, m, z] = pts;
    let ctrl = (2.0 * m.0 - (a.0 + z.0) * 0.5, 2.0 * m.1 - (a.1 + z.1) * 0.5);
    limb_into(s, a, ctrl, z, w0 + 2.0 * g, w1 + 2.0 * g);
}

/// A mitten hand at `wrist`, pointing along `deg`: a palm and a thumb on the inner side
/// (`i` the arm), opened by `spread`.
pub fn mitten_into(s: &mut dyn Sink, wrist: P, deg: f32, size: f32, spread: f32, i: usize, g: f32) {
    let palm = rig::reach(wrist, size * 0.6, deg);
    ellipse_into(s, palm, size * 0.62 + g, size * 0.72 + g, -deg);
    let side = if i == 0 { 1.0 } else { -1.0 };
    let thumb_deg = deg + side * (40.0 + 35.0 * spread.clamp(0.0, 1.0));
    let thumb = rig::reach(wrist, size * 0.55, thumb_deg);
    ellipse_into(s, thumb, size * 0.22 + g, size * 0.36 + g, -thumb_deg);
}

/// A shoe (or hoof, or paw) at `foot` on the ground, its toe pointing out by `out`
/// (−1 left … 1 right), turned up when lifted.
pub fn shoe_into(s: &mut dyn Sink, foot: P, w: f32, h: f32, out: f32, lift: f32, g: f32) {
    let turn = out * (6.0 + 50.0 * (-lift).clamp(0.0, 0.2) / 0.2);
    let c = (foot.0 + out * w * 0.25, foot.1 - h * 0.5);
    ellipse_into(s, c, w * 0.5 + g, h * 0.5 + g, turn);
}

/// The contact shadow under a body on the ground transform `ground`: smaller and fainter
/// the higher it is.
pub fn shadow(c: &mut Canvas, ground: Transform, pose: &Pose, b: &Body, width: f32, ink: [u8; 4]) {
    if !c.shadow {
        return;
    }
    let lifted = (-b.foot[0].1.min(b.foot[1].1)).max(0.0) * 0.3;
    let high = rig::clamp01((pose.lift * 0.6 + (-b.y).max(0.0) + lifted) / 0.5);
    with(c, ground, |c| {
        d::glow(
            c,
            (b.x * 0.8, -0.005),
            width * (1.0 - 0.45 * high),
            width * 0.15 * (1.0 - 0.3 * high),
            d::alpha(ink, 1.0 - 0.55 * high),
            false,
        );
    });
}

/// The colours of a character's extras.
#[derive(Copy, Clone, Debug)]
pub struct FxLook {
    pub ink: Ink,
    pub sparkle: [u8; 4],
    pub note: [u8; 4],
    pub dust: [u8; 4],
    pub heart: [u8; 4],
    pub confetti: [[u8; 4]; 5],
}

/// The beat's extras and a mood's own, drawn at the character's anchors (`anchor` gives a
/// point in the canvas's units; `ground` is the floor's transform).
#[allow(clippy::too_many_arguments)]
pub fn extras(
    c: &mut Canvas,
    pose: &Pose,
    b: &Body,
    ground: Transform,
    look: &FxLook,
    anchor: &dyn Fn(Anchor) -> P,
    met: &dyn Fn() -> P,
) {
    let strength = dance::fx_strength(pose);
    let ink = look.ink;
    let mut list = [None; 4];
    let mut n = 0;
    dance::extras(pose.mv, pose.phase, pose.cycle, pose, &mut |f| {
        if n < list.len() {
            list[n] = Some(f);
            n += 1;
        }
    });
    for f in list.into_iter().flatten() {
        match f {
            Fx::Sparkle { at, t } => {
                let p = if at == Anchor::Hands {
                    met()
                } else {
                    anchor(at)
                };
                fx::sparkle(c, p, 0.17 * (0.6 + 0.4 * strength), t, look.sparkle, &ink);
            }
            Fx::Note { at, t, kind } => {
                let fill = d::alpha(look.note, 0.5 + 0.5 * strength);
                fx::note(
                    c,
                    anchor(at),
                    0.12 * (0.75 + 0.25 * strength),
                    t,
                    fill,
                    &ink,
                    kind,
                );
            }
            Fx::Dust { t } => with(c, ground, |c| {
                let seed = i64::from(pose.cycle);
                fx::dust(
                    c,
                    (b.x, 0.0),
                    0.5 * (0.5 + 0.5 * strength),
                    t,
                    look.dust,
                    seed,
                );
            }),
            Fx::Confetti { t } => with(c, ground, |c| {
                let k = (6.0 + 12.0 * strength) as usize;
                let seed = i64::from(pose.cycle);
                fx::confetti(c, (0.0, -1.05), 0.35, t, k, &look.confetti, seed);
            }),
        }
    }
    let secs = pose.seconds as f32;
    let top = anchor(Anchor::Top);
    match pose.mood {
        Mood::Sleepy => {
            let age = (secs / 2.2).rem_euclid(1.0);
            fx::zzz(c, (top.0 + 0.2, top.1 + 0.15), 0.09, age, look.note, &ink);
        }
        Mood::Excited => {
            let age = (pose.beat_phase * 1.4).min(1.0);
            fx::sparkle(
                c,
                (top.0 + 0.32, top.1 + 0.2),
                0.11,
                age,
                look.sparkle,
                &ink,
            );
        }
        Mood::Shy => {
            let age = (secs / 3.1).rem_euclid(1.0);
            fx::heart(c, (top.0 - 0.3, top.1 + 0.25), 0.1, age, look.heart, &ink);
        }
        _ => {}
    }
}

/// The body for a pose in a character's style: the move, blended from the move it is
/// leaving, turned by the pose's `turn` (`turn_yaw` degrees at full turn).
pub fn body(pose: &Pose, style: &dance::Style, turn_yaw: f32) -> Body {
    let now = dance::dance(pose.mv, pose.phase, pose.cycle, pose, style);
    let mut b = match pose.from {
        Some((from, phase)) if pose.blend < 1.0 => {
            let was = dance::dance(from, phase, pose.from_cycle, pose, style);
            Body::lerp(&was, &now, rig::ease_in_out(pose.blend))
        }
        _ => now,
    };
    b.yaw += pose.turn.clamp(-1.0, 1.0) * turn_yaw;
    b.mouth = b.mouth.max(0.0) * (0.6 + 0.4 * rig::vigour(pose));
    b
}

/// The body of the current move at `phase` of its cycle (where an extra was born).
pub fn body_at(pose: &Pose, style: &dance::Style, phase: f32, turn_yaw: f32) -> Body {
    let mut b = dance::dance(pose.mv, phase, pose.cycle, pose, style);
    b.yaw += pose.turn.clamp(-1.0, 1.0) * turn_yaw;
    b
}
