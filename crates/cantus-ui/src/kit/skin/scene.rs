//! The palette of Chorda's scene (round 37, ADR 0104): the materials of a piano and of the
//! rooms it is recorded in, as sRGB data, and the two lights the scene is shown in — the
//! evening stage of a dark theme and the daylight of a light one. Like a unit's sheet
//! (ADR 0074 §1) it is reviewed data, not painting code: the scene's painter
//! (`views/inst/chorda_scene*.rs`) lights and mixes these in linear light and names no
//! colour of its own.
//!
//! The scene is a display (ADR 0074 §7): it keeps its own ground — a lit room — in every
//! theme, as a CRT keeps its phosphor in Paper; the theme chooses its light.

/// An sRGB colour.
pub type Srgb = [u8; 3];

/// A case finish: its colour, the grain's darker and lighter figure (wood only), and how
/// it takes light — the clear coat's reflection (0 matte … 1 a mirror's coat), the key
/// light's highlight, and how tight that highlight is.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Paint {
    pub base: Srgb,
    pub grain_dark: Srgb,
    pub grain_light: Srgb,
    pub coat: f32,
    pub spec: f32,
    pub shine: f32,
    pub wood: bool,
}

/// Polished ebony: almost all of what it shows is what it reflects.
pub const BLACK: Paint = Paint {
    base: [11, 11, 12],
    grain_dark: [0, 0, 0],
    grain_light: [0, 0, 0],
    coat: 1.0,
    spec: 1.3,
    shine: 90.0,
    wood: false,
};
/// Polished white lacquer: a warm white, a little ivory, never the page's white.
pub const WHITE: Paint = Paint {
    base: [226, 223, 214],
    grain_dark: [0, 0, 0],
    grain_light: [0, 0, 0],
    coat: 0.55,
    spec: 1.0,
    shine: 70.0,
    wood: false,
};
/// Satin walnut: open grain under a thin coat.
pub const WALNUT: Paint = Paint {
    base: [106, 70, 44],
    grain_dark: [54, 33, 19],
    grain_light: [140, 98, 62],
    coat: 0.35,
    spec: 0.45,
    shine: 24.0,
    wood: true,
};
/// Polished rosewood: deep red-brown, a strong dark figure, a full coat.
pub const ROSEWOOD: Paint = Paint {
    base: [94, 40, 27],
    grain_dark: [38, 14, 9],
    grain_light: [128, 62, 40],
    coat: 0.8,
    spec: 1.0,
    shine: 60.0,
    wood: true,
};

/// The parts of a piano that are the same whatever its finish.
pub mod part {
    use super::Srgb;
    /// The cast-iron frame's gold, its shade and its highlight.
    pub const GOLD: Srgb = [184, 146, 78];
    pub const GOLD_SHADE: Srgb = [104, 76, 36];
    pub const GOLD_LIGHT: Srgb = [240, 212, 150];
    /// The spruce soundboard and its grain.
    pub const SPRUCE: Srgb = [210, 170, 112];
    pub const SPRUCE_GRAIN: Srgb = [168, 124, 72];
    /// Plain steel and the copper-wound bass.
    pub const STEEL: Srgb = [206, 208, 210];
    pub const COPPER: Srgb = [184, 112, 58];
    /// The dampers' and the hammers' felt; the red felt of the key slip and the string rail.
    pub const DAMPER: Srgb = [30, 28, 28];
    pub const HAMMER: Srgb = [226, 220, 204];
    pub const FELT_RED: Srgb = [128, 30, 28];
    /// The keys.
    pub const IVORY: Srgb = [240, 236, 224];
    pub const KEY_GAP: Srgb = [70, 66, 62];
    pub const EBONY: Srgb = [20, 20, 22];
    /// Pedals, casters and hinges.
    pub const BRASS: Srgb = [168, 134, 78];
    /// The tuning pins' nickel.
    pub const NICKEL: Srgb = [190, 192, 196];
    /// The fallboard's lettering.
    pub const LETTERING: Srgb = [214, 182, 112];
}

/// The microphones, their stands and cables.
pub mod gear {
    use super::Srgb;
    pub const MIC_BODY: Srgb = [172, 174, 178];
    pub const MIC_DARK: Srgb = [34, 34, 37];
    pub const GRILLE: Srgb = [120, 122, 126];
    pub const STAND: Srgb = [30, 30, 33];
    pub const CHROME: Srgb = [214, 216, 220];
    pub const CABLE: Srgb = [18, 18, 20];
    /// The red ring of a microphone that listens (the pair the perspective sets).
    pub const TALLY: Srgb = [220, 70, 52];
    /// The control room's console LEDs (amber, green, red, blue), a screen's glow, the
    /// window's dark glass.
    pub const LEDS: [Srgb; 4] = [
        [255, 170, 60],
        [90, 230, 120],
        [240, 80, 60],
        [90, 150, 255],
    ];
    pub const SCREEN: Srgb = [60, 100, 150];
    pub const GLASS: Srgb = [12, 16, 20];
    /// A window's daylight (linear).
    pub const SKYLIGHT: [f32; 3] = [1.0, 1.0, 1.05];
}

