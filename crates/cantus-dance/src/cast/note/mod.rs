//! **Corchea**, the living eighth note (`builtin:note`) — the family's showman (ADR 0090):
//! a glossy plum note head that is head and body at once, its stem rising behind with a
//! flag that waves late, white gloves, gold shoes and a gold bow tie. It dances jazzy and
//! sharp: claps, points, kicks, and bows to the audience at the end of a phrase.

mod draw;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::Style;
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:note";

/// The note's colours (straight RGBA, turned by the hue), its ink (never turned).
pub(crate) mod pal {
    use crate::rig::draw::{Ink, Look};

    pub const INK: [u8; 4] = [40, 24, 38, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.024,
    };
    pub const PLUM: Look = Look {
        base: [118, 72, 108, 255],
        shade: [84, 50, 80, 255],
        light: [166, 116, 156, 255],
        rim: [208, 168, 200, 255],
    };
    pub const GLOVE: Look = Look {
        base: [252, 250, 246, 255],
        shade: [216, 208, 210, 255],
        light: [255, 255, 255, 255],
        rim: [0, 0, 0, 0],
    };
    pub const GOLD: Look = Look {
        base: [250, 196, 56, 255],
        shade: [212, 142, 30, 255],
        light: [255, 236, 150, 255],
        rim: [0, 0, 0, 0],
    };
    pub const EYE_WHITE: [u8; 4] = [255, 255, 255, 255];
    pub const PUPIL: [u8; 4] = [34, 22, 34, 255];
    pub const IRIS: [u8; 4] = [96, 64, 90, 255];
    pub const MOUTH: [u8; 4] = [70, 26, 44, 255];
    pub const TONGUE: [u8; 4] = [255, 130, 146, 255];
    pub const LIP: [u8; 4] = [246, 222, 236, 255];
    pub const CHEEK: [u8; 4] = [255, 120, 150, 255];
    pub const SHADOW: [u8; 4] = [30, 18, 28, 110];
    pub const SPARKLE: [u8; 4] = [255, 230, 120, 255];
    pub const NOTE: [u8; 4] = [255, 214, 92, 255];
    pub const DUST: [u8; 4] = [228, 218, 224, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 206, 64, 255],
        [255, 120, 86, 255],
        [255, 160, 190, 255],
        [255, 250, 236, 255],
        [166, 116, 156, 255],
    ];
}

pub struct Note {
    info: CharacterInfo,
    style: Style,
}

impl Note {
    pub fn new() -> Note {
        Note {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Corchea".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.09,
                squash: 0.16,
                arms: 1.3,
                sway: 0.08,
                lag: 0.14,
                hop: 0.26,
                stride: 0.14,
            },
        }
    }
}

impl Default for Note {
    fn default() -> Self {
        Note::new()
    }
}

impl Character for Note {
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
            Move::Point,
            Move::Kick,
            Move::ArmsUp,
            Move::Spin,
            Move::Shuffle,
            Move::Twist,
            Move::Wave,
            Move::Clap,
            Move::Jump,
            Move::Point,
            Move::Bow,
            Move::Freeze,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.83,
            top: -1.73,
            right: 0.96,
            bottom: 0.13,
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
