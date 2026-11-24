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

/// r40:scene — the hybrid test (round 40 phase 3): the designer's dark stage for the
/// per-pixel renderer — black velvet walls, a polished stage-walnut floor.
pub const STAGE: RoomPaint = RoomPaint {
    wall: [30, 25, 22],
    wall2: [26, 22, 20],
    ceiling: [10, 9, 8],
    floor: [86, 60, 40],
    floor2: [76, 53, 35],
    joint: [44, 30, 20],
    fabric: [34, 26, 24],
    fabric2: [44, 32, 28],
    wood: [70, 50, 36],
    floor_gloss: 0.75,
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

/// r40:scene — the hybrid test: the stage's evening — one strong focused key, the room
/// barely lit around its pool.
pub const STAGE_EVENING: Light = Light {
    key: [2.1, 1.80, 1.40],
    ambient: [0.035, 0.036, 0.04],
    sky: [0.012, 0.012, 0.014],
    horizon: [0.03, 0.026, 0.022],
    ground: [0.01, 0.009, 0.008],
    softbox: [3.4, 3.1, 2.7],
    haze: [0.012, 0.011, 0.01],
    glow: [1.8, 1.35, 0.85],
    spill: 0.12,
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

// ───────────────────────────── round 40 phase 3: the projected scenes (ADR 0126) ─────────
//
// The data the scene module (`src/scene/`) lights and draws: the two moods, every finish
// and material as a parameter record, the floors, the walls, the rugs, and the few fixed
// colours of the models (lamps, LEDs, a kick's port). Colours are display-space floats
// 0 … 1 multiplied channel by channel — the designer's generator's numbers, so a port is
// checked against `kit-model.json` value for value.

/// A display-space colour, 0 … 1 a channel.
pub type C3 = [f32; 3];

/// An sRGB byte triple as a display-space colour.
pub const fn srgb(r: u8, g: u8, b: u8) -> C3 {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// A mood: the light a theme family shows the scene in (evening for a dark theme,
/// daylight for a light one) — the key, the ambient fill, what a mirror sees.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Mood {
    pub key_col: C3,
    pub key: f32,
    pub amb: f32,
    pub amb_col: C3,
    pub env_floor: f32,
    pub env_wall: f32,
    pub env_ceiling: f32,
    pub env_box: f32,
    pub strip: f32,
    pub env_tint: C3,
    pub fill_floor: f32,
    pub glow: f32,
    pub sky: C3,
    /// The air's haze over the far walls, and how dark the picture's falloff gets.
    pub haze: C3,
    pub falloff: f32,
    /// The shadows' strength.
    pub shadow: f32,
    /// The key's spot: half its light this far (cm) from its focus, and how steeply it
    /// falls off.
    pub spot_r: f32,
    pub spot_k: f32,
}

pub const MOOD_EVENING: Mood = Mood {
    key_col: [1.0, 0.90, 0.76],
    key: 1.08,
    amb: 0.24,
    amb_col: [0.80, 0.86, 1.0],
    env_floor: 0.17,
    env_wall: 0.26,
    env_ceiling: 0.40,
    env_box: 2.1,
    strip: 1.5,
    env_tint: [0.96, 0.92, 0.86],
    fill_floor: 0.10,
    glow: 1.0,
    sky: [0.10, 0.12, 0.16],
    haze: [0.08, 0.07, 0.06],
    falloff: 0.34,
    shadow: 1.0,
    spot_r: 260.0,
    spot_k: 2.4,
};

pub const MOOD_DAYLIGHT: Mood = Mood {
    key_col: [1.0, 0.98, 0.95],
    key: 0.70,
    amb: 1.02,
    amb_col: [0.95, 0.97, 1.0],
    env_floor: 0.36,
    env_wall: 0.74,
    env_ceiling: 0.92,
    env_box: 1.6,
    strip: 1.2,
    env_tint: [0.92, 0.95, 1.0],
    fill_floor: 0.28,
    glow: 0.35,
    sky: [0.80, 0.86, 0.94],
    // Warm, not grey: a daylit room's air (round 40: a grey veil washed Paper out).
    haze: [0.80, 0.74, 0.66],
    falloff: 0.10,
    shadow: 0.7,
    spot_r: 260.0,
    spot_k: 2.4,
};

/// The stage's evening (delivery 6, the owner's choice): low-key surroundings, one strong
/// focused key — the instrument stands in its own pool of light.
pub const MOOD_STAGE_EVENING: Mood = Mood {
    amb: 0.075,
    key: 1.55,
    spot_r: 190.0,
    spot_k: 3.2,
    fill_floor: 0.05,
    env_floor: 0.05,
    env_wall: 0.07,
    env_ceiling: 0.10,
    env_box: 2.6,
    strip: 1.7,
    glow: 1.0,
    ..MOOD_EVENING
};

/// The stage by daylight.
pub const MOOD_STAGE_DAYLIGHT: Mood = Mood {
    // The house lights up: warm and lower than a room's daylight, so the stage keeps its
    // depth (round 40: a blue-grey ambient washed the Paper stage out). Round 40 polish:
    // a step higher, the corners' falloff halved — the stage sits in a light window.
    amb: 0.38,
    falloff: 0.05,
    amb_col: [1.0, 0.93, 0.84],
    key: 1.25,
    spot_r: 230.0,
    spot_k: 2.8,
    fill_floor: 0.12,
    env_floor: 0.16,
    env_wall: 0.30,
    env_ceiling: 0.45,
    env_box: 2.2,
    strip: 1.5,
    glow: 0.55,
    ..MOOD_DAYLIGHT
};

/// How a material answers the light.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MatKind {
    /// A clear coat over a colour (finishes, gold, ivory, ebony; a wood blends its grain).
    Lacquer,
    /// A mirror tinted (chrome, nickel, brass): it shows the environment.
    Chrome,
    /// Black hardware: a dull dark with a tight highlight.
    Black,
    /// Felt, matte and a coated head: the ambient and the key only.
    Matte,
    /// Lathed bronze: anisotropic highlights round the cymbal.
    Bronze,
}

/// A material as parameters.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Mat {
    pub kind: MatKind,
    pub albedo: C3,
    /// A wood's darker and lighter figure.
    pub dark: C3,
    pub light: C3,
    pub coat: f32,
    pub spec: f32,
    pub shine: f32,
    pub wood: bool,
    pub figure: f32,
    /// A metal's tint.
    pub tint: C3,
}

