//! MIDI clock (ADR 0079 §1–§2): the messages, and following another device's clock.
//!
//! **The messages** (MIDI 1.0, *System Real Time* and *System Common*): 24 clocks per
//! quarter note (`F8`); Start (`FA`, from the song's beginning), Continue (`FB`, from the
//! song position pointer), Stop (`FC`); the Song Position Pointer (`F2 lsb msb`, in
//! sixteenth notes — six clocks — a 14-bit number). After Start or Continue the **next**
//! clock is the first beat played.
//!
//! **Following** (the slave). A master's clocks arrive with the jitter of its timer, the
//! cable and our driver (±1–3 ms is common). The follower filters their arrival times with
//! a **third-order tracking loop** — the fading-memory α-β-γ filter (Brookner, *Tracking
//! and Kalman Filtering Made Easy*, 1998, ch. 1; the discrete form of a type-3 phase-locked
//! loop): it keeps the next clock's time, the period and the period's change per clock,
//! and moves each by `α`, `β`, `2γ` times a clock's error, with `α = 1 − θ³`,
//! `β = 1.5(1 − θ)²(1 + θ)`, `γ = ½(1 − θ)³` from one memory constant `θ`. A second-order
//! loop (F. Adriaensen, *Using a DLL to filter time*, LAC 2005 — [`Dll`], which the time
//! code chaser uses) lags a tempo ramp by a constant error of milliseconds; the third term
//! follows a ramp with none. `θ` starts short (a lock in a beat) and lengthens to
//! [`THETA`] once locked.

use super::{SourceKind, Timeline};

pub const CLOCK: u8 = 0xF8;
pub const START: u8 = 0xFA;
pub const CONTINUE: u8 = 0xFB;
pub const STOP: u8 = 0xFC;
pub const SPP: u8 = 0xF2;
/// Clocks per quarter note.
pub const PER_BEAT: f64 = 24.0;

/// The song position pointer for a position in beats (whole sixteenths, as the message
/// carries; the 14 bits' range clamped).
pub fn spp(beats: f64) -> [u8; 3] {
    let v = (beats * 4.0).floor().clamp(0.0, 16_383.0) as u16;
    [SPP, (v & 0x7F) as u8, (v >> 7) as u8]
}

/// The clock tracker's memory once locked (θ: the closer to 1, the longer its memory and
/// the smoother, the slower to follow a change).
pub const THETA: f64 = 0.90;
/// While acquiring (the first beat's worth of clocks).
pub const THETA_ACQUIRE: f64 = 0.75;

/// A fading-memory α-β-γ tracker on event times (µs), one step per event.
#[derive(Copy, Clone, Debug)]
pub struct Tracker {
    /// The predicted time of the next event, the period, the period's change per event.
    pub next: f64,
    pub period: f64,
    pub accel: f64,
    events: u32,
}

impl Tracker {
    pub fn new(t: f64, period: f64) -> Tracker {
        Tracker {
            next: t + period,
            period,
            accel: 0.0,
            events: 1,
        }
    }

    /// Resume at `t` with a period already known (after a gap): no acquisition — the
    /// period is the master's, only its phase starts again.
    pub fn resume(t: f64, period: f64) -> Tracker {
        Tracker {
            events: 24,
            ..Tracker::new(t, period)
        }
    }

    /// An event at `t`: its filtered time.
    pub fn feed(&mut self, t: f64, theta: f64) -> f64 {
        let a = 1.0 - theta * theta * theta;
        let b = 1.5 * (1.0 - theta) * (1.0 - theta) * (1.0 + theta);
        let g = 0.5 * (1.0 - theta) * (1.0 - theta) * (1.0 - theta);
        let r = t - self.next;
        let x = self.next + a * r;
        self.period += b * r;
        self.accel += 2.0 * g * r;
        // The next: one period on, the period changing as it has.
        self.next = x + self.period + 0.5 * self.accel;
        self.period += self.accel;
        self.events = self.events.saturating_add(1);
        x
    }

    pub fn events(&self) -> u32 {
        self.events
    }
}

