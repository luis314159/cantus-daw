//! Drawing the mecha from a [`Body`] (ADR 0105): shadow, the jets and the back vanes
//! (behind), a far arm, the armoured legs, the hips and waist, the chest with its pearl
//! plates, seams of light and speaker core (or the back pack, seen from behind), the near
//! arms, the helmet (swept blades, the V visor and its light, the sculpted faceplate),
//! the extras. Gunmetal hard-shaded with a coral rim, pearl plates, coral light.

use super::{face, pal, Mecha};
use crate::cast::edge::{self, along, poly_into, slab_into};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, ribbon_into, Sink};
use crate::rig::{self, Ball, P};
use cantus_core::inst::dancer::Move;
use tiny_skia::Transform;

pub(super) const ANCHOR_Y: f32 = -0.36;
/// The ball the chest's features sit on.
pub(super) const TORSO: (P, f32, f32) = ((0.0, -0.93), 0.2, 0.2);
/// The helmet's middle and size.
pub(super) const HEAD: (P, f32, f32) = ((0.0, -1.215), 0.11, 0.12);
const UPPER: f32 = 0.19;
const FORE: f32 = 0.18;
/// Hip to ankle standing (what a crouch folds).
const LEG: f32 = 0.58;

pub(super) fn ball(c: (P, f32, f32), yaw: f32) -> Ball {
    Ball {
        center: c.0,
        rx: c.1,
        ry: c.2,
        yaw,
    }
}

/// A rounded rectangle centred on `c` (`w` × `h`, corners `r`), grown by `g`.
pub(super) fn rounded_into(s: &mut dyn Sink, c: P, w: f32, h: f32, r: f32, g: f32) {
    let (hw, hh) = (w * 0.5 + g, h * 0.5 + g);
    if hw <= 0.0 || hh <= 0.0 {
        return;
    }
    let r = (r + g).clamp(0.0, hw.min(hh));
    let k = r * (1.0 - 0.552_284_7);
    let (x0, x1, y0, y1) = (c.0 - hw, c.0 + hw, c.1 - hh, c.1 + hh);
    s.move_to(x0 + r, y0);
    s.line_to(x1 - r, y0);
    s.cubic_to(x1 - k, y0, x1, y0 + k, x1, y0 + r);
    s.line_to(x1, y1 - r);
    s.cubic_to(x1, y1 - k, x1 - k, y1, x1 - r, y1);
    s.line_to(x0 + r, y1);
    s.cubic_to(x0 + k, y1, x0, y1 - k, x0, y1 - r);
    s.line_to(x0, y0 + r);
    s.cubic_to(x0, y0 + k, x0 + k, y0, x0 + r, y0);
    s.close();
}

/// A straight tapered tube from `a` to `z` (`w0` to `w1` wide), grown by `g`.
fn tube_into(s: &mut dyn Sink, a: P, z: P, w0: f32, w1: f32, g: f32) {
    let m = rig::lerp_p(a, z, 0.5);
    d::limb_into(s, a, m, z, w0 + 2.0 * g, w1 + 2.0 * g);
}

/// How wide the body looks turned by `yaw` (it is deeper than flat).
fn width_of(yaw: f32) -> f32 {
    0.8 + 0.2 * rig::cos_deg(yaw).abs()
}

/// The shoulder of arm `i` on the chest ball.
fn shoulder(torso: &Ball, i: usize) -> P {
    let side = if i == 0 { -1.0 } else { 1.0 };
    torso.at(side * 84.0, 34.0, 1.27).at
}

fn arm_of(torso: &Ball, b: &Body, i: usize) -> ([P; 3], f32) {
    let pts = kit::arm_points(shoulder(torso, i), b.arm[i], b.bend[i], UPPER, FORE, i);
    (pts, kit::forearm_deg(b.arm[i], b.bend[i], i))
}

/// Seams of light (never turned by the hue): one fill (the visor and the core have their
/// halos; a seam is thin enough to read without).
pub(super) fn glow_lines(c: &mut Canvas, lines: &dyn Fn(&mut dyn Sink, f32)) {
    d::fixed(c, &|s, _| lines(s, 0.0), pal::GLOW);
}

