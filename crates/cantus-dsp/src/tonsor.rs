//! Tonsor: the clipper, with a true-peak ceiling (ADR 0056).
//!
//! **The path.** The input, times the drive, goes through the curve — at the base rate, or
//! oversampled 2 … 16× ([`crate::dynfx_os`]: what the curve *changes* is band-limited back
//! and added to the input delayed, so a sample the curve leaves alone comes out untouched,
//! to the bit). Then, when the **true-peak ceiling** is on, a guard: the waveform between
//! the samples is read as the meters read it (`terminus::os::TruePeak`, 4× and parabolas),
//! and where it would pass the ceiling a gain under 1 is held over the samples around it
//! and smoothed by a moving mean of the same width — so the gain at every sample is no
//! more than the sample needs, with no step — and applied to the signal delayed by the
//! guard's look-ahead. Last, the output gain (and, with *gain match*, the drive taken back
//! out, so a louder sound never wins the comparison).
//!
//! **The curves**, from the ceiling `c` and the knee's start `k = c·10^(−knee/20)`, with
//! `v = (|u| − k)/(c − k)`; below `k` every curve is the straight line, to the bit:
//! - *Hard*: a corner rounded by a parabola, `k + (c−k)(v − v²/4)`, flat at the ceiling from
//!   `v = 2` — a knee of 0 is a plain clip;
//! - *Soft*: `k + (c−k)·tanh v`, which never quite reaches it;
//! - *Analog*: `k + (c−k)·v/√(1+v²)`, the slowest shoulder — the rounding of a diode pair.
//!
//! **Nulls.** At the defaults (0 dB drive, a 0 dBFS ceiling, no knee, 1×, no guard, 0 dB
//! out) a signal with no sample over 0 dBFS comes out as it went in; at any oversampling,
//! with the guard on or off, a signal with no overs comes out delayed by exactly the
//! reported latency. **Latency** is a function of the configuration — the oversampling
//! and the guard — never of a level ([`latency_for`]).
//!
//! **The meter**: how much (the deepest reduction, held), how often (clipped samples and
//! samples seen, as running counts the editor turns into a rate), the input's inter-sample
//! overs (samples under the ceiling whose waveform passes it) and the output's true peak.

use crate::dynfx_os::{self, Down, Tables, Up, MAX_FACTOR};
use crate::math;
use crate::modfx_line::Glide;
use crate::terminus::os::{TruePeak, TP_DELAY};

/// The curve's shoulder.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    Hard,
    #[default]
    Soft,
    Analog,
}

impl Curve {
    pub const ALL: [Curve; 3] = [Curve::Hard, Curve::Soft, Curve::Analog];

