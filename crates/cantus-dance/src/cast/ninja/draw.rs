//! Fuga's own moves and drawing from a [`Body`]: shadow, the scarf's tails (behind), a far
//! arm, the legs with their wraps, the trousers, the gi and its gold sash, the scarf round
//! the neck, the near arms, the head, the extras. A jump is an aerial (a flip turned in the
//! picture's plane, tucked), a freeze a crouched landing.

use super::{head, moves, pal, Ninja};
use crate::cast::edge::{self, poly_into, slab_into};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{self, Anchor, Body, Style};
use crate::rig::draw::{self as d, ellipse_into, ribbon_into, Sink};
use crate::rig::{self, Ball, P, TAU};
use cantus_core::inst::dancer::Move;
use cantus_dsp::math;
use tiny_skia::Transform;

pub(super) const ANCHOR_Y: f32 = -0.3;
pub(super) const TORSO: (P, f32, f32) = ((0.0, -0.86), 0.13, 0.15);
pub(super) const HEAD: (P, f32, f32) = ((0.0, -1.16), 0.115, 0.125);
const UPPER: f32 = 0.2;
const FORE: f32 = 0.19;
/// Hip to ankle standing (what a crouch folds).
const LEG: f32 = 0.59;

pub(super) fn ball(c: (P, f32, f32), yaw: f32) -> Ball {
    Ball {
        center: c.0,
        rx: c.1,
        ry: c.2,
        yaw,
    }
}

/// A move as Fuga dances it: the family's, but its own stance, kick, spin, bow, jump and
/// freeze.
fn dance_move(mv: Move, p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    match mv {
        Move::Idle => moves::ready(pose, st),
        Move::Kick => moves::high_kick(p, cycle, pose, st),
        Move::Spin => moves::tornado(p, pose, st),
        Move::Bow => moves::salute(p, cycle, pose, st),
        Move::Jump => aerial(p, pose, st),
        Move::Freeze => landing(p, cycle, pose),
        _ => dance::dance(mv, p, cycle, pose, st),
    }
}

/// The body for a pose, blended from the move it leaves, turned by `turn`.
pub(super) fn body(n: &Ninja, pose: &Pose) -> Body {
    let st = &n.style;
    let now = dance_move(pose.mv, pose.phase, pose.cycle, pose, st);
    let mut b = match pose.from {
        Some((from, phase)) if pose.blend < 1.0 => {
            let was = dance_move(from, phase, pose.from_cycle, pose, st);
            Body::lerp(&was, &now, rig::ease_in_out(pose.blend))
        }
        _ => now,
    };
    b.yaw += pose.turn.clamp(-1.0, 1.0) * 40.0;
    b.mouth = b.mouth.max(0.0) * (0.6 + 0.4 * rig::vigour(pose));
    b
}

/// The jump: the family's take-off and landing, tucked tight in the air.
fn aerial(p: f32, pose: &Pose, st: &Style) -> Body {
    let mut b = dance::jump(p, pose, st);
    let tuck =
        rig::smooth(rig::window(p, 0.6, 0.7)) * (1.0 - rig::smooth(rig::window(p, 0.86, 0.95)));
    let fold = (0.24 + 0.08 * dance::extreme(pose)) * tuck;
    b.foot = [
        rig::lerp_p(b.foot[0], (-0.02, b.y - fold), tuck),
        rig::lerp_p(b.foot[1], (0.02, b.y - fold), tuck),
    ];
    b.arm = [
        rig::lerp(b.arm[0], -6.0, tuck),
        rig::lerp(b.arm[1], -6.0, tuck),
    ];
    b.bend = [rig::lerp(b.bend[0], 70.0, tuck); 2];
    b.squash -= 0.04 * tuck;
    b.squint = b.squint.max(0.6 * tuck);
    b
}

