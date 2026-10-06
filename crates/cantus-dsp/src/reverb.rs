//! Algorithmic reverb in the Freeverb topology (8 parallel damped combs → 4 series
//! allpasses per channel, stereo spread) with a pre-delay. Deterministic, allocation-free
//! after construction. Convolution and better-sounding FDN designs come later.

use crate::rest::ZeroRun;

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const FIXED_GAIN: f32 = 0.015;
const SCALE_WET: f32 = 3.0;
const MAX_PREDELAY_SECONDS: f32 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub struct ReverbSettings {
    pub size: f32,
    pub damping: f32,
    pub width: f32,
    pub predelay_ms: f32,
    pub mix: f32,
}

impl Default for ReverbSettings {
    fn default() -> Self {
        ReverbSettings {
            size: 0.7,
            damping: 0.4,
            width: 1.0,
            predelay_ms: 10.0,
            mix: 0.25,
        }
    }
}

#[derive(Clone, Debug)]
struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
    quiet: ZeroRun,
}

impl Comb {
    fn new(len: usize) -> Self {
        Comb {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            store: 0.0,
            quiet: ZeroRun::FULL,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * (1.0 - damp) + self.store * damp;
        if self.store.abs() < 1e-20 {
            self.store = 0.0;
        }
        self.buf[self.idx] = input + self.store * feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.store = 0.0;
        self.idx = 0;
        self.quiet.clear();
    }

    fn at_rest(&self) -> bool {
        self.store == 0.0 && self.quiet.covers(self.buf.len())
    }
}

#[derive(Clone, Debug)]
struct Allpass {
    buf: Vec<f32>,
    idx: usize,
    quiet: ZeroRun,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Allpass {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            quiet: ZeroRun::FULL,
        }
    }

    fn at_rest(&self) -> bool {
        self.quiet.covers(self.buf.len())
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.idx];
        let out = bufout - input;
        self.buf[self.idx] = input + bufout * 0.5;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.idx = 0;
        self.quiet.clear();
    }
}

