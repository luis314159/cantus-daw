//! What Lamina's tines and reeds are, in millimetres: pure functions the voice and the
//! editor share, so what is drawn is what sounds.
//!
//! **The tine** is a clamped-free steel cantilever (1.5 mm diameter here). Its first
//! bending mode is `f₁ = (β₁²/2πL²)·√(EI/ρA)` with `β₁ = 1.8751`; for a round rod
//! `√(EI/ρA) = c·r/2` with `c = √(E/ρ) ≈ 5050 m/s` for steel. So the length that sounds a
//! pitch follows from the pitch: 64 mm at C4, 197 mm at A0, 16 mm at C8 — the range a
//! real tine-piano spans (its tuning springs trim the rest). The upper modes sit at
//! `(βₙ/β₁)²` = 6.267 and 17.547 times the fundamental, and how strongly a strike
//! excites each depends on where the hammer meets the tine, through the mode shapes
//! `φₙ(ξ)` (textbook clamped-free forms, `ξ` the fraction of the length from the clamp).
//!
//! **The reed** is a steel cantilever with a solder tip mass: its upper modes sit higher
//! relative to the fundamental (7.1 and 20.4 times; our estimate for a tip mass about a
//! third of the reed's own) and the shapes are taken as the bare cantilever's.
//!
//! **The pickups** are what makes the bark (Pfeifle & Münster 2017; Falaize & Hélie
//! 2017):
//!
//! - *Electromagnetic* (tine): the flux through the coil falls off with the tip's distance
//!   from the pole piece, `Φ(x) = (1 + ((x − a)/d)²)^(−3/2)` for a tip displaced `x` mm
//!   across a pole `a` mm off the tine's rest line and `d` mm away. The output is `dΦ/dt`.
//!   Centred (`a = 0`) the curve is even and the note jumps an octave; off centre it is
//!   mostly the fundamental with an asymmetric distortion that grows as the tip swings
//!   into the steep part of the curve — hard strikes and a close pickup bark.
//! - *Electrostatic* (reed): the capacitance between the reed and its slot, `C(x) =
//!   (1 + ((x − a)/d)²)^(−1/2)`, a wider curve with a longer reach; the output is `V·dC/dt`.

/// Diameter of the tine, mm.
pub const TINE_DIAMETER_MM: f32 = 1.5;
/// Speed of bending waves' material constant for steel, √(E/ρ), m/s.
pub const STEEL_C_MPS: f32 = 5_050.0;
/// βₙL for the first three clamped-free modes.
pub const BETA: [f32; 3] = [1.875_104, 4.694_091, 7.854_757];
/// σₙ of the clamped-free mode shapes.
pub const SIGMA: [f32; 3] = [0.734_096, 1.018_467, 0.999_224];
/// Frequency ratios of the tine's first three modes, (βₙ/β₁)².
pub const TINE_RATIOS: [f32; 3] = [1.0, 6.266_97, 17.547_5];
/// The reed's, with its tip mass (our estimate).
pub const REED_RATIOS: [f32; 3] = [1.0, 7.1, 20.4];
/// The tone bar's own second mode (a free-free bar's 2.756), which rings in the attack.
pub const BAR_RATIO: f32 = 2.756;

/// The engine.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Engine {
    /// A tine and its tone bar, an electromagnetic pickup.
    #[default]
    Tine,
    /// A reed with a tip mass, an electrostatic pickup.
    Reed,
}

impl Engine {
    pub fn from_index(i: f32) -> Engine {
        if i.round() as i32 == 1 {
            Engine::Reed
        } else {
            Engine::Tine
        }
    }
}

/// The length of tine (mm) whose first mode is `hz`.
pub fn tine_length_mm(hz: f32) -> f32 {
    let r = TINE_DIAMETER_MM * 0.5e-3;
    let k = STEEL_C_MPS * r / 2.0;
    let l = (BETA[0] * BETA[0] * k / (std::f32::consts::TAU * hz.max(1.0))).sqrt();
    l * 1_000.0
}

/// The reed's free length (mm) for a pitch: a flat 0.8 mm reed loaded by its solder, an
/// empirical fit to the instrument's 18–60 mm range (our estimate).
pub fn reed_length_mm(hz: f32) -> f32 {
    (62.0 * (110.0 / hz.max(1.0)).powf(0.42)).clamp(12.0, 75.0)
}

