//! Sibilus's detector: the band the user chose, and — for a single voice — a classifier
//! that says how much of what is in that band is *sibilance*, from two features anyone can
//! check by ear and on a spectrum.
//!
//! ## The two features
//!
//! 1. **Share**: the energy in the detection band against the energy of the whole key, in
//!    dB. An "s" puts most of its energy up there (−3 … 0 dB); a vowel, however bright,
//!    puts almost all of it in its low harmonics (−25 … −12 dB).
//! 2. **Flatness**: the key's spectrum across the band, split into [`SUB_BANDS`]
//!    sixth-order band-passes, and the geometric mean of their energies over the
//!    arithmetic mean. Noise — which is what a fricative is — spreads over all of them
//!    (0.87 … 0.98 for sibilance inside the band, about 0.5 for an "sh" only half in it);
//!    a whistle, a ringing resonance or a synth's bright partial sits in one or two (0.20
//!    at worst, a sine exactly between two sub-bands). Worked out from the filters'
//!    responses in `docs/plugins/deesser.md`, and measured by the tests.
//!
//! The confidence is the product of the two, each through a ramp; the **Sensitivity**
//! control moves both ramps. Both features are averaged over [`FEATURE_MS`], and the
//! confidence is recomputed every [`super::UPDATE`] samples counted from the start of the
//! stream, so it does not depend on the block size.

use crate::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};

/// Sub-bands the flatness is measured over.
pub const SUB_BANDS: usize = 6;
/// Band-pass sections per sub-band: three, so a sine leaks little into its neighbours.
const SUB_ORDER: usize = 3;
/// The averaging time of the classifier's features, ms.
pub const FEATURE_MS: f32 = 5.0;
/// The averaging time of the level the threshold is compared with, ms.
pub const LEVEL_MS: f32 = 2.0;
/// The narrowest span the flatness is measured over, octaves: a narrow detection band is
/// widened about its centre for this measurement only, or its sub-bands would ring.
const MIN_FLAT_SPAN: f32 = 1.0;
/// A sub-band's energy never counts as less than this share of the mean, so one empty
/// sub-band (a sibilant that stops short of the top of the band) does not zero the
/// geometric mean.
const FLAT_FLOOR: f32 = 0.02;

/// Where the share ramp sits at the middle sensitivity, dB: 0 below the first, 1 above the
/// second.
pub const SHARE_RAMP_DB: (f32, f32) = (-15.0, -7.0);
/// The flatness ramp at the middle sensitivity.
pub const FLAT_RAMP: (f32, f32) = (0.3, 0.55);
/// How far sensitivity 0 or 1 moves the share ramp, dB (each way).
const SENS_SHARE_DB: f32 = 8.0;
/// How far it moves the flatness ramp (each way).
const SENS_FLAT: f32 = 0.1;

/// One biquad, transposed direct form II, with denormals flushed.
#[derive(Copy, Clone, Debug)]
pub(super) struct Biquad {
    pub c: Coefs,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Biquad {
            c: Coefs::IDENTITY,
            z1: 0.0,
            z2: 0.0,
        }
    }
}

impl Biquad {
    #[cfg(test)]
    pub fn new(c: Coefs) -> Self {
        Biquad {
            c,
            ..Biquad::default()
        }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let c = &self.c;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        if self.z1.abs() < 1e-20 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-20 {
            self.z2 = 0.0;
        }
        y
    }

    fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// What the classifier concluded, and why.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Features {
    /// The band's share of the key's energy, dB.
    pub share_db: f32,
    /// Geometric over arithmetic mean of the sub-band energies, 0 … 1.
    pub flatness: f32,
    /// How sure it is that this is sibilance, 0 … 1.
    pub confidence: f32,
}

