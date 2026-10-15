//! Flexus: the through-zero flanger (ADR 0055).
//!
//! A copy of the signal read from a line whose delay sweeps, mixed with the dry: the comb
//! of notches that moves is the flange. Three ways to move it — the family's **LFO**
//! (synced or free), the input's **envelope** (louder sweeps further), or a **manual**
//! position for automation — and **feedback** either way, the wet's polarity, the stereo
//! phase.
//!
//! **Through zero**: as two tape machines did, the dry path is delayed by a fixed centre
//! ([`TZ_CENTRE_MS`]) and the wet sweeps either side of it, so the moving copy passes
//! *through* the dry one — the deep cancellation at the crossing that a plain flanger never
//! reaches. The centre is the unit's latency, reported for delay compensation. At the
//! centre point the wet is read at a whole number of samples, the same sample the dry path
//! holds, so with the polarity inverted at 50 % the output is **exact silence** — the test
//! holds that to the bit.
//!
//! **Nulls**: at mix 0 (the default) the dry passes untouched (delayed by the centre in
//! through-zero mode, which delay compensation takes back).

use crate::modfx_clock::{peak_db, peak_of, tel, Clock, Transport};
use crate::modfx_lfo::{Lfo, LfoSettings, Rate, Shape};
use crate::modfx_line::{Glide, ModLine};

/// What moves the sweep (`flexus.mode.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Lfo,
    Envelope,
    Manual,
}

