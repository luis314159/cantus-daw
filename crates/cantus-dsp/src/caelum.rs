//! **Caelum**, the shimmer reverb (fx side round, wave A; the name is provisional — one
//! constant, [`NAME`]) — Latin *caelum*, the sky: the reverb for spaces no building holds.
//! Spatium makes rooms, Aula plays real ones; Caelum makes what neither can — tails that
//! climb in pitch (**shimmer**), washes that never end (**infinite**), a sound that swells
//! into its space (**bloom**), a held chord (**freeze**), and a wet that steps aside while
//! the dry plays (**ducking**).
//!
//! ```text
//!        ┌──────────────────────────── dry ────────────────────────────────────────┐
//!  in ─┬─ pre-delay ─ bloom taps ─ diffusers ─ freeze gate ─ tank ─ wet tone ─ width ─ duck ─ mix ─ out
//!      │                                          (8 or 16 lines, ↺ shimmer)       │
//!      └─ (or the sidechain key) ─ ducking detector ───────────────────────────────┘
//! ```
//!
//! - The way in ([`input`]): the pre-delay (ms; the engine converts a synced value with
//!   [`sync_ms`]), the bloom, four modulated diffusers a side, the freeze gate.
//! - The tank ([`tank`]): 8 or 16 modulated lines, an all-pass in each, the Hadamard
//!   matrix, Jot's absorption in three bands (decay, high damping, low damping), infinite,
//!   freeze, and the shimmer's exchange through the shifters ([`shift`]).
//! - The wet: a level that follows the decay (a quarter of the dB its energy would),
//!   a low cut and a high cut, width, ducking, and Spatium's mix law.
//!
//! ## The contract
//!
//! Allocation-free after `new`; deterministic ([`crate::math`] only); everything runs per
//! sample, so the output does not depend on the block size; **at mix 0 the output is the
//! input to the bit** (the tank still runs, so its tail is where it would be when the mix
//! comes back). The latency is zero: the wet's delay is the pre-delay, the dry is never
//! delayed. **At rest exactly**: once the input has been silent for as long as the way in
//! reaches and the tank has held nothing above −120 dB for as long as its longest line, it
//! is drained to +0.0 (one line's length of writes) and then it is a new unit, skipped.

#![allow(clippy::needless_range_loop)]

use crate::dynfx_follow::{Follower, Quiet, Times};
use crate::math;

#[path = "caelum_input.rs"]
pub mod input;
#[path = "caelum_parts.rs"]
pub mod parts;
#[path = "caelum_presets.rs"]
pub mod presets;
#[path = "caelum_shift.rs"]
pub mod shift;
#[path = "caelum_tank.rs"]
pub mod tank;

#[cfg(test)]
#[path = "caelum_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "caelum_tests_measure.rs"]
mod tests_measure;

use input::InputStage;
use parts::{Svf2, SvfCoef};
use tank::{Decay, Shimmer, Tank, TankDesign};

pub use tank::Lines;

/// The unit's name, in one place (provisional).
pub const NAME: &str = "Caelum";

/// The decay's range, s (above the top is [`CaelumSettings::infinite`]).
pub const DECAY_MIN_S: f32 = 0.3;
pub const DECAY_MAX_S: f32 = 60.0;
/// The decay the wet's level follows at most, s.
const LEVEL_MAX_S: f32 = 30.0;
/// The governor's ceiling: the wet's RMS, linear (−6 dBFS).
const CEILING_RMS: f32 = 0.5;
/// An infinite tank's level: an 8 s tank's, 8 dB down — it keeps everything it is fed,
/// so a bar or two of playing piles up to where an 8 s tail would stand.
const LEVEL_INFINITE_S: f32 = 8.0;
const LEVEL_INFINITE_TRIM: f32 = 0.4;
/// Calibration of the wet's level (see [`late_gain`]), measured on pink noise.
const LATE_CAL: f32 = 0.89;
/// Octave centres the meter reports the achieved decay at, Hz.
pub const OCTAVES: [f32; 8] = [
    63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0,
];

/// A synced pre-delay: `beats` (a quarter note is 1) at `bpm`, ms, clamped to the range.
pub fn sync_ms(beats: f32, bpm: f32) -> f32 {
    (beats.max(0.0) * 60_000.0 / bpm.max(1.0)).min(input::MAX_PREDELAY_MS)
}