/// A value's place on a ramp from `lo` (0) to `hi` (1), smoothed at both ends.
#[inline]
fn ramp(v: f32, lo: f32, hi: f32) -> f32 {
    let x = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The classifier, as a pure function of the averaged energies: what the tests and the
/// documentation check. `sensitivity` is 0 … 1, 0.5 is the middle.
pub fn classify(full: f32, band: f32, sub: &[f32; SUB_BANDS], sensitivity: f32) -> Features {
    let share_db = if full > 1e-24 && band > 1e-24 {
        (10.0 * (band / full).log10()).min(0.0)
    } else {
        -120.0
    };
    let mean = sub.iter().sum::<f32>() / SUB_BANDS as f32;
    let flatness = if mean > 1e-24 {
        let floor = mean * FLAT_FLOOR;
        let log_mean = sub.iter().map(|e| e.max(floor).ln()).sum::<f32>() / SUB_BANDS as f32;
        (log_mean.exp() / mean).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let shift = (sensitivity.clamp(0.0, 1.0) - 0.5) * 2.0;
    let share = ramp(
        share_db,
        SHARE_RAMP_DB.0 - shift * SENS_SHARE_DB,
        SHARE_RAMP_DB.1 - shift * SENS_SHARE_DB,
    );
    let flat = ramp(
        flatness,
        FLAT_RAMP.0 - shift * SENS_FLAT,
        FLAT_RAMP.1 - shift * SENS_FLAT,
    );
    Features {
        share_db,
        flatness,
        confidence: share * flat,
    }
}

/// The coefficients of a detection band, designed once and shared by both channels.
#[derive(Copy, Clone, Debug)]
pub(super) struct BandDesign {
    hp: [Coefs; 2],
    lp: [Coefs; 2],
    sub: [Coefs; SUB_BANDS],
}

impl BandDesign {
    pub fn new(sr: f32, lo: f32, hi: f32) -> Self {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let mut hp = [Coefs::IDENTITY; 2];
        let n = design(
            EqShape::LowCut,
            sr,
            lo,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            24.0,
            &mut c,
        );
        hp[..n.min(2)].copy_from_slice(&c[..n.min(2)]);
        let mut lp = [Coefs::IDENTITY; 2];
        // A top edge at the top of the audio band is no edge at all.
        if hi < sr * 0.45 {
            let n = design(
                EqShape::HighCut,
                sr,
                hi,
                0.0,
                std::f32::consts::FRAC_1_SQRT_2,
                24.0,
                &mut c,
            );
            lp[..n.min(2)].copy_from_slice(&c[..n.min(2)]);
        }
        // Sub-bands: log-spaced over the band (widened to MIN_FLAT_SPAN about its
        // centre), each a band-pass run SUB_ORDER times, whose cascade crosses its
        // neighbours at −3 dB: |H|² = 1 / (1 + Q²(u − 1/u)²) raised to SUB_ORDER is ½
        // at the edge u.
        let centre = (lo * hi).sqrt();
        let span = (hi / lo).log2().max(MIN_FLAT_SPAN);
        let bw = span / SUB_BANDS as f32;
        let u = 2f32.powf(bw * 0.5);
        let at_edge = 0.5f32.powf(1.0 / SUB_ORDER as f32);
        let q = (1.0 / at_edge - 1.0).sqrt() / (u - 1.0 / u);
        let mut sub = [Coefs::IDENTITY; SUB_BANDS];
        for (k, s) in sub.iter_mut().enumerate() {
            let f = centre * 2f32.powf(bw * (k as f32 + 0.5) - span * 0.5);
            design(EqShape::BandPass, sr, f, 0.0, q, 12.0, &mut c);
            *s = c[0];
        }
        BandDesign { hp, lp, sub }
    }
}

/// One channel's detector.
#[derive(Clone, Debug, Default)]
pub(super) struct Detector {
    hp: [Biquad; 2],
    lp: [Biquad; 2],
    sub: [[Biquad; SUB_ORDER]; SUB_BANDS],
    /// DC blocker state for the broadband energy.
    dc_x: f32,
    dc_y: f32,
    e_full: f32,
    e_band: f32,
    e_sub: [f32; SUB_BANDS],
    /// Mean square of the band, for the level.
    ms: f32,
    pub features: Features,
}

/// Smoothing coefficients, computed once per sample rate.
#[derive(Copy, Clone, Debug)]
pub(super) struct Coefs2 {
    pub feature: f32,
    pub level: f32,
    pub dc: f32,
}

impl Coefs2 {
    pub fn new(sr: f32) -> Self {
        let k = |ms: f32| (-1.0 / (ms * 0.001 * sr)).exp();
        Coefs2 {
            feature: k(FEATURE_MS),
            level: k(LEVEL_MS),
            dc: (-2.0 * std::f32::consts::PI * 30.0 / sr).exp(),
        }
    }
}

#[inline]
fn smooth(e: &mut f32, x2: f32, a: f32) {
    *e = x2 + (*e - x2) * a;
    if *e < 1e-30 {
        *e = 0.0;
    }
}

impl Detector {
    pub fn set(&mut self, d: &BandDesign) {
        for (b, c) in self.hp.iter_mut().zip(d.hp) {
            b.c = c;
        }
        for (b, c) in self.lp.iter_mut().zip(d.lp) {
            b.c = c;
        }
        for (cascade, c) in self.sub.iter_mut().zip(d.sub) {
            for b in cascade {
                b.c = c;
            }
        }
    }

    pub fn clear(&mut self) {
        for b in self.hp.iter_mut().chain(self.lp.iter_mut()) {
            b.clear();
        }
        for b in self.sub.iter_mut().flatten() {
            b.clear();
        }
        self.dc_x = 0.0;
        self.dc_y = 0.0;
        self.e_full = 0.0;
        self.e_band = 0.0;
        self.e_sub = [0.0; SUB_BANDS];
        self.ms = 0.0;
        self.features = Features::default();
    }

    /// One sample of key: returns the band signal (what "listen to the trigger" plays)
    /// and the band's level, linear, where a full-scale sine reads 1.
    #[inline]
    pub fn tick(&mut self, x: f32, k: &Coefs2, voice: bool) -> (f32, f32) {
        let mut b = x;
        for f in self.hp.iter_mut().chain(self.lp.iter_mut()) {
            b = f.tick(b);
        }
        smooth(&mut self.ms, b * b, k.level);
        if voice {
            // The broadband energy, without DC or rumble.
            let y = k.dc * (self.dc_y + x - self.dc_x);
            self.dc_x = x;
            self.dc_y = if y.abs() < 1e-20 { 0.0 } else { y };
            smooth(&mut self.e_full, y * y, k.feature);
            smooth(&mut self.e_band, b * b, k.feature);
            // The sub-bands hear the key itself, not the band: a band narrower than
            // MIN_FLAT_SPAN is measured over a wider span, and its skirts must not count
            // as a hole in the spectrum.
            for (cascade, e) in self.sub.iter_mut().zip(self.e_sub.iter_mut()) {
                let mut s = y;
                for b in cascade {
                    s = b.tick(s);
                }
                smooth(e, s * s, k.feature);
            }
        }
        (b, (2.0 * self.ms).sqrt())
    }

    /// As [`Self::clear`] leaves it, for what reaches the audio: every filter and average
    /// empty and no confidence (what the classifier says of silence; the share and flatness
    /// it reports beside it are the editor's).
    pub fn is_clear(&self) -> bool {
        self.hp
            .iter()
            .chain(self.lp.iter())
            .chain(self.sub.iter().flatten())
            .all(|b| b.z1 == 0.0 && b.z2 == 0.0)
            && self.dc_x == 0.0
            && self.dc_y == 0.0
            && self.e_full == 0.0
            && self.e_band == 0.0
            && self.e_sub.iter().all(|e| *e == 0.0)
            && self.ms == 0.0
            && self.features.confidence == 0.0
    }

    /// Re-run the classifier on the averaged features.
    pub fn classify(&mut self, sensitivity: f32) {
        self.features = classify(self.e_full, self.e_band, &self.e_sub, sensitivity);
    }
}
