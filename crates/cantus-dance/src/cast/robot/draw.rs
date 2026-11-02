//! Drawing the robot from a [`Body`]: shadow, the wind-up key (behind unless it turns
//! away), a far arm, the piston legs, the metronome shell with its copper trim and
//! antenna, the screen and its LED face, the pendulum in its slot, the near arms, extras.

use super::{pal, Robot};
use crate::cast::kit::{self, FxLook};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Anchor, Body};
use crate::rig::draw::{self as d, ellipse_into, rect_into, ribbon_into, Sink};
use crate::rig::face::{self, Face};
use crate::rig::{self, fx, Ball, OnBall, P};
use cantus_core::inst::dancer::Mood;
use tiny_skia::Transform;

const ANCHOR_Y: f32 = -0.12;
/// The ball its features sit on (the shell is drawn as a rounded metronome around it).
const BALL: (P, f32, f32) = ((0.0, -0.5), 0.27, 0.375);

fn ball(yaw: f32) -> Ball {
    Ball {
        center: BALL.0,
        rx: BALL.1,
        ry: BALL.2,
        yaw,
    }
}

/// How wide the shell looks turned by `yaw` (a metronome is deeper than a ball is flat).
fn width_of(yaw: f32) -> f32 {
    0.78 + 0.22 * rig::cos_deg(yaw).abs()
}

/// The shell: a rounded trapezoid, wide at the base.
fn shell_into(s: &mut dyn Sink, ws: f32, g: f32) {
    d::blob_grown_into(
        s,
        &[
            (-0.16 * ws, -0.88),
            (0.0, -0.915),
            (0.16 * ws, -0.88),
            (0.24 * ws, -0.5),
            (0.29 * ws, -0.18),
            (0.0, -0.12),
            (-0.29 * ws, -0.18),
            (-0.24 * ws, -0.5),
        ],
        g,
    );
}

/// A rounded rectangle centred on `c` (`w` × `h`, corners `r`), grown by `g`.
fn rounded_into(s: &mut dyn Sink, c: P, w: f32, h: f32, r: f32, g: f32) {
    let (hw, hh) = (w * 0.5 + g, h * 0.5 + g);
    let r = (r + g).min(hw).min(hh);
    let k = r * (1.0 - 0.552_284_7);
    s.move_to(c.0 - hw + r, c.1 - hh);
    s.line_to(c.0 + hw - r, c.1 - hh);
    s.cubic_to(
        c.0 + hw - k,
        c.1 - hh,
        c.0 + hw,
        c.1 - hh + k,
        c.0 + hw,
        c.1 - hh + r,
    );
    s.line_to(c.0 + hw, c.1 + hh - r);
    s.cubic_to(
        c.0 + hw,
        c.1 + hh - k,
        c.0 + hw - k,
        c.1 + hh,
        c.0 + hw - r,
        c.1 + hh,
    );
    s.line_to(c.0 - hw + r, c.1 + hh);
    s.cubic_to(
        c.0 - hw + k,
        c.1 + hh,
        c.0 - hw,
        c.1 + hh - k,
        c.0 - hw,
        c.1 + hh - r,
    );
    s.line_to(c.0 - hw, c.1 - hh + r);
    s.cubic_to(
        c.0 - hw,
        c.1 - hh + k,
        c.0 - hw + k,
        c.1 - hh,
        c.0 - hw + r,
        c.1 - hh,
    );
    s.close();
}

/// The screen and its LED face (every mood as a glyph), on the front of the shell.
fn draw_screen(c: &mut Canvas, bl: &Ball, b: &Body, pose: &Pose) {
    // LEDs are light, not paint: never turned by the hue (a turned amber passes near the
    // green key).
    let hue = c.hue;
    c.hue = 0.0;
    draw_screen_lit(c, bl, b, pose);
    c.hue = hue;
}