/// How far round the aerial has turned (degrees, its side alternating by cycle).
pub(super) fn flip_deg(pose: &Pose) -> f32 {
    if pose.mv != Move::Jump || pose.energy < 0.35 {
        return 0.0;
    }
    let turn = rig::ease_in_out(rig::window(pose.phase, 0.6, 0.9));
    let k = rig::smooth((pose.energy - 0.35) / 0.25);
    360.0 * turn * k * rig::side(pose.cycle)
}

/// The freeze: a crouched landing on one knee, a hand to the floor, the other arm back,
/// looking up, held and breathing.
fn landing(p: f32, cycle: u32, pose: &Pose) -> Body {
    let hit = if cycle == 0 {
        rig::ease_out_back(rig::window(p, 0.0, 0.06))
    } else {
        1.0
    };
    let x = dance::extreme(pose);
    let breath = 0.008 * math::sin(pose.seconds as f32 * TAU / 2.4);
    let mut f = Body::rest();
    f.y = (0.2 + 0.1 * x) + breath;
    f.squash = 0.03;
    f.lean = 9.0;
    f.foot = [(-0.17, 0.0), (0.14, -0.005)];
    f.arm = [-14.0, 128.0];
    f.bend = [-8.0, 22.0];
    f.spread = [0.9, 0.5];
    f.yaw = 18.0;
    f.nod = 10.0;
    f.tilt = -6.0;
    f.look = (-0.1, -0.3);
    f.squint = 0.3;
    f.tail = -26.0;
    f.top = 18.0;
    f.mouth = 0.0;
    Body::lerp(&Body::rest(), &f, hit)
}

/// How hard the wind blows the scarf, the headband and the ponytail.
pub(super) fn wind(pose: &Pose) -> f32 {
    if pose.playing {
        0.45 + 0.55 * rig::clamp01(pose.energy)
    } else {
        0.42
    }
}

/// A trailing tail of cloth from `root`: `n` points, `len` long, streaming to the viewer's
/// left in the wind, swung by what the body does and fluttering along its length.
pub(super) fn streamer(
    root: P,
    len: f32,
    n: usize,
    seed: f32,
    b: &Body,
    pose: &Pose,
    w: f32,
) -> [P; 8] {
    let t = pose.seconds as f32;
    let n = n.clamp(2, 8);
    let mut pts = [root; 8];
    let base = rig::lerp(-36.0, -88.0, w) + b.tail * 0.9 - b.lean * 0.6;
    let mut at = root;
    for (j, o) in pts.iter_mut().enumerate().take(n).skip(1) {
        let u = j as f32 / (n - 1) as f32;
        // A standing S-curve along the cloth, and a flutter running down it.
        let wave = 22.0 * math::sin(u * rig::PI * 1.8 + seed);
        let flutter = (7.0 + 18.0 * w) * u * math::sin(t * TAU * 1.4 + seed - u * 3.4);
        at = rig::reach(
            at,
            len / (n - 1) as f32,
            base - 26.0 * u * w + wave + flutter,
        );
        *o = at;
    }
    for o in pts.iter_mut().skip(n) {
        *o = at;
    }
    pts
}

/// A tapered tail through `pts`, `w0` wide at the root and `w1` at the tip, grown by `g`.
pub(super) fn tail_into(s: &mut dyn Sink, pts: &[P], w0: f32, w1: f32, g: f32) {
    d::taper_into(s, pts, w0 + 2.0 * g, w1 + 2.0 * g);
}

/// The end of a torn tail: two ragged points past its last point (`pts` its centre line,
/// `w` its width there), grown by `g`.
pub(super) fn tatter_into(s: &mut dyn Sink, pts: &[P], w: f32, g: f32) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let (a, e) = (pts[n - 2], pts[n - 1]);
    let (dx, dy) = (e.0 - a.0, e.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-5);
    let u = (dx / len, dy / len);
    let nm = (-u.1, u.0);
    let h = w * 0.5 + g;
    let at = |f: f32, o: f32| (e.0 + u.0 * f + nm.0 * o, e.1 + u.1 * f + nm.1 * o);
    let rag = [
        at(-0.02, -h),
        at(0.05 + g, -h * 0.55),
        at(0.012, -h * 0.05),
        at(0.036 + g, h * 0.45),
        at(-0.004, h * 0.6),
        at(-0.02, h),
    ];
    d::polygon_into(s, &rag);
}

