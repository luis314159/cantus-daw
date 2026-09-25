//! Spatium's rooms and curves: pure functions of the settings, shared by the audio thread
//! and the editor, so the display draws exactly what the engine is asked to do.
//!
//! ## The decay curve
//!
//! The one number a musician sets is the **decay**: the reverberation time (RT60) in the
//! middle of the spectrum, in seconds. Everything else is a *multiplier* on it, per
//! frequency:
//!
//! ```text
//! T60(f) = decay · room(f) · brightness(f) · decay_eq(f)        clamped to [MIN_T60, MAX_T60]
//! ```
//!
//! - `room(f)` is the room model's own balance: bass that rings a little longer in a hall,
//!   air absorption that shortens the top. It is 1 at 1 kHz, so `decay` really is the
//!   mid-band RT60.
//! - `brightness(f)` shortens (or lengthens) the high end: 0.5 is neutral.
//! - `decay_eq(f)` is the curve the user draws, as up to [`DECAY_BANDS`] bands of the same
//!   shapes as the EQ (bell, shelves, notch), whose "gain" is a decay multiplier: a bell
//!   of ×2 is a bell of +6.02 dB in the log domain.
//!
//! ## The rooms
//!
//! [`ROOMS`] are twelve room models from a vocal booth to a vast space. The **Space**
//! control morphs through them continuously: every number is interpolated (geometrically
//! for times, linearly for ratios), so there are no steps. A model is data — delay-line
//! span, natural decay, bass and air ratios, the early-reflection window and density, the
//! gap before the late field, and diffusion — and the editor prints its name.

// The DSP here walks several parallel arrays (lines, sections, grid points) with one
// index; that reads clearer than zipping five iterators.
#![allow(clippy::needless_range_loop)]

use crate::aequalis::{design, Coefs, MAX_SECTIONS};
use crate::EqShape;

pub const MODELS: usize = 12;
pub const DECAY_BANDS: usize = 6;
pub const POST_BANDS: usize = 6;
/// Early reflections per side.
pub const ER_TAPS: usize = 16;
/// The shortest and longest RT60 the engine is asked for, seconds.
pub const MIN_T60: f32 = 0.05;
pub const MAX_T60: f32 = 60.0;
/// An early-reflection level at or below this (dB, as an offset) switches them off.
pub const ER_OFF_DB: f32 = -30.0;
/// The sample rate the decay curve's band shapes are designed at. The curve is a curve,
/// not a filter: designing it at a fixed rate keeps it the same at every project rate.
const CURVE_SR: f32 = 96_000.0;

/// The algorithm family.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// A natural room: the feedback delay network with early reflections.
    Room,
    /// A plate: short, dense lines, no early reflections, fast build-up, bright.
    Plate,
    /// An '80s digital unit: sparse early reflections, deep modulation, a limited
    /// bandwidth and a shimmering top.
    Vintage,
    /// A spring tank: dispersive chirps in a few long, dark loops.
    Spring,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Room, Style::Plate, Style::Vintage, Style::Spring];

    pub fn from_index(i: usize) -> Style {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// The shapes a decay band can take.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DecayShape {
    Bell,
    LowShelf,
    HighShelf,
    Notch,
}

impl DecayShape {
    pub const ALL: [DecayShape; 4] = [
        DecayShape::Bell,
        DecayShape::LowShelf,
        DecayShape::HighShelf,
        DecayShape::Notch,
    ];

    pub fn from_index(i: usize) -> DecayShape {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn eq_shape(self) -> EqShape {
        match self {
            DecayShape::Bell => EqShape::Bell,
            DecayShape::LowShelf => EqShape::LowShelf,
            DecayShape::HighShelf => EqShape::HighShelf,
            DecayShape::Notch => EqShape::Notch,
        }
    }

    /// Whether the multiplier means anything (a notch has none: it takes the frequency
    /// down to the shortest decay there is).
    pub fn has_mult(self) -> bool {
        self != DecayShape::Notch
    }
}

/// One band of the decay curve.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DecayBand {
    pub active: bool,
    pub shape: DecayShape,
    pub freq: f32,
    /// Decay multiplier at the band: 0.25 … 4.
    pub mult: f32,
    pub q: f32,
}

