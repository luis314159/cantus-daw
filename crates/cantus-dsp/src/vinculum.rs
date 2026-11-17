//! **Vinculum** — the bus compressor (Latin *vinculum*, a bond: what glues a mix).
//!
//! Three models, each derived from a published circuit or paper, behind one set of
//! controls (generic names; no product's name or dress):
//!
//! | Model | Topology | Detector | Gain law | Time |
//! |---|---|---|---|---|
//! | **VCA** (a bus compressor of the console kind) | feed-forward | peak, in dB, after the gain computer (Giannoulis et al. 2012, eq. 17) | threshold, ratio 2 / 4 / 10, a soft knee ([`law::vca_gr_db`]) | attack 0.1 – 30 ms, release 0.1 / 0.3 / 0.6 / 1.2 s or Auto (two stages) |
//! | **Opto** (a T4-style cell) | feedback, solved at its equilibrium | the panel's light, the mean square of the key over 5 ms | `1 / (1 + G)`, `G = c·(u − u₀)^p` ([`law::OptoLaw`]) — Compress or Limit | the published cell's ([`law::OptoCell`]): about 10 ms attack, half released in about 60 ms, the rest over 0.5 – 15 s, slower the longer and harder it worked |
//! | **Vari-mu** (a remote-cutoff valve) | feedback | the rectified output peak over a threshold | the valve's bias law, ratio rising with level ([`law::MuLaw`]) — Compress or Limit | six positions, two of them programme-dependent |
//!
//! Every model has a side-chain high-pass (the detector only), a mix (parallel
//! compression), a make-up gain on the compressed path, and a gain-reduction reading for a
//! needle ([`Vinculum::gain_reduction_db`]). The vari-mu adds its valve's colour
//! ([`valve`]), which deepens with the reduction.
//!
//! ## The null contract
//!
//! - **Mix 0** hands every sample through untouched, to the bit.
//! - **A threshold above the signal with no make-up** touches no sample: every law gives
//!   exactly 0 dB below its knee, a gain of exactly 1 is skipped rather than multiplied,
//!   and the mix of a wet sample equal to its dry one is the dry one.
//! - **The valve colour is a control** (default 0, skipped): it colours by design, so with
//!   colour above 0 the vari-mu touches the signal even when it does not compress — the
//!   family's rule that a control at its default touches nothing holds through it.
//!
//! Everything runs per sample from per-sample state, so the output does not depend on the
//! block size; latency is 0 (no lookahead; the colour's half-band delays only the residual
//! it adds). A state under its floor is exactly 0, so silence brings every model to rest.

use crate::math;

#[path = "vinculum_law.rs"]
pub mod law;
#[path = "vinculum_valve.rs"]
pub mod valve;

use law::{Ballistics, Dual, MuLaw, OptoCell, OptoLaw, OptoTimes};

/// The threshold's range, dBFS. At its top nothing a mix carries is compressed.
pub const THRESHOLD_MIN_DB: f32 = -50.0;
pub const THRESHOLD_MAX_DB: f32 = 24.0;
/// The VCA's knee, dB (our choice: a console bus compressor's knee is firm, not hard).
pub const VCA_KNEE_DB: f32 = 3.0;
/// The VCA's stepped ratios, attacks (ms) and releases (ms).
pub const VCA_RATIOS: [f32; 3] = [2.0, 4.0, 10.0];
pub const VCA_ATTACKS_MS: [f32; 6] = [0.1, 0.3, 1.0, 3.0, 10.0, 30.0];
pub const VCA_RELEASES_MS: [f32; 4] = [100.0, 300.0, 600.0, 1200.0];
/// The VCA's Auto release: a fast stage, and a slow one that charges while it works.
pub const AUTO_FAST_MS: f32 = 100.0;
pub const AUTO_CHARGE_MS: f32 = 400.0;
pub const AUTO_SLOW_MS: f32 = 1600.0;
pub const AUTO_SHARE: f32 = 0.6;
/// The opto cell (fx-b:vinculum, the owner's decision of 2026-11-17: matched to the
/// published measurements, `vinculum_tests_opto.rs`): the panel's averaging; the attack's
/// time constant dim and fully lit (the cell quickens with light), which measure about
/// 10 ms on the reduction; the first stage's release (half of any reduction in about
/// 60 ms); the slow stage's charge, its release fresh and with a full memory; the share
/// of it the cell holds; how fast the memory fills and empties; the reduction at which the
/// cell is fully lit and the memory full.
pub const OPTO_PANEL_MS: f32 = 5.0;
pub const OPTO_ATTACK_DIM_MS: f32 = 12.0;
pub const OPTO_ATTACK_LIT_MS: f32 = 6.0;
pub const OPTO_FAST_MS: f32 = 65.0;
pub const OPTO_CHARGE_MS: f32 = 100.0;
pub const OPTO_SLOW_FRESH_MS: f32 = 600.0;
pub const OPTO_SLOW_FULL_MS: f32 = 12_000.0;
pub const OPTO_SHARE: f32 = 0.42;
pub const OPTO_FILL_MS: f32 = 4000.0;
pub const OPTO_REST_MS: f32 = 3000.0;
pub const OPTO_FULL_DB: f32 = 12.0;

