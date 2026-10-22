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

    /// The user's markers either side of marker `i` (the warp's own, [`WarpMarker::auto`],
    /// lie between and are carried along).
    fn user_neighbours(&self, i: usize) -> (Option<usize>, Option<usize>) {
        let m = &self.markers;
        let p = (0..i).rev().find(|&j| !m[j].auto);
        let q = (i + 1..m.len()).find(|&j| !m[j].auto);
        (p, q)
    }

    /// The ticks a marker at `i` may move between: its user neighbours', a tick apart from
    /// them and from each of the warp's own markers between (round 29's second review: the
    /// samples of a ramp never stop a drag).
    fn tick_room(&self, i: usize) -> (i64, i64) {
        let (p, q) = self.user_neighbours(i);
        let n = self.markers.len();
        let before = (i - p.map_or(0, |j| j + 1)) as i64;
        let after = (q.unwrap_or(n) - i - 1) as i64;
        let lo = p.map_or(i64::MIN / 4, |j| {
            self.markers[j].tick.0 + MIN_GAP.0 * (before + 1)
        });
        let hi = q.map_or(i64::MAX / 4, |j| {
            self.markers[j].tick.0 - MIN_GAP.0 * (after + 1)
        });
        (lo, hi)
    }

    /// The frames a marker at `i` may hold (strictly between its user neighbours').
    fn frame_room(&self, i: usize) -> (f64, f64) {
        let (p, q) = self.user_neighbours(i);
        let lo = p.map_or(f64::NEG_INFINITY, |j| self.markers[j].frame);
        let hi = q.map_or(f64::INFINITY, |j| self.markers[j].frame);
        (lo, hi)
    }

    /// A user's marker put in: the warp's own markers in its way (on its tick, or out of
    /// order with it) give way to it.
    fn insert_user(&self, m: WarpMarker) -> Option<Warp> {
        let mut w = self.clone();
        w.markers.retain(|k| {
            !k.auto
                || ((k.tick.0 - m.tick.0).abs() >= MIN_GAP.0
                    && (k.tick < m.tick) == (k.frame < m.frame))
        });
        w.insert(m)
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
        if let Some(i) = self.marker_at(tick).filter(|&i| self.markers[i].auto) {
            // One of the warp's own: now the user's.
            let mut w = self.clone();
            w.markers[i].auto = false;
            return Some(w);
        }
        self.insert_user(WarpMarker::at(self.frame_at(tick.0 as f64), tick))
    }

    /// A marker pairing source frame `frame` with content tick `tick` exactly; `None` where
    /// it would not keep the markers in order.
    pub fn with_marker(&self, frame: f64, tick: Ticks) -> Option<Warp> {
        self.insert_user(WarpMarker::at(frame, tick))
    }

    /// A marker on source frame `frame` (a transient), at the whole tick nearest where it
    /// is heard now.
    pub fn add_marker_at_frame(&self, frame: f64) -> Option<Warp> {
        self.insert_user(WarpMarker::at(
            frame,
            Ticks(self.tick_at(frame).round() as i64),
        ))
    }

    /// Marker `i` moved to content tick `tick` (clamped between its user neighbours): the
    /// audio on either side of it stretches. The warp's own markers between it and those
    /// neighbours keep their frames and their share of the distance (the tempo curve they
    /// sample keeps its shape); none is ever dropped, so every index stays.
    pub fn move_marker(&self, i: usize, tick: Ticks) -> Warp {
        let mut w = self.clone();
        let n = w.markers.len();
        if i >= n {
            return w;
        }
        w.markers[i].auto = false;
        let (lo, hi) = w.tick_room(i);
        let old = w.markers[i].tick.0;
        let new = tick.0.clamp(lo, hi.max(lo));
        if new == old {
            return w;
        }
        let (p, q) = w.user_neighbours(i);
        let scale = |t: i64, a: i64| {
            a + ((t - a) as f64 * (new - a) as f64 / (old - a) as f64).round() as i64
        };
        match p {
            Some(j) => {
                let a = w.markers[j].tick.0;
                for m in &mut w.markers[j + 1..i] {
                    m.tick = Ticks(scale(m.tick.0, a));
                }
            }
            None => w.markers[..i]
                .iter_mut()
                .for_each(|m| m.tick.0 += new - old),
        }
        let end = q.unwrap_or(n);
        match q {
            Some(j) => {
                let b = w.markers[j].tick.0;
                for m in &mut w.markers[i + 1..j] {
                    m.tick = Ticks(scale(m.tick.0, b));
                }
            }
            None => w.markers[i + 1..]
                .iter_mut()
                .for_each(|m| m.tick.0 += new - old),
        }
        w.markers[i].tick = Ticks(new);
        // A tick apart on both sides (the room above holds a tick for each): each run
        // pushed off its far end, then off the moved marker.
        let from = p.map_or(0, |j| j + 1);
        let ms = &mut w.markers;
        for j in from..i {
            if j > 0 {
                ms[j].tick.0 = ms[j].tick.0.max(ms[j - 1].tick.0 + MIN_GAP.0);
            }
        }
        for j in (from..i).rev() {
            ms[j].tick.0 = ms[j].tick.0.min(ms[j + 1].tick.0 - MIN_GAP.0);
        }
        for j in (i + 1..end).rev() {
            if j + 1 < n {
                ms[j].tick.0 = ms[j].tick.0.min(ms[j + 1].tick.0 - MIN_GAP.0);
            }
        }
        for j in i + 1..end {
            ms[j].tick.0 = ms[j].tick.0.max(ms[j - 1].tick.0 + MIN_GAP.0);
        }
        w
    }

    /// The source under marker `i` changed to frame `frame` (clamped strictly between its
    /// user neighbours' frames): the content slides under a marker that stays put, the
    /// warp's own markers either side carried along in proportion.
    pub fn move_content(&self, i: usize, frame: f64) -> Warp {
        let mut w = self.clone();
        if i >= w.markers.len() || !frame.is_finite() {
            return w;
        }
        let (lo, hi) = w.frame_room(i);
        let eps = 1e-3;
        let f = frame.max(lo + eps).min(hi - eps);
        if !(f > lo && f < hi) {
            return w;
        }
        let old = w.markers[i].frame;
        let (p, q) = w.user_neighbours(i);
        let scale = |x: f64, a: f64| a + (x - a) * (f - a) / (old - a);
        match p {
            Some(j) => {
                let a = w.markers[j].frame;
                for m in &mut w.markers[j + 1..i] {
                    m.frame = scale(m.frame, a);
                }
            }
            None => w.markers[..i].iter_mut().for_each(|m| m.frame += f - old),
        }
        match q {
            Some(j) => {
                let b = w.markers[j].frame;
                for m in &mut w.markers[i + 1..j] {
                    m.frame = scale(m.frame, b);
                }
            }
            None => w.markers[i + 1..]
                .iter_mut()
                .for_each(|m| m.frame += f - old),
        }
        w.markers[i].frame = f;
        w.markers[i].auto = false;
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
    /// timeline tick `origin` (`start − offset`), one metre throughout.
    pub fn quantise_from(
        &self,
        transients: &[f64],
        grid: Ticks,
        origin: Ticks,
        strength: f32,
        from: Ticks,
        to: Ticks,
    ) -> Warp {
        if grid.0 <= 0 {
            return self.clone();
        }
        let (g, o) = (grid.0 as f64, origin.0 as f64);
        let line = move |t: f64| ((t + o) / g).round() * g - o;
        self.quantise_to(transients, &line, strength, from, to)
    }

    /// Quantise to the lines `line` gives: the content tick of the grid line nearest a
    /// content tick (the interface counts it through the metres, `views/bars.rs`).
    pub fn quantise_to(
        &self,
        transients: &[f64],
        line: &dyn Fn(f64) -> f64,
        strength: f32,
        from: Ticks,
        to: Ticks,
    ) -> Warp {
        let mut w = self.clone();
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
                let l = line(t);
                Some((f, (t + s * (l - t)).round() as i64))
            })
            .collect();
        // The range's ends pinned where they are heard first: only the audio between
        // them stretches, never what follows the last pulled hit (round 29's review).
        if !wanted.is_empty() {
            // And a guard a span beyond each end where no marker is: the map goes on past
            // its last marker at the slope beside it, which must stay the old one.
            let span = Ticks((to - from).0.max(4 * crate::time::PPQ));
            let mut ends = vec![from, to];
            if w.markers.iter().all(|m| m.tick <= to) {
                ends.push(to + span);
            }
            if w.markers.iter().all(|m| m.tick >= from) && from - span >= Ticks::ZERO {
                ends.push(from - span);
            }
            for end in ends {
                if w.markers.iter().all(|m| m.tick != end || m.auto) {
                    let frame = w.frame_at(end.0 as f64);
                    if let Some(n) = w.insert_user(WarpMarker::at(frame, end)) {
                        w = n;
                    }
                }
            }
        }
        for (f, tick) in wanted {
            if let Some(i) = w.markers.iter().position(|m| (m.frame - f).abs() < 1.0) {
                w = w.move_marker(i, Ticks(tick));
            } else {
                // Its own marker: placed where it is heard, then moved to the target.
                let here = Ticks(w.tick_at(f).round() as i64);
                let Some(n) = w.insert_user(WarpMarker::at(f, here)) else {
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
            if let Some(n) = w.insert(WarpMarker::at(f, Ticks(base + beat * PPQ))) {
                w = n;
            }
        }
        // A marker at the source's start keeps the lead-in at the source's own tempo.
        // On a whole tick at or just after the start, at exactly the source's tempo.
        if let Some(m0) = w.markers.first().copied().filter(|m| m.frame > 0.0) {
            let fpt = frames_per_tick(rate, bpm);
            let t0 = (m0.tick.0 as f64 - m0.frame / fpt).ceil().max(0.0) as i64;
            if let Some(n) = w.insert(WarpMarker::at(
                m0.frame - (m0.tick.0 - t0) as f64 * fpt,
                Ticks(t0),
            )) {
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

impl Warp {
    /// The warp that plays a clip exactly as it plays unwarped (ADR 0060: natural speed
    /// from its content origin `start − offset`, through the tempo map), following the
    /// tempo: the user's markers on the content origin and the window's two ends, and the
    /// warp's own ([`WarpMarker::auto`]: never drawn or grabbed, soft for the stretcher) on
    /// every tempo point between and, inside a ramp, as close as keeps the straight line
    /// between two within a quarter of a frame of the curve (round 29's second review: a
    /// marker every sixteenth missed a steep ramp by milliseconds, and drew a barcode of
    /// handles nobody placed) — each on the source frame the unwarped rule reads there
    /// (`audio_time::frame_at`). What a first warp edit starts from, so nothing moves until a
    /// marker is dragged. Under one tempo it is the identity.
    pub fn natural_of(
        map: &crate::tempo::TempoMap,
        start: Ticks,
        offset: Ticks,
        length: Ticks,
        slip_frames: i32,
        rate: u32,
    ) -> Warp {
        let origin = start - offset;
        let end = start + length;
        let rate_f = rate.max(1) as f64;
        // (tick, the warp's own)
        let mut ticks: Vec<(i64, bool)> = vec![(origin.0, false), (start.0, false), (end.0, false)];
        let (lo, hi) = (origin.0.min(start.0), end.0.max(origin.0));
        for p in map.points() {
            if p.tick.0 > lo && p.tick.0 < hi {
                ticks.push((p.tick.0, true));
            }
        }
        let cap = super::MAX_MARKERS / 2;
        let mut t = lo as f64;
        while t < hi as f64 && ticks.len() < cap {
            let next = map.next_change_after(t).unwrap_or(f64::INFINITY);
            if !map.in_ramp(t) {
                t = next;
                continue;
            }
            let to = next.min(hi as f64);
            let h = ramp_step(map, t, to, rate_f);
            let n = (t + h).floor().max(t.floor() + 1.0);
            if n >= to {
                t = to;
                continue;
            }
            ticks.push((n as i64, true));
            t = n;
        }
        // In order; on one tick the user's marker stays.
        ticks.sort_unstable();
        ticks.dedup_by_key(|x| x.0);
        let mut w = Warp::new(rate.max(1), map.bpm_at(start.0 as f64));
        for (t, auto) in ticks {
            let frame = crate::audio_time::frame_at(
                map,
                start,
                offset,
                slip_frames as f64,
                rate_f,
                t as f64,
            );
            let m = WarpMarker {
                frame,
                tick: Ticks(t - origin.0),
                auto,
            };
            if let Some(n) = w.insert(m) {
                w = n;
            }
        }
        w
    }
}

/// How many ticks a ramp's straight line may span from `t` (before `to`, where the ramp
/// ends) and stay within a quarter of a frame of the curve: the chord of a curve `f` over
/// `h` misses it by at most `h² · max|f''| / 8`, and the unwarped frame's `f''` inside a
/// ramp of slope `k` (BPM a tick) is `rate · 60 · |k| / (PPQ · bpm²)` — largest where the
/// tempo is lowest.
fn ramp_step(map: &crate::tempo::TempoMap, t: f64, to: f64, rate: f64) -> f64 {
    const MISS: f64 = 0.25;
    let k = map.slope_at(t).abs();
    if k == 0.0 {
        return (to - t).max(1.0);
    }
    let bend = |bpm: f64| rate * 60.0 * k / (PPQ as f64 * bpm * bpm).max(1e-12);
    let step = |bpm: f64| (8.0 * MISS / bend(bpm)).sqrt();
    let mut lowest = map.bpm_at(t);
    let mut h = step(lowest);
    // A slowing ramp is slowest at the far end: twice, so the step's own end is counted.
    for _ in 0..2 {
        let far = map.bpm_at((t + h).min(to));
        lowest = lowest.min(far);
        h = step(lowest);
    }
    h.max(1.0)
}