/// A second-order delay-locked loop on event times (µs).
#[derive(Copy, Clone, Debug)]
pub struct Dll {
    /// The predicted time of the next event, and the period.
    pub next: f64,
    pub period: f64,
    events: u32,
}

impl Dll {
    /// Start from an event at `t` with a period guessed from the last interval.
    pub fn new(t: f64, period: f64) -> Dll {
        Dll {
            next: t + period,
            period,
            events: 1,
        }
    }

    /// The loop's coefficients at bandwidth `hz`.
    fn coeffs(&self, hz: f64) -> (f64, f64) {
        let w = 2.0 * std::f64::consts::PI * hz * self.period * 1e-6;
        let w = w.min(0.9);
        (std::f64::consts::SQRT_2 * w, w * w)
    }

    /// An event at `t`: returns its filtered time.
    pub fn feed(&mut self, t: f64, hz: f64) -> f64 {
        let (b, c) = self.coeffs(hz);
        let e = t - self.next;
        let filtered = self.next + b * e;
        self.next += b * e + self.period;
        self.period += c * e;
        self.events = self.events.saturating_add(1);
        filtered
    }

    pub fn events(&self) -> u32 {
        self.events
    }
}

/// A gap this many periods long means the clock stopped and started again.
const GAP_PERIODS: f64 = 4.0;
/// While playing, this long without a clock is a stop (µs).
const DROPOUT_US: i64 = 400_000;

/// Following a MIDI clock.
#[derive(Clone, Debug, Default)]
pub struct ClockFollower {
    dll: Option<Tracker>,
    last: Option<i64>,
    /// The beat of the newest clock and its filtered time.
    beat: f64,
    at: f64,
    playing: bool,
    /// After Start or Continue: the beat the next clock is.
    pending: Option<f64>,
    /// The song position pointer's place, in beats.
    spp: f64,
    epoch: u32,
    /// The tracker's memory now (short while acquiring).
    pub theta: Option<f64>,
    /// Clocks counted for a gap by its length that may still arrive late, bunched (a
    /// driver that held them): each that does is already counted.
    credit: f64,
}

impl ClockFollower {
    pub fn new() -> Self {
        Self::default()
    }

    /// A message from the master's port, arrived at host time `host` (µs).
    pub fn feed(&mut self, host: i64, bytes: &[u8]) {
        let Some(&status) = bytes.first() else { return };
        if matches!(status, START | CONTINUE | STOP | SPP) {
            self.credit = 0.0;
        }
        match status {
            CLOCK => self.clock(host),
            START => {
                self.pending = Some(0.0);
                self.spp = 0.0;
            }
            CONTINUE => self.pending = Some(self.spp),
            STOP => {
                if self.playing {
                    // Where the next clock would have been.
                    self.spp = self.beat + 1.0 / PER_BEAT;
                }
                self.playing = false;
                self.pending = None;
            }
            SPP if bytes.len() >= 3 => {
                let v = u16::from(bytes[1] & 0x7F) | u16::from(bytes[2] & 0x7F) << 7;
                self.spp = f64::from(v) / 4.0;
                if self.playing {
                    // A locate while running: the next clock is the new place.
                    self.pending = Some(self.spp);
                    self.playing = false;
                } else {
                    self.beat = self.spp;
                }
                self.epoch = self.epoch.wrapping_add(1);
            }
            _ => {}
        }
    }

