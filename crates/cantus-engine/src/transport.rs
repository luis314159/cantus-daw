//! Sample-accurate transport.
//!
//! Position is an integer sample counter since an *origin tick*, so it is exactly
//! reproducible regardless of block size (the key to the offline render being bit-identical
//! to real-time playback). Ticks are derived, never accumulated.

use crate::fx::Wraps;
use cantus_core::{LoopRegion, Ticks, PPQ};

/// The sample (counted from `origin`) at which a transport playing at `bpm` reaches
/// `tick` — the transport's own arithmetic, so an offline render that must stop exactly
/// where an event would fire computes the same sample, to the last bit (round 23).
#[inline]
pub fn sample_at(origin: f64, tick: f64, bpm: f64, sample_rate: f64) -> u64 {
    let x = (tick - origin) / (bpm * PPQ as f64 / (60.0 * sample_rate));
    if x <= 0.0 {
        0
    } else {
        (x - 1e-6).ceil().max(0.0) as u64
    }
}

#[derive(Clone, Debug)]
pub struct Transport {
    sample_rate: f64,
    bpm: f64,
    playing: bool,
    /// Tick at which `samples` was zero.
    origin_tick: f64,
    samples: u64,
    loop_region: Option<LoopRegion>,
    /// The loop's wraps since a start or a seek — the samples since the last, how many,
    /// the last passes' lengths; `None` before the first (review of fix3: the audio an
    /// insert hears late is from before the loop only on its first entry; review of fix4:
    /// and never from more passes than were played).
    wraps: Option<Wraps>,
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
            wraps: None,
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
        if !self.playing {
            self.wraps = None;
        }
        self.playing = true;
    }

    /// Samples played since the loop last wrapped — `None` before the first wrap since the
    /// transport started or was moved.
    #[inline]
    pub fn since_wrap(&self) -> Option<u64> {
        self.wraps.as_ref().map(|w| w.since)
    }

    /// The loop's wraps since the transport started or was moved, with the pass playing
    /// and the loop's length now — `None` before the first, or with no loop ahead.
    pub fn wraps(&self) -> Option<Wraps> {
        let mut w = self.wraps?;
        let l = self.loop_region.as_ref()?;
        w.pass = w.since + self.samples_until_loop_end()?;
        w.loop_samples = (l.end.0 - l.start.0) as f64 / self.ticks_per_sample();
        Some(w)
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
        self.wraps = None;
    }

    /// Move the playhead to the loop's start as a wrap (the loop's end due at a pass's first
    /// sample): counted with the others, as `advance` counts its own (review of fix4).
    pub fn wrap_to(&mut self, start: Ticks) {
        self.origin_tick = start.0 as f64;
        self.samples = 0;
        match self.wraps.as_mut() {
            Some(w) => w.wrap(),
            None => self.wraps = Some(Wraps::first(0, 0, 0.0)),
        }
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
        sample_at(self.origin_tick, tick, self.bpm, self.sample_rate)
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
        if let Some(w) = self.wraps.as_mut() {
            w.since += n;
        }
        if let Some(l) = &self.loop_region {
            let start = l.start.0 as f64;
            let end = l.end.0 as f64;
            let end_s = self.target_sample(end);
            if self.samples >= end_s && before < end_s && self.position_f64() >= start {
                // Carry the sub-sample overshoot so loops never drift.
                let overshoot = self.position_f64() - end;
                self.origin_tick = start + overshoot.max(0.0);
                self.samples = 0;
                match self.wraps.as_mut() {
                    Some(w) => w.wrap(),
                    None => self.wraps = Some(Wraps::first(0, 0, 0.0)),
                }
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

    /// Play `samples` one at a time from `from` with the loop `[ls, le)` (in ticks), the
    /// tempo changed to `bpm_at(pass)` at each wrap: the clock at each wrap.
    fn run(
        bpm: f64,
        ls: i64,
        le: i64,
        from: i64,
        samples: u64,
        bpm_at: impl Fn(usize) -> f64,
    ) -> (Transport, Vec<u64>) {
        let mut t = Transport::new(48_000.0);
        t.set_bpm(bpm);
        t.set_loop(Some(LoopRegion {
            start: Ticks(ls),
            end: Ticks(le),
        }));
        t.seek(Ticks(from));
        t.play();
        let mut wraps = Vec::new();
        for clock in 1..=samples {
            if t.advance(1) {
                wraps.push(clock);
                t.set_bpm(bpm_at(wraps.len()));
            }
        }
        (t, wraps)
    }

    /// The truth `Wraps::before_wrap` stands for: how many samples before the wrap that
    /// ends its stretch the audio `back` samples ago lies.
    fn truth(wraps: &[u64], now: u64, back: u64) -> Option<u64> {
        let at = now.checked_sub(back)?;
        if wraps.last().is_none_or(|w| at >= *w) {
            return None;
        }
        wraps.iter().find(|w| **w > at).map(|w| w - at)
    }

    /// Review of fix4: the wraps a late insert's audio lies behind are the ones played —
    /// none before the first, as many as there were after — at their real samples: exact
    /// for the kept passes even when the tempo changed between them (the passes' lengths
    /// differ), and within a sample of the truth further back while the tempo holds (a loop
    /// whose length is no whole number of samples: the measured worst case, here over 200
    /// passes of a 1/16-beat loop at 133 BPM).
    #[test]
    fn the_wraps_are_the_ones_played_at_their_samples() {
        // 1/16 beat at 133 BPM: 1 353.38… samples a pass.
        let (t, wraps) = run(133.0, 960, 1020, 0, 300_000, |_| 133.0);
        let w = t.wraps().expect("wrapped");
        assert_eq!(w.count as usize, wraps.len());
        let now = 300_000;
        let mut worst = 0u64;
        for back in (1..=now).step_by(7) {
            let (got, want) = (w.before_wrap(back), truth(&wraps, now, back));
            match (got, want) {
                (Some(g), Some(x)) => {
                    let kept_span: u64 = now - wraps[wraps.len() - 1 - crate::fx::PASSES_KEPT];
                    if back <= kept_span {
                        assert_eq!(g, x, "kept passes are exact, {back} back");
                    }
                    worst = worst.max(g.abs_diff(x));
                }
                (None, None) => {}
                // Before the clock's start: the song before the play, straight.
                (Some(_), None) if back > now => {}
                (g, x) => panic!("{back} back: {g:?}, the truth {x:?}"),
            }
        }
        assert!(worst <= 1, "the measured worst case: {worst} samples");
        // Only one wrap played: the audio before it is the lead-in, however far back.
        let (t, wraps) = run(120.0, 7680, 8640, 0, 230_000, |_| 120.0);
        let w = t.wraps().expect("wrapped");
        assert_eq!(wraps.len(), 1);
        let since = 230_000 - wraps[0];
        assert_eq!(w.before_wrap(since + 60_000), Some(60_000));
        // A tempo that changes at every wrap, the passes 24 000 · 120/bpm samples long:
        // exact over the kept passes.
        let (t, wraps) = run(120.0, 960, 1920, 960, 400_000, |n| 100.0 + (n % 5) as f64 * 9.0);
        let w = t.wraps().expect("wrapped");
        let now = 400_000;
        let kept_span = now - wraps[wraps.len() - 1 - crate::fx::PASSES_KEPT.min(wraps.len() - 1)];
        for back in (1..kept_span).step_by(3) {
            assert_eq!(w.before_wrap(back), truth(&wraps, now, back), "{back} back");
        }
    }
}
