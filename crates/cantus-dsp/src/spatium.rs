//! Spatium, the reverb.
//!
//! ```text
//!             ┌─────────── dry ────────────────────────────────────────────────┐
//!  in ─┬─ freeze gate ─ pre-delay ─┬─ early reflections ─ tone ─┐             mix ─ out
//!      │                           └─ gap ─ diffusion ─ late field ┴─ post EQ ─ width ─ duck ┘
//!      └─ (or the sidechain key) ─ ducking detector ──────────────────────────┘
//! ```
//!
//! - **The late field** is a 16-line feedback delay network with a per-line absorption
//!   filter fitted to the decay curve (`fdn`), or, in the Spring style, a tank of
//!   dispersive springs (`spring`).
//! - **The decay curve** — decay in seconds × the room × brightness × the drawn curve —
//!   and the rooms are pure functions in `model`, shared with the editor.
//! - **Early reflections** are a tapped delay, a pattern per room scaled by `er_size`,
//!   decorrelated between the sides and darkened with distance.
//! - **The post EQ** is Aequalis, on the wet signal only, with automatic gain compensation;
//!   brightness's tilt and a style's bandwidth ride on it as two more bands.
//! - **Ducking** follows the sidechain key when one is routed, the input otherwise: a
//!   threshold, a range, an attack and a release, in dB and ms.
//!
//! ## The contract
//!
//! Allocation-free after `new`; deterministic; every glide and smoother runs per sample,
//! so the output does not depend on the block size; and at **mix 0 the output is the input
//! to the bit** (the network still runs, so its tail is where it would be when the mix
//! comes back). The latency is zero.

// The parts live in files of their own beside this one (`spatium_*.rs`), declared here so
// the plug-in is owned by its files and `lib.rs` does not change.
#[path = "spatium_fdn.rs"]
pub mod fdn;
#[path = "spatium_measure.rs"]
pub mod measure;
#[path = "spatium_model.rs"]
pub mod model;
#[path = "spatium_spring.rs"]
pub mod spring;

use crate::aequalis::{Aequalis, BandSettings, EqShape};
use crate::spatium::fdn::{Absorber, Fdn, LateDesign, LINES};
use crate::spatium::model::{
    brightness_tilt_db, diffusion, er_level_db, er_pattern, er_tone_hz, late_gap_ms, late_level_db,
    line_lengths_ms, modulation, room_at, style_shape, DecayBand, Style, TargetCurve, DECAY_BANDS,
    ER_SIZE_MAX, ER_TAPS, MAX_ER_MS, MAX_LINE_MS, MAX_PREDELAY_MS, POST_BANDS,
};
use crate::spatium::spring::{SpringDesign, SpringTank};

pub use crate::spatium::measure::OCTAVES;

/// Samples processed per inner chunk (the post EQ runs on the wet chunk).
const CHUNK: usize = 64;
/// The spring tank's first spring at the smallest and largest Space, ms.
const SPRING_MS: (f32, f32) = (22.0, 48.0);
/// Calibration of the late field's level (see `late_gain`): measured on pink noise so that
/// at a 2 s decay the late field's steady-state power equals the input's (every style
/// read +2.0 ± 0.2 dB before it; `the_wet_level_is_calibrated`).
const LATE_CAL: f32 = 0.79;

/// A post-EQ band as the parameter table makes it: a bell at 1 kHz, Q 0.7, cuts at 24
/// dB/octave when chosen (the table's defaults, so a new reverb's settings are these).
pub const POST_BAND_DEFAULT: BandSettings = BandSettings {
    active: false,
    shape: EqShape::Bell,
    freq: 1_000.0,
    gain_db: 0.0,
    q: 0.7,
    slope: 24.0,
    placement: crate::Placement::Stereo,
    dynamic: false,
    threshold_db: -24.0,
    range_db: -6.0,
    ratio: 4.0,
    attack_ms: 10.0,
    release_ms: 120.0,
    below: false,
    external: false,
};

