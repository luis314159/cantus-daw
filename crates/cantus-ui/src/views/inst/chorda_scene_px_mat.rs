//! r40:scene-px — the photographic renderer's materials: what each surface is made of, as
//! parameters a physically plausible shading reads (a clear coat over a colour, a metal's
//! tint and roughness, a lathe's or a brush's grain, felt's sheen, what glows), and the
//! procedural textures computed from a sample's place (wood figure and pores, cast iron,
//! spruce, a grille's mesh, a drum head's coating, a cymbal's lathe and dents). The
//! colours are the scene module's (`kit::skin::scene`), made linear.

use super::mesh::{lin, lmix, lscale, Lin, V3};
use crate::kit::skin::scene::{self as pal, Mat, MatKind, C3};

pub type MatId = u16;

/// How a surface answers the light.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Kind {
    /// A colour under a clear coat (lacquer, varnish, painted iron, ivory, plastic).
    Lacquer,
    /// A metal: the environment tinted, no diffuse.
    Metal,
    /// A metal with a grain (lathed bronze, brushed steel): highlights stretched across the
    /// grain (`dP/du` is the grain).
    Aniso,
    /// Felt, fabric, plaster, a coated head: diffuse with a soft sheen at grazing angles.
    Matte,
    /// A light (a lamp, a window, an LED): its own colour.
    Emit,
}

/// A texture computed from the sample's place.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Tex {
    Plain,
    /// Wood: its figure along `uv.0`, across `uv.1` (metres).
    Wood {
        seed: u32,
        figure: f32,
    },
    /// The frame's cast iron in bronze paint.
    Cast,
    /// The soundboard's spruce.
    Spruce,
    /// The room's floor, rug, walls, curtain (their paint is the room's).
    Floor,
    Rug,
    Wall,
    Wall2,
    Curtain,
    /// Woven cloth.
    Fabric,
    /// A microphone's grille.
    Grille,
    /// A drum head round `c` on `axis`, radius `r` (m): coated (mottle, collar) or black.
    Head {
        c: V3,
        axis: V3,
        r: f32,
        coated: bool,
    },
    /// A cymbal round `c` on `axis`, radius `r` (m): lathe grooves and hammer dents.
    Lathe {
        c: V3,
        axis: V3,
        r: f32,
    },
    /// The music desk's fretwork.
    Fret,
    /// r42:arcus-look — a tonewood cut on the quarter (a violin family's plates): its
    /// grain's straight lines along `uv.0`, `lines` a metre across, and, with `flame` > 0,
    /// maple's flame — the curl's bright and dark stripes across the grain.
    Tonewood {
        seed: u32,
        lines: f32,
        flame: f32,
    },
    /// r42:arcus-upper — a realistic figure's skin: its albedo's detail in `uv` (a
    /// luminance factor, a red shift); light through it warms its shade.
    Skin,
    /// Velvet: its pile's pale sheen where it turns from the eye.
    Velvet,
    /// Hair: streaks along its strands (`uv` cm along, across).
    Hair,
    /// r42:arcus-upper — a realistic figure's alpha-tested card (`figure::cards`): its
    /// texture's grey at `uv` shades the base; where its alpha is under half the raster
    /// sees through it.
    Card(u8),
}

/// A material.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PMat {
    pub kind: Kind,
    pub base: Lin,
    /// A wood's darker and lighter figure.
    pub dark: Lin,
    pub light: Lin,
    /// The clear coat's strength (0 none … 1 a full gloss).
    pub coat: f32,
    /// 0 a mirror … 1 rough.
    pub rough: f32,
    pub tex: Tex,
    pub emit: Lin,
    /// A grand lid's underside: it mirrors the frame below it.
    pub lid_under: bool,
}

impl PMat {
    pub const fn new(kind: Kind, base: Lin, coat: f32, rough: f32) -> PMat {
        PMat {
            kind,
            base,
            dark: base,
            light: base,
            coat,
            rough,
            tex: Tex::Plain,
            emit: [0.0; 3],
            lid_under: false,
        }
    }

    pub fn lacquer(base: Lin, coat: f32, rough: f32) -> PMat {
        PMat::new(Kind::Lacquer, base, coat, rough)
    }

    pub fn metal(tint: Lin, rough: f32) -> PMat {
        PMat::new(Kind::Metal, tint, 1.0, rough)
    }

    pub fn aniso(tint: Lin, rough: f32, tex: Tex) -> PMat {
        PMat {
            tex,
            ..PMat::new(Kind::Aniso, tint, 1.0, rough)
        }
    }