fn draw_screen_lit(c: &mut Canvas, bl: &Ball, b: &Body, pose: &Pose) {
    let on = bl.at(0.0, 27.0 + b.nod * 0.3, 1.0);
    if on.z <= 0.12 {
        return;
    }
    let sq = 0.3 + 0.7 * on.squeeze;
    let (w, h) = (0.38 * sq, 0.235);
    let sc = (on.at.0 * 0.9, on.at.1);
    let screen = |s: &mut dyn Sink, g: f32| rounded_into(s, sc, w, h, 0.05, g);
    d::outline(c, &screen, &pal::OUTLINE);
    d::fixed(c, &screen, pal::SCREEN);
    // A glare across the glass.
    let glare = [
        (sc.0 - w * 0.4, sc.1 - h * 0.18),
        (sc.0 - w * 0.2, sc.1 - h * 0.38),
    ];
    d::fixed(c, &|s, _| ribbon_into(s, &glare, 0.018), pal::SCREEN_LIGHT);
    let f: Face = face::face_of(pose.mood);
    let look = (
        (b.look.0 * 0.8 + f.look.0).clamp(-1.0, 1.0) * 0.02,
        (b.look.1 * 0.8 + f.look.1).clamp(-1.0, 1.0) * 0.015,
    );
    let eyes = [
        (sc.0 - 0.085 * sq, sc.1 - 0.025),
        (sc.0 + 0.085 * sq, sc.1 - 0.025),
    ];
    let happy = f.arcs || f.lower > 0.15 || b.squint > 0.45;
    let leds = |s: &mut dyn Sink, g: f32| {
        if f.shades {
            rounded_into(s, (sc.0, sc.1 - 0.02), 0.24 * sq, 0.04, 0.02, g);
        } else {
            for (i, e) in eyes.iter().enumerate() {
                let e = (e.0 + look.0, e.1 + look.1);
                let shut = (f.upper + pose.blink * (1.0 - f.upper)).max(b.wink[i]);
                if happy || shut > 0.9 {
                    let k = if happy { -0.03 } else { 0.0 };
                    ribbon_into(
                        s,
                        &[
                            (e.0 - 0.03, e.1 + 0.012),
                            (e.0, e.1 + k),
                            (e.0 + 0.03, e.1 + 0.012),
                        ],
                        0.02 + 2.0 * g,
                    );
                } else if !f.star {
                    let r = 0.03 * f.eye;
                    let hh = (0.085 * f.eye * (1.0 - 0.9 * shut)).max(0.012);
                    let ww = if f.round { 0.07 * f.eye } else { 0.05 * f.eye };
                    rounded_into(s, e, ww * sq.max(0.5), hh, r.min(hh * 0.5), g);
                }
            }
        }
        // The mouth.
        let o = f.mouth.max(b.mouth);
        let m = (sc.0, sc.1 + 0.06);
        if f.round && o > 0.1 {
            ellipse_into(s, m, 0.022 + g, 0.022 + g, 0.0);
        } else if o > 0.12 {
            rounded_into(s, m, 0.07 * sq.max(0.5), 0.012 + 0.03 * o, 0.012, g);
        } else {
            let up = 0.015 * f.smile;
            let tilt = 0.01 * f.smirk;
            ribbon_into(
                s,
                &[
                    (m.0 - 0.04, m.1 - up - tilt),
                    (m.0, m.1 + up * 0.4),
                    (m.0 + 0.04, m.1 - up + tilt),
                ],
                0.014 + 2.0 * g,
            );
        }
    };
    d::flat(c, &|s, _| leds(s, 0.008), pal::LED_GLOW);
    d::flat(c, &|s, _| leds(s, 0.0), pal::LED);
    if f.star {
        for e in eyes {
            fx::sparkle(
                c,
                (e.0 + look.0, e.1 + look.1),
                0.08,
                0.5,
                pal::LED,
                &d::Ink {
                    width: 0.0,
                    ..pal::OUTLINE
                },
            );
        }
    }
    if f.cheeks > 0.5 || pose.mood == Mood::Shy {
        let blush = |s: &mut dyn Sink, _: f32| {
            for e in eyes {
                ribbon_into(
                    s,
                    &[(e.0 - 0.02, e.1 + 0.05), (e.0 + 0.02, e.1 + 0.05)],
                    0.012,
                );
            }
        };
        d::flat(c, &blush, pal::LED_PINK);
    }
}