/// Points of a shoulder plate of arm `i` (in the plate's own frame, mirrored by `side`),
/// turned a little with the arm.
fn pad_pts(sh: P, side: f32, arm: f32, local: &[P]) -> [P; 8] {
    let k = arm.clamp(-30.0, 150.0) * 0.14 * side;
    let mut out = [(0.0, 0.0); 8];
    for (o, &(x, y)) in out.iter_mut().zip(local.iter()) {
        *o = rig::rotate((sh.0 + side * x, sh.1 + y), sh, k);
    }
    out
}

const PAD: [P; 7] = [
    (-0.07, -0.035),
    (0.0, -0.088),
    (0.104, -0.074),
    (0.13, -0.012),
    (0.104, 0.05),
    (0.03, 0.072),
    (-0.052, 0.04),
];

/// Arms (the ones `which`): the graphite upper arm with its piston, the gunmetal gauntlet
/// with a pearl plate and a seam of light, a graphite fist, the shoulder plate over all.
fn draw_arms(c: &mut Canvas, torso: &Ball, b: &Body, which: &[usize]) {
    let upper = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (p, _) = arm_of(torso, b, i);
            tube_into(s, p[0], p[1], 0.05, 0.044, g);
            ellipse_into(s, p[1], 0.032 + g, 0.032 + g, 0.0);
        }
    };
    const GAUNT: [(f32, f32); 4] = [(0.0, 0.03), (0.2, 0.048), (0.8, 0.052), (1.0, 0.042)];
    let gaunt = |k: usize, s: &mut dyn Sink, g: f32| {
        let (p, _) = arm_of(torso, b, which[k]);
        slab_into(s, p[1], p[2], &GAUNT, g);
    };
    let fists = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (p, deg) = arm_of(torso, b, i);
            kit::mitten_into(s, p[2], deg, 0.05, b.spread[i], i, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        upper(s, g);
        for k in 0..which.len() {
            gaunt(k, s, g);
        }
        fists(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &upper, &pal::FRAME.look(), 1);
    let pistons = |s: &mut dyn Sink, _: f32| {
        for &i in which {
            let (p, _) = arm_of(torso, b, i);
            let side = if i == 0 { -1.0 } else { 1.0 };
            let a = along(p[0], p[1], 0.3, side * 0.016);
            let z = along(p[0], p[1], 0.9, side * 0.014);
            ribbon_into(s, &[a, z], 0.01);
        }
    };
    d::flat(c, &pistons, pal::PISTON);
    edge::hard_each(c, which.len(), &gaunt, &pal::GUNMETAL, true);
    let plates = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (p, _) = arm_of(torso, b, i);
            let side = if i == 0 { 1.0 } else { -1.0 };
            let a = along(p[1], p[2], 0.25, side * 0.012);
            let z = along(p[1], p[2], 0.72, side * 0.014);
            slab_into(s, a, z, &[(0.0, 0.016), (1.0, 0.022)], g);
        }
    };
    d::flat(c, &plates, pal::PEARL.base);
    glow_lines(c, &|s, g| {
        for &i in which {
            let (p, _) = arm_of(torso, b, i);
            let side = if i == 0 { -1.0 } else { 1.0 };
            let a = along(p[1], p[2], 0.3, side * 0.03);
            let z = along(p[1], p[2], 0.86, side * 0.034);
            ribbon_into(s, &[a, z], 0.007 + 2.0 * g);
        }
    });
    d::bands(c, &fists, &pal::FRAME.look(), 2);
    // The shoulder plates.
    let pad = |k: usize, s: &mut dyn Sink, g: f32| {
        let i = which[k];
        let side = if i == 0 { -1.0 } else { 1.0 };
        poly_into(
            s,
            &pad_pts(shoulder(torso, i), side, b.arm[i], &PAD)[..7],
            g,
        );
    };
    let pads = |s: &mut dyn Sink, g: f32| {
        for k in 0..which.len() {
            pad(k, s, g);
        }
    };
    d::outline(c, &pads, &pal::OUTLINE);
    edge::hard_each(c, which.len(), &pad, &pal::GUNMETAL, true);
    let trims = |s: &mut dyn Sink, _: f32| {
        for &i in which {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let local = [(-0.05, -0.05), (0.0, -0.078), (0.098, -0.064)];
            let t = pad_pts(shoulder(torso, i), side, b.arm[i], &local);
            ribbon_into(s, &t[..3], 0.018);
        }
    };
    d::flat(c, &trims, pal::PEARL.base);
    glow_lines(c, &|s, g| {
        for &i in which {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let local = [(-0.02, 0.012), (0.06, 0.0), (0.108, -0.016)];
            let t = pad_pts(shoulder(torso, i), side, b.arm[i], &local);
            ribbon_into(s, &t[..3], 0.007 + 2.0 * g);
        }
    });
}