impl Default for DecayBand {
    fn default() -> Self {
        DecayBand {
            active: false,
            shape: DecayShape::Bell,
            freq: 1_000.0,
            mult: 1.0,
            q: 0.7,
        }
    }
}

/// One room model: everything the Space control morphs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Room {
    /// Shortest and longest late delay line, ms.
    pub min_ms: f32,
    pub max_ms: f32,
    /// The room's natural mid-band RT60, seconds.
    pub decay_s: f32,
    /// RT60 multiplier in the bass (below ~200 Hz) and at 8 kHz.
    pub bass: f32,
    pub air: f32,
    /// First and last early reflection, ms after the pre-delay.
    pub er_start_ms: f32,
    pub er_end_ms: f32,
    /// How many reflections per side (fractional while morphing; rounded when used).
    pub er_count: f32,
    /// How far the reflections fall across their window, dB.
    pub er_fall_db: f32,
    /// Where the late field starts, ms after the pre-delay.
    pub gap_ms: f32,
    /// Input diffusion, 0…1: how smeared the onset of the late field is.
    pub diffusion: f32,
}

#[allow(clippy::too_many_arguments)]
const fn room(
    min_ms: f32,
    max_ms: f32,
    decay_s: f32,
    bass: f32,
    air: f32,
    er: (f32, f32, f32, f32),
    gap_ms: f32,
    diffusion: f32,
) -> Room {
    Room {
        min_ms,
        max_ms,
        decay_s,
        bass,
        air,
        er_start_ms: er.0,
        er_end_ms: er.1,
        er_count: er.2,
        er_fall_db: er.3,
        gap_ms,
        diffusion,
    }
}

/// Twelve rooms, smallest to largest. The editor names them (`fx.space.room.N`).
pub const ROOMS: [Room; MODELS] = [
    // booth
    room(2.2, 8.0, 0.2, 1.0, 0.8, (0.9, 7.0, 10.0, 9.0), 2.0, 0.55),
    // small room
    room(
        3.5,
        12.0,
        0.35,
        1.05,
        0.75,
        (1.3, 12.0, 11.0, 10.0),
        3.0,
        0.55,
    ),
    // studio
    room(
        5.5,
        17.0,
        0.5,
        1.1,
        0.72,
        (2.0, 18.0, 12.0, 10.0),
        5.0,
        0.58,
    ),
    // chamber
    room(
        8.0,
        24.0,
        0.75,
        1.15,
        0.7,
        (3.0, 25.0, 13.0, 11.0),
        7.0,
        0.6,
    ),
    // club
    room(
        11.0,
        31.0,
        1.0,
        1.2,
        0.66,
        (4.0, 32.0, 14.0, 11.0),
        9.0,
        0.6,
    ),
    // theatre
    room(
        14.0,
        40.0,
        1.35,
        1.2,
        0.64,
        (6.0, 40.0, 14.0, 12.0),
        11.0,
        0.62,
    ),
    // church
    room(
        18.0,
        50.0,
        1.8,
        1.3,
        0.6,
        (8.0, 50.0, 15.0, 12.0),
        14.0,
        0.64,
    ),
    // concert hall
    room(
        22.0,
        62.0,
        2.3,
        1.25,
        0.62,
        (10.0, 60.0, 16.0, 13.0),
        17.0,
        0.65,
    ),
    // large hall
    room(
        27.0,
        75.0,
        3.0,
        1.3,
        0.6,
        (12.0, 72.0, 16.0, 13.0),
        20.0,
        0.66,
    ),
    // arena
    room(
        33.0,
        90.0,
        4.2,
        1.35,
        0.55,
        (15.0, 85.0, 16.0, 14.0),
        24.0,
        0.68,
    ),
    // cathedral
    room(
        40.0,
        110.0,
        6.0,
        1.4,
        0.5,
        (18.0, 100.0, 16.0, 14.0),
        28.0,
        0.7,
    ),
    // vast
    room(
        48.0,
        135.0,
        9.0,
        1.4,
        0.5,
        (22.0, 120.0, 16.0, 15.0),
        32.0,
        0.72,
    ),
];