const fn lacquer(albedo: C3, coat: f32, spec: f32, shine: f32) -> Mat {
    Mat {
        kind: MatKind::Lacquer,
        albedo,
        dark: albedo,
        light: albedo,
        coat,
        spec,
        shine,
        wood: false,
        figure: 0.07,
        tint: [1.0; 3],
    }
}

#[allow(clippy::too_many_arguments)]
const fn wood(
    albedo: C3,
    dark: C3,
    light: C3,
    coat: f32,
    spec: f32,
    shine: f32,
    figure: f32,
) -> Mat {
    Mat {
        kind: MatKind::Lacquer,
        albedo,
        dark,
        light,
        coat,
        spec,
        shine,
        wood: true,
        figure,
        tint: [1.0; 3],
    }
}

const fn metal(tint: C3) -> Mat {
    Mat {
        kind: MatKind::Chrome,
        albedo: tint,
        dark: tint,
        light: tint,
        coat: 0.0,
        spec: 0.0,
        shine: 1.0,
        wood: false,
        figure: 0.07,
        tint,
    }
}

const fn plain(kind: MatKind, albedo: C3) -> Mat {
    Mat {
        kind,
        albedo,
        dark: albedo,
        light: albedo,
        coat: 0.0,
        spec: 0.0,
        shine: 1.0,
        wood: false,
        figure: 0.07,
        tint: [1.0; 3],
    }
}

