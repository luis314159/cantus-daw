//! Voicings: each chord of a chart as pitches in a range, moving as little as possible
//! from the chord before (voice leading), the most essential tones kept when there are
//! fewer voices than tones, a bass under them if asked.
//!
//! The search is exhaustive and small: each chosen pitch class may sit in any octave of
//! the range (two to four choices in a usual range), at most eight voices — a few
//! thousand candidates a chord at worst. The cost of a candidate is how far its voices
//! move from the previous chord's (sorted, voice by voice), plus penalties for a gap of
//! more than an octave between neighbours, for intervals under a fifth low in the range
//! (they turn to mud below E3) and, for the first chord, for sitting far from the middle
//! of the range. Ties keep the lower candidate: the result is deterministic.

use super::chart::Chart;
use super::chord::Chord;

/// The most voices above the bass.
pub const MAX_VOICES: usize = 8;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VoiceSpec {
    /// Voices above the bass, 1…[`MAX_VOICES`].
    pub voices: usize,
    /// The range of those voices, MIDI pitches, inclusive.
    pub low: u8,
    pub high: u8,
    /// A bass voice under them (the chord's bass: the slash's note or the root), in this
    /// range; `None`: no bass.
    pub bass: Option<(u8, u8)>,
}

/// Below this pitch, neighbouring voices closer than a fifth sound muddy.
const MUD: u8 = 52;

/// Candidates a chord may try before the search narrows to where the music is.
const SEARCH_LIMIT: usize = 20_000;

/// Each chord of `chart` voiced (ascending pitches, the bass first when asked); an empty
/// voicing for `N.C.`. `None` when the range cannot hold the voices (each pitch class
/// needs an octave of room: `high - low` ≥ 11, and at least `voices` distinct pitches).
pub fn voice_chart(chart: &Chart, spec: &VoiceSpec) -> Option<Vec<Vec<u8>>> {
    if spec.voices == 0
        || spec.voices > MAX_VOICES
        || spec.high > 127
        || spec.high < spec.low.saturating_add(11)
    {
        return None;
    }
    if let Some((lo, hi)) = spec.bass {
        if hi > 127 || hi < lo.saturating_add(11) {
            return None;
        }
    }
    let mut out = Vec::with_capacity(chart.chords.len());
    let mut prev: Option<Vec<u8>> = None;
    let mut prev_bass: Option<u8> = None;
    for c in &chart.chords {
        let Some(chord) = c.chord.as_ref() else {
            out.push(Vec::new());
            continue;
        };
        let upper = voice_one(chord, spec, prev.as_deref())?;
        let mut v = Vec::with_capacity(upper.len() + 1);
        if let Some((lo, hi)) = spec.bass {
            let b = nearest_in(chord.bass, lo, hi, prev_bass.unwrap_or(lo));
            prev_bass = Some(b);
            v.push(b);
        }
        v.extend(&upper);
        prev = Some(upper);
        out.push(v);
    }
    Some(out)
}

/// The pitch of class `pc` in `lo..=hi` nearest `to` (the lower on a tie).
fn nearest_in(pc: u8, lo: u8, hi: u8, to: u8) -> u8 {
    (lo..=hi)
        .filter(|p| p % 12 == pc)
        .min_by_key(|p| ((*p as i32 - to as i32).abs(), *p))
        .unwrap_or(lo)
}

/// The pitch classes the voices take: the most essential tones, then doublings (the root,
/// the fifth, the third) when there are more voices than tones.
fn classes(chord: &Chord, voices: usize) -> Vec<u8> {
    let by = chord.by_priority();
    let mut pcs: Vec<u8> = by.iter().copied().take(voices).collect();
    // A chord of more tones than voices keeps its colour, but the root is worth more than
    // a second tension when the voicing has no bass of its own.
    let root = chord.root;
    if voices >= 3 && by.len() > voices && !pcs.contains(&root) {
        pcs.pop();
        pcs.push(root);
    }
    let doubling = [Some(root), chord.fifth(), by.first().copied()];
    let mut k = 0;
    while pcs.len() < voices {
        if let Some(pc) = doubling[k % doubling.len()] {
            pcs.push(pc);
        }
        k += 1;
        if k > 3 * MAX_VOICES {
            break;
        }
    }
    pcs
}

