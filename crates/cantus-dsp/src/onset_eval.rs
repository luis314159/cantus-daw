//! **How well a transcription matches the truth** (round 34, ADR 0086): the measures the
//! field reports, by `mir_eval`'s rules, so Cantus's numbers stand beside published ones.
//!
//! - **Notes** ([`notes`], `mir_eval.transcription.precision_recall_f1_overlap`): a note
//!   found matches a true one when its onset is within 50 ms, its pitch within 50 cents,
//!   and — for the measure *with* offsets — its end within 20 % of the true note's length
//!   or 50 ms, whichever is more. Distances are rounded to 4 decimals before the
//!   comparison, as `mir_eval` does.
//! - **Onsets** ([`onsets`], `mir_eval.onset.f_measure`): an onset within 50 ms of a true
//!   one; for drums, class by class.
//!
//! Each true event matches at most one found and the other way round, the matching the
//! largest possible (a maximum bipartite matching, as `mir_eval` computes it); precision is
//! the matches over what was found, recall over the truth, F their harmonic mean.

/// A note: onset and offset in seconds, pitch in MIDI semitones (fractions allowed).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Note {
    pub onset: f64,
    pub offset: f64,
    pub pitch: f64,
}

/// Precision, recall and F-measure, and the counts behind them.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Score {
    pub precision: f64,
    pub recall: f64,
    pub f: f64,
    pub matched: usize,
    pub truth: usize,
    pub found: usize,
}

impl Score {
    pub fn of(matched: usize, truth: usize, found: usize) -> Score {
        let precision = if found == 0 {
            0.0
        } else {
            matched as f64 / found as f64
        };
        let recall = if truth == 0 {
            0.0
        } else {
            matched as f64 / truth as f64
        };
        let f = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        Score {
            precision,
            recall,
            f,
            matched,
            truth,
            found,
        }
    }

    /// Several pieces' counts summed (a micro-average).
    pub fn sum(scores: &[Score]) -> Score {
        let (m, t, f) = scores.iter().fold((0, 0, 0), |(m, t, f), s| {
            (m + s.matched, t + s.truth, f + s.found)
        });
        Score::of(m, t, f)
    }
}

/// `np.around(x, 4)`.
fn round4(x: f64) -> f64 {
    (x * 1e4).round_ties_even() / 1e4
}

/// The note F-measure. `offset_ratio` `None` ignores offsets ("F no offset", the field's
/// main number); `Some(0.2)` is the measure with offsets.
pub fn notes(truth: &[Note], found: &[Note], offset_ratio: Option<f64>) -> Score {
    const ONSET: f64 = 0.05;
    const CENTS: f64 = 50.0;
    const OFFSET_MIN: f64 = 0.05;
    let edges: Vec<Vec<usize>> = truth
        .iter()
        .map(|r| {
            found
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    let on = round4((r.onset - e.onset).abs()) <= ONSET;
                    let pitch = round4((100.0 * (r.pitch - e.pitch)).abs()) <= CENTS;
                    let off = offset_ratio.is_none_or(|ratio| {
                        let tol = (ratio * (r.offset - r.onset)).max(OFFSET_MIN);
                        round4((r.offset - e.offset).abs()) <= tol
                    });
                    on && pitch && off
                })
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    Score::of(matching(&edges, found.len()), truth.len(), found.len())
}

