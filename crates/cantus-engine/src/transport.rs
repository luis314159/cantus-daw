//! Sample-accurate transport.
//!
//! Position is an integer sample counter since an *origin tick*, so it is exactly
//! reproducible regardless of block size (the key to the offline render being bit-identical
//! to real-time playback). Ticks are derived, never accumulated.

use cantus_core::{LoopRegion, Ticks, PPQ};

#[derive(Clone, Debug)]
pub struct Transport {
    sample_rate: f64,
    bpm: f64,
    playing: bool,
    /// Tick at which `samples` was zero.
    origin_tick: f64,
    samples: u64,
    loop_region: Option<LoopRegion>,
}

impl Transport {
    pub fn new(sample_rate: f64) -> Self {
        Transport {
            sample_rate,
            bpm: 120.0,
            playing: false,
            origin_tick: 0.0,
            samples: 0,
            loop_region: None,
        }
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn loop_region(&self) -> Option<&LoopRegion> {
        self.loop_region.as_ref()
    }

    /// Ticks advanced per sample at the current tempo.
    #[inline]
    pub fn ticks_per_sample(&self) -> f64 {
        self.bpm * PPQ as f64 / (60.0 * self.sample_rate)
    }

    /// Exact current position in ticks (fractional).
    #[inline]
    pub fn position_f64(&self) -> f64 {
        self.position_at(self.samples)
    }

    /// The position, in ticks, at a sample counted from the origin — the same arithmetic
    /// as [`Self::position_f64`], so a control point computed ahead lands on the tick the
    /// playhead will report there.
    #[inline]
    pub fn position_at(&self, samples: u64) -> f64 {
        self.origin_tick + samples as f64 * self.ticks_per_sample()
    }

    pub fn position(&self) -> Ticks {
        Ticks(self.position_f64().floor() as i64)
    }

    /// The tick the sample counter counts from: the last start, seek, loop wrap or tempo
    /// change. With [`Self::samples`], what an audio clip's frame is computed from, so
    /// playback is the same for every block size.
    #[inline]
    pub fn origin_tick(&self) -> f64 {
        self.origin_tick
    }

    /// Samples since [`Self::origin_tick`].
    #[inline]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Where the playhead was `back` samples ago, exactly as [`Self::position_f64`] said
    /// then — or `None` if it has jumped since (a seek, a loop wrap, a tempo change).
    #[inline]
    pub fn position_back(&self, back: u64) -> Option<f64> {
        (self.samples >= back)
            .then(|| self.origin_tick + (self.samples - back) as f64 * self.ticks_per_sample())
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Move the playhead. A tick before zero is a pre-roll (the count-in before bar 1),
    /// which plays nothing but the click until the timeline begins; the application
    /// clamps the seeks the user makes.
    pub fn seek(&mut self, tick: Ticks) {
        self.origin_tick = tick.0 as f64;
        self.samples = 0;
    }

    /// Change tempo without moving the playhead.
    pub fn set_bpm(&mut self, bpm: f64) {
        if bpm != self.bpm {
            self.origin_tick = self.position_f64();
            self.samples = 0;
            self.bpm = bpm;
        }
    }

    pub fn set_loop(&mut self, region: Option<LoopRegion>) {
        self.loop_region = region;
    }

    /// Absolute sample index (since the origin) at which the playhead reaches `tick`.
    /// Computed from the origin, never from the running position, so it is identical
    /// for every block size. A tiny epsilon keeps events that land exactly on a sample
    /// from slipping one sample late through floating-point noise.
    #[inline]
    fn target_sample(&self, tick: f64) -> u64 {
        let x = (tick - self.origin_tick) / self.ticks_per_sample();
        if x <= 0.0 {
            0
        } else {
            (x - 1e-6).ceil().max(0.0) as u64
        }
    }

    /// Number of samples from now until the playhead reaches `tick` (0 if already past).
    #[inline]
    pub fn samples_until(&self, tick: f64) -> u64 {
        self.target_sample(tick).saturating_sub(self.samples)
    }

    /// Samples until the loop end, if looping and the playhead is inside the loop.
    pub fn samples_until_loop_end(&self) -> Option<u64> {
        let l = self.loop_region.as_ref()?;
        let pos = self.position_f64();
        if pos < l.start.0 as f64 {
            return None;
        }
        let end_s = self.target_sample(l.end.0 as f64);
        if self.samples >= end_s {
            return Some(0);
        }
        Some(end_s - self.samples)
    }

    /// Advance by `n` samples while playing. Returns `true` if the loop wrapped.
    pub fn advance(&mut self, n: u64) -> bool {
        if !self.playing {
            return false;
        }
        let before = self.samples;
        self.samples += n;
        if let Some(l) = &self.loop_region {
            let start = l.start.0 as f64;
            let end = l.end.0 as f64;
            let end_s = self.target_sample(end);
            if self.samples >= end_s && before < end_s && self.position_f64() >= start {
                // Carry the sub-sample overshoot so loops never drift.
                let overshoot = self.position_f64() - end;
                self.origin_tick = start + overshoot.max(0.0);
                self.samples = 0;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_is_exact_and_block_size_independent() {
        let mut a = Transport::new(48_000.0);
        let mut b = Transport::new(48_000.0);
        a.set_bpm(133.0);
        b.set_bpm(133.0);
        a.play();
        b.play();
        for _ in 0..1000 {
            a.advance(64);
        }
        for _ in 0..125 {
            b.advance(512);
        }
        assert_eq!(a.position_f64(), b.position_f64());
        assert_eq!(a.position(), b.position());
        // 64000 samples at 133 bpm = 1.3333 s = 2.9556 beats
        let beats = a.position_f64() / PPQ as f64;
        assert!((beats - 64_000.0 / 48_000.0 * 133.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn loop_wraps_without_drift() {
        let mut t = Transport::new(44_100.0);
        t.set_loop(Some(LoopRegion {
            start: Ticks::beats(1),
            end: Ticks::beats(3),
        }));
        t.seek(Ticks::beats(1));
        t.play();
        let mut wraps = 0;
        for _ in 0..10_000 {
            if t.advance(256) {
                wraps += 1;
            }
        }
        // 2 beats at 120 bpm = 1 s = 44100 samples; 2 560 000 samples ≈ 58 loops.
        assert_eq!(wraps, 58);
        let pos = t.position_f64();
        assert!(pos >= Ticks::beats(1).0 as f64 && pos < Ticks::beats(3).0 as f64);
        let expected =
            Ticks::beats(1).0 as f64 + (2_560_000.0 - 58.0 * 44_100.0) * t.ticks_per_sample();
        assert!(
            (pos - expected).abs() < 1e-6,
            "pos {pos} expected {expected}"
        );
    }

    #[test]
    fn samples_until_rounds_up() {
        let mut t = Transport::new(48_000.0);
        t.play();
        // 1 tick = 48000*60/(120*960) = 25 samples exactly.
        assert_eq!(t.samples_until(1.0), 25);
        assert_eq!(t.samples_until(0.5), 13);
        assert_eq!(t.samples_until(-4.0), 0);
        t.advance(10);
        assert_eq!(t.samples_until(1.0), 15);
    }
}