/// Everything Caelum is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CaelumSettings {
    /// 0…1: the tank's lines from 15 ms to 320 ms on average.
    pub size: f32,
    /// The middle band's RT60, s ([`DECAY_MIN_S`]…[`DECAY_MAX_S`]).
    pub decay_s: f32,
    /// The middle held for good (the damping still darkens).
    pub infinite: bool,
    pub predelay_ms: f32,
    /// 0…1: the diffusers' and the lines' all-passes' gain.
    pub diffusion: f32,
    /// High damping: the corner (Hz) and the highs' decay as a fraction of the middle's.
    pub high_hz: f32,
    pub high_decay: f32,
    /// Low damping: the corner (Hz) and the lows' decay as a multiple of the middle's.
    pub low_hz: f32,
    pub low_decay: f32,
    /// Modulation: depth 0…1 (to 4 ms) and rate, Hz.
    pub mod_depth: f32,
    pub mod_rate_hz: f32,
    /// Shimmer: amount 0…1, the two voices' shifts (semitones), voice B's level 0…1, the
    /// tone in the loop (a low cut and a high cut, Hz).
    pub shimmer: f32,
    pub pitch_a: f32,
    pub pitch_b: f32,
    pub pitch_b_level: f32,
    pub shimmer_low_hz: f32,
    pub shimmer_high_hz: f32,
    /// Bloom time, ms (0 is off).
    pub bloom_ms: f32,
    pub freeze: bool,
    /// Ducking: range (0 is off), threshold, attack, release.
    pub duck_db: f32,
    pub duck_thr_db: f32,
    pub duck_att_ms: f32,
    pub duck_rel_ms: f32,
    /// The wet's tone: a low cut (at or below 20 Hz off) and a high cut (at or above
    /// 20 kHz off).
    pub low_cut_hz: f32,
    pub high_cut_hz: f32,
    /// 0 mono … 1 as made … 1.5 wider.
    pub width: f32,
    pub lines: Lines,
    /// Dry/wet, 0…1: the dry stays at unity to 0.5, the wet reaches unity at 0.5.
    pub mix: f32,
}

impl Default for CaelumSettings {
    /// The parameters' defaults: a null (mix 0) over a musical space.
    fn default() -> Self {
        CaelumSettings {
            size: 0.6,
            decay_s: 4.0,
            infinite: false,
            predelay_ms: 20.0,
            diffusion: 0.75,
            high_hz: 6_000.0,
            high_decay: 0.5,
            low_hz: 250.0,
            low_decay: 1.0,
            mod_depth: 0.3,
            mod_rate_hz: 0.4,
            shimmer: 0.0,
            pitch_a: 12.0,
            pitch_b: 7.0,
            pitch_b_level: 0.0,
            shimmer_low_hz: 300.0,
            shimmer_high_hz: 9_000.0,
            bloom_ms: 0.0,
            freeze: false,
            duck_db: 0.0,
            duck_thr_db: -30.0,
            duck_att_ms: 10.0,
            duck_rel_ms: 400.0,
            low_cut_hz: 20.0,
            high_cut_hz: 20_000.0,
            width: 1.0,
            lines: Lines::Sixteen,
            mix: 0.0,
        }
    }
}

impl CaelumSettings {
    fn decay(&self) -> Decay {
        Decay {
            t60_s: self.decay_s.clamp(DECAY_MIN_S, DECAY_MAX_S),
            infinite: self.infinite,
            high_hz: self.high_hz,
            high_ratio: self.high_decay,
            low_hz: self.low_hz,
            low_ratio: self.low_decay,
        }
    }

    /// Everything that decides the tank's design.
    fn design_key(&self) -> (f32, Decay, Lines) {
        (self.size, self.decay(), self.lines)
    }

    fn shimmer(&self) -> Shimmer {
        Shimmer {
            amount: self.shimmer,
            a_st: self.pitch_a,
            b_st: self.pitch_b,
            b_level: self.pitch_b_level,
            low_hz: self.shimmer_low_hz,
            high_hz: self.shimmer_high_hz,
        }
    }
}

/// The two halves of the mix law (Spatium's): dry at unity to 50 %, wet at unity from 50 %.
pub fn mix_gains(mix: f32) -> (f32, f32) {
    crate::spatium::mix_gains(mix)
}

