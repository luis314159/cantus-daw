//! Vetus's unit (ADR 0130): the six modules in their order, their switches'
//! crossfades, the 16-sample grid of the song where coefficients are designed and filters
//! settled, the rest predicate and the telemetry. The modules' maths are in `vetus_*.rs`.

use super::age::{self, Age};
use super::digital::{Bits, BitsMode, Rate};
use super::filter::{Svf, SvfCoef};
use super::noise::Noise;
use super::VetusSettings;
use crate::math;
use crate::modfx_clock::{peak_db, peak_of, synced_inc, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate as LfoRate, Shape};
use crate::modfx_line::{Glide, ModLine};
use crate::vibrans::{sweep_for, MAX_SWEEP_S};

/// The grid of the song, samples: coefficients are designed and filters settled at the
/// start of each of its cells.
const GRID: i64 = 16;
const SNAP: f32 = 1e-6;
/// Samples kept beyond the sweep for the interpolator.
const MARGIN: f32 = 2.0;
/// The modules, in order.
const BITS: usize = 0;
const RATE: usize = 1;
const WOB: usize = 2;
const AGE: usize = 3;
const NOISE: usize = 4;
const TONE: usize = 5;
/// The noise follows the music fully from −30 dBFS up.
const FOLLOW_FULL: f32 = 31.6;
const LFO_SEED: u64 = 0x5EED_7E75;

#[path = "vetus_run.rs"]
mod run;

/// Vetus's own telemetry, after the modulation family's header (`modfx_clock::tel`).
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// The modules sounding now, a bit each in the signal's order.
    pub const ON: usize = OWN;
    /// The bits and the sample rate after Character.
    pub const BITS: usize = OWN + 1;
    pub const RATE: usize = OWN + 2;
    /// The wow's and the flutter's positions now (−1 … 1).
    pub const WOW: usize = OWN + 3;
    pub const FLUT: usize = OWN + 4;
    /// The speed's deviation now, cents.
    pub const SPEED_CT: usize = OWN + 5;
    /// The age's gain now (dB, ≤ 0) and brightness (0 … 1).
    pub const DROP_DB: usize = OWN + 6;
    pub const BRIGHT: usize = OWN + 7;
    /// Clicks and pops in the last block; the noise's peak there, dBFS.
    pub const CLICKS: usize = OWN + 8;
    pub const NOISE_DB: usize = OWN + 9;
    /// The tone's edges now, Hz.
    pub const LO: usize = OWN + 10;
    pub const HI: usize = OWN + 11;
    pub const CHARACTER: usize = OWN + 12;
    /// The record's turn (0 … 1) at 33⅓ a minute, from the song position.
    pub const TURN: usize = OWN + 13;
    /// The follower's level (linear).
    pub const ENV: usize = OWN + 14;
}

#[derive(Clone, Debug)]
pub struct Vetus {
    sr: f32,
    set: VetusSettings,
    eff: VetusSettings,
    clock: Clock,
    /// Each module's share now (its switch, crossfaded).
    on: [Glide; 6],
    bits: Bits,
    dc: [Svf; 2],
    dc_c: SvfCoef,
    rate: Rate,
    /// The rate, log2 Hz, glided on the grid.
    rate_l: Glide,
    lines: [ModLine; 2],
    wob_live: bool,
    wow: Lfo,
    drift: Lfo,
    flut: Lfo,
    amp_w: Glide,
    amp_f: Glide,
    base: Glide,
    age: Age,
    age_lp: [Svf; 2],
    age_c: SvfCoef,
    /// The age's gain at the start and the end of the grid cell; its brightness.
    age_g: (f32, f32),
    age_b: f32,
    noise: Noise,
    env: f32,
    env_att: f32,
    env_rel: f32,
    tone: [[Svf; 5]; 2],
    tone_c: [SvfCoef; 5],
    lo_l: Glide,
    hi_l: Glide,
    res: Glide,
    /// The tone must be designed again at the next cell.
    dirty: bool,
    /// The grid cell the coefficients belong to.
    cell: i64,
    mix: Glide,
    out: Glide,
    tel: [f32; 64],
    blocks: u32,
}