/// One leg: hip, knee, ankle, foot, its side, how high the foot is.
fn leg_of(t: &Transform, b: &Body, ws: f32, i: usize) -> (P, P, P, P, f32, f32) {
    let side = if i == 0 { -1.0 } else { 1.0 };
    let hip = kit::map(t, (side * 0.075 * ws, -0.66));
    let f = b.foot[i];
    let foot = (side * 0.105 + f.0, f.1);
    let ankle = (foot.0, foot.1 - 0.08);
    let (dx, dy) = (ankle.0 - hip.0, ankle.1 - hip.1);
    let fold = (LEG - (dx * dx + dy * dy).sqrt()).max(0.0);
    let knee = (
        (hip.0 + ankle.0) * 0.5 + side * (0.014 + 0.8 * fold),
        (hip.1 + ankle.1) * 0.5 - 0.01 - 0.3 * fold,
    );
    (hip, knee, ankle, foot, side, f.1)
}

/// A boot: angular, its toe out, turned up when lifted.
fn boot_pts(foot: P, side: f32, lift: f32) -> [P; 6] {
    let turn = side * (4.0 + 50.0 * (-lift).clamp(0.0, 0.2) / 0.2);
    let heel = (foot.0 - side * 0.02, foot.1);
    [
        (-0.07, 0.0),
        (0.09, 0.0),
        (0.085, -0.04),
        (0.04, -0.088),
        (-0.045, -0.088),
        (-0.075, -0.045),
    ]
    .map(|(x, y)| rig::rotate((foot.0 + side * x, foot.1 + y), heel, turn))
}

/// The legs: graphite thighs under gunmetal plates with pearl fronts, pearl knee guards,
/// pistons behind the knees, heavy calves with seams of light, angular boots.
fn draw_legs(c: &mut Canvas, t: &Transform, b: &Body, ws: f32) {
    let leg = |i: usize| leg_of(t, b, ws, i);
    let thighs = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (hip, knee, ..) = leg(i);
            tube_into(s, hip, knee, 0.064, 0.052, g);
        }
    };
    const GUARD: [(f32, f32); 3] = [(0.1, 0.044), (0.5, 0.056), (0.88, 0.04)];
    let guard = |i: usize, s: &mut dyn Sink, g: f32| {
        let (hip, knee, ..) = leg(i);
        slab_into(s, hip, knee, &GUARD, g);
    };
    const CALF: [(f32, f32); 4] = [(0.0, 0.036), (0.3, 0.062), (0.75, 0.054), (1.0, 0.046)];
    let calf = |i: usize, s: &mut dyn Sink, g: f32| {
        let (_, knee, ankle, ..) = leg(i);
        slab_into(s, knee, ankle, &CALF, g);
    };
    let boot = |i: usize| {
        let (.., foot, side, lift) = leg(i);
        boot_pts(foot, side, lift)
    };
    let boots = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            poly_into(s, &boot(i), g);
        }
    };
    let knee_pts = |i: usize| {
        let (_, knee, ..) = leg(i);
        let p = [
            (-0.03, -0.034),
            (0.03, -0.034),
            (0.042, 0.004),
            (0.0, 0.04),
            (-0.042, 0.004),
        ];
        p.map(|(x, y)| (knee.0 + x, knee.1 + y))
    };
    let all = |s: &mut dyn Sink, g: f32| {
        thighs(s, g);
        for i in 0..2 {
            guard(i, s, g);
            calf(i, s, g);
        }
        boots(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &thighs, &pal::FRAME.look(), 1);
    let pistons = |s: &mut dyn Sink, _: f32| {
        for i in 0..2 {
            let (hip, knee, ankle, _, side, _) = leg(i);
            let a = along(hip, knee, 0.7, -side * 0.03);
            let z = along(knee, ankle, 0.32, -side * 0.03);
            ribbon_into(s, &[a, z], 0.012);
        }
    };
    d::flat(c, &pistons, pal::PISTON);
    edge::hard_each(c, 2, &guard, &pal::GUNMETAL, true);
    let fronts = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (hip, knee, _, _, side, _) = leg(i);
            let a = along(hip, knee, 0.2, side * 0.006);
            let z = along(hip, knee, 0.78, side * 0.006);
            slab_into(s, a, z, &[(0.0, 0.022), (0.5, 0.03), (1.0, 0.018)], g);
        }
    };
    d::flat(c, &fronts, pal::PEARL.base);
    edge::hard_each(c, 2, &calf, &pal::GUNMETAL, true);
    d::bands(c, &boots, &pal::GUNMETAL.look(), 2);
    glow_lines(c, &|s, g| {
        for i in 0..2 {
            let (_, knee, ankle, _, side, _) = leg(i);
            let a = along(knee, ankle, 0.22, side * 0.024);
            let z = along(knee, ankle, 0.72, side * 0.03);
            ribbon_into(s, &[a, z], 0.008 + 2.0 * g);
            let bt = boot(i);
            let sole = [
                rig::lerp_p(bt[0], bt[5], 0.3),
                rig::lerp_p(bt[1], bt[2], 0.3),
            ];
            ribbon_into(s, &sole, 0.008 + 2.0 * g);
        }
    });
    let knees = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            poly_into(s, &knee_pts(i), g);
        }
    };
    d::outline(c, &knees, &pal::OUTLINE.thin(0.8));
    d::flat(c, &knees, pal::PEARL.base);
}

