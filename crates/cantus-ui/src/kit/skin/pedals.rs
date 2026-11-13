//! The palette of pedals (the fx side round's Fragor, ADR 0131): each enclosure's finish as
//! reviewed data, like a unit's sheet (ADR 0074 §1) — the painter
//! (`views/fx/pedal_draw.rs`) lights these and names no colour of its own. Any effect
//! drawn as a pedal (a future board of every Cantus effect) picks a finish here.
//!
//! The finishes are Cantus's own: no product's colours, graphics or lettering. A pedal is
//! a physical object on a board, so — like a display (ADR 0074 §7) — it keeps its paint in
//! every theme; the theme only changes the board under it and the light on it.

use super::super::color::Hsl;
use super::Pair;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// How an enclosure is painted.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Finish {
    /// The enclosure's paint.
    pub body: Hsl,
    /// The accent printed in its artwork (one colour beside the lettering).
    pub band: Hsl,
    /// The lettering and the knobs' scale printed on the paint.
    pub ink: Hsl,
    /// The knobs' caps.
    pub cap: Hsl,
    /// The LED when lit.
    pub led: Hsl,
    /// Metal flake or brushed steel in the paint (a finer sheen), or plain enamel.
    pub metallic: bool,
}

/// Green overdrive: deep jade enamel with a pale celadon band.
pub const JADE: Finish = Finish {
    body: hsl(158.0, 0.42, 0.30),
    band: hsl(150.0, 0.30, 0.78),
    ink: hsl(150.0, 0.25, 0.94),
    cap: hsl(160.0, 0.08, 0.12),
    led: hsl(4.0, 0.95, 0.56),
    metallic: true,
};

/// Op-amp distortion: graphite, a crimson band, white lettering.
pub const GRAPHITE: Finish = Finish {
    body: hsl(220.0, 0.06, 0.17),
    band: hsl(352.0, 0.70, 0.46),
    ink: hsl(0.0, 0.0, 0.92),
    cap: hsl(220.0, 0.05, 0.08),
    led: hsl(4.0, 0.95, 0.56),
    metallic: false,
};

/// Silicon distortion: burnt copper, a dark band.
pub const COPPER: Finish = Finish {
    body: hsl(22.0, 0.62, 0.40),
    band: hsl(20.0, 0.30, 0.16),
    ink: hsl(30.0, 0.30, 0.95),
    cap: hsl(20.0, 0.10, 0.10),
    led: hsl(4.0, 0.95, 0.56),
    metallic: true,
};

/// Germanium fuzz: oxblood hammertone, an ivory band.
pub const OXBLOOD: Finish = Finish {
    body: hsl(355.0, 0.55, 0.28),
    band: hsl(40.0, 0.35, 0.84),
    ink: hsl(38.0, 0.30, 0.92),
    cap: hsl(0.0, 0.05, 0.10),
    led: hsl(48.0, 0.98, 0.58),
    metallic: false,
};

/// Triangle muff: brushed steel, an ink-blue band.
pub const STEEL: Finish = Finish {
    body: hsl(210.0, 0.06, 0.62),
    band: hsl(222.0, 0.55, 0.30),
    ink: hsl(222.0, 0.45, 0.18),
    cap: hsl(220.0, 0.06, 0.12),
    led: hsl(4.0, 0.95, 0.56),
    metallic: true,
};

/// Fragor's five pedals' finishes, by the DSP's model index.
pub const FRAGOR: [Finish; 5] = [JADE, GRAPHITE, COPPER, OXBLOOD, STEEL];

/// The board the pedals stand on (its slats), the cables' jacket, the jacks' nickel and
/// the footswitches' chrome — one per theme mode.
pub const BOARD: Pair = Pair {
    dark: hsl(225.0, 0.08, 0.11),
    light: hsl(220.0, 0.08, 0.30),
};
pub const BOARD_SLAT: Pair = Pair {
    dark: hsl(225.0, 0.07, 0.15),
    light: hsl(220.0, 0.07, 0.36),
};
pub const CABLE: Hsl = hsl(220.0, 0.08, 0.10);
pub const CABLE_SHEEN: Hsl = hsl(220.0, 0.10, 0.34);
pub const NICKEL: Hsl = hsl(210.0, 0.06, 0.72);
pub const CHROME_DARK: Hsl = hsl(215.0, 0.08, 0.30);
pub const CHROME_LIGHT: Hsl = hsl(210.0, 0.10, 0.93);
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);

// The valve amp's parts (Fragor's tube strip, `views/fx/valve_draw.rs`): a physical object
// too, the same in every theme.
pub const GRILLE: Hsl = hsl(30.0, 0.06, 0.13);
pub const GRILLE_TOP: Hsl = hsl(30.0, 0.06, 0.19);
pub const GRILLE_HOLE: Hsl = hsl(30.0, 0.10, 0.05);
/// A filament's light: warm orange-amber.
pub const FILAMENT: Hsl = hsl(30.0, 1.0, 0.56);
pub const BAKELITE: Hsl = hsl(20.0, 0.10, 0.07);
pub const BAKELITE_TOP: Hsl = hsl(20.0, 0.08, 0.16);
pub const PLATE: Hsl = hsl(220.0, 0.04, 0.30);
pub const PLATE_SEAM: Hsl = hsl(220.0, 0.04, 0.18);
pub const MICA: Hsl = hsl(40.0, 0.20, 0.80);
pub const GETTER: Hsl = hsl(210.0, 0.08, 0.72);
pub const GLASS: Hsl = hsl(195.0, 0.15, 0.88);
/// An amber jewel pilot light.
pub const JEWEL: Hsl = hsl(34.0, 1.0, 0.52);
pub const BRASS: Hsl = hsl(42.0, 0.55, 0.42);
pub const BRASS_LIGHT: Hsl = hsl(44.0, 0.60, 0.62);
pub const BRASS_DARK: Hsl = hsl(38.0, 0.55, 0.20);
