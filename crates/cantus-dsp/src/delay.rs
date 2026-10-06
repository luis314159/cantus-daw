//! Stereo delay with feedback tone control and ping-pong mode.
//!
//! The delay line is allocated once (2 s at the given sample rate); time changes glide
//! through a smoother and a fractional read head, so sweeping the time bends pitch like a
//! tape echo instead of clicking.

use crate::math;
use crate::smoother::Smoother;

pub const MAX_DELAY_SECONDS: f32 = 2.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DelaySettings {
    pub time_ms: f32,
    pub feedback: f32,
    pub tone_hz: f32,
    pub ping_pong: bool,
    pub mix: f32,
}

impl Default for DelaySettings {
    fn default() -> Self {
        DelaySettings {
            time_ms: 375.0,
            feedback: 0.4,
            tone_hz: 6_000.0,
            ping_pong: true,
            mix: 0.3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StereoDelay {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    time: Smoother,
    mix: Smoother,
    lp_l: f32,
    lp_r: f32,
    lp_coef: f32,
    settings: DelaySettings,
}

impl StereoDelay {
    pub fn new(sample_rate: f32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + 2;
        let s = DelaySettings::default();
        let mut d = StereoDelay {
            sample_rate,
            buf_l: vec![0.0; len],
            buf_r: vec![0.0; len],
            write: 0,
            time: Smoother::new(s.time_ms * 0.001 * sample_rate, 0.05, sample_rate),
            mix: Smoother::new(s.mix, 0.02, sample_rate),
            lp_l: 0.0,
            lp_r: 0.0,
            lp_coef: 1.0,
            settings: s.clone(),
        };
        d.set(&s);
        d
    }

    pub fn settings(&self) -> &DelaySettings {
        &self.settings
    }

    pub fn set(&mut self, s: &DelaySettings) {
        let max_samples = (self.buf_l.len() - 2) as f32;
        let samples = (s.time_ms.max(0.0) * 0.001 * self.sample_rate).clamp(1.0, max_samples);
        self.time.set_target(samples);
        self.mix.set_target(s.mix.clamp(0.0, 1.0));
        let fc = s.tone_hz.clamp(20.0, self.sample_rate * 0.45);
        self.lp_coef = 1.0 - math::exp(-std::f32::consts::TAU * fc / self.sample_rate);
        self.settings = s.clone();
    }

    /// Clear the line and jump smoothers to their targets.
    pub fn reset(&mut self) {
        self.buf_l.iter_mut().for_each(|x| *x = 0.0);
        self.buf_r.iter_mut().for_each(|x| *x = 0.0);
        self.write = 0;
        self.lp_l = 0.0;
        self.lp_r = 0.0;
        let max_samples = (self.buf_l.len() - 2) as f32;
        self.time
            .snap((self.settings.time_ms * 0.001 * self.sample_rate).clamp(1.0, max_samples));
        self.mix.snap(self.settings.mix.clamp(0.0, 1.0));
    }

    #[inline]
    fn read(buf: &[f32], write: usize, delay: f32) -> f32 {
        let len = buf.len();
        let pos = write as f32 - delay;
        let pos = if pos < 0.0 { pos + len as f32 } else { pos };
        let i0 = pos.floor() as usize % len;
        let i1 = (i0 + 1) % len;
        let frac = pos - pos.floor();
        buf[i0] + (buf[i1] - buf[i0]) * frac
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let fb = self.settings.feedback.clamp(0.0, 0.98);
        let ping_pong = self.settings.ping_pong;
        let len = self.buf_l.len();
        for i in 0..l.len().min(r.len()) {
            let d = self.time.next();
            let mix = self.mix.next();
            let in_l = l[i];
            let in_r = r[i];
            let rd_l = Self::read(&self.buf_l, self.write, d);
            let rd_r = Self::read(&self.buf_r, self.write, d);
            // Tone: one-pole low-pass in the feedback path.
            self.lp_l += self.lp_coef * (rd_l - self.lp_l);
            self.lp_r += self.lp_coef * (rd_r - self.lp_r);
            flush(&mut self.lp_l);
            flush(&mut self.lp_r);
            let (wr_l, wr_r) = if ping_pong {
                ((in_l + in_r) * 0.5 + fb * self.lp_r, fb * self.lp_l)
            } else {
                (in_l + fb * self.lp_l, in_r + fb * self.lp_r)
            };
            self.buf_l[self.write] = wr_l;
            self.buf_r[self.write] = wr_r;
            self.write = (self.write + 1) % len;
            l[i] = in_l + (rd_l - in_l) * mix;
            r[i] = in_r + (rd_r - in_r) * mix;
        }
    }
}

#[inline]
fn flush(x: &mut f32) {
    if x.abs() < 1e-20 {
        *x = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_lands_at_the_right_sample() {
        let mut d = StereoDelay::new(48_000.0);
        d.set(&DelaySettings {
            time_ms: 10.0,
            feedback: 0.0,
            tone_hz: 20_000.0,
            ping_pong: false,
            mix: 1.0,
        });
        d.reset();
        let n = 2_000;
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        l[0] = 1.0;
        r[0] = 1.0;
        d.process(&mut l, &mut r);
        let peak_at = l
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak_at, 480);
        assert!(l[480] > 0.9);
    }

    #[test]
    fn ping_pong_alternates_channels_and_decays() {
        let mut d = StereoDelay::new(48_000.0);
        d.set(&DelaySettings {
            time_ms: 5.0,
            feedback: 0.5,
            tone_hz: 20_000.0,
            ping_pong: true,
            mix: 1.0,
        });
        d.reset();
        let n = 2_000;
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        l[0] = 1.0;
        d.process(&mut l, &mut r);
        // First echo on the left (input goes into the left line), second on the right.
        assert!(
            l[240].abs() > 0.4 && r[240].abs() < 1e-3,
            "l {} r {}",
            l[240],
            r[240]
        );
        assert!(
            r[480].abs() > 0.2 && l[480].abs() < 1e-3,
            "l {} r {}",
            l[480],
            r[480]
        );
        assert!(l[720].abs() < l[240].abs());
        assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()));
    }

    #[test]
    fn deterministic_and_silent_on_silence() {
        let run = || {
            let mut d = StereoDelay::new(44_100.0);
            let mut l: Vec<f32> = (0..4_000).map(|i| math::sin((i as f32) * 0.01)).collect();
            let mut r = l.clone();
            d.process(&mut l, &mut r);
            (l, r)
        };
        assert_eq!(run(), run());
        let mut d = StereoDelay::new(44_100.0);
        let mut l = vec![0.0; 1_000];
        let mut r = vec![0.0; 1_000];
        d.process(&mut l, &mut r);
        assert!(l.iter().chain(r.iter()).all(|x| *x == 0.0));
    }
}