/// The pendulum's angle (degrees): at one end on every beat, the other on the next,
/// counted by the song's own position in beats (`Pose::beats`), so it swings on through
/// bar lines and changes of metre; stopped, a slow sway.
pub(super) fn pendulum_deg(pose: &Pose) -> f32 {
    if !pose.playing {
        return 4.0 * cantus_dsp::math::sin(pose.seconds as f32 * 1.3);
    }
    let two = pose.beats.rem_euclid(2.0) as f32;
    26.0 * rig::cos_deg(180.0 * two)
}

/// The pendulum's slot on the front, and its rod and weight ticking with the beats.
fn draw_pendulum(c: &mut Canvas, bl: &Ball, pose: &Pose) {
    let on: OnBall = bl.at(0.0, -32.0, 1.0);
    if on.z <= 0.15 {
        return;
    }
    let sq = 0.3 + 0.7 * on.squeeze;
    let slot_c = (on.at.0 * 0.9, on.at.1 - 0.01);
    let slot = |s: &mut dyn Sink, g: f32| rounded_into(s, slot_c, 0.075 * sq, 0.24, 0.035, g);
    d::outline(c, &slot, &pal::OUTLINE.thin(0.7));
    d::fixed(c, &slot, pal::SCREEN);
    // The metronome's scale: copper ticks on both sides of the slot.
    let ticks = |s: &mut dyn Sink, _: f32| {
        for k in 0..5 {
            let y = slot_c.1 - 0.09 + 0.045 * k as f32;
            let len = if k % 2 == 0 { 0.03 } else { 0.018 };
            for side in [-1.0f32, 1.0] {
                let x = slot_c.0 + side * (0.06 * sq);
                ribbon_into(s, &[(x, y), (x + side * len * sq, y)], 0.01);
            }
        }
    };
    d::flat(c, &ticks, pal::COPPER.shade);
    // Side to side on every beat, at its ends on the beats.
    let theta = pendulum_deg(pose) * sq;
    let pivot = (slot_c.0, slot_c.1 + 0.105);
    let tip = rig::reach(pivot, 0.22, 180.0 + theta);
    let wt = rig::lerp_p(pivot, tip, 0.62);
    let rod = |s: &mut dyn Sink, g: f32| ribbon_into(s, &[pivot, tip], 0.016 + 2.0 * g);
    d::part(c, &rod, &pal::METAL, &pal::OUTLINE.thin(0.6), 1);
    let weight = |s: &mut dyn Sink, g: f32| rect_into(s, wt, 0.05, 0.035, -theta, g);
    d::part(c, &weight, &pal::COPPER, &pal::OUTLINE.thin(0.7), 2);
    d::fixed(
        c,
        &|s, _| ellipse_into(s, pivot, 0.014, 0.014, 0.0),
        pal::INK,
    );
}