    pub fn from_index(i: f32) -> Curve {
        match i.round() as i32 {
            0 => Curve::Hard,
            2 => Curve::Analog,
            _ => Curve::Soft,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TonsorSettings {
    /// Input gain, dB.
    pub drive_db: f32,
    /// The ceiling, dBFS (dBTP with the guard on).
    pub ceiling_db: f32,
    /// How far under the ceiling the curve starts to bend, dB.
    pub knee_db: f32,
    pub curve: Curve,
    /// 1, 2, 4, 8 or 16.
    pub oversampling: usize,
    /// The true-peak guard.
    pub true_peak: bool,
    /// Take the drive back out after the curve.
    pub match_gain: bool,
    pub out_db: f32,
}

impl Default for TonsorSettings {
    fn default() -> Self {
        TonsorSettings {
            drive_db: 0.0,
            ceiling_db: 0.0,
            knee_db: 0.0,
            curve: Curve::Soft,
            oversampling: 1,
            true_peak: false,
            match_gain: false,
            out_db: 0.0,
        }
    }
}

impl TonsorSettings {
    /// The oversampling factor, one of [`dynfx_os::FACTORS`].
    pub fn factor(&self) -> usize {
        dynfx_os::FACTORS
            .iter()
            .copied()
            .find(|f| *f >= self.oversampling)
            .unwrap_or(dynfx_os::MAX_FACTOR)
    }
}

/// Half the guard's smoothing window (samples).
pub const GUARD_HALF: usize = 16;
/// How much wider the hold is than the smoothing, each side: the gain at a sample is then
/// no more than what every sample this close needs — the reconstruction between samples
/// is made mostly of the nearest few.
pub const GUARD_REACH: usize = 4;
/// Where the guard aims, under the ceiling: −0.1 dB.
pub const GUARD_AIM: f32 = 0.988_553_1;
/// The guard's windows: the hold, the smoothing.
const HOLD_W: usize = 2 * (GUARD_HALF + GUARD_REACH) + 1;
const GUARD_W: usize = 2 * GUARD_HALF + 1;
/// The guard's look-ahead: the detector's delay and the two half-windows.
pub const GUARD_LATENCY: usize = TP_DELAY + 2 * GUARD_HALF + GUARD_REACH;
/// The dry line of the oversampled path.
const DRY: usize = dynfx_os::HALF + dynfx_os::DOWN_HALF;
/// Silent input samples after which every line and window is back to new (more than the
/// longest history: interpolator, decimator, dry line, detector, two windows, look-ahead).
const HISTORY: u32 = 512;
/// Glides land within this (−120 dB of unity).
const SNAP: f32 = 1e-6;
/// Counters wrap here, where `f32` still counts by one.
const WRAP: u32 = 1 << 24;

/// The samples of latency of a configuration.
pub fn latency_for(s: &TonsorSettings) -> usize {
    dynfx_os::latency(s.factor()) + if s.true_peak { GUARD_LATENCY } else { 0 }
}

/// The curve: `u` through a ceiling `c` whose knee starts at `k` (both > 0, `k ≤ c`).
#[inline]
pub fn shape(u: f32, k: f32, c: f32, curve: Curve) -> f32 {
    let m = u.abs();
    if m <= k {
        return u;
    }
    let w = c - k;
    let y = if w <= c * 1e-6 {
        c
    } else {
        let v = (m - k) / w;
        match curve {
            Curve::Hard => {
                if v >= 2.0 {
                    c
                } else {
                    k + w * (v - 0.25 * v * v)
                }
            }
            Curve::Soft => k + w * math::tanh(v),
            Curve::Analog => k + w * (v / (1.0 + v * v).sqrt()),
        }
    };
    y.min(c).copysign(u)
}

/// The telemetry's layout (64 numbers, the plug-in contract's).
pub mod tel {
    pub const SAMPLE_RATE: usize = 0;
    /// Blocks processed (wraps): new telemetry or old.
    pub const BLOCKS: usize = 1;
    /// After the drive: sample peak and true peak, dB, held about 50 ms.
    pub const IN_PEAK: usize = 2;
    pub const IN_TP: usize = 3;
    /// The output's, dB, held.
    pub const OUT_PEAK: usize = 4;
    pub const OUT_TP: usize = 5;
    /// The deepest reduction the curve made, dB (≥ 0), held.
    pub const REDUCTION: usize = 6;
    /// Running counts (wrapping at 2²⁴): samples the curve changed, samples seen, input
    /// samples under the ceiling whose waveform passes it.
    pub const CLIPPED: usize = 7;
    pub const SAMPLES: usize = 8;
    pub const OVERS: usize = 9;
    /// The guard's deepest reduction, dB (≥ 0), held.
    pub const GUARD: usize = 10;
    /// The latency, samples.
    pub const LATENCY: usize = 11;
}

/// A peak kept for at least `window` samples, so an editor reading it once a frame sees
/// every transient (the last block's alone would miss them).
#[derive(Copy, Clone, Debug, Default)]
struct Hold {
    cur: f32,
    prev: f32,
    age: u32,
}

impl Hold {
    fn push(&mut self, v: f32, frames: usize, window: u32) {
        if self.age >= window {
            self.prev = self.cur;
            self.cur = 0.0;
            self.age = 0;
        }
        // A non-finite level is a moment's nonsense: it is not kept.
        if v.is_finite() {
            self.cur = self.cur.max(v);
        }
        self.age = self.age.saturating_add(frames as u32);
    }

    fn get(&self) -> f32 {
        self.cur.max(self.prev)
    }
}

/// The oversampled path of one channel.
#[derive(Clone, Debug)]
struct Chan {
    up: Up,
    down: Down,
    dry: [f32; DRY],
    pos: usize,
}

impl Default for Chan {
    fn default() -> Self {
        Chan {
            up: Up::default(),
            down: Down::default(),
            dry: [0.0; DRY],
            pos: 0,
        }
    }
}

/// The true-peak guard, linked across the channels.
#[derive(Clone, Debug)]
struct Guard {
    tp: [TruePeak; 2],
    need: [f32; HOLD_W],
    npos: usize,
    need_low: usize,
    held: [f32; GUARD_W],
    held_low: usize,
    pos: usize,
    audio: [[f32; GUARD_LATENCY]; 2],
    apos: usize,
}

impl Default for Guard {
    fn default() -> Self {
        Guard {
            tp: [TruePeak::new(), TruePeak::new()],
            need: [1.0; HOLD_W],
            npos: 0,
            need_low: 0,
            held: [1.0; GUARD_W],
            held_low: 0,
            pos: 0,
            audio: [[0.0; GUARD_LATENCY]; 2],
            apos: 0,
        }
    }
}

impl Guard {
    /// One frame in; the frame `GUARD_LATENCY` back out, under the ceiling `c`. Returns
    /// the gain applied too.
    #[inline]
    fn run(&mut self, l: f32, r: f32, c: f32) -> (f32, f32, f32) {
        let lvl = self.tp[0].push(l).max(self.tp[1].push(r));
        // Aimed a hair under the ceiling: the detector reads within about 0.1 dB of the
        // waveform rebuilt at 32 points a sample.
        let aim = c * GUARD_AIM;
        let need = if lvl > aim { aim / lvl } else { 1.0 };
        // The hold: the least need over the window.
        let old = self.need[self.npos];
        self.need_low = self.need_low + usize::from(need < 1.0) - usize::from(old < 1.0);
        self.need[self.npos] = need;
        self.npos = if self.npos + 1 == HOLD_W {
            0
        } else {
            self.npos + 1
        };
        let h = if self.need_low == 0 {
            1.0
        } else {
            self.need.iter().fold(1.0f32, |m, v| m.min(*v))
        };
        let old = self.held[self.pos];
        self.held_low = self.held_low + usize::from(h < 1.0) - usize::from(old < 1.0);
        self.held[self.pos] = h;
        self.pos = if self.pos + 1 == GUARD_W { 0 } else { self.pos + 1 };
        // The mean over the window, oldest first (so the sum's order is the stream's).
        let g = if self.held_low == 0 {
            1.0
        } else {
            let mut s = 0f32;
            for k in 0..GUARD_W {
                s += self.held[(self.pos + k) % GUARD_W];
            }
            s / GUARD_W as f32
        };
        let (dl, dr) = (self.audio[0][self.apos], self.audio[1][self.apos]);
        self.audio[0][self.apos] = l;
        self.audio[1][self.apos] = r;
        self.apos = if self.apos + 1 == GUARD_LATENCY {
            0
        } else {
            self.apos + 1
        };
        if g == 1.0 {
            (dl, dr, 1.0)
        } else {
            (dl * g, dr * g, g)
        }
    }
}

/// The clipper.
#[derive(Clone, Debug)]
pub struct Tonsor {
    sr: f32,
    set: TonsorSettings,
    tables: Tables,
    factor: usize,
    chans: [Chan; 2],
    guard: Box<Guard>,
    drive: Glide,
    ceiling: Glide,
    knee: Glide,
    post: Glide,
    /// Silent input frames in a row.
    zrun: u32,
    // ---- the meter (only the editor reads it) ----
    meter_tp: [TruePeak; 2],
    out_tp: [TruePeak; 2],
    /// The input's sample peaks, `TP_DELAY` frames, for the overs count.
    recent: [f32; TP_DELAY],
    rpos: usize,
    holds: [Hold; 6],
    clipped: u32,
    samples: u32,
    overs: u32,
    blocks: u32,
}

impl Tonsor {
    pub fn new(sample_rate: f32) -> Tonsor {
        let set = TonsorSettings::default();
        Tonsor {
            sr: sample_rate,
            set,
            tables: Tables::new(),
            factor: 1,
            chans: [Chan::default(), Chan::default()],
            guard: Box::default(),
            drive: Glide::new(1.0, 5.0, sample_rate, SNAP),
            ceiling: Glide::new(1.0, 5.0, sample_rate, SNAP),
            knee: Glide::new(1.0, 5.0, sample_rate, SNAP),
            post: Glide::new(1.0, 5.0, sample_rate, SNAP),
            zrun: HISTORY,
            meter_tp: [TruePeak::new(), TruePeak::new()],
            out_tp: [TruePeak::new(), TruePeak::new()],
            recent: [0.0; TP_DELAY],
            rpos: 0,
            holds: [Hold::default(); 6],
            clipped: 0,
            samples: 0,
            overs: 0,
            blocks: 0,
        }
    }

    pub fn settings(&self) -> &TonsorSettings {
        &self.set
    }

    /// New settings: gains glide; a new oversampling or guard starts their lines empty.
    pub fn set(&mut self, s: &TonsorSettings) {
        let gain = |db: f32| if db == 0.0 { 1.0 } else { math::db_to_gain(db) };
        self.drive.target = gain(s.drive_db);
        self.ceiling.target = gain(s.ceiling_db.min(0.0));
        self.knee.target = gain(-s.knee_db.max(0.0));
        let back = if s.match_gain { -s.drive_db } else { 0.0 };
        self.post.target = gain(s.out_db + back);
        let factor = s.factor();
        if factor != self.factor {
            self.factor = factor;
            for c in &mut self.chans {
                *c = Chan::default();
            }
        }
        if s.true_peak != self.set.true_peak {
            *self.guard = Guard::default();
        }
        self.set = *s;
    }

    /// Back to the start of a stream: lines empty, glides on their targets.
    pub fn reset(&mut self) {
        for c in &mut self.chans {
            *c = Chan::default();
        }
        *self.guard = Guard::default();
        for g in [
            &mut self.drive,
            &mut self.ceiling,
            &mut self.knee,
            &mut self.post,
        ] {
            g.jump(g.target);
        }
        self.zrun = HISTORY;
        self.meter_tp = [TruePeak::new(), TruePeak::new()];
        self.out_tp = [TruePeak::new(), TruePeak::new()];
        self.recent = [0.0; TP_DELAY];
        self.holds = [Hold::default(); 6];
        self.clipped = 0;
        self.samples = 0;
        self.overs = 0;
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        latency_for(&self.set)
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !(self.drive.still() && self.ceiling.still() && self.knee.still() && self.post.still())
        {
            return Some("a gain is gliding");
        }
        if self.zrun < HISTORY {
            return Some("the lines still hold sound");
        }
        None
    }

    /// `frames` of silence skipped at rest.
    pub fn skip(&mut self, frames: usize) {
        self.zrun = self.zrun.saturating_add(frames.min(u32::MAX as usize) as u32);
        self.samples = ((self.samples as usize + frames) % WRAP as usize) as u32;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let f = self.factor;
        let tp = self.set.true_peak;
        let curve = self.set.curve;
        let (mut in_pk, mut in_tp, mut out_pk, mut out_tp) = (0f32, 0f32, 0f32, 0f32);
        let (mut red, mut guard_red) = (1f32, 1f32);
        let mut pts = [[0f32; MAX_FACTOR]; 2];
        for i in 0..n {
            let g = self.drive.next();
            let c = self.ceiling.next();
            let k = c * self.knee.next();
            let post = self.post.next();
            let mut x = [l[i], r[i]];
            if g != 1.0 {
                x[0] *= g;
                x[1] *= g;
            }
            if x[0].to_bits() == 0 && x[1].to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
            } else {
                self.zrun = 0;
            }
            // The meter: the input's levels and its inter-sample overs.
            let a = x[0].abs().max(x[1].abs());
            in_pk = in_pk.max(a);
            let t_in = self.meter_tp[0].push(x[0]).max(self.meter_tp[1].push(x[1]));
            in_tp = in_tp.max(t_in);
            // `t_in` is the level of the frame TP_DELAY back: the oldest in `recent`.
            let then = self.recent[self.rpos];
            self.recent[self.rpos] = a;
            self.rpos = (self.rpos + 1) % TP_DELAY;
            if t_in > c && then <= c {
                self.overs = (self.overs + 1) % WRAP;
            }
            // The curve.
            let mut y = [0f32; 2];
            let mut clipped = false;
            for ch in 0..2 {
                if f == 1 {
                    let s = shape(x[ch], k, c, curve);
                    if s != x[ch] {
                        clipped = true;
                        red = red.max(x[ch].abs() / s.abs().max(1e-30));
                    }
                    y[ch] = s;
                } else {
                    let cs = &mut self.chans[ch];
                    let p = &mut pts[ch];
                    cs.up.push(x[ch], f, &self.tables, p);
                    for u in p.iter_mut().take(f) {
                        let s = shape(*u, k, c, curve);
                        if s != *u {
                            clipped = true;
                            red = red.max(u.abs() / s.abs().max(1e-30));
                            *u = s - *u;
                        } else {
                            *u = 0.0;
                        }
                    }
                    let d = cs.down.push(p, f, &self.tables);
                    let dry = cs.dry[cs.pos];
                    cs.dry[cs.pos] = x[ch];
                    cs.pos = if cs.pos + 1 == DRY { 0 } else { cs.pos + 1 };
                    y[ch] = if d.to_bits() == 0 { dry } else { dry + d };
                }
            }
            if clipped {
                self.clipped = (self.clipped + 1) % WRAP;
            }
            if tp {
                let (gl, gr, gg) = self.guard.run(y[0], y[1], c);
                y = [gl, gr];
                guard_red = guard_red.max(1.0 / gg);
            }
            if post != 1.0 {
                y[0] *= post;
                y[1] *= post;
            }
            out_pk = out_pk.max(y[0].abs().max(y[1].abs()));
            let t_out = self.out_tp[0].push(y[0]).max(self.out_tp[1].push(y[1]));
            out_tp = out_tp.max(t_out);
            l[i] = y[0];
            r[i] = y[1];
        }
        self.samples = ((self.samples as usize + n) % WRAP as usize) as u32;
        self.blocks = (self.blocks + 1) % WRAP;
        let window = (0.05 * self.sr) as u32;
        for (h, v) in self
            .holds
            .iter_mut()
            .zip([in_pk, in_tp, out_pk, out_tp, red, guard_red])
        {
            h.push(v, n, window);
        }
    }

    /// The editor's numbers ([`tel`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        use crate::dynfx_follow::to_db;
        out[tel::SAMPLE_RATE] = self.sr;
        out[tel::BLOCKS] = self.blocks as f32;
        out[tel::IN_PEAK] = to_db(self.holds[0].get());
        out[tel::IN_TP] = to_db(self.holds[1].get());
        out[tel::OUT_PEAK] = to_db(self.holds[2].get());
        out[tel::OUT_TP] = to_db(self.holds[3].get());
        // Reductions are kept as ratios (≥ 1): in dB, ≥ 0.
        out[tel::REDUCTION] = to_db(self.holds[4].get().max(1.0));
        out[tel::CLIPPED] = self.clipped as f32;
        out[tel::SAMPLES] = self.samples as f32;
        out[tel::OVERS] = self.overs as f32;
        out[tel::GUARD] = to_db(self.holds[5].get().max(1.0));
        out[tel::LATENCY] = self.latency() as f32;
    }
}

#[cfg(test)]
#[path = "tonsor_tests.rs"]
mod tests;
