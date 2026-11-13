//! Cribrum's filter: one of the synths' five zero-delay-feedback models
//! ([`crate::va::filter`] — the same solver, so the same curve and the same
//! self-oscillation), in the modes its topology gives, at 12 or 24 dB/oct, or three of its
//! state-variable band-passes as a voice's formants.
//!
//! | Model | Low | Band | High | Notch | Peak | 24 dB/oct |
//! |---|---|---|---|---|---|---|
//! | state-variable | ✓ | ✓ | ✓ | ✓ | ✓ | low and high: a second stage at Butterworth damping |
//! | ladder, OTA cascade | ✓ | ✓ | ✓ | ✓ | band | the stages mixed (as Oberheim's Xpander did) |
//! | diode ladder | ✓ | low | low | low | low | the fourth stage's tap |
//! | Sallen-Key | ✓ | low | low | low | low | 12 only |
//!
//! **The four-pole mixes.** The ladder's stages are equal one-poles `1/(1+s)` driven by
//! `u = x − k·y₄`, so a sum `Σ cᵢ·yᵢ` (`y₀ = u`) is `N(s)/(1+s)⁴` with `N` the
//! coefficients' polynomial in `1 + s`: high-pass `s⁴` is `u − 4y₁ + 6y₂ − 4y₃ + y₄`,
//! band-pass `4s²` is `4(y₂ − 2y₃ + y₄)`, the notch `(1 + s²)²` is
//! `u − 4y₁ + 8y₂ − 8y₃ + 4y₄`, and the 12 dB/oct ones the same over `(1+s)²`; the loop's
//! resonance is shared by every mix. The diode ladder's stages load each other, so its
//! taps are not powers of one stage and it gives only its low-pass.
//!
//! **Vowels.** Three band-passes at a tenor's first three formants for A, E, I, O and U
//! (the Csound manual's table of formant values), the vowel between two of them
//! interpolated in log frequency, log bandwidth and decibels; the cutoff scales every
//! formant (1 kHz is the table's voice, two octaves either way), the resonance narrows
//! them.

use crate::math;
use crate::va::filter::{FilterCoefs, FilterSettings, Model, Solve, VaFilter};
use crate::va::math::tan_pi;

/// The models, in the order the `model` parameter lists them.
pub const MODELS: [Model; 5] = [
    Model::Svf,
    Model::Ladder,
    Model::Cascade,
    Model::Diode,
    Model::SallenKey,
];

/// The model of a parameter's index.
pub fn model_of(i: f32) -> Model {
    let i = if i.is_finite() { i.round() as i64 } else { 0 };
    MODELS[i.clamp(0, MODELS.len() as i64 - 1) as usize]
}

/// The responses (`cribrum.mode.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Low,
    Band,
    High,
    Notch,
    Peak,
    Vowel,
}

impl Mode {
    pub const ALL: [Mode; 6] = [
        Mode::Low,
        Mode::Band,
        Mode::High,
        Mode::Notch,
        Mode::Peak,
        Mode::Vowel,
    ];

    pub fn from_index(i: f32) -> Mode {
        let i = if i.is_finite() { i.round() as i64 } else { 0 };
        Mode::ALL[i.clamp(0, Mode::ALL.len() as i64 - 1) as usize]
    }
}

/// What a model gives of a mode and a slope: the mode and the 24 dB/oct actually heard
/// (the editor shows these, so nothing it draws is a promise the model does not keep).
pub fn effective(model: Model, mode: Mode, slope24: bool) -> (Mode, bool) {
    match (model, mode) {
        (_, Mode::Vowel) => (Mode::Vowel, false),
        (Model::Svf, Mode::Low | Mode::High) => (mode, slope24),
        (Model::Svf, m) => (m, false),
        (Model::Ladder | Model::Cascade, Mode::Peak) => (Mode::Band, slope24),
        (Model::Ladder | Model::Cascade, m) => (m, slope24),
        (Model::Diode, _) => (Mode::Low, slope24),
        (Model::SallenKey, _) => (Mode::Low, false),
    }
}

/// The resonance parameter (0 … 1) as the model's: its self-oscillation threshold (1.0)
/// at 95 %.
#[inline]
pub fn model_reso(res: f32) -> f32 {
    res.clamp(0.0, 1.0) * 1.05
}

/// One filter's settings as it runs now (the cutoff and the resonance are the moved ones).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Voice {
    pub model: Model,
    pub mode: Mode,
    pub slope24: bool,
    /// Hz.
    pub cut: f32,
    /// 0 … 1 (the parameter's).
    pub res: f32,
    /// Linear gain into the nonlinearities.
    pub drive: f32,
    /// 0 … 4.
    pub vowel: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            model: Model::Svf,
            mode: Mode::Low,
            slope24: false,
            cut: 1_000.0,
            res: 0.25,
            drive: 1.0,
            vowel: 0.0,
        }
    }
}