/// The longest late line any style asks for, ms (for sizing buffers).
pub const MAX_LINE_MS: f32 = 135.0;
/// The longest early reflection, ms (the largest room at the largest size).
pub const MAX_ER_MS: f32 = 120.0 * ER_SIZE_MAX;
pub const ER_SIZE_MAX: f32 = 2.0;
/// The longest pre-delay, ms.
pub const MAX_PREDELAY_MS: f32 = 500.0;

fn geo(a: f32, b: f32, t: f32) -> f32 {
    (a.max(1e-6).ln() * (1.0 - t) + b.max(1e-6).ln() * t).exp()
}

fn lin(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The room at a Space position (0…1), interpolated between the two nearest models.
pub fn room_at(space: f32) -> Room {
    let pos = space.clamp(0.0, 1.0) * (MODELS - 1) as f32;
    let i = (pos.floor() as usize).min(MODELS - 2);
    let t = pos - i as f32;
    let (a, b) = (ROOMS[i], ROOMS[i + 1]);
    Room {
        min_ms: geo(a.min_ms, b.min_ms, t),
        max_ms: geo(a.max_ms, b.max_ms, t),
        decay_s: geo(a.decay_s, b.decay_s, t),
        bass: lin(a.bass, b.bass, t),
        air: lin(a.air, b.air, t),
        er_start_ms: geo(a.er_start_ms, b.er_start_ms, t),
        er_end_ms: geo(a.er_end_ms, b.er_end_ms, t),
        er_count: lin(a.er_count, b.er_count, t),
        er_fall_db: lin(a.er_fall_db, b.er_fall_db, t),
        gap_ms: geo(a.gap_ms, b.gap_ms, t),
        diffusion: lin(a.diffusion, b.diffusion, t),
    }
}

/// The nearest named model to a Space position.
pub fn room_index(space: f32) -> usize {
    (space.clamp(0.0, 1.0) * (MODELS - 1) as f32).round() as usize
}

/// The Space position of a model.
pub fn room_position(index: usize) -> f32 {
    index.min(MODELS - 1) as f32 / (MODELS - 1) as f32
}

/// The decay a room has of its own, seconds — what the Space control suggests.
pub fn natural_decay(space: f32) -> f32 {
    room_at(space).decay_s
}

/// A smooth step in log-frequency: 0 well below `corner`, 1 well above.
fn above(hz: f32, corner: f32) -> f32 {
    let r = (hz / corner).powi(2);
    r / (1.0 + r)
}

/// How a style changes the room it is given.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StyleShape {
    /// Scale on the late delay lines.
    pub size: f32,
    /// Extra bass and air ratio (multiplied into the room's).
    pub bass: f32,
    pub air: f32,
    /// Early reflections: level offset, dB (−∞ is none) and density scale.
    pub er_db: f32,
    pub er_density: f32,
    /// Diffusion override blend (0: the room's, 1: `diffusion`).
    pub diffusion: f32,
    pub diffusion_blend: f32,
    /// Modulation depth and rate scale.
    pub mod_depth: f32,
    pub mod_rate: f32,
    /// A low-pass on the wet signal, Hz (0: none).
    pub bandwidth_hz: f32,
}

