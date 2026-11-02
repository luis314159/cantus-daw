//! The light visualiser's numbers (ADR 0091 §4): the master's spectrum in a few dozen
//! bands and its level, smoothed for the eye, from the samples the engine's master tap
//! copied (`cantus_engine::master_tap`) — the analysis is here, on the interface's side, at
//! most [`LIVE_RATE`] times a second, never on the audio thread.
//!
//! Bands are a third of an octave wide from 40 Hz to 16 kHz on a log axis, read through
//! `cantus_dsp::spectrum` (a Hann-windowed 2 048-point FFT of both channels, their power
//! averaged), mapped from [`FLOOR_DB`]…[`CEIL_DB`] to 0…1. A band rises at once and falls at
//! [`FALL_PER_S`] of the scale a second — a meter's ballistics, so nothing flickers.

use cantus_dsp::math;
use cantus_dsp::spectrum::SpectrumAnalyser;

/// How many bands.
pub const BANDS: usize = 28;
/// The lowest and highest bands' centres, Hz.
pub const LOW_HZ: f32 = 40.0;
pub const HIGH_HZ: f32 = 16_000.0;
/// What the scale spans, dBFS.
pub const FLOOR_DB: f32 = -66.0;
pub const CEIL_DB: f32 = -6.0;
/// How fast a band or the level falls, in shares of the scale a second.
pub const FALL_PER_S: f32 = 1.4;
/// Analyses a second, at most, in a video's frames (each frame reads its time's).
pub const RATE: f64 = 30.0;
/// Analyses a second the live visualiser makes: the bands fall smoothly between them, so a
/// modest rate is enough and leaves the processor alone (the companion runs all day).
pub const LIVE_RATE: f64 = 20.0;
/// The FFT's size.
pub const FFT: usize = 2048;

/// A band's centre frequency.
pub fn band_hz(i: usize) -> f32 {
    let t = i as f32 / (BANDS - 1) as f32;
    LOW_HZ * math::powf(HIGH_HZ / LOW_HZ, t)
}

/// dBFS on the display's 0…1 scale.
pub fn scale(db: f32) -> f32 {
    ((db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0)
}

/// The visualiser's state.
pub struct Visualiser {
    analyser: SpectrumAnalyser,
    /// 0…1 per band, as shown.
    pub bands: [f32; BANDS],
    /// 0…1: the master's peak on the same scale, as shown.
    pub level: f32,
    /// When it last analysed and last moved (egui's clock).
    analysed_at: f64,
    moved_at: f64,
    /// Samples since the last analysis.
    fresh: usize,
}

impl Default for Visualiser {
    fn default() -> Self {
        Visualiser {
            analyser: SpectrumAnalyser::new(FFT),
            bands: [0.0; BANDS],
            level: 0.0,
            analysed_at: f64::NEG_INFINITY,
            moved_at: 0.0,
            fresh: 0,
        }
    }
}

impl Visualiser {
    /// Add interleaved left/right pairs from the tap.
    pub fn feed(&mut self, pairs: &[f32]) {
        self.analyser.push(pairs);
        self.fresh += pairs.len() / 2;
    }

    /// Bring the display up to `now`: analyse when due and something came, fall where
    /// nothing rises. `peak` is the master's peak (linear, both channels' larger).
    pub fn update(&mut self, now: f64, sample_rate: f32, peak: f32) {
        let dt = (now - self.moved_at).clamp(0.0, 0.25) as f32;
        self.moved_at = now;
        let fall = FALL_PER_S * dt;
        let lvl = if peak > 1e-6 {
            scale(20.0 * math::log10(peak))
        } else {
            0.0
        };
        self.level = lvl.max(self.level - fall);
        let due = now - self.analysed_at >= 0.95 / LIVE_RATE;
        if due && self.fresh > 0 && self.analyser.has_data() {
            self.analysed_at = now;
            self.fresh = 0;
            self.analyser.compute();
            for (i, b) in self.bands.iter_mut().enumerate() {
                let hz = band_hz(i);
                let l = self.analyser.level_db(0, sample_rate, hz, 1.0 / 3.0);
                let r = self.analyser.level_db(1, sample_rate, hz, 1.0 / 3.0);
                // The two channels' power averaged.
                let p = 0.5 * (math::powf(10.0, l / 10.0) + math::powf(10.0, r / 10.0));
                let db = if p > 1e-12 {
                    10.0 * math::log10(p)
                } else {
                    -120.0
                };
                *b = scale(db).max(*b - fall);
            }
        } else {
            for b in &mut self.bands {
                *b = (*b - fall).max(0.0);
            }
        }
    }

    /// Whether anything still shows (a band or the level above zero).
    pub fn moving(&self) -> bool {
        self.level > 0.0 || self.bands.iter().any(|b| *b > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .flat_map(|i| {
                let v = amp * math::sin(2.0 * std::f32::consts::PI * hz * i as f32 / 48_000.0);
                [v, v]
            })
            .collect()
    }

    #[test]
    fn a_tone_lights_its_band_and_not_the_far_ones() {
        let mut v = Visualiser::default();
        v.feed(&sine(1000.0, 0.5, 4096));
        v.update(1.0, 48_000.0, 0.5);
        let at = (0..BANDS)
            .min_by(|a, b| {
                (band_hz(*a) - 1000.0)
                    .abs()
                    .total_cmp(&(band_hz(*b) - 1000.0).abs())
            })
            .unwrap();
        assert!(v.bands[at] > 0.7, "the 1 kHz band: {:?}", v.bands);
        assert!(
            v.bands[0] < 0.1 && v.bands[BANDS - 1] < 0.1,
            "{:?}",
            v.bands
        );
        // −6 dBFS peak is the scale's top.
        assert!((v.level - 1.0).abs() < 1e-3, "{}", v.level);
    }

    #[test]
    fn silence_falls_at_the_meters_pace_and_rests_at_zero() {
        let mut v = Visualiser::default();
        v.feed(&sine(200.0, 0.5, 4096));
        v.update(1.0, 48_000.0, 0.5);
        assert!(v.moving());
        v.update(1.25, 48_000.0, 0.0);
        let after = v.level;
        assert!((after - (1.0 - FALL_PER_S * 0.25)).abs() < 1e-3, "{after}");
        for i in 0..10 {
            v.update(1.5 + i as f64 * 0.25, 48_000.0, 0.0);
        }
        assert!(!v.moving(), "everything at rest");
    }

    #[test]
    fn the_bands_span_the_range_on_a_log_axis() {
        assert!((band_hz(0) - LOW_HZ).abs() < 0.01);
        assert!((band_hz(BANDS - 1) - HIGH_HZ).abs() < 1.0);
        let r1 = band_hz(1) / band_hz(0);
        let r2 = band_hz(BANDS - 1) / band_hz(BANDS - 2);
        assert!((r1 - r2).abs() < 1e-3);
    }
}
