//! The warp edits (ADR 0061): pure functions from a warp to the next, each keeping it valid
//! — a marker is never moved past its neighbours, never put where one already is. The
//! interface makes every gesture from these and sends the result as one `SetClipWarp`.

use super::{frames_per_tick, Detected, Warp, WarpMarker, MIN_GAP};
use crate::time::{Ticks, PPQ};

impl Warp {
    /// The index of the marker on `tick`, if any.
    pub fn marker_at(&self, tick: Ticks) -> Option<usize> {
        self.markers.iter().position(|m| m.tick == tick)
    }

    /// The ticks a marker at `i` may move between (exclusive of its neighbours).
    fn tick_room(&self, i: usize) -> (i64, i64) {
        let lo = i
            .checked_sub(1)
            .map_or(i64::MIN / 4, |j| self.markers[j].tick.0 + MIN_GAP.0);
        let hi = self
            .markers
            .get(i + 1)
            .map_or(i64::MAX / 4, |m| m.tick.0 - MIN_GAP.0);
        (lo, hi)
    }

    /// The frames a marker at `i` may hold (strictly between its neighbours').
    fn frame_room(&self, i: usize) -> (f64, f64) {
        let lo = i
            .checked_sub(1)
            .map_or(f64::NEG_INFINITY, |j| self.markers[j].frame);
        let hi = self.markers.get(i + 1).map_or(f64::INFINITY, |m| m.frame);
        (lo, hi)
    }

    /// Insert `m` in order, unless it would sit on or between a neighbour's tick or frame.
    fn insert(&self, m: WarpMarker) -> Option<Warp> {
        let i = self.markers.partition_point(|k| k.tick < m.tick);
        let before_ok = i == 0
            || (m.tick.0 - self.markers[i - 1].tick.0 >= MIN_GAP.0
                && m.frame > self.markers[i - 1].frame);
        let after_ok = self
            .markers
            .get(i)
            .is_none_or(|n| n.tick.0 - m.tick.0 >= MIN_GAP.0 && n.frame > m.frame);
        if !before_ok || !after_ok || !m.frame.is_finite() {
            return None;
        }
        let mut w = self.clone();
        w.markers.insert(i, m);
        Some(w)
    }

    /// A marker on content tick `tick`, pinning the source frame heard there now (nothing
    /// moves until it is dragged). `None` when one is already there.
    pub fn add_marker(&self, tick: Ticks) -> Option<Warp> {
        self.insert(WarpMarker {
            frame: self.frame_at(tick.0 as f64),
            tick,
        })
    }

    /// A marker pairing source frame `frame` with content tick `tick` exactly; `None` where
    /// it would not keep the markers in order.
    pub fn with_marker(&self, frame: f64, tick: Ticks) -> Option<Warp> {
        self.insert(WarpMarker { frame, tick })
    }

    /// A marker on source frame `frame` (a transient), at the whole tick nearest where it
    /// is heard now.
    pub fn add_marker_at_frame(&self, frame: f64) -> Option<Warp> {
        self.insert(WarpMarker {
            frame,
            tick: Ticks(self.tick_at(frame).round() as i64),
        })
    }

    /// Marker `i` moved to content tick `tick` (clamped between its neighbours): the audio
    /// on either side of it stretches.
    pub fn move_marker(&self, i: usize, tick: Ticks) -> Warp {
        let mut w = self.clone();
        if i < w.markers.len() {
            let (lo, hi) = w.tick_room(i);
            w.markers[i].tick = Ticks(tick.0.clamp(lo, hi.max(lo)));
        }
        w
    }

    /// The source under marker `i` changed to frame `frame` (clamped strictly between its
    /// neighbours' frames): the content slides under a marker that stays put.
    pub fn move_content(&self, i: usize, frame: f64) -> Warp {
        let mut w = self.clone();
        if i < w.markers.len() && frame.is_finite() {
            let (lo, hi) = w.frame_room(i);
            let eps = 1e-3;
            let f = frame.max(lo + eps).min(hi - eps);
            if f > lo && f < hi {
                w.markers[i].frame = f;
            }
        }
        w
    }

    /// Marker `i` taken away.
    pub fn remove_marker(&self, i: usize) -> Warp {
        let mut w = self.clone();
        if i < w.markers.len() {
            w.markers.remove(i);
        }
        w
    }

    /// The tempo the source plays at on average across its markers (its own tempo with
    /// fewer than two), BPM at the source's rate — what the inspector shows.
    pub fn tempo(&self) -> f64 {
        let m = &self.markers;
        if m.len() < 2 {
            return self.bpm;
        }
        let (a, b) = (m[0], m[m.len() - 1]);
        let fpt = (b.frame - a.frame) / (b.tick.0 - a.tick.0) as f64;
        self.rate as f64 * 60.0 / (fpt * PPQ as f64)
    }

    /// The source's tempo set to `bpm`: every marker's tick scaled about the first so the
    /// average is `bpm` (the clip plays that much faster or slower as a whole), and the
    /// tempo outside them with it. Markers that would meet keep a tick apart.
    pub fn with_tempo(&self, bpm: f64) -> Warp {
        let mut w = self.clone();
        if !bpm.is_finite() || bpm <= 0.0 {
            return w;
        }
        let k = bpm / self.tempo();
        w.bpm = (self.bpm * k).clamp(super::MIN_BPM, super::MAX_BPM);
        if let Some(first) = self.markers.first().map(|m| m.tick.0) {
            let mut last = i64::MIN;
            for m in &mut w.markers {
                let t = first + ((m.tick.0 - first) as f64 * k).round() as i64;
                let t = t.max(last.saturating_add(MIN_GAP.0));
                m.tick = Ticks(t);
                last = t;
            }
        }
        w
    }

