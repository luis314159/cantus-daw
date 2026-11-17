//! fxb:cedo — the palette of the objects Cedo draws (the fx side round, wave B, ADR 0144
//! provisional): the bed the ducking engine stands on in three finishes (a machine tool's
//! hammered grey-green enamel for Workshop, gloss black lined in red for Locomotive, a
//! mahogany plinth edged in brass for Exhibition), the engine's paint, its brass and
//! steel, the bellows' leather and boards, the cast nameplate, the solenoid's copper, the
//! level gauge's enamel and the screen's riveted frame. Reviewed data, like a unit's sheet
//! (ADR 0074 §1): the painters (`views/fx/cedo_*.rs`) light these and name no colour of
//! their own. Objects, so they keep their colours in every theme; the screen stays dark.
//!
//! Every value is Cantus's own: no product's colours or finish.

use super::super::color::Hsl;

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// A three-tone material: its lit edge, its body, its shadowed side.
#[derive(Copy, Clone, Debug)]
pub struct Tone {
    pub light: Hsl,
    pub mid: Hsl,
    pub dark: Hsl,
}

const fn tone(light: Hsl, mid: Hsl, dark: Hsl) -> Tone {
    Tone { light, mid, dark }
}

/// How the bed's surface is drawn.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Hammered enamel: a soft dimpled sheen.
    Hammered,
    /// Gloss lacquer: a long reflection and a lining stripe.
    Gloss,
    /// Polished wood: its grain in long curves.
    Wood,
}

/// One colourway's finishes.
#[derive(Copy, Clone, Debug)]
pub struct Finish {
    pub surface: Surface,
    /// The bed: its lit top, its face, its shadowed foot; the texture's light; the
    /// chamfer's edge.
    pub bed: Tone,
    pub grain: Hsl,
    pub edge: Hsl,
    /// The lining painted along the bed and the engine's castings.
    pub lining: Hsl,
    /// The engine's castings: the A-frame, the beam, the pedestals.
    pub iron: Tone,
    /// The bright fittings: bearings, the nameplate, the gauge's frame, the rivets.
    pub brass: Tone,
    /// The machined steel: the cam, the shaft, the rods, the flywheel's rim.
    pub steel: Tone,
    /// The bellows' leather and its boards.
    pub leather: Tone,
    pub boards: Tone,
    /// The nameplate's ground between its raised letters.
    pub name_ground: Hsl,
}

/// **Workshop**: hammered grey-green enamel, the engine painted a deeper green, brass
/// bearings, tan leather on oak boards.
pub const WORKSHOP: Finish = Finish {
    surface: Surface::Hammered,
    bed: tone(
        hsl(150.0, 0.10, 0.40),
        hsl(152.0, 0.11, 0.29),
        hsl(156.0, 0.13, 0.17),
    ),
    grain: hsl(140.0, 0.10, 0.52),
    edge: hsl(140.0, 0.08, 0.60),
    lining: hsl(46.0, 0.50, 0.62),
    iron: tone(
        hsl(146.0, 0.20, 0.42),
        hsl(150.0, 0.24, 0.27),
        hsl(156.0, 0.28, 0.13),
    ),
    brass: tone(
        hsl(46.0, 0.70, 0.78),
        hsl(42.0, 0.58, 0.52),
        hsl(36.0, 0.56, 0.24),
    ),
    steel: tone(
        hsl(200.0, 0.08, 0.86),
        hsl(206.0, 0.07, 0.58),
        hsl(212.0, 0.09, 0.30),
    ),
    leather: tone(
        hsl(30.0, 0.46, 0.52),
        hsl(26.0, 0.48, 0.36),
        hsl(22.0, 0.50, 0.18),
    ),
    boards: tone(
        hsl(36.0, 0.42, 0.62),
        hsl(32.0, 0.40, 0.44),
        hsl(26.0, 0.42, 0.24),
    ),
    name_ground: hsl(150.0, 0.22, 0.14),
};