pub fn style_shape(style: Style) -> StyleShape {
    match style {
        Style::Room => StyleShape {
            size: 1.0,
            bass: 1.0,
            air: 1.0,
            er_db: 0.0,
            er_density: 1.0,
            diffusion: 0.0,
            diffusion_blend: 0.0,
            mod_depth: 1.0,
            mod_rate: 1.0,
            bandwidth_hz: 0.0,
        },
        Style::Plate => StyleShape {
            size: 0.42,
            bass: 0.85,
            air: 1.45,
            er_db: -40.0,
            er_density: 1.0,
            diffusion: 0.78,
            diffusion_blend: 1.0,
            mod_depth: 0.6,
            mod_rate: 1.2,
            bandwidth_hz: 0.0,
        },
        Style::Vintage => StyleShape {
            size: 1.0,
            bass: 1.1,
            air: 1.3,
            er_db: -3.0,
            er_density: 0.45,
            diffusion: 0.5,
            diffusion_blend: 0.7,
            mod_depth: 2.6,
            mod_rate: 1.6,
            bandwidth_hz: 9_000.0,
        },
        Style::Spring => StyleShape {
            size: 1.0,
            bass: 0.9,
            air: 0.7,
            er_db: -40.0,
            er_density: 1.0,
            diffusion: 0.0,
            diffusion_blend: 0.0,
            mod_depth: 0.3,
            mod_rate: 1.0,
            bandwidth_hz: 4_500.0,
        },
    }
}

/// The room's own RT60 multiplier at `hz` (1 at 1 kHz).
pub fn room_shape(room: &Room, style: Style, hz: f32) -> f32 {
    let st = style_shape(style);
    let bass = (room.bass * st.bass).max(0.05);
    let air = (room.air * st.air).max(0.05);
    // Normalise so the multiplier is exactly 1 at 1 kHz.
    let lb = |f: f32| bass.log2() * (1.0 - above(f, 220.0));
    let la = |f: f32| air.log2() * above(f, 5_500.0) * 1.35;
    ((lb(hz) + la(hz)) - (lb(1_000.0) + la(1_000.0))).exp2()
}

/// Brightness as an RT60 multiplier at `hz`: 0.5 is neutral, 0 shortens the top to a
/// quarter, 1 lengthens it by 60 %.
pub fn brightness_mult(bright: f32, hz: f32) -> f32 {
    let b = bright.clamp(0.0, 1.0);
    let top = if b < 0.5 {
        (b - 0.5) * 4.0
    } else {
        (b - 0.5) * 1.36
    };
    let w = |f: f32| above(f, 3_000.0);
    (top * (w(hz) - w(1_000.0))).exp2()
}

/// Brightness also tilts the tone of the wet signal: ±3 dB of high shelf at 4 kHz.
pub fn brightness_tilt_db(bright: f32) -> f32 {
    let t = (bright.clamp(0.0, 1.0) - 0.5) * 6.0;
    if t.abs() < 1e-4 {
        0.0
    } else {
        t
    }
}

/// The target decay curve for a set of settings: the room, the brightness and the decay
/// bands designed once, then evaluated at any frequency.
#[derive(Copy, Clone, Debug)]
pub struct TargetCurve {
    pub decay: f32,
    pub room: Room,
    pub style: Style,
    pub bright: f32,
    pub eq_on: bool,
    bands: [(bool, [Coefs; MAX_SECTIONS], usize); DECAY_BANDS],
}

impl TargetCurve {
    pub fn new(
        style: Style,
        space: f32,
        decay: f32,
        bright: f32,
        eq_on: bool,
        bands: &[DecayBand; DECAY_BANDS],
    ) -> Self {
        let mut out = [(false, [Coefs::IDENTITY; MAX_SECTIONS], 0usize); DECAY_BANDS];
        for (o, b) in out.iter_mut().zip(bands) {
            if !b.active {
                continue;
            }
            let gain = 20.0 * b.mult.clamp(0.05, 20.0).log10();
            let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
            let n = design(
                b.shape.eq_shape(),
                CURVE_SR,
                b.freq.clamp(10.0, 30_000.0),
                gain,
                b.q.clamp(0.1, 20.0),
                12.0,
                &mut c,
            );
            *o = (n > 0, c, n);
        }
        TargetCurve {
            decay: decay.clamp(MIN_T60, MAX_T60),
            room: room_at(space),
            style,
            bright,
            eq_on,
            bands: out,
        }
    }