/// Everything Spatium is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpatiumSettings {
    pub style: Style,
    /// 0…1 across the rooms.
    pub space: f32,
    /// Mid-band RT60, seconds.
    pub decay_s: f32,
    pub predelay_ms: f32,
    pub distance: f32,
    pub character: f32,
    pub brightness: f32,
    /// 0 mono … 1 natural … 1.2 wider.
    pub width: f32,
    /// Dry/wet, 0…1: the dry stays at unity up to 0.5, the wet reaches unity at 0.5.
    pub mix: f32,
    pub freeze: bool,
    /// Early reflections: level offset (dB; at or below `ER_OFF_DB` they are off) and
    /// size (×0.25…×2 of the room's).
    pub er_db: f32,
    pub er_size: f32,
    /// Ducking: range (0 is off), threshold, attack, release.
    pub duck_db: f32,
    pub duck_thr_db: f32,
    pub duck_att_ms: f32,
    pub duck_rel_ms: f32,
    pub decay_eq_on: bool,
    pub decay_bands: [DecayBand; DECAY_BANDS],
    pub post_eq_on: bool,
    pub post_auto_gain: bool,
    pub post_bands: [BandSettings; POST_BANDS],
}

impl Default for SpatiumSettings {
    fn default() -> Self {
        SpatiumSettings {
            style: Style::Room,
            space: 0.5,
            decay_s: 1.6,
            predelay_ms: 8.0,
            distance: 0.35,
            character: 0.3,
            brightness: 0.45,
            width: 1.0,
            mix: 0.25,
            freeze: false,
            er_db: 0.0,
            er_size: 1.0,
            duck_db: 0.0,
            duck_thr_db: -30.0,
            duck_att_ms: 10.0,
            duck_rel_ms: 400.0,
            decay_eq_on: true,
            decay_bands: [DecayBand::default(); DECAY_BANDS],
            post_eq_on: true,
            post_auto_gain: true,
            post_bands: [POST_BAND_DEFAULT; POST_BANDS],
        }
    }
}

impl SpatiumSettings {
    /// The decay curve these settings ask for.
    pub fn target_curve(&self) -> TargetCurve {
        TargetCurve::new(
            self.style,
            self.space,
            self.decay_s,
            self.brightness,
            self.decay_eq_on,
            &self.decay_bands,
        )
    }

    /// Everything that decides the late field's design.
    fn late_key(&self) -> (Style, f32, f32, f32, bool, [DecayBand; DECAY_BANDS]) {
        (
            self.style,
            self.space,
            self.decay_s,
            self.brightness,
            self.decay_eq_on,
            self.decay_bands,
        )
    }

    /// The Aequalis bands the wet signal runs through: the user's post EQ, brightness's
    /// tilt, and the style's bandwidth. And the auto-gain for the user's part, dB.
    pub fn wet_eq(&self) -> ([BandSettings; POST_BANDS + 2], f32) {
        let mut bands = [BandSettings::default(); POST_BANDS + 2];
        if self.post_eq_on {
            bands[..POST_BANDS].copy_from_slice(&self.post_bands);
        }
        let tilt = brightness_tilt_db(self.brightness);
        if tilt != 0.0 {
            bands[POST_BANDS] = BandSettings {
                active: true,
                shape: EqShape::HighShelf,
                freq: 4_000.0,
                gain_db: tilt,
                q: 0.6,
                ..BandSettings::default()
            };
        }
        let bw = style_shape(self.style).bandwidth_hz;
        if bw > 0.0 {
            bands[POST_BANDS + 1] = BandSettings {
                active: true,
                shape: EqShape::HighCut,
                freq: bw,
                q: 0.707,
                slope: 12.0,
                ..BandSettings::default()
            };
        }
        let comp = if self.post_eq_on && self.post_auto_gain {
            auto_gain_db(&self.post_bands)
        } else {
            0.0
        };
        (bands, comp)
    }
}

/// What the post EQ does to the wet level on average, inverted: the power average of its
/// response over a third-octave grid (equal weight per octave, i.e. pink), dB.
pub fn auto_gain_db(bands: &[BandSettings]) -> f32 {
    if !bands.iter().any(|b| b.active) {
        return 0.0;
    }
    let mut acc = 0.0f64;
    let n = 31;
    for j in 0..n {
        let hz = 20.0 * 2f32.powf(j as f32 / 3.0);
        let db: f32 = bands
            .iter()
            .filter(|b| b.active)
            .map(|b| Aequalis::band_response_db(b, 48_000.0, hz))
            .sum();
        acc += 10f64.powf(db as f64 / 10.0);
    }
    let mean = acc / n as f64;
    (-10.0 * mean.max(1e-6).log10()).clamp(-12.0, 12.0) as f32
}