/// A tenor's formants for A, E, I, O, U: (Hz, dB, bandwidth Hz) of the first three.
pub const FORMANTS: [[(f32, f32, f32); 3]; 5] = [
    [(650.0, 0.0, 50.0), (1_080.0, -6.0, 90.0), (2_650.0, -7.0, 120.0)],
    [(400.0, 0.0, 70.0), (1_700.0, -14.0, 80.0), (2_600.0, -12.0, 100.0)],
    [(290.0, 0.0, 40.0), (1_870.0, -15.0, 90.0), (2_800.0, -18.0, 100.0)],
    [(400.0, 0.0, 40.0), (800.0, -10.0, 80.0), (2_600.0, -12.0, 100.0)],
    [(350.0, 0.0, 40.0), (600.0, -20.0, 60.0), (2_700.0, -17.0, 100.0)],
];

/// The vowel filter's level after its band-passes (they keep a narrow share of a broad
/// input): measured on pink noise, so A sits near the dry level.
pub const VOWEL_MAKEUP: f32 = 4.0;

/// The formants at vowel `v` (0 … 4), the cutoff `cut` (1 kHz: the table's), the
/// resonance `res`: (Hz, linear gain, bandwidth Hz) each.
pub fn formants(v: f32, cut: f32, res: f32) -> [(f32, f32, f32); 3] {
    let v = if v.is_finite() { v.clamp(0.0, 4.0) } else { 0.0 };
    let i = (v.floor() as usize).min(3);
    let t = v - i as f32;
    let shift = (cut / 1_000.0).clamp(0.25, 4.0);
    let narrow = math::exp2(0.6 - 2.4 * res.clamp(0.0, 1.0));
    std::array::from_fn(|n| {
        let (f0, a0, b0) = FORMANTS[i][n];
        let (f1, a1, b1) = FORMANTS[i + 1][n];
        let f = f0 * math::exp2(t * math::log2(f1 / f0));
        let b = b0 * math::exp2(t * math::log2(b1 / b0));
        let a = a0 + (a1 - a0) * t;
        (f * shift, math::db_to_gain(a), b * shift * narrow)
    })
}

/// The settings of a state-variable band-pass `bw` wide at `f`: its damping `k = bw/f`.
fn band_coefs(f: f32, bw: f32, drive: f32) -> FilterCoefs {
    let k = (bw / f.max(1.0)).clamp(0.005, 2.0);
    FilterCoefs::new(&FilterSettings {
        model: Model::Svf,
        reso: 1.0 - 0.5 * k,
        morph: 0.5,
        band: true,
        drive,
        ..FilterSettings::default()
    })
}

/// The resonance of the second stage of a 24 dB/oct state-variable: Butterworth's
/// damping `√2`.
const BUTTERWORTH_RESO: f32 = 1.0 - std::f32::consts::FRAC_1_SQRT_2;

/// How hard every model's loop is solved: twice a sample (the synths' "good"), which
/// holds the self-oscillation in tune.
pub const SOLVE: Solve = Solve::Twice;

/// One channel of one filter: up to three of the models' states (a 24 dB/oct
/// state-variable's two stages, or the three formants), and the coefficients last made,
/// kept while the resonance and the drive stay.
#[derive(Copy, Clone, Debug, Default)]
pub struct Chan {
    f: [VaFilter; 3],
    /// (model, mode, slope, resonance, drive) the coefficients were made for.
    made: Option<(Model, Mode, bool, u32, u32)>,
    c: [FilterCoefs; 2],
}

impl Chan {
    pub fn reset(&mut self) {
        self.f = [VaFilter::default(); 3];
    }

    /// Every state is a new filter's.
    pub fn at_rest(&self) -> bool {
        self.f.iter().all(VaFilter::at_rest)
    }