/// The vari-mu's six time positions: attack, release, and (for the programme-dependent
/// two) the slow stage's charge, release and share. After the published table of the
/// classic six-position valve limiter: 0.2 ms / 0.3 s, 0.2 / 0.8, 0.4 / 2, 0.8 / 5, then
/// 0.2 ms with 2 s for a peak and 10 s for many, and 0.4 ms with 0.3 s for a peak and
/// 10 s (to 25 s) for sustained programme.
pub const MU_TIMES: [(f32, f32, f32, f32, f32); 6] = [
    (0.2, 300.0, 0.0, 0.0, 0.0),
    (0.2, 800.0, 0.0, 0.0, 0.0),
    (0.4, 2000.0, 0.0, 0.0, 0.0),
    (0.8, 5000.0, 0.0, 0.0, 0.0),
    (0.2, 2000.0, 1500.0, 10000.0, 0.7),
    (0.4, 300.0, 1500.0, 15000.0, 0.7),
];

/// The colour's depth with no reduction, and at [`COLOUR_FULL_GR_DB`] or more.
pub const COLOUR_REST_DEPTH: f32 = 0.3;
pub const COLOUR_FULL_GR_DB: f32 = 20.0;

/// The floors under which a state is exactly 0 (each in its own domain).
const FLOOR_DB: f32 = 1e-6;
const FLOOR_MS: f32 = 1e-24;

/// The circuit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Model {
    #[default]
    Vca,
    Opto,
    VariMu,
}

impl Model {
    pub const ALL: [Model; 3] = [Model::Vca, Model::Opto, Model::VariMu];

    pub fn from_index(i: f32) -> Model {
        Model::ALL[(i.round().max(0.0) as usize).min(2)]
    }
}

/// The opto's and the vari-mu's switch.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Character {
    #[default]
    Compress,
    Limit,
}

/// Everything the unit is set to. The defaults are a null: the threshold at the top, no
/// make-up, no colour.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VinculumSettings {
    pub model: Model,
    /// dBFS: the VCA's threshold; the opto's and the vari-mu's output level where they
    /// begin to work.
    pub threshold_db: f32,
    /// The VCA's ratio (2, 4 or 10 on the panel; any value ≥ 1 here).
    pub ratio: f32,
    /// The VCA's attack, ms (0.1 – 30).
    pub attack_ms: f32,
    /// The VCA's release, ms (100 – 1200), unless Auto.
    pub release_ms: f32,
    pub auto_release: bool,
    /// The opto's and the vari-mu's Compress / Limit.
    pub character: Character,
    /// The vari-mu's time position, 0 … 5.
    pub mu_time: usize,
    /// The side chain's high-pass, Hz (0 = off; 20 – 500).
    pub sc_hpf_hz: f32,
    /// The compressed path's share, 0 … 1.
    pub mix: f32,
    /// The compressed path's gain, dB (0 – 24).
    pub makeup_db: f32,
    /// The vari-mu's valve colour, 0 … 1.
    pub colour: f32,
}

