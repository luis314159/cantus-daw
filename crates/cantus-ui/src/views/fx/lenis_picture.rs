//! Lenis's picture (design sheet v1): the 128 display points of the input's spectrum and
//! of the cut the engine sends, held whole between frames. The engine sends them in five
//! interleaved pages of 26 (`cantus_engine::fx::lenis::t`); each page refreshes its points
//! as it arrives, so the picture is never missing a point — an older page leaves a value a
//! few frames older. When the frames stop (the transport stopped, the unit at rest) the
//! picture settles: the spectrum sinks to the floor and the cut lets go, so the screen
//! comes to a still line, never a frozen frame.

use cantus_dsp::lenis::DISPLAY;
use cantus_engine::fx::lenis::{page_point, t};

/// The floor the screen draws the spectrum down to, dBFS.
pub const FLOOR_DB: f32 = -100.0;

/// How long without a new frame before the picture settles, seconds.
const STILL_AFTER: f32 = 0.25;

/// What the editor holds of the engine's numbers.
#[derive(Clone, Debug)]
pub struct Picture {
    /// The input's spectrum, dBFS, per display point.
    pub level: [f32; DISPLAY],
    /// The cut, dB ≥ 0, per display point.
    pub cut: [f32; DISPLAY],
    /// The most any bin is cut now, dB, and the readout's held value (held a second, then
    /// falling at 12 dB a second, as a peak meter does).
    pub most: f32,
    pub held: f32,
    hold: f32,
    /// The latency reported, samples; the quality in force; the sample rate.
    pub latency: f32,
    pub quality: usize,
    pub sample_rate: f32,
    /// The input's and the output's peaks in the last block, linear.
    pub in_peak: f32,
    pub out_peak: f32,
    /// Whether a signal's picture is on the screen (anything over the floor).
    pub seen: bool,
    /// The frame counter last seen, and how long since it moved.
    frames: f32,
    idle: f32,
    /// The clock (seconds of updates) and when each point was last refreshed: the line
    /// falls from a peak at [`FALL_DB_S`], as an analyser's does, and rises at once.
    clock: f32,
    stamp: [f32; DISPLAY],
}

/// How fast the spectrum's line falls, dB a second.
pub const FALL_DB_S: f32 = 48.0;

impl Default for Picture {
    fn default() -> Self {
        Picture {
            level: [FLOOR_DB; DISPLAY],
            cut: [0.0; DISPLAY],
            most: 0.0,
            held: 0.0,
            hold: 0.0,
            latency: 0.0,
            quality: 1,
            sample_rate: 0.0,
            in_peak: 0.0,
            out_peak: 0.0,
            seen: false,
            frames: -1.0,
            idle: 0.0,
            clock: 0.0,
            stamp: [0.0; DISPLAY],
        }
    }
}

impl Picture {
    /// Take this frame's telemetry (`dt` seconds since the last call).
    pub fn update(&mut self, tel: &[f32; 64], dt: f32) {
        self.clock += dt;
        let live = tel[t::SAMPLE_RATE] > 0.0;
        let moved = live && tel[t::FRAMES] != self.frames;
        if moved {
            self.frames = tel[t::FRAMES];
            self.idle = 0.0;
            let page = (tel[t::PAGE].max(0.0) as usize).min(t::PAGES - 1);
            for j in 0..t::PER_PAGE {
                if let Some(i) = page_point(page, j) {
                    let new = tel[t::LEVEL + j].max(FLOOR_DB);
                    let fell = self.level[i] - FALL_DB_S * (self.clock - self.stamp[i]);
                    self.level[i] = new.max(fell);
                    self.stamp[i] = self.clock;
                    self.cut[i] = tel[t::REDUCTION + j].max(0.0);
                }
            }
            self.most = tel[t::MAX_REDUCTION].max(0.0);
            self.in_peak = tel[t::IN_PEAK];
            self.out_peak = tel[t::OUT_PEAK];
        } else {
            self.idle += dt;
        }
        if live {
            self.latency = tel[t::LATENCY];
            self.quality = (tel[t::QUALITY].max(0.0) as usize).min(2);
            self.sample_rate = tel[t::SAMPLE_RATE];
        }
        if self.idle > STILL_AFTER {
            // Settling: the spectrum sinks at 60 dB a second, the cut lets go at 30.
            let sink = 60.0 * dt;
            let go = 30.0 * dt;
            for (l, c) in self.level.iter_mut().zip(self.cut.iter_mut()) {
                *l = (*l - sink).max(FLOOR_DB);
                *c = (*c - go).max(0.0);
            }
            self.most = (self.most - go).max(0.0);
            self.in_peak = 0.0;
            self.out_peak = 0.0;
        }
        // The readout: held a second, then falling.
        if self.most >= self.held {
            self.held = self.most;
            self.hold = 1.0;
        } else if self.hold > 0.0 {
            self.hold -= dt;
        } else {
            self.held = (self.held - 12.0 * dt).max(self.most);
        }
        self.seen = self.level.iter().any(|l| *l > FLOOR_DB + 0.5);
    }