/// The chest: broad and faceted at the shoulders, cut hard to the waist.
fn chest_pts(ws: f32) -> [P; 11] {
    [
        (-0.245, -1.09),
        (-0.1, -1.138),
        (0.1, -1.138),
        (0.245, -1.09),
        (0.218, -0.99),
        (0.14, -0.87),
        (0.08, -0.8),
        (0.0, -0.79),
        (-0.08, -0.8),
        (-0.14, -0.87),
        (-0.218, -0.99),
    ]
    .map(|(x, y)| (x * ws, y))
}

fn pelvis_pts(ws: f32) -> [P; 6] {
    [
        (-0.12, -0.752),
        (0.12, -0.752),
        (0.135, -0.68),
        (0.05, -0.62),
        (-0.05, -0.62),
        (-0.135, -0.68),
    ]
    .map(|(x, y)| (x * ws, y))
}

/// How hard the jets fire: on a jump's take-off, less in the air.
pub(super) fn jet_power(pose: &Pose) -> f32 {
    if pose.mv != Move::Jump {
        return 0.0;
    }
    let p = pose.phase;
    let k = rig::pulse(p, 0.62, 0.16).max(0.45 * rig::arc(rig::window(p, 0.55, 0.95)));
    k * (0.5 + 0.5 * rig::vigour(pose))
}

/// A back vane on side `side`: a swept angular blade behind the shoulder.
fn vane_pts(torso: &Ball, side: f32) -> [P; 4] {
    let root = torso.at(side * 150.0, 46.0, 1.0).at;
    let k = rig::cos_deg(torso.yaw).abs() * 0.7 + 0.3;
    let lean = rig::sin_deg(torso.yaw) * -0.05;
    let tip = (root.0 + side * 0.24 * k + lean, root.1 - 0.2);
    [
        (root.0 - side * 0.02, root.1 + 0.03),
        (root.0 + side * 0.05 * k, root.1 - 0.04),
        tip,
        (root.0 + side * 0.08 * k, root.1 + 0.02),
    ]
}

fn draw_vanes(c: &mut Canvas, torso: &Ball) {
    let vane = |k: usize, s: &mut dyn Sink, g: f32| {
        let side = if k == 0 { -1.0 } else { 1.0 };
        poly_into(s, &vane_pts(torso, side), g);
    };
    let both = |s: &mut dyn Sink, g: f32| {
        vane(0, s, g);
        vane(1, s, g);
    };
    d::outline(c, &both, &pal::OUTLINE);
    edge::hard_each(c, 2, &vane, &pal::GUNMETAL, true);
    glow_lines(c, &|s, g| {
        for side in [-1.0f32, 1.0] {
            let v = vane_pts(torso, side);
            ribbon_into(
                s,
                &[rig::lerp_p(v[1], v[2], 0.15), rig::lerp_p(v[1], v[2], 0.85)],
                0.008 + 2.0 * g,
            );
        }
    });
}

