//! **Sol**, the child (`builtin:child`) — the family's youngest (ADR 0090): a round, friendly
//! face with big eyes, a cloud of curly hair with a gold star clip, a sunny T-shirt with a
//! star, coral shorts and white sneakers. Sol (the sun, and the note G) dances with all a
//! child's energy: claps, jumps, waves, spins and arms in the air.

mod draw;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Style;
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:child";

/// The child's colours (straight RGBA, turned by the hue), its ink (never turned).
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [48, 30, 30, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const SKIN: Look = Look {
        base: [214, 152, 112, 255],
        shade: [178, 114, 82, 255],
        light: [236, 186, 146, 255],
        rim: [0, 0, 0, 0],
    };
    pub const HAIR: Look = Look {
        base: [70, 46, 40, 255],
        shade: [46, 30, 28, 255],
        light: [108, 74, 60, 255],
        rim: [0, 0, 0, 0],
    };
    pub const SHIRT: Look = Look {
        base: [255, 202, 64, 255],
        shade: [226, 152, 40, 255],
        light: [255, 230, 140, 255],
        rim: [0, 0, 0, 0],
    };
    pub const SHORTS: Look = Look {
        base: [232, 98, 82, 255],
        shade: [190, 66, 60, 255],
        light: [250, 150, 130, 255],
        rim: [0, 0, 0, 0],
    };
    pub const SHOE: Look = Look {
        base: [252, 250, 246, 255],
        shade: [214, 206, 204, 255],
        light: [255, 255, 255, 255],
        rim: [0, 0, 0, 0],
    };
    pub const STRIPE: [u8; 4] = [232, 98, 82, 255];
    pub const STAR: [u8; 4] = [255, 252, 240, 255];
    pub const CLIP: [u8; 4] = [255, 214, 72, 255];
    pub const EYE_WHITE: [u8; 4] = [255, 255, 255, 255];
    pub const PUPIL: [u8; 4] = [36, 24, 26, 255];
    pub const IRIS: [u8; 4] = [104, 66, 46, 255];
    pub const MOUTH: [u8; 4] = [120, 40, 52, 255];
    pub const TONGUE: [u8; 4] = [255, 128, 140, 255];
    pub const CHEEK: [u8; 4] = [255, 120, 130, 255];
    pub const SHADOW: [u8; 4] = [30, 20, 22, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [230, 220, 208, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [232, 98, 82, 255],
        [255, 160, 190, 255],
        [255, 250, 236, 255],
        [255, 150, 90, 255],
    ];
}

pub struct Child {
    info: CharacterInfo,
    style: Style,
}

impl Child {
    pub fn new() -> Child {
        Child {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Sol".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.1,
                squash: 0.12,
                arms: 1.3,
                sway: 0.07,
                lag: 0.14,
                hop: 0.3,
                stride: 0.15,
            },
        }
    }
}

impl Default for Child {
    fn default() -> Self {
        Child::new()
    }
}

impl Character for Child {
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
            Move::Clap,
            Move::StepLeft,
            Move::StepRight,
            Move::Wave,
            Move::Jump,
            Move::ArmsUp,
            Move::Spin,
            Move::Shuffle,
            Move::Kick,
            Move::Point,
            Move::Clap,
            Move::Twist,
            Move::Jump,
            Move::ArmsUp,
            Move::Bow,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.76,
            top: -1.49,
            right: 0.91,
            bottom: 0.09,
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
