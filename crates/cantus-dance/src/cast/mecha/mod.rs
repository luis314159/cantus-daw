//! **Volta**, the mecha (`builtin:mecha`) — the family's action robot (ADR 0105): heavy
//! sculpted gunmetal plates with pearl armour over a graphite frame, exposed pistons at the
//! joints, coral light in its seams; an angular helmet with a narrow V visor of light whose
//! slits change with every mood, a sculpted faceplate whose grille lights up when it sings,
//! blades swept flat back from its temples; a speaker core in its chest that pulses on the
//! beat, vanes on its back and jets that fire when it jumps. Its name is music's *volta*
//! (the turn, the ending of a repeat) and the volt it runs on. It dances sharp and
//! powerful: pops that hit the beat, locks that hold, hero poses.

mod draw;
mod face;
mod moves;

use crate::character::{Bounds, Character, CharacterInfo, Licence, Origin};
use crate::pose::Pose;
use crate::render::Canvas;
use crate::rig::dance::{Body, Style};
use cantus_core::inst::dancer::Move;

pub const ID: &str = "builtin:mecha";

/// The mecha's colours (straight RGBA, turned by the hue but the glow and the glass), its
/// ink. Gunmetal armour with pearl plates, a graphite frame, coral light.
pub(crate) mod pal {
    use crate::cast::edge::Hard;
    use crate::rig::draw::Ink;

    pub const INK: [u8; 4] = [16, 14, 22, 255];
    pub const OUTLINE: Ink = Ink {
        color: INK,
        width: 0.016,
    };
    /// The rim the back-light paints on the armour: a warm coral edge.
    pub const RIM: [u8; 4] = [255, 118, 112, 255];
    pub const GUNMETAL: Hard = Hard {
        base: [92, 96, 114, 255],
        shade: [52, 54, 70, 255],
        rim: RIM,
    };
    pub const PEARL: Hard = Hard {
        base: [230, 230, 238, 255],
        shade: [150, 152, 172, 255],
        rim: [255, 200, 192, 255],
    };
    pub const FRAME: Hard = Hard {
        base: [44, 44, 58, 255],
        shade: [28, 28, 38, 255],
        rim: [200, 92, 96, 255],
    };
    pub const PISTON: [u8; 4] = [176, 178, 192, 255];
    /// Light (never turned by the hue): the visor, the core, the lines, the jets.
    pub const GLOW: [u8; 4] = [255, 70, 74, 255];
    pub const GLOW_HOT: [u8; 4] = [255, 214, 204, 255];
    pub const GLOW_HALO: [u8; 4] = [124, 30, 44, 255];
    pub const VISOR: [u8; 4] = [14, 10, 18, 255];
    pub const JET: [u8; 4] = [255, 138, 64, 255];
    pub const JET_HOT: [u8; 4] = [255, 240, 210, 255];
    pub const SHADOW: [u8; 4] = [14, 12, 20, 120];
    pub const SPARKLE: [u8; 4] = [255, 196, 120, 255];
    pub const NOTE: [u8; 4] = [255, 96, 96, 255];
    pub const DUST: [u8; 4] = [200, 198, 210, 200];
    pub const CONFETTI: [[u8; 4]; 5] = [
        [255, 70, 74, 255],
        [255, 196, 120, 255],
        [230, 230, 238, 255],
        [92, 96, 114, 255],
        [255, 138, 64, 255],
    ];
}

pub struct Mecha {
    info: CharacterInfo,
    style: Style,
}

impl Mecha {
    pub fn new() -> Mecha {
        Mecha {
            info: CharacterInfo {
                id: ID.to_string(),
                name: "Volta".to_string(),
                author: "Cantus".to_string(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            style: Style {
                bounce: 0.07,
                squash: 0.05,
                arms: 1.2,
                sway: 0.07,
                lag: 0.1,
                hop: 0.3,
                stride: 0.16,
            },
        }
    }

    /// The body for a pose: the family's choreography in Volta's style.
    pub fn body(&self, pose: &Pose) -> Body {
        let st = &self.style;
        let now = moves::dance_move(pose.mv, pose.phase, pose.cycle, pose, st);
        let mut b = match pose.from {
            Some((from, phase)) if pose.blend < 1.0 => {
                let was = moves::dance_move(from, phase, pose.from_cycle, pose, st);
                Body::lerp(&was, &now, crate::rig::ease_in_out(pose.blend))
            }
            _ => now,
        };
        b.yaw += pose.turn.clamp(-1.0, 1.0) * 40.0;
        b.mouth = b.mouth.max(0.0) * (0.6 + 0.4 * crate::rig::vigour(pose));
        b
    }

    /// Its colours, for the model sheet.
    #[doc(hidden)]
    pub fn swatches() -> Vec<(&'static str, [u8; 4])> {
        use pal::*;
        let mut v = Vec::new();
        for (n, h) in [("gunmetal", GUNMETAL), ("pearl", PEARL), ("frame", FRAME)] {
            v.extend([(n, h.shade), (n, h.base), (n, h.rim)]);
        }
        v.extend([
            ("piston", PISTON),
            ("glow halo", GLOW_HALO),
            ("glow", GLOW),
            ("glow hot", GLOW_HOT),
            ("visor", VISOR),
            ("jet", JET),
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

impl Default for Mecha {
    fn default() -> Self {
        Mecha::new()
    }
}

impl Character for Mecha {
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
            Move::StepLeft,
            Move::StepRight,
            Move::Shuffle,
            Move::Clap,
            Move::ArmsUp,
            Move::Freeze,
            Move::Kick,
            Move::Twist,
            Move::Wave,
            Move::Point,
            Move::Shuffle,
            Move::Jump,
            Move::Bounce,
            Move::Spin,
        ]
    }

    /// Measured (`examples/cast_bounds.rs`) with a margin; `tests/cast.rs` holds it.
    fn bounds(&self) -> Bounds {
        Bounds {
            left: -0.83,
            top: -1.53,
            right: 0.83,
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
