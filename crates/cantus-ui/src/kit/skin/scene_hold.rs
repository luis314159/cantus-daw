//! r42:arcus-upper — what holds an Arcus instrument in its picture (the owner, 2026-11-24):
//! the stands' metal and pads, the musician's skin, clothes and hair, the artist's wooden
//! mannequin — reviewed data, as the rest of the scene's palette. The scene's painters
//! (`scene/bowed_figure.rs`, `scene/bowed_hold.rs`, `views/inst/chorda_scene_px_bowed.rs`,
//! `views/inst/arcus_draw_body.rs`) light these and name no colour of their own.

use super::{lacquer, plain, srgb, Mat, MatKind, Srgb, C3};

/// The skin tones a musician may be shown in (light to deep), each with its shade (the
/// live hand's, drawn flat over the picture).
pub const SKINS: [Srgb; 5] = [
    [236, 196, 170],
    [218, 168, 132],
    [190, 136, 98],
    [146, 96, 64],
    [104, 66, 44],
];
pub const SKIN_SHADES: [Srgb; 5] = [
    [176, 128, 104],
    [158, 108, 80],
    [130, 84, 56],
    [96, 58, 36],
    [64, 38, 24],
];
/// The concert clothes: black, a deep navy, a wine red, a warm grey — the shirt or jacket
/// and its sleeves (the live arm's in the same colour).
pub const CLOTHS: [Srgb; 4] = [[38, 38, 42], [34, 46, 78], [104, 32, 44], [112, 108, 104]];
pub const CLOTH_SHADES: [Srgb; 4] = [[18, 18, 22], [18, 24, 44], [58, 16, 24], [70, 66, 64]];
/// The light along a sleeve's or a limb's lit side.
pub const SHEEN: Srgb = [255, 250, 240];
/// The trousers or skirt (always dark), the shoes, the hair (a warm dark brown).
pub const TROUSERS: C3 = srgb(30, 30, 34);
pub const SHOES: C3 = srgb(16, 14, 14);
pub const HAIR: C3 = srgb(58, 38, 28);
/// The face's few strokes: the closed eyes, the brows, the smile; the cheeks' warmth.
pub const FEATURE: C3 = srgb(70, 44, 34);
pub const BLUSH: C3 = srgb(226, 132, 118);
/// The artist's mannequin: light beech, a honey maple, a dark walnut — its joints a shade
/// deeper.
pub const WOODS: [Srgb; 3] = [[222, 186, 138], [196, 142, 84], [120, 78, 48]];
pub const WOOD_JOINTS: [Srgb; 3] = [[196, 158, 110], [164, 112, 60], [92, 58, 34]];
/// The stands: black powder-coated steel, their soft pads.
pub const STAND: Mat = lacquer(srgb(26, 26, 28), 0.35, 0.6, 40.0);
pub const PAD: Mat = plain(MatKind::Matte, srgb(40, 34, 32));

/// A skin, a cloth or a wood as the Drawn picture's material: matte cloth, a little sheen
/// on skin and on the waxed wood.
pub fn skin(i: u8) -> Mat {
    lacquer(c(SKINS[(i as usize).min(4)]), 0.12, 0.25, 18.0)
}
pub fn cloth(i: u8) -> Mat {
    plain(MatKind::Matte, c(CLOTHS[(i as usize).min(3)]))
}
pub fn wood(i: u8, joint: bool) -> Mat {
    let w = if joint { WOOD_JOINTS } else { WOODS };
    lacquer(c(w[(i as usize).min(2)]), 0.3, 0.45, 30.0)
}
pub const fn flat(albedo: C3) -> Mat {
    plain(MatKind::Matte, albedo)
}

fn c(s: Srgb) -> C3 {
    srgb(s[0], s[1], s[2])
}
