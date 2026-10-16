//! The time map: output frame → source frame, piecewise linear through breakpoints.

/// Where each output frame reads the source: breakpoints `(output, source)`, outputs
/// strictly increasing, sources non-decreasing (a flat stretch holds a moment). Between
/// breakpoints the map is linear; before the first and after the last it goes on at the
/// slope of the segment beside it (slope 1 with a single breakpoint; the identity with
/// none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    points: Vec<(f64, f64)>,
}

/// A slope this close to 1, and an offset this close to a whole frame, read the source
/// sample for sample.
const PLAIN_SLOPE: f64 = 1e-12;
const PLAIN_OFFSET: f64 = 1e-7;

/// Around a transient, the percussive part is copied over this long before it…
const HOLD_BEFORE_S: f64 = 0.004;
/// …and this long after it (the attack and the first of the decay).
const HOLD_AFTER_S: f64 = 0.030;
/// A held stretch takes at most this share of the gap to the next transient or
/// breakpoint, so the rest can still stretch.
const HOLD_SHARE: f64 = 0.45;

impl Map {
    /// The identity: output frame `t` reads source frame `t`.
    pub fn identity() -> Self {
        Map::default()
    }

    /// A map through `points`; `None` unless every coordinate is finite, the outputs
    /// strictly increase and the sources never decrease.
    pub fn new(points: Vec<(f64, f64)>) -> Option<Self> {
        let finite = points.iter().all(|p| p.0.is_finite() && p.1.is_finite());
        let ok = points
            .windows(2)
            .all(|w| w[1].0 > w[0].0 && w[1].1 >= w[0].1);
        (finite && ok).then_some(Map { points })
    }

    /// A constant speed: output frame `t` reads source frame `from + t · speed`.
    pub fn linear(from: f64, speed: f64) -> Self {
        Map {
            points: vec![(0.0, from), (1.0, from + speed)],
        }
    }

    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// The segment holding output time `t`: a point on it and its slope.
    fn segment(&self, t: f64) -> ((f64, f64), f64) {
        let p = &self.points;
        match p.len() {
            0 => ((0.0, 0.0), 1.0),
            1 => (p[0], 1.0),
            n => {
                let i = p.partition_point(|q| q.0 <= t).clamp(1, n - 1);
                let (a, b) = (p[i - 1], p[i]);
                (a, (b.1 - a.1) / (b.0 - a.0))
            }
        }
    }

    /// The source frame output frame `t` reads.
    pub fn source(&self, t: f64) -> f64 {
        let (a, k) = self.segment(t);
        a.1 + (t - a.0) * k
    }

    /// The map's speed at output frame `t` (source frames per output frame).
    pub fn slope(&self, t: f64) -> f64 {
        self.segment(t).1
    }

    /// The first output frame that reads source frame `s` (the inverse; a flat stretch
    /// answers its start).
    pub fn output(&self, s: f64) -> f64 {
        let p = &self.points;
        match p.len() {
            0 => s,
            1 => p[0].0 + (s - p[0].1),
            n => {
                let i = p.partition_point(|q| q.1 < s).clamp(1, n - 1);
                let (a, b) = (p[i - 1], p[i]);
                let k = (b.1 - a.1) / (b.0 - a.0);
                if k <= 0.0 {
                    return if s <= a.1 { a.0 } else { b.0 };
                }
                a.0 + (s - a.1) / k
            }
        }
    }