/// The scarf's two torn tails from the side of its neck (in the picture's frame).
fn draw_scarf_tails(c: &mut Canvas, t: &Transform, torso: &Ball, b: &Body, pose: &Pose) {
    let w = wind(pose);
    let knot = torso.at(-104.0, 62.0, 0.9).at;
    let root = kit::map(t, (knot.0, knot.1));
    let long = streamer(root, 0.66, 7, 0.0, b, pose, (w + 0.12).min(1.0));
    let short = streamer(
        (root.0 + 0.01, root.1 + 0.02),
        0.5,
        5,
        2.6,
        b,
        pose,
        w * 0.55,
    );
    let piece = |k: usize, s: &mut dyn Sink, g: f32| {
        if k == 0 {
            tail_into(s, &short[..5], 0.05, 0.04, g);
            tatter_into(s, &short[..5], 0.04, g);
        } else {
            tail_into(s, &long[..7], 0.056, 0.044, g);
            tatter_into(s, &long[..7], 0.044, g);
        }
    };
    let tails = |s: &mut dyn Sink, g: f32| {
        piece(0, s, g);
        piece(1, s, g);
    };
    d::outline(c, &tails, &pal::OUTLINE);
    edge::hard_each(c, 2, &piece, &pal::SCARF, true);
}

fn shoulder(torso: &Ball, i: usize) -> P {
    let side = if i == 0 { -1.0 } else { 1.0 };
    torso.at(side * 84.0, 40.0, 1.36).at
}

fn arm_of(torso: &Ball, b: &Body, i: usize) -> ([P; 3], f32) {
    let pts = kit::arm_points(shoulder(torso, i), b.arm[i], b.bend[i], UPPER, FORE, i);
    (pts, kit::forearm_deg(b.arm[i], b.bend[i], i))
}

/// Straps across a guard from `a` to `z`, at shares `at` of the way.
fn straps_into(s: &mut dyn Sink, a: P, z: P, half: f32, at: &[f32]) {
    for &k in at {
        let p = edge::along(a, z, k, -half);
        let q = edge::along(a, z, k + 0.06, half);
        ribbon_into(s, &[p, q], 0.012);
    }
}

/// Arms (the ones `which`): a dark sleeve, a gunmetal forearm guard with crimson straps,
/// a dark glove.
fn draw_arms(c: &mut Canvas, torso: &Ball, b: &Body, which: &[usize]) {
    let sleeve = |k: usize, s: &mut dyn Sink, g: f32| {
        let (p, _) = arm_of(torso, b, which[k]);
        let z = rig::lerp_p(p[0], p[1], 1.1);
        slab_into(s, p[0], z, &[(0.0, 0.03), (0.5, 0.034), (1.0, 0.03)], g);
    };
    const GUARD: [(f32, f32); 4] = [(0.0, 0.024), (0.15, 0.03), (0.85, 0.028), (1.0, 0.022)];
    let guard = |k: usize, s: &mut dyn Sink, g: f32| {
        let (p, _) = arm_of(torso, b, which[k]);
        slab_into(s, rig::lerp_p(p[1], p[2], 0.08), p[2], &GUARD, g);
    };
    let hands = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (p, deg) = arm_of(torso, b, i);
            kit::mitten_into(s, p[2], deg, 0.042, b.spread[i], i, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        hands(s, g);
        for k in 0..which.len() {
            guard(k, s, g);
            sleeve(k, s, g);
        }
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &hands, &pal::GI.look(), 1);
    edge::hard_each(c, which.len(), &guard, &pal::GUARD, true);
    let straps = |s: &mut dyn Sink, _: f32| {
        for &i in which {
            let (p, _) = arm_of(torso, b, i);
            straps_into(s, p[1], p[2], 0.03, &[0.3, 0.66]);
        }
    };
    d::flat(c, &straps, pal::STRAP);
    edge::hard_each(c, which.len(), &sleeve, &pal::GI, true);
}