/// The finishes of a piano's case and a kit's shells.
pub const FINISH_BLACK: Mat = lacquer(srgb(11, 11, 12), 1.0, 1.3, 90.0);
pub const FINISH_WHITE: Mat = lacquer(srgb(226, 223, 214), 0.55, 1.0, 70.0);
pub const FINISH_WALNUT: Mat = wood(
    srgb(106, 70, 44),
    srgb(54, 33, 19),
    srgb(140, 98, 62),
    0.35,
    0.45,
    24.0,
    0.10,
);
pub const FINISH_ROSEWOOD: Mat = wood(
    srgb(94, 40, 27),
    srgb(38, 14, 9),
    srgb(128, 62, 40),
    0.8,
    1.0,
    60.0,
    0.12,
);
pub const FINISH_MAPLE: Mat = wood(
    srgb(168, 122, 77),
    srgb(112, 76, 44),
    srgb(206, 160, 108),
    0.8,
    1.1,
    90.0,
    0.08,
);

/// The materials every model shares.
pub mod mat {
    use super::*;
    pub const CHROME: Mat = metal([0.92, 0.92, 0.93]);
    pub const NICKEL: Mat = metal([0.80, 0.80, 0.82]);
    pub const BRASS: Mat = metal([0.92, 0.74, 0.44]);
    pub const BLACK: Mat = plain(MatKind::Black, [0.0; 3]);
    pub const FELT: Mat = plain(MatKind::Matte, [0.80, 0.78, 0.74]);
    pub const RED_FELT: Mat = plain(MatKind::Matte, srgb(128, 30, 28));
    pub const COATED: Mat = plain(MatKind::Matte, [0.90, 0.90, 0.88]);
    pub const BRONZE: Mat = plain(MatKind::Bronze, [0.78, 0.62, 0.38]);
    pub const GOLD: Mat = lacquer(srgb(184, 146, 78), 0.25, 0.8, 30.0);
    pub const SPRUCE: Mat = plain(MatKind::Matte, srgb(210, 170, 112));
    pub const IVORY: Mat = lacquer(srgb(240, 236, 224), 0.15, 0.4, 40.0);
    pub const EBONY: Mat = lacquer(srgb(20, 20, 22), 0.5, 0.8, 60.0);
    pub const MIC_BODY: Mat = metal([0.42, 0.42, 0.44]);
    pub const MIC_DARK: Mat = plain(MatKind::Black, [0.0; 3]);
    /// The tally ring of a microphone that listens.
    pub const TALLY: Mat = plain(MatKind::Matte, [0.86, 0.27, 0.20]);
    /// A wooden kick hoop's stain.
    pub const HOOP_WOOD: C3 = [0.42, 0.29, 0.17];
    /// A sheet of music on a piano's music desk (r40 integration), and its staves' ink.
    pub const PAPER: Mat = plain(MatKind::Matte, srgb(238, 232, 216));
    pub const STAFF: C3 = [0.16, 0.15, 0.14];
}

/// How a floor is laid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Laid {
    Boards,
    Slabs,
    Terrazzo,
    Stones,
}

/// A floor: its two tones, its joints, its finish's coat (which also sets how strongly it
/// mirrors), and its pieces' width (cm).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FloorPaint {
    pub laid: Laid,
    pub a: C3,
    pub b: C3,
    pub joint: C3,
    pub coat: f32,
    pub w: f32,
}

const fn floor(laid: Laid, a: C3, b: C3, joint: C3, coat: f32, w: f32) -> FloorPaint {
    FloorPaint {
        laid,
        a,
        b,
        joint,
        coat,
        w,
    }
}

pub const FLOOR_LIGHT_OAK: FloorPaint = floor(
    Laid::Boards,
    srgb(206, 170, 124),
    srgb(188, 150, 104),
    srgb(120, 92, 62),
    0.25,
    16.0,
);
pub const FLOOR_HONEY_OAK: FloorPaint = floor(
    Laid::Boards,
    srgb(150, 112, 74),
    srgb(134, 98, 64),
    srgb(80, 58, 38),
    0.30,
    16.0,
);
pub const FLOOR_DARK_WALNUT: FloorPaint = floor(
    Laid::Boards,
    srgb(92, 62, 42),
    srgb(78, 52, 35),
    srgb(40, 27, 18),
    0.40,
    18.0,
);
pub const FLOOR_STAGE_WALNUT: FloorPaint = floor(
    Laid::Boards,
    srgb(86, 60, 40),
    srgb(76, 53, 35),
    srgb(44, 30, 20),
    0.55,
    20.0,
);
pub const FLOOR_EBONY_STAGE: FloorPaint = floor(
    Laid::Boards,
    srgb(34, 30, 28),
    srgb(28, 25, 24),
    srgb(14, 12, 12),
    0.65,
    22.0,
);
pub const FLOOR_CONCRETE: FloorPaint = floor(
    Laid::Slabs,
    srgb(128, 126, 122),
    srgb(112, 110, 106),
    srgb(84, 82, 80),
    0.20,
    120.0,
);
pub const FLOOR_TERRAZZO: FloorPaint = floor(
    Laid::Terrazzo,
    srgb(196, 190, 180),
    srgb(182, 176, 166),
    srgb(150, 144, 136),
    0.35,
    60.0,
);
pub const FLOOR_FLAGSTONE: FloorPaint = floor(
    Laid::Stones,
    srgb(128, 120, 106),
    srgb(112, 104, 92),
    srgb(80, 74, 66),
    0.10,
    60.0,
);