    /// The decay curve's own multiplier at `hz` (1 where it is flat).
    pub fn eq_mult(&self, hz: f32) -> f32 {
        if !self.eq_on {
            return 1.0;
        }
        let db: f32 = self
            .bands
            .iter()
            .filter(|b| b.0)
            .map(|(_, c, n)| {
                c[..*n]
                    .iter()
                    .map(|s| s.magnitude_db(CURVE_SR, hz))
                    .sum::<f32>()
            })
            .sum();
        // Clamp far notches before the exponent: the curve's floor is MIN_T60 anyway.
        10f32.powf(db.clamp(-120.0, 60.0) / 20.0)
    }

    /// One band's multiplier alone at `hz`.
    pub fn band_mult(&self, band: usize, hz: f32) -> f32 {
        let (on, c, n) = &self.bands[band];
        if !on {
            return 1.0;
        }
        let db: f32 = c[..*n].iter().map(|s| s.magnitude_db(CURVE_SR, hz)).sum();
        10f32.powf(db.clamp(-120.0, 60.0) / 20.0)
    }

    /// RT60 at `hz` before the decay curve: the decay, the room and the brightness.
    pub fn base_t60(&self, hz: f32) -> f32 {
        (self.decay * room_shape(&self.room, self.style, hz) * brightness_mult(self.bright, hz))
            .clamp(MIN_T60, MAX_T60)
    }

    /// The RT60 the engine is asked for at `hz`, seconds.
    pub fn t60(&self, hz: f32) -> f32 {
        (self.decay
            * room_shape(&self.room, self.style, hz)
            * brightness_mult(self.bright, hz)
            * self.eq_mult(hz))
        .clamp(MIN_T60, MAX_T60)
    }
}

