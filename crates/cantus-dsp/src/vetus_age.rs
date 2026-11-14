//! Vetus's age (ADR 0133, provisional): **dropouts** (a worn tape's oxide missing for a
//! moment: the level falls and the top dulls) and **wear** (the level and the brightness
//! wandering slowly, as an old tape's do).
//!
//! Both are **deterministic processes of the song sample**, never a generator that runs:
//! the song is cut in cells of 25 ms; a hash of the seed and a cell's number says whether a
//! dropout starts in it, where, for how long (10 … 120 ms) and how deep (−2 dB … −30 dB with
//! the amount), shaped by raised-cosine edges. The wear is a smooth random — values on a
//! grid of 0.7 s joined by raised cosines. So a moment of the song ages the same way offline
//! and live, at every block size and from any bar.
//!
//! What it returns at each sample is a gain and a brightness (0 … 1: 1 untouched), both
//! pure functions of the song sample; the unit applies them (a gain and a 2-pole low-pass
//! from 20 kHz down to 1.5 kHz).

use crate::math;
use crate::modfx_clock::hash64;
use crate::vetus::noise::{hash3, unit};

const SEED: u64 = 0xA6E0_0D20;
const S_EVENT: u64 = 11;
const S_WHERE: u64 = 12;
const S_LEN: u64 = 13;
const S_DEPTH: u64 = 14;
const S_WEAR_L: u64 = 15;
const S_WEAR_B: u64 = 16;

/// The longest dropout, seconds; a cell's length.
const MAX_LEN_S: f32 = 0.12;
const CELL_S: f32 = 0.025;
/// The wear's grid, seconds.
const WEAR_S: f32 = 0.7;

/// The age's settings, after Character.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct AgeParams {
    pub drop: f32,
    pub wear: f32,
}

/// The age's clock-free maths.
#[derive(Copy, Clone, Debug)]
pub struct Age {
    cell: i64,
    /// Cells back a dropout may have started and still last.
    back: i64,
    wear_len: i64,
}

/// A raised cosine from 0 to 1 over `x` in 0 … 1.
#[inline]
fn rise(x: f32) -> f32 {
    0.5 - 0.5 * math::cos(std::f32::consts::PI * x.clamp(0.0, 1.0))
}

impl Age {
    pub fn new(sample_rate: f32) -> Age {
        let cell = ((CELL_S * sample_rate) as i64).max(1);
        Age {
            cell,
            back: ((MAX_LEN_S * sample_rate) as i64) / cell + 1,
            wear_len: ((WEAR_S * sample_rate) as i64).max(1),
        }
    }

    /// The dropouts' dip at song sample `n` (0: none, 1: the deepest dropout's whole depth),
    /// and its gain.
    pub fn dropout(&self, p: &AgeParams, n: i64) -> (f32, f32) {
        if p.drop <= 0.0 {
            return (0.0, 1.0);
        }
        // At 100 %, about four a second.
        let chance = 0.1 * p.drop * p.drop.sqrt();
        let deepest = 2.0 + 28.0 * p.drop;
        let c0 = n.div_euclid(self.cell);
        let mut gain = 1.0f32;
        let mut dip = 0.0f32;
        for c in (c0 - self.back)..=c0 {
            if unit(hash3(SEED, S_EVENT, c)) >= chance {
                continue;
            }
            let start = c * self.cell + (unit(hash3(SEED, S_WHERE, c)) * self.cell as f32) as i64;
            let u = unit(hash3(SEED, S_LEN, c));
            let len = ((0.15 + 0.85 * u * u) * MAX_LEN_S / CELL_S * self.cell as f32) as i64;
            let t = n - start;
            if t < 0 || t >= len.max(1) {
                continue;
            }
            let x = t as f32 / len.max(1) as f32;
            // Edges of a third of the dropout each side.
            let w = rise(x * 3.0).min(rise((1.0 - x) * 3.0));
            let v = unit(hash3(SEED, S_DEPTH, c));
            let depth = 2.0 + (deepest - 2.0) * v;
            let g = crate::math::db_to_gain(-depth * w);
            gain *= g;
            dip = dip.max(w * depth / 30.0);
        }
        (dip, gain)
    }

    /// A smooth random in −1 … 1 of stream `s` at song sample `n`.
    fn wander(&self, s: u64, n: i64) -> f32 {
        let i = n.div_euclid(self.wear_len);
        let x = (n - i * self.wear_len) as f32 / self.wear_len as f32;
        let v = |k: i64| unit(hash64(hash3(SEED, s, k))) * 2.0 - 1.0;
        let (a, b) = (v(i), v(i + 1));
        a + (b - a) * rise(x)
    }

    /// The gain and the brightness (0 … 1) at song sample `n`.
    pub fn at(&self, p: &AgeParams, n: i64) -> (f32, f32) {
        let (dip, gain) = self.dropout(p, n);
        let mut g = gain;
        let mut bright = 1.0 - 0.8 * dip.min(1.0);
        if p.wear > 0.0 {
            g *= crate::math::db_to_gain(2.5 * p.wear * self.wander(S_WEAR_L, n));
            let w = 0.5 + 0.5 * self.wander(S_WEAR_B, n);
            bright *= 1.0 - p.wear * (0.35 + 0.35 * w);
        }
        (g, bright)
    }
}

/// The low-pass's cutoff for a brightness: 20 kHz at 1, 1.5 kHz at 0 (log between).
pub fn cutoff(bright: f32) -> f32 {
    1_500.0 * math::exp2(bright.clamp(0.0, 1.0) * 3.737)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_at_zero() {
        let a = Age::new(48_000.0);
        for n in (0..480_000).step_by(97) {
            assert_eq!(a.at(&AgeParams::default(), n), (1.0, 1.0));
        }
    }

    /// The dropouts are a function of the song sample, their count grows with the amount
    /// and the deepest reach their depth.
    #[test]
    fn dropouts_come_as_often_and_as_deep_as_asked() {
        let a = Age::new(48_000.0);
        let count = |drop: f32| {
            let p = AgeParams { drop, wear: 0.0 };
            let mut inside = false;
            let mut n_drops = 0;
            let mut deepest = 1.0f32;
            for n in 0..48_000 * 60 {
                let (_, g) = a.dropout(&p, n);
                deepest = deepest.min(g);
                if g < 0.97 && !inside {
                    n_drops += 1;
                }
                inside = g < 0.97;
            }
            (n_drops, 20.0 * math::log10(deepest))
        };
        let (few, shallow) = count(0.2);
        let (many, deep) = count(1.0);
        assert!(few < many / 3, "{few} against {many}");
        assert!((150..300).contains(&many), "about four a second: {many}");
        assert!(deep < -24.0 && shallow > -16.0, "{deep} {shallow}");
        assert_eq!(
            a.at(
                &AgeParams {
                    drop: 0.7,
                    wear: 0.4
                },
                123_456
            ),
            a.at(
                &AgeParams {
                    drop: 0.7,
                    wear: 0.4
                },
                123_456
            )
        );
    }

    #[test]
    fn the_brightness_maps_to_the_cutoff() {
        assert!((cutoff(1.0) - 20_000.0).abs() < 100.0);
        assert!((cutoff(0.0) - 1_500.0).abs() < 1.0);
    }
}