/// The legs: baggy dark trousers, gunmetal shin guards strapped over the wraps, dark
/// split-toe shoes.
fn draw_legs(c: &mut Canvas, t: &Transform, b: &Body, ws: f32) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, (side * 0.064 * ws, -0.64));
        let f = b.foot[i];
        let foot = (side * 0.09 + f.0, f.1);
        let ankle = (foot.0, foot.1 - 0.05);
        let (dx, dy) = (ankle.0 - hip.0, ankle.1 - hip.1);
        let dist = (dx * dx + dy * dy).sqrt();
        // A folded leg bends its knee out (and forward in a crouch).
        let fold = (LEG - dist).max(0.0);
        let knee = (
            (hip.0 + ankle.0) * 0.5 + side * (0.012 + 0.9 * fold),
            (hip.1 + ankle.1) * 0.5 - 0.35 * fold,
        );
        (hip, knee, ankle, foot, side, f.1)
    };
    let thigh = |i: usize, s: &mut dyn Sink, g: f32| {
        let (hip, knee, ..) = leg(i);
        slab_into(s, hip, knee, &[(0.0, 0.044), (0.55, 0.046), (1.0, 0.04)], g);
    };
    let shin = |i: usize, s: &mut dyn Sink, g: f32| {
        let (_, knee, ankle, ..) = leg(i);
        slab_into(
            s,
            knee,
            ankle,
            &[(0.0, 0.03), (0.4, 0.032), (1.0, 0.024)],
            g,
        );
    };
    let shoes = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (.., foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.12, 0.05, side, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            shin(i, s, g);
            thigh(i, s, g);
        }
        shoes(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &shoes, &pal::GI.look(), 1);
    edge::hard_each(c, 2, &shin, &pal::GUARD, true);
    let straps = |s: &mut dyn Sink, _: f32| {
        for i in 0..2 {
            let (_, knee, ankle, ..) = leg(i);
            straps_into(s, knee, ankle, 0.032, &[0.28, 0.7]);
        }
    };
    d::flat(c, &straps, pal::STRAP);
    edge::hard_each(c, 2, &thigh, &pal::GI, true);
}

