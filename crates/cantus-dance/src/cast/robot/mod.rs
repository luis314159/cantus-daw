//! **Tic**, the metronome robot (`builtin:robot`) — the family's precise one (ADR 0090):
//! an ivory metronome with copper trim, a screen face whose amber LED eyes and mouth change
//! with every mood, a pendulum in its front that ticks side to side on the beats, an
//! antenna whose bulb flashes on the accent, hose arms with round mitts and piston legs.
//! It dances sharp and on the grid: pops, points and robot shuffles.

mod draw;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Style;
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:robot";

/// The robot's colours (straight RGBA, turned by the hue — the LEDs too), its ink.
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [42, 36, 46, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const SHELL: Look = Look {
        base: [238, 230, 212, 255],
        shade: [204, 190, 166, 255],
        light: [252, 249, 240, 255],
        rim: [255, 255, 250, 255],
    };
    pub const COPPER: Look = Look {
        base: [214, 122, 66, 255],
        shade: [168, 86, 46, 255],
        light: [244, 172, 116, 255],
        rim: [0, 0, 0, 0],
    };
    pub const METAL: Look = Look {
        base: [158, 152, 156, 255],
        shade: [116, 110, 118, 255],
        light: [200, 196, 198, 255],
        rim: [0, 0, 0, 0],
    };
    pub const SCREEN: [u8; 4] = [46, 40, 54, 255];
    pub const SCREEN_LIGHT: [u8; 4] = [70, 62, 80, 255];
    pub const LED: [u8; 4] = [255, 196, 72, 255];
    pub const LED_GLOW: [u8; 4] = [128, 96, 62, 255];
    pub const LED_PINK: [u8; 4] = [255, 128, 140, 255];
    pub const SHADOW: [u8; 4] = [28, 22, 30, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [226, 218, 206, 200];
    pub const HEART: [u8; 4] = [255, 128, 140, 255];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [214, 122, 66, 255],
        [255, 150, 160, 255],
        [255, 250, 236, 255],
        [200, 196, 198, 255],
    ];
}

pub struct Robot {
    info: CharacterInfo,
    style: Style,
}

impl Robot {
    pub fn new() -> Robot {
        Robot {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Tic".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.07,
                squash: 0.09,
                arms: 1.25,
                sway: 0.07,
                lag: 0.1,
                hop: 0.22,
                stride: 0.13,
            },
        }
    }
}

impl Default for Robot {
    fn default() -> Self {
        Robot::new()
    }
}

impl Character for Robot {
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
            Move::Point,
            Move::Shuffle,
            Move::Shuffle,
            Move::StepLeft,
            Move::StepRight,
            Move::ArmsUp,
            Move::Freeze,
            Move::Kick,
            Move::Clap,
            Move::Twist,
            Move::Wave,
            Move::Point,
            Move::Jump,
            Move::Bounce,
            Move::Spin,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.86,
            top: -1.45,
            right: 0.9,
            bottom: 0.1,
        }
    }

    fn faces_itself(&self) -> bool {
        true
    }

    fn draw(&self, pose: &Pose, canvas: &mut Canvas) {
        let body = super::kit::body(pose, &self.style, 40.0);
        draw::draw(self, pose, &body, canvas);
    }
}
