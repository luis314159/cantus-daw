//! r42:arcus-upper — the figures that play an instrument, shared by every instrument that
//! shows one (ADR 0159; the owner, 2026-11-24: the musicians and the mannequins reusable by
//! the guitar family, the winds, Chorda's pianist, Tympanum's drummer).
//!
//! - [`rig`]: turns, bones' transforms and aims, the two-bone reach.
//! - [`play`]: the **PlayRig** contract an instrument fills (the seat, the head, each hand's
//!   pose or grip, the occluders) and the poser for the realistic figures
//!   (`bowed_human`'s `.fig` bodies).
//! - [`grip`]: the grip library (a bow at its frog, a rod).
//! - [`live`]: the occluding capsules the live limbs are shaded by.
//!
//! Baking stays where it is cached (`bowed_mesh::mesh_for`: per instrument and holder,
//! nothing at rest); the stylised musician and the mannequin keep their own skeleton
//! (`bowed_figure`).

pub mod grip;
pub mod live;
pub mod play;
pub mod rig;
