//! Chorda's keyboard: what each of the 88 keys is, from published measurements and the
//! closed-form physics of a stiff string (ADR 0059). Everything here is computed off the
//! audio thread, in `f64`, with `cantus_dsp::math` (and `libm::erf`): the same numbers on
//! every machine.
//!
//! **Inharmonicity** `B(m)` across the keyboard (MIDI note `m`, A0 = 21 … C8 = 108):
//! Rigaud, David & Daudet's two-asymptote model (DAFx 2011, from recordings of five pianos):
//! `B(m) = e^(s_B·m + y_B) + e^(s_T·m + y_T)`, the treble bridge's asymptote fixed by them
//! for every piano (`s_T = 9.26·10⁻²`, `y_T = −13.64`), the bass bridge's set so that C2 has
//! the `B = 1.0·10⁻⁴` Bank measured on a grand (MSc thesis, 2000, §2.2.3; the model then
//! gives A♯4 `7.8·10⁻⁴` against his measured `7.5·10⁻⁴`).
//!
//! **Tuning**: Rigaud et al.'s aural-tuning model. A4's first partial is the reference;
//! octaves are tuned from it by cancelling the beat between partial `2ρ` of the lower note
//! and partial `ρ` of the upper, `ρ(m) = (K − 1)/2 · (1 − erf((m − m₀)/α)) + 1`
//! (`m₀ = 64`, `α = 24`, `K = 4.51`: from 8:4–10:5 octaves in the low bass to 2:1 in the
//! treble), and the semitones of each octave are spread evenly between its ends. *Stretch*
//! scales `ρ − 1` (0: every octave 2:1 between first and second partials).
//!
//! **Strings**: Bensa, Bilbao, Kronland-Martinet & Smith (JASA 114, 2003, Table I) give a
//! real piano's C2, C4 and C7: length, the damping `b₁` (s⁻¹) and `b₂` (m²/s) of the
//! partial decay rate `σ_k = b₁ + b₂ (kπ/L)²`. Between them (and beyond, along the end
//! slopes) every quantity is interpolated on a logarithmic scale against the key.
//!
//! **Hammers**: Chaigne & Askenfelt's measured hammers (JASA 95, 1994) for C2, C4 and C7
//! — mass, stiffness `K` and exponent `p` of `F = K·u^p`, the strike at 12 % of the length
//! (1/16 at C7) — interpolated the same way. Stulov's hysteresis (JASA 97, 1995) is added
//! with an instantaneous stiffness `K/(1 − ε)`, so the slow-loading stiffness stays theirs.

use crate::math;

/// The lowest and highest keys (A0, C8).
pub const LOWEST: u8 = 21;
pub const HIGHEST: u8 = 108;
pub const KEYS: usize = (HIGHEST - LOWEST + 1) as usize;

/// Rigaud et al.'s treble asymptote.
const S_T: f64 = 9.26e-2;
const Y_T: f64 = -13.64;
/// The bass asymptote: the treble's slope mirrored, its intercept through Bank's C2.
const S_B: f64 = -9.0e-2;
const Y_B: f64 = -6.38;

/// The octave-type model's parameters (Rigaud et al., averaged over three well-tuned pianos).
const RHO_M0: f64 = 64.0;
const RHO_ALPHA: f64 = 24.0;
const RHO_K: f64 = 4.51;

/// Bensa et al.'s measured strings (C2, C4, C7): key, length (m), b₁ (s⁻¹), b₂ (m²/s).
const STRINGS: [(f64, f64, f64, f64); 3] = [
    (36.0, 1.23, 0.25, 7.5e-5),
    (60.0, 0.63, 1.1, 2.7e-4),
    (96.0, 0.10, 9.17, 2.1e-3),
];