/// The two jets' flames, from nozzles low on its back (in the torso's frame).
fn draw_jets(c: &mut Canvas, torso: &Ball, power: f32, pose: &Pose) {
    if power <= 0.02 {
        return;
    }
    let flick = 1.0 + 0.12 * cantus_dsp::math::sin(pose.seconds as f32 * 47.0);
    let flame = |s: &mut dyn Sink, k: f32| {
        for side in [-1.0f32, 1.0] {
            let n = torso.at(side * 150.0, -40.0, 1.0).at;
            let len = (0.14 + 0.46 * power) * flick * k;
            let w = 0.044 * (0.6 + 0.4 * power) * k.sqrt();
            let pts = [
                (n.0 - w, n.1),
                (n.0 + w, n.1),
                (n.0 + w * 0.4, n.1 + len * 0.6),
                (n.0, n.1 + len),
                (n.0 - w * 0.4, n.1 + len * 0.6),
            ];
            d::blob_into(s, &pts);
        }
    };
    d::fixed(c, &|s, _| flame(s, 1.0), pal::JET);
    d::fixed(c, &|s, _| flame(s, 0.55), pal::JET_HOT);
}

/// The front of the torso: waist, hips, neck, the faceted chest with pearl plates, seams
/// of light, vents and the speaker core that pulses on the beat; or the back pack.
fn draw_front(c: &mut Canvas, torso: &Ball, ws: f32, pose: &Pose) {
    let waist = |s: &mut dyn Sink, g: f32| d::rect_into(s, (0.0, -0.778), 0.13 * ws, 0.07, 0.0, g);
    d::part(c, &waist, &pal::FRAME.look(), &pal::OUTLINE, 1);
    let pelvis = |s: &mut dyn Sink, g: f32| poly_into(s, &pelvis_pts(ws), g);
    edge::part(c, &pelvis, &pal::GUNMETAL, &pal::OUTLINE, true);
    let neck = |s: &mut dyn Sink, g: f32| d::rect_into(s, (0.0, -1.14), 0.07, 0.07, 0.0, g);
    d::part(c, &neck, &pal::FRAME.look(), &pal::OUTLINE, 1);
    let chest = |s: &mut dyn Sink, g: f32| poly_into(s, &chest_pts(ws), g);
    edge::part(c, &chest, &pal::GUNMETAL, &pal::OUTLINE, true);
    let front = torso.at(0.0, 4.0, 1.0);
    let back = torso.at(180.0, 4.0, 1.0);
    if front.z > 0.25 {
        let sq = 0.35 + 0.65 * front.squeeze;
        let facing = front.z > 0.5;
        let fx = front.at.0 * 0.85;
        // Pearl plates over the chest, either side of the core.
        let plate = |k: usize, s: &mut dyn Sink, g: f32| {
            let side = if k == 0 { -1.0 } else { 1.0 };
            let pts = [
                (fx + side * 0.04 * sq, -1.115),
                (fx + side * 0.2 * ws * sq, -1.084),
                (fx + side * 0.17 * sq, -0.99),
                (fx + side * 0.07 * sq, -0.96),
            ];
            poly_into(s, &pts, g);
        };
        let plates = |s: &mut dyn Sink, g: f32| {
            plate(0, s, g);
            plate(1, s, g);
        };
        d::outline(c, &plates, &pal::OUTLINE.thin(0.7));
        edge::hard_each(c, 2, &plate, &pal::PEARL, false);
        if facing {
            // The abs: three dark plates, and the seams of light to the hips.
            let abs = |s: &mut dyn Sink, _: f32| {
                for k in 0..3 {
                    let y = -0.905 + 0.034 * k as f32;
                    let w = (0.07 - 0.01 * k as f32) * sq;
                    rounded_into(s, (fx, y), w, 0.026, 0.006, 0.0);
                }
            };
            d::flat(c, &abs, pal::FRAME.base);
            glow_lines(c, &|s, g| {
                for side in [-1.0f32, 1.0] {
                    let pts = [
                        (fx + side * 0.06 * sq, -0.95),
                        (fx + side * 0.1 * sq, -0.87),
                        (fx + side * 0.07 * sq, -0.8),
                    ];
                    ribbon_into(s, &pts, 0.007 + 2.0 * g);
                }
            });
            let vents = |s: &mut dyn Sink, _: f32| {
                for side in [-1.0f32, 1.0] {
                    for k in 0..3 {
                        let y = -1.112 + 0.016 * k as f32;
                        let x = fx + side * (0.07 + 0.008 * k as f32) * sq;
                        d::rect_into(s, (x, y), 0.03 * sq, 0.007, side * 8.0, 0.0);
                    }
                }
            };
            d::flat(c, &vents, pal::FRAME.base);
        }
        // The speaker core: a hexagonal housing, a coral light that swells on the beat.
        let core_c = (fx, -1.0);
        let hex = |r: f32, s: &mut dyn Sink| {
            let pts: [P; 6] = std::array::from_fn(|k| {
                let a = 60.0 * k as f32 + 30.0;
                (
                    core_c.0 + r * rig::cos_deg(a) * sq,
                    core_c.1 + r * rig::sin_deg(a),
                )
            });
            d::polygon_into(s, &pts);
        };
        d::outline(c, &|s, g| hex(0.05 + g, s), &pal::OUTLINE.thin(0.8));
        d::flat(c, &|s, _| hex(0.05, s), pal::FRAME.base);
        let beat = if pose.playing {
            rig::pulse(pose.beat_phase, 0.0, 0.3)
        } else {
            0.0
        };
        let r = 0.028 + 0.008 * beat;
        d::fixed(c, &|s, _| hex(r + 0.008, s), pal::GLOW_HALO);
        d::fixed(c, &|s, _| hex(r, s), pal::GLOW);
        d::fixed(c, &|s, _| hex(r * 0.45, s), pal::GLOW_HOT);
        // Sound rings leave it on the beat, at energy.
        let strength = crate::rig::dance::fx_strength(pose);
        let age = pose.beat_phase / 0.6;
        if pose.playing && strength > 0.0 && age < 1.0 {
            let rad = 0.07 + 0.1 * age;
            let col = d::alpha(pal::GLOW, (1.0 - age) * strength * 0.85);
            for side in [-1.0f32, 1.0] {
                let pts: [P; 5] = std::array::from_fn(|k| {
                    let a = -36.0 + 18.0 * k as f32;
                    (
                        core_c.0 + side * rad * rig::cos_deg(a) * sq,
                        core_c.1 + rad * rig::sin_deg(a),
                    )
                });
                d::ribbon(c, &pts, 0.01, col, false);
            }
        }
    } else if back.z > 0.25 {
        let sq = 0.35 + 0.65 * back.squeeze;
        let bx = back.at.0 * 0.85;
        let pack = |s: &mut dyn Sink, g: f32| {
            let p = [
                (-0.1, -1.07),
                (0.1, -1.07),
                (0.115, -0.93),
                (0.07, -0.85),
                (-0.07, -0.85),
                (-0.115, -0.93),
            ];
            poly_into(s, &p.map(|(x, y)| (bx + x * sq, y)), g);
        };
        edge::part(c, &pack, &pal::FRAME, &pal::OUTLINE, true);
        let nozzles = |s: &mut dyn Sink, g: f32| {
            for side in [-1.0f32, 1.0] {
                d::rect_into(s, (bx + side * 0.05 * sq, -0.875), 0.05 * sq, 0.04, 0.0, g);
            }
        };
        d::part(
            c,
            &nozzles,
            &pal::GUNMETAL.look(),
            &pal::OUTLINE.thin(0.8),
            2,
        );
        glow_lines(c, &|s, g| {
            ribbon_into(
                s,
                &[(bx - 0.07 * sq, -1.04), (bx + 0.07 * sq, -1.04)],
                0.008 + 2.0 * g,
            );
            for side in [-1.0f32, 1.0] {
                let p = (bx + side * 0.05 * sq, -0.862);
                ribbon_into(
                    s,
                    &[(p.0 - 0.016 * sq, p.1), (p.0 + 0.016 * sq, p.1)],
                    0.008 + 2.0 * g,
                );
            }
        });
    }
}