    fn clock(&mut self, host: i64) {
        let t = host as f64;
        if self.credit > 0.0 {
            // A clock held back with the gap, arriving bunched: already counted.
            let bunched = match (self.last, self.dll) {
                (Some(l), Some(d)) => ((host - l) as f64) < 0.5 * d.period,
                _ => false,
            };
            if bunched {
                self.credit -= 1.0;
                self.last = Some(host);
                return;
            }
            self.credit = 0.0;
        }
        let gap = match (self.last, self.dll) {
            (Some(l), Some(d)) => (host - l) as f64 > GAP_PERIODS * d.period,
            _ => false,
        };
        // Clocks lost in a gap while the master plays, counted by the time they took.
        let mut lost = 0.0;
        if gap {
            // A silence (a stop, a locate, a cable, a driver): the old loop's prediction is
            // no longer the master's, its period still is — resume with it (round 2, D2:
            // reseeded from one jittered interval, the tempo was a tenth off for a while,
            // and after every locate); kept whole, a 2 s gap read 32 BPM (review r33 #1).
            if let (Some(l), Some(d)) = (self.last, self.dll) {
                if self.playing && self.pending.is_none() {
                    lost = ((host - l) as f64 / d.period).round().max(1.0) - 1.0;
                    self.credit = lost;
                }
                self.dll = Some(Tracker::resume(t, d.period));
            }
        }
        let filtered = match (self.last, self.dll.as_mut()) {
            (Some(_), _) if gap => t,
            (Some(_), Some(d)) => {
                let th = if d.events() < 24 {
                    THETA_ACQUIRE
                } else {
                    THETA
                };
                self.theta = Some(th);
                d.feed(t, th)
            }
            (Some(l), _) => {
                let period = (host - l) as f64;
                if period > 0.0 && period < 125_000.0 {
                    self.dll = Some(Tracker::new(t, period));
                }
                t
            }
            (None, _) => t,
        };
        self.last = Some(host);
        if let Some(b) = self.pending.take() {
            self.playing = true;
            self.beat = b;
            self.at = filtered;
            self.epoch = self.epoch.wrapping_add(1);
        } else if self.playing {
            self.beat += (1.0 + lost) / PER_BEAT;
            self.at = filtered;
        }
    }

    /// The tempo the clocks say, BPM (0 before two clocks).
    pub fn tempo(&self) -> f64 {
        self.dll.map_or(0.0, |d| 60e6 / (PER_BEAT * d.period))
    }

    /// Whether the master plays, as of host time `now` (a clock silent too long is a stop).
    pub fn playing(&self, now: i64) -> bool {
        self.playing && self.last.is_some_and(|l| now - l < DROPOUT_US)
    }

