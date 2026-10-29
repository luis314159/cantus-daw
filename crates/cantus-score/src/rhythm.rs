//! Rhythm: a stretch of a bar written as values the metre shows, tied where it must be
//! (Gould, *Behind Bars*, "Meter" and "Rests").
//!
//! A note is written as one value when the value exists and it hides no point stronger
//! than where it starts — a half note on beat 1 or 3 of 4/4, a dotted quarter on a beat —
//! or when it is the centred syncopation every reader knows (quarter–half–quarter,
//! eighth–quarter–eighth inside a half bar, sixteenth–eighth–sixteenth inside a beat), or
//! a dotted value inside one beat that ends on it (the Scotch snap). Otherwise it is cut at
//! the strongest point it hides and each piece written again. Rests are stricter: no
//! syncopation, no dotted rest in simple time, and a rest over a beat only on the half bar.
//! Inside a tuplet the same rules run on its own units.

use crate::metre::{Metre, WEAK};
use crate::model::Dur;
use crate::read::{Window, STRAIGHT_FINEST};

/// One written piece of a note or a rest.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Bar-relative start and length as heard.
    pub start: i64,
    pub len: i64,
    pub dur: Dur,
    /// The tuplet window it lies in (an index into the windows given).
    pub window: Option<usize>,
}

/// How a region of the bar is counted.
struct Region<'a> {
    start: i64,
    end: i64,
    /// Written = heard × `num` / `den` (1/1 outside a tuplet).
    num: i64,
    den: i64,
    /// The finest point it has.
    step: i64,
    strength: &'a dyn Fn(i64) -> u8,
    /// Dotted rests are allowed only on a compound beat of exactly this length.
    compound_beat: Option<i64>,
    /// Inside a tuplet: any value it can write fits (its few units are read at a glance;
    /// quarter triplets cross the middle of their half note).
    flat: bool,
}