    pub fn matte(base: Lin) -> PMat {
        PMat::new(Kind::Matte, base, 0.0, 1.0)
    }

    pub fn emit(col: Lin) -> PMat {
        PMat {
            emit: col,
            ..PMat::new(Kind::Emit, [0.0; 3], 0.0, 1.0)
        }
    }

    pub fn with(self, tex: Tex) -> PMat {
        PMat { tex, ..self }
    }
}

/// The scene module's colour (sRGB 0…1) made linear.
pub fn lin3(c: C3) -> Lin {
    let f =
        |x: f32| egui::ecolor::linear_f32_from_gamma_u8((x.clamp(0.0, 1.0) * 255.0).round() as u8);
    [f(c[0]), f(c[1]), f(c[2])]
}

/// A finish (a piano's case, a kit's shells) as the photographic renderer paints it: the
/// designer's albedo, figure and coat; how glossy from the coat and the shine.
pub fn finish(m: &Mat, seed: u32) -> PMat {
    let rough = (1.6 / m.shine.max(4.0).sqrt() - 0.13).clamp(0.015, 0.45)
        * if m.coat > 0.9 { 0.6 } else { 1.0 };
    let coat = (0.35 + 0.65 * m.coat).min(1.0);
    let base = lin3(m.albedo);
    match m.kind {
        MatKind::Lacquer if m.wood => PMat {
            dark: lin3(m.dark),
            light: lin3(m.light),
            tex: Tex::Wood {
                seed,
                figure: m.figure,
            },
            ..PMat::lacquer(base, coat, rough)
        },
        MatKind::Lacquer => PMat::lacquer(base, coat, rough),
        MatKind::Chrome => PMat::metal(lin3(m.tint), 0.03),
        MatKind::Bronze => PMat::aniso(lin3(m.albedo), 0.25, Tex::Plain),
        MatKind::Black => PMat::lacquer(lin3([0.035, 0.035, 0.04]), 0.45, 0.22),
        MatKind::Matte => PMat::matte(base),
    }
}

/// The parts every scene shares, linear.
pub mod parts {
    use super::*;
    pub fn chrome() -> PMat {
        // Round 40 (visual): a stand's chrome a touch brushed — a mirror this sharp on a
        // thin tube beads with the rug's pattern (a rope down every mast).
        PMat::metal(lin3([0.86, 0.87, 0.88]), 0.12)
    }
    pub fn nickel() -> PMat {
        PMat::metal(lin3([0.78, 0.77, 0.75]), 0.06)
    }
    pub fn brass() -> PMat {
        PMat::metal(lin(pal::part::BRASS), 0.10)
    }
    pub fn black_hw() -> PMat {
        PMat::lacquer(lin3([0.03, 0.03, 0.034]), 0.5, 0.18)
    }
    pub fn rubber() -> PMat {
        PMat::lacquer(lin3([0.025, 0.025, 0.027]), 0.15, 0.5)
    }
    pub fn gold() -> PMat {
        // The harp's gold paint, a shade deeper than the designer's flat gold: under the
        // key it must still read gold, not cream.
        PMat::lacquer(lin3(pal::srgb(196, 160, 74)), 0.28, 0.26).with(Tex::Cast)
    }
    pub fn spruce() -> PMat {
        PMat::lacquer(lin(pal::part::SPRUCE), 0.25, 0.3).with(Tex::Spruce)
    }
    pub fn ivory() -> PMat {
        PMat::lacquer(lin(pal::part::IVORY), 0.55, 0.12)
    }
    pub fn ebony() -> PMat {
        PMat::lacquer(lin(pal::part::EBONY), 0.7, 0.08)
    }
    pub fn felt_red() -> PMat {
        PMat::matte(lin(pal::part::FELT_RED)).with(Tex::Fabric)
    }
    pub fn damper() -> PMat {
        PMat::matte(lin(pal::part::DAMPER)).with(Tex::Fabric)
    }
    pub fn hammer() -> PMat {
        PMat::matte(lin(pal::part::HAMMER))
    }
    pub fn steel() -> PMat {
        PMat::metal(lin(pal::part::STEEL), 0.12)
    }
    pub fn copper() -> PMat {
        PMat::metal(lin(pal::part::COPPER), 0.15)
    }
    pub fn mic_body() -> PMat {
        // Satin nickel, darker than chrome: a pencil under the key is a mic, not a lamp.
        PMat::aniso(lin3([0.34, 0.34, 0.35]), 0.32, Tex::Plain)
    }
    pub fn grille() -> PMat {
        PMat::metal(lin(pal::gear::GRILLE), 0.3).with(Tex::Grille)
    }
    pub fn tally() -> PMat {
        PMat::emit(lscale(lin(pal::gear::TALLY), 1.6))
    }
    pub fn felt() -> PMat {
        PMat::matte(lin3([0.80, 0.78, 0.74])).with(Tex::Fabric)
    }
}