/// The walls' surface pattern.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    Plain,
    Panels,
    Acoustic,
    Brick,
    Stone,
    /// A velvet curtain in deep folds (the stage).
    Curtain,
}

/// Walls: their two albedos, their wood (slats, fins, frames), their fabrics, their pattern.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WallPaint {
    pub wall: C3,
    pub wall2: C3,
    pub wood: C3,
    pub fabric: C3,
    pub fabric2: C3,
    pub pattern: Pattern,
}

/// A plain wall by day (Paper): the sun through a window warms and lifts a plaster or a
/// limestone that the evening's paint leaves a neutral grey (round 40 cp3: the booth's and
/// the church's walls read grey beside a daylit photograph). Per channel, on the paint.
pub const DAYLIT_WALL: C3 = [1.16, 1.09, 0.96];

/// `w` as the day lights it: its walls warmer and a little lighter, the rest as painted.
pub fn daylit(w: WallPaint) -> WallPaint {
    let lift = |c: C3| -> C3 { std::array::from_fn(|i| (c[i] * DAYLIT_WALL[i]).min(1.0)) };
    WallPaint {
        wall: lift(w.wall),
        wall2: lift(w.wall2),
        ..w
    }
}

const fn walls(
    wall: C3,
    wall2: C3,
    wood: C3,
    fabric: C3,
    fabric2: C3,
    pattern: Pattern,
) -> WallPaint {
    WallPaint {
        wall,
        wall2,
        wood,
        fabric,
        fabric2,
        pattern,
    }
}

pub const WALLS_WARM_PLASTER: WallPaint = walls(
    srgb(96, 90, 84),
    srgb(88, 83, 78),
    srgb(146, 104, 66),
    srgb(58, 66, 78),
    srgb(132, 108, 80),
    Pattern::Plain,
);
pub const WALLS_WARM_WOOD: WallPaint = walls(
    srgb(104, 92, 80),
    srgb(96, 86, 76),
    srgb(160, 116, 74),
    srgb(64, 70, 80),
    srgb(120, 98, 74),
    Pattern::Plain,
);
pub const WALLS_WALNUT_PANELS: WallPaint = walls(
    srgb(84, 58, 40),
    srgb(76, 52, 36),
    srgb(98, 66, 44),
    srgb(52, 40, 32),
    srgb(110, 80, 56),
    Pattern::Panels,
);
pub const WALLS_PLASTER: WallPaint = walls(
    srgb(214, 208, 198),
    srgb(200, 194, 184),
    srgb(196, 164, 122),
    srgb(150, 156, 164),
    srgb(180, 170, 156),
    Pattern::Plain,
);
pub const WALLS_CHARCOAL: WallPaint = walls(
    srgb(56, 58, 62),
    srgb(50, 52, 56),
    srgb(74, 76, 80),
    srgb(40, 42, 46),
    srgb(66, 68, 72),
    Pattern::Acoustic,
);
pub const WALLS_BLUE_FABRIC: WallPaint = walls(
    srgb(62, 72, 90),
    srgb(56, 64, 80),
    srgb(150, 116, 80),
    srgb(34, 54, 92),
    srgb(44, 66, 108),
    Pattern::Plain,
);
pub const WALLS_GREEN_FABRIC: WallPaint = walls(
    srgb(60, 78, 70),
    srgb(54, 70, 62),
    srgb(150, 116, 80),
    srgb(32, 70, 56),
    srgb(42, 86, 70),
    Pattern::Plain,
);
pub const WALLS_BRICK: WallPaint = walls(
    srgb(142, 74, 52),
    srgb(126, 66, 48),
    srgb(150, 110, 72),
    srgb(54, 58, 66),
    srgb(120, 96, 72),
    Pattern::Brick,
);
pub const WALLS_HALL_WOOD: WallPaint = walls(
    srgb(118, 84, 56),
    srgb(108, 76, 50),
    srgb(134, 96, 62),
    srgb(70, 52, 40),
    srgb(92, 68, 48),
    Pattern::Plain,
);
pub const WALLS_STAGE_DARK: WallPaint = walls(
    srgb(30, 25, 22),
    srgb(26, 22, 20),
    srgb(70, 50, 36),
    srgb(34, 26, 24),
    srgb(44, 32, 28),
    Pattern::Curtain,
);
pub const WALLS_LIMESTONE: WallPaint = walls(
    srgb(156, 146, 128),
    srgb(142, 132, 116),
    srgb(134, 96, 62),
    srgb(120, 40, 34),
    srgb(96, 32, 28),
    Pattern::Stone,
);

