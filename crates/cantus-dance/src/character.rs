//! What a character is (ADR 0087 §2).
//!
//! **Its space.** A character draws in its own units: the origin is the point on the
//! ground between its feet, x to the right, **y down** (tiny-skia's), and standing it is
//! about **one unit tall** (its head near y = −1). [`Character::bounds`] is the box every
//! move of it stays inside (a jump's lift and a spin are the renderer's, on top).
//! The renderer maps that space onto the stage: position, size, lift, spin and facing.
//!
//! **Its moves.** Every character answers every [`Move`] — with a move of its own, or the
//! move's [`Move::fallback`] — and every [`Mood`]. A move is drawn from a [`Pose`]: which
//! move, how far through its cycle, how much energy, the face; the character never reads
//! a clock (the pose is a pure function of the song: `pose::pose_at`).

use crate::pose::Pose;
use crate::render::Canvas;
use cantus_core::inst::dancer::Move;

/// Where a character came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Cantus's own cast (`builtin:<id>`).
    Builtin,
    /// Imported from a file: its format's name ("Lottie", "Aseprite", …).
    Imported { format: &'static str },
}

/// A character's licence, as shown before it is used (ADR 0089 §4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Licence {
    /// Cantus's own, under the application's licence (GPL-3.0-or-later).
    Cantus,
    /// What the file or its package states (an SPDX id or the words), with where it said so.
    Stated { text: String, source: String },
    /// Nothing stated: shown as "unknown — make sure you may use it".
    Unknown,
}

/// What the picker, the catalogue and the licence line show.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterInfo {
    /// Stable: `builtin:<id>` or `file:<path relative to the project>` (the `character`
    /// text key, `cantus_core::inst::dancer::CHARACTER_KEY`).
    pub id: String,
    /// Its name as shown (the cast's from the locale files' `cast.<id>.name`; an import's
    /// from the file or its file name).
    pub name: String,
    pub author: String,
    pub origin: Origin,
    pub licence: Licence,
}

/// A box in a character's units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Bounds {
    /// What a character one unit tall standing on the origin fills.
    pub const STANDING: Bounds = Bounds {
        left: -0.5,
        top: -1.1,
        right: 0.5,
        bottom: 0.05,
    };

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

/// A character that dances. Included and imported characters alike; `Send + Sync` so a
/// video export draws on a worker thread while the screen draws on the interface's.
pub trait Character: Send + Sync {
    fn info(&self) -> &CharacterInfo;

    /// Whether it has a move of its own for `m`; when not, `m.fallback()` plays.
    fn has_move(&self, m: Move) -> bool;

    /// A move's natural cycle in quarter notes at speed 1 (a step is one beat, a spin two):
    /// the pose's phase runs 0…1 over it.
    fn move_beats(&self, m: Move) -> f32 {
        match m {
            Move::Spin | Move::Bow | Move::Twist | Move::Wave => 2.0,
            Move::Freeze | Move::Idle => 4.0,
            _ => 1.0,
        }
    }

    /// The routine it dances to the beat when nothing chooses a move (`move` = 0, no
    /// note): one move per bar, repeating. Its own, so each character has its style.
    fn routine(&self) -> &[Move] {
        &[
            Move::Bounce,
            Move::Bounce,
            Move::StepLeft,
            Move::StepRight,
            Move::Clap,
            Move::Sway,
            Move::ArmsUp,
            Move::Spin,
        ]
    }

    /// The box every move stays inside, in its units.
    fn bounds(&self) -> Bounds {
        Bounds::STANDING
    }

    /// Whether it draws its own facing from `pose.turn` (a three-quarter view); when not,
    /// the renderer mirrors it for a negative `turn`.
    fn faces_itself(&self) -> bool {
        false
    }

    /// Draw `pose` onto `canvas`, whose transform already maps this character's units onto
    /// the pixels (position, size, lift, spin and facing applied).
    fn draw(&self, pose: &Pose, canvas: &mut Canvas);
}