/// A room's surfaces.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RoomPaint {
    /// The walls, and the second wall's (a hair different, so the corner reads).
    pub wall: Srgb,
    pub wall2: Srgb,
    pub ceiling: Srgb,
    /// The floor's boards: two tones and the joints.
    pub floor: Srgb,
    pub floor2: Srgb,
    pub joint: Srgb,
    /// The wall treatment: two fabrics, and wood.
    pub fabric: Srgb,
    pub fabric2: Srgb,
    pub wood: Srgb,
    /// How glossy the floor is (0 matte … 1 a stage's polish).
    pub floor_gloss: f32,
}

/// An intimate booth: warm grey plaster, charcoal-blue and camel absorbers, an oak floor.
pub const BOOTH: RoomPaint = RoomPaint {
    wall: [96, 90, 84],
    wall2: [88, 83, 78],
    ceiling: [70, 66, 62],
    floor: [128, 92, 60],
    floor2: [112, 80, 52],
    joint: [64, 46, 30],
    fabric: [58, 66, 78],
    fabric2: [132, 108, 80],
    wood: [146, 104, 66],
    floor_gloss: 0.15,
};
/// A studio's live room: wood slats, a control-room window, a maple floor.
pub const STUDIO: RoomPaint = RoomPaint {
    wall: [104, 92, 80],
    wall2: [96, 86, 76],
    ceiling: [62, 58, 54],
    floor: [150, 112, 74],
    floor2: [134, 98, 64],
    joint: [80, 58, 38],
    fabric: [64, 70, 80],
    fabric2: [120, 98, 74],
    wood: [160, 116, 74],
    floor_gloss: 0.3,
};
/// A hall or scoring stage: tall wooden walls, a dark polished stage floor.
pub const HALL: RoomPaint = RoomPaint {
    wall: [118, 84, 56],
    wall2: [108, 76, 50],
    ceiling: [38, 36, 34],
    floor: [86, 60, 40],
    floor2: [76, 53, 35],
    joint: [44, 30, 20],
    fabric: [70, 52, 40],
    fabric2: [92, 68, 48],
    wood: [134, 96, 62],
    floor_gloss: 0.55,
};
/// A church: pale limestone, tall windows, an old stone floor.
pub const CHURCH: RoomPaint = RoomPaint {
    wall: [156, 146, 128],
    wall2: [142, 132, 116],
    ceiling: [120, 112, 100],
    floor: [128, 120, 106],
    floor2: [112, 104, 92],
    joint: [80, 74, 66],
    fabric: [120, 40, 34],
    fabric2: [96, 32, 28],
    wood: [104, 72, 46],
    floor_gloss: 0.25,
};

/// The rug under a piano in a small room: its field, border and fine line.
pub const RUG_FIELD: Srgb = [116, 32, 28];
pub const RUG_BORDER: Srgb = [56, 24, 26];
pub const RUG_LINE: Srgb = [196, 162, 108];

/// What the scene is lit by, in linear light (1 is the white a matte surface shows at full
/// light): the key light's colour and strength, the ambient fill, the environment a gloss
/// reflects (sky over the horizon over the floor), a softbox's reflection, the haze far
/// walls take, and the windows' and lamps' glow.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Light {
    pub key: [f32; 3],
    pub ambient: [f32; 3],
    pub sky: [f32; 3],
    pub horizon: [f32; 3],
    pub ground: [f32; 3],
    pub softbox: [f32; 3],
    pub haze: [f32; 3],
    pub glow: [f32; 3],
    /// How much of the room outside the pool of light around the piano is lit (0 a dark
    /// stage … 1 an even room).
    pub spill: f32,
}

/// The evening stage (a dark theme): a warm key light on the piano, a dim room.
pub const EVENING: Light = Light {
    key: [1.55, 1.30, 1.02],
    ambient: [0.14, 0.145, 0.16],
    sky: [0.035, 0.035, 0.04],
    horizon: [0.10, 0.09, 0.08],
    ground: [0.02, 0.018, 0.016],
    softbox: [2.6, 2.4, 2.1],
    haze: [0.05, 0.045, 0.04],
    glow: [1.8, 1.35, 0.85],
    spill: 0.45,
};

/// Daylight (a light theme): an even, cooler room with the key light still on the piano.
pub const DAYLIGHT: Light = Light {
    key: [1.3, 1.26, 1.18],
    ambient: [0.46, 0.47, 0.5],
    sky: [0.6, 0.62, 0.66],
    horizon: [0.42, 0.40, 0.38],
    ground: [0.10, 0.09, 0.08],
    softbox: [2.4, 2.4, 2.4],
    haze: [0.36, 0.37, 0.39],
    glow: [1.6, 1.45, 1.2],
    spill: 0.9,
};