/// The wet's gain for a tank of `lines` lines whose mean loop is `mean_loop` samples and
/// whose decay is `t60` s: calibrated so the wet's steady-state power is the input's times
/// `(T/2 s)^¼` — longer decays are louder, by a quarter of the dB their energy alone
/// would make them (Spatium's half is too much for a minute-long wash).
///
/// In a lossless-matrix tank, the stored energy is the injected power times the energy's
/// mean life `fs·T/13.8`, and it leaves the lines at `1/m̄` of it a sample; each output row
/// sees `1/N` of that, and both inputs feed it.
pub fn late_gain(sample_rate: f32, lines: usize, mean_loop: f32, t60: f32) -> f32 {
    let t = t60.clamp(DECAY_MIN_S, LEVEL_MAX_S);
    let w = math::powf(t / 2.0, 0.25);
    (w * 13.8 * lines as f32 * mean_loop / (2.0 * t * sample_rate)).sqrt() * LATE_CAL
}

/// What the editor draws from the engine, refreshed every block.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CaelumMeter {
    /// Wet peaks, L and R, linear, held and falling ~20 dB/s.
    pub wet_peak: [f32; 2],
    /// The input's (or the key's) peak, same ballistics.
    pub in_peak: f32,
    /// Ducking's gain reduction now, dB (positive).
    pub duck_db: f32,
    /// 0…1.
    pub freeze: f32,
    /// The bloom's share now, 0…1.
    pub bloom: f32,
    /// The tank's energy, dB (smoothed over 100 ms).
    pub energy_db: f32,
    /// What the shimmer gives back, as a share of the tank's energy, 0…1.
    pub shimmer: f32,
    /// The governor's and the hold's trim now, dB (0: none).
    pub trim_db: f32,
    pub predelay_ms: f32,
    /// The lines' mean length now, ms.
    pub line_ms: f32,
    pub mod_depth_ms: f32,
    pub keyed: bool,
    /// The decay the tank achieves at each of [`OCTAVES`], s (`f32::INFINITY` held).
    pub achieved: [f32; OCTAVES.len()],
}

/// A glide that lands (Spatium's).
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
    fn next(&mut self, k: f32, eps: f32) -> f32 {
        crate::spatium::fdn::glide(&mut self.now, self.target, k, eps);
        self.now
    }

    fn landed(&self) -> bool {
        self.now == self.target
    }
}

/// Caelum's processing state.
#[derive(Clone, Debug)]
pub struct Caelum {
    sample_rate: f32,
    settings: CaelumSettings,
    configured: bool,
    input: InputStage,
    tank: Tank,
    late_gain: Glide,
    mix: Glide,
    width: Glide,
    k_gain: f32,
    // The wet's tone.
    lc_on: bool,
    hc_on: bool,
    lc: [Svf2; 2],
    hc: [Svf2; 2],
    lc_c: SvfCoef,
    hc_c: SvfCoef,
    // Ducking.
    duck: Follower,
    duck_det: Times,
    duck_quiet: Quiet,
    duck_gr: f32,
    duck_att: f32,
    duck_rel: f32,
    /// Samples of silence in a row (both sides +0.0), for the drain.
    in_silent: usize,
    // Meters.
    meter: CaelumMeter,
    fall: f32,
}

impl Caelum {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate.max(8_000.0);
        let mut c = Caelum {
            sample_rate: fs,
            settings: CaelumSettings::default(),
            configured: false,
            input: InputStage::new(fs),
            tank: Tank::new(fs),
            late_gain: Glide::new(0.0),
            mix: Glide::new(0.0),
            width: Glide::new(1.0),
            k_gain: 1.0 - math::exp(-1.0 / (0.02 * fs)),
            lc_on: false,
            hc_on: false,
            lc: [Svf2::default(); 2],
            hc: [Svf2::default(); 2],
            lc_c: SvfCoef::new(20.0, fs),
            hc_c: SvfCoef::new(20_000.0, fs),
            duck: Follower::default(),
            duck_det: Times::new(0.0, 50.0, fs),
            duck_quiet: Quiet::default(),
            duck_gr: 0.0,
            duck_att: 1.0,
            duck_rel: 1.0,
            in_silent: usize::MAX / 2,
            meter: CaelumMeter::default(),
            fall: math::powf(10f32, -20.0 / 20.0 / fs),
        };
        let d = CaelumSettings::default();
        c.set(&d);
        c.reset();
        c
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn settings(&self) -> &CaelumSettings {
        &self.settings
    }

    /// The tank's current design (lengths and absorption).
    pub fn design(&self) -> &TankDesign {
        self.tank.design()
    }

    /// Samples of delay this adds to the signal: none.
    pub fn latency(&self) -> usize {
        0
    }

    /// What the editor draws.
    pub fn meter(&self) -> CaelumMeter {
        self.meter
    }