/// How the head shows turned by `yaw`: its width, how far the face has moved toward the
/// side it turns to, how much of the face shows (0 edge-on … 1 facing us; < 0 away).
#[derive(Copy, Clone)]
pub(super) struct HeadView {
    pub c: P,
    pub w: f32,
    pub fx: f32,
    pub sq: f32,
    pub facing: f32,
}

pub(super) fn head_view(yaw: f32) -> HeadView {
    let (s, k) = (rig::sin_deg(yaw), rig::cos_deg(yaw));
    HeadView {
        c: HEAD.0,
        w: 0.86 + 0.14 * k.abs(),
        fx: s * 0.075,
        sq: 0.3 + 0.7 * k.max(0.0),
        facing: k,
    }
}

/// The helmet's outline: a faceted crown, a crest, cheeks cut in to a sculpted jaw; the
/// face juts toward the way it turns.
fn helmet_pts(v: &HeadView) -> [P; 13] {
    const PTS: [(f32, f32, f32); 13] = [
        (0.0, -0.135, 0.5),
        (0.062, -0.12, 0.2),
        (0.104, -0.072, 0.0),
        (0.114, 0.006, 0.0),
        (0.1, 0.07, 0.3),
        (0.066, 0.106, 0.6),
        (0.026, 0.122, 0.9),
        (-0.026, 0.122, 0.9),
        (-0.066, 0.106, 0.6),
        (-0.1, 0.07, 0.3),
        (-0.114, 0.006, 0.0),
        (-0.104, -0.072, 0.0),
        (-0.062, -0.12, 0.2),
    ];
    PTS.map(|(x, y, j)| (v.c.0 + x * v.w + v.fx * j * 0.55, v.c.1 + y))
}

