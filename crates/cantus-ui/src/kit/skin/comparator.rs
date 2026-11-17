//! fxc:comparatio — the palette of the objects Comparatio draws (the fx side round, wave C,
//! ADR 0146 provisional; design sheet v1): the console's cheeks and faceplate (one finish
//! per colourway), the two decks' meter glass, the screen's glass and graticule, the big
//! selector's chrome and its jewel, the blind drawer's keys. Reviewed data, like a unit's
//! sheet (ADR 0074 §1): the painter (`views/fx/comparatio_art.rs`) lights these and names
//! no colour of its own. Objects, so they keep their colours in every theme; the screen and
//! the meters stay dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A console's finish: the cheeks (wood or metal), the faceplate's light, face and foot,
/// its fine grain, its chamfer, the legend printed on it and the engraving's ink.
pub struct Console {
    pub cheek_light: Hsl,
    pub cheek: Hsl,
    pub cheek_dark: Hsl,
    pub cheek_grain: Hsl,
    pub top: Hsl,
    pub face: Hsl,
    pub foot: Hsl,
    pub grain: Hsl,
    pub edge: Hsl,
    pub print: Hsl,
    pub engrave: Hsl,
}

/// Walnut cheeks, a smoked dark faceplate.
pub const WALNUT: Console = Console {
    cheek_light: hsl(26.0, 0.42, 0.40),
    cheek: hsl(22.0, 0.46, 0.26),
    cheek_dark: hsl(18.0, 0.44, 0.13),
    cheek_grain: hsl(20.0, 0.50, 0.10),
    top: hsl(30.0, 0.08, 0.30),
    face: hsl(30.0, 0.07, 0.20),
    foot: hsl(28.0, 0.08, 0.11),
    grain: hsl(30.0, 0.08, 0.36),
    edge: hsl(34.0, 0.16, 0.52),
    print: hsl(38.0, 0.30, 0.82),
    engrave: hsl(28.0, 0.10, 0.06),
};
/// A broadcast desk's brushed steel, blue-grey end caps.
pub const STEEL: Console = Console {
    cheek_light: hsl(210.0, 0.12, 0.46),
    cheek: hsl(212.0, 0.14, 0.32),
    cheek_dark: hsl(214.0, 0.16, 0.18),
    cheek_grain: hsl(210.0, 0.10, 0.52),
    top: hsl(205.0, 0.06, 0.74),
    face: hsl(206.0, 0.06, 0.60),
    foot: hsl(210.0, 0.08, 0.38),
    grain: hsl(205.0, 0.08, 0.84),
    edge: hsl(200.0, 0.12, 0.92),
    print: hsl(214.0, 0.30, 0.14),
    engrave: hsl(214.0, 0.20, 0.12),
};
/// Ivory enamel, cheeks of dark teak.
pub const IVORY: Console = Console {
    cheek_light: hsl(28.0, 0.34, 0.36),
    cheek: hsl(24.0, 0.38, 0.22),
    cheek_dark: hsl(20.0, 0.40, 0.11),
    cheek_grain: hsl(20.0, 0.42, 0.08),
    top: hsl(42.0, 0.40, 0.92),
    face: hsl(40.0, 0.34, 0.84),
    foot: hsl(36.0, 0.24, 0.58),
    grain: hsl(44.0, 0.40, 0.96),
    edge: hsl(46.0, 0.50, 0.98),
    print: hsl(24.0, 0.30, 0.18),
    engrave: hsl(24.0, 0.24, 0.20),
};

/// What the console's shadows are made of.
pub const SHADOW: Hsl = hsl(24.0, 0.20, 0.04);
/// The desk the console stands on, a dark felt.
pub const DESK: Hsl = hsl(220.0, 0.10, 0.12);
pub const DESK_LIGHT: Hsl = hsl(220.0, 0.08, 0.20);

/// The screens' and meters' glass, their graticule and words.
pub const GLASS_TOP: Hsl = hsl(222.0, 0.24, 0.10);
pub const GLASS_FOOT: Hsl = hsl(226.0, 0.30, 0.05);
pub const GRATICULE: Hsl = hsl(220.0, 0.16, 0.28);
pub const SCREEN_INK: Hsl = hsl(210.0, 0.20, 0.84);
pub const SCREEN_DIM: Hsl = hsl(216.0, 0.12, 0.56);
pub const SHEEN: Hsl = hsl(210.0, 0.30, 0.92);
/// The difference curve's ink and its uncertainty.
pub const DIFF: Hsl = hsl(52.0, 0.20, 0.90);
/// The match EQ's suggestion and its *Send to Aequalis* key (phase B): an orange apart from
/// every colourway's two voices.
pub const MATCH: Hsl = hsl(26.0, 0.90, 0.62);

/// Polished chrome (the big selector, the bezels): light, body, shadow, glint.
pub const CHROME_LIGHT: Hsl = hsl(210.0, 0.10, 0.90);
pub const CHROME: Hsl = hsl(212.0, 0.08, 0.64);
pub const CHROME_DARK: Hsl = hsl(216.0, 0.10, 0.26);
pub const GLINT: Hsl = hsl(200.0, 0.30, 0.99);

/// The blind drawer's keys: their body, their top, their engraved letter.
pub const KEY_BODY: Hsl = hsl(30.0, 0.08, 0.16);
pub const KEY_TOP: Hsl = hsl(30.0, 0.06, 0.28);
pub const KEY_INK: Hsl = hsl(40.0, 0.20, 0.88);

/// A lamp unlit; a lit lamp's white-hot heart; a wrong answer's lamp.
pub const LAMP_OFF: Hsl = hsl(30.0, 0.16, 0.14);
pub const LAMP_HOT: Hsl = hsl(48.0, 0.90, 0.92);
pub const WRONG: Hsl = hsl(2.0, 0.70, 0.56);
pub const RIGHT: Hsl = hsl(132.0, 0.56, 0.52);
