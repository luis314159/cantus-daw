//! fxb:vinculum — the palette of the objects Vinculum draws (the fx side round, wave B, ADR
//! 0139 provisional): the rack the bus compressor is screwed into (its rails, their square
//! holes, the cage nuts), the faceplate in three finishes (charcoal anodised for Console,
//! cream enamel for Broadcast, brushed stainless for Steel) with the paint its engraving is
//! filled with, the knobs each finish wears, the backlit meter cards and their needles, and
//! the three gain elements seen through the faceplate's window — the console's VCA chip on
//! its board, the opto's electroluminescent panel facing its photocell, the vari-mu's
//! valves. Reviewed data, like a unit's sheet (ADR 0074 §1): the painters
//! (`views/fx/vinculum_*.rs`) light these and name no colour of their own. Objects, so they
//! keep their colours in every theme; the screens stay dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A faceplate's finish: its lit top, its face, its shadowed foot, the brush's or the
/// sheen's fine lines, the polished or chamfered edge, the paint the engraving holds, and
/// the cut's own shadow.
pub struct Plate {
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    pub grain: Hsl,
    pub edge: Hsl,
    pub legend: Hsl,
    pub cut: Hsl,
}

/// Charcoal anodised aluminium, its engraving filled with a warm white (Console).
pub const CHARCOAL: Plate = Plate {
    top: hsl(214.0, 0.08, 0.31),
    face: hsl(214.0, 0.09, 0.19),
    foot: hsl(218.0, 0.10, 0.10),
    grain: hsl(210.0, 0.08, 0.46),
    edge: hsl(208.0, 0.10, 0.58),
    legend: hsl(44.0, 0.22, 0.88),
    cut: hsl(220.0, 0.12, 0.05),
};
/// Cream enamel over steel, its engraving filled with a deep brown (Broadcast).
pub const CREAM: Plate = Plate {
    top: hsl(44.0, 0.46, 0.91),
    face: hsl(41.0, 0.36, 0.80),
    foot: hsl(36.0, 0.28, 0.60),
    grain: hsl(46.0, 0.50, 0.96),
    edge: hsl(46.0, 0.55, 0.97),
    legend: hsl(18.0, 0.42, 0.17),
    cut: hsl(28.0, 0.30, 0.40),
};
/// Brushed stainless steel, its engraving filled with black (Steel).
pub const STAINLESS: Plate = Plate {
    top: hsl(204.0, 0.07, 0.84),
    face: hsl(207.0, 0.06, 0.66),
    foot: hsl(212.0, 0.07, 0.44),
    grain: hsl(204.0, 0.07, 0.93),
    edge: hsl(204.0, 0.10, 0.97),
    legend: hsl(216.0, 0.18, 0.10),
    cut: hsl(214.0, 0.08, 0.34),
};

/// A knob's materials: its body (and the body's lit rim), its cap (the top and the foot of
/// its turned face), the pointer's line, the skirt's engraved scale.
pub struct Knob {
    pub body: Hsl,
    pub body_light: Hsl,
    pub cap_top: Hsl,
    pub cap_foot: Hsl,
    pub pointer: Hsl,
}

/// Console: black knobs with turned aluminium caps, a white pointer.
pub const KNOB_CONSOLE: Knob = Knob {
    body: hsl(222.0, 0.07, 0.07),
    body_light: hsl(214.0, 0.07, 0.34),
    cap_top: hsl(205.0, 0.07, 0.86),
    cap_foot: hsl(212.0, 0.07, 0.48),
    pointer: hsl(44.0, 0.20, 0.94),
};
/// Broadcast: brown phenolic pointer knobs, a cream line.
pub const KNOB_BROADCAST: Knob = Knob {
    body: hsl(14.0, 0.42, 0.11),
    body_light: hsl(18.0, 0.36, 0.33),
    cap_top: hsl(16.0, 0.34, 0.30),
    cap_foot: hsl(12.0, 0.40, 0.12),
    pointer: hsl(44.0, 0.48, 0.88),
};
/// Steel: satin black knobs with brushed steel caps, a red pointer.
pub const KNOB_STEEL: Knob = Knob {
    body: hsl(220.0, 0.05, 0.11),
    body_light: hsl(212.0, 0.05, 0.40),
    cap_top: hsl(204.0, 0.06, 0.90),
    cap_foot: hsl(210.0, 0.06, 0.56),
    pointer: hsl(2.0, 0.78, 0.50),
};

/// The rack: its steel rails (lit edge, face, the fold's shadow), the ears' slots, the dark
/// of the rack behind the units.
pub const RAIL_TOP: Hsl = hsl(208.0, 0.06, 0.62);
pub const RAIL: Hsl = hsl(210.0, 0.05, 0.40);
pub const RAIL_FOOT: Hsl = hsl(214.0, 0.06, 0.22);
pub const HOLE: Hsl = hsl(222.0, 0.12, 0.03);
pub const BACK: Hsl = hsl(222.0, 0.10, 0.045);

