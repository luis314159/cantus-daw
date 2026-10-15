//! Tap tempo (ADR 0052): the tempo is the average of the last taps' intervals; a pause
//! starts a new run; each run is one undo step (the tempo it ends on is what is committed —
//! the taps between merge into it).

/// The most taps averaged: the last eight, seven intervals.
const TAPS: usize = 8;
/// A pause this long (seconds) ends a run whatever the tempo (20 BPM is 3 s a beat).
const PAUSE: f64 = 3.0;
/// …and so does a gap this many times the run's average interval.
const PAUSE_RATIO: f64 = 2.0;

/// A run of taps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TapTempo {
    /// Tap times in seconds, oldest first; the first `len` are used.
    times: [f64; TAPS],
    len: usize,
}

impl TapTempo {
    /// A tap at `now` (seconds, any origin that only moves forward). Returns the tempo the
    /// run gives now — `None` for the first tap of a run — rounded to 0.1 BPM and kept
    /// between 20 and 999. `started` says this tap began a new run.
    pub fn tap(&mut self, now: f64) -> Tap {
        let started = self.ended(now);
        if started {
            self.len = 0;
        }
        if self.len == TAPS {
            self.times.copy_within(1.., 0);
            self.len -= 1;
        }
        self.times[self.len] = now;
        self.len += 1;
        Tap {
            bpm: self.bpm(),
            started,
        }
    }

    /// Whether the run has ended by `now` (no taps, or a pause since the last).
    pub fn ended(&self, now: f64) -> bool {
        if self.len == 0 {
            return true;
        }
        let gap = now - self.times[self.len - 1];
        let limit = match self.mean_interval() {
            Some(m) => (m * PAUSE_RATIO).min(PAUSE),
            None => PAUSE,
        };
        !(0.0..=limit).contains(&gap)
    }

    /// Taps in the current run.
    pub fn taps(&self) -> usize {
        self.len
    }

    /// Forget the run.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    fn mean_interval(&self) -> Option<f64> {
        (self.len >= 2)
            .then(|| (self.times[self.len - 1] - self.times[0]) / (self.len - 1) as f64)
            .filter(|m| *m > 0.0)
    }

    /// The run's tempo: 60 over the mean interval.
    pub fn bpm(&self) -> Option<f64> {
        let m = self.mean_interval()?;
        Some(((60.0 / m) * 10.0).round().clamp(200.0, 9_990.0) / 10.0)
    }
}

/// What one tap gave.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tap {
    pub bpm: Option<f64>,
    pub started: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_taps_give_their_tempo() {
        let mut t = TapTempo::default();
        assert_eq!(t.tap(10.0).bpm, None, "one tap is no tempo");
        assert_eq!(t.tap(10.5).bpm, Some(120.0));
        assert_eq!(t.tap(11.0).bpm, Some(120.0));
        // Uneven taps are averaged: 0.5, 0.6, 0.4 → 0.5 s → 120.
        let mut t = TapTempo::default();
        for at in [0.0, 0.5, 1.1, 1.5] {
            t.tap(at);
        }
        assert_eq!(t.bpm(), Some(120.0));
    }

    #[test]
    fn only_the_last_eight_taps_count() {
        let mut t = TapTempo::default();
        // Eight taps at 100 BPM (0.6 s), then eight at 150 (0.4 s).
        let mut at = 0.0;
        for _ in 0..8 {
            t.tap(at);
            at += 0.6;
        }
        at -= 0.6;
        for _ in 0..8 {
            at += 0.4;
            t.tap(at);
        }
        assert_eq!(t.taps(), 8);
        assert_eq!(t.bpm(), Some(150.0));
    }

    #[test]
    fn a_pause_starts_a_new_run() {
        let mut t = TapTempo::default();
        assert!(t.tap(0.0).started);
        assert!(!t.tap(0.5).started);
        // Twice the run's interval later: a new run, no tempo from the old taps.
        assert!(!t.ended(1.4));
        assert!(t.ended(1.6));
        let tap = t.tap(1.6);
        assert!(tap.started);
        assert_eq!(tap.bpm, None);
        // However slow, three seconds ends it.
        let mut slow = TapTempo::default();
        slow.tap(0.0);
        assert!(slow.ended(3.01));
    }

    #[test]
    fn the_tempo_stays_in_the_projects_range() {
        let mut t = TapTempo::default();
        t.tap(0.0);
        assert_eq!(t.tap(0.01).bpm, Some(999.0));
        let mut t = TapTempo::default();
        t.tap(0.0);
        assert_eq!(t.tap(2.9).bpm, Some(20.7));
    }
}
