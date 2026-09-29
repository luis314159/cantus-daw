//! Zero-delay-feedback filters: five topologies, each a trapezoidal (TPT) model whose
//! feedback loop is solved for the current sample instead of delayed by one, with its
//! nonlinearities linearised around an operating point and re-solved (Zavalishin, *The Art
//! of VA Filter Design*; the "cheap non-linear zero-delay" method, Voipio).
//!
//! | Model | Topology | Character |
//! |---|---|---|
//! | [`Model::Ladder`] | four transistor stages, `tanh` at each differential pair, global feedback | warm; bass loss with resonance |
//! | [`Model::Cascade`] | four OTA integrators, `tanh` at each OTA input | brighter, tighter |
//! | [`Model::Diode`] | coupled diode ladder (unbuffered stages) | rubbery, squelchy, deep bass loss |
//! | [`Model::Svf`] | two-pole state-variable with amplitude-dependent damping | full bass at any Q; LP → notch/BP → HP |
//! | [`Model::SallenKey`] | two-pole Sallen-Key, diode limiter in the feedback | aggressive, strongly level-dependent |
//!
//! **Every model self-oscillates at resonance 1.0, exactly at its cutoff.** The linear
//! prototype of each is solved for where its loop reaches −180° and unit gain, and the
//! model is scaled so that point is the cutoff the player set: a four-pole ladder at
//! `k = 4`, the diode ladder at `k = 901/49` with its integrators 1.195× below the cutoff,
//! the two-poles at zero damping. Resonance above 1.0 is held by the nonlinearities.
//!
//! **Quality** is how many times the loop is solved per sample ([`Solve`]): once around
//! the previous sample's operating point, twice, or until the output moves by less than
//! 10⁻⁶ (at most eight passes).

use super::math::{tanh, tanh_gain};

#[path = "va_filter_models.rs"]
mod models;
pub use models::describe;

/// The topologies.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Model {
    #[default]
    Ladder,
    Cascade,
    Diode,
    Svf,
    SallenKey,
}

impl Model {
    pub const ALL: [Model; 5] = [
        Model::Ladder,
        Model::Cascade,
        Model::Diode,
        Model::Svf,
        Model::SallenKey,
    ];

    pub fn from_index(i: f32) -> Model {
        match i.round() as i32 {
            1 => Model::Cascade,
            2 => Model::Diode,
            3 => Model::Svf,
            4 => Model::SallenKey,
            _ => Model::Ladder,
        }
    }

    /// The loop gain at resonance 1.0: where the model starts to self-oscillate.
    pub fn k_threshold(self) -> f32 {
        match self {
            Model::Ladder | Model::Cascade => 4.0,
            Model::Diode => DIODE_K,
            Model::Svf | Model::SallenKey => 2.0,
        }
    }

    /// Whether it is a four-pole model with poles to tap and bass to compensate.
    pub fn four_pole(self) -> bool {
        matches!(self, Model::Ladder | Model::Cascade | Model::Diode)
    }
}

/// The diode ladder's (uniform coupled RC ladder's) critical loop gain: with
/// `D(s) = s⁴ + 7s³ + 15s² + 10s + 1`, `Im D(jω) = 0` at `ω² = 10/7`, where
/// `−Re D = 901/49`.
pub const DIODE_K: f32 = 901.0 / 49.0;
/// …and the frequency of that crossing, as a multiple of the integrators' own.
pub const DIODE_W: f32 = 1.195_228_6; // √(10/7)

/// Circuit revisions: the same topology, pushed differently.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Revision {
    #[default]
    Clean,
    /// An asymmetric input stage: even harmonics, a grittier resonance.
    Rough,
    /// Saturates earlier and drives harder into self-oscillation.
    Pushed,
}

impl Revision {
    pub fn from_index(i: f32) -> Revision {
        match i.round() as i32 {
            1 => Revision::Rough,
            2 => Revision::Pushed,
            _ => Revision::Clean,
        }
    }
}

