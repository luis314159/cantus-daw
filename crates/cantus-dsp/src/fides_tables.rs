//! Fides's tables: the partials of each modal resonator and where it is struck and heard
//! (its mode shapes), and the bodies' modes. Every number is from the published physics
//! (cited beside it) or derived from it here, in `f64`, with `crate::math`; nothing is
//! copied from another instrument's code. Built off the audio thread or at a note's start
//! (a few dozen calls), never per sample.

use crate::math;

/// The most modes one modal resonator plays.
pub const MAX_MODES: usize = 48;

/// Which resonator, in `a.type`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ResKind {
    #[default]
    String,
    Bar,
    Marimba,
    Tine,
    Plate,
    Membrane,
    Drumhead,
    Tube,
    Pipe,
    Bell,
    Glass,
    Pan,
}

impl ResKind {
    pub const ALL: [ResKind; 12] = [
        ResKind::String,
        ResKind::Bar,
        ResKind::Marimba,
        ResKind::Tine,
        ResKind::Plate,
        ResKind::Membrane,
        ResKind::Drumhead,
        ResKind::Tube,
        ResKind::Pipe,
        ResKind::Bell,
        ResKind::Glass,
        ResKind::Pan,
    ];

    pub fn from_index(i: f32) -> ResKind {
        let k = (i.round().max(0.0) as usize).min(Self::ALL.len() - 1);
        Self::ALL[k]
    }

    /// A waveguide (a string, a tube, a pipe) rather than a bank of modes.
    pub fn is_guide(self) -> bool {
        matches!(self, ResKind::String | ResKind::Tube | ResKind::Pipe)
    }
}

/// One partial of a modal table: its frequency over the fundamental's, and its shape's
/// two indices (a beam's mode number, a membrane's `(m, n)`, a plate's `(m, n)`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Partial {
    pub ratio: f64,
    pub m: u8,
    pub n: u8,
}

const fn pt(ratio: f64, m: u8, n: u8) -> Partial {
    Partial { ratio, m, n }
}

/// βₙL of a free–free beam (Fletcher & Rossing, *The Physics of Musical Instruments*, 2nd
/// ed., §2.13): the roots of `cos βL · cosh βL = 1`; past the fifth, `(2n + 1)π/2`.
fn beta_free(n: usize) -> f64 {
    const B: [f64; 5] = [
        4.730_040_7,
        7.853_204_6,
        10.995_607_8,
        14.137_165_5,
        17.278_759_6,
    ];
    B.get(n)
        .copied()
        .unwrap_or((2.0 * n as f64 + 3.0) * std::f64::consts::FRAC_PI_2)
}

/// βₙL of a clamped–free beam (a cantilever: a tine): the roots of `cos βL · cosh βL = −1`
/// (ibid.); past the fifth, `(2n − 1)π/2` counted from one.
fn beta_clamped(n: usize) -> f64 {
    const B: [f64; 5] = [
        1.875_104_1,
        4.694_091_1,
        7.854_757_4,
        10.995_540_7,
        14.137_168_4,
    ];
    B.get(n)
        .copied()
        .unwrap_or((2.0 * n as f64 + 1.0) * std::f64::consts::FRAC_PI_2)
}

/// Zeros `j_{m,n}` of the Bessel functions, ascending: a circular membrane's modes
/// (Abramowitz & Stegun, table 9.5), as `(m, n, j)`.
const BESSEL: [(u8, u8, f64); 30] = [
    (0, 1, 2.4048),
    (1, 1, 3.8317),
    (2, 1, 5.1356),
    (0, 2, 5.5201),
    (3, 1, 6.3802),
    (1, 2, 7.0156),
    (4, 1, 7.5883),
    (2, 2, 8.4172),
    (0, 3, 8.6537),
    (5, 1, 8.7715),
    (3, 2, 9.7610),
    (6, 1, 9.9361),
    (1, 3, 10.1735),
    (4, 2, 11.0647),
    (7, 1, 11.0864),
    (2, 3, 11.6198),
    (0, 4, 11.7915),
    (8, 1, 12.2251),
    (5, 2, 12.3386),
    (3, 3, 13.0152),
    (1, 4, 13.3237),
    (9, 1, 13.3543),
    (6, 2, 13.5893),
    (4, 3, 14.3725),
    (10, 1, 14.4755),
    (2, 4, 14.7960),
    (7, 2, 14.8213),
    (0, 5, 14.9309),
    (11, 1, 15.5898),
    (5, 3, 15.7002),
];