impl Default for VinculumSettings {
    fn default() -> Self {
        VinculumSettings {
            model: Model::Vca,
            threshold_db: THRESHOLD_MAX_DB,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 300.0,
            auto_release: false,
            character: Character::Compress,
            mu_time: 1,
            sc_hpf_hz: 0.0,
            mix: 1.0,
            makeup_db: 0.0,
            colour: 0.0,
        }
    }
}

impl VinculumSettings {
    fn clamped(&self) -> VinculumSettings {
        let fin = |v: f32, d: f32| if v.is_finite() { v } else { d };
        VinculumSettings {
            threshold_db: fin(self.threshold_db, THRESHOLD_MAX_DB)
                .clamp(THRESHOLD_MIN_DB, THRESHOLD_MAX_DB),
            ratio: fin(self.ratio, 4.0).clamp(1.0, 100.0),
            attack_ms: fin(self.attack_ms, 10.0).clamp(0.1, 30.0),
            release_ms: fin(self.release_ms, 300.0).clamp(50.0, 5000.0),
            mu_time: self.mu_time.min(MU_TIMES.len() - 1),
            sc_hpf_hz: {
                let f = fin(self.sc_hpf_hz, 0.0);
                if f < 20.0 {
                    0.0
                } else {
                    f.min(500.0)
                }
            },
            mix: fin(self.mix, 1.0).clamp(0.0, 1.0),
            makeup_db: fin(self.makeup_db, 0.0).clamp(0.0, 24.0),
            colour: fin(self.colour, 0.0).clamp(0.0, 1.0),
            ..*self
        }
    }
}

/// A second-order Butterworth high-pass (the cookbook's), transposed direct form II, with
/// the per-sample denormal flush that brings it to an exact rest.
#[derive(Copy, Clone, Debug, Default)]
struct HighPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl HighPass {
    fn set(&mut self, hz: f32, sr: f32) {
        let w0 = 2.0 * std::f32::consts::PI * hz.min(sr * 0.45) / sr;
        let (sin, cos) = math::sin_cos(w0);
        let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 + cos) / 2.0 / a0;
        self.b1 = -(1.0 + cos) / a0;
        self.b2 = self.b0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if self.z1.abs() < 1e-20 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-20 {
            self.z2 = 0.0;
        }
        // fx-b:vinculum (phase B) — on silence a low corner's states can settle into a
        // cycle around 1e-19 that the flush above never catches (the engine's rest test, the
        // high-pass at 120 Hz): fed exact zeros, states that small are let go. The key only
        // drives the detector, and 1e-15 is 300 dB under any threshold.
        if x == 0.0 && self.z1.abs() < 1e-15 && self.z2.abs() < 1e-15 {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        y
    }