/// `a` towards `b` by `g` (exactly `b` at 1).
#[inline]
fn blend(a: [f32; 2], b: [f32; 2], g: f32) -> [f32; 2] {
    if g >= 1.0 {
        b
    } else {
        [a[0] + (b[0] - a[0]) * g, a[1] + (b[1] - a[1]) * g]
    }
}

impl Vetus {
    /// fx/final — every glide at its target: what the last `set` asked for, taken at once
    /// (`FxUnit::snap`: a start, a seek, a loop's wrap). The sound it holds is kept.
    pub fn snap(&mut self) {
        for g in self.on.iter_mut() {
            g.jump(g.target);
        }
        self.rate_l.jump(self.rate_l.target);
        self.amp_w.jump(self.amp_w.target);
        self.amp_f.jump(self.amp_f.target);
        self.base.jump(self.base.target);
        self.lo_l.jump(self.lo_l.target);
        self.hi_l.jump(self.hi_l.target);
        self.res.jump(self.res.target);
        self.mix.jump(self.mix.target);
        self.out.jump(self.out.target);
    }

    pub fn new(sample_rate: f32) -> Vetus {
        let sr = sample_rate;
        let grid_sr = sr / GRID as f32;
        let max = (MAX_SWEEP_S * 2.0 * sr) as usize + 16;
        let set = VetusSettings::default();
        let mut v = Vetus {
            sr,
            set,
            eff: set,
            clock: Clock::new(),
            on: [Glide::new(0.0, 10.0 / 3.0, sr, SNAP); 6],
            bits: Bits::new(set.bits, false, BitsMode::Round),
            dc: [Svf::default(); 2],
            dc_c: SvfCoef::high(8.0, 0.5, sr),
            rate: Rate::new(sr),
            rate_l: Glide::new(math::log2(set.rate_hz), 15.0, grid_sr, 1e-5),
            lines: [ModLine::new(max), ModLine::new(max)],
            wob_live: false,
            wow: Lfo::new(sr),
            drift: Lfo::new(sr),
            flut: Lfo::new(sr),
            amp_w: Glide::new(0.0, 60.0, sr, 1e-4),
            amp_f: Glide::new(0.0, 60.0, sr, 1e-4),
            base: Glide::new(0.0, 60.0, sr, 1e-4),
            age: Age::new(sr),
            age_lp: [Svf::default(); 2],
            age_c: SvfCoef::default(),
            age_g: (1.0, 1.0),
            age_b: 1.0,
            noise: Noise::new(sr),
            env: 0.0,
            env_att: 1.0 - math::exp(-1.0 / (0.005 * sr)),
            env_rel: 1.0 - math::exp(-1.0 / (0.3 * sr)),
            tone: [[Svf::default(); 5]; 2],
            tone_c: [SvfCoef::default(); 5],
            lo_l: Glide::new(math::log2(set.lo), 15.0, grid_sr, 1e-5),
            hi_l: Glide::new(math::log2(set.hi), 15.0, grid_sr, 1e-5),
            res: Glide::new(set.res, 15.0, grid_sr, 1e-5),
            dirty: true,
            cell: i64::MIN,
            mix: Glide::new(1.0, 10.0, sr, SNAP),
            out: Glide::new(1.0, 10.0, sr, SNAP),
            tel: [0.0; 64],
            blocks: 0,
        };
        v.set(&set);
        v.reset();
        v
    }

    pub fn settings(&self) -> &VetusSettings {
        &self.set
    }

    /// The settings after Character: what is heard.
    pub fn effective(&self) -> &VetusSettings {
        &self.eff
    }