/// How hard the loop is solved each sample.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Solve {
    /// Once, linearised around the previous sample's operating point.
    Once,
    /// Twice: the second pass linearised around the first pass's answer.
    Twice,
    /// Until the output moves by less than 10⁻⁶, at most eight passes.
    Converge,
}

impl Solve {
    fn passes(self) -> (u8, u8) {
        match self {
            Solve::Once => (1, 1),
            Solve::Twice => (2, 2),
            Solve::Converge => (2, 8),
        }
    }
}

/// What a filter is set to, apart from its cutoff (which may move every sample).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FilterSettings {
    pub model: Model,
    /// 0…1.2; 1.0 is the self-oscillation threshold.
    pub reso: f32,
    /// Which pole the four-pole models are tapped at: 1…4 (6, 12, 18, 24 dB/oct).
    pub poles: u8,
    /// State-variable: 0 low-pass, ½ notch or band-pass, 1 high-pass.
    pub morph: f32,
    /// State-variable: the middle of the morph is a band-pass (else a notch).
    pub band: bool,
    /// Four-pole models: how much of the `1/(1+k)` bass loss is restored, 0…1.
    pub bass_comp: f32,
    /// Linear gain into the model's first nonlinearity.
    pub drive: f32,
    pub rev: Revision,
}

impl Default for FilterSettings {
    fn default() -> Self {
        FilterSettings {
            model: Model::Ladder,
            reso: 0.0,
            poles: 4,
            morph: 0.0,
            band: false,
            bass_comp: 0.0,
            drive: 1.0,
            rev: Revision::Clean,
        }
    }
}

/// Coefficients derived from the settings: computed when they change, never per sample.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FilterCoefs {
    pub model: Model,
    /// Loop gain (four-pole, Sallen-Key) or damping (state-variable).
    pub k: f32,
    /// Input gain: drive, times the bass compensation `1 + c·k`.
    pub in_gain: f32,
    /// Integrator scale: the cutoff's `g` times this is the integrators' own.
    pub warp: f32,
    /// The saturators' argument scale (a larger scale saturates earlier; the small-signal
    /// gain is unchanged).
    pub sat: f32,
    /// The input stage's asymmetry.
    pub bias: f32,
    /// State-variable: the amplitude-dependent damping.
    pub nl: f32,
    pub tap: usize,
    pub morph: f32,
    pub band: bool,
    /// How much of the saturation's pole sag the tuning trims back: none at low
    /// resonance (a driven filter darkens, as it should), all of it from the
    /// self-oscillation threshold (where the pitch must track the keys).
    pub track: f32,
    /// The model's own tuning trim: the mean pole factor understates the pitch the
    /// saturation costs (the loop spends most of each cycle where the stages saturate
    /// hardest). Measured per model at resonance 1.05.
    pub trim: f32,
}

impl FilterCoefs {
    pub fn new(s: &FilterSettings) -> Self {
        let reso = s.reso.clamp(0.0, 1.25);
        let model = s.model;
        let k = match model {
            Model::Svf => 2.0 - 2.0 * reso,
            _ => model.k_threshold() * reso,
        };
        let comp = if model.four_pole() {
            1.0 + s.bass_comp.clamp(0.0, 1.0) * k
        } else {
            1.0
        };
        let (sat, bias, nl) = match s.rev {
            Revision::Clean => (1.0, 0.0, 0.8),
            Revision::Rough => (1.0, 0.35, 1.1),
            Revision::Pushed => (1.7, 0.0, 0.5),
        };
        FilterCoefs {
            model,
            k,
            in_gain: s.drive.max(0.0) * comp,
            warp: if model == Model::Diode {
                1.0 / DIODE_W
            } else {
                1.0
            },
            sat,
            bias,
            nl,
            tap: s.poles.clamp(1, 4) as usize - 1,
            morph: s.morph.clamp(0.0, 1.0),
            band: s.band,
            trim: match model {
                Model::Cascade => 1.45,
                Model::Diode => 1.38,
                _ => 1.5,
            },
            track: if model.four_pole() {
                ((reso - 0.5) * 2.0).clamp(0.0, 1.0)
            } else {
                ((reso - 0.8) * 5.0).clamp(0.0, 1.0)
            },
        }
    }

