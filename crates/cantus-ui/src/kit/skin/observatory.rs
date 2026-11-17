//! fxa:caelum — the palette of the objects Caelum draws (the fx side round, wave A, ADR 0141
//! provisional; design sheet v2): the observatory's window and its sill, the astrolabe the
//! voices are read on, the rack of glass tubes the decay glows in. One metal per colourway
//! (lacquered brass for Night, rose gold for Dawn, a bronze whose engraving holds verdigris
//! for Aurora); the glass, the lamps' lenses, the frost. Reviewed data, like a unit's sheet
//! (ADR 0074 §1): the painter (`views/fx/caelum_art.rs`) lights these and names no colour of
//! its own. Objects, so they keep their colours in every theme; the sky stays dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A metal: its lit top, its face, its shadowed foot, the fine lines a lathe or a brush
/// leaves, the polished edge, and the ink an engraving holds.
pub struct Metal {
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    pub grain: Hsl,
    pub edge: Hsl,
    pub engrave: Hsl,
}

/// Lacquered brass, warm and polished (Night).
pub const BRASS: Metal = Metal {
    top: hsl(44.0, 0.56, 0.66),
    face: hsl(40.0, 0.50, 0.43),
    foot: hsl(33.0, 0.48, 0.19),
    grain: hsl(46.0, 0.62, 0.80),
    edge: hsl(48.0, 0.72, 0.86),
    engrave: hsl(28.0, 0.45, 0.11),
};
/// Rose gold, soft and pale (Dawn).
pub const ROSE: Metal = Metal {
    top: hsl(18.0, 0.46, 0.78),
    face: hsl(13.0, 0.36, 0.57),
    foot: hsl(6.0, 0.30, 0.29),
    grain: hsl(20.0, 0.52, 0.90),
    edge: hsl(24.0, 0.62, 0.93),
    engrave: hsl(352.0, 0.34, 0.17),
};
/// Dark bronze, its engraving filled with verdigris (Aurora).
pub const BRONZE: Metal = Metal {
    top: hsl(36.0, 0.36, 0.47),
    face: hsl(32.0, 0.30, 0.27),
    foot: hsl(28.0, 0.32, 0.11),
    grain: hsl(164.0, 0.32, 0.56),
    edge: hsl(40.0, 0.46, 0.64),
    engrave: hsl(164.0, 0.38, 0.72),
};

/// The dark gasket round a window's glass.
pub const GASKET: Hsl = hsl(232.0, 0.16, 0.03);
/// Glass: its reflection and the light along its edge.
pub const GLASS: Hsl = hsl(212.0, 0.30, 0.93);
/// A star's white core.
pub const STAR: Hsl = hsl(222.0, 0.40, 0.96);
/// A lamp's lens unlit: dark glass.
pub const LENS: Hsl = hsl(232.0, 0.14, 0.10);
/// The frost a frozen tank lays on the glass, and the freeze lamp's light.
pub const ICE: Hsl = hsl(194.0, 0.78, 0.80);
/// A pushbutton's black cap and its lit edge.
pub const CAP: Hsl = hsl(230.0, 0.10, 0.09);
pub const CAP_LIGHT: Hsl = hsl(226.0, 0.10, 0.34);
/// Shadow under a part.
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