/// The motif round a rug's border.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Motif {
    /// A chain of small ovals.
    Chain,
    /// Small diamonds.
    Diamonds,
    /// Fine stripes.
    Stripes,
}

/// A rug: its field, border, the border's motif and its colour, the inner line; `day`
/// dims a saturated field by daylight.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RugPaint {
    pub field: C3,
    pub border: C3,
    pub chain: C3,
    pub line: C3,
    pub day: f32,
    pub motif: Motif,
}

pub const RUG_RED: RugPaint = RugPaint {
    field: srgb(150, 28, 26),
    border: srgb(70, 16, 14),
    chain: srgb(196, 150, 84),
    line: srgb(214, 182, 112),
    day: 0.68,
    motif: Motif::Chain,
};
pub const RUG_BLUE: RugPaint = RugPaint {
    field: srgb(36, 56, 98),
    border: srgb(18, 26, 48),
    chain: srgb(196, 170, 120),
    line: srgb(206, 196, 170),
    day: 0.80,
    motif: Motif::Diamonds,
};
pub const RUG_GREY: RugPaint = RugPaint {
    field: srgb(112, 110, 106),
    border: srgb(66, 64, 62),
    chain: srgb(170, 166, 158),
    line: srgb(196, 192, 184),
    day: 0.90,
    motif: Motif::Stripes,
};

