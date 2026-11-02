//! The dancing character (round 35, ADRs 0087–0091).
//!
//! A **dancer** is a native instrument that makes no sound (`cantus_core::inst::dancer`):
//! its numbers are automation and modulation targets, notes on its track choose its moves,
//! and its unit tells the screen where the song is (`cantus_engine::dance`). This crate is
//! everything after that:
//!
//! - [`character`]: what a character is — the [`Character`] trait every included and
//!   imported character implements, its information and licence.
//! - [`pose`]: the pose at a moment, a **pure function** of the engine's frame
//!   ([`pose::pose_at`]) — the same song position and numbers give the same pose whatever
//!   the frame rate, which is what lets a video and the screen agree.
//! - [`render`]: the one renderer — tiny-skia on the processor, into a pixmap at the exact
//!   pixel size, crisp at any scale; the screen, the picker's previews, the companion window
//!   and every frame of a video draw through it.
//! - [`cast`]: the characters Cantus includes, ours and original.
//! - [`import`]: characters people bring (Lottie, sprite sheets, DragonBones, Rive — ADR
//!   0089), checked and sandboxed: no script runs, every size is bounded, the licence shown.

#![forbid(unsafe_code)]

pub mod cast;
pub mod character;
pub mod import;
pub mod pose;
pub mod render;
pub mod rig; // r35:cast

pub use cantus_core::inst::dancer::{Mood, Move};
pub use cantus_engine::dance::{DanceFrame, DanceNote};
pub use character::{Character, CharacterInfo, Licence, Origin};
pub use pose::{pose_at, Pose};
pub use render::{render, Background, Canvas, Stage};
