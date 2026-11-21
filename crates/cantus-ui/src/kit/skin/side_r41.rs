//! r41 — round 41's faces (Nitor; ADR 0151). **The contract's placeholder**: a plain face
//! under a key of its own with a faint tint, so `every_unit_has_a_face_of_its_own` and the
//! key's uniqueness hold from the first day. The `nitor` builder replaces it with a face
//! unlike every other unit's (hue or material), both themes, approved by the owner.

use super::{Skin, Tint};

/// Nitor's placeholder sheet (the builder's own face replaces it).
pub const NITOR: Skin = Skin {
    surface: Tint {
        hue: 52.0,
        sat: 0.18,
        amount: 0.12,
    },
    ..Skin::plain("nitor")
};