fn voice_one(chord: &Chord, spec: &VoiceSpec, prev: Option<&[u8]>) -> Option<Vec<u8>> {
    let pcs = classes(chord, spec.voices);
    let mut options: Vec<Vec<u8>> = pcs
        .iter()
        .map(|pc| (spec.low..=spec.high).filter(|p| p % 12 == *pc).collect())
        .collect();
    // A wide range and many voices: search only near where the music is (the previous
    // chord's span, or the middle of the range), widened a fifth each way.
    let combos: usize = options.iter().map(Vec::len).product();
    if combos > SEARCH_LIMIT {
        let (lo, hi) = match prev {
            Some(p) if !p.is_empty() => (p[0] as i32 - 7, p[p.len() - 1] as i32 + 7),
            _ => {
                let c = (spec.low as i32 + spec.high as i32) / 2;
                (c - 12, c + 12)
            }
        };
        let narrowed: Vec<Vec<u8>> = options
            .iter()
            .map(|o| {
                o.iter()
                    .copied()
                    .filter(|p| (lo..=hi).contains(&(*p as i32)))
                    .collect()
            })
            .collect();
        if narrowed.iter().all(|o| !o.is_empty()) {
            options = narrowed;
        }
    }
    if options.iter().any(Vec::is_empty) {
        return None;
    }
    let mut best: Option<(i64, Vec<u8>)> = None;
    let mut pick = vec![0usize; options.len()];
    loop {
        let mut v: Vec<u8> = pick.iter().zip(&options).map(|(i, o)| o[*i]).collect();
        v.sort_unstable();
        let distinct = v.windows(2).all(|w| w[0] != w[1]);
        if distinct {
            let cost = cost(&v, spec, prev);
            if best
                .as_ref()
                .is_none_or(|(c, b)| cost < *c || (cost == *c && v < *b))
            {
                best = Some((cost, v));
            }
        }
        // The next combination (odometer).
        let mut i = 0;
        loop {
            if i == pick.len() {
                return best.map(|(_, v)| v);
            }
            pick[i] += 1;
            if pick[i] < options[i].len() {
                break;
            }
            pick[i] = 0;
            i += 1;
        }
    }
}

fn cost(v: &[u8], spec: &VoiceSpec, prev: Option<&[u8]>) -> i64 {
    let mut c: i64 = 0;
    for w in v.windows(2) {
        let gap = (w[1] - w[0]) as i64;
        if gap > 12 {
            c += 4 * (gap - 12);
        }
        if w[0] < MUD && gap < 7 {
            c += 6 * (7 - gap);
        }
    }
    match prev {
        Some(p) if !p.is_empty() => {
            // Voice by voice over the shorter of the two, plus the shift of the middle.
            let n = p.len().min(v.len());
            for i in 0..n {
                c += 2 * (v[i] as i64 - p[i] as i64).abs();
            }
            let mid = |x: &[u8]| x.iter().map(|&p| p as i64).sum::<i64>() / x.len() as i64;
            c += (mid(v) - mid(p)).abs();
        }
        _ => {
            let centre = (spec.low as i64 + spec.high as i64) / 2;
            let mid = v.iter().map(|&p| p as i64).sum::<i64>() / v.len() as i64;
            c += 2 * (mid - centre).abs();
            // A first chord in close position (within an octave and a half) reads best.
            let span = (v[v.len() - 1] - v[0]) as i64;
            if span > 18 {
                c += span - 18;
            }
        }
    }
    c
}