// ---- noise -------------------------------------------------------------------------------

#[inline]
pub fn hash3(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ s.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

/// Value noise in 0…1, smooth.
pub fn noise(x: f32, y: f32, s: u32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (x - fx, y - fy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = hash3(ix, iy, s);
    let b = hash3(ix + 1, iy, s);
    let c = hash3(ix, iy + 1, s);
    let d = hash3(ix + 1, iy + 1, s);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sy
}

/// Fractal noise, `oct` octaves, in 0…1.
pub fn fbm(x: f32, y: f32, s: u32, oct: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..oct {
        sum += amp * noise(x * f, y * f, s.wrapping_add(o * 977));
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

pub fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Wood's figure at `u` (along the grain) and `v` (across), metres: 0 the early wood's pale
/// … 1 the late wood's dark, with its cathedral figure and, fine enough, its pores; `fp`
/// metres a pixel covers (finer detail fades out instead of aliasing).
pub fn wood(u: f32, v: f32, seed: u32, fp: f32) -> f32 {
    // Sliced veneer: rings that drift and swell (a cathedral now and then), long streaks
    // along the grain, never a regular stripe.
    let warp = fbm(u * 0.5, v * 2.5, seed, 3) * 2.4 + fbm(u * 0.08, v * 0.6, seed + 7, 2) * 3.0;
    let rings = v * 18.0 + warp * 2.0 + (u * 1.1 + v * 1.7).sin() * 0.3;
    let f = rings - rings.floor();
    let ring = smooth(0.5, 0.85, f)
        * (1.0 - smooth(0.9, 1.0, f))
        * (0.4 + 0.6 * fbm(u * 0.7, v * 4.0, seed + 5, 2));
    let streak = fbm(u * 1.2, v * 110.0, seed + 3, 3);
    let mut t = 0.3 * ring + 0.7 * streak;
    // Pores (under a millimetre) only where a pixel is finer than they are.
    let pore_w = (0.0003 / fp.max(1e-5)).min(1.0);
    if pore_w > 0.2 {
        let pores = noise(u * 260.0, v * 1400.0, seed + 11);
        t += 0.25 * pore_w * smooth(0.7, 0.95, pores);
    }
    t.clamp(0.0, 1.0)
}

/// r42:arcus-look — a quartered tonewood at `u` (along the grain) and `v` (across), metres:
/// 0 the early wood's pale … 1 the late wood's dark line, the flame's stripes when `flame`
/// is above 0; `fp` metres a pixel covers (a grain finer than a pixel fades to its mean).
pub fn tonewood(u: f32, v: f32, seed: u32, lines: f32, flame: f32, fp: f32) -> f32 {
    let warp = fbm(u * 2.0, v * 9.0, seed, 2) * 1.4 + (u * 3.1 + seed as f32).sin() * 0.25;
    let x = v * lines + warp;
    let f = x - x.floor();
    let late = smooth(0.55, 0.82, f) * (1.0 - smooth(0.86, 1.0, f));
    let fade = smooth(0.22, 0.55, lines * fp);
    let mut t =
        0.32 + 0.5 * (late + (0.3 - late) * fade) + 0.18 * fbm(u * 6.0, v * 40.0, seed + 3, 3);
    if flame > 0.0 {
        // The curl: stripes ~1 cm apart, leaning and wavering across the board.
        let ph = u * 520.0 + v * 140.0 + 5.0 * fbm(v * 6.0, u * 1.5, seed + 9, 2);
        let curl = 0.5 + 0.5 * ph.sin();
        let curl = smooth(0.25, 0.75, curl);
        t = t * (1.0 - 0.75 * flame) + (0.15 + 0.7 * curl) * 0.75 * flame;
    }
    t.clamp(0.0, 1.0)
}

/// A wood finish's colour at uv.
pub fn wood_col(m: &PMat, uv: [f32; 2], seed: u32, figure: f32, fp: f32) -> Lin {
    let t = wood(uv[0], uv[1], seed, fp);
    // Round 40 (visual): a veneer's figure is quieter than its two extremes — the stripes
    // read as plywood at full contrast.
    let k = (figure / 0.1).clamp(0.4, 1.0) * 0.8;
    let c = lmix(m.light, m.dark, (0.5 + (t - 0.5) * k).clamp(0.0, 1.0));
    lmix(c, m.base, 0.35)
}
