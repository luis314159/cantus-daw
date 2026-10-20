//! The time map: output frame → source frame, piecewise linear through breakpoints.

/// Where each output frame reads the source: breakpoints `(output, source)`, outputs
/// strictly increasing, sources non-decreasing (a flat stretch holds a moment). Between
/// breakpoints the map is linear; before the first and after the last it goes on at the
/// slope of the segment beside it (slope 1 with a single breakpoint; the identity with
/// none).
///
/// A breakpoint is **hard** (a window's end, a warp marker: a place the audio is pinned to)
/// or **soft** (a sample of a smooth curve — the tempo map's ramp, every few hundred frames:
/// [`Map::with_soft`]). Rhythm cuts only at hard ones, and a hold around a transient may
/// move soft ones (ADR 0061 §2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    points: Vec<(f64, f64)>,
    /// Per point, whether it is soft; empty: every point is hard.
    soft: Vec<bool>,
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
        (finite && ok).then_some(Map {
            points,
            soft: Vec::new(),
        })
    }

    /// A map through `points` where `soft[i]` marks a soft point (see [`Map`]); `None` as
    /// [`Map::new`], or when `soft` is not one flag per point.
    pub fn with_soft(points: Vec<(f64, f64)>, soft: Vec<bool>) -> Option<Self> {
        if soft.len() != points.len() {
            return None;
        }
        let mut m = Map::new(points)?;
        if soft.iter().any(|s| *s) {
            m.soft = soft;
        }
        Some(m)
    }

    /// Whether point `i` is soft.
    pub fn is_soft(&self, i: usize) -> bool {
        self.soft.get(i).copied().unwrap_or(false)
    }

    /// The hard points (window ends and markers), in order.
    pub fn hard_points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.points
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.is_soft(*i))
            .map(|(_, p)| *p)
    }

    /// A constant speed: output frame `t` reads source frame `from + t · speed`.
    pub fn linear(from: f64, speed: f64) -> Self {
        Map {
            points: vec![(0.0, from), (1.0, from + speed)],
            soft: Vec::new(),
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

    /// This map with a pure shift held around every transient in `onsets` (source frames):
    /// [`HOLD_BEFORE_S`] before each to [`HOLD_AFTER_S`] after — the percussive part's
    /// holds. See [`Map::hold`].
    pub fn held_at(&self, onsets: &[f64], rate: f32) -> Map {
        let r = rate as f64;
        self.hold(onsets, HOLD_BEFORE_S * r, HOLD_AFTER_S * r).0
    }

    /// This map with a pure shift (slope 1) held from `before` frames ahead of every
    /// transient in `onsets` (source frames) to `after` frames past it, each hold through
    /// the transient's own place — so what is read through it is copied, never stretched.
    /// The hard breakpoints stay where they are (a hold never crosses one, and takes at
    /// most [`HOLD_SHARE`] of the room on either side, in output and in source frames
    /// alike); the stretch moves into the rest. Soft points (a tempo ramp's samples) do not
    /// bound a hold: those inside a hold, and those in the room beside it that takes up its
    /// stretch, are dropped — the curve there is followed by one straight segment — and
    /// the rest stay. Returns the map and the holds kept, in order.
    pub fn hold(&self, onsets: &[f64], before: f64, after: f64) -> (Map, Vec<Hold>) {
        let mut pts: Vec<(f64, f64)> = self.points.clone();
        let mut soft: Vec<bool> = (0..pts.len()).map(|i| self.is_soft(i)).collect();
        if pts.len() < 2 {
            let a = self.source(0.0);
            pts = vec![(0.0, a), (1.0, a + 1.0)];
            soft = vec![false; 2];
        }
        let fixed: Vec<f64> = self.hard_points().map(|p| p.0).collect();
        let places: Vec<f64> = onsets.iter().map(|&s| self.output(s)).collect();
        let mut holds: Vec<Hold> = Vec::new();
        for (i, &o) in places.iter().enumerate() {
            let prev_fixed = fixed.iter().rev().find(|&&f| f < o).copied();
            let next_fixed = fixed.iter().find(|&&f| f > o).copied();
            let prev_onset = i.checked_sub(1).map(|j| places[j]);
            let next_onset = places.get(i + 1).copied();
            let s = self.source(o);
            let back = [prev_fixed, prev_onset]
                .into_iter()
                .flatten()
                .map(|f| (o - f).min(s - self.source(f)) * HOLD_SHARE)
                .fold(before, f64::min)
                .max(0.0);
            let ahead = [next_fixed, next_onset]
                .into_iter()
                .flatten()
                .map(|f| (f - o).min(self.source(f) - s) * HOLD_SHARE)
                .fold(after, f64::min)
                .max(0.0);
            if back + ahead >= 1.0 {
                holds.push(Hold {
                    from: o - back,
                    at: o,
                    to: o + ahead,
                });
            }
        }
        if holds.is_empty() {
            if !soft.iter().any(|s| *s) {
                soft.clear();
            }
            return (Map { points: pts, soft }, holds);
        }
        // The room each hold's stretch is taken up in: as far again as the hold reaches on
        // each side divided by its share (in output and source frames both), never past a
        // hard point or the next transient (the shares above keep it inside). Soft points
        // there go; the zone's ends are kept, on the curve.
        let mut zones: Vec<(f64, f64)> = holds
            .iter()
            .map(|h| {
                let s = self.source(h.at);
                let rb = (h.at - h.from) / HOLD_SHARE;
                let ra = (h.to - h.at) / HOLD_SHARE;
                let lo = (h.at - rb).min(self.output(s - rb));
                let hi = (h.at + ra).max(self.output(s + ra));
                (lo, hi)
            })
            .collect();
        zones.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::with_capacity(zones.len());
        for z in zones {
            match merged.last_mut() {
                Some(l) if z.0 <= l.1 => l.1 = l.1.max(z.1),
                _ => merged.push(z),
            }
        }
        let inside = |o: f64, r: &[(f64, f64)]| r.iter().any(|z| o > z.0 && o < z.1);
        let held: Vec<(f64, f64)> = holds.iter().map(|h| (h.from, h.to)).collect();
        let mut kept_pts: Vec<(f64, f64)> = Vec::with_capacity(pts.len() + 4 * holds.len());
        let mut kept_soft: Vec<bool> = Vec::with_capacity(kept_pts.capacity());
        for (p, s) in pts.iter().zip(&soft) {
            if *s && (inside(p.0, &merged) || inside(p.0, &held)) {
                continue;
            }
            kept_pts.push(*p);
            kept_soft.push(*s);
        }
        let any_soft = soft.iter().any(|s| *s);
        if any_soft {
            for z in &merged {
                for o in [z.0, z.1] {
                    if !inside(o, &held) {
                        kept_pts.push((o, self.source(o)));
                        kept_soft.push(false);
                    }
                }
            }
        }
        let (mut pts, mut soft) = (kept_pts, kept_soft);
        for h in &holds {
            let s = self.source(h.at);
            pts.push((h.from, s - (h.at - h.from)));
            pts.push((h.to, s + (h.to - h.at)));
            soft.extend([false, false]);
        }
        let mut both: Vec<((f64, f64), bool)> = pts.into_iter().zip(soft).collect();
        both.sort_by(|a, b| a.0 .0.total_cmp(&b.0 .0));
        both.dedup_by(|b, a| b.0 .0 == a.0 .0);
        let pts: Vec<(f64, f64)> = both.iter().map(|b| b.0).collect();
        let soft: Vec<bool> = both.iter().map(|b| b.1).collect();
        // Never read backwards: a point that would is dropped (it cannot happen inside the
        // shares above; this keeps the map a function whatever the input).
        let mut kept: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        let mut kept_soft: Vec<bool> = Vec::with_capacity(pts.len());
        for (p, s) in pts.into_iter().zip(soft) {
            match kept.last() {
                Some(l) if p.1 < l.1 => {}
                _ => {
                    kept.push(p);
                    kept_soft.push(s);
                }
            }
        }
        if !kept_soft.iter().any(|s| *s) {
            kept_soft.clear();
        }
        (
            Map {
                points: kept,
                soft: kept_soft,
            },
            holds,
        )
    }
}

/// A stretch of output held to a pure shift around a transient (output frames).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hold {
    pub from: f64,
    /// Where the transient lands.
    pub at: f64,
    pub to: f64,
}