/// Chaigne & Askenfelt's hammers (C2, C4, C7): key, mass (kg), K (N/m^p), p, strike.
const HAMMERS: [(f64, f64, f64, f64, f64); 3] = [
    (36.0, 4.9e-3, 4.0e8, 2.3, 0.12),
    (60.0, 2.97e-3, 4.5e9, 2.5, 0.12),
    (96.0, 2.2e-3, 1.0e12, 3.0, 0.0625),
];

/// The string tension (N): a grand's strings are all near it (Bank 2000, §2.2).
pub const TENSION: f64 = 700.0;

/// The highest key with a damper (E6), as on a grand.
pub const LAST_DAMPER: u8 = 88;

/// Interpolate `ys` (taken at `xs`, ascending) at `x`, linearly, continuing the end slopes.
fn interp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    let i = if x <= xs[1] {
        0
    } else if x >= xs[n - 2] {
        n - 2
    } else {
        (1..n - 1).find(|&i| x < xs[i + 1]).unwrap_or(n - 2)
    };
    let t = (x - xs[i]) / (xs[i + 1] - xs[i]);
    ys[i] + t * (ys[i + 1] - ys[i])
}

/// Interpolate on a logarithmic scale.
fn interp_log(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let logs: Vec<f64> = ys.iter().map(|y| math::ln(*y)).collect();
    math::exp(interp(xs, &logs, x))
}

/// The inharmonicity coefficient of key `m` at `stiffness` (1: the model's).
pub fn inharmonicity(m: f64, stiffness: f64) -> f64 {
    stiffness * (math::exp(S_B * m + Y_B) + math::exp(S_T * m + Y_T))
}

/// The octave type `ρ` an octave ending on key `m` is tuned to (1: 2:1, 2: 4:2 …), its
/// stretch above 2:1 scaled by `stretch`.
pub fn octave_type(m: f64, stretch: f64) -> f64 {
    let rho = (RHO_K - 1.0) / 2.0 * (1.0 - libm::erf((m - RHO_M0) / RHO_ALPHA)) + 1.0;
    1.0 + stretch * (rho - 1.0)
}

/// The tuning of the whole keyboard: each key's first partial (Hz), A4 at `a4`.
pub fn tuning(a4: f64, stretch: f64, stiffness: f64) -> [f64; KEYS] {
    let b = |m: f64| inharmonicity(m, stiffness);
    // F₀ of the ideal string, from the first partial: f₁ = F₀ √(1 + B).
    let f0_of = |m: f64, f1: f64| f1 / (1.0 + b(m)).sqrt();
    // The A keys, octave by octave from A4 (69) — up to A8 (117) and down to A0 (21).
    let mut anchors: Vec<(f64, f64)> = vec![(69.0, f0_of(69.0, a4))];
    let mut f0 = anchors[0].1;
    let mut m = 69.0;
    while m < 117.0 {
        let up = m + 12.0;
        let rho = octave_type(up, stretch);
        f0 = 2.0 * f0 * ((1.0 + b(m) * 4.0 * rho * rho) / (1.0 + b(up) * rho * rho)).sqrt();
        anchors.push((up, f0));
        m = up;
    }
    let (mut m, mut f0) = (69.0, anchors[0].1);
    while m > 21.0 {
        let down = m - 12.0;
        let rho = octave_type(m, stretch);
        f0 = f0 / 2.0 * ((1.0 + b(m) * rho * rho) / (1.0 + b(down) * 4.0 * rho * rho)).sqrt();
        anchors.push((down, f0));
        m = down;
    }
    anchors.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = [0.0; KEYS];
    for (i, f) in out.iter_mut().enumerate() {
        let m = LOWEST as f64 + i as f64;
        let k = anchors
            .iter()
            .rposition(|a| a.0 <= m)
            .unwrap_or(0)
            .min(anchors.len() - 2);
        let (m0, f_lo) = anchors[k];
        let f_hi = anchors[k + 1].1;
        let f0 = f_lo * math::powf(f_hi / f_lo, (m - m0) / 12.0);
        *f = f0 * (1.0 + b(m)).sqrt();
    }
    out
}