/// A rack screw: black oxide with a lit rim, on a nylon washer.
pub const SCREW: Hsl = hsl(220.0, 0.06, 0.16);
pub const SCREW_LIGHT: Hsl = hsl(212.0, 0.06, 0.58);
pub const WASHER: Hsl = hsl(42.0, 0.16, 0.78);
pub const WASHER_SHADE: Hsl = hsl(38.0, 0.12, 0.50);

/// Chrome: the meters' bezels, the toggles' levers, the lamps' rings.
pub const CHROME: Hsl = hsl(210.0, 0.07, 0.72);
pub const CHROME_LIGHT: Hsl = hsl(205.0, 0.10, 0.94);
pub const CHROME_DARK: Hsl = hsl(215.0, 0.08, 0.26);

/// The meter card lit from behind (paper, mixed with the lamp's light), its printing, the
/// needle, the red zone's paint.
pub const CARD: Hsl = hsl(46.0, 0.42, 0.90);
pub const CARD_INK: Hsl = hsl(28.0, 0.34, 0.12);
pub const NEEDLE: Hsl = hsl(220.0, 0.10, 0.06);
pub const CARD_RED: Hsl = hsl(2.0, 0.70, 0.42);

/// The window's dark gasket and its glass.
pub const GASKET: Hsl = hsl(222.0, 0.10, 0.03);
pub const GLASS: Hsl = hsl(200.0, 0.20, 0.92);
/// The inside of the unit behind the window.
pub const CAVITY: Hsl = hsl(220.0, 0.10, 0.06);

/// The VCA: its board, the copper and the gilded pads, the chip's body and legs, the
/// control voltage's light.
pub const BOARD: Hsl = hsl(152.0, 0.42, 0.15);
pub const BOARD_LIGHT: Hsl = hsl(150.0, 0.36, 0.24);
pub const COPPER: Hsl = hsl(32.0, 0.52, 0.46);
pub const GOLD: Hsl = hsl(46.0, 0.66, 0.58);
pub const CHIP: Hsl = hsl(222.0, 0.06, 0.09);
pub const CHIP_LIGHT: Hsl = hsl(216.0, 0.06, 0.30);
pub const LEG: Hsl = hsl(205.0, 0.06, 0.74);
pub const SILK: Hsl = hsl(48.0, 0.18, 0.86);

/// The opto: the electroluminescent panel's light (dark and lit), the photocell's
/// cadmium track and its silver electrodes, the cell's black housing.
pub const EL_DARK: Hsl = hsl(176.0, 0.20, 0.16);
pub const EL_LIT: Hsl = hsl(172.0, 0.78, 0.64);
pub const CDS: Hsl = hsl(26.0, 0.58, 0.36);
pub const ELECTRODE: Hsl = hsl(206.0, 0.08, 0.80);
pub const HOUSING: Hsl = hsl(220.0, 0.06, 0.08);
pub const HOUSING_LIGHT: Hsl = hsl(214.0, 0.06, 0.32);

/// The vari-mu: the valves' glass, the plates' grey metal, the mica spacers, the getter's
/// silvering, the heaters' glow (dark and lit), the anode's blue glow under heavy work.
pub const VALVE_GLASS: Hsl = hsl(196.0, 0.20, 0.82);
pub const ANODE: Hsl = hsl(214.0, 0.06, 0.30);
pub const ANODE_LIGHT: Hsl = hsl(210.0, 0.07, 0.58);
pub const MICA: Hsl = hsl(42.0, 0.30, 0.70);
pub const GETTER: Hsl = hsl(210.0, 0.12, 0.70);
pub const HEATER: Hsl = hsl(20.0, 0.92, 0.52);
pub const HEATER_HOT: Hsl = hsl(34.0, 0.98, 0.70);
pub const FLUORESCE: Hsl = hsl(222.0, 0.80, 0.66);

/// The jewel lamps' lenses: the key (amber), the release's Auto (green), the limit (red is
/// the face's second voice), the valve colour (orange).
pub const LENS_AMBER: Hsl = hsl(38.0, 0.92, 0.56);
pub const LENS_GREEN: Hsl = hsl(128.0, 0.62, 0.50);
pub const LENS_ORANGE: Hsl = hsl(22.0, 0.92, 0.56);
pub const LENS_RED: Hsl = hsl(356.0, 0.80, 0.54);

/// Shadow under a part.
pub const SHADOW: Hsl = hsl(0.0, 0.0, 0.0);