/// The gi with its high collar, the sash and its gold fermata buckle, the scarf wrapped
/// round the collar (in the torso's frame).
fn draw_torso(c: &mut Canvas, torso: &Ball, ws: f32) {
    let seat = |s: &mut dyn Sink, g: f32| {
        let p = [
            (-0.1, -0.735),
            (0.1, -0.735),
            (0.105, -0.64),
            (0.0, -0.6),
            (-0.105, -0.64),
        ];
        poly_into(s, &p.map(|(x, y)| (x * ws, y)), g);
    };
    edge::part(c, &seat, &pal::GI, &pal::OUTLINE, false);
    let gi = |s: &mut dyn Sink, g: f32| {
        let p = [
            (-0.115, -1.0),
            (0.115, -1.0),
            (0.155, -0.94),
            (0.128, -0.82),
            (0.1, -0.72),
            (0.112, -0.655),
            (0.0, -0.64),
            (-0.112, -0.655),
            (-0.1, -0.72),
            (-0.128, -0.82),
            (-0.155, -0.94),
        ];
        poly_into(s, &p.map(|(x, y)| (x * ws, y)), g);
    };
    edge::part(c, &gi, &pal::GI, &pal::OUTLINE, true);
    let front = torso.at(0.0, 10.0, 1.0);
    let sq = 0.35 + 0.65 * front.squeeze;
    let fx = front.at.0 * 0.85;
    if front.z > 0.1 {
        // The wrap's edge, the left side over the right.
        let lapel = [
            (fx + 0.05 * sq, -0.99),
            (fx + 0.01 * sq, -0.9),
            (fx - 0.05 * sq, -0.74),
        ];
        d::flat(c, &|s, _| ribbon_into(s, &lapel, 0.01), pal::GI.rim);
    }
    // The sash and its buckle.
    let sash = |s: &mut dyn Sink, g: f32| d::rect_into(s, (0.0, -0.72), 0.225 * ws, 0.042, 0.0, g);
    edge::part(c, &sash, &pal::SASH, &pal::OUTLINE, false);
    if front.z > 0.3 {
        let at = (fx, -0.72);
        let buckle = |s: &mut dyn Sink, g: f32| d::rect_into(s, at, 0.04 * sq, 0.034, 0.0, g);
        d::outline(c, &buckle, &pal::OUTLINE.thin(0.7));
        d::fixed(c, &buckle, pal::GI.shade);
        fermata(c, at, 0.6 * sq, 0.55);
    }
    // The high collar, and the scarf wrapped over it.
    let collar = |s: &mut dyn Sink, g: f32| {
        let p = [
            (-0.07, -0.995),
            (0.07, -0.995),
            (0.062, -1.06),
            (fx * 0.3 + 0.02, -1.085),
            (fx * 0.3 - 0.02, -1.085),
            (-0.062, -1.06),
        ];
        poly_into(s, &p.map(|(x, y)| (x * ws.max(0.9), y)), g);
    };
    edge::part(c, &collar, &pal::GI, &pal::OUTLINE, true);
    let roll = |s: &mut dyn Sink, g: f32| {
        let p = [
            (-0.092, -1.035),
            (0.092, -1.035),
            (0.1, -1.0),
            (0.06, -0.975),
            (-0.06, -0.975),
            (-0.1, -1.0),
        ];
        poly_into(s, &p.map(|(x, y)| (x * ws, y)), g);
    };
    edge::part(c, &roll, &pal::SCARF, &pal::OUTLINE, true);
    let knot_on = torso.at(-104.0, 62.0, 0.9);
    if knot_on.z > 0.2 {
        let k = knot_on.at;
        let knot = |s: &mut dyn Sink, g: f32| {
            let p = [
                (-0.026, -0.018),
                (0.02, -0.024),
                (0.03, 0.012),
                (-0.01, 0.026),
            ];
            poly_into(s, &p.map(|(x, y)| (k.0 + x, k.1 + y)), g);
        };
        edge::part(c, &knot, &pal::SCARF, &pal::OUTLINE.thin(0.8), false);
    }
}

/// The fermata (music's *hold*) in gold: an arc over a dot, `size` × its standard,
/// squeezed across by `sq`.
pub(super) fn fermata(c: &mut Canvas, at: P, sq: f32, size: f32) {
    let (x, y) = at;
    let arc: [P; 5] = std::array::from_fn(|k| {
        let a = rig::PI + rig::PI * k as f32 / 4.0;
        let (sn, cs) = math::sin_cos(a);
        (x + 0.032 * size * cs * sq, y + (0.012 + 0.026 * sn) * size)
    });
    let mark = |s: &mut dyn Sink, g: f32| {
        ribbon_into(s, &arc, (0.012 + 2.0 * g) * size);
        ellipse_into(
            s,
            (x, y + 0.006 * size),
            (0.009 * sq + g) * size,
            (0.009 + g) * size,
            0.0,
        );
    };
    d::fixed(c, &mark, pal::GOLD);
}