    fn is_zero(&self) -> bool {
        self.z1.to_bits() == 0 && self.z2.to_bits() == 0
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// What the meter reads after a block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VinculumMeter {
    /// The reduction at the block's end, dB (≥ 0).
    pub gr_db: f32,
    /// The deepest reduction during the block, dB.
    pub gr_peak_db: f32,
}

/// The bus compressor: one stereo pair, linked (the louder side drives both).
#[derive(Clone, Debug)]
pub struct Vinculum {
    sr: f32,
    s: VinculumSettings,
    // Derived.
    thr_inv: f32,
    makeup: f32,
    vca_b: Ballistics,
    opto_times: OptoTimes,
    panel_k: f32,
    mu_b: Ballistics,
    // State.
    hp: [HighPass; 2],
    vca: Dual,
    opto: OptoCell,
    /// The opto panel's light: the key's mean square.
    panel: f32,
    /// The opto loop's equilibrium, Compress and Limit (solved once, here).
    opto_eq: [law::OptoEquilibrium; 2],
    /// The vari-mu loop's equilibrium, Compress and Limit (solved once, here).
    mu_eq: [law::MuEquilibrium; 2],
    mu: Dual,
    valve: valve::Valve,
    // Meter.
    gr_db: f32,
    gr_peak: f32,
}

impl Vinculum {
    pub fn new(sample_rate: f32) -> Vinculum {
        let mut v = Vinculum {
            sr: sample_rate.max(1.0),
            s: VinculumSettings::default(),
            thr_inv: 1.0,
            makeup: 1.0,
            vca_b: Ballistics::single(10.0, 300.0, FLOOR_DB, 48_000.0),
            opto_times: opto_times(48_000.0),
            panel_k: 0.0,
            mu_b: Ballistics::single(0.2, 300.0, FLOOR_DB, 48_000.0),
            hp: [HighPass::default(); 2],
            vca: Dual::default(),
            opto: OptoCell::default(),
            panel: 0.0,
            opto_eq: [
                law::OptoEquilibrium::new(&OptoLaw::compress()),
                law::OptoEquilibrium::new(&OptoLaw::limit()),
            ],
            mu_eq: [
                law::MuEquilibrium::new(&MuLaw::compress()),
                law::MuEquilibrium::new(&MuLaw::limit()),
            ],
            mu: Dual::default(),
            valve: valve::Valve::new(),
            gr_db: 0.0,
            gr_peak: 0.0,
        };
        v.set(&VinculumSettings::default());
        v
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    pub fn settings(&self) -> &VinculumSettings {
        &self.s
    }

    /// New settings. The states carry on, except where the model changes (the new model
    /// starts from rest) or the colour is switched off (its filters empty).
    pub fn set(&mut self, settings: &VinculumSettings) {
        let s = settings.clamped();
        if s.model != self.s.model {
            self.reset();
        }
        if (s.sc_hpf_hz == 0.0) != (self.s.sc_hpf_hz == 0.0) {
            for h in &mut self.hp {
                h.reset();
            }
        }
        if s.colour == 0.0 {
            self.valve.reset();
        }
        let sr = self.sr;
        self.thr_inv = 1.0 / math::db_to_gain(s.threshold_db);
        self.makeup = if s.makeup_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(s.makeup_db)
        };
        self.vca_b = if s.auto_release {
            Ballistics::dual(
                s.attack_ms,
                AUTO_FAST_MS,
                AUTO_CHARGE_MS,
                AUTO_SLOW_MS,
                AUTO_SHARE,
                FLOOR_DB,
                sr,
            )
        } else {
            Ballistics::single(s.attack_ms, s.release_ms, FLOOR_DB, sr)
        };
        self.opto_times = opto_times(sr);
        self.panel_k = law::coef(OPTO_PANEL_MS, sr);
        let (a, r, c, sl, sh) = MU_TIMES[s.mu_time];
        self.mu_b = if sh > 0.0 {
            Ballistics::dual(a, r, c, sl, sh, FLOOR_DB, sr)
        } else {
            Ballistics::single(a, r, FLOOR_DB, sr)
        };
        if s.sc_hpf_hz > 0.0 {
            for h in &mut self.hp {
                h.set(s.sc_hpf_hz, sr);
            }
        }
        self.s = s;
    }

    /// Latency, samples: always 0.
    pub fn latency(&self) -> usize {
        0
    }

    pub fn reset(&mut self) {
        for h in &mut self.hp {
            h.reset();
        }
        self.vca.reset();
        self.opto.reset();
        self.panel = 0.0;
        self.mu.reset();
        self.valve.reset();
        self.gr_db = 0.0;
        self.gr_peak = 0.0;
    }

    /// At rest (ADR 0031 §5.1): every state the output depends on as a new unit has it.
    pub fn at_rest(&self) -> bool {
        let hp = self.hp.iter().all(HighPass::is_zero);
        let model = match self.s.model {
            Model::Vca => self.vca.is_zero(),
            Model::Opto => self.opto.is_zero() && self.panel.to_bits() == 0,
            Model::VariMu => self.mu.is_zero(),
        };
        hp && model && self.valve.is_zero()
    }

    /// Frames of silence skipped at rest: nothing in the unit counts time, so nothing moves.
    pub fn skip(&mut self, _frames: usize) {}

