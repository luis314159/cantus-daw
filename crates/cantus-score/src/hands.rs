//! The grand staff's split: which notes the right hand plays (upper staff) and which the
//! left (lower) — following the hands, not a fixed line at middle C.
//!
//! Each hand keeps a centre that drifts with what it plays. At every onset the chord is
//! cut where the two hands are cheapest: each note costs its distance from its hand's
//! centre, a hand stretched past a tenth (with what it still holds) costs more, and a
//! note on the far side of middle C costs a little — so a melody that dips below C4 stays
//! in the right hand and a bass line that climbs over it stays in the left.

use crate::read::ReadNote;

/// The widest a hand comfortably holds, in semitones (a tenth).
const SPAN: i32 = 16;

/// For each of `notes` (sorted by start), 0 for the upper staff or 1 for the lower.
pub fn split(notes: &[ReadNote], fixed: Option<u8>) -> Vec<u8> {
    if let Some(p) = fixed {
        return notes.iter().map(|n| u8::from(n.pitch < p)).collect();
    }
    let mut out = vec![0u8; notes.len()];
    let (mut rh, mut lh) = (67.0f64, 48.0f64);
    // Each hand's last onset: a line continues from it.
    let (mut last_r, mut last_l): (Vec<i32>, Vec<i32>) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < notes.len() {
        let t = notes[i].start;
        let mut j = i;
        while j < notes.len() && notes[j].start == t {
            j += 1;
        }
        let mut idx: Vec<usize> = (i..j).collect();
        idx.sort_by_key(|&k| notes[k].pitch);
        // What each hand still holds at this onset.
        let held = |hand: u8| -> Vec<i32> {
            (0..i)
                .filter(|&k| out[k] == hand && notes[k].end > t)
                .map(|k| notes[k].pitch as i32)
                .collect()
        };
        let (held_r, held_l) = (held(0), held(1));
        let note_cost = |p: i32, centre: f64, last: &[i32], held: &[i32], right: bool| {
            let near = last.iter().map(|&q| (p - q).abs()).min().unwrap_or(0) as f64;
            let wrong_side = if right { p < 60 } else { p >= 60 };
            0.5 * (p as f64 - centre).abs()
                + near
                + if wrong_side { 1.5 } else { 0.0 }
                // A hand holding a chord is less likely to move on with a new line.
                + 2.0 * held.len() as f64
        };
        let mut best = (f64::MAX, 0usize);
        for cut in 0..=idx.len() {
            let left: Vec<i32> = idx[..cut].iter().map(|&k| notes[k].pitch as i32).collect();
            let right: Vec<i32> = idx[cut..].iter().map(|&k| notes[k].pitch as i32).collect();
            let mut cost: f64 = left
                .iter()
                .map(|&p| note_cost(p, lh, &last_l, &held_l, false))
                .sum::<f64>()
                + right
                    .iter()
                    .map(|&p| note_cost(p, rh, &last_r, &held_r, true))
                    .sum::<f64>();
            let stretch = |a: &[i32], b: &[i32]| {
                let (lo, hi) = a
                    .iter()
                    .chain(b.iter())
                    .fold((i32::MAX, i32::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
                if lo > hi {
                    0
                } else {
                    (hi - lo - SPAN).max(0)
                }
            };
            cost += 12.0 * (stretch(&left, &held_l) + stretch(&right, &held_r)) as f64;
            // Never cross a hand that is still holding a note.
            if let (Some(&top_l), Some(&low_r)) = (left.iter().max(), held_r.iter().min()) {
                if top_l > low_r {
                    cost += 40.0;
                }
            }
            if let (Some(&low_r), Some(&top_l)) = (right.iter().min(), held_l.iter().max()) {
                if low_r < top_l {
                    cost += 40.0;
                }
            }
            if cost < best.0 {
                best = (cost, cut);
            }
        }
        let cut = best.1;
        for (n, &k) in idx.iter().enumerate() {
            out[k] = u8::from(n < cut);
        }
        let pitches = |ks: &[usize]| {
            ks.iter()
                .map(|&k| notes[k].pitch as i32)
                .collect::<Vec<_>>()
        };
        let mean = |ps: &[i32]| ps.iter().map(|&p| p as f64).sum::<f64>() / ps.len() as f64;
        if cut > 0 {
            last_l = pitches(&idx[..cut]);
            lh = 0.6 * lh + 0.4 * mean(&last_l);
        }
        if cut < idx.len() {
            last_r = pitches(&idx[cut..]);
            rh = 0.6 * rh + 0.4 * mean(&last_r);
        }
        i = j;
    }
    out
}

/// Whether a part wants a grand staff on *Auto*: it reaches well into both clefs, at
/// once or over the part (a piano part), rather than living in one.
pub fn wants_grand(notes: &[ReadNote]) -> bool {
    if notes.len() < 2 {
        return false;
    }
    let mut p: Vec<u8> = notes.iter().map(|n| n.pitch).collect();
    p.sort_unstable();
    let lo = p[p.len() / 10];
    let hi = p[p.len() - 1 - p.len() / 10];
    // A tenth or more on each side of middle C, and two octaves between.
    lo <= 52 && hi >= 67 && hi - lo >= 24
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(start: i64, end: i64, pitch: u8) -> ReadNote {
        ReadNote {
            id: start as u64 * 1000 + pitch as u64,
            start,
            end,
            pitch,
            velocity: 80,
        }
    }

    #[test]
    fn a_melody_below_middle_c_stays_in_the_right_hand() {
        // RH: E4 D4 C4 B3 A3 over a LH held C3–G3 fifth.
        let mut notes = vec![n(0, 3840, 48), n(0, 3840, 55)];
        for (i, p) in [64u8, 62, 60, 59, 57].iter().enumerate() {
            notes.push(n(i as i64 * 480, i as i64 * 480 + 480, *p));
        }
        notes.sort_by_key(|n| (n.start, n.pitch));
        let s = split(&notes, None);
        for (note, staff) in notes.iter().zip(&s) {
            let want = u8::from(note.pitch < 56);
            assert_eq!(*staff, want, "{note:?}");
        }
    }

    #[test]
    fn a_fixed_split_is_a_line() {
        let notes = vec![n(0, 10, 61), n(0, 10, 62)];
        assert_eq!(split(&notes, Some(62)), vec![1, 0]);
    }
}
