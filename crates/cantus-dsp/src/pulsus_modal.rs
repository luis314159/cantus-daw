//! Modal synthesis for Pulsus II's toms, percussion, modal snare and modal cymbal.
//!
//! A struck object is a sum of decaying sinusoids, its **modes**: frequencies from the
//! object's shape (Fletcher & Rossing), amplitudes from where it is struck (the mode shape
//! at the strike point) and with what (a contact of a few milliseconds low-passes the
//! strike), decays that fall faster for the high modes (τₖ = τ₀·(f₀/fₖ)^γ), which is what
//! makes it sound like skin and not glass. Each mode is a two-pole resonator, three
//! multiply-adds a sample.
//!
//! The lowest mode of every table is ratio 1, so a pad's tune is its fundamental: the
//! tuning test holds for every material by construction.

use crate::math;

pub(crate) const MAX_MODES: usize = 32;

/// The first 32 modes of an ideal circular membrane, as `(m, j_mn)`: the Bessel zero
/// sets the frequency (relative to j₀₁ = 2.40483) and `m` the mode shape's order.
const MEMBRANE: [(u8, f32); MAX_MODES] = [
    (0, 2.40483),
    (1, 3.83171),
    (2, 5.13562),
    (0, 5.52008),
    (3, 6.38016),
    (1, 7.01559),
    (4, 7.58834),
    (2, 8.41724),
    (0, 8.65373),
    (5, 8.77148),
    (3, 9.76102),
    (6, 9.93611),
    (1, 10.17347),
    (4, 11.06471),
    (7, 11.08637),
    (2, 11.61984),
    (0, 11.79153),
    (8, 12.22509),
    (5, 12.3386),
    (3, 13.0152),
    (1, 13.32369),
    (9, 13.3543),
    (6, 13.58929),
    (4, 14.37254),
    (10, 14.4755),
    (2, 14.79595),
    (7, 14.82127),
    (0, 14.93092),
    (11, 15.58985),
    (5, 15.70017),
    (8, 16.03777),
    (3, 16.22347),
];

/// A free–free bar (claves, a xylophone's bar untuned): 1 : 2.756 : 5.404 : 8.933 …
const BAR: [f32; 8] = [
    1.0, 2.7565, 5.4039, 8.9330, 13.3443, 18.6379, 24.8135, 31.8719,
];
/// A bar cut for a tuned percussion instrument (a woodblock, a marimba's bar): 1 : ~4 : ~10.
const WOOD: [f32; 6] = [1.0, 3.93, 9.24, 16.0, 24.3, 34.0];
/// A free circular plate.
const PLATE: [f32; 16] = [
    1.0, 1.731, 2.328, 3.910, 4.117, 5.538, 6.201, 7.000, 7.806, 8.925, 9.636, 10.52, 11.62, 12.44,
    13.51, 14.91,
];
/// A bell's partials, from the hum up: hum, prime, tierce, quint, nominal and above.
const BELL: [f32; 12] = [
    1.0, 2.0, 2.39, 3.0, 4.0, 5.33, 6.0, 8.0, 10.7, 12.0, 13.4, 16.0,
];

/// Bessel function of the first kind, J_m(x), by its power series (x ≤ ~20).
fn bessel_j(m: u32, x: f64) -> f64 {
    let half = 0.5 * x;
    let mut term = 1.0;
    for i in 1..=m {
        term *= half / i as f64;
    }
    let mut sum = term;
    for k in 1..80u32 {
        term *= -half * half / (k as f64 * (k + m) as f64);
        sum += term;
        if term.abs() < 1e-15 && k > m + 4 {
            break;
        }
    }
    sum
}

/// What a strike needs besides the table.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Strike {
    /// The fundamental, Hz.
    pub f0: f32,
    /// The fundamental's decay to −60 dB, seconds.
    pub decay: f32,
    /// 0.1…1: how much faster the high modes die.
    pub damping: f32,
    /// 0 centre … 1 edge.
    pub position: f32,
    /// 0 soft (5 ms contact) … 1 hard (0.3 ms).
    pub hardness: f32,
}

/// Which object: 0 membrane, 1 bar, 2 wood, 3 plate, 4 bell, and two the other voices use:
/// 5 a snare head, 6 a cymbal.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Object {
    Membrane { shell: f32 },
    Bar,
    Wood,
    Plate,
    Bell,
    Cymbal,
}

impl Object {
    pub(crate) fn from_material(material: u8, shell: f32) -> Object {
        match material {
            1 => Object::Bar,
            2 => Object::Wood,
            3 => Object::Plate,
            4 => Object::Bell,
            _ => Object::Membrane { shell },
        }
    }