/// The models' few fixed colours: what is light (lamps, LEDs, windows) or a detail too
/// small for a material.
pub mod ink {
    use super::C3;
    /// Shadows and the picture's falloff.
    pub const SHADE: C3 = [0.0, 0.0, 0.0];
    /// The halo under a selection's outline (the one of the two that stands further from
    /// the theme's accent), so the outline reads on any part of any picture.
    pub const HALO_DARK: C3 = [0.04, 0.045, 0.05];
    pub const HALO_LIGHT: C3 = [0.97, 0.97, 0.96];
    /// A glint of white (a window's reflection).
    pub const GLINT: C3 = [1.0, 1.0, 1.0];
    /// Cables.
    pub const CABLE: C3 = [0.07, 0.07, 0.08];
    /// A tom's air vent: its chrome ring and its hole.
    pub const VENT: C3 = [0.85, 0.87, 0.89];
    pub const VENT_HOLE: C3 = [0.12, 0.14, 0.16];
    /// A kick's port, and the ring printed round its resonant head.
    pub const PORT: C3 = [0.02, 0.02, 0.025];
    pub const KICK_RING: C3 = [0.70, 0.60, 0.38];
    /// A resonant black head's three tones (centre, middle, rim).
    /// The wound bass strings' copper, warmed under the stage's key.
    pub const COPPER_WARM: C3 = [1.08, 0.92, 0.74];
    /// A sheet of music's tint under the key: a page's warm white.
    pub const PAPER_WARM: C3 = [1.0, 0.96, 0.88];
    /// A coated head's tint over its lit white: the coating's warm off-white.
    pub const COATING: C3 = [0.93, 0.91, 0.86];
    /// A black lid's edge: the board's thickness seen round its outline.
    pub const BLACK_EDGE: C3 = [0.012, 0.011, 0.010];
    pub const BLACK_HEAD: [C3; 3] = [[0.27, 0.27, 0.29], [0.12, 0.122, 0.13], [0.05, 0.05, 0.055]];
    /// A cymbal's lathe lines (the dark groove, the bright land) and its hammer dents.
    pub const LATHE_DARK: C3 = [0.0, 0.0, 0.0];
    pub const LATHE_LIGHT: C3 = [1.0, 0.96, 0.86];
    pub const DENT_LIGHT: C3 = [1.0, 0.93, 0.75];
    pub const DENT_DARK: C3 = [0.25, 0.18, 0.08];
    /// Bronze's anisotropic highlight tint.
    pub const BRONZE_GLINT: C3 = [1.0, 0.86, 0.60];
    /// A piano's strings (wound bass, plain steel), its tuning pins, the bench's tufts.
    pub const BASS_STRING: C3 = [0.72, 0.44, 0.23];
    pub const STEEL_STRING: C3 = [0.85, 0.86, 0.88];
    pub const PIN: C3 = [0.80, 0.80, 0.82];
    pub const PIN_UPRIGHT: C3 = [0.84, 0.84, 0.86];
    pub const TUFT: C3 = [0.02, 0.02, 0.02];
    /// The fallboard's emblem, the key slip's red felt, the keybed's black.
    pub const EMBLEM: C3 = [0.84, 0.71, 0.44];
    pub const KEY_FELT: C3 = [0.50, 0.12, 0.11];
    pub const KEYBED: C3 = [0.06, 0.06, 0.07];
    pub const KEY_SIDE: C3 = [0.01, 0.01, 0.012];
    pub const DAMPER: C3 = [0.04, 0.04, 0.045];
    /// Warm lamplight (booth lamp, uplights, their pools), a lamp's bulb by evening and by
    /// day.
    pub const LAMP: C3 = [1.0, 0.82, 0.55];
    pub const BULB_EVENING: C3 = [1.0, 0.92, 0.75];
    pub const BULB_DAY: C3 = [0.8, 0.78, 0.72];
    pub const UPLIGHT: C3 = [1.0, 0.78, 0.48];
    pub const WASH: C3 = [1.0, 0.80, 0.52];
    pub const UPLIGHT_FOOT: C3 = [1.0, 0.86, 0.6];
    /// The stage: the uplights' warmth on the curtain, its crest, their feet, the rig's
    /// lamps far up.
    pub const CURTAIN_WARM: C3 = [1.0, 0.74, 0.44];
    pub const CURTAIN_CREST: C3 = [1.0, 0.9, 0.75];
    pub const STAGE_FOOT: C3 = [1.0, 0.88, 0.64];
    pub const RIG_LAMP: C3 = [1.0, 0.86, 0.62];
    /// The control room's window: its glass by evening and by day, its screen, its LEDs.
    pub const GLASS_EVENING: C3 = [0.05, 0.06, 0.08];
    pub const GLASS_DAY: C3 = [0.30, 0.34, 0.40];
    pub const SCREEN: C3 = [0.24, 0.40, 0.62];
    pub const LEDS: [C3; 4] = [
        [1.0, 0.67, 0.24],
        [0.35, 0.90, 0.47],
        [0.94, 0.31, 0.24],
        [0.35, 0.59, 1.0],
    ];
    /// A church's windows by day and by evening.
    pub const WINDOW_DAY: C3 = [1.0, 0.97, 0.90];
    pub const WINDOW_EVENING: C3 = [0.55, 0.62, 0.85];
    /// Terrazzo's chips.
    pub const CHIP_DARK: C3 = [0.32, 0.30, 0.28];
    pub const CHIP_LIGHT: C3 = [0.95, 0.93, 0.90];
}

/// r42:arcus-look — the bowed strings' materials (Arcus's cello and double bass).
#[path = "scene_bowed.rs"]
pub mod bowed;