/// The onset F-measure within `window` seconds (0.05 in `mir_eval`).
pub fn onsets(truth: &[f64], found: &[f64], window: f64) -> Score {
    let edges: Vec<Vec<usize>> = truth
        .iter()
        .map(|r| {
            found
                .iter()
                .enumerate()
                .filter(|(_, e)| (r - *e).abs() <= window)
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    Score::of(matching(&edges, found.len()), truth.len(), found.len())
}

/// The size of a maximum matching of a bipartite graph (Kuhn's augmenting paths): `edges`
/// lists, for each left node, the right nodes it may match.
pub fn matching(edges: &[Vec<usize>], right: usize) -> usize {
    let mut owner: Vec<Option<usize>> = vec![None; right];
    let mut size = 0;
    for l in 0..edges.len() {
        let mut seen = vec![false; right];
        if augment(l, edges, &mut owner, &mut seen) {
            size += 1;
        }
    }
    size
}

fn augment(l: usize, edges: &[Vec<usize>], owner: &mut [Option<usize>], seen: &mut [bool]) -> bool {
    for &r in &edges[l] {
        if seen[r] {
            continue;
        }
        seen[r] = true;
        if owner[r].is_none_or(|o| augment(o, edges, owner, seen)) {
            owner[r] = Some(l);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(onset: f64, offset: f64, pitch: f64) -> Note {
        Note {
            onset,
            offset,
            pitch,
        }
    }

    #[test]
    fn a_perfect_transcription_scores_one_and_an_empty_one_zero() {
        let t = [n(0.0, 0.5, 60.0), n(0.5, 1.0, 64.0)];
        assert_eq!(notes(&t, &t, Some(0.2)).f, 1.0);
        assert_eq!(notes(&t, &[], None).f, 0.0);
        assert_eq!(notes(&[], &t, None).f, 0.0);
    }

    #[test]
    fn the_tolerances_are_mir_evals() {
        let t = [n(1.0, 2.0, 60.0)];
        // 50 ms is in (rounded to 4 decimals first), 51 ms is out.
        assert_eq!(notes(&t, &[n(1.05, 2.0, 60.0)], None).matched, 1);
        assert_eq!(notes(&t, &[n(1.051, 2.0, 60.0)], None).matched, 0);
        // 50 cents is in, a semitone is out.
        assert_eq!(notes(&t, &[n(1.0, 2.0, 60.5)], None).matched, 1);
        assert_eq!(notes(&t, &[n(1.0, 2.0, 61.0)], None).matched, 0);
        // Offsets: 20 % of a second is 200 ms; a short note gets 50 ms.
        assert_eq!(notes(&t, &[n(1.0, 2.2, 60.0)], Some(0.2)).matched, 1);
        assert_eq!(notes(&t, &[n(1.0, 2.21, 60.0)], Some(0.2)).matched, 0);
        assert_eq!(notes(&t, &[n(1.0, 2.21, 60.0)], None).matched, 1);
        let short = [n(1.0, 1.1, 60.0)];
        assert_eq!(notes(&short, &[n(1.0, 1.15, 60.0)], Some(0.2)).matched, 1);
        assert_eq!(notes(&short, &[n(1.0, 1.16, 60.0)], Some(0.2)).matched, 0);
    }

    #[test]
    fn the_matching_is_the_largest_not_the_greediest() {
        // Truth a can take found x or y, truth b only x: greedy a→x leaves b alone.
        let t = [n(1.00, 2.0, 60.0), n(1.06, 2.0, 60.0)];
        let f = [n(1.03, 2.0, 60.0), n(0.96, 2.0, 60.0)];
        let s = notes(&t, &f, None);
        assert_eq!(s.matched, 2, "{s:?}");
        // And each event at most once.
        let s = notes(
            &[n(1.0, 2.0, 60.0)],
            &[n(1.0, 2.0, 60.0), n(1.01, 2.0, 60.0)],
            None,
        );
        assert_eq!((s.matched, s.precision, s.recall), (1, 0.5, 1.0));
    }

    #[test]
    fn onsets_and_sums() {
        let s = onsets(&[0.0, 1.0, 2.0], &[0.04, 1.06, 2.0, 3.0], 0.05);
        assert_eq!((s.matched, s.truth, s.found), (2, 3, 4));
        assert!((s.f - 2.0 * (0.5 * 2.0 / 3.0) / (0.5 + 2.0 / 3.0)).abs() < 1e-12);
        let sum = Score::sum(&[s, Score::of(3, 3, 3)]);
        assert_eq!((sum.matched, sum.truth, sum.found), (5, 6, 7));
    }
}