    pub fn set(&mut self, s: &VetusSettings) {
        let e = s.aged();
        self.set = *s;
        self.eff = e;
        for (g, on) in self.on.iter_mut().zip(e.switches()) {
            g.target = if on { 1.0 } else { 0.0 };
        }
        self.bits = Bits::new(e.bits, e.dither, e.bits_mode);
        self.rate_l.target = math::log2(e.rate_hz.clamp(1_000.0, self.sr));
        let wow = LfoSettings {
            rate: e.wow_rate,
            seed: LFO_SEED,
            ..LfoSettings::default()
        };
        self.wow.set(&wow, &self.clock);
        // The wandering part: random values joined smoothly, twice as slow as the wow.
        let drift = LfoSettings {
            rate: match e.wow_rate {
                LfoRate::Hz(h) => LfoRate::Hz(h * 0.5),
                LfoRate::Beats(b) => LfoRate::Beats(b * 2.0),
            },
            shape: Shape::Random,
            edge: 1.0,
            seed: LFO_SEED ^ 0xD81F,
            ..LfoSettings::default()
        };
        self.drift.set(&drift, &self.clock);
        let flut = LfoSettings {
            rate: LfoRate::Hz(f64::from(e.flut_hz)),
            seed: LFO_SEED ^ 0xF1,
            ..LfoSettings::default()
        };
        self.flut.set(&flut, &self.clock);
        self.retarget();
        self.mix.target = e.mix.clamp(0.0, 1.0);
        self.out.target = if e.out_db == 0.0 {
            1.0
        } else {
            math::db_to_gain(e.out_db)
        };
        self.lo_l.target = math::log2(e.lo.clamp(10.0, 0.45 * self.sr));
        self.hi_l.target = math::log2(e.hi.clamp(100.0, 0.45 * self.sr));
        self.res.target = e.res.clamp(0.0, 1.0);
        self.dirty = true;
    }

    /// The wow's rate now, Hz.
    fn wow_hz(&self) -> f64 {
        match self.eff.wow_rate {
            LfoRate::Hz(h) => h,
            LfoRate::Beats(b) => {
                synced_inc(b, self.clock.tempo(), f64::from(self.sr)) * f64::from(self.sr)
            }
        }
    }

    /// The wobble's sweeps and centre for the settings and the tempo.
    fn retarget(&mut self) {
        let (aw, af) = if self.eff.wob_on {
            (
                sweep_for(self.eff.wow_ct, self.wow_hz(), Shape::Sine, self.sr),
                sweep_for(
                    self.eff.flut_ct,
                    f64::from(self.eff.flut_hz),
                    Shape::Sine,
                    self.sr,
                ),
            )
        } else {
            (0.0, 0.0)
        };
        self.amp_w.target = aw;
        self.amp_f.target = af;
        self.base.target = if aw + af > 0.0 { aw + af + MARGIN } else { 0.0 };
    }

    pub fn set_transport(&mut self, tr: &Transport) {
        let epoch = self.clock.epoch();
        self.clock.set(tr);
        for l in [&mut self.wow, &mut self.drift, &mut self.flut] {
            l.follow(&self.clock);
        }
        if matches!(self.eff.wow_rate, LfoRate::Beats(_)) {
            self.retarget();
            if self.clock.epoch() != epoch {
                for g in [&mut self.amp_w, &mut self.amp_f, &mut self.base] {
                    g.jump(g.target);
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        for l in [&mut self.wow, &mut self.drift, &mut self.flut] {
            l.reset();
        }
        for l in &mut self.lines {
            l.reset();
        }
        self.wob_live = false;
        self.retarget();
        for g in self.on.iter_mut().chain([
            &mut self.rate_l,
            &mut self.amp_w,
            &mut self.amp_f,
            &mut self.base,
            &mut self.lo_l,
            &mut self.hi_l,
            &mut self.res,
            &mut self.mix,
            &mut self.out,
        ]) {
            g.jump(g.target);
        }
        self.rate.reset();
        self.clear_filters();
        self.noise.reset();
        self.env = 0.0;
        self.cell = i64::MIN;
        self.dirty = true;
        self.tel = [0.0; 64];
        self.blocks = 0;
    }

    fn clear_filters(&mut self) {
        for f in self
            .dc
            .iter_mut()
            .chain(self.age_lp.iter_mut())
            .chain(self.tone.iter_mut().flatten())
        {
            f.reset();
        }
    }

    pub fn latency(&self) -> usize {
        0
    }
}
