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
//! - [`stylised`]: the stylised musician's and the mannequin's poser on the same contract.
//! - [`neck_hand`]: the left hand on a violin's or a viola's neck, posed from the table
//!   authored offline against the owner's photographs.
//!
//! Baking stays where it is cached (`bowed_mesh::mesh_for`: per instrument and holder,
//! nothing at rest); the stylised musician and the mannequin keep their own skeleton
//! (`bowed_figure`).

pub mod cards;
pub mod cloth; // r42:arcus-polish
pub mod concert; // r42:arcus-polish3
pub mod grip;
pub mod live;
pub mod neck_hand;
pub mod play;
pub mod rig;
pub mod stylised;

/// r42:arcus-upper — how finely the figures' live arms and hands are drawn (the owner,
/// 2026-11-24: "I'd rather it look good than save ms, or make it configurable in case
/// someone wants fewer ms"). A preference of this machine, never the project's.
///
/// - **High** (the default): the full look — every triangle of the hand the offline
///   thinning keeps, its light from the kept normal map, the creases' occlusion.
/// - **Light**: fewer triangles (the arm's vertices merged per bone on a coarser grid, more
///   so the smaller the hand is on screen), the light per corner, no creases' occlusion —
///   within the views' +1 ms budget; the grip, the hand behind the neck and the fingertip on
///   its stop exactly as High.
/// - **Auto**: High, stepping down to Light while the window's frames run long (their
///   average over the last two seconds over 12 ms) and back once they are short again
///   (under 7 ms), as FL Studio lowers its visuals under load. Off unless chosen.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    High,
    Light,
    Auto,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::High, Quality::Light, Quality::Auto];

    /// Its name's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            Quality::High => "arcus_upper.quality.high",
            Quality::Light => "arcus_upper.quality.light",
            Quality::Auto => "arcus_upper.quality.auto",
        }
    }

    /// Whether the light level is drawn, `long` saying the window's frames run long (only
    /// `Auto` asks).
    pub fn light(self, long: bool) -> bool {
        match self {
            Quality::High => false,
            Quality::Light => true,
            Quality::Auto => long,
        }
    }
}

/// r42:arcus-upper — `Auto`'s state: whether the window's frames run long, from their
/// average (ms) and the last state (stepping down over 12 ms, back under 7 ms).
pub fn frames_long(avg_ms: f32, was: bool) -> bool {
    if was {
        avg_ms > 7.0
    } else {
        avg_ms > 12.0
    }
}

/// r42:arcus-polish2 — how the realistic figures are lit (the owner, 2026-11-25: "an
/// orchestral concert presentation, dramatic, elegant and professional"). A preference of
/// this machine; the picture is baked again when it changes.
///
/// - **Concert** (the default): the stage — warm spots from above and in front, the hall dim
///   behind, the figure's edge lit from behind (`Room::Stage`).
/// - **Studio**: the even studio light the pictures had before.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Lighting {
    #[default]
    Concert,
    Studio,
}

impl Lighting {
    pub const ALL: [Lighting; 2] = [Lighting::Concert, Lighting::Studio];

    /// Its name's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            Lighting::Concert => "arcus_upper.light.concert",
            Lighting::Studio => "arcus_upper.light.studio",
        }
    }
}

/// r42:arcus-hand — the realistic figures' eyes (the owner, 2026-11-25: he liked them
/// closed — "parecían concentrados o disfrutando de la música" — and wants the choice). A
/// preference of this machine, never the project's; the picture is baked again when it
/// changes (it is part of the holder the picture is cached by).
///
/// - **Closed** (the default): the lids at rest over the eyes, relaxed and a little
///   downcast, the lashes on their edge (`assets/figures/<who>_lids.bin`, made by
///   `tools/arcus-figures/lids.py`).
/// - **Open**: the eyes on the strings, as the figures are built.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Eyes {
    #[default]
    Closed,
    Open,
}

impl Eyes {
    pub const ALL: [Eyes; 2] = [Eyes::Closed, Eyes::Open];

    /// Its name's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            Eyes::Closed => "arcus_upper.eyes.closed",
            Eyes::Open => "arcus_upper.eyes.open",
        }
    }
}