/// What one key is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub key: u8,
    /// The first partial's frequency (Hz) and the inharmonicity coefficient.
    pub f1: f64,
    pub b: f64,
    /// Strings struck by one hammer: 1, 2 or 3.
    pub strings: u8,
    /// Speaking length (m), transverse wave speed (m/s), wave impedance (kg/s).
    pub length: f64,
    pub speed: f64,
    pub impedance: f64,
    /// The partials' decay rate `σ_k = b1 + b2 (kπ/L)²` (s⁻¹).
    pub b1: f64,
    pub b2: f64,
    /// The hammer: mass (kg), stiffness and exponent of `K·u^p`, strike (fraction of the
    /// length from the agraffe).
    pub hammer_mass: f64,
    pub hammer_k: f64,
    pub hammer_p: f64,
    pub strike: f64,
    pub damper: bool,
}

impl Key {
    /// The ideal string's fundamental `F₀ = f₁ / √(1 + B)`.
    pub fn f0(&self) -> f64 {
        self.f1 / (1.0 + self.b).sqrt()
    }

    /// Partial `n`'s frequency: `n F₀ √(1 + B n²)`.
    pub fn partial(&self, n: u32) -> f64 {
        let n = n as f64;
        n * self.f0() * (1.0 + self.b * n * n).sqrt()
    }

    /// Partial `n`'s decay rate (s⁻¹) and T60 (s).
    pub fn decay_rate(&self, n: u32) -> f64 {
        let beta = n as f64 * std::f64::consts::PI / self.length;
        self.b1 + self.b2 * beta * beta
    }

    pub fn t60(&self, n: u32) -> f64 {
        3.0 * std::f64::consts::LN_10 / self.decay_rate(n)
    }
}

/// How many strings key `m` has: one in the lowest notes, two above, three from D2.
pub fn strings(m: u8) -> u8 {
    match m {
        0..=30 => 1,
        31..=41 => 2,
        _ => 3,
    }
}

/// The 88 keys for a tuning.
pub fn keyboard(a4: f64, stretch: f64, stiffness: f64) -> [Key; KEYS] {
    let f1 = tuning(a4, stretch, stiffness);
    let xs: Vec<f64> = STRINGS.iter().map(|s| s.0).collect();
    let col = |i: usize| -> Vec<f64> {
        STRINGS
            .iter()
            .map(|s| [s.1, s.2, s.3][i])
            .collect::<Vec<f64>>()
    };
    let (lens, b1s, b2s) = (col(0), col(1), col(2));
    let hx: Vec<f64> = HAMMERS.iter().map(|h| h.0).collect();
    let hcol = |i: usize| -> Vec<f64> {
        HAMMERS
            .iter()
            .map(|h| [h.1, h.2, h.3, h.4][i])
            .collect::<Vec<f64>>()
    };
    let (masses, ks, ps, strikes) = (hcol(0), hcol(1), hcol(2), hcol(3));
    std::array::from_fn(|i| {
        let key = LOWEST + i as u8;
        let m = key as f64;
        let b = inharmonicity(m, stiffness);
        let length = interp_log(&xs, &lens, m);
        let f0 = f1[i] / (1.0 + b).sqrt();
        let speed = 2.0 * length * f0;
        Key {
            key,
            f1: f1[i],
            b,
            strings: strings(key),
            length,
            speed,
            impedance: TENSION / speed,
            b1: interp_log(&xs, &b1s, m),
            b2: interp_log(&xs, &b2s, m),
            hammer_mass: interp_log(&hx, &masses, m),
            hammer_k: interp_log(&hx, &ks, m),
            hammer_p: interp(&hx, &ps, m).clamp(2.0, 3.5),
            strike: interp(&hx, &strikes, m).clamp(1.0 / 16.0, 0.125),
            damper: key <= LAST_DAMPER,
        }
    })
}