    /// When output frames `0..len` read the source sample for sample — slope 1 at a whole
    /// offset throughout — that offset (output frame `t` reads source frame `t + offset`).
    pub fn plain_over(&self, len: usize) -> Option<i64> {
        let end = len as f64;
        let mut offset: Option<f64> = None;
        let mut check = |a: (f64, f64), k: f64| -> bool {
            let d = a.1 - a.0;
            let whole = d.round();
            if (k - 1.0).abs() > PLAIN_SLOPE || (d - whole).abs() > PLAIN_OFFSET {
                return false;
            }
            match offset {
                None => {
                    offset = Some(whole);
                    true
                }
                Some(o) => o == whole,
            }
        };
        let p = &self.points;
        if p.len() < 2 {
            let (a, k) = self.segment(0.0);
            return check(a, k).then(|| offset.unwrap_or(0.0) as i64);
        }
        // Every segment that output frames 0..len touch (the edges extend the first and
        // the last).
        for i in 1..p.len() {
            let (a, b) = (p[i - 1], p[i]);
            let lo = if i == 1 { f64::NEG_INFINITY } else { a.0 };
            let hi = if i == p.len() - 1 { f64::INFINITY } else { b.0 };
            if hi <= 0.0 || lo >= end {
                continue;
            }
            if !check(a, (b.1 - a.1) / (b.0 - a.0)) {
                return None;
            }
        }
        offset.map(|o| o as i64)
    }

    /// This map with a pure shift (slope 1) held around every transient in `onsets`
    /// (source frames): from a little before each to the first of its decay, each hold
    /// through the transient's own place, so a hit read through it is copied, never
    /// stretched. The breakpoints stay where they are (a hold never crosses one, and takes
    /// at most [`HOLD_SHARE`] of the room on either side); the stretch moves into the rest.
    pub fn held_at(&self, onsets: &[f64], rate: f32) -> Map {
        let before = HOLD_BEFORE_S * rate as f64;
        let after = HOLD_AFTER_S * rate as f64;
        let mut pts: Vec<(f64, f64)> = self.points.clone();
        if pts.len() < 2 {
            let a = self.source(0.0);
            pts = vec![(0.0, a), (1.0, a + 1.0)];
        }
        // Output places of the transients, and the fixed points (breakpoints) around them.
        let fixed: Vec<f64> = self.points.iter().map(|p| p.0).collect();
        let places: Vec<(f64, f64)> = onsets.iter().map(|&s| (self.output(s), s)).collect();
        let mut holds: Vec<(f64, f64, f64)> = Vec::new(); // (out, back, ahead)
        for (i, &(o, _)) in places.iter().enumerate() {
            let prev_fixed = fixed.iter().rev().find(|&&f| f < o).copied();
            let next_fixed = fixed.iter().find(|&&f| f > o).copied();
            let prev_onset = i.checked_sub(1).map(|j| places[j].0);
            let next_onset = places.get(i + 1).map(|p| p.0);
            // Room in output frames and in source frames alike: a hold copies, so it
            // takes as much of each.
            let s = self.source(o);
            let room_back = [prev_fixed, prev_onset]
                .into_iter()
                .flatten()
                .map(|f| (o - f).min(s - self.source(f)) * HOLD_SHARE)
                .fold(before, f64::min);
            let room_ahead = [next_fixed, next_onset]
                .into_iter()
                .flatten()
                .map(|f| (f - o).min(self.source(f) - s) * HOLD_SHARE)
                .fold(after, f64::min);
            if room_back + room_ahead >= 1.0 {
                holds.push((o, room_back.max(0.0), room_ahead.max(0.0)));
            }
        }
        if holds.is_empty() {
            return Map { points: pts };
        }
        // The breakpoints, and each hold's two ends (a straight line through the
        // transient's place at slope 1).
        for &(o, back, ahead) in &holds {
            let s = self.source(o);
            pts.push((o - back, s - back));
            pts.push((o + ahead, s + ahead));
        }
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        pts.dedup_by(|b, a| b.0 == a.0);
        // A hold whose ends would make the source run back (a squeezed segment) is
        // dropped there: keep the map a function that never reads backwards.
        let mut kept: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        for p in pts {
            match kept.last() {
                Some(l) if p.1 < l.1 => {}
                _ => kept.push(p),
            }
        }
        Map { points: kept }
    }
}