    /// Clear every buffer and land every glide on its target: a new unit.
    pub fn reset(&mut self) {
        self.input.reset();
        self.tank.reset();
        for g in [&mut self.late_gain, &mut self.mix, &mut self.width] {
            g.now = g.target;
        }
        for f in self.lc.iter_mut().chain(self.hc.iter_mut()) {
            f.clear();
        }
        self.duck.reset();
        self.duck_quiet.reset();
        self.duck_gr = 0.0;
        self.in_silent = usize::MAX / 2;
        self.meter.wet_peak = [0.0; 2];
        self.meter.in_peak = 0.0;
    }

    /// New settings. Called with every snapshot: the tank is redesigned only when something
    /// that decides it changed.
    pub fn set(&mut self, s: &CaelumSettings) {
        let fs = self.sample_rate;
        let first = !self.configured;
        let ms = fs * 0.001;
        if first || s.design_key() != self.settings.design_key() {
            let ap = tank::allpass_totals(&tank::allpass_lengths(fs));
            let d = TankDesign::new(fs, s.lines.count(), s.size, &s.decay(), &ap);
            self.tank.set_design(&d);
            for (a, hz) in self.meter.achieved.iter_mut().zip(OCTAVES) {
                *a = d.achieved_t60(hz);
            }
            let t = if s.infinite {
                LEVEL_INFINITE_S
            } else {
                s.decay_s
            };
            let lg = late_gain(fs, d.lines, d.mean_loop(), t)
                * if s.infinite { LEVEL_INFINITE_TRIM } else { 1.0 };
            self.late_gain.target = lg;
            // The ceiling, in the lines' summed energy: a wet RMS of `CEILING_RMS` on a side
            // is `N · rms² / gain²` of it (each output row sees 1/N of the lines).
            // The early taps add their share to every output row.
            let rows = 1.0 + tank::EARLY_GAIN * tank::EARLY_GAIN;
            self.tank.set_ceiling(
                d.lines as f32 * CEILING_RMS * CEILING_RMS / (lg * lg * rows).max(1e-12),
            );
        }
        self.tank.set_infinite(s.infinite);
        self.tank.set_modulation(
            s.mod_depth.clamp(0.0, 1.0) * tank::MAX_DEPTH_MS * ms,
            s.mod_rate_hz.clamp(0.02, 10.0),
        );
        self.tank.set_diffusion(0.7 * s.diffusion.clamp(0.0, 1.0));
        self.tank.set_shimmer(&s.shimmer());
        self.tank.set_freeze(s.freeze);
        self.input.set(
            s.predelay_ms,
            s.bloom_ms,
            0.8 * s.diffusion.clamp(0.0, 1.0),
            s.mod_depth,
            s.freeze,
        );
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.width.target = s.width.clamp(0.0, 1.5);
        self.lc_on = s.low_cut_hz > 20.0;
        self.hc_on = s.high_cut_hz < 20_000.0;
        self.lc_c = SvfCoef::new(s.low_cut_hz.max(20.0), fs);
        self.hc_c = SvfCoef::new(s.high_cut_hz.min(20_000.0), fs);
        self.duck_att = crate::dynfx_follow::coef(s.duck_att_ms.max(0.1), fs);
        self.duck_rel = crate::dynfx_follow::coef(s.duck_rel_ms.max(1.0), fs);
        self.settings = *s;
        self.configured = true;
        if first {
            self.input.snap();
            self.reset();
        }
    }
}