/// The two halves of the mix law: dry stays at unity to 50 %, wet reaches unity at 50 %.
pub fn mix_gains(mix: f32) -> (f32, f32) {
    let m = mix.clamp(0.0, 1.0);
    ((2.0 * (1.0 - m)).min(1.0), (2.0 * m).min(1.0))
}

/// What the editor draws from the engine, refreshed every block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SpatiumMeter {
    /// Wet peaks, L and R, linear, held and falling ~20 dB/s.
    pub wet_peak: [f32; 2],
    /// Input (or key) peak, same ballistics.
    pub in_peak: f32,
    /// Ducking's gain reduction now, dB (positive).
    pub duck_db: f32,
    /// 0…1.
    pub freeze: f32,
    /// Correlation of the wet L and R (−1…1), over about 300 ms.
    pub correlation: f32,
    pub predelay_ms: f32,
    /// The late lines' mean length now, ms.
    pub line_ms: f32,
    pub mod_depth_ms: f32,
    pub keyed: bool,
    /// The decay the late field achieves at each of [`OCTAVES`], seconds.
    pub achieved: [f32; OCTAVES.len()],
}

/// A Schroeder all-pass, for input diffusion.
#[derive(Clone, Debug)]
struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Allpass {
            buf: vec![0.0; len.max(1)],
            idx: 0,
        }
    }

    #[inline]
    fn tick(&mut self, x: f32, g: f32) -> f32 {
        let d = self.buf[self.idx];
        let v = x + g * d;
        let v = if v.abs() < 1e-18 { 0.0 } else { v };
        self.buf[self.idx] = v;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        d - g * v
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.idx = 0;
    }
}

/// A one-pole smoother that lands exactly on its target.
#[derive(Copy, Clone, Debug)]
struct Glide {
    now: f32,
    target: f32,
}

impl Glide {
    fn new(v: f32) -> Self {
        Glide { now: v, target: v }
    }

    #[inline]
    fn next(&mut self, coef: f32, eps: f32) -> f32 {
        crate::spatium::fdn::glide(&mut self.now, self.target, coef, eps);
        self.now
    }

    fn snap(&mut self) {
        self.now = self.target;
    }
}

const DIFFUSER_MS: [[f32; 4]; 2] = [[4.771, 3.595, 12.73, 9.307], [4.533, 3.721, 11.97, 9.841]];

/// Spatium's processing state.
#[derive(Clone, Debug)]
pub struct Spatium {
    sample_rate: f32,
    settings: SpatiumSettings,
    configured: bool,
    absorber: Absorber,
    design: LateDesign,
    fdn: Fdn,
    spring: SpringTank,
    spring_design: Option<SpringDesign>,
    /// The input after the freeze gate: straight L and R (for the late field) and each side
    /// with some of the other (for the reflections).
    in_mask: usize,
    in_w: usize,
    buf: [Vec<f32>; 4],
    predelay: Glide,
    gap: Glide,
    er_t: [[Glide; ER_TAPS]; 2],
    er_g: [[Glide; ER_TAPS]; 2],
    er_lp: [f32; 2],
    er_lp_coef: f32,
    diff: [[Allpass; 4]; 2],
    diff_g: Glide,
    er_gain: Glide,
    late_gain: Glide,
    in_gain: Glide,
    mix: Glide,
    width: Glide,
    /// Coefficients: gains (~20 ms), times (~80 ms, audible as a short glide in pitch).
    k_gain: f32,
    k_time: f32,
    // Ducking.
    duck_env: f32,
    duck_gr: f32,
    duck_det: f32,
    duck_att: f32,
    duck_rel: f32,
    // Wet signal path.
    post: Aequalis,
    post_active: bool,
    wl: [f32; CHUNK],
    wr: [f32; CHUNK],
    gain_dry: [f32; CHUNK],
    gain_wet: [f32; CHUNK],
    gain_duck: [f32; CHUNK],
    width_now: [f32; CHUNK],
    // Meters.
    meter: SpatiumMeter,
    fall: f32,
    corr: [f32; 3],
    corr_coef: f32,
}

