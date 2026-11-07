//! **Fuga**, the ninja (`builtin:ninja`) — the family's acrobat and a new dancer's
//! character (ADR 0105): a near-black gi lit from behind in violet with a high collar,
//! gunmetal guards strapped on the forearms and shins, a fitted mask over the lower face,
//! narrow amber eyes under thick angled brows, spiky swept hair, and a deep crimson scarf
//! whose torn tails stream behind every move; a gold fermata (music's sign for *hold*) on
//! the sash. Its name is music's *fuga* (a fugue, and in Spanish a flight). It dances low
//! and acrobatic — flips, high kicks, a crouched landing, a martial bow — and never carries
//! a blade.

mod draw;
mod head;
mod moves;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Body, Style};
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:ninja";

/// The ninja's colours (straight RGBA, turned by the hue but the eyes), its ink: a
/// near-black gi lit from behind in violet, deep crimson cloth, gunmetal guards.
pub(crate) mod pal {
    use crate::cast::edge::Hard;
    use crate::rig::draw::Ink;

    pub const INK: [u8; 4] = [12, 8, 16, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.016,
    };
    /// The back-light: a violet rim on everything dark.
    pub const RIM: [u8; 4] = [176, 104, 206, 255];
    pub const GI: Hard = Hard {
        base: [56, 48, 72, 255],
        shade: [30, 26, 40, 255],
        rim: RIM,
    };
    pub const MASK: Hard = Hard {
        base: [44, 38, 56, 255],
        shade: [24, 20, 32, 255],
        rim: RIM,
    };
    pub const SCARF: Hard = Hard {
        base: [182, 26, 48, 255],
        shade: [106, 12, 30, 255],
        rim: [255, 104, 96, 255],
    };
    pub const GUARD: Hard = Hard {
        base: [96, 92, 112, 255],
        shade: [52, 50, 66, 255],
        rim: [214, 170, 240, 255],
    };
    pub const SASH: Hard = Hard {
        base: [74, 62, 92, 255],
        shade: [44, 36, 56, 255],
        rim: RIM,
    };
    pub const SKIN: Hard = Hard {
        base: [234, 188, 154, 255],
        shade: [178, 120, 100, 255],
        rim: [255, 214, 196, 255],
    };
    pub const HAIR: Hard = Hard {
        base: [36, 30, 44, 255],
        shade: [18, 14, 24, 255],
        rim: RIM,
    };
    pub const STRAP: [u8; 4] = [128, 18, 38, 255];
    pub const GOLD: [u8; 4] = [255, 196, 72, 255];
    pub const EYE_WHITE: [u8; 4] = [246, 242, 240, 255];
    pub const IRIS: [u8; 4] = [242, 164, 40, 255];
    pub const IRIS_DARK: [u8; 4] = [150, 62, 22, 255];
    pub const PUPIL: [u8; 4] = [26, 10, 14, 255];
    pub const LASH: [u8; 4] = [12, 8, 16, 255];
    pub const SHADOW: [u8; 4] = [14, 10, 18, 120];
    pub const SPARKLE: [u8; 4] = [255, 214, 140, 255];
    pub const NOTE: [u8; 4] = [255, 196, 72, 255];
    pub const DUST: [u8; 4] = [206, 200, 210, 200];
    pub const PETAL: [[u8; 4]; 5] = [
        [182, 26, 48, 255],
        [255, 104, 96, 255],
        [176, 104, 206, 255],
        [246, 242, 240, 255],
        [255, 196, 72, 255],
    ];
}

pub struct Ninja {
    info: CharacterInfo,
    style: Style,
}

impl Ninja {
    pub fn new() -> Ninja {
        Ninja {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Fuga".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.09,
                squash: 0.08,
                arms: 1.3,
                sway: 0.08,
                lag: 0.16,
                hop: 0.34,
                stride: 0.18,
            },
        }
    }

    /// The body for a pose: the family's choreography in Fuga's style, with its own moves.
    pub fn body(&self, pose: &Pose) -> Body {
        draw::body(self, pose)
    }

    /// Its colours, for the model sheet.
    #[doc(hidden)]
    pub fn swatches() -> Vec<(&'static str, [u8; 4])> {
        use pal::*;
        let mut v = Vec::new();
        for (n, h) in [
            ("gi", GI),
            ("mask", MASK),
            ("scarf", SCARF),
            ("guard", GUARD),
            ("skin", SKIN),
            ("hair", HAIR),
        ] {
            v.extend([(n, h.shade), (n, h.base), (n, h.rim)]);
        }
        v.extend([
            ("strap", STRAP),
            ("gold", GOLD),
            ("iris", IRIS),
            ("iris dark", IRIS_DARK),
            ("ink", INK),
        ]);
        v
    }

    /// Draw a given body (the sheets turn it round for a turnaround).
    #[doc(hidden)]
    pub fn draw_body(&self, pose: &Pose, body: &Body, canvas: &mut Canvas) {
        super::edge::at_scale(canvas, |c| draw::draw(self, pose, body, c));
    }
}

impl Default for Ninja {
    fn default() -> Self {
        Ninja::new()
    }
}

impl Character for Ninja {
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
            Move::Kick,
            Move::Shuffle,
            Move::Point,
            Move::Jump,
            Move::Freeze,
            Move::Sway,
            Move::Clap,
            Move::Twist,
            Move::Kick,
            Move::ArmsUp,
            Move::Spin,
            Move::Jump,
            Move::Bow,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.95,
            top: -1.53,
            right: 0.95,
            bottom: 0.1,
        }
    }

    fn faces_itself(&self) -> bool {
        true
    }

    fn draw(&self, pose: &Pose, canvas: &mut Canvas) {
        let body = self.body(pose);
        super::edge::at_scale(canvas, |c| draw::draw(self, pose, &body, c));
    }
}
