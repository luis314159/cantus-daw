//! **Miso**, the cat (`builtin:cat`) — the family's cool one (ADR 0090): a smoky grey tabby
//! with a big round head that turns first and a body that follows, pointy ears that flick
//! on the beat, amber eyes, a white muzzle and paws, a long tail that trails every move,
//! and a gold scarf. Slinky in style: sways and twists, steps light, ends a phrase striking
//! a pose.

mod draw;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Style;
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:cat";

/// The cat's colours (straight RGBA, turned by the hue), its ink (never turned).
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [44, 34, 48, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const FUR: Look = Look {
        base: [172, 164, 170, 255],
        shade: [130, 122, 134, 255],
        light: [212, 206, 210, 255],
        rim: [236, 232, 236, 255],
    };
    pub const STRIPE: [u8; 4] = [112, 104, 118, 255];
    pub const WHITE: Look = Look {
        base: [252, 248, 242, 255],
        shade: [222, 214, 210, 255],
        light: [255, 255, 255, 255],
        rim: [0, 0, 0, 0],
    };
    pub const PINK: [u8; 4] = [250, 150, 160, 255];
    pub const NOSE: [u8; 4] = [236, 110, 126, 255];
    pub const SCARF: Look = Look {
        base: [250, 196, 56, 255],
        shade: [214, 146, 30, 255],
        light: [255, 230, 140, 255],
        rim: [0, 0, 0, 0],
    };
    pub const SCARF_STRIPE: [u8; 4] = [226, 92, 64, 255];
    pub const EYE_WHITE: [u8; 4] = [255, 255, 255, 255];
    pub const PUPIL: [u8; 4] = [36, 26, 40, 255];
    pub const IRIS: [u8; 4] = [176, 118, 46, 255];
    pub const MOUTH: [u8; 4] = [120, 40, 58, 255];
    pub const TONGUE: [u8; 4] = [255, 128, 140, 255];
    pub const CHEEK: [u8; 4] = [255, 128, 150, 255];
    pub const SHADOW: [u8; 4] = [28, 20, 30, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [226, 220, 214, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [255, 120, 86, 255],
        [250, 150, 160, 255],
        [255, 250, 236, 255],
        [176, 168, 176, 255],
    ];
}

pub struct Cat {
    info: CharacterInfo,
    style: Style,
}

impl Cat {
    pub fn new() -> Cat {
        Cat {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Miso".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.08,
                squash: 0.13,
                arms: 1.2,
                sway: 0.085,
                lag: 0.18,
                hop: 0.24,
                stride: 0.13,
            },
        }
    }
}

impl Default for Cat {
    fn default() -> Self {
        Cat::new()
    }
}

impl Character for Cat {
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
            Move::Sway,
            Move::Bounce,
            Move::StepLeft,
            Move::StepRight,
            Move::Twist,
            Move::Point,
            Move::Shuffle,
            Move::Spin,
            Move::Sway,
            Move::Kick,
            Move::Wave,
            Move::Clap,
            Move::ArmsUp,
            Move::Twist,
            Move::Jump,
            Move::Freeze,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.78,
            top: -1.53,
            right: 0.96,
            bottom: 0.16,
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