/// A blade swept flat back from the temple on side `side`.
fn blade_pts(v: &HeadView, yaw: f32, side: f32) -> [P; 4] {
    let s = rig::sin_deg(yaw);
    let k = rig::cos_deg(yaw).abs();
    let root = (v.c.0 + side * 0.09 * v.w, v.c.1 - 0.05);
    let tip = (root.0 + side * 0.07 * k - s * 0.17, root.1 - 0.05);
    [
        (root.0, root.1 + 0.03),
        (root.0 + side * 0.01 * k - s * 0.02, root.1 - 0.025),
        tip,
        (
            rig::lerp(root.0, tip.0, 0.55),
            rig::lerp(root.1, tip.1, 0.4) + 0.012,
        ),
    ]
}

fn draw_blades(c: &mut Canvas, v: &HeadView, yaw: f32, near: bool) {
    let s = rig::sin_deg(yaw);
    // Turned toward its own left (positive yaw), its right side (−1) is the far one.
    let mine = |side: f32| {
        if s.abs() <= 0.2 {
            near
        } else {
            (side == s.signum()) != near
        }
    };
    let blade = |k: usize, sk: &mut dyn Sink, g: f32| {
        let side = if k == 0 { -1.0 } else { 1.0 };
        if mine(side) {
            poly_into(sk, &blade_pts(v, yaw, side), g);
        }
    };
    let both = |sk: &mut dyn Sink, g: f32| {
        blade(0, sk, g);
        blade(1, sk, g);
    };
    d::outline(c, &both, &pal::OUTLINE);
    edge::hard_each(c, 2, &blade, &pal::PEARL, true);
    glow_lines(c, &|sk, g| {
        for side in [-1.0f32, 1.0] {
            if mine(side) {
                let b = blade_pts(v, yaw, side);
                ribbon_into(
                    sk,
                    &[rig::lerp_p(b[3], b[2], 0.0), rig::lerp_p(b[3], b[2], 0.8)],
                    0.006 + 2.0 * g,
                );
            }
        }
    });
}

/// The narrow V visor: two slits meeting low in the middle (points: upper edge left to
/// right, then the lower edge back).
pub(super) fn visor_pts(v: &HeadView) -> [P; 6] {
    let (x, y, q) = (v.c.0 + v.fx, v.c.1, v.sq);
    [
        (x - 0.096 * q, y - 0.03),
        (x, y + 0.004),
        (x + 0.096 * q, y - 0.03),
        (x + 0.09 * q, y - 0.008),
        (x, y + 0.026),
        (x - 0.09 * q, y - 0.008),
    ]
}