impl Spatium {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate.max(8_000.0);
        let in_need = (fs * (MAX_PREDELAY_MS + MAX_ER_MS + 80.0) * 0.001) as usize + 8;
        let in_size = in_need.next_power_of_two();
        let diff = |side: usize| -> [Allpass; 4] {
            let mk = |ms: f32| Allpass::new((ms * 0.001 * fs) as usize);
            [
                mk(DIFFUSER_MS[side][0]),
                mk(DIFFUSER_MS[side][1]),
                mk(DIFFUSER_MS[side][2]),
                mk(DIFFUSER_MS[side][3]),
            ]
        };
        let mut s = Spatium {
            sample_rate: fs,
            settings: SpatiumSettings::default(),
            configured: false,
            absorber: Absorber::new(fs),
            design: LateDesign::identity(fs),
            fdn: Fdn::new(fs, MAX_LINE_MS),
            spring: SpringTank::new(fs, SPRING_MS.1),
            spring_design: None,

            in_mask: in_size - 1,
            in_w: 0,
            buf: [
                vec![0.0; in_size],
                vec![0.0; in_size],
                vec![0.0; in_size],
                vec![0.0; in_size],
            ],
            predelay: Glide::new(0.0),
            gap: Glide::new(0.0),
            er_t: [[Glide::new(0.0); ER_TAPS]; 2],
            er_g: [[Glide::new(0.0); ER_TAPS]; 2],
            er_lp: [0.0; 2],
            er_lp_coef: 1.0,
            diff: [diff(0), diff(1)],
            diff_g: Glide::new(0.6),
            er_gain: Glide::new(0.0),
            late_gain: Glide::new(0.0),
            in_gain: Glide::new(1.0),
            mix: Glide::new(0.0),
            width: Glide::new(1.0),
            k_gain: 1.0 - (-1.0 / (0.02 * fs)).exp(),
            k_time: 1.0 - (-1.0 / (0.08 * fs)).exp(),
            duck_env: 0.0,
            duck_gr: 0.0,
            duck_det: (-1.0 / (0.05 * fs)).exp(),
            duck_att: 1.0,
            duck_rel: 1.0,
            post: Aequalis::new(fs),
            post_active: false,
            wl: [0.0; CHUNK],
            wr: [0.0; CHUNK],
            gain_dry: [1.0; CHUNK],
            gain_wet: [0.0; CHUNK],
            gain_duck: [1.0; CHUNK],
            width_now: [1.0; CHUNK],
            meter: SpatiumMeter::default(),
            fall: 10f32.powf(-20.0 / 20.0 / fs),
            corr: [0.0; 3],
            corr_coef: 1.0 - (-1.0 / (0.3 * fs)).exp(),
        };
        let d = SpatiumSettings::default();
        s.set(&d);
        s.reset();
        s
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn settings(&self) -> &SpatiumSettings {
        &self.settings
    }

    /// The late field's current design (lengths and fitted filters).
    pub fn design(&self) -> &LateDesign {
        &self.design
    }