/// A deterministic hash to [0, 1): the same pattern on every run and every machine.
pub fn hash01(i: u32, salt: u32) -> f32 {
    let mut x = i
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(salt.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// The early reflections: per side, a time after the pre-delay (ms) and a gain. Reads from
/// the same side's input, with `cross` of the other side mixed in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ErPattern {
    pub count: usize,
    pub t_ms: [[f32; ER_TAPS]; 2],
    pub gain: [[f32; ER_TAPS]; 2],
    pub cross: f32,
    /// Where the window ends, ms after the pre-delay.
    pub window_ms: f32,
}

/// The reflections a room makes at a size (0.25…2) and character. Gains are normalised to
/// unit energy per side; the level is applied separately ([`er_level_db`]).
pub fn er_pattern(room: &Room, style: Style, size: f32, character: f32) -> ErPattern {
    let st = style_shape(style);
    let size = size.clamp(0.25, ER_SIZE_MAX);
    let count = ((room.er_count * st.er_density).round() as usize).clamp(3, ER_TAPS);
    let start = room.er_start_ms * size;
    let end = room.er_end_ms * size;
    let jitter = (0.35 + 0.6 * character.clamp(0.0, 1.0)).min(0.95);
    let mut t_ms = [[0.0; ER_TAPS]; 2];
    let mut gain = [[0.0; ER_TAPS]; 2];
    for side in 0..2 {
        let mut energy = 0.0;
        for k in 0..count {
            let seed = (k as u32) * 2 + side as u32;
            let u = ((k as f32 + 0.5 + jitter * (hash01(seed, 11) - 0.5)) / count as f32)
                .clamp(0.0, 1.0);
            let t = start + (end - start) * u.powf(0.8);
            let fall = room.er_fall_db * (t - start) / (end - start).max(1e-3);
            let spread = (hash01(seed, 23) - 0.5) * 4.0 * (0.5 + character.clamp(0.0, 1.0));
            let mut g = 10f32.powf(-(fall + spread) / 20.0);
            // The first two arrive in phase, as off the nearest walls; after that the
            // signs scatter, so the pattern does not comb-filter the source.
            if k >= 2 && hash01(seed, 37) < 0.45 {
                g = -g;
            }
            t_ms[side][k] = t;
            gain[side][k] = g;
            energy += g * g;
        }
        let norm = 1.0 / energy.max(1e-9).sqrt();
        for g in gain[side][..count].iter_mut() {
            *g *= norm;
        }
    }
    ErPattern {
        count,
        t_ms,
        gain,
        cross: 0.3,
        window_ms: end,
    }
}

/// The level of the early reflections, dB: loud and close at distance 0, soft at 1, plus
/// the user's offset. `None` when they are off.
pub fn er_level_db(style: Style, distance: f32, offset_db: f32) -> Option<f32> {
    if offset_db <= ER_OFF_DB {
        return None;
    }
    let st = style_shape(style);
    Some(1.0 - 12.0 * distance.clamp(0.0, 1.0) + offset_db + st.er_db)
}

/// The level of the late field relative to its calibrated unit, dB: a little under the
/// reflections up close, a little over them far away.
pub fn late_level_db(distance: f32) -> f32 {
    -3.0 + 3.0 * distance.clamp(0.0, 1.0)
}

/// How far the late field's onset trails the reflections, ms (distance delays it).
pub fn late_gap_ms(room: &Room, distance: f32) -> f32 {
    room.gap_ms * (0.4 + 1.2 * distance.clamp(0.0, 1.0))
}

/// Input diffusion, 0…1.
pub fn diffusion(room: &Room, style: Style, distance: f32, character: f32) -> f32 {
    let st = style_shape(style);
    let base = lin(room.diffusion, st.diffusion, st.diffusion_blend);
    (base + 0.12 * (distance - 0.35) - 0.15 * (character - 0.3)).clamp(0.3, 0.8)
}

/// The high cut on the reflections, Hz: close is bright, far is dark.
pub fn er_tone_hz(distance: f32) -> f32 {
    geo(16_000.0, 4_500.0, distance.clamp(0.0, 1.0))
}

/// Modulation of the late lines: depth (ms) and rate (Hz) from Character and style.
pub fn modulation(style: Style, character: f32) -> (f32, f32) {
    let st = style_shape(style);
    let c = character.clamp(0.0, 1.0);
    (
        (0.25 * c + 2.2 * c * c) * st.mod_depth,
        (0.25 + 1.3 * c) * st.mod_rate,
    )
}

/// Late delay-line lengths, ms, before rounding to primes: spread geometrically between
/// the room's shortest and longest, then interleaved so neighbours differ in size.
pub fn line_lengths_ms<const N: usize>(room: &Room, style: Style) -> [f32; N] {
    let st = style_shape(style);
    let mut out = [0.0; N];
    for (k, o) in out.iter_mut().enumerate() {
        // Interleave: 0, N-1, 1, N-2, … so short and long lines alternate.
        let j = if k % 2 == 0 { k / 2 } else { N - 1 - k / 2 };
        let t = j as f32 / (N - 1) as f32;
        *o = geo(room.min_ms, room.max_ms, t) * st.size;
    }
    out
}

/// Pre-delay divisions offered by the tempo sync, in beats (a quarter note is one beat).
/// Index 0 is "free" (no sync). The editor computes the milliseconds from the project's
/// tempo; the engine only ever sees milliseconds.
pub const SYNC_BEATS: [f32; 12] = [
    0.0,
    1.0 / 16.0, // 1/64
    1.0 / 8.0,  // 1/32
    1.0 / 6.0,  // 1/16 triplet
    1.0 / 4.0,  // 1/16
    3.0 / 8.0,  // 1/16 dotted
    1.0 / 3.0,  // 1/8 triplet
    1.0 / 2.0,  // 1/8
    3.0 / 4.0,  // 1/8 dotted
    2.0 / 3.0,  // 1/4 triplet
    1.0,        // 1/4
    3.0 / 2.0,  // 1/4 dotted
];

/// Milliseconds of a sync division at a tempo.
pub fn sync_ms(division: usize, bpm: f64) -> f32 {
    let beats = SYNC_BEATS[division.min(SYNC_BEATS.len() - 1)];
    (beats as f64 * 60_000.0 / bpm.max(1.0)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rooms_grow_and_morph_without_steps() {
        for w in ROOMS.windows(2) {
            assert!(w[1].decay_s > w[0].decay_s);
            assert!(w[1].max_ms > w[0].max_ms);
        }
        // Continuity: a small move of Space is a small change of every number.
        let mut prev = room_at(0.0);
        for i in 1..=1000 {
            let r = room_at(i as f32 / 1000.0);
            assert!((r.decay_s / prev.decay_s - 1.0).abs() < 0.01);
            assert!((r.max_ms / prev.max_ms - 1.0).abs() < 0.01);
            prev = r;
        }
        assert_eq!(room_at(1.0).decay_s, ROOMS[MODELS - 1].decay_s);
    }

    #[test]
    fn decay_is_the_mid_band_rt60() {
        let bands = [DecayBand::default(); DECAY_BANDS];
        for style in Style::ALL {
            for space in [0.0, 0.3, 0.7, 1.0] {
                let c = TargetCurve::new(style, space, 2.5, 0.2, true, &bands);
                assert!((c.t60(1_000.0) - 2.5).abs() < 1e-3, "{style:?} {space}");
            }
        }
        // Brightness below neutral shortens the top; above lengthens it.
        let dark = TargetCurve::new(Style::Room, 0.5, 2.0, 0.0, true, &bands);
        let bright = TargetCurve::new(Style::Room, 0.5, 2.0, 1.0, true, &bands);
        assert!(dark.t60(10_000.0) < 0.4 * bright.t60(10_000.0));
    }

    #[test]
    fn a_decay_band_multiplies_the_curve_where_it_sits() {
        let mut bands = [DecayBand::default(); DECAY_BANDS];
        bands[0] = DecayBand {
            active: true,
            shape: DecayShape::Bell,
            freq: 250.0,
            mult: 2.0,
            q: 1.0,
        };
        let on = TargetCurve::new(Style::Room, 0.5, 2.0, 0.5, true, &bands);
        let off = TargetCurve::new(Style::Room, 0.5, 2.0, 0.5, false, &bands);
        let ratio = on.t60(250.0) / off.t60(250.0);
        assert!((ratio - 2.0).abs() < 0.01, "×{ratio}");
        assert!((on.t60(8_000.0) / off.t60(8_000.0) - 1.0).abs() < 0.02);
    }

    #[test]
    fn early_reflections_are_normalised_and_inside_their_window() {
        for i in 0..MODELS {
            let r = ROOMS[i];
            let p = er_pattern(&r, Style::Room, 1.0, 0.3);
            for side in 0..2 {
                let e: f32 = p.gain[side][..p.count].iter().map(|g| g * g).sum();
                assert!((e - 1.0).abs() < 1e-4);
                for t in &p.t_ms[side][..p.count] {
                    assert!(*t >= r.er_start_ms - 1e-3 && *t <= r.er_end_ms + 1e-3);
                }
            }
            // The two sides differ: a mono source gets a stereo pattern.
            assert_ne!(p.t_ms[0], p.t_ms[1]);
        }
    }

    #[test]
    fn sync_divisions_are_note_values() {
        assert_eq!(sync_ms(10, 120.0), 500.0);
        assert_eq!(sync_ms(7, 120.0), 250.0);
        assert!((sync_ms(8, 120.0) - 375.0).abs() < 1e-3);
    }
}