/// The wind-up key in its back, turning slowly.
fn draw_key(c: &mut Canvas, bl: &Ball, pose: &Pose) {
    let on = bl.at(180.0, -5.0, 1.0);
    let side = rig::sin_deg(180.0 + bl.yaw);
    let out = if on.z > 0.0 { 0.0 } else { side * 0.12 };
    let root = on.at;
    let end = (root.0 + out, root.1);
    let turn = (pose.seconds as f32 * 90.0).rem_euclid(360.0);
    let face_on = rig::cos_deg(turn).abs().max(0.25);
    let key = |s: &mut dyn Sink, g: f32| {
        ribbon_into(s, &[root, end], 0.03 + 2.0 * g);
        let lobe = 0.075 * face_on;
        let hc = (end.0 + side.signum() * 0.03 * (1.0 - on.z.max(0.0)), end.1);
        ellipse_into(s, (hc.0, hc.1 - 0.07), lobe + g, 0.06 + g, 0.0);
        ellipse_into(s, (hc.0, hc.1 + 0.07), lobe + g, 0.06 + g, 0.0);
    };
    d::part(c, &key, &pal::COPPER, &pal::OUTLINE, 2);
}

/// The antenna on top, swaying late, its bulb lit on the accent.
fn draw_antenna(c: &mut Canvas, b: &Body, pose: &Pose) {
    let root = (0.0, -0.92);
    let tip = rig::reach(root, 0.16, 180.0 - b.top * 0.9);
    let mid = rig::reach(root, 0.08, 180.0 - b.top * 0.4);
    let rod = |s: &mut dyn Sink, g: f32| ribbon_into(s, &[root, mid, tip], 0.022 + 2.0 * g);
    d::part(c, &rod, &pal::METAL, &pal::OUTLINE.thin(0.8), 1);
    let lit = pose.accent > 0.35;
    // A lit bulb is an LED: never turned by the hue.
    let hue = c.hue;
    if lit {
        c.hue = 0.0;
    }
    let bulb = |s: &mut dyn Sink, g: f32| ellipse_into(s, tip, 0.042 + g, 0.042 + g, 0.0);
    if lit {
        d::flat(
            c,
            &|s, _| ellipse_into(s, tip, 0.07, 0.07, 0.0),
            d::alpha(pal::LED, 0.35),
        );
    }
    let look = if lit {
        d::Look {
            base: pal::LED,
            shade: d::mix(pal::LED, pal::COPPER.base, 0.4),
            light: [255, 240, 190, 255],
            rim: [0, 0, 0, 0],
        }
    } else {
        pal::COPPER
    };
    d::part(c, &bulb, &look, &pal::OUTLINE.thin(0.8), 3);
    c.hue = hue;
}

/// Arms (the ones `which`): metal hoses from the shoulders, round ivory mitts.
fn draw_arms(c: &mut Canvas, bl: &Ball, b: &Body, which: &[usize]) {
    let arm = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = bl.at(side * 84.0, 20.0, 0.95).at;
        let pts = kit::arm_points(sh, b.arm[i], b.bend[i], 0.14, 0.13, i);
        (pts, kit::forearm_deg(b.arm[i], b.bend[i], i))
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            kit::hose_into(s, arm(i).0, 0.055, 0.05, g);
        }
    };
    let mitts = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, deg) = arm(i);
            kit::mitten_into(s, pts[2], deg, 0.065, b.spread[i], i, g);
        }
    };
    let joints = |s: &mut dyn Sink, g: f32| {
        for &i in which {
            let (pts, _) = arm(i);
            ellipse_into(s, pts[0], 0.04 + g, 0.04 + g, 0.0);
            ellipse_into(s, pts[1], 0.032 + g, 0.032 + g, 0.0);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        mitts(s, g);
        joints(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::METAL, 2);
    d::bands(c, &joints, &pal::COPPER, 2);
    d::bands(c, &mitts, &pal::SHELL, 2);
}

/// Piston legs from under the shell to copper feet.
fn draw_legs(c: &mut Canvas, t: &Transform, b: &Body, ws: f32) {
    let leg = |i: usize| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = kit::map(t, (side * 0.13 * ws, -0.15));
        let f = b.foot[i];
        let foot = (side * 0.13 + f.0, f.1);
        let knee = ((hip.0 + foot.0) * 0.5, (hip.1 + foot.1) * 0.5);
        ([hip, knee, (foot.0, foot.1 - 0.03)], foot, side, f.1)
    };
    let limbs = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            kit::hose_into(s, leg(i).0, 0.065, 0.06, g);
        }
    };
    let feet = |s: &mut dyn Sink, g: f32| {
        for i in 0..2 {
            let (_, foot, side, lift) = leg(i);
            kit::shoe_into(s, foot, 0.14, 0.065, side * 0.5, lift, g);
        }
    };
    let all = |s: &mut dyn Sink, g: f32| {
        limbs(s, g);
        feet(s, g);
    };
    d::outline(c, &all, &pal::OUTLINE);
    d::bands(c, &limbs, &pal::METAL, 2);
    d::bands(c, &feet, &pal::COPPER, 2);
}