/// The clamped-free mode shape `n` (0-based) at `xi` (0 at the clamp, 1 at the tip).
pub fn mode_shape(n: usize, xi: f32) -> f32 {
    let b = BETA[n.min(2)] * xi.clamp(0.0, 1.0);
    let (b, s) = (b as f64, SIGMA[n.min(2)] as f64);
    (b.cosh() - b.cos() - s * (b.sinh() - b.sin())) as f32
}

/// How strongly a strike at `xi` excites mode `n`, relative to its displacement at the tip
/// (where the pickup reads it).
pub fn strike_weight(n: usize, xi: f32) -> f32 {
    (mode_shape(n, xi) / mode_shape(n, 1.0)).abs()
}

/// The electromagnetic pickup's flux for a tip at `x` mm, the pole `align` mm off the rest
/// line and `gap` mm away (1 at the closest point).
#[inline]
pub fn flux(x: f32, gap: f32, align: f32) -> f32 {
    let u = (x - align) / gap.max(0.05);
    let q = 1.0 + u * u;
    1.0 / (q * q.sqrt())
}

/// The electrostatic pickup's capacitance, relative (1 at the closest point).
#[inline]
pub fn capacitance(x: f32, gap: f32, align: f32) -> f32 {
    let u = (x - align) / gap.max(0.05);
    1.0 / (1.0 + u * u).sqrt()
}

/// The engine's transfer curve.
#[inline]
pub fn transfer(engine: Engine, x: f32, gap: f32, align: f32) -> f32 {
    match engine {
        Engine::Tine => flux(x, gap, align),
        Engine::Reed => capacitance(x, gap, align),
    }
}

/// The transfer curve in double precision, for the voice: the pickup's output is the
/// difference of two nearly equal values, and a quiet note's swing would drown in single
/// precision's rounding.
#[inline]
pub fn transfer64(engine: Engine, x: f64, gap: f64, align: f64) -> f64 {
    let u = (x - align) / gap.max(0.05);
    let q = 1.0 + u * u;
    match engine {
        Engine::Tine => 1.0 / (q * q.sqrt()),
        Engine::Reed => 1.0 / q.sqrt(),
    }
}

/// How far the tip swings at full velocity, mm, for a fundamental of `hz`: long bass
/// tines swing wide, short treble tines little (4.1 mm at C2, 2.0 mm at C4, 1.0 at C6; our
/// estimate, scaled for the reed by its stiffer, shorter body).
pub fn swing_ff_mm(engine: Engine, hz: f32) -> f32 {
    let a = 2.0 * (261.63 / hz.max(1.0)).powf(0.52);
    let a = a.clamp(0.35, 5.0);
    match engine {
        Engine::Tine => a,
        Engine::Reed => a * 0.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_c4_tine_is_about_64_mm_and_lengths_fall_with_pitch() {
        let c4 = tine_length_mm(261.63);
        assert!((c4 - 64.0).abs() < 2.0, "{c4}");
        assert!(tine_length_mm(27.5) > 180.0 && tine_length_mm(4186.0) < 20.0);
    }

    #[test]
    fn the_mode_shapes_are_zero_at_the_clamp_and_two_at_the_tip() {
        for n in 0..3 {
            assert!(mode_shape(n, 0.0).abs() < 1e-6);
            assert!(
                (mode_shape(n, 1.0).abs() - 2.0).abs() < 0.01,
                "{n}: {}",
                mode_shape(n, 1.0)
            );
        }
        // Near the clamp, the upper modes are excited relatively more.
        assert!(strike_weight(2, 0.08) > strike_weight(0, 0.08) * 5.0);
    }

    #[test]
    fn a_centred_pickup_is_even_and_an_offset_one_is_not() {
        assert_eq!(flux(0.5, 1.6, 0.0), flux(-0.5, 1.6, 0.0));
        assert!(flux(0.5, 1.6, 0.6) > flux(-0.5, 1.6, 0.6));
        assert_eq!(flux(0.0, 1.0, 0.0), 1.0);
        assert_eq!(capacitance(0.0, 1.0, 0.0), 1.0);
    }
}