    /// The timeline to publish at `now`.
    pub fn timeline(&self, now: i64) -> Timeline {
        let playing = self.playing(now);
        Timeline {
            kind: SourceKind::Clock,
            tempo: self.tempo(),
            host: if playing { self.at.round() as i64 } else { now },
            position: self.beat,
            rate: 1.0,
            playing,
            quantum: 0.0,
            start_stop: true,
            epoch: self.epoch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_song_position_pointer_counts_sixteenths() {
        assert_eq!(spp(0.0), [0xF2, 0, 0]);
        assert_eq!(spp(8.0), [0xF2, 32, 0]);
        // 1000 sixteenths: 1000 = 7·128 + 104.
        assert_eq!(spp(250.0), [0xF2, 104, 7]);
        let mut f = ClockFollower::new();
        f.feed(0, &spp(250.0));
        assert_eq!(f.timeline(0).position, 250.0);
    }

    /// Start, then the first clock is beat 0; Stop holds the beat; Continue resumes there.
    #[test]
    fn start_stop_and_continue() {
        let mut f = ClockFollower::new();
        let p = 20_833i64; // 120 BPM
        f.feed(0, &[START]);
        for k in 0..48 {
            f.feed(k * p, &[CLOCK]);
        }
        let t = f.timeline(47 * p);
        assert!(t.playing);
        assert!((t.position - 47.0 / 24.0).abs() < 1e-9, "{}", t.position);
        f.feed(48 * p, &[STOP]);
        assert!(!f.timeline(48 * p).playing);
        f.feed(49 * p, &[CONTINUE]);
        f.feed(50 * p, &[CLOCK]);
        let t = f.timeline(50 * p);
        assert!(
            t.playing && (t.position - 2.0).abs() < 1e-9,
            "{}",
            t.position
        );
        // Silence is a stop.
        assert!(!f.playing(50 * p + 500_000));
    }

    /// The true beat at `t` of a 120 BPM master that started at `t0` (µs).
    fn beat_120(t0: f64, t: f64) -> f64 {
        (t - t0) / 500_000.0
    }

    /// Review r33 #1: a master stopped for 2 s (no clocks while stopped), then Start: the
    /// follower drops its old loop and acquires again — the tempo within 1 % and the place
    /// within a millisecond a beat later (it read ~32 BPM, the gap taken for a period).
    #[test]
    fn a_long_silence_then_start_acquires_again() {
        let p = 1e6 / 48.0; // 120 BPM: 48 clocks a second
        let mut f = ClockFollower::new();
        f.feed(0, &[START]);
        for k in 0..48 {
            f.feed((k as f64 * p) as i64, &[CLOCK]);
        }
        f.feed((48.0 * p) as i64, &[STOP]);
        let t0 = 48.0 * p + 2_000_000.0;
        f.feed(t0 as i64 - 1_000, &[START]);
        // A beat after the Start: 24 clocks.
        for k in 0..25 {
            f.feed((t0 + k as f64 * p) as i64, &[CLOCK]);
        }
        let t = t0 + 24.0 * p + 5_000.0;
        let tl = f.timeline(t as i64);
        assert!(
            (f.tempo() / 120.0 - 1.0).abs() < 0.01,
            "tempo {}",
            f.tempo()
        );
        let err_ms = (tl.beat_at(t as i64) - beat_120(t0, t)) * 500.0;
        assert!(err_ms.abs() < 1.0, "{err_ms} ms");
    }

    /// Review r33 #1: a 100 ms hiccup. Five clocks lost (a cable, a driver): the tempo is
    /// back within 1 % a sixth of a beat later (it read 25 BPM, the hiccup taken for a
    /// period). Five clocks held back and delivered at once: the same, and the place within
    /// a millisecond a beat later (no clock is lost, so none is missing from the count).
    #[test]
    fn a_hiccup_does_not_become_the_tempo() {
        let p = 1e6 / 48.0;
        for held in [false, true] {
            let mut f = ClockFollower::new();
            f.feed(0, &[START]);
            for k in 0..129 {
                let mut at = k as f64 * p;
                if (100..105).contains(&k) {
                    if !held {
                        continue;
                    }
                    at = 104.0 * p + 200.0;
                }
                f.feed(at as i64, &[CLOCK]);
                if k == 108 {
                    let t = f.tempo();
                    assert!((t / 120.0 - 1.0).abs() < 0.01, "held {held}, at 108: {t}");
                }
            }
            if held {
                let t = 128.0 * p + 3_000.0;
                let err_ms = (f.timeline(t as i64).beat_at(t as i64) - beat_120(0.0, t)) * 500.0;
                assert!(err_ms.abs() < 1.0, "{err_ms} ms");
            }
        }
    }

    /// A jitter of up to ±`j` µs, the same every run.
    fn jitter(k: u64, j: f64) -> f64 {
        let mut x = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        x ^= x >> 31;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 29;
        ((x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0) * j
    }

    /// Round 2, D2: a hiccup (five clocks lost) in a clock jittering ±1.5 ms keeps the
    /// tempo — within 2 % at every clock after it (reseeded from one jittered interval it
    /// was off by up to a tenth) — and the place: the lost clocks are counted by the time
    /// they took, so the follower is on the master's beat at once (it was five clocks
    /// behind, a 104 ms jump for the engine).
    #[test]
    fn a_hiccup_in_a_jittery_clock_keeps_tempo_and_place() {
        let p = 1e6 / 48.0;
        let mut f = ClockFollower::new();
        f.feed(0, &[START]);
        for k in 0..300u64 {
            if (150..155).contains(&k) {
                continue;
            }
            let at = 1_000.0 + k as f64 * p + jitter(k, 1_500.0);
            f.feed(at as i64, &[CLOCK]);
            if k > 60 {
                let t = f.tempo();
                assert!((t / 120.0 - 1.0).abs() < 0.02, "clock {k}: {t} BPM");
            }
            if k == 155 {
                // Right after the gap: the beat of clock 155, not of clock 150.
                let b = f.timeline(at as i64).position;
                assert!((b - 155.0 / 24.0).abs() < 1e-9, "beat {b} after the gap");
            }
        }
    }
}