pub(super) fn draw(n: &Ninja, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = crate::cast::edge::ground(c, pose);
    kit::shadow(c, ground, pose, b, 0.24, pal::SHADOW);
    // The aerial turns everything about its middle.
    let flip = rig::rotation_at(flip_deg(pose), b.x, b.y - 0.62);
    let fb = base.pre_concat(flip);
    c.transform = fb;
    let t = kit::torso(b, ANCHOR_Y);
    let tt = fb.pre_concat(t);
    let torso = ball(TORSO, kit::torso_yaw(b.yaw));
    let head = ball(HEAD, b.yaw);
    let head_local = t.pre_concat(rig::rotation_at(b.tilt, 0.0, -1.04));
    let head_t = fb.pre_concat(head_local);
    let ws = 0.8 + 0.2 * rig::cos_deg(torso.yaw).abs();
    let away = torso.at(180.0, 0.0, 1.0).z > 0.0;
    if !away {
        draw_scarf_tails(c, &t, &torso, b, pose);
    }
    let arm_z = |i: usize| torso.at(if i == 0 { -82.0 } else { 82.0 }, 40.0, 1.0).z;
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
    kit::with(c, tt, |c| draw_arms(c, &torso, b, &far[..nf]));
    draw_legs(c, &t, b, ws);
    kit::with(c, tt, |c| draw_torso(c, &torso, ws));
    if away {
        draw_scarf_tails(c, &t, &torso, b, pose);
    }
    kit::with(c, tt, |c| draw_arms(c, &torso, b, &near[..nn]));
    kit::with(c, head_t, |c| head::draw_head(c, &head, b, pose));
    c.transform = base;
    let hand = |bd: &Body, mt: &Transform, i: usize| {
        let tb = ball(TORSO, kit::torso_yaw(bd.yaw));
        kit::map(mt, arm_of(&tb, bd, i).0[2])
    };
    let ft = flip.pre_concat(t);
    let fh = flip.pre_concat(head_local);
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let p = kit::map(&fh, head.at(0.0, -50.0, 1.0).at);
                (p.0 + side * 0.28, p.1 - 0.12)
            }
            Anchor::Top => kit::map(&fh, head.at(0.0, 90.0, 2.0).at),
            Anchor::Hand(i) => hand(b, &ft, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &ft, 0), hand(b, &ft, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let mb = kit::body_at(pose, &n.style, 0.0, 40.0);
        let mt = kit::torso(&mb, ANCHOR_Y);
        rig::lerp_p(hand(&mb, &mt, 0), hand(&mb, &mt, 1), 0.5)
    };
    let look = FxLook {
        ink: pal::OUTLINE,
        sparkle: pal::SPARKLE,
        note: pal::NOTE,
        dust: pal::DUST,
        heart: pal::SCARF.rim,
        confetti: pal::PETAL,
    };
    kit::extras(c, pose, b, ground, &look, &anchor, &met);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jump(phase: f32, energy: f32, cycle: u32) -> Pose {
        Pose {
            mv: Move::Jump,
            phase,
            energy,
            cycle,
            ..Pose::default()
        }
    }

    #[test]
    fn the_aerial_turns_once_round_in_the_air_and_lands_upright() {
        assert_eq!(flip_deg(&jump(0.0, 1.0, 0)), 0.0);
        assert_eq!(flip_deg(&jump(0.5, 1.0, 0)), 0.0, "not before the take-off");
        let mid = flip_deg(&jump(0.75, 1.0, 0)).abs();
        assert!(mid > 90.0 && mid < 270.0, "{mid}");
        assert_eq!(flip_deg(&jump(0.95, 1.0, 0)).abs(), 360.0);
        // Sides take turns; at low energy it only jumps.
        assert_eq!(
            flip_deg(&jump(0.75, 1.0, 0)),
            -flip_deg(&jump(0.75, 1.0, 1))
        );
        assert_eq!(flip_deg(&jump(0.75, 0.2, 0)), 0.0);
    }
}