/// **Locomotive**: gloss black lined in red, polished brass, oxblood leather on black
/// boards.
pub const LOCOMOTIVE: Finish = Finish {
    surface: Surface::Gloss,
    bed: tone(
        hsl(220.0, 0.08, 0.24),
        hsl(222.0, 0.10, 0.10),
        hsl(224.0, 0.12, 0.04),
    ),
    grain: hsl(210.0, 0.10, 0.70),
    edge: hsl(214.0, 0.08, 0.46),
    lining: hsl(356.0, 0.72, 0.48),
    iron: tone(
        hsl(220.0, 0.08, 0.30),
        hsl(222.0, 0.10, 0.12),
        hsl(224.0, 0.12, 0.05),
    ),
    brass: tone(
        hsl(48.0, 0.80, 0.80),
        hsl(44.0, 0.66, 0.54),
        hsl(38.0, 0.62, 0.26),
    ),
    steel: tone(
        hsl(204.0, 0.10, 0.90),
        hsl(208.0, 0.08, 0.62),
        hsl(214.0, 0.10, 0.32),
    ),
    leather: tone(
        hsl(354.0, 0.46, 0.40),
        hsl(352.0, 0.52, 0.26),
        hsl(350.0, 0.56, 0.12),
    ),
    boards: tone(
        hsl(220.0, 0.08, 0.30),
        hsl(222.0, 0.10, 0.14),
        hsl(224.0, 0.12, 0.06),
    ),
    name_ground: hsl(356.0, 0.66, 0.30),
};

/// **Exhibition**: a nickel-plated model on a polished mahogany plinth edged in brass,
/// bottle-green leather on walnut boards.
pub const EXHIBITION: Finish = Finish {
    surface: Surface::Wood,
    bed: tone(
        hsl(14.0, 0.50, 0.34),
        hsl(10.0, 0.54, 0.22),
        hsl(6.0, 0.56, 0.11),
    ),
    grain: hsl(18.0, 0.56, 0.42),
    edge: hsl(44.0, 0.66, 0.66),
    lining: hsl(44.0, 0.62, 0.58),
    iron: tone(
        hsl(205.0, 0.08, 0.90),
        hsl(210.0, 0.07, 0.66),
        hsl(216.0, 0.09, 0.34),
    ),
    brass: tone(
        hsl(46.0, 0.72, 0.78),
        hsl(42.0, 0.60, 0.52),
        hsl(36.0, 0.58, 0.24),
    ),
    steel: tone(
        hsl(200.0, 0.06, 0.94),
        hsl(206.0, 0.06, 0.70),
        hsl(212.0, 0.08, 0.38),
    ),
    leather: tone(
        hsl(150.0, 0.30, 0.34),
        hsl(152.0, 0.36, 0.21),
        hsl(156.0, 0.42, 0.09),
    ),
    boards: tone(
        hsl(24.0, 0.40, 0.40),
        hsl(20.0, 0.42, 0.26),
        hsl(16.0, 0.46, 0.12),
    ),
    name_ground: hsl(8.0, 0.56, 0.16),
};

/// The shadow under every part.
pub const SHADOW: Hsl = hsl(220.0, 0.30, 0.03);
/// The glint on polished metal and on the glass.
pub const GLINT: Hsl = hsl(48.0, 0.30, 0.97);
/// The solenoid's winding (copper enamel wire).
pub const COPPER: Tone = tone(
    hsl(24.0, 0.72, 0.62),
    hsl(20.0, 0.70, 0.40),
    hsl(16.0, 0.72, 0.18),
);
/// The solenoid's cheeks.
pub const BAKELITE: Tone = tone(
    hsl(18.0, 0.30, 0.30),
    hsl(14.0, 0.36, 0.16),
    hsl(10.0, 0.40, 0.07),
);
/// The level gauge's enamel card and its black figures.
pub const ENAMEL: Hsl = hsl(46.0, 0.30, 0.90);
pub const ENAMEL_SHADE: Hsl = hsl(40.0, 0.22, 0.72);
pub const FIGURES: Hsl = hsl(220.0, 0.20, 0.10);
/// The gauge's red index.
pub const INDEX: Hsl = hsl(2.0, 0.78, 0.46);
/// The screen's black behind its glass.
pub const SCREEN_BACK: Hsl = hsl(210.0, 0.30, 0.03);
/// The pilot lamp's unlit glass.
pub const LAMP_OFF: Hsl = hsl(30.0, 0.20, 0.16);