/// Write `[s, e)` (bar-relative) of a bar with `metre`, given the tuplet windows the
/// voice needs (bar-relative). `rest` for a rest.
pub fn write(metre: &Metre, windows: &[Window], s: i64, e: i64, rest: bool) -> Vec<Piece> {
    let mut out = Vec::new();
    // Cut at the windows' edges first.
    let mut cuts: Vec<i64> = vec![s, e];
    for w in windows {
        for t in [w.start, w.end()] {
            if t > s && t < e {
                cuts.push(t);
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let straight = |t: i64| metre.strength(t);
    let compound_beat = metre
        .beats
        .iter()
        .find(|b| metre.beat_is_ternary(b.1))
        .map(|b| b.1);
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        match windows.iter().position(|w| w.start <= a && b <= w.end()) {
            Some(wi) => {
                let w = windows[wi];
                let units = w.len / w.unit;
                let half = if units % 2 == 0 && units >= 6 {
                    units / 2
                } else {
                    0
                };
                let strength = move |t: i64| -> u8 {
                    let off = t - w.start;
                    if off <= 0 || off >= w.len {
                        0
                    } else if half > 0 && off == half * w.unit {
                        1
                    } else if off % w.unit == 0 {
                        2
                    } else {
                        WEAK
                    }
                };
                let r = Region {
                    start: w.start,
                    end: w.end(),
                    num: w.actual as i64,
                    den: w.normal as i64,
                    step: w.unit,
                    strength: &strength,
                    compound_beat: None,
                    flat: true,
                };
                let first = out.len();
                decompose(&r, a, b, rest, &mut out);
                for p in &mut out[first..] {
                    p.window = Some(wi);
                }
            }
            None => {
                let r = Region {
                    start: 0,
                    end: metre.len,
                    num: 1,
                    den: 1,
                    step: STRAIGHT_FINEST,
                    strength: &straight,
                    compound_beat,
                    flat: false,
                };
                decompose(&r, a, b, rest, &mut out);
            }
        }
    }
    out
}

fn written(r: &Region, len: i64) -> Option<Dur> {
    let w = len * r.num;
    if w % r.den != 0 {
        return None;
    }
    Dur::of(w / r.den)
}

/// The strongest point strictly inside `(s, e)` and where it is (the first of equals).
fn strongest_inside(r: &Region, s: i64, e: i64) -> Option<(u8, i64)> {
    let mut best: Option<(u8, i64)> = None;
    let mut t = r.start + ((s - r.start) / r.step + 1) * r.step;
    while t < e {
        if t > s {
            let st = (r.strength)(t);
            if best.is_none_or(|(b, _)| st < b) {
                best = Some((st, t));
            }
        }
        t += r.step;
    }
    best
}

fn fits(r: &Region, s: i64, e: i64, rest: bool) -> Option<Dur> {
    let dur = written(r, e - s)?;
    if r.flat {
        return (!rest || dur.dots == 0).then_some(dur);
    }
    let Some((inner, _)) = strongest_inside(r, s, e) else {
        return Some(dur);
    };
    if inner == WEAK {
        return Some(dur);
    }
    let ss = (r.strength)(s);
    let se = (r.strength)(e);
    if rest {
        if dur.dots > 0 && (r.num != 1 || r.compound_beat != Some(e - s)) {
            return None;
        }
        // A rest over a beat: only from a half bar (or the bar) to the next.
        if inner <= 2 {
            return (ss <= 1 && se <= 1).then_some(dur);
        }
        return (ss <= inner).then_some(dur);
    }
    if ss <= inner {
        return Some(dur);
    }
    // The centred syncopation: the note sits in the middle of a span twice its length
    // that starts and ends on points stronger than any it hides.
    let d = e - s;
    if d % 2 == 0 {
        let (c0, c1) = (s - d / 2, e + d / 2);
        if c0 >= r.start && c1 <= r.end && (r.strength)(c0) < inner && (r.strength)(c1) < inner {
            return Some(dur);
        }
    }
    // A dotted value inside one beat that ends on a beat (or stronger).
    if dur.dots == 1 && inner > 2 && se <= 2 {
        return Some(dur);
    }
    None
}

fn decompose(r: &Region, s: i64, e: i64, rest: bool, out: &mut Vec<Piece>) {
    if e <= s {
        return;
    }
    if let Some(dur) = fits(r, s, e, rest) {
        out.push(Piece {
            start: s,
            len: e - s,
            dur,
            window: None,
        });
        return;
    }
    match strongest_inside(r, s, e) {
        Some((_, at)) => {
            decompose(r, s, at, rest, out);
            decompose(r, at, e, rest, out);
        }
        None => {
            // No point inside to cut at (a length the grid cannot write): the longest
            // value that fits, then the rest.
            let mut len = e - s;
            while len > 0 && written(r, len).is_none() {
                len -= 1;
            }
            if len == 0 {
                return;
            }
            out.push(Piece {
                start: s,
                len,
                dur: written(r, len).unwrap(),
                window: None,
            });
            decompose(r, s + len, e, rest, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::time::TimeSignature;

    const Q: i64 = 960;

    fn metre(n: u8, d: u8) -> Metre {
        let sig = TimeSignature::new(n, d);
        Metre::new(sig, sig.ticks_per_bar().0)
    }

    fn values(p: &[Piece]) -> Vec<(i8, u8)> {
        p.iter().map(|p| (p.dur.log, p.dur.dots)).collect()
    }

    #[test]
    fn notes_in_four_four() {
        let m = metre(4, 4);
        assert_eq!(values(&write(&m, &[], 0, 4 * Q, false)), vec![(0, 0)]);
        assert_eq!(values(&write(&m, &[], 0, 3 * Q, false)), vec![(1, 1)]);
        // Quarter–half–quarter: the half on beat 2 is the centred syncopation.
        assert_eq!(values(&write(&m, &[], Q, 3 * Q, false)), vec![(1, 0)]);
        // A dotted half from beat 2 shows the half bar: quarter tied to half.
        assert_eq!(
            values(&write(&m, &[], Q, 4 * Q, false)),
            vec![(2, 0), (1, 0)]
        );
        // A dotted quarter off the beat shows beat 2: eighth tied to quarter.
        assert_eq!(
            values(&write(&m, &[], Q / 2, 2 * Q, false)),
            vec![(3, 0), (2, 0)]
        );
        // Eighth–quarter–eighth inside the half bar.
        assert_eq!(
            values(&write(&m, &[], Q / 2, 3 * Q / 2, false)),
            vec![(2, 0)]
        );
        // …but not over the half bar.
        assert_eq!(
            values(&write(&m, &[], 3 * Q / 2, 5 * Q / 2, false)),
            vec![(3, 0), (3, 0)]
        );
        // The Scotch snap: sixteenth, dotted eighth.
        assert_eq!(values(&write(&m, &[], Q / 4, Q, false)), vec![(3, 1)]);
    }

    #[test]
    fn rests_in_four_four() {
        let m = metre(4, 4);
        assert_eq!(values(&write(&m, &[], 2 * Q, 4 * Q, true)), vec![(1, 0)]);
        assert_eq!(
            values(&write(&m, &[], Q, 3 * Q, true)),
            vec![(2, 0), (2, 0)]
        );
        assert_eq!(
            values(&write(&m, &[], Q / 2, 2 * Q, true)),
            vec![(3, 0), (2, 0)]
        );
        assert_eq!(
            values(&write(&m, &[], 0, 3 * Q / 4, true)),
            vec![(3, 0), (4, 0)]
        );
    }

    #[test]
    fn compound_time() {
        let m = metre(6, 8);
        assert_eq!(values(&write(&m, &[], 0, 3 * Q / 2, false)), vec![(2, 1)]);
        assert_eq!(
            values(&write(&m, &[], Q / 2, 3 * Q / 2, false)),
            vec![(2, 0)]
        );
        assert_eq!(values(&write(&m, &[], 0, 3 * Q / 2, true)), vec![(2, 1)]);
        assert_eq!(
            values(&write(&m, &[], Q, 2 * Q, false)),
            vec![(3, 0), (3, 0)]
        );
    }

    #[test]
    fn triplets() {
        let m = metre(4, 4);
        let w = [Window {
            start: 0,
            len: Q,
            unit: Q / 3,
            actual: 3,
            normal: 2,
        }];
        let p = write(&m, &w, 0, Q / 3, false);
        assert_eq!(values(&p), vec![(3, 0)]);
        assert_eq!(p[0].window, Some(0));
        assert_eq!(values(&write(&m, &w, Q / 3, Q, false)), vec![(2, 0)]);
        // Across the window's end: the triplet eighth tied to a quarter.
        let p = write(&m, &w, 2 * Q / 3, 2 * Q, false);
        assert_eq!(values(&p), vec![(3, 0), (2, 0)]);
        assert_eq!(p[1].window, None);
    }
}
