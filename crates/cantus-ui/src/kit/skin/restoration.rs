//! fxc:purus — the palette of the objects Purus draws (the fx side round, wave C, ADR 0142;
//! design sheet v1): the restorer's cutting mat, the instrument's machined
//! housing (one finish per colourway), the screen's glass and its spectrogram's heat, the
//! steel of the tuning fork, the loupe's lens, the cartridges' levers and lamps. Reviewed
//! data, like a unit's sheet (ADR 0074 §1): the painter (`views/fx/purus_art.rs`) lights
//! these and names no colour of its own. Objects, so they keep their colours in every theme;
//! the screen stays dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A finish: its lit top, its face, its shadowed foot, the fine lines a mill leaves, the
/// polished chamfer, the ink an engraving holds, and the words printed on it.
pub struct Finish {
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    pub grain: Hsl,
    pub edge: Hsl,
    pub engrave: Hsl,
    pub print: Hsl,
}

/// A cutting mat: its ground, its millimetre lines, its centimetre lines, its printed
/// numbers.
pub struct Mat {
    pub ground: Hsl,
    pub fine: Hsl,
    pub bold: Hsl,
    pub print: Hsl,
}

/// Champagne enamel hammered over metal (Champagne).
pub const CHAMPAGNE: Finish = Finish {
    top: hsl(42.0, 0.30, 0.82),
    face: hsl(40.0, 0.24, 0.68),
    foot: hsl(34.0, 0.20, 0.38),
    grain: hsl(44.0, 0.30, 0.90),
    edge: hsl(46.0, 0.40, 0.94),
    engrave: hsl(30.0, 0.30, 0.18),
    print: hsl(28.0, 0.30, 0.14),
};
/// White enamel over steel (Surgical).
pub const ENAMEL: Finish = Finish {
    top: hsl(210.0, 0.10, 0.96),
    face: hsl(210.0, 0.08, 0.88),
    foot: hsl(214.0, 0.10, 0.58),
    grain: hsl(210.0, 0.12, 0.99),
    edge: hsl(205.0, 0.20, 0.99),
    engrave: hsl(216.0, 0.30, 0.22),
    print: hsl(218.0, 0.34, 0.20),
};
/// Graphite: a dark carbon-filled body (Graphite).
pub const GRAPHITE: Finish = Finish {
    top: hsl(220.0, 0.06, 0.34),
    face: hsl(222.0, 0.07, 0.22),
    foot: hsl(224.0, 0.08, 0.10),
    grain: hsl(220.0, 0.08, 0.42),
    edge: hsl(214.0, 0.10, 0.58),
    engrave: hsl(220.0, 0.10, 0.06),
    print: hsl(210.0, 0.12, 0.86),
};

/// A restorer's green mat (Champagne).
pub const MAT_GREEN: Mat = Mat {
    ground: hsl(152.0, 0.34, 0.24),
    fine: hsl(150.0, 0.24, 0.32),
    bold: hsl(140.0, 0.26, 0.46),
    print: hsl(60.0, 0.30, 0.78),
};
/// A navy mat (Surgical).
pub const MAT_NAVY: Mat = Mat {
    ground: hsl(218.0, 0.40, 0.22),
    fine: hsl(218.0, 0.28, 0.30),
    bold: hsl(210.0, 0.30, 0.46),
    print: hsl(200.0, 0.30, 0.82),
};
/// A slate mat (Graphite).
pub const MAT_SLATE: Mat = Mat {
    ground: hsl(200.0, 0.10, 0.30),
    fine: hsl(200.0, 0.08, 0.38),
    bold: hsl(196.0, 0.10, 0.52),
    print: hsl(190.0, 0.12, 0.84),
};

/// What the mat's shadows are made of.
pub const SHADOW: Hsl = hsl(160.0, 0.30, 0.04);

/// The screen: its glass at the top and the foot, its graticule, its words.
pub const GLASS_TOP: Hsl = hsl(200.0, 0.30, 0.10);
pub const GLASS_FOOT: Hsl = hsl(210.0, 0.34, 0.05);
pub const GRATICULE: Hsl = hsl(190.0, 0.20, 0.30);
pub const SCREEN_INK: Hsl = hsl(40.0, 0.24, 0.80);
pub const SCREEN_DIM: Hsl = hsl(200.0, 0.12, 0.56);
/// The glass's reflection, a cool sheen.
pub const SHEEN: Hsl = hsl(190.0, 0.30, 0.90);

/// The spectrogram's heat for what is heard, quiet to loud: a warm ember.
pub const HEAT: [Hsl; 5] = [
    hsl(250.0, 0.30, 0.06),
    hsl(330.0, 0.40, 0.16),
    hsl(16.0, 0.70, 0.36),
    hsl(36.0, 0.90, 0.58),
    hsl(50.0, 0.90, 0.86),
];

/// Polished steel (the tuning fork, the screws, the levers): its light, its body, its
/// shadow, its brightest glint.
pub const STEEL_LIGHT: Hsl = hsl(210.0, 0.10, 0.86);
pub const STEEL: Hsl = hsl(212.0, 0.08, 0.62);
pub const STEEL_DARK: Hsl = hsl(216.0, 0.10, 0.30);
pub const GLINT: Hsl = hsl(200.0, 0.30, 0.98);

/// The loupe's lens: its tint and its rim's black.
pub const LENS: Hsl = hsl(190.0, 0.40, 0.70);
pub const RIM: Hsl = hsl(220.0, 0.10, 0.10);

/// A lamp's lens unlit, and the glass of a lit one's heart.
pub const LAMP_OFF: Hsl = hsl(30.0, 0.20, 0.16);
pub const LAMP_HOT: Hsl = hsl(48.0, 0.90, 0.90);