impl Caelum {
    /// Process a stereo block in place. `key` is the sidechain, when one is routed: the
    /// ducking listens to it instead of the input.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        self.meter.keyed = key.is_some();
        let kg = self.k_gain;
        let diffusers = (input::DIFFUSER_TOTAL_MS * 0.001 * self.sample_rate) as usize + 64;
        for j in 0..n {
            let (dl, dr) = (left[j], right[j]);
            let det = match key {
                Some((kl, kr)) if j < kl.len() && j < kr.len() => kl[j].abs().max(kr[j].abs()),
                _ => dl.abs().max(dr.abs()),
            };
            self.meter.in_peak = if det > self.meter.in_peak {
                det
            } else {
                self.meter.in_peak * self.fall
            };
            let gd = self.duck_gain(det);
            self.in_silent = if dl == 0.0 && dr == 0.0 {
                self.in_silent.saturating_add(1)
            } else {
                0
            };
            let arrived = self.in_silent > self.input.span() + diffusers;
            let drain = arrived && self.tank.quiet_long();
            let (il, ir) = self.input.tick(dl, dr, drain);
            let (tl, tr) = self.tank.tick(il, ir, !arrived, drain);
            let lg = self.late_gain.next(kg, 1e-7);
            let (mut l, mut r) = (tl * lg, tr * lg);
            if self.lc_on {
                l = self.lc[0].run(l, &self.lc_c).1;
                r = self.lc[1].run(r, &self.lc_c).1;
            }
            if self.hc_on {
                l = self.hc[0].run(l, &self.hc_c).0;
                r = self.hc[1].run(r, &self.hc_c).0;
            }
            if drain {
                for f in self.lc.iter_mut().chain(self.hc.iter_mut()) {
                    f.clear();
                }
            }
            let wd = self.width.next(kg, 1e-6);
            if wd != 1.0 {
                let mid = 0.5 * (l + r);
                let side = 0.5 * (l - r) * wd;
                l = mid + side;
                r = mid - side;
            }
            l *= gd;
            r *= gd;
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
            // The mix. At zero wet the output is the input, untouched.
            let (gdry, gwet) = mix_gains(self.mix.next(kg, 1e-6));
            if gwet != 0.0 {
                left[j] = dl * gdry + l * gwet;
                right[j] = dr * gdry + r * gwet;
            }
        }
        self.input.wrote(n);
        self.tank.wrote(n);
        self.finish_meter();
    }

    /// The ducking's gain for this sample. The detector is a peak with a 50 ms fall; the
    /// gain reduction it asks — the level over the threshold, up to the range — is reached
    /// at the attack's speed and let go at the release's, so the release is how fast the
    /// wet comes back.
    #[inline]
    fn duck_gain(&mut self, det: f32) -> f32 {
        let env = self.duck.run(det, &self.duck_det);
        if self.duck_quiet.note(det) {
            self.duck.settle(tank::SETTLE);
        }
        let range = self.settings.duck_db;
        if range > 0.0 {
            let level = if env > 1e-9 {
                20.0 * math::log10(env)
            } else {
                -180.0
            };
            let want = (level - self.settings.duck_thr_db).clamp(0.0, range);
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
                math::db_to_gain(-self.duck_gr)
            } else {
                1.0
            }
        } else {
            self.duck_gr = 0.0;
            1.0
        }
    }

    fn finish_meter(&mut self) {
        let ms = self.sample_rate * 0.001;
        let m = &mut self.meter;
        m.duck_db = self.duck_gr;
        m.freeze = self.tank.freeze_amount();
        m.bloom = self.input.bloom();
        let e = self.tank.energy();
        m.energy_db = if e > 1e-20 {
            10.0 * math::log10(e)
        } else {
            -200.0
        };
        m.shimmer = if e > 1e-20 {
            (self.tank.shimmer_power() / e).min(1.0)
        } else {
            0.0
        };
        let t = self.tank.trim();
        m.trim_db = if t != 1.0 {
            20.0 * math::log10(t.max(1e-6))
        } else {
            0.0
        };
        m.predelay_ms = self.input.predelay_samples() / ms;
        m.line_ms = self.tank.mean_length() / ms;
        m.mod_depth_ms = self.tank.depth() / ms;
    }

    /// At rest (ADR 0031 §5.1): with silence in (and a silent key) it gives exact silence
    /// out and keeps its state, so its calls may be skipped ([`Self::skip`]) — or it may be
    /// reset — without changing a sample. `None` when it is; else why not.
    pub fn rest_blocker(&self) -> Option<&'static str> {
        if let Some(why) = self.input.rest_blocker() {
            return Some(why);
        }
        if let Some(why) = self.tank.rest_blocker() {
            return Some(why);
        }
        if !self.lc.iter().chain(&self.hc).all(|f| f.is_zero()) {
            return Some("wet tone");
        }
        if !self.late_gain.landed() || !self.mix.landed() || !self.width.landed() {
            return Some("glides");
        }
        if !self.duck.is_zero() || self.duck_gr.to_bits() != 0 {
            return Some("ducking");
        }
        None
    }

    /// [`Self::rest_blocker`] is `None`.
    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    /// `n` samples of silence not run, at rest: the rings' positions and every phase move
    /// as processing would move them; the meters (the editor's only) fall to nothing.
    pub fn skip(&mut self, n: usize) {
        self.input.skip(n);
        self.tank.skip(n);
        self.in_silent = self.in_silent.saturating_add(n);
        self.duck_quiet.skip(n);
        self.meter.wet_peak = [0.0; 2];
        self.meter.in_peak = 0.0;
    }
}
