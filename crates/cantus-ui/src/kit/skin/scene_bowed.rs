//! r42:arcus-look — the bowed strings' palette (Arcus's cello and double bass, ADR 0157):
//! the varnished spruce top and flamed maple of the body, the bridge's pale maple, the
//! strings' steel and tungsten, the bow's pernambuco, ebony frog, pearl eye and hair, and
//! the player's fingertip — reviewed data, as the rest of the scene's palette. The scene's
//! painters (`scene/bowed*.rs`, `views/inst/chorda_scene_px_bowed.rs`,
//! `views/inst/arcus_draw*.rs`) light and mix these and name no colour of their own.

use super::{lacquer, plain, srgb, wood, Mat, MatKind, Srgb, C3};

/// A cello's top: spruce under an orange-brown Cremonese varnish (its grain's darker late
/// wood and paler early wood, a full clear coat).
pub const CELLO_TOP: Mat = wood(
    srgb(178, 96, 40),
    srgb(118, 54, 20),
    srgb(214, 138, 66),
    0.78,
    0.95,
    70.0,
    0.10,
);
/// A cello's back, ribs, neck and scroll: flamed maple under the same varnish (the flame's
/// dark and light stripes wider apart than the spruce's grain).
pub const CELLO_MAPLE: Mat = wood(
    srgb(164, 78, 30),
    srgb(96, 40, 15),
    srgb(222, 136, 62),
    0.78,
    0.95,
    70.0,
    0.16,
);
/// A double bass's top and maple: a deeper brown varnish, as an orchestra's bass wears.
pub const BASS_TOP: Mat = wood(
    srgb(140, 72, 32),
    srgb(88, 40, 16),
    srgb(180, 104, 52),
    0.70,
    0.85,
    60.0,
    0.10,
);
pub const BASS_MAPLE: Mat = wood(
    srgb(122, 58, 24),
    srgb(70, 30, 12),
    srgb(176, 98, 46),
    0.70,
    0.85,
    60.0,
    0.15,
);
/// r42:arcus-upper — a violin's top and maple: a golden-orange Cremonese varnish, lighter
/// than the cello's (the "Messiah"'s colour as its photographs show it).
pub const VIOLIN_TOP: Mat = wood(
    srgb(192, 112, 48),
    srgb(130, 64, 24),
    srgb(228, 154, 80),
    0.80,
    0.95,
    72.0,
    0.10,
);
pub const VIOLIN_MAPLE: Mat = wood(
    srgb(178, 90, 36),
    srgb(106, 46, 18),
    srgb(234, 152, 74),
    0.80,
    0.95,
    72.0,
    0.16,
);
/// r42:arcus-upper — a viola's: a deeper red-brown varnish, between the violin's and the
/// bass's.
pub const VIOLA_TOP: Mat = wood(
    srgb(166, 80, 36),
    srgb(106, 44, 18),
    srgb(206, 120, 60),
    0.76,
    0.92,
    68.0,
    0.10,
);
pub const VIOLA_MAPLE: Mat = wood(
    srgb(152, 66, 28),
    srgb(90, 34, 14),
    srgb(208, 118, 56),
    0.76,
    0.92,
    68.0,
    0.16,
);
/// The bridge: pale maple, unvarnished (a wax's faint sheen), its medullary flecks.
pub const BRIDGE: Mat = wood(
    srgb(226, 198, 150),
    srgb(196, 160, 112),
    srgb(240, 220, 182),
    0.12,
    0.3,
    20.0,
    0.06,
);
/// Ebony: the fingerboard, the nut, the pegs, the tailpiece, the saddle, the endpin's
/// collar — polished, nearly black, a little warm.
pub const EBONY: Mat = lacquer(srgb(24, 20, 18), 0.55, 0.9, 60.0);
/// The strings: plain steel (the cello's A and D), tungsten-silver wound (its G and C,
/// the bass's every string), darker.
pub const STEEL: Mat = super::metal([0.86, 0.86, 0.87]);
pub const TUNGSTEN: Mat = super::metal([0.58, 0.57, 0.58]);
/// The fine tuners' and the endpin's nickel; the endpin's rubber foot.
pub const NICKEL: Mat = super::metal([0.80, 0.80, 0.82]);
pub const RUBBER: Mat = plain(MatKind::Black, [0.0; 3]);
/// The tailgut's black cord.
pub const CORD: Mat = plain(MatKind::Black, [0.0; 3]);
/// The player's chair's seat (and the bassist's stool's): a dark cloth.
pub const SEAT: Mat = plain(MatKind::Matte, srgb(46, 38, 36));
/// Inside the body, seen through an f-hole: the dark of the box.
pub const HOLE: C3 = [0.045, 0.028, 0.018];
/// The purfling's black strips (its white core is too fine to show at the picture's size).
pub const PURFLING: C3 = [0.07, 0.05, 0.04];
/// The f-holes' cut edges: the spruce's pale end grain.
pub const CUT: C3 = [0.58, 0.38, 0.20];
/// The strings' silk wrappings at the tailpiece and the pegs (A, D, G, C).
pub const SILK: [C3; 4] = [
    [0.55, 0.12, 0.10],
    [0.70, 0.56, 0.20],
    [0.16, 0.34, 0.20],
    [0.20, 0.22, 0.42],
];

/// The live picture (drawn over the baked instrument every frame): the bow, the finger.
pub mod live {
    use super::Srgb;
    /// The bow's stick: pernambuco, its dark and its lit side.
    pub const STICK: Srgb = [86, 36, 18];
    pub const STICK_LIT: Srgb = [168, 88, 44];
    /// The frog's ebony and its lit edge, its pearl eye and slide, its silver ferrule and
    /// the stick's winding and the tip's ivory-white plate.
    pub const FROG: Srgb = [22, 18, 16];
    pub const FROG_LIT: Srgb = [78, 70, 66];
    pub const PEARL: Srgb = [228, 230, 238];
    pub const SILVER: Srgb = [206, 208, 212];
    pub const TIP: Srgb = [236, 232, 220];
    /// The horsehair ribbon, lit and in its own shade; where it grips the string (rosin
    /// and pressure darken it).
    pub const HAIR: Srgb = [240, 234, 216];
    pub const HAIR_SHADE: Srgb = [184, 174, 152];
    pub const HAIR_GRIP: Srgb = [150, 132, 104];
    /// The player's fingertip: the skin, its shade, the nail.
    pub const SKIN: Srgb = [212, 156, 124];
    pub const SKIN_SHADE: Srgb = [128, 78, 58];
    pub const NAIL: Srgb = [236, 206, 192];
    /// A shadow cast on the instrument (the bow's, the finger's).
    pub const SHADOW: Srgb = [12, 6, 4];
}

/// An sRGB colour as the scene's linear-ready triple (0 … 1).
pub const fn c3(s: Srgb) -> C3 {
    srgb(s[0], s[1], s[2])
}