pub(super) fn draw(robot: &Robot, pose: &Pose, b: &Body, c: &mut Canvas) {
    let base = c.transform;
    let ground = rig::ground(c, pose, true);
    let t = kit::torso(b, ANCHOR_Y);
    let tt = base.pre_concat(t);
    let bl = ball(b.yaw);
    let ws = width_of(b.yaw);
    kit::shadow(c, ground, pose, b, 0.34, pal::SHADOW);
    let key_z = bl.at(180.0, -5.0, 1.0).z;
    let arm_z = |i: usize| bl.at(if i == 0 { -84.0 } else { 84.0 }, 20.0, 1.0).z;
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
        if key_z <= 0.0 {
            draw_key(c, &bl, pose);
        }
        draw_arms(c, &bl, b, &far[..nf]);
    });
    draw_legs(c, &t, b, ws);
    kit::with(c, tt, |c| {
        draw_antenna(c, b, pose);
        d::part(
            c,
            &|s, g| shell_into(s, ws, g),
            &pal::SHELL,
            &pal::OUTLINE,
            3,
        );
        let band =
            |s: &mut dyn Sink, g: f32| rounded_into(s, (0.0, -0.16), 0.6 * ws, 0.08, 0.04, g);
        d::part(c, &band, &pal::COPPER, &pal::OUTLINE.thin(0.8), 2);
        let cap =
            |s: &mut dyn Sink, g: f32| ellipse_into(s, (0.0, -0.89), 0.13 * ws + g, 0.05 + g, 0.0);
        d::part(c, &cap, &pal::COPPER, &pal::OUTLINE.thin(0.8), 2);
        let face_t = c
            .transform
            .pre_concat(crate::rig::rotation_at(b.tilt * 0.6, 0.0, -0.66));
        kit::with(c, face_t, |c| draw_screen(c, &bl, b, pose));
        draw_pendulum(c, &bl, pose);
        if key_z > 0.0 {
            draw_key(c, &bl, pose);
        }
        draw_arms(c, &bl, b, &near[..nn]);
    });
    let hand = |m: &Body, mt: &Transform, i: usize| {
        let mb = ball(m.yaw);
        let side = if i == 0 { -1.0 } else { 1.0 };
        let sh = mb.at(side * 84.0, 20.0, 0.95).at;
        kit::map(
            mt,
            kit::arm_points(sh, m.arm[i], m.bend[i], 0.14, 0.18, i)[2],
        )
    };
    let anchor = |at: Anchor| -> P {
        match at {
            Anchor::Mouth(side) => {
                let m = kit::map(&t, bl.at(0.0, 27.0, 1.0).at);
                (m.0 + side * 0.32, m.1 - 0.12)
            }
            Anchor::Top => kit::map(&t, (0.0, -1.18)),
            Anchor::Hand(i) => hand(b, &t, i.min(1)),
            Anchor::Hands => rig::lerp_p(hand(b, &t, 0), hand(b, &t, 1), 0.5),
            Anchor::Feet => (b.x, 0.0),
        }
    };
    let met = || {
        let m = kit::body_at(pose, &robot.style, 0.0, 40.0);
        let mt = kit::torso(&m, ANCHOR_Y);
        rig::lerp_p(hand(&m, &mt, 0), hand(&m, &mt, 1), 0.5)
    };
    let look = FxLook {
        ink: pal::OUTLINE,
        sparkle: pal::SPARKLE,
        note: pal::NOTE,
        dust: pal::DUST,
        heart: pal::HEART,
        confetti: pal::CONFETTI,
    };
    kit::extras(c, pose, b, ground, &look, &anchor, &met);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pose in a song of one metre throughout (`beats` quarter notes a bar).
    fn at(bar: i32, beat: u32, beats: u32, phase: f32) -> Pose {
        at_beats(
            f64::from(bar) * f64::from(beats) + f64::from(beat) + f64::from(phase),
            bar,
            beat,
            beats,
            phase,
        )
    }

    fn at_beats(song: f64, bar: i32, beat: u32, beats: u32, phase: f32) -> Pose {
        Pose {
            playing: true,
            beats: song,
            bar,
            beat_in_bar: beat,
            beats_in_bar: beats,
            beat_phase: phase,
            ..Pose::default()
        }
    }

    #[test]
    fn the_pendulum_swings_on_through_the_bar_line_in_any_metre() {
        for beats in [3u32, 4, 5, 7] {
            for bar in [0, 1, 6, 7] {
                let before = pendulum_deg(&at(bar, beats - 1, beats, 0.999));
                let after = pendulum_deg(&at(bar + 1, 0, beats, 0.0));
                assert!(
                    (before - after).abs() < 0.5,
                    "{beats}/4, bar {bar}: {before} then {after}"
                );
            }
            // And it reaches an end on every beat, alternating.
            let a = pendulum_deg(&at(2, 0, beats, 0.0));
            let b = pendulum_deg(&at(2, 1, beats, 0.0));
            assert!((a + b).abs() < 1e-3 && a.abs() > 25.0);
        }
    }

    #[test]
    fn the_pendulum_swings_on_through_a_change_of_metre() {
        // Five bars of 4/4 (beats 0…19), two of 3/4 (20…25), then 7/8 (from 26, three and a
        // half quarter notes a bar): at each change the last beat's end and the next beat's
        // start are the same swing.
        let changes = [(4, 4u32, 20.0, 5, 3u32), (6, 3, 26.0, 7, 7)];
        for (bar, beats, at_change, next_bar, next_beats) in changes {
            let before = pendulum_deg(&at_beats(at_change - 0.001, bar, beats - 1, beats, 0.999));
            let after = pendulum_deg(&at_beats(at_change, next_bar, 0, next_beats, 0.0));
            assert!(
                (before - after).abs() < 0.5,
                "{beats}/4 bar {bar} to {next_beats} bar {next_bar}: {before} then {after}"
            );
        }
    }
}

#[cfg(test)]
mod hue_tests {
    use crate::render::{render, Background, Stage};
    use crate::{cast, Character, Mood, Move, Pose};

    #[test]
    fn the_leds_never_turn_with_the_hue() {
        let ch = cast::by_id(super::super::ID).expect("the robot");
        let led = super::pal::LED;
        let count = |hue: f32| {
            let pose = Pose {
                mv: Move::Bounce,
                mood: Mood::Surprised,
                playing: true,
                hue,
                ..Pose::default()
            };
            let stage = Stage::new(400, 400, Background::Transparent);
            let p = render(&stage, &[(ch.as_ref() as &dyn Character, pose)]).expect("a stage");
            p.pixels()
                .iter()
                .filter(|c| {
                    c.alpha() == 255 && [c.red(), c.green(), c.blue()] == [led[0], led[1], led[2]]
                })
                .count()
        };
        let still = count(0.0);
        assert!(still > 30, "the LEDs show ({still} pixels)");
        for hue in [-120.0, 90.0, 120.0, 180.0] {
            assert_eq!(count(hue), still, "at {hue}° the LEDs are the same");
        }
    }
}
