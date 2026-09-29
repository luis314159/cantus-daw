//! What the desk's meters show: FL's ballistics (instant rise, a constant fall in dB), a
//! peak hold, and the meter-wave view's history — per strip, in flat buffers that never
//! allocate once they have grown.
//!
//! Time is egui's frame time, not the wall clock: the same frames show the same meters,
//! which is what lets a test drive three seconds of fall in a few milliseconds.

use crate::kit::meter::{Levels, MIN_DB};

/// How long a peak is held before it falls to the level.
pub const HOLD_SECONDS: f64 = 1.2;
/// How fast a meter falls, in dB per second. Rise is instant, as on a digital peak meter;
/// the fall is the ballistics — 24 dB/s reads as motion rather than as flicker, and lets
/// a transient be *seen*, which a block-by-block reading does not.
pub const FALL_DB_PER_S: f32 = 24.0;
/// How many frames of level the meter-wave view keeps.
pub const WAVE_LEN: usize = 96;
/// Below this a meter shows nothing: silence is its own value, not `linear_to_db`'s
/// −96 dB floor, or a meter would never finish falling.
const SILENT: f32 = -120.0;

/// One channel's ballistics.
#[derive(Copy, Clone, Debug)]
struct Ballistic {
    db: f32,
    at: f64,
}

impl Default for Ballistic {
    fn default() -> Self {
        Ballistic {
            db: SILENT,
            at: 0.0,
        }
    }
}

impl Ballistic {
    /// The level shown after a block peaking at `peak` (linear), at time `now`.
    fn update(&mut self, peak: f32, now: f64) -> f32 {
        let dt = (now - self.at).max(0.0) as f32;
        let fallen = self.db - FALL_DB_PER_S * dt;
        let incoming = if peak > 0.0 {
            20.0 * peak.log10()
        } else {
            SILENT
        };
        self.db = incoming.max(fallen).max(SILENT);
        self.at = now;
        self.db
    }
}

/// A peak hold: the highest level of the last [`HOLD_SECONDS`].
#[derive(Copy, Clone, Debug)]
struct Hold {
    db: f32,
    at: f64,
}

impl Default for Hold {
    fn default() -> Self {
        Hold {
            db: SILENT,
            at: 0.0,
        }
    }
}

impl Hold {
    fn update(&mut self, db: f32, now: f64) -> f32 {
        if db >= self.db || now - self.at > HOLD_SECONDS {
            self.db = db;
            self.at = now;
        }
        self.db
    }

    /// Still showing something that will change without new audio.
    fn pending(&self, now: f64) -> bool {
        self.db > MIN_DB && now - self.at <= HOLD_SECONDS
    }
}

/// The recent levels of one strip, newest last, in a ring.
#[derive(Clone, Debug)]
pub struct Wave {
    buf: [f32; WAVE_LEN],
    head: usize,
    len: usize,
}

impl Default for Wave {
    fn default() -> Self {
        Wave {
            buf: [SILENT; WAVE_LEN],
            head: 0,
            len: 0,
        }
    }
}

impl Wave {
    fn push(&mut self, db: f32) {
        self.buf[self.head] = db;
        self.head = (self.head + 1) % WAVE_LEN;
        self.len = (self.len + 1).min(WAVE_LEN);
    }

    /// How many levels it holds.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The `i`-th level from the oldest, in dB.
    pub fn get(&self, i: usize) -> f32 {
        let start = (self.head + WAVE_LEN - self.len) % WAVE_LEN;
        self.buf[(start + i) % WAVE_LEN]
    }
}

/// One strip's meter state (the master uses two channels, a strip one).
#[derive(Clone, Debug, Default)]
struct Channel {
    level: [Ballistic; 2],
    hold: [Hold; 2],
    wave: Wave,
}

/// Every strip's meter, and the master's.
#[derive(Debug, Default)]
pub struct Meters {
    strips: Vec<Channel>,
    master: Channel,
    /// Something shown this frame is still moving without new audio.
    moving: bool,
}

impl Meters {
    /// Call once a frame before any meter is read.
    pub fn begin_frame(&mut self) {
        self.moving = false;
    }

    /// Whether a meter read this frame is still falling or holding: the desk wants the
    /// next frame (at 60 Hz at most) even with the transport stopped.
    pub fn moving(&self) -> bool {
        self.moving
    }

    fn slot(&mut self, index: usize) -> &mut Channel {
        if self.strips.len() <= index {
            self.strips.resize_with(index + 1, Channel::default);
        }
        &mut self.strips[index]
    }