    /// Clear every buffer and land every glide on its target.
    pub fn reset(&mut self) {
        self.fdn.reset();
        self.spring.reset();
        for b in &mut self.buf {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        self.in_w = 0;
        for side in &mut self.diff {
            for a in side.iter_mut() {
                a.clear();
            }
        }
        self.er_lp = [0.0; 2];
        for g in [
            &mut self.predelay,
            &mut self.gap,
            &mut self.diff_g,
            &mut self.er_gain,
            &mut self.late_gain,
            &mut self.in_gain,
            &mut self.mix,
            &mut self.width,
        ] {
            g.snap();
        }
        for side in 0..2 {
            for k in 0..ER_TAPS {
                self.er_t[side][k].snap();
                self.er_g[side][k].snap();
            }
        }
        self.duck_env = 0.0;
        self.duck_gr = 0.0;
        self.post.reset();
        self.corr = [0.0; 3];
        self.meter.wet_peak = [0.0; 2];
        self.meter.in_peak = 0.0;
    }

    /// Samples of delay this adds to the signal: none.
    pub fn latency(&self) -> usize {
        0
    }

    /// New settings. Called with every snapshot: the late field is re-fitted only when
    /// something that decides it changed.
    pub fn set(&mut self, s: &SpatiumSettings) {
        let fs = self.sample_rate;
        let first = !self.configured;
        let ms = fs * 0.001;
        let room = room_at(s.space);
        if first || s.late_key() != self.settings.late_key() {
            let curve = s.target_curve();
            if s.style == Style::Spring {
                let size = SPRING_MS.0 * (SPRING_MS.1 / SPRING_MS.0).powf(s.space.clamp(0.0, 1.0));
                let d = SpringDesign::new(&self.absorber, &curve, size);
                self.spring.set_design(&d);
                self.spring_design = Some(d);
            }
            let lengths =
                crate::spatium::fdn::prime_lengths(&line_lengths_ms::<LINES>(&room, s.style), fs);
            self.design = LateDesign::new(&self.absorber, &curve, &lengths);
            self.fdn.set_design(&self.design);
            for (a, hz) in self.meter.achieved.iter_mut().zip(OCTAVES) {
                *a = if s.style == Style::Spring {
                    curve.t60(hz)
                } else {
                    self.design.achieved_t60(hz)
                };
            }
        }
        let (depth_ms, rate) = modulation(s.style, s.character);
        self.fdn.set_modulation(depth_ms * ms, rate);
        self.fdn.set_freeze(s.freeze);
        self.spring.set_freeze(s.freeze);
        self.in_gain.target = if s.freeze { 0.0 } else { 1.0 };

        // Early reflections.
        let er = er_pattern(
            &room,
            s.style,
            s.er_size.clamp(0.25, ER_SIZE_MAX),
            s.character,
        );
        let er_lin = match er_level_db(s.style, s.distance, s.er_db) {
            Some(db) if s.style != Style::Spring => 10f32.powf(db / 20.0),
            _ => 0.0,
        };
        for side in 0..2 {
            for k in 0..ER_TAPS {
                if k < er.count {
                    self.er_t[side][k].target = er.t_ms[side][k] * ms;
                    self.er_g[side][k].target = er.gain[side][k];
                } else {
                    self.er_g[side][k].target = 0.0;
                }
            }
        }
        self.er_gain.target = er_lin;
        let fc = er_tone_hz(s.distance).min(0.45 * fs);
        self.er_lp_coef = 1.0 - (-2.0 * std::f32::consts::PI * fc / fs).exp();

        // Late field: onset, diffusion, level.
        self.gap.target = late_gap_ms(&room, s.distance) * ms;
        self.diff_g.target = if s.style == Style::Spring {
            0.0
        } else {
            diffusion(&room, s.style, s.distance, s.character)
        };
        self.predelay.target = s.predelay_ms.clamp(0.0, MAX_PREDELAY_MS) * ms;
        let mean_loop = if s.style == Style::Spring {
            self.spring_design
                .as_ref()
                .map(|d| d.mean_loop)
                .unwrap_or(1000.0)
        } else {
            self.design.lengths.iter().sum::<f32>() / LINES as f32
        };
        self.late_gain.target = late_gain(fs, mean_loop, s.decay_s, s.style)
            * 10f32.powf(late_level_db(s.distance) / 20.0);

        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.width.target = s.width.clamp(0.0, 1.5);

        // Ducking.
        self.duck_att = 1.0 - (-1.0 / (s.duck_att_ms.max(0.1) * ms)).exp();
        self.duck_rel = 1.0 - (-1.0 / (s.duck_rel_ms.max(1.0) * ms)).exp();

        // The wet EQ.
        let (bands, comp) = s.wet_eq();
        self.post.set(&bands, comp);
        self.post_active = self.post.is_active();

        self.settings = *s;
        self.configured = true;
        if first {
            self.reset();
        }
    }

    /// What the editor draws.
    pub fn meter(&self) -> SpatiumMeter {
        self.meter
    }

    /// Process a stereo block in place. `key` is the sidechain, when one is routed: the
    /// ducking listens to it instead of the input.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        let mut pos = 0;
        self.meter.keyed = key.is_some();
        while pos < n {
            let m = CHUNK.min(n - pos);
            let k = key.map(|(kl, kr)| {
                let e = (pos + m).min(kl.len()).min(kr.len());
                let s = pos.min(e);
                (&kl[s..e], &kr[s..e])
            });
            self.run(&left[pos..pos + m], &right[pos..pos + m], k);
            // An inactive EQ touches nothing but still counts the samples, so a band
            // switched on later lands on the same update grid whatever the block size.
            self.post.process(&mut self.wl[..m], &mut self.wr[..m]);
            self.finish(&mut left[pos..pos + m], &mut right[pos..pos + m]);
            pos += m;
        }
    }