/// A marimba bar's partials: the undercut tunes the second to two octaves and the third
/// to about three octaves and a major third (1 : 3.99 : 9.2; Fletcher & Rossing §19.2,
/// Bork's measurements); above them the bar's own modes, untuned.
const MARIMBA: [f64; 8] = [1.0, 3.99, 9.2, 15.3, 22.9, 31.8, 42.1, 53.7];
/// A church bell's partials over the prime (hum, prime, tierce, quint, nominal, …;
/// Rossing, *The Science of Percussion Instruments*, ch. 9, a tuned bell).
const BELL: [f64; 14] = [
    0.5, 1.0, 1.183, 1.506, 2.0, 2.514, 2.662, 3.011, 4.166, 5.433, 6.796, 8.215, 9.71, 11.3,
];
/// A kettledrum's preferred modes over its (1,1): the air load tunes `(m,1)` near
/// 1 : 1.5 : 2 : 2.5 : 3 (Rossing, *Science of Percussion*, ch. 3, timpani), with the
/// heavily damped (0,1) below and a few `(m,2)` among them.
const DRUMHEAD: [(f64, u8, u8); 14] = [
    (0.85, 0, 1),
    (1.0, 1, 1),
    (1.5, 2, 1),
    (1.71, 0, 2),
    (1.98, 3, 1),
    (2.14, 1, 2),
    (2.44, 4, 1),
    (2.64, 2, 2),
    (2.89, 5, 1),
    (3.12, 3, 2),
    (3.34, 6, 1),
    (3.59, 4, 2),
    (3.79, 7, 1),
    (4.05, 5, 2),
];
/// A steel-pan note: the maker tunes its fundamental, octave and twelfth (1 : 2 : 3;
/// Rossing & Hansen, "Steelpans", *The Science of Percussion*, ch. 11); the modes above,
/// near the next harmonics.
const PAN: [f64; 10] = [1.0, 2.0, 3.0, 4.03, 5.08, 6.02, 7.21, 8.12, 9.37, 10.6];

/// A rectangular plate's aspect, simply supported: an irrational ratio so no two modes
/// coincide.
const PLATE_ASPECT: f64 = 0.7236;

/// The partials of a modal resonator, ascending by ratio (a waveguide has none).
pub fn partials(kind: ResKind, out: &mut [Partial; MAX_MODES]) -> usize {
    let mut n = 0;
    let mut push = |p: Partial| {
        if n < MAX_MODES {
            out[n] = p;
            n += 1;
        }
    };
    match kind {
        ResKind::String | ResKind::Tube | ResKind::Pipe => {}
        ResKind::Bar => {
            let b1 = beta_free(0);
            for k in 0..24 {
                let b = beta_free(k);
                push(pt((b / b1) * (b / b1), k as u8, 0));
            }
        }
        ResKind::Marimba => {
            for (k, r) in MARIMBA.iter().enumerate() {
                push(pt(*r, k as u8, 0));
            }
        }
        ResKind::Tine => {
            let b1 = beta_clamped(0);
            for k in 0..8 {
                let b = beta_clamped(k);
                push(pt((b / b1) * (b / b1), k as u8, 0));
            }
        }
        ResKind::Membrane => {
            for (m, nn, j) in BESSEL {
                push(pt(j / BESSEL[0].2, m, nn));
            }
        }
        ResKind::Drumhead => {
            for (r, m, nn) in DRUMHEAD {
                push(pt(r, m, nn));
            }
        }
        ResKind::Plate => {
            // f_mn ∝ m² + (n/aspect)², over f_11; the lowest MAX_MODES of a 12×12 grid.
            let mut grid = [(0.0f64, 0u8, 0u8); 144];
            let mut g = 0;
            for m in 1..=12u8 {
                for nn in 1..=12u8 {
                    let (x, y) = (m as f64, nn as f64 / PLATE_ASPECT);
                    let f = x * x + y * y;
                    grid[g] = (f, m, nn);
                    g += 1;
                }
            }
            grid.sort_by(|a, b| a.0.total_cmp(&b.0));
            let f11 = grid[0].0;
            for (f, m, nn) in grid.iter().take(MAX_MODES) {
                push(pt(f / f11, *m, *nn));
            }
        }
        ResKind::Bell => {
            for (k, r) in BELL.iter().enumerate() {
                push(pt(*r, k as u8, 0));
            }
        }
        ResKind::Glass => {
            // The inextensional ring's n(n² − 1)/√(n² + 1) (Rayleigh), n = 2 …, over n = 2,
            // compressed by a bowl's taper to about 1 : 2.4 : 4.2 (Rossing, "Wine glasses,
            // bell modes, and Lord Rayleigh", *Phys. Teach.* 28, 1990).
            let ring = |n: f64| n * (n * n - 1.0) / (n * n + 1.0).sqrt();
            for k in 0..10 {
                let r = ring(k as f64 + 2.0) / ring(2.0);
                push(pt(math::powf(r, 0.85), k as u8, 0));
            }
        }
        ResKind::Pan => {
            for (k, r) in PAN.iter().enumerate() {
                push(pt(*r, k as u8, 0));
            }
        }
    }
    n
}

