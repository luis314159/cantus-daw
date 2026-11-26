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
/// r42:arcus-polish4 — the realistic man's hair: a dark brown, near black in the shade, as
/// the concert photographs' men's (ref-06, ref-10); the stylised musician keeps [`HAIR`].
pub const HAIR_DARK: C3 = srgb(46, 35, 29);
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

/// The realistic figures' (`scene::bowed_human`): the concert dress's black velvet (its
/// sheen at grazing angles is the painters'), the suit's charcoal wool, the shirt's white,
/// a deep navy tie, her light honey-blonde hair (his is `HAIR`).
pub const VELVET: C3 = srgb(16, 14, 18);
pub const VELVET_SHEEN: C3 = srgb(92, 88, 100);
pub const WOOL: C3 = srgb(30, 31, 36);
/// r42:arcus-hand — the back light along a live sleeve's edge (the owner, 2026-11-25: the
/// black sleeve got lost against the black dress): a cool, soft rim, as a stage's back
/// light catches dark cloth.
pub const CLOTH_RIM: C3 = srgb(150, 152, 170);
pub const SHIRT: C3 = srgb(232, 232, 228);
pub const TIE: C3 = srgb(30, 36, 64);
pub const HAIR_FAIR: C3 = srgb(204, 178, 134);
/// r42:arcus-upper — fair brows: a light brown, darker than the blonde (the owner's review).
pub const BROW_FAIR: C3 = srgb(122, 92, 64);
/// A realistic figure's skin tones (fair to deep): a real skin's albedo, warmer and less
/// bright than the stylised figure's (lit by the scene's key a fair skin read as white).
pub const REAL_SKINS: [Srgb; 5] = [
    [218, 166, 136],
    [196, 140, 106],
    [164, 110, 78],
    [124, 80, 54],
    [86, 54, 38],
];
/// A realistic figure's eyes under their lowered lids (whites in the lids' shade, a grey-
/// green iris), the lashes' dark brown (sheet v7).
pub const EYE_WHITE: C3 = srgb(206, 198, 192);
pub const IRIS: C3 = srgb(78, 88, 86);
pub const LASH: C3 = srgb(40, 30, 26);
/// The eyes' gloss.
pub fn eye(albedo: C3) -> Mat {
    lacquer(albedo, 0.3, 0.5, 40.0)
}
/// A realistic figure's nail: its skin paler and pinker, glossier.
pub fn nail(i: u8) -> Mat {
    let s = REAL_SKINS[(i as usize).min(4)];
    let f = |x: u8, k: f32, add: f32| ((x as f32 * k + add) / 255.0).clamp(0.0, 1.0);
    lacquer(
        [f(s[0], 1.0, 22.0), f(s[1], 1.0, 18.0), f(s[2], 1.0, 22.0)],
        0.12,
        0.2,
        30.0,
    )
}
/// A realistic figure's skin in the Drawn picture: soft, a broad faint sheen (no plastic).
pub fn real_skin(i: u8) -> Mat {
    lacquer(c(REAL_SKINS[(i as usize).min(4)]), 0.02, 0.04, 8.0)
}
/// Hair with its sheen.
pub fn hair(albedo: C3) -> Mat {
    lacquer(albedo, 0.1, 0.2, 18.0)
}

/// A skin, a cloth or a wood as the Drawn picture's material: matte cloth, a little sheen
/// on skin and on the waxed wood.
pub fn skin(i: u8) -> Mat {
    lacquer(c(SKINS[(i as usize).min(4)]), 0.12, 0.25, 18.0)
}
/// Skin a little in shade (the neck under the jaw): between the skin and its shade.
pub fn skin_shade(i: u8) -> Mat {
    let (a, b) = (SKINS[(i as usize).min(4)], SKIN_SHADES[(i as usize).min(4)]);
    let m = |k: usize| ((a[k] as u16 * 3 + b[k] as u16 * 2) / 5) as u8;
    lacquer(c([m(0), m(1), m(2)]), 0.1, 0.2, 16.0)
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