/// The helmet, in the head's frame.
fn draw_head(c: &mut Canvas, b: &Body, pose: &Pose) {
    let v = head_view(b.yaw);
    draw_blades(c, &v, b.yaw, false);
    let helmet = |s: &mut dyn Sink, g: f32| poly_into(s, &helmet_pts(&v), g);
    edge::part(c, &helmet, &pal::GUNMETAL, &pal::OUTLINE, true);
    // The pearl crest down the brow.
    let cx = v.c.0 + v.fx * 0.4;
    let crest = |s: &mut dyn Sink, g: f32| {
        let p = [
            (cx - 0.016, v.c.1 - 0.05),
            (cx + 0.016, v.c.1 - 0.05),
            (cx + 0.006, v.c.1 - 0.128),
            (cx - 0.006, v.c.1 - 0.128),
        ];
        poly_into(s, &p, g);
    };
    d::flat(c, &crest, pal::PEARL.base);
    if v.facing > -0.02 {
        let (x, y, q) = (v.c.0 + v.fx, v.c.1, v.sq);
        // The sculpted faceplate under the visor.
        let plate = |s: &mut dyn Sink, g: f32| {
            let p = [
                (x - 0.088 * q, y - 0.004),
                (x + 0.088 * q, y - 0.004),
                (x + 0.074 * q, y + 0.07),
                (x + 0.026 * q, y + 0.112),
                (x - 0.026 * q, y + 0.112),
                (x - 0.074 * q, y + 0.07),
            ];
            poly_into(s, &p, g);
        };
        d::outline(c, &plate, &pal::OUTLINE.thin(0.7));
        edge::hard(c, &plate, &pal::PEARL, false);
        // Three slits in the chin.
        let slits = |s: &mut dyn Sink, _: f32| {
            for k in [-1.0f32, 0.0, 1.0] {
                d::rect_into(s, (x + k * 0.014 * q, y + 0.09), 0.006 * q, 0.022, 0.0, 0.0);
            }
        };
        d::flat(c, &slits, pal::FRAME.base);
        // The visor.
        let visor = |s: &mut dyn Sink, g: f32| poly_into(s, &visor_pts(&v), g);
        d::outline(c, &visor, &pal::OUTLINE);
        d::fixed(c, &visor, pal::VISOR);
        face::leds(c, &v, b, pose);
    }
    draw_blades(c, &v, b.yaw, true);
}

pub(super) fn draw(m: &Mecha, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = crate::cast::edge::ground(c, pose);
    let t = kit::torso(b, ANCHOR_Y);
    let tt = base.pre_concat(t);
    let torso = ball(TORSO, kit::torso_yaw(b.yaw));
    let head_local = t.pre_concat(rig::rotation_at(b.tilt, 0.0, -1.12));
    let head_t = base.pre_concat(head_local);
    let ws = width_of(torso.yaw);
    kit::shadow(c, ground, pose, b, 0.34, pal::SHADOW);
    let power = jet_power(pose);
    let away = torso.at(180.0, 0.0, 1.0).z > 0.0;
    if !away {
        kit::with(c, tt, |c| {
            draw_jets(c, &torso, power, pose);
            draw_vanes(c, &torso);
        });
    }
    let arm_z = |i: usize| torso.at(if i == 0 { -84.0 } else { 84.0 }, 30.0, 1.0).z;
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
    kit::with(c, tt, |c| {
        draw_front(c, &torso, ws, pose);
        if away {
            draw_vanes(c, &torso);
            draw_jets(c, &torso, power, pose);
        }
        draw_arms(c, &torso, b, &near[..nn]);
    });
    kit::with(c, head_t, |c| draw_head(c, b, pose));
    let hand = |bd: &Body, mt: &Transform, i: usize| {
        let tb = ball(TORSO, kit::torso_yaw(bd.yaw));
        kit::map(mt, arm_of(&tb, bd, i).0[2])
    };
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let p = kit::map(&head_local, (HEAD.0 .0, HEAD.0 .1 + 0.06));
                (p.0 + side * 0.28, p.1 - 0.12)
            }
            Anchor::Top => kit::map(&head_local, (HEAD.0 .0, HEAD.0 .1 - 0.24)),
            Anchor::Hand(i) => hand(b, &t, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &t, 0), hand(b, &t, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let mb = kit::body_at(pose, &m.style, 0.0, 40.0);
        let mt = kit::torso(&mb, ANCHOR_Y);
        rig::lerp_p(hand(&mb, &mt, 0), hand(&mb, &mt, 1), 0.5)
    };
    let look = FxLook {
        ink: pal::OUTLINE,
        sparkle: pal::SPARKLE,
        note: pal::NOTE,
        dust: pal::DUST,
        heart: pal::NOTE,
        confetti: pal::CONFETTI,
    };
    kit::extras(c, pose, b, ground, &look, &anchor, &met);
}