    /// Mode `k`'s frequency ratio and its weight at the strike position, or `None` past
    /// the table's end.
    fn mode(self, k: usize, position: f32) -> Option<(f32, f32)> {
        let rho = 0.05 + 0.85 * position.clamp(0.0, 1.0);
        let spread = |r: &[f32]| -> Option<(f32, f32)> {
            let ratio = *r.get(k)?;
            // A beam's or plate's mode shape at the strike point, as a cosine pattern:
            // striking the middle favours the odd modes, the end excites all of them.
            let w = 0.25 + 0.75 * math::cos((k as f32 + 0.5) * std::f32::consts::PI * rho).abs();
            Some((ratio, w / (1.0 + k as f32).sqrt()))
        };
        match self {
            Object::Membrane { shell } => {
                let (m, j) = *MEMBRANE.get(k)?;
                let ideal = j / MEMBRANE[0].1;
                // Air loading and a second head pull the modes towards harmonic.
                let ratio = math::powf(ideal, 1.0 - 0.25 * shell.clamp(0.0, 1.0));
                let w = bessel_j(m as u32, (j * rho) as f64).abs() as f32;
                Some((ratio, w))
            }
            Object::Bar => spread(&BAR),
            Object::Wood => spread(&WOOD),
            Object::Plate => spread(&PLATE),
            Object::Bell => spread(&BELL),
            Object::Cymbal => {
                if k >= MAX_MODES {
                    return None;
                }
                // Dense and inharmonic: a stretched series with a fixed, irregular wobble.
                let kf = k as f32;
                let ratio = math::powf(1.0 + kf, 1.38) * (1.0 + 0.06 * math::sin(1.7 * kf * kf));
                Some((ratio.max(1.0), 1.0 / (1.0 + kf).sqrt()))
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct Mode {
    y1: f32,
    y2: f32,
    r: f32,
    r2: f32,
    hz: f32,
    a1: f32,
}

/// Up to [`MAX_MODES`] two-pole resonators, struck together.
#[derive(Clone, Debug)]
pub(crate) struct Bank {
    modes: [Mode; MAX_MODES],
    n: usize,
    sample_rate: f32,
}

impl Bank {
    pub(crate) fn new(sample_rate: f32) -> Bank {
        Bank {
            modes: [Mode::default(); MAX_MODES],
            n: 0,
            sample_rate,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.n = 0;
    }

    /// Strike: set every mode's frequency, decay and amplitude and start it ringing at
    /// zero phase (the first sample is zero: no step, no click). The amplitudes are
    /// normalised to sum to one, the loudest a strike can be.
    pub(crate) fn strike(&mut self, object: Object, count: usize, s: Strike) {
        let sr = self.sample_rate;
        let gamma = 0.3 + 1.2 * s.damping.clamp(0.0, 1.0);
        let contact = 0.005 * math::powf(0.06f32, s.hardness.clamp(0.0, 1.0));
        let mut n = 0;
        let mut total = 0.0;
        for k in 0..count.min(MAX_MODES) {
            let Some((ratio, weight)) = object.mode(k, s.position) else {
                break;
            };
            let hz = s.f0 * ratio;
            if hz >= sr * 0.45 {
                break;
            }
            // The strike's spectrum: a half-sine force pulse of the contact time.
            let x = 2.0 * hz * contact;
            let d = 1.0 - x * x;
            let h = if d.abs() < 1e-3 {
                std::f32::consts::FRAC_PI_4
            } else {
                (math::cos(std::f32::consts::PI * hz * contact) / d).abs()
            };
            let amp = weight * h.min(1.0);
            let t60 = (s.decay * math::powf(s.f0 / hz, gamma)).max(0.002);
            let r = math::exp(-6.907_755 / (t60 * sr));
            let theta = std::f32::consts::TAU * hz / sr;
            let m = &mut self.modes[n];
            m.r = r;
            m.r2 = r * r;
            m.hz = hz;
            m.a1 = 2.0 * r * math::cos(theta);
            // y[−1] = 0, y[−2] = −A·sin θ / r: the first output is A·r·sin θ.
            m.y1 = 0.0;
            m.y2 = -amp * math::sin(theta) / r;
            total += amp;
            n += 1;
        }
        self.n = n;
        // Normalise so the modes' amplitudes sum to one: the loudest a strike can be.
        if total > 0.0 {
            let g = 1.0 / total;
            for m in &mut self.modes[..n] {
                m.y2 *= g;
            }
        }
    }

    /// Scale every frequency by `ratio` (tension glide, a pitch envelope), keeping the
    /// resonators' state, so the change is continuous.
    pub(crate) fn retune(&mut self, ratio: f32) {
        let k = std::f32::consts::TAU / self.sample_rate;
        for m in &mut self.modes[..self.n] {
            m.a1 = 2.0 * m.r * math::cos((k * m.hz * ratio).min(3.0));
        }
    }

    #[inline]
    pub(crate) fn tick(&mut self) -> f32 {
        let mut sum = 0.0;
        for m in &mut self.modes[..self.n] {
            let y = m.a1 * m.y1 - m.r2 * m.y2;
            m.y2 = m.y1;
            m.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
            sum += y;
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bessel_zeros_are_zeros() {
        for (m, j) in MEMBRANE {
            let v = bessel_j(m as u32, j as f64);
            assert!(v.abs() < 1e-4, "J_{m}({j}) = {v}");
        }
        assert!((bessel_j(0, 0.0) - 1.0).abs() < 1e-12);
        assert!((bessel_j(1, 1.0) - 0.440_050_585_7).abs() < 1e-9);
    }

    #[test]
    fn every_object_starts_on_ratio_one() {
        for object in [
            Object::Membrane { shell: 0.0 },
            Object::Membrane { shell: 1.0 },
            Object::Bar,
            Object::Wood,
            Object::Plate,
            Object::Bell,
            Object::Cymbal,
        ] {
            assert_eq!(object.mode(0, 0.3).unwrap().0, 1.0, "{object:?}");
        }
    }

    #[test]
    fn a_centre_strike_excites_only_the_round_modes() {
        let o = Object::Membrane { shell: 0.0 };
        // rho is 0.05 at position 0: the m > 0 modes are nearly still.
        let (_, w01) = o.mode(0, 0.0).unwrap();
        let (_, w11) = o.mode(1, 0.0).unwrap();
        assert!(w01 > 0.9 && w11 < 0.1, "{w01} {w11}");
    }
}
