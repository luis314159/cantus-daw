//! A stand-in character for the contract (round 35): a round body, a head, arms and legs
//! that answer every move in a simple way. The cast builder replaces it.

use crate::character::{Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use cantus_core::inst::dancer::{Mood, Move};
use cantus_dsp::math;
use tiny_skia::{Path, PathBuilder};

pub struct Placeholder {
    info: CharacterInfo,
}

impl Placeholder {
    pub fn new() -> Placeholder {
        Placeholder {
            info: CharacterInfo {
                id: "builtin:placeholder".to_string(),
                name: "Placeholder".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
        }
    }
}

impl Default for Placeholder {
    fn default() -> Self {
        Placeholder::new()
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r)
}

fn line(points: &[(f32, f32)]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (x0, y0) = *points.first()?;
    pb.move_to(x0, y0);
    for &(x, y) in &points[1..] {
        pb.line_to(x, y);
    }
    pb.finish()
}

/// A limb from `from`, `len` long, at `deg` from straight down.
fn limb(from: (f32, f32), len: f32, deg: f32) -> (f32, f32) {
    let (s, c) = math::sin_cos(deg.to_radians());
    (from.0 + s * len, from.1 + c * len)
}

impl Character for Placeholder {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn has_move(&self, _m: Move) -> bool {
        true
    }

    fn draw(&self, pose: &Pose, c: &mut Canvas) {
        let tau = std::f32::consts::TAU;
        let e = 0.3 + 0.7 * pose.energy;
        let swing = math::sin(pose.phase * tau);
        let bob = -0.04 * e * math::sin(pose.beat_phase * tau).abs();
        let side = if pose.cycle.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let (mut arm_l, mut arm_r, mut leg_l, mut leg_r, mut lean) = (20.0, -20.0, 6.0, -6.0, 0.0);
        match pose.mv {
            Move::Idle | Move::Freeze => {}
            Move::Bounce | Move::Jump => {
                arm_l = 40.0 + 30.0 * e * swing;
                arm_r = -arm_l;
            }
            Move::StepLeft | Move::StepRight | Move::Shuffle | Move::Kick => {
                leg_l = 25.0 * e * swing * side;
                leg_r = -leg_l;
                arm_l = 30.0 - leg_l;
                arm_r = -30.0 - leg_l;
            }
            Move::Sway | Move::Twist => lean = 12.0 * e * swing,
            Move::Clap | Move::Point | Move::Bow => {
                arm_l = 120.0 + 40.0 * e * swing;
                arm_r = -arm_l;
            }
            Move::Wave | Move::ArmsUp | Move::Spin => {
                arm_l = 160.0 + 15.0 * e * swing;
                arm_r = if pose.mv == Move::Wave { -30.0 } else { -arm_l };
            }
        }
        let body = c.color([88, 140, 230, 255]);
        let ink = c.fixed([30, 32, 40, 255]);
        let hip = (lean * 0.004, -0.35 + bob);
        let neck = (lean * 0.012, -0.72 + bob);
        let w = 0.07;
        for (from, deg) in [(hip, leg_l), (hip, leg_r)] {
            if let Some(p) = line(&[from, limb(from, 0.36, deg)]) {
                c.stroke(&p, ink, w);
            }
        }
        if let Some(p) = line(&[(neck.0, neck.1 + 0.06), hip]) {
            c.stroke(&p, body, 0.26);
        }
        for (deg, sgn) in [(arm_l, 1.0), (arm_r, -1.0)] {
            let from = (neck.0 + 0.1 * sgn, neck.1 + 0.08);
            if let Some(p) = line(&[from, limb(from, 0.3, deg)]) {
                c.stroke(&p, ink, w * 0.8);
            }
        }
        if let Some(p) = circle(neck.0, neck.1 - 0.16, 0.17) {
            c.fill(&p, body);
        }
        let eye = 0.025 * (1.0 - pose.blink).max(0.15);
        for dx in [-0.06, 0.06] {
            if let Some(p) = circle(neck.0 + dx, neck.1 - 0.18, eye) {
                c.fill(&p, ink);
            }
        }
        let smile = match pose.mood {
            Mood::Happy | Mood::Excited | Mood::Cool => 0.04,
            Mood::Surprised => -0.0,
            _ => 0.015,
        };
        if let Some(p) = line(&[
            (neck.0 - 0.06, neck.1 - 0.1),
            (neck.0, neck.1 - 0.1 + smile),
            (neck.0 + 0.06, neck.1 - 0.1),
        ]) {
            c.stroke(&p, ink, 0.02);
        }
    }
}