    /// A strip's levels after ballistics: `peak` is the engine's linear block peak (one
    /// value: a strip is metered after its fader, in mono). Pushes the wave when `wave`.
    pub fn strip(&mut self, index: usize, peak: f32, now: f64, wave: bool) -> Levels {
        let ch = self.slot(index);
        let db = ch.level[0].update(peak, now);
        let hold = ch.hold[0].update(db, now);
        if wave {
            ch.wave.push(db);
        }
        let moving = db > MIN_DB || ch.hold[0].pending(now);
        self.moving |= moving;
        Levels {
            level: [db.max(MIN_DB); 2],
            hold: [hold.max(MIN_DB); 2],
        }
    }

    /// The master's levels, left and right.
    pub fn master(&mut self, peak: [f32; 2], now: f64, wave: bool) -> Levels {
        let ch = &mut self.master;
        let mut out = Levels::SILENT;
        for (c, p) in peak.into_iter().enumerate() {
            let db = ch.level[c].update(p, now);
            let hold = ch.hold[c].update(db, now);
            out.level[c] = db.max(MIN_DB);
            out.hold[c] = hold.max(MIN_DB);
            self.moving |= db > MIN_DB || ch.hold[c].pending(now);
        }
        if wave {
            ch.wave.push(out.level[0].max(out.level[1]));
        }
        out
    }

    /// The held peak of a strip, in dB (for its readout).
    pub fn held(&self, index: usize) -> f32 {
        self.strips.get(index).map_or(SILENT, |c| c.hold[0].db)
    }

    pub fn master_held(&self) -> f32 {
        self.master.hold[0].db.max(self.master.hold[1].db)
    }

    /// Forget a strip's held peak (a click on its readout, as in FL).
    pub fn reset_hold(&mut self, index: Option<usize>) {
        let ch = match index {
            Some(i) => self.slot(i),
            None => &mut self.master,
        };
        ch.hold = [Hold::default(); 2];
    }

    pub fn wave(&self, index: usize) -> Option<&Wave> {
        self.strips.get(index).map(|c| &c.wave)
    }

    pub fn master_wave(&self) -> &Wave {
        &self.master.wave
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Instant rise, a constant fall in dB: a 0 dBFS hit reads −12 dB half a second
    /// later, whatever the frame rate, and lands at silence.
    #[test]
    fn meters_rise_at_once_and_fall_at_a_constant_rate() {
        let mut m = Meters::default();
        assert_eq!(m.strip(0, 1.0, 0.0, false).level[0], 0.0);
        let half = m.strip(0, 0.0, 0.5, false).level[0];
        assert!((half + 12.0).abs() < 0.01, "{half}");
        // In ten small steps or one big one, the same place.
        let mut w = Meters::default();
        w.strip(0, 1.0, 0.0, false);
        let mut last = 0.0;
        for i in 1..=10 {
            last = w.strip(0, 0.0, 0.05 * i as f64, false).level[0];
        }
        assert!((last - half).abs() < 0.01);
        // A louder block in the fall takes over at once.
        let db = m.strip(0, 0.9, 0.6, false).level[0];
        assert!((db - 20.0 * 0.9f32.log10()).abs() < 1e-4);
        m.begin_frame();
        m.strip(0, 0.0, 0.7, false);
        assert!(m.moving());
        m.begin_frame();
        assert_eq!(m.strip(0, 0.0, 10.0, false).level[0], MIN_DB);
        assert!(!m.moving(), "a meter at silence has landed");
    }

    #[test]
    fn a_peak_is_held_then_falls_to_the_level() {
        let mut m = Meters::default();
        m.strip(0, 1.0, 0.0, false);
        let l = m.strip(0, 0.1, 0.5, false);
        assert_eq!(l.hold[0], 0.0, "held for {HOLD_SECONDS} s");
        let l = m.strip(0, 0.1, 2.0, false);
        assert!(
            (l.hold[0] + 20.0).abs() < 0.01,
            "then it follows: {}",
            l.hold[0]
        );
        m.reset_hold(Some(0));
        assert_eq!(m.held(0), SILENT);
    }

    #[test]
    fn the_wave_keeps_the_last_levels_in_order() {
        let mut m = Meters::default();
        for i in 0..(WAVE_LEN + 10) {
            m.strip(0, if i % 2 == 0 { 1.0 } else { 0.0 }, i as f64, true);
        }
        let w = m.wave(0).unwrap();
        assert_eq!(w.len(), WAVE_LEN);
        // The newest is the last pushed (odd: silent after a 1-s fall of 24 dB).
        assert!(w.get(WAVE_LEN - 1) < w.get(WAVE_LEN - 2));
    }
}
