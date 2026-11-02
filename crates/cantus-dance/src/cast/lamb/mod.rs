//! **Lana**, the lamb (`builtin:lamb`) — the family's gentle one (ADR 0090): a cloud of
//! cream wool with a cocoa face, big bright eyes, floppy ears that flop late, a tuft with a
//! pink bow, hooves, a puff of a tail and a little gold bell that swings on the beat. She
//! dances hoppy and light: bounces, skips and jumps for joy.

mod draw;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Style;
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:lamb";

/// The lamb's colours (straight RGBA, turned by the hue), its ink (never turned).
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [52, 36, 40, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const WOOL: Look = Look {
        base: [250, 243, 228, 255],
        shade: [222, 206, 186, 255],
        light: [255, 254, 248, 255],
        rim: [0, 0, 0, 0],
    };
    pub const FACE: Look = Look {
        base: [110, 78, 70, 255],
        shade: [82, 58, 54, 255],
        light: [146, 108, 96, 255],
        rim: [0, 0, 0, 0],
    };
    pub const HOOF: [u8; 4] = [64, 46, 46, 255];
    pub const PINK: [u8; 4] = [255, 176, 186, 255];
    pub const BOW: Look = Look {
        base: [244, 120, 150, 255],
        shade: [206, 82, 116, 255],
        light: [255, 176, 196, 255],
        rim: [0, 0, 0, 0],
    };
    pub const BELL: Look = Look {
        base: [250, 196, 56, 255],
        shade: [212, 142, 30, 255],
        light: [255, 236, 150, 255],
        rim: [0, 0, 0, 0],
    };
    pub const EYE_WHITE: [u8; 4] = [255, 255, 255, 255];
    pub const PUPIL: [u8; 4] = [34, 24, 30, 255];
    pub const IRIS: [u8; 4] = [104, 70, 52, 255];
    pub const MOUTH: [u8; 4] = [70, 30, 40, 255];
    pub const TONGUE: [u8; 4] = [255, 130, 146, 255];
    pub const LIP: [u8; 4] = [246, 214, 204, 255];
    pub const CHEEK: [u8; 4] = [255, 128, 150, 255];
    pub const SHADOW: [u8; 4] = [30, 20, 24, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [236, 226, 210, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [244, 120, 150, 255],
        [255, 176, 186, 255],
        [255, 250, 236, 255],
        [255, 150, 90, 255],
    ];
}

pub struct Lamb {
    info: CharacterInfo,
    style: Style,
}

impl Lamb {
    pub fn new() -> Lamb {
        Lamb {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Lana".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.11,
                squash: 0.15,
                arms: 1.0,
                sway: 0.06,
                lag: 0.16,
                hop: 0.3,
                stride: 0.12,
            },
        }
    }
}

impl Default for Lamb {
    fn default() -> Self {
        Lamb::new()
    }
}

impl Character for Lamb {
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
            Move::Jump,
            Move::StepLeft,
            Move::StepRight,
            Move::Clap,
            Move::Bounce,
            Move::Wave,
            Move::Spin,
            Move::Shuffle,
            Move::Jump,
            Move::Sway,
            Move::Kick,
            Move::ArmsUp,
            Move::Clap,
            Move::Bow,
            Move::Spin,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.76,
            top: -1.54,
            right: 0.86,
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