    /// The screenshots' demo: the whole picture the real unit left, and its telemetry.
    pub fn seed(&mut self, level: &[f32], cut: &[f32], tel: &[f32; 64]) {
        *self = Picture::default();
        for (d, s) in self.level.iter_mut().zip(level) {
            *d = s.max(FLOOR_DB);
        }
        for (d, s) in self.cut.iter_mut().zip(cut) {
            *d = s.max(0.0);
        }
        self.frames = tel[t::FRAMES];
        self.most = tel[t::MAX_REDUCTION].max(0.0);
        self.held = self.most.max(cut.iter().fold(0.0, |m, c| m.max(*c)));
        self.hold = 1.0;
        self.latency = tel[t::LATENCY];
        self.quality = (tel[t::QUALITY].max(0.0) as usize).min(2);
        self.sample_rate = tel[t::SAMPLE_RATE];
        self.in_peak = tel[t::IN_PEAK];
        self.out_peak = tel[t::OUT_PEAK];
        self.seen = self.level.iter().any(|l| *l > FLOOR_DB + 0.5);
    }

    /// The screenshots' demo: hold the seeded picture a moment before it may settle.
    pub fn hold_demo(&mut self) {
        self.idle = -1.0;
    }

    /// Whether the picture is still moving (the editor then asks for the next frame).
    pub fn moving(&self) -> bool {
        self.idle <= STILL_AFTER + 2.0 && (self.seen || self.held > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(p: usize, frames: f32, level: f32, cut: f32) -> [f32; 64] {
        let mut tel = [0.0; 64];
        tel[t::SAMPLE_RATE] = 48_000.0;
        tel[t::FRAMES] = frames;
        tel[t::PAGE] = p as f32;
        for j in 0..t::PER_PAGE {
            tel[t::LEVEL + j] = level;
            tel[t::REDUCTION + j] = cut;
        }
        tel
    }

    #[test]
    fn every_page_fills_its_own_points_and_keeps_the_others() {
        let mut pic = Picture::default();
        for p in 0..t::PAGES {
            pic.update(&page(p, p as f32 + 1.0, -30.0, 4.0), 1.0 / 60.0);
        }
        assert!(pic.level.iter().all(|l| *l == -30.0));
        assert!(pic.cut.iter().all(|c| *c == 4.0));
        // A new page 2 changes only its own points.
        pic.update(&page(2, 9.0, -20.0, 0.0), 1.0 / 60.0);
        for i in 0..DISPLAY {
            let mine = (0..t::PER_PAGE).any(|j| page_point(2, j) == Some(i));
            assert_eq!(pic.level[i], if mine { -20.0 } else { -30.0 }, "point {i}");
        }
    }

    #[test]
    fn when_the_frames_stop_the_picture_settles_to_a_still_line() {
        let mut pic = Picture::default();
        for p in 0..t::PAGES {
            pic.update(&page(p, p as f32 + 1.0, -30.0, 6.0), 1.0 / 60.0);
        }
        let still = page(4, 5.0, -30.0, 6.0);
        for _ in 0..240 {
            pic.update(&still, 1.0 / 60.0);
        }
        assert!(pic.level.iter().all(|l| *l == FLOOR_DB));
        assert!(pic.cut.iter().all(|c| *c == 0.0));
        assert!(!pic.seen);
    }
}