    /// The gain of the input stage's saturator at `v` (asymmetric in the Rough revision).
    #[inline]
    fn input_gain(&self, v: f32) -> f32 {
        if self.bias == 0.0 {
            return tanh_gain(self.sat * v);
        }
        let b = self.bias;
        let tb = tanh(b);
        let w = self.sat * v;
        if w.abs() < 1e-4 {
            1.0 - tb * tb
        } else {
            (tanh(w + b) - tb) / w
        }
    }
}

impl Default for FilterCoefs {
    fn default() -> Self {
        FilterCoefs::new(&FilterSettings::default())
    }
}

/// A filter's state: four integrators and the last solution (the next sample's operating
/// point). The same state serves every model, so a model switch never allocates.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VaFilter {
    s: [f32; 4],
    e: [f32; 4],
    /// The solution the sample before `e`: with it the first pass starts from a straight
    /// line through the last two answers, not from the last one.
    p: [f32; 4],
    /// The mean factor by which saturation moved the poles in the last sample (1: not at
    /// all), set by the model.
    pole: f32,
    /// How far the poles have sagged on average lately (0: not at all) — what the tuning
    /// trims back, as a calibrated circuit tracks.
    sag: f32,
    /// Passes the last sample took (for the iteration histogram).
    pub last_passes: u8,
}

/// How fast the tuning follows the sag, per sample, relative to the cutoff's `g`: a few
/// cycles of the cutoff.
const SAG_RATE: f32 = 0.05;
/// Start each sample's solve from the last two answers extended (see [`VaFilter::p`]).
pub const EXTRAPOLATE: bool = true;

impl VaFilter {
    pub fn reset(&mut self) {
        *self = VaFilter::default();
    }

    /// Where the next sample's solution probably is: the last two, extended.
    #[inline]
    fn estimate(&self) -> [f32; 4] {
        if EXTRAPOLATE {
            [
                2.0 * self.e[0] - self.p[0],
                2.0 * self.e[1] - self.p[1],
                2.0 * self.e[2] - self.p[2],
                2.0 * self.e[3] - self.p[3],
            ]
        } else {
            self.e
        }
    }

    /// Keep a new solution (and the one before it).
    #[inline]
    fn keep(&mut self, y: [f32; 4]) {
        self.p = self.e;
        self.e = y;
    }

    /// Whether every state is exactly zero.
    pub fn is_silent(&self) -> bool {
        self.s.iter().chain(self.e.iter()).all(|x| *x == 0.0)
    }

    /// One sample. `g` is `tan(π·cutoff/fs)` for the cutoff shown ([`super::math::tan_pi`]);
    /// the model applies its own warp.
    #[inline]
    pub fn tick(&mut self, x: f32, g: f32, c: &FilterCoefs, solve: Solve) -> f32 {
        let g = g * c.warp / (1.0 - self.sag);
        let y = match c.model {
            Model::Ladder => self.ladder(x, g, c, solve, false),
            Model::Cascade => self.ladder(x, g, c, solve, true),
            Model::Diode => self.diode(x, g, c, solve),
            Model::Svf => self.svf(x, g, c, solve),
            Model::SallenKey => self.sallen_key(x, g, c, solve),
        };
        for s in &mut self.s {
            *s = super::math::flush(*s);
        }
        let a = (g * SAG_RATE).min(0.05);
        self.sag =
            super::math::flush(self.sag + ((1.0 - self.pole) * c.trim * c.track - self.sag) * a)
                .clamp(0.0, 0.6);
        y
    }
}
