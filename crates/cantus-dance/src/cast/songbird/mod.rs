//! **Trino**, the songbird (`builtin:songbird`) — the first of the family and a new
//! dancer's character (ADR 0090). Cantus means *song*: a round little bird in warm
//! tangerine with a cream belly, a three-feather crest, a perky tail and berry headphones,
//! who dances bouncy and light, claps with its wing tips and sings on the downbeat.
//!
//! Drawn as a turning ball (the rig's pseudo-3D): its face goes round when it spins and
//! turns three-quarter for `turn`, so it draws its own facing.

mod draw;
mod face;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{self, Body, Style};
use crate::rig::{ease_in_out, vigour};
use cantus_core::inst::dancer::Move;

/// Its id, forever (`builtin:songbird`).
pub const ID: &str = "builtin:songbird";

/// The songbird's colours (straight RGBA, turned by the pose's hue), its ink (never turned).
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [52, 30, 36, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const BODY: Look = Look {
        base: [255, 141, 62, 255],
        shade: [222, 88, 46, 255],
        light: [255, 196, 122, 255],
        rim: [255, 228, 168, 255],
    };
    pub const WING: Look = Look {
        base: [242, 106, 50, 255],
        shade: [196, 66, 38, 255],
        light: [255, 160, 96, 255],
        rim: [255, 206, 150, 255],
    };
    pub const BELLY: Look = Look {
        base: [255, 240, 214, 255],
        shade: [246, 200, 158, 255],
        light: [255, 252, 240, 255],
        rim: [0, 0, 0, 0],
    };
    pub const BEAK: Look = Look {
        base: [255, 200, 64, 255],
        shade: [228, 146, 34, 255],
        light: [255, 236, 150, 255],
        rim: [0, 0, 0, 0],
    };
    pub const LEG: Look = Look {
        base: [248, 178, 64, 255],
        shade: [214, 124, 40, 255],
        light: [255, 220, 130, 255],
        rim: [0, 0, 0, 0],
    };
    pub const PHONES: Look = Look {
        base: [232, 62, 104, 255],
        shade: [204, 48, 90, 255],
        light: [255, 136, 164, 255],
        rim: [255, 196, 210, 255],
    };
    pub const BAND: [u8; 4] = [70, 58, 74, 255];
    pub const TIP: [u8; 4] = [214, 70, 52, 255];
    pub const CHEEK: [u8; 4] = [255, 92, 112, 190];
    pub const EYE_WHITE: [u8; 4] = [255, 255, 255, 255];
    pub const PUPIL: [u8; 4] = [38, 24, 44, 255];
    pub const IRIS: [u8; 4] = [92, 58, 70, 255];
    pub const MOUTH: [u8; 4] = [120, 36, 52, 255];
    pub const TONGUE: [u8; 4] = [255, 120, 128, 255];
    pub const SHADOW: [u8; 4] = [30, 18, 24, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [236, 220, 200, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [255, 120, 86, 255],
        [232, 62, 104, 255],
        [255, 250, 236, 255],
        [255, 150, 170, 255],
    ];
}

pub struct Songbird {
    info: CharacterInfo,
    style: Style,
}

impl Songbird {
    pub fn new() -> Songbird {
        Songbird {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Trino".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.1,
                squash: 0.16,
                arms: 1.25,
                sway: 0.09,
                lag: 0.14,
                hop: 0.26,
                stride: 0.14,
            },
        }
    }

    /// The body of the current move at `phase` of its cycle (where an extra was born).
    pub(crate) fn body_at(&self, pose: &Pose, phase: f32) -> Body {
        let mut b = dance::dance(pose.mv, phase, pose.cycle, pose, &self.style);
        b.yaw += pose.turn.clamp(-1.0, 1.0) * 40.0;
        b
    }

    /// The body at this pose, blended from the move it is leaving.
    pub(crate) fn body(&self, pose: &Pose) -> Body {
        let now = dance::dance(pose.mv, pose.phase, pose.cycle, pose, &self.style);
        let mut b = match pose.from {
            Some((from, phase)) if pose.blend < 1.0 => {
                let was = dance::dance(from, phase, pose.from_cycle, pose, &self.style);
                Body::lerp(&was, &now, ease_in_out(pose.blend))
            }
            _ => now,
        };
        // The pose's own turn: a three-quarter view, the ball turned toward it.
        b.yaw += pose.turn.clamp(-1.0, 1.0) * 40.0;
        // Energy breathes into the face a little.
        b.mouth = b.mouth.max(0.0) * (0.6 + 0.4 * vigour(pose));
        b
    }
}

impl Default for Songbird {
    fn default() -> Self {
        Songbird::new()
    }
}

impl Character for Songbird {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }

    fn has_move(&self, _m: Move) -> bool {
        true
    }

    fn move_beats(&self, m: Move) -> f32 {
        match m {
            Move::Spin | Move::Bow | Move::Twist | Move::Wave | Move::Jump => 2.0,
            Move::Freeze | Move::Idle => 4.0,
            _ => 1.0,
        }
    }

    fn routine(&self) -> &[Move] {
        &[
            Move::Bounce,
            Move::StepLeft,
            Move::StepRight,
            Move::Clap,
            Move::Point,
            Move::Shuffle,
            Move::ArmsUp,
            Move::Spin,
            Move::Bounce,
            Move::Sway,
            Move::Kick,
            Move::Clap,
            Move::Wave,
            Move::Twist,
            Move::Jump,
            Move::Spin,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -1.0,
            top: -1.6,
            right: 1.0,
            bottom: 0.11,
        }
    }

    fn faces_itself(&self) -> bool {
        true
    }

    fn draw(&self, pose: &Pose, canvas: &mut Canvas) {
        let body = self.body(pose);
        draw::draw(self, pose, &body, canvas);
    }
}