impl Mode {
    pub fn from_index(i: f32) -> Mode {
        match i.round() as i32 {
            1 => Mode::Envelope,
            2 => Mode::Manual,
            _ => Mode::Lfo,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FlexusSettings {
    pub mode: Mode,
    pub through_zero: bool,
    /// 0 … 1, the equal-gain crossfade (both full at ½).
    pub mix: f32,
    /// The sweep's shortest delay, ms (plain mode).
    pub delay_ms: f32,
    /// How far the sweep reaches, ms.
    pub depth_ms: f32,
    /// −0.95 … 0.95.
    pub feedback: f32,
    /// The wet copy upside down.
    pub invert: bool,
    pub lfo: LfoSettings,
    /// The right channel's LFO ahead of the left, cycles (0 … ½).
    pub spread: f32,
    /// The manual sweep position, 0 … 1.
    pub manual: f32,
    /// The envelope's attack and release, ms, and its gain into the sweep, dB.
    pub attack_ms: f32,
    pub release_ms: f32,
    pub sens_db: f32,
    pub out_db: f32,
}

impl Default for FlexusSettings {
    fn default() -> Self {
        FlexusSettings {
            mode: Mode::Lfo,
            through_zero: false,
            mix: 0.0,
            delay_ms: 0.5,
            depth_ms: 4.0,
            feedback: 0.0,
            invert: false,
            lfo: LfoSettings {
                rate: Rate::Hz(f64::from(0.25f32)),
                shape: Shape::Triangle,
                ..LfoSettings::default()
            },
            spread: 0.0,
            manual: 0.5,
            attack_ms: 5.0,
            release_ms: 200.0,
            sens_db: 24.0,
            out_db: 0.0,
        }
    }
}

/// The through-zero centre: the dry path's delay and the unit's latency.
pub const TZ_CENTRE_MS: f32 = 5.0;
/// The longest delay a read asks for, ms.
const MAX_MS: f32 = 20.0;
/// The shortest delay the wet is read at, samples (the interpolator's neighbours).
const MIN_READ: f32 = 1.0;
const SNAP: f32 = 1e-6;
const SETTLE: f32 = 1e-20;
const SETTLE_RUN: u32 = 64;

/// Flexus's own telemetry: the sweep's position (0 … 1), the wet's delay against the dry
/// (ms; negative before it, in through-zero), the envelope (dBFS), the latency (samples).
pub const T_SWEEP: usize = tel::OWN;
pub const T_DELAY_MS: usize = tel::OWN + 1;
pub const T_ENV_DB: usize = tel::OWN + 2;
pub const T_LATENCY: usize = tel::OWN + 3;

/// The through-zero centre at a sample rate, whole samples.
pub fn tz_centre(sample_rate: f32) -> usize {
    (TZ_CENTRE_MS * 0.001 * sample_rate).round() as usize
}

#[derive(Clone, Debug)]
pub struct Flexus {
    sr: f32,
    set: FlexusSettings,
    clock: Clock,
    lfo: Lfo,
    lines: [ModLine; 2],
    /// The dry path's line, through-zero.
    dry: [ModLine; 2],
    centre: usize,
    /// The last wet sample, for the feedback.
    back: [f32; 2],
    env: f32,
    k_att: f32,
    k_rel: f32,
    delay: Glide,
    depth: Glide,
    feedback: Glide,
    mix: Glide,
    out: Glide,
    spread: Glide,
    manual: Glide,
    sens: Glide,
    zrun: u32,
    tel: [f32; 16],
    blocks: u32,
}

impl Flexus {
    pub fn new(sample_rate: f32) -> Flexus {
        let max = (MAX_MS * 0.001 * sample_rate) as usize + 8;
        let centre = tz_centre(sample_rate);
        let mut f = Flexus {
            sr: sample_rate,
            set: FlexusSettings::default(),
            clock: Clock::new(),
            lfo: Lfo::new(sample_rate),
            lines: [ModLine::new(max), ModLine::new(max)],
            dry: [ModLine::new(centre + 4), ModLine::new(centre + 4)],
            centre,
            back: [0.0; 2],
            env: 0.0,
            k_att: 0.0,
            k_rel: 0.0,
            delay: Glide::new(0.0, 30.0, sample_rate, 1e-3),
            depth: Glide::new(0.0, 30.0, sample_rate, 1e-3),
            feedback: Glide::new(0.0, 20.0, sample_rate, SNAP),
            mix: Glide::new(0.0, 10.0, sample_rate, SNAP),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            spread: Glide::new(0.0, 30.0, sample_rate, SNAP),
            manual: Glide::new(0.5, 20.0, sample_rate, SNAP),
            sens: Glide::new(1.0, 20.0, sample_rate, SNAP),
            zrun: 0,
            tel: [0.0; 16],
            blocks: 0,
        };
        let s = f.set;
        f.set(&s);
        f.reset();
        f
    }

    pub fn settings(&self) -> &FlexusSettings {
        &self.set
    }

    pub fn set(&mut self, s: &FlexusSettings) {
        let ms = |v: f32| v * 0.001 * self.sr;
        self.delay.target = ms(s.delay_ms.clamp(0.05, 5.0)).max(MIN_READ);
        self.depth.target = ms(s.depth_ms.clamp(0.0, 10.0));
        self.feedback.target = s.feedback.clamp(-0.95, 0.95);
        self.mix.target = s.mix.clamp(0.0, 1.0);
        self.out.target = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
        self.spread.target = s.spread.clamp(0.0, 0.5);
        self.manual.target = s.manual.clamp(0.0, 1.0);
        self.sens.target = crate::math::db_to_gain(s.sens_db.clamp(0.0, 48.0));
        let k = |t_ms: f32| 1.0 - crate::math::exp(-1.0 / (t_ms.max(0.05) * 0.001 * self.sr));
        self.k_att = k(s.attack_ms);
        self.k_rel = k(s.release_ms);
        self.lfo.set(&s.lfo, &self.clock);
        self.set = *s;
    }

    pub fn set_transport(&mut self, t: &Transport) {
        self.clock.set(t);
        self.lfo.follow(&self.clock);
    }

    pub fn reset(&mut self) {
        self.clock.reset();
        self.lfo.reset();
        for l in self.lines.iter_mut().chain(self.dry.iter_mut()) {
            l.reset();
        }
        self.back = [0.0; 2];
        self.env = 0.0;
        for g in [
            &mut self.delay,
            &mut self.depth,
            &mut self.feedback,
            &mut self.mix,
            &mut self.out,
            &mut self.spread,
            &mut self.manual,
            &mut self.sens,
        ] {
            g.jump(g.target);
        }
        self.zrun = 0;
        self.tel = [0.0; 16];
        self.blocks = 0;
    }

    /// The dry path's delay in through-zero mode; nothing otherwise.
    pub fn latency(&self) -> usize {
        if self.set.through_zero {
            self.centre
        } else {
            0
        }
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [
            &self.delay,
            &self.depth,
            &self.feedback,
            &self.mix,
            &self.out,
            &self.spread,
            &self.manual,
            &self.sens,
        ]
        .iter()
        .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self
            .lines
            .iter()
            .chain(self.dry.iter())
            .all(ModLine::is_silent)
        {
            return Some("a line holds sound");
        }
        if self.back.iter().any(|v| v.to_bits() != 0) || self.env.to_bits() != 0 {
            return Some("the feedback or the envelope holds a value");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.clock.skip(frames);
        for l in self.lines.iter_mut().chain(self.dry.iter_mut()) {
            l.skip(frames);
        }
        self.zrun = self.zrun.saturating_add(frames as u32);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let tz = self.set.through_zero;
        let centre = self.centre as f32;
        let (mut ml, mut mr, mut d_now) = (0.0f32, 0.0f32, 0.0f32);
        for i in 0..n {
            let (x, y) = (l[i], r[i]);
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
            } else {
                self.zrun = 0;
            }
            let settle = self.zrun >= SETTLE_RUN;
            // The envelope (peak, attack and release), whatever moves the sweep, so it is
            // in step when the mode changes.
            let a = x.abs().max(y.abs());
            let k = if a > self.env { self.k_att } else { self.k_rel };
            self.env += (a - self.env) * k;
            if settle && self.env < SETTLE {
                self.env = 0.0;
            }
            let delay = self.delay.next();
            let depth = self.depth.next();
            let fb = self.feedback.next();
            let (gd, gw) = crate::concentus::mix_gains(self.mix.next());
            let out = self.out.next();
            let spread = self.spread.next();
            let manual = self.manual.next();
            let sens = self.sens.next();
            // What goes back into the lines: the last wet, flushed once it is only a
            // denormal's worth after a silence.
            let (mut bl, mut br) = (self.back[0] * fb, self.back[1] * fb);
            if settle && bl.abs() < SETTLE && br.abs() < SETTLE {
                bl = 0.0;
                br = 0.0;
                self.back = [0.0; 2];
            }
            self.lines[0].push(x + bl);
            self.lines[1].push(y + br);
            // The dry line runs in every mode, so switching to through-zero finds it in step.
            {
                self.dry[0].push(x);
                self.dry[1].push(y);
            }
            (ml, mr) = match self.set.mode {
                Mode::Lfo => {
                    let a = self.lfo.value(&self.clock, i);
                    let b = if spread == 0.0 {
                        a
                    } else {
                        self.lfo.value_offset(&self.clock, i, f64::from(spread))
                    };
                    (a, b)
                }
                Mode::Envelope => {
                    let p = (self.env * sens).min(1.0);
                    (2.0 * p - 1.0, 2.0 * p - 1.0)
                }
                Mode::Manual => (2.0 * manual - 1.0, 2.0 * manual - 1.0),
            };
            let (dl, dr, dry_x, dry_y) = if tz {
                let half = (depth * 0.5).min(centre - MIN_READ);
                (
                    centre + half * ml,
                    centre + half * mr,
                    self.dry[0].tap(self.centre),
                    self.dry[1].tap(self.centre),
                )
            } else {
                (
                    delay + depth * 0.5 * (ml + 1.0),
                    delay + depth * 0.5 * (mr + 1.0),
                    x,
                    y,
                )
            };
            d_now = if tz { dl - centre } else { dl };
            let mut wl = self.lines[0].read(dl);
            let mut wr = self.lines[1].read(dr);
            self.back = [wl, wr];
            if self.set.invert {
                wl = -wl;
                wr = -wr;
            }
            let (mut ox, mut oy) = if gw == 0.0 {
                (dry_x, dry_y)
            } else {
                (dry_x * gd + wl * gw, dry_y * gd + wr * gw)
            };
            if out != 1.0 {
                ox *= out;
                oy *= out;
            }
            l[i] = ox;
            r[i] = oy;
        }
        self.clock.advance(n);
        self.blocks = (self.blocks + 1) % (1 << 24);
        let cycles = self.lfo.cycles(&self.clock, 0);
        let (pl, _) = self.lfo.phase_at(cycles);
        let (pr, _) = self.lfo.phase_at(cycles + f64::from(self.spread.value));
        self.tel[tel::SAMPLE_RATE] = self.sr;
        self.tel[tel::TEMPO] = self.clock.tempo() as f32;
        self.tel[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        self.tel[tel::PHASE_L] = pl as f32;
        self.tel[tel::PHASE_R] = pr as f32;
        self.tel[tel::MOD_L] = ml;
        self.tel[tel::MOD_R] = mr;
        self.tel[tel::IN_PEAK] = peak_db(in_peak);
        self.tel[tel::OUT_PEAK] = peak_db(peak_of(&l[..n], &r[..n]));
        self.tel[tel::BLOCKS] = self.blocks as f32;
        self.tel[T_SWEEP] = 0.5 * (ml + 1.0);
        self.tel[T_DELAY_MS] = d_now / self.sr * 1000.0;
        self.tel[T_ENV_DB] = peak_db(self.env);
        self.tel[T_LATENCY] = self.latency() as f32;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out[..16].copy_from_slice(&self.tel);
    }
}

#[cfg(test)]
#[path = "flexus_tests.rs"]
mod tests;
