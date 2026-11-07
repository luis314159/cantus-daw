//! Fuga's own moves (ADR 0105), on top of the family's choreography (`rig::dance`): a low
//! ready stance for idle, a high kick that leans away from it, a tornado spin off the
//! ground, a martial bow with fist to palm. The aerial and the crouched landing are in
//! `draw.rs` beside the flip that turns the picture. Phase 0 is the beat, as everywhere.

use crate::pose::Pose;
use crate::rig::dance::{self, Body, Style};
use crate::rig::{self, lerp};
use cantus_core::inst::dancer::Move;

/// Idle: the family's breath and glances, in a low stance — knees bent, feet apart,
/// hands loose and ready.
pub(super) fn ready(pose: &Pose, st: &Style) -> Body {
    let mut b = dance::idle(pose, st);
    b.y += 0.05;
    b.foot = [(-0.05, 0.0), (0.05, 0.0)];
    b.arm = [b.arm[0] + 16.0, b.arm[1] + 12.0];
    b.bend = [36.0, 28.0];
    b.spread = [0.2, 0.2];
    b.lean += 3.0;
    b
}

/// The kick, high: the family's, the kicking foot thrown twice as far and as high, the
/// body leaning away from it, the arms out for balance.
pub(super) fn high_kick(p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let mut b = dance::kick(p, cycle, pose, st);
    for i in 0..2 {
        let f = b.foot[i];
        if f.0.abs() > 0.03 {
            b.foot[i] = (f.0 * 2.0, f.1 * 2.6);
        }
    }
    b.lean *= 1.7;
    let reach = (b.foot[0].1.min(b.foot[1].1) / -0.5).clamp(0.0, 1.0);
    b.arm = [lerp(b.arm[0], 104.0, reach), lerp(b.arm[1], 104.0, reach)];
    b.bend = [lerp(b.bend[0], -6.0, reach); 2];
    b.y = b.y.min(0.0) + 0.02 * reach;
    b
}

/// The spin, off the ground: the family's whirl, lifted through its middle with the
/// knees drawn up and the arms wide.
pub(super) fn tornado(p: f32, pose: &Pose, st: &Style) -> Body {
    let mut b = dance::spin(p, pose, st);
    let up = rig::arc(rig::window(p, 0.18, 0.78)) * rig::vigour(pose);
    b.y -= 0.14 * up;
    b.foot = [(-0.04, -0.12 * up), (0.06, -0.2 * up)];
    b.arm = [lerp(b.arm[0], 96.0, up), lerp(b.arm[1], 96.0, up)];
    b.spread = [1.0; 2];
    b.tail += 30.0 * up;
    b
}

/// The bow, martial: the family's bow with the right fist in the left palm before the
/// chest.
pub(super) fn salute(p: f32, cycle: u32, pose: &Pose, st: &Style) -> Body {
    let mut b = dance::dance(Move::Bow, p, cycle, pose, st);
    let down = rig::ease_in_out(rig::window(p, 0.04, 0.3));
    let up = rig::ease_out_back(rig::window(p, 0.72, 0.98));
    let k = down * (1.0 - up);
    b.arm = [lerp(10.0, 58.0, k), lerp(10.0, -26.0, k)];
    b.bend = [lerp(0.0, 100.0, k), lerp(0.0, 112.0, k)];
    b.spread = [lerp(0.3, 1.0, k), 0.0];
    b.nod *= 0.7;
    b
}
