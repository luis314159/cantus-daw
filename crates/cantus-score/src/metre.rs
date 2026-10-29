//! A bar's metric structure, after Gould (*Behind Bars*, "Meter"): its beats and their
//! groups, how strong each point of the bar is, and the units a beam may span.
//!
//! Strength: 0 the bar line, 1 the middle of a four-beat bar (the half bar Gould asks to
//! show), 2 a beat, then each halving of the beat one weaker; a point no binary division
//! reaches (inside a tuplet) is [`WEAK`].

use crate::model::WHOLE;
use cantus_core::time::TimeSignature;

/// The strength of a point that no straight division of the bar reaches.
pub const WEAK: u8 = 255;

/// A bar's metre, read for notation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metre {
    pub sig: TimeSignature,
    /// The bar's length as notated (an off-line metre change may cut a bar short).
    pub len: i64,
    /// Where each beat starts and how long it is (a dotted beat in compound metres; the
    /// 3 + 2 of 5/8).
    pub beats: Vec<(i64, i64)>,
    /// Compound (6/8, 9/8, 12/8, 6/4): beats divide in three.
    pub compound: bool,
}

impl Metre {
    pub fn new(sig: TimeSignature, len: i64) -> Metre {
        let n = sig.numerator.max(1) as i64;
        let d = sig.denominator.max(1) as i64;
        let unit = WHOLE / d;
        let compound = n % 3 == 0 && n > 3;
        let groups: Vec<i64> = if compound {
            vec![3; (n / 3) as usize]
        } else if d >= 8 && n == 3 {
            // 3/8 is felt (and beamed) as one beat.
            vec![3]
        } else if d >= 8 && n > 3 && n % 2 == 1 {
            // 5/8 = 3 + 2, 7/8 = 2 + 2 + 3, 11/8 = 2 + 2 + 2 + 2 + 3.
            if n == 5 {
                vec![3, 2]
            } else {
                let mut g = vec![2; ((n - 3) / 2) as usize];
                g.push(3);
                g
            }
        } else if d >= 8 && n % 2 == 0 && n > 2 {
            // 4/8, 8/8: beats of two eighths (a quarter).
            vec![2; (n / 2) as usize]
        } else {
            vec![1; n as usize]
        };
        let mut beats = Vec::with_capacity(groups.len());
        let mut at = 0;
        for g in groups {
            let l = g * unit;
            if at >= len {
                break;
            }
            beats.push((at, l.min(len - at)));
            at += l;
        }
        if beats.is_empty() {
            beats.push((0, len.max(1)));
        }
        Metre {
            sig,
            len,
            beats,
            compound,
        }
    }

    /// The beat holding bar-relative tick `t`.
    pub fn beat_at(&self, t: i64) -> (i64, i64) {
        let i = self.beats.partition_point(|b| b.0 <= t).saturating_sub(1);
        self.beats[i]
    }

    /// Whether a beat divides in three (a dotted beat).
    pub fn beat_is_ternary(&self, beat_len: i64) -> bool {
        let unit = WHOLE / self.sig.denominator.max(1) as i64;
        beat_len == 3 * unit
    }

    /// How strong bar-relative tick `t` is (see the module's docs).
    pub fn strength(&self, t: i64) -> u8 {
        if t <= 0 || t >= self.len {
            return 0;
        }
        if self.beats.len() == 4 && t == self.beats[2].0 {
            return 1;
        }
        let (b0, bl) = self.beat_at(t);
        let off = t - b0;
        if off == 0 {
            return 2;
        }
        // Inside the beat: a dotted beat's thirds first, then halvings.
        let mut level = 3u8;
        let mut step = if self.beat_is_ternary(bl) {
            bl / 3
        } else {
            bl / 2
        };
        while step > 0 && level < 20 {
            if off % step == 0 {
                return level;
            }
            if step % 2 != 0 {
                break;
            }
            step /= 2;
            level += 1;
        }
        WEAK
    }

    /// The units a beam may span: each beat, or each half bar of 4/4 when it holds only
    /// eighths (`plain_eighths` says which half bars do).
    pub fn beam_units(&self, plain_eighths: impl Fn(i64, i64) -> bool) -> Vec<(i64, i64)> {
        let mut out = Vec::with_capacity(self.beats.len());
        let quarter = WHOLE / 4;
        let four_four = self.beats.len() == 4
            && self.beats.iter().all(|b| b.1 == quarter)
            && self.sig.denominator == 4;
        let mut i = 0;
        while i < self.beats.len() {
            let (s, l) = self.beats[i];
            if four_four && (i == 0 || i == 2) {
                let (s2, l2) = self.beats[i + 1];
                if plain_eighths(s, s2 + l2) {
                    out.push((s, l + l2));
                    i += 2;
                    continue;
                }
            }
            // A half-note beat (2/2, 3/2) beams its eighths in fours; a quarter beat its
            // own; a dotted beat its three eighths.
            out.push((s, l));
            i += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(n: u8, d: u8) -> Metre {
        let sig = TimeSignature::new(n, d);
        Metre::new(sig, sig.ticks_per_bar().0)
    }

    #[test]
    fn beats_by_metre() {
        assert_eq!(m(4, 4).beats.len(), 4);
        assert_eq!(m(6, 8).beats, vec![(0, 1440), (1440, 1440)]);
        assert_eq!(m(7, 8).beats, vec![(0, 960), (960, 960), (1920, 1440)]);
        assert_eq!(m(5, 8).beats, vec![(0, 1440), (1440, 960)]);
        assert_eq!(m(3, 8).beats, vec![(0, 1440)]);
        assert_eq!(m(2, 2).beats, vec![(0, 1920), (1920, 1920)]);
    }

    #[test]
    fn strengths() {
        let q = 960;
        let four = m(4, 4);
        assert_eq!(four.strength(0), 0);
        assert_eq!(four.strength(2 * q), 1);
        assert_eq!(four.strength(q), 2);
        assert_eq!(four.strength(q / 2), 3);
        assert_eq!(four.strength(q / 4), 4);
        assert_eq!(four.strength(320), WEAK);
        let six = m(6, 8);
        assert_eq!(six.strength(1440), 2);
        assert_eq!(six.strength(480), 3);
        assert_eq!(six.strength(240), 4);
        let three = m(3, 4);
        assert_eq!(three.strength(2 * q), 2);
    }
}
