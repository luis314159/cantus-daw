//! fxa:loquax — the palette of the objects Loquax draws (the fx side round, wave A, ADR 0139
//! provisional): the vocoder's gooseneck microphone (its satin-black head, the woven grille,
//! the ribbed neck, the chrome socket), the bezels its screen and meters sit in (one per
//! colourway: brushed aluminium, black lacquer, the road case's steel), the meter bridge's
//! smoked faces and needles. Reviewed data, like a unit's sheet (ADR 0074 §1): the painter
//! (`views/fx/loquax_art.rs`) lights these and names no colour of its own. Objects, so —
//! like a display (ADR 0074 §7) — they keep their colours in every theme, on the dark
//! screen.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// The microphone's head and body: satin black, its edge light, the grille's woven wire
/// over the dark foam behind it.
pub const SATIN: Hsl = hsl(220.0, 0.06, 0.09);
pub const SATIN_LIGHT: Hsl = hsl(215.0, 0.06, 0.34);
pub const WIRE: Hsl = hsl(210.0, 0.05, 0.62);
pub const WIRE_SHADE: Hsl = hsl(215.0, 0.06, 0.30);
pub const FOAM: Hsl = hsl(220.0, 0.08, 0.05);

/// The gooseneck: dark nickel coils, their lit ridges.
pub const COIL: Hsl = hsl(212.0, 0.05, 0.22);
pub const COIL_LIGHT: Hsl = hsl(210.0, 0.06, 0.62);
pub const COIL_SHADE: Hsl = hsl(220.0, 0.06, 0.07);

/// Chrome: the socket, the rings, the screws.
pub const CHROME: Hsl = hsl(210.0, 0.07, 0.72);
pub const CHROME_LIGHT: Hsl = hsl(205.0, 0.10, 0.93);
pub const CHROME_DARK: Hsl = hsl(215.0, 0.08, 0.28);

/// A bezel's three finishes (the colourways): its lit top, its face, its shadowed foot.
pub struct Finish {
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    /// The grain or the sheen's lines.
    pub grain: Hsl,
}

/// Brushed aluminium (Aluminium).
pub const ALUMINIUM: Finish = Finish {
    top: hsl(205.0, 0.06, 0.80),
    face: hsl(208.0, 0.05, 0.62),
    foot: hsl(212.0, 0.06, 0.40),
    grain: hsl(210.0, 0.05, 0.88),
};
/// Black lacquer with a deep shine (Ink).
pub const LACQUER: Finish = Finish {
    top: hsl(230.0, 0.06, 0.30),
    face: hsl(232.0, 0.06, 0.10),
    foot: hsl(235.0, 0.08, 0.04),
    grain: hsl(220.0, 0.08, 0.70),
};
/// The road case's black steel, its edges worn bright (Road case).
pub const STEEL: Finish = Finish {
    top: hsl(210.0, 0.04, 0.36),
    face: hsl(212.0, 0.04, 0.20),
    foot: hsl(215.0, 0.05, 0.10),
    grain: hsl(210.0, 0.04, 0.52),
};

/// The bezel's dark gasket round the glass.
pub const GASKET: Hsl = hsl(220.0, 0.08, 0.03);
/// Glass: its reflection.
pub const GLASS: Hsl = hsl(195.0, 0.18, 0.90);
/// The meters' smoked faces (lit from behind), the needle, the red of the last decibels.
pub const FACE: Hsl = hsl(210.0, 0.10, 0.07);
pub const FACE_LIT: Hsl = hsl(205.0, 0.12, 0.13);
pub const NEEDLE: Hsl = hsl(40.0, 0.30, 0.90);
pub const RED: Hsl = hsl(4.0, 0.75, 0.55);
/// Shadow under a part.
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