    /// The reduction now, dB (≥ 0): what the needle shows.
    pub fn gain_reduction_db(&self) -> f32 {
        self.gr_db
    }

    /// The meter's reading since the last call (the peak is cleared).
    pub fn take_meter(&mut self) -> VinculumMeter {
        let m = VinculumMeter {
            gr_db: self.gr_db,
            gr_peak_db: self.gr_peak,
        };
        self.gr_peak = self.gr_db;
        m
    }

    /// The side chain's level of one frame: the louder side, through the high-pass.
    #[inline]
    fn key(&mut self, l: f32, r: f32) -> f32 {
        let (l, r) = (crate::dynfx_follow::sane(l), crate::dynfx_follow::sane(r));
        if self.s.sc_hpf_hz > 0.0 {
            let a = self.hp[0].run(l);
            let b = self.hp[1].run(r);
            a.abs().max(b.abs())
        } else {
            l.abs().max(r.abs())
        }
    }

    /// The VCA's gain for a key level: the reduction in dB, smoothed after the computer.
    #[inline]
    fn vca_gain(&mut self, level: f32) -> (f32, f32) {
        let s = &self.s;
        let target = if level > 1e-10 {
            law::vca_gr_db(
                20.0 * math::log10(level),
                s.threshold_db,
                s.ratio,
                VCA_KNEE_DB,
            )
        } else {
            0.0
        };
        let gr = self.vca.step(target, &self.vca_b);
        let g = if gr == 0.0 {
            1.0
        } else {
            math::db_to_gain(-gr)
        };
        (g, gr)
    }

    /// The opto's gain. The panel's light is the key's mean square over
    /// [`OPTO_PANEL_MS`]; the loop's equilibrium for it — the reduction whose output
    /// lights the cell to exactly that reduction, the static curve of the feedback law —
    /// is solved at zero delay (tabulated once, [`law::OptoEquilibrium`]), and the cell
    /// moves toward it at its own time ([`law::OptoCell`]). So the attack and the two
    /// releases are the published ones measured on the reduction, at any depth and either
    /// character. (In the loop itself — the cell lit by the output it leaves — an
    /// eighth-power panel, the Limit, closed a 16 dB step in 0.9 ms, a tenth of the
    /// published ~10 ms: a feedback loop speeds the cell by its gain, as the vari-mu's
    /// first pass showed.)
    #[inline]
    fn opto_gain(&mut self, level: f32) -> (f32, f32) {
        let next = self.panel + (level * level - self.panel) * self.panel_k;
        self.panel = if next < FLOOR_MS { 0.0 } else { next };
        let eq = &self.opto_eq[(self.s.character == Character::Limit) as usize];
        let lit = self.panel * self.thr_inv * self.thr_inv;
        let target = if lit > eq.on_ms {
            eq.at(10.0 * math::log10(lit))
        } else {
            0.0
        };
        let gr = if target == 0.0 && self.opto.is_zero() {
            0.0
        } else {
            self.opto.step(target, &self.opto_times)
        };
        let g = if gr == 0.0 {
            1.0
        } else {
            math::db_to_gain(-gr)
        };
        (g, gr)
    }

    /// The vari-mu's gain. The loop's equilibrium — the charge `c` (the reduction, dB)
    /// at which the output it leaves asks for exactly `c`, `c = GR(knee(x − c))`, the
    /// static curve — is solved at zero delay (tabulated once, [`law::MuEquilibrium`]),
    /// and the charge moves toward it at the table's time constants. So the reduction
    /// rises and falls at the stated attack and release, measured on the reduction, at
    /// every level and either character (a charge moving toward the output's own
    /// demand at the rectifier's rate gets there `R` times sooner, `R` the local ratio:
    /// the first pass measured 0.06 ms for 0.2); and with no sample of delay in the
    /// loop, the limit's 20 : 1 at 0.2 ms cannot ring.
    #[inline]
    fn mu_gain(&mut self, level: f32) -> (f32, f32) {
        let xs = level * self.thr_inv;
        let target = if xs > 1e-10 {
            let eq = &self.mu_eq[(self.s.character == Character::Limit) as usize];
            eq.at(20.0 * math::log10(xs))
        } else {
            0.0
        };
        let gr = self.mu.step(target, &self.mu_b);
        let g = if gr == 0.0 {
            1.0
        } else {
            math::db_to_gain(-gr)
        };
        (g, gr)
    }

    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.process_keyed(left, right, None);
    }

