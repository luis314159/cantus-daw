//! fxa:fractus — the palette of the objects Fractus draws (the fx side round, wave A, ADR 0137):
//! the sequencer's machined chassis (one finish per colourway: black anodised
//! aluminium, white powder-coated steel, violet lacquer with chrome lips), the step pads
//! behind its glass, the scope's tube, the keys that play the lanes (ivory and ebony), the
//! ice of a frozen cloud. Reviewed data, like a unit's sheet (ADR 0074 §1): the painter
//! (`views/fx/fractus_art.rs`) lights these and names no colour of its own. Objects, so they
//! keep their colours in every theme; the screens stay dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A chassis finish: its lit top, its face, its shadowed foot, the grain or speckle, the
/// machined edge a cutter leaves bright, and the engraved words' ink.
pub struct Chassis {
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    pub grain: Hsl,
    pub edge: Hsl,
    pub engrave: Hsl,
    /// Brushed lines (aluminium) rather than a powder's speckle.
    pub brushed: bool,
    /// Chrome lips round the windows (lacquer).
    pub lips: bool,
}

/// Black anodised aluminium, brushed, its cut edges bright (Black).
pub const ANODISED: Chassis = Chassis {
    top: hsl(226.0, 0.06, 0.27),
    face: hsl(228.0, 0.06, 0.13),
    foot: hsl(232.0, 0.07, 0.06),
    grain: hsl(220.0, 0.05, 0.50),
    edge: hsl(214.0, 0.08, 0.74),
    engrave: hsl(220.0, 0.06, 0.80),
    brushed: true,
    lips: false,
};
/// White powder-coated steel, a fine speckle (Signal).
pub const POWDER: Chassis = Chassis {
    top: hsl(220.0, 0.10, 0.97),
    face: hsl(222.0, 0.07, 0.87),
    foot: hsl(224.0, 0.07, 0.62),
    grain: hsl(225.0, 0.08, 0.45),
    edge: hsl(210.0, 0.10, 0.99),
    engrave: hsl(226.0, 0.14, 0.20),
    brushed: false,
    lips: false,
};
/// Deep violet lacquer with chrome lips (Acid).
pub const VIOLET: Chassis = Chassis {
    top: hsl(272.0, 0.34, 0.38),
    face: hsl(270.0, 0.40, 0.17),
    foot: hsl(266.0, 0.45, 0.07),
    grain: hsl(280.0, 0.40, 0.80),
    edge: hsl(275.0, 0.30, 0.70),
    engrave: hsl(282.0, 0.30, 0.86),
    brushed: false,
    lips: true,
};

/// The dark gasket round a window's glass.
pub const GASKET: Hsl = hsl(230.0, 0.10, 0.03);
/// A step pad behind the glass, unlit: its well and its rim.
pub const PAD: Hsl = hsl(232.0, 0.12, 0.075);
pub const PAD_RIM: Hsl = hsl(228.0, 0.10, 0.17);
/// The scope's tube: its face, darker at the corners.
pub const TUBE: Hsl = hsl(200.0, 0.22, 0.07);
pub const TUBE_EDGE: Hsl = hsl(205.0, 0.20, 0.025);
/// The keys: ivory and its shade, ebony and its lit edge, the felt under them.
pub const IVORY: Hsl = hsl(42.0, 0.28, 0.90);
pub const IVORY_SHADE: Hsl = hsl(38.0, 0.16, 0.66);
pub const EBONY: Hsl = hsl(230.0, 0.10, 0.07);
pub const EBONY_LIGHT: Hsl = hsl(225.0, 0.08, 0.36);
pub const FELT: Hsl = hsl(350.0, 0.45, 0.20);
/// A frozen cloud's ice: its lamp, its frost on the glass.
pub const ICE: Hsl = hsl(192.0, 0.75, 0.74);
/// Shadow under a part.
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