    fn coefs(&mut self, model: Model, mode: Mode, slope24: bool, res: f32, drive: f32) {
        let key = (model, mode, slope24, res.to_bits(), drive.to_bits());
        if self.made == Some(key) {
            return;
        }
        self.made = Some(key);
        let four = model.four_pole();
        let main = FilterSettings {
            model,
            reso: model_reso(res),
            // The ladders are mixed from all four stages; the diode ladder is tapped.
            poles: if model == Model::Diode && !slope24 {
                2
            } else {
                4
            },
            // The state-variable's outputs are read from its taps (high-pass returned).
            morph: 1.0,
            band: false,
            // Half the loop's bass loss given back on the low-pass, as a synth's filter
            // keeps its body at high resonance; the other mixes take the stages as they are.
            bass_comp: if four && mode == Mode::Low { 0.5 } else { 0.0 },
            drive,
            ..FilterSettings::default()
        };
        self.c[0] = FilterCoefs::new(&main);
        self.c[1] = FilterCoefs::new(&FilterSettings {
            model: Model::Svf,
            reso: BUTTERWORTH_RESO,
            morph: if mode == Mode::High { 1.0 } else { 0.0 },
            ..FilterSettings::default()
        });
    }

    /// One sample through the filter `v` at sample rate `fs` (the oversampled rate).
    ///
    /// A filter left with nothing of a signal in it (every integrator and solution +0.0
    /// after a sample of +0.0 out) is made new: the synths' tuning trim keeps a sag in a
    /// resonant state-variable even in silence, which no new filter has, so without this a
    /// silent filter would never be one (ADR 0031 §5.1's rest).
    #[inline]
    pub fn tick(&mut self, x: f32, v: &Voice, fs: f32) -> f32 {
        let y = self.run(x, v, fs);
        if y.to_bits() == 0 {
            for f in &mut self.f {
                if f.quiet() && !f.at_rest() {
                    *f = VaFilter::default();
                }
            }
        }
        y
    }

    #[inline]
    fn run(&mut self, x: f32, v: &Voice, fs: f32) -> f32 {
        let (mode, slope24) = effective(v.model, v.mode, v.slope24);
        if mode == Mode::Vowel {
            return self.vowel(x, v, fs);
        }
        self.coefs(v.model, mode, slope24, v.res, v.drive);
        let g = tan_pi(v.cut / fs);
        let c = self.c[0];
        let y = self.f[0].tick(x, g, &c, SOLVE);
        let out = match v.model {
            Model::Svf => {
                let t = self.f[0].taps();
                let (bp, lp, hp) = (t[0], t[1], y);
                match mode {
                    Mode::Low if slope24 => self.f[1].tick(lp, g, &self.c[1], SOLVE),
                    Mode::High if slope24 => self.f[1].tick(hp, g, &self.c[1], SOLVE),
                    Mode::Low => lp,
                    Mode::High => hp,
                    Mode::Band => 2.0 * bp,
                    Mode::Notch => lp + hp,
                    _ => lp - hp,
                }
            }
            Model::Ladder | Model::Cascade => {
                let [y1, y2, y3, y4] = self.f[0].taps();
                let u = x * c.in_gain - c.k * y4;
                match (mode, slope24) {
                    (Mode::Low, true) => y4,
                    (Mode::Low, false) => y2,
                    (Mode::High, true) => u - 4.0 * y1 + 6.0 * y2 - 4.0 * y3 + y4,
                    (Mode::High, false) => u - 2.0 * y1 + y2,
                    (Mode::Notch, true) => u - 4.0 * y1 + 8.0 * y2 - 8.0 * y3 + 4.0 * y4,
                    (Mode::Notch, false) => u - 2.0 * y1 + 2.0 * y2,
                    (_, true) => 4.0 * (y2 - 2.0 * y3 + y4),
                    (_, false) => 2.0 * (y1 - y2),
                }
            }
            _ => y,
        };
        out * drive_trim(v.drive)
    }

    #[inline]
    fn vowel(&mut self, x: f32, v: &Voice, fs: f32) -> f32 {
        self.made = None;
        let mut sum = 0.0;
        for (n, (f, a, bw)) in formants(v.vowel, v.cut, v.res).into_iter().enumerate() {
            let f = f.min(0.45 * fs);
            let c = band_coefs(f, bw, v.drive);
            // The band-pass's peak is 1/k: fed k/4 of the input and read four times, every
            // formant peaks at its own gain, and its state stays near the input's level, so
            // the state-variable's level-dependent damping (which holds its
            // self-oscillation) does not squash a narrow formant.
            let h = 0.25 * c.k.max(0.005);
            let _ = self.f[n].tick(x * h, tan_pi(f / fs), &c, SOLVE);
            sum += a * 4.0 * self.f[n].taps()[0];
        }
        sum * VOWEL_MAKEUP * drive_trim(v.drive)
    }
}

/// The level a drive is given back at: half its decibels, so a driven filter is louder
/// where it saturates little and never jumps by the drive's whole gain.
#[inline]
pub fn drive_trim(drive: f32) -> f32 {
    if drive == 1.0 {
        1.0
    } else {
        1.0 / drive.max(1e-3).sqrt()
    }
}