/// A free–free beam's mode `k` (from 0) at `x` ∈ [0, ½] of its length from an end:
/// `cosh βx + cos βx − σ(sinh βx + sin βx)`, and for a large β its exact limit there,
/// `cos βx − sin βx + e^(−βx)` (the far end's term is below e^(−β/2)).
pub fn beam_free(k: usize, x: f64) -> f64 {
    let b = beta_free(k);
    let bx = b * x.clamp(0.0, 0.5);
    if b < 26.0 {
        let s = (math::cosh(b) - math::cos(b)) / (math::sinh(b) - math::sin(b));
        math::cosh(bx) + math::cos(bx) - s * (math::sinh(bx) + math::sin(bx))
    } else {
        math::cos(bx) - math::sin(bx) + math::exp(-bx)
    }
}

/// A cantilever's mode `k` at `x` ∈ [0, 1] from its free end (a tine is plucked at its
/// tip): `cosh βy − cos βy − σ(sinh βy − sin βy)` at `y = 1 − x` from the clamp, over its
/// value at the tip (±2).
pub fn beam_clamped(k: usize, x: f64) -> f64 {
    let b = beta_clamped(k);
    let y = (1.0 - x).clamp(0.0, 1.0);
    let s = (math::cosh(b) + math::cos(b)) / (math::sinh(b) + math::sin(b));
    let f = |y: f64| {
        let by = b * y;
        math::cosh(by) - math::cos(by) - s * (math::sinh(by) - math::sin(by))
    };
    f(y) / f(1.0).abs()
}

/// The Bessel function `J_m(x)` by its power series in `f64` (`x` ≤ 16 here: the terms'
/// cancellation costs under six digits of sixteen).
pub fn bessel_j(m: u32, x: f64) -> f64 {
    let h = 0.5 * x;
    let mut term = 1.0f64;
    for k in 1..=m {
        term *= h / k as f64;
    }
    let mut sum = term;
    let h2 = h * h;
    for k in 1..60u32 {
        term *= -h2 / (k as f64 * (k + m) as f64);
        sum += term;
        if term.abs() < 1e-18 * sum.abs().max(1e-300) {
            break;
        }
    }
    sum
}

/// Where a partial is at `pos` (the strike or the microphone), as the fraction of the
/// length from the end the controls name (a bar's end, a membrane's rim, a plate's
/// corner, a bell's lip). Unnormalised; signs kept (the phases a real one has).
pub fn shape(kind: ResKind, p: &Partial, pos: f64) -> f64 {
    let pos = pos.clamp(0.0, 0.5);
    match kind {
        ResKind::String | ResKind::Tube | ResKind::Pipe => 1.0,
        // The bars' and the pan's modes as a free–free beam's (a marimba's undercut keeps
        // their nodes near the uniform bar's, Fletcher & Rossing §19.2).
        ResKind::Bar | ResKind::Marimba => 0.5 * beam_free(p.m as usize, pos),
        ResKind::Tine => beam_clamped(p.m as usize, pos),
        ResKind::Membrane | ResKind::Drumhead => {
            // From the rim (pos 0) to the centre (pos ½): r = 1 − 2·pos, on the line θ = 0.
            let j = BESSEL
                .iter()
                .find(|(m, n, _)| *m == p.m && *n == p.n)
                .map_or(BESSEL[0].2, |b| b.2);
            let r = 1.0 - 2.0 * pos;
            bessel_j(p.m as u32, j * r) / 0.58
        }
        ResKind::Plate => {
            // Along a line across the plate, off its axes of symmetry.
            let (x, y) = (pos, 0.31 + 0.53 * pos);
            let pi = std::f64::consts::PI;
            math::sin(p.m as f64 * pi * x) * math::sin(p.n as f64 * pi * y)
        }
        // A bell's, a glass's and a pan's partials along the height from the lip: the n-th
        // ring mode's amplitude falls off away from the lip, the more the higher it is.
        ResKind::Bell | ResKind::Glass | ResKind::Pan => {
            let k = p.m as f64;
            let pi = std::f64::consts::PI;
            math::cos(pi * pos * (0.6 + 0.35 * k)) * (1.0 - 0.6 * pos)
        }
    }
}