#[derive(Clone, Debug)]
pub struct Reverb {
    sample_rate: f32,
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    allpasses_l: Vec<Allpass>,
    allpasses_r: Vec<Allpass>,
    predelay: Vec<f32>,
    predelay_write: usize,
    predelay_samples: usize,
    predelay_quiet: ZeroRun,
    feedback: f32,
    damp: f32,
    wet1: f32,
    wet2: f32,
    mix: f32,
    settings: ReverbSettings,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44_100.0;
        let tune = |n: usize| ((n as f32) * scale).round() as usize;
        let s = ReverbSettings::default();
        let mut r = Reverb {
            sample_rate,
            combs_l: COMB_TUNING.iter().map(|n| Comb::new(tune(*n))).collect(),
            combs_r: COMB_TUNING
                .iter()
                .map(|n| Comb::new(tune(*n + STEREO_SPREAD)))
                .collect(),
            allpasses_l: ALLPASS_TUNING
                .iter()
                .map(|n| Allpass::new(tune(*n)))
                .collect(),
            allpasses_r: ALLPASS_TUNING
                .iter()
                .map(|n| Allpass::new(tune(*n + STEREO_SPREAD)))
                .collect(),
            predelay: vec![0.0; (MAX_PREDELAY_SECONDS * sample_rate) as usize + 2],
            predelay_write: 0,
            predelay_samples: 0,
            predelay_quiet: ZeroRun::FULL,
            feedback: 0.0,
            damp: 0.0,
            wet1: 0.0,
            wet2: 0.0,
            mix: 0.0,
            settings: s.clone(),
        };
        r.set(&s);
        r
    }

    pub fn settings(&self) -> &ReverbSettings {
        &self.settings
    }

    pub fn set(&mut self, s: &ReverbSettings) {
        self.feedback = 0.28 + s.size.clamp(0.0, 1.0) * 0.7;
        self.damp = s.damping.clamp(0.0, 1.0) * 0.4;
        let width = s.width.clamp(0.0, 1.0);
        self.wet1 = SCALE_WET * (width / 2.0 + 0.5);
        self.wet2 = SCALE_WET * ((1.0 - width) / 2.0);
        self.mix = s.mix.clamp(0.0, 1.0);
        let max = self.predelay.len() - 2;
        self.predelay_samples =
            ((s.predelay_ms.max(0.0) * 0.001 * self.sample_rate) as usize).min(max);
        self.settings = s.clone();
    }

    pub fn reset(&mut self) {
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.clear();
        }
        for a in self
            .allpasses_l
            .iter_mut()
            .chain(self.allpasses_r.iter_mut())
        {
            a.clear();
        }
        self.predelay.iter_mut().for_each(|x| *x = 0.0);
        self.predelay_write = 0;
        self.predelay_quiet.clear();
    }

    /// At rest (ADR 0031 §5.1): the pre-delay, every comb and every allpass hold only
    /// zeros and the combs' damping filters are empty — what [`Self::reset`] leaves. The
    /// settings are plain values (no glide), so nothing else moves.
    pub fn at_rest(&self) -> bool {
        self.predelay_quiet.covers(self.predelay.len())
            && self
                .combs_l
                .iter()
                .chain(self.combs_r.iter())
                .all(Comb::at_rest)
            && self
                .allpasses_l
                .iter()
                .chain(self.allpasses_r.iter())
                .all(Allpass::at_rest)
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let len = self.predelay.len();
        let n = l.len().min(r.len());
        for i in 0..n {
            let in_l = l[i];
            let in_r = r[i];
            // Pre-delay on the mono send.
            let send = (in_l + in_r) * FIXED_GAIN;
            self.predelay[self.predelay_write] = send;
            let read_idx = (self.predelay_write + len - self.predelay_samples) % len;
            let input = self.predelay[read_idx];
            self.predelay_write = (self.predelay_write + 1) % len;

            let mut out_l = 0.0;
            let mut out_r = 0.0;
            for c in &mut self.combs_l {
                out_l += c.process(input, self.feedback, self.damp);
            }
            for c in &mut self.combs_r {
                out_r += c.process(input, self.feedback, self.damp);
            }
            for a in &mut self.allpasses_l {
                out_l = a.process(out_l);
            }
            for a in &mut self.allpasses_r {
                out_r = a.process(out_r);
            }
            let wet_l = out_l * self.wet1 + out_r * self.wet2;
            let wet_r = out_r * self.wet1 + out_l * self.wet2;
            l[i] = in_l + (wet_l - in_l) * self.mix;
            r[i] = in_r + (wet_r - in_r) * self.mix;
        }
        self.predelay_quiet
            .wrote(&self.predelay, self.predelay_write, n);
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.quiet.wrote(&c.buf, c.idx, n);
        }
        for a in self
            .allpasses_l
            .iter_mut()
            .chain(self.allpasses_r.iter_mut())
        {
            a.quiet.wrote(&a.buf, a.idx, n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_response(sr: f32, s: &ReverbSettings, seconds: f32) -> Vec<f32> {
        let mut rv = Reverb::new(sr);
        rv.set(s);
        let n = (sr * seconds) as usize;
        let mut l = vec![0.0; n];
        let mut r = vec![0.0; n];
        l[0] = 1.0;
        r[0] = 1.0;
        for start in (0..n).step_by(512) {
            let end = (start + 512).min(n);
            let (ll, rr) = (&mut l[start..end], &mut r[start..end]);
            rv.process(ll, rr);
        }
        l.iter()
            .zip(r.iter())
            .map(|(a, b)| (a * a + b * b).sqrt())
            .collect()
    }

    #[test]
    fn tail_exists_and_decays() {
        let s = ReverbSettings {
            mix: 1.0,
            predelay_ms: 0.0,
            ..Default::default()
        };
        let ir = impulse_response(48_000.0, &s, 6.0);
        let energy = |from: usize, to: usize| ir[from..to].iter().map(|x| x * x).sum::<f32>();
        let early = energy(4_800, 9_600);
        let late = energy(96_000, 100_800);
        let very_late = energy(240_000, 244_800);
        assert!(early > 1e-3, "no early reflections: {early}");
        assert!(late < early, "tail must decay: {late} vs {early}");
        assert!(very_late < late * 0.5, "tail must keep decaying");
        assert!(ir.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn predelay_shifts_the_onset() {
        let dry = ReverbSettings {
            mix: 1.0,
            predelay_ms: 0.0,
            ..Default::default()
        };
        let pre = ReverbSettings {
            mix: 1.0,
            predelay_ms: 50.0,
            ..Default::default()
        };
        let a = impulse_response(48_000.0, &dry, 0.5);
        let b = impulse_response(48_000.0, &pre, 0.5);
        let onset = |ir: &[f32]| ir.iter().position(|x| x.abs() > 1e-6).unwrap();
        assert!(
            onset(&b) >= onset(&a) + 2_400 - 2,
            "{} vs {}",
            onset(&a),
            onset(&b)
        );
    }

    #[test]
    fn deterministic_and_silent_on_silence() {
        let s = ReverbSettings::default();
        assert_eq!(
            impulse_response(44_100.0, &s, 1.0),
            impulse_response(44_100.0, &s, 1.0)
        );
        let mut rv = Reverb::new(44_100.0);
        let mut l = vec![0.0; 2_000];
        let mut r = vec![0.0; 2_000];
        rv.process(&mut l, &mut r);
        assert!(l.iter().chain(r.iter()).all(|x| *x == 0.0));
    }
}