    /// The first pass over a chunk: the reverb into the wet scratch, and the per-sample
    /// gains the second pass applies.
    fn run(&mut self, left: &[f32], right: &[f32], key: Option<(&[f32], &[f32])>) {
        let spring = self.settings.style == Style::Spring;
        let duck_on = self.settings.duck_db > 0.0;
        let (duck_range, duck_thr) = (self.settings.duck_db, self.settings.duck_thr_db);
        let (kg, kt) = (self.k_gain, self.k_time);
        let mask = self.in_mask;
        for j in 0..left.len() {
            let (dl, dr) = (left[j], right[j]);
            // Ducking: the key's peak (or the input's), a threshold, a range.
            let det = match key {
                Some((kl, kr)) if j < kl.len() && j < kr.len() => kl[j].abs().max(kr[j].abs()),
                _ => dl.abs().max(dr.abs()),
            };
            self.duck_env = if det > self.duck_env {
                det
            } else {
                self.duck_env * self.duck_det
            };
            if self.duck_env < 1e-12 {
                self.duck_env = 0.0;
            }
            self.meter.in_peak = if det > self.meter.in_peak {
                det
            } else {
                self.meter.in_peak * self.fall
            };
            let gd = if duck_on {
                let level = if self.duck_env > 1e-9 {
                    20.0 * self.duck_env.log10()
                } else {
                    -180.0
                };
                let want = (level - duck_thr).clamp(0.0, duck_range);
                let k = if want > self.duck_gr {
                    self.duck_att
                } else {
                    self.duck_rel
                };
                self.duck_gr += (want - self.duck_gr) * k;
                if self.duck_gr < 1e-5 {
                    self.duck_gr = 0.0;
                }
                if self.duck_gr > 0.0 {
                    10f32.powf(-self.duck_gr / 20.0)
                } else {
                    1.0
                }
            } else {
                self.duck_gr = 0.0;
                1.0
            };
            self.gain_duck[j] = gd;
            let (gdry, gwet) = mix_gains(self.mix.next(kg, 1e-6));
            self.gain_dry[j] = gdry;
            self.gain_wet[j] = gwet;
            self.width_now[j] = self.width.next(kg, 1e-6);

            // The input, gated by freeze, into the pre-delay buffers.
            let gin = self.in_gain.next(kg, 1e-6);
            let (il, ir) = (dl * gin, dr * gin);
            let w = self.in_w;
            self.buf[0][w] = il;
            self.buf[1][w] = ir;
            self.buf[2][w] = il + 0.3 * ir;
            self.buf[3][w] = ir + 0.3 * il;
            let pd = self.predelay.next(kt, 1e-3);
            let read = |b: &Vec<f32>, d: f32| -> f32 {
                let nf = d.floor();
                let f = d - nf;
                let n = nf as usize;
                let a = b[w.wrapping_sub(n) & mask];
                if f == 0.0 {
                    a
                } else {
                    let c = b[w.wrapping_sub(n + 1) & mask];
                    a + (c - a) * f
                }
            };

            // Early reflections.
            let eg = self.er_gain.next(kg, 1e-7);
            let mut er = [0.0f32; 2];
            if eg > 0.0 || self.er_gain.target > 0.0 {
                for (side, e) in er.iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for k in 0..ER_TAPS {
                        let g = self.er_g[side][k].next(kg, 1e-7);
                        let t = self.er_t[side][k].next(kt, 1e-3);
                        if g != 0.0 {
                            acc += g * read(&self.buf[2 + side], pd + t);
                        }
                    }
                    let y = &mut self.er_lp[side];
                    *y += (acc - *y) * self.er_lp_coef;
                    if y.abs() < 1e-18 {
                        *y = 0.0;
                    }
                    *e = *y * eg;
                }
            }

            // The late field's input: after the gap, through the diffusers.
            let gap = self.gap.next(kt, 1e-3);
            let mut ll = read(&self.buf[0], pd + gap);
            let mut lr = read(&self.buf[1], pd + gap);
            let dg = self.diff_g.next(kg, 1e-6);
            if dg > 0.0 {
                for a in self.diff[0].iter_mut() {
                    ll = a.tick(ll, dg);
                }
                for a in self.diff[1].iter_mut() {
                    lr = a.tick(lr, dg);
                }
            }
            let (late_l, late_r) = if spring {
                self.spring.tick(0.5 * (ll + lr))
            } else {
                self.fdn.tick(ll, lr)
            };
            let lg = self.late_gain.next(kg, 1e-7);
            self.wl[j] = er[0] + late_l * lg;
            self.wr[j] = er[1] + late_r * lg;
            self.in_w = (w + 1) & mask;
        }
    }

    /// The second pass: width, ducking, meters, and the mix into the output.
    fn finish(&mut self, left: &mut [f32], right: &mut [f32]) {
        for j in 0..left.len() {
            let (mut l, mut r) = (self.wl[j], self.wr[j]);
            let wd = self.width_now[j];
            if wd != 1.0 {
                let mid = 0.5 * (l + r);
                let side = 0.5 * (l - r) * wd;
                l = mid + side;
                r = mid - side;
            }
            let g = self.gain_duck[j];
            l *= g;
            r *= g;
            // Meters on the wet signal as it leaves.
            let pk = &mut self.meter.wet_peak;
            pk[0] = if l.abs() > pk[0] {
                l.abs()
            } else {
                pk[0] * self.fall
            };
            pk[1] = if r.abs() > pk[1] {
                r.abs()
            } else {
                pk[1] * self.fall
            };
            let c = self.corr_coef;
            self.corr[0] += (l * r - self.corr[0]) * c;
            self.corr[1] += (l * l - self.corr[1]) * c;
            self.corr[2] += (r * r - self.corr[2]) * c;
            // Mix. At zero wet the output is the input, untouched.
            let gw = self.gain_wet[j];
            if gw != 0.0 {
                let gdry = self.gain_dry[j];
                left[j] = left[j] * gdry + l * gw;
                right[j] = right[j] * gdry + r * gw;
            }
        }
        for c in &mut self.corr {
            if c.abs() < 1e-30 {
                *c = 0.0;
            }
        }
        let denom = (self.corr[1] * self.corr[2]).sqrt();
        self.meter.correlation = if denom > 1e-20 {
            (self.corr[0] / denom).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.meter.duck_db = self.duck_gr;
        self.meter.freeze = self.fdn.freeze_amount();
        self.meter.predelay_ms = self.predelay.now / (self.sample_rate * 0.001);
        self.meter.line_ms = self.fdn.mean_length() / (self.sample_rate * 0.001);
        self.meter.mod_depth_ms = self.fdn.depth() / (self.sample_rate * 0.001);
    }
}

/// The late field's output gain for a mean loop length (samples) and decay: calibrated so
/// the steady-state wet power follows √(T60 / 2 s) of the input's — longer decays are
/// louder, as in a room, but by half as many dB as the energy alone would make them.
pub fn late_gain(sample_rate: f32, mean_loop: f32, decay_s: f32, style: Style) -> f32 {
    let t_ref = (decay_s.max(0.05) * 2.0).sqrt();
    let lines = if style == Style::Spring {
        crate::spatium::spring::SPRINGS as f32
    } else {
        LINES as f32
    };
    (lines * 13.8 * mean_loop / (sample_rate * t_ref)).sqrt() * LATE_CAL
}

#[cfg(test)]
#[path = "spatium_tests.rs"]
mod tests;