    /// Process a stereo block in place, the side chain listening to `key` (another strip's
    /// signal, the classic bus compressor's external key) when given, to the input
    /// otherwise; through the high-pass either way. In the feedback models the key takes
    /// the input's place in the loop: the opto's and the vari-mu's equilibria are solved on the
    /// key. `None` is [`Vinculum::process`] to
    /// the bit; a key missing samples is silence there. (fx-b:vinculum, phase B)
    pub fn process_keyed(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        key: Option<(&[f32], &[f32])>,
    ) {
        let n = left.len().min(right.len());
        let mix = self.s.mix;
        let colour = if self.s.model == Model::VariMu {
            self.s.colour
        } else {
            0.0
        };
        let makeup = self.makeup;
        let mut peak = self.gr_peak;
        let mut last = self.gr_db;
        for i in 0..n {
            let (xl, xr) = (left[i], right[i]);
            let level = match key {
                Some((kl, kr)) => self.key(
                    kl.get(i).copied().unwrap_or(0.0),
                    kr.get(i).copied().unwrap_or(0.0),
                ),
                None => self.key(xl, xr),
            };
            let (g, gr) = match self.s.model {
                Model::Vca => self.vca_gain(level),
                Model::Opto => self.opto_gain(level),
                Model::VariMu => self.mu_gain(level),
            };
            last = gr;
            peak = peak.max(gr);
            let depth = if colour > 0.0 {
                colour
                    * (COLOUR_REST_DEPTH
                        + (1.0 - COLOUR_REST_DEPTH) * (gr / COLOUR_FULL_GR_DB).min(1.0))
            } else {
                0.0
            };
            // The colour's filters hear the input whatever the mix, so a mix raised later
            // finds them as if they had always run.
            let (rl, rr) = if depth > 0.0 {
                (self.valve.run(0, xl), self.valve.run(1, xr))
            } else {
                (0.0, 0.0)
            };
            if mix == 0.0 {
                continue;
            }
            for (x, res, out) in [(xl, rl, &mut left[i]), (xr, rr, &mut right[i])] {
                let mut wet = x;
                if depth > 0.0 {
                    wet += depth * res;
                }
                if g != 1.0 {
                    wet *= g;
                }
                if makeup != 1.0 {
                    wet *= makeup;
                }
                *out = if wet.to_bits() == x.to_bits() || mix == 1.0 {
                    wet
                } else {
                    x + mix * (wet - x)
                };
            }
        }
        self.gr_db = last;
        self.gr_peak = peak;
    }
}

/// The opto cell's coefficients at a sample rate.
fn opto_times(sr: f32) -> OptoTimes {
    OptoTimes {
        attack_dim: law::coef(OPTO_ATTACK_DIM_MS, sr),
        attack_lit: law::coef(OPTO_ATTACK_LIT_MS, sr),
        release: law::coef(OPTO_FAST_MS, sr),
        charge: law::coef(OPTO_CHARGE_MS, sr),
        slow: {
            let span = math::ln(OPTO_SLOW_FULL_MS / OPTO_SLOW_FRESH_MS);
            let mut k = [0f32; law::OPTO_SLOW_N + 1];
            for (i, v) in k.iter_mut().enumerate() {
                let m = i as f32 / law::OPTO_SLOW_N as f32;
                *v = 1000.0 / (OPTO_SLOW_FRESH_MS * sr) * math::exp(-span * m);
            }
            k
        },
        share: OPTO_SHARE,
        fill: law::coef(OPTO_FILL_MS, sr),
        rest: law::coef(OPTO_REST_MS, sr),
        inv_full_db: 1.0 / OPTO_FULL_DB,
        floor: FLOOR_DB,
    }
}

#[cfg(test)]
#[path = "vinculum_tests.rs"]
mod tests;