    /// The content stretched `k` times longer about content tick `about` (a clip's right
    /// edge dragged with Alt): every marker's distance from it scaled, the tempo outside
    /// the markers scaled with them (a source read slower is a source of a faster tempo,
    /// against the project's). Markers that would meet keep a tick apart.
    pub fn stretched(&self, about: Ticks, k: f64) -> Warp {
        let mut w = self.clone();
        if !k.is_finite() || k <= 0.0 {
            return w;
        }
        w.bpm = (self.bpm * k).clamp(super::MIN_BPM, super::MAX_BPM);
        let mut last = i64::MIN;
        for m in &mut w.markers {
            let t = about.0 + ((m.tick.0 - about.0) as f64 * k).round() as i64;
            let t = t.max(last.saturating_add(MIN_GAP.0));
            m.tick = Ticks(t);
            last = t;
        }
        w
    }

    /// Quantise: each transient (source frames) heard between content ticks `from` and
    /// `to` pulled toward the nearest line of `grid` by `strength` (0 … 1) — a marker put on
    /// it (or the one already there moved). Transients that would cross a neighbour stay.
    pub fn quantise(
        &self,
        transients: &[f64],
        grid: Ticks,
        strength: f32,
        from: Ticks,
        to: Ticks,
    ) -> Warp {
        self.quantise_from(transients, grid, Ticks::ZERO, strength, from, to)
    }

    /// [`Warp::quantise`] to a grid laid from timeline tick 0 when content tick 0 sits at
    /// timeline tick `origin` (`start − offset`): the lines the user sees.
    pub fn quantise_from(
        &self,
        transients: &[f64],
        grid: Ticks,
        origin: Ticks,
        strength: f32,
        from: Ticks,
        to: Ticks,
    ) -> Warp {
        let mut w = self.clone();
        if grid.0 <= 0 {
            return w;
        }
        let s = strength.clamp(0.0, 1.0) as f64;
        // The targets first, from the map as it is (a pulled transient moves its
        // neighbours' places only once it is a marker).
        let wanted: Vec<(f64, i64)> = transients
            .iter()
            .filter_map(|&f| {
                let t = self.tick_at(f);
                if t < from.0 as f64 || t >= to.0 as f64 {
                    return None;
                }
                let o = origin.0 as f64;
                let line = ((t + o) / grid.0 as f64).round() * grid.0 as f64 - o;
                Some((f, (t + s * (line - t)).round() as i64))
            })
            .collect();
        for (f, tick) in wanted {
            if let Some(i) = w.markers.iter().position(|m| (m.frame - f).abs() < 1.0) {
                w = w.move_marker(i, Ticks(tick));
            } else {
                // Its own marker: placed where it is heard, then moved to the target.
                let here = Ticks(w.tick_at(f).round() as i64);
                let Some(n) = w.insert(WarpMarker {
                    frame: f,
                    tick: here,
                }) else {
                    continue;
                };
                let i = n.markers.iter().position(|m| m.frame == f).unwrap_or(0);
                w = n.move_marker(i, Ticks(tick));
            }
        }
        w
    }

    /// The warp of an analysed source (auto-warp): markers on the first beat of every bar
    /// of `beats` (source frames, the first downbeat at `downbeat`), the downbeat on the
    /// first whole bar at or after the source's start, the tempo `bpm` outside them.
    pub fn from_grid(
        rate: u32,
        bpm: f64,
        beats: &[f64],
        downbeat: usize,
        beats_per_bar: u32,
        detected: Detected,
    ) -> Warp {
        let mut w = Warp::new(rate, bpm);
        w.detected = Some(detected);
        let bpb = beats_per_bar.max(1) as i64;
        let Some(&first) = beats.get(downbeat) else {
            return w;
        };
        // Whole bars of lead-in before the downbeat, so frame 0 sits at a tick ≥ 0.
        let per_beat = rate as f64 * 60.0 / bpm;
        let lead_beats = (first / per_beat).max(0.0);
        // (A downbeat within a quarter beat of the start — an attack found a few samples
        // in — has no lead-in.)
        let lead_bars = ((lead_beats - 0.25).max(0.0) / bpb as f64).ceil() as i64;
        let base = lead_bars * bpb * PPQ;
        for (k, &f) in beats.iter().enumerate().skip(downbeat) {
            let beat = (k - downbeat) as i64;
            if beat % bpb != 0 {
                continue;
            }
            if let Some(n) = w.insert(WarpMarker {
                frame: f,
                tick: Ticks(base + beat * PPQ),
            }) {
                w = n;
            }
        }
        // A marker at the source's start keeps the lead-in at the source's own tempo.
        // On a whole tick at or just after the start, at exactly the source's tempo.
        if let Some(m0) = w.markers.first().copied().filter(|m| m.frame > 0.0) {
            let fpt = frames_per_tick(rate, bpm);
            let t0 = (m0.tick.0 as f64 - m0.frame / fpt).ceil().max(0.0) as i64;
            if let Some(n) = w.insert(WarpMarker {
                frame: m0.frame - (m0.tick.0 - t0) as f64 * fpt,
                tick: Ticks(t0),
            }) {
                w = n;
            }
        }
        w
    }

    /// Every marker removed: the source plays at its own tempo from content tick 0.
    pub fn reset(&self) -> Warp {
        let mut w = self.clone();
        w.markers.clear();
        w
    }
}
