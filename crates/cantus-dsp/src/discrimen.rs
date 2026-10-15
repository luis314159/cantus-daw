//! Discrimen: the transient splitter (ADR 0056) — the sound cut into its **transient** and
//! its **body**, each with its own gain, compressor and tilt, summed back.
//!
//! **The split is exact.** A mask `m` (0 … 1) says how much of each sample is transient;
//! the transient part is `m·x` and the body what is left — computed so that the two add up
//! to `x` **to the bit** for any mask: whichever part is the larger is taken by a product
//! and the other by a subtraction that is exact (Sterbenz: `x − y` is exact when `y` is
//! within a factor of two of `x`). With nothing processed, the output is the input — or,
//! with the look-ahead, the input delayed by the latency — bit for bit.
//!
//! **The detection** is the family's (`dynfx_transient`): the attack reading (a fast
//! envelope over a slow one, in dB) through a smoothstep, full at `3 + 21·(1 − sensitivity)`
//! dB; the *speed* is how long a transient lasts. Two modes:
//! - **zero latency**: the mask from the detector as it is — the first fraction of a
//!   millisecond of an onset reaches the body before the mask is up;
//! - **look-ahead** (2 ms): the audio waits while the mask looks ahead (a running maximum
//!   over the look-ahead), so an onset is in the transient part from its first sample.
//!
//! **Each part**: a gain, a one-knob compressor (threshold `−36·amount` dBFS, ratio
//! `1 + 5·amount`, the transient's fast, the body's slow) and a tilt around 1 kHz. A
//! section at its default does not run and holds nothing: switched off, it is emptied.

use crate::dynfx_follow::{sane, Follower, Quiet, Times, SETTLE};
use crate::dynfx_meter::{Columns, Fold, Hold};
use crate::dynfx_transient::{Diff, DiffTimes, RunMax};
use crate::math;
use crate::modfx_line::{Glide, OnePole};

/// What is heard.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Listen {
    #[default]
    Both,
    Transient,
    Body,
}

impl Listen {
    pub fn from_index(i: f32) -> Listen {
        match i.round() as i32 {
            1 => Listen::Transient,
            2 => Listen::Body,
            _ => Listen::Both,
        }
    }
}

/// One part's processing.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Part {
    pub gain_db: f32,
    /// 0 … 1: the compressor's amount.
    pub comp: f32,
    /// −6 … +6 dB: highs up and lows down (or the reverse) around 1 kHz.
    pub tilt_db: f32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiscrimenSettings {
    pub look_ahead: bool,
    /// 0 … 1.
    pub sensitivity: f32,
    /// How long a transient lasts, ms.
    pub speed_ms: f32,
    pub transient: Part,
    pub body: Part,
    pub listen: Listen,
    pub out_db: f32,
}

impl Default for DiscrimenSettings {
    fn default() -> Self {
        DiscrimenSettings {
            look_ahead: false,
            sensitivity: 0.5,
            speed_ms: 10.0,
            transient: Part::default(),
            body: Part::default(),
            listen: Listen::Both,
            out_db: 0.0,
        }
    }
}

/// The look-ahead, ms.
pub const LOOK_MS: f32 = 2.0;
/// The longest look-ahead line (2 ms at 384 kHz).
const MAX_LOOK: usize = 1_024;
/// The tilt's pivot, Hz.
pub const TILT_HZ: f32 = 1_000.0;
const SNAP: f32 = 1e-6;
/// Filter states under this are flushed after a run of silence.
const STATE_SETTLE: f32 = 1e-20;

/// Samples of latency of a configuration: the look-ahead's, or none.
pub fn latency_for(s: &DiscrimenSettings, sample_rate: f32) -> usize {
    if s.look_ahead {
        ((LOOK_MS * 0.001 * sample_rate).round() as usize).clamp(1, MAX_LOOK - 1)
    } else {
        0
    }
}

/// `x` split by the mask `m`: `(transient, body)`, adding up to `x` to the bit.
#[inline]
pub fn split(x: f32, m: f32) -> (f32, f32) {
    if x == 0.0 {
        // ±0: both parts the same zero, so their sum keeps its sign.
        return (x, x);
    }
    if m >= 0.5 {
        let t = m * x;
        (t, x - t)
    } else {
        let b = (1.0 - m) * x;
        (x - b, b)
    }
}

/// The mask of an attack reading at a sensitivity: a smoothstep up to full at
/// `3 + 21·(1 − sensitivity)` dB.
#[inline]
pub fn mask_of(attack_db: f32, sensitivity: f32) -> f32 {
    let full = 3.0 + 21.0 * (1.0 - sensitivity.clamp(0.0, 1.0));
    let u = (attack_db / full).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The static gain reduction of a part's compressor, dB (≥ 0), at an envelope level.
#[inline]
pub fn comp_reduction_db(amount: f32, level_db: f32) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }
    let thr = -36.0 * amount;
    let ratio = 1.0 + 5.0 * amount;
    if level_db > thr {
        (level_db - thr) * (1.0 - 1.0 / ratio)
    } else {
        0.0
    }
}

/// One part's running sections.
#[derive(Copy, Clone, Debug)]
struct Section {
    env: Follower,
    times: Times,
    lp: [OnePole; 2],
    gain: Glide,
    lo: f32,
    hi: f32,
    gr: f32,
}

impl Section {
    fn new(attack_ms: f32, release_ms: f32, sr: f32) -> Section {
        Section {
            env: Follower::default(),
            times: Times::new(attack_ms, release_ms, sr),
            lp: [OnePole::default(); 2],
            gain: Glide::new(1.0, 10.0, sr, SNAP),
            lo: 1.0,
            hi: 1.0,
            gr: 0.0,
        }
    }

    fn clear(&self) -> bool {
        self.env.is_zero() && self.lp.iter().all(OnePole::is_zero)
    }

    /// One frame of this part through its compressor, tilt and gain.
    #[inline]
    fn run(&mut self, p: &Part, g_lp: f32, x: [f32; 2]) -> [f32; 2] {
        let gain = self.gain.next();
        // Not a number: silence here, so no envelope or filter of the part keeps it.
        let mut y = [sane(x[0]), sane(x[1])];
        if p.comp > 0.0 {
            let lvl = self.env.run(y[0].abs().max(y[1].abs()), &self.times);
            let gr = comp_reduction_db(p.comp, crate::dynfx_follow::to_db(lvl));
            self.gr = self.gr.max(gr);
            if gr > 0.0 {
                let g = math::db_to_gain(-gr);
                y[0] *= g;
                y[1] *= g;
            }
        }
        if p.tilt_db != 0.0 {
            for (c, v) in y.iter_mut().enumerate() {
                let lo = self.lp[c].run(*v, g_lp);
                *v = lo * self.lo + (*v - lo) * self.hi;
            }
        }
        if gain != 1.0 {
            y[0] *= gain;
            y[1] *= gain;
        }
        y
    }

    /// Whether the part touches its signal at all.
    fn idle(&self, p: &Part) -> bool {
        p.comp <= 0.0 && p.tilt_db == 0.0 && self.gain.still() && self.gain.value == 1.0
    }

    fn settle(&mut self) {
        self.env.settle(SETTLE);
        for f in &mut self.lp {
            f.settle(STATE_SETTLE);
        }
    }
}

/// The telemetry's layout; columns (`dynfx_meter`) of the transient part's peak, the body's
/// and the mask from index 16.
pub mod tel {
    pub const SAMPLE_RATE: usize = 0;
    pub const BLOCKS: usize = 1;
    /// Held about 50 ms, dB.
    pub const T_PEAK: usize = 3;
    pub const B_PEAK: usize = 4;
    pub const OUT_PEAK: usize = 5;
    /// The mask's highest, 0 … 1, held.
    pub const MASK: usize = 6;
    /// Each part's compressor's deepest reduction, dB, held.
    pub const T_GR: usize = 7;
    pub const B_GR: usize = 8;
    pub const LATENCY: usize = 9;
}

#[derive(Clone, Debug)]
pub struct Discrimen {
    sr: f32,
    set: DiscrimenSettings,
    times: DiffTimes,
    diff: Diff,
    ahead: RunMax,
    look: usize,
    line: Box<[[f32; MAX_LOOK]; 2]>,
    pos: usize,
    zrun: u32,
    quiet: Quiet,
    t: Section,
    b: Section,
    g_lp: f32,
    out: Glide,
    // ---- the meter ----
    holds: [Hold; 6],
    cols: Columns,
    blocks: u32,
}

impl Discrimen {
    pub fn new(sample_rate: f32) -> Discrimen {
        let set = DiscrimenSettings::default();
        Discrimen {
            sr: sample_rate,
            set,
            times: DiffTimes::new(set.speed_ms, 250.0, sample_rate),
            diff: Diff::default(),
            ahead: RunMax::default(),
            look: 0,
            line: Box::new([[0.0; MAX_LOOK]; 2]),
            pos: 0,
            zrun: u32::MAX,
            quiet: Quiet::default(),
            t: Section::new(0.5, 40.0, sample_rate),
            b: Section::new(10.0, 150.0, sample_rate),
            g_lp: OnePole::coef(TILT_HZ, sample_rate),
            out: Glide::new(1.0, 10.0, sample_rate, SNAP),
            holds: [Hold::default(); 6],
            cols: Columns::new([Fold::Max; 3]),
            blocks: 0,
        }
    }

    pub fn settings(&self) -> &DiscrimenSettings {
        &self.set
    }

    pub fn set(&mut self, s: &DiscrimenSettings) {
        if s.speed_ms != self.set.speed_ms {
            self.times = DiffTimes::new(s.speed_ms, 250.0, self.sr);
        }
        let look = latency_for(s, self.sr);
        if look != self.look {
            self.look = look;
            self.line.iter_mut().for_each(|l| l.fill(0.0));
            self.pos = 0;
            self.ahead.reset();
        }
        let gain = |db: f32| if db == 0.0 { 1.0 } else { math::db_to_gain(db) };
        for (sec, p) in [(&mut self.t, &s.transient), (&mut self.b, &s.body)] {
            sec.gain.target = gain(p.gain_db);
            // A section that stops running is emptied (it would hold its state forever).
            if p.comp <= 0.0 {
                sec.env.reset();
            }
            if p.tilt_db == 0.0 {
                sec.lp = [OnePole::default(); 2];
            }
            let t = p.tilt_db.clamp(-6.0, 6.0);
            sec.hi = gain(0.5 * t);
            sec.lo = gain(-0.5 * t);
        }
        self.out.target = gain(s.out_db);
        self.set = *s;
    }

    pub fn reset(&mut self) {
        self.diff.reset();
        self.ahead.reset();
        self.line.iter_mut().for_each(|l| l.fill(0.0));
        self.pos = 0;
        self.zrun = u32::MAX;
        self.quiet.reset();
        for sec in [&mut self.t, &mut self.b] {
            sec.env.reset();
            sec.lp = [OnePole::default(); 2];
            sec.gain.jump(sec.gain.target);
            sec.gr = 0.0;
        }
        self.out.jump(self.out.target);
        self.holds = [Hold::default(); 6];
        self.cols.reset();
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        self.look
    }

    /// Both parts' compressors and tilts hold nothing.
    pub fn sections_clear(&self) -> bool {
        self.t.clear() && self.b.clear()
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if !(self.t.gain.still() && self.b.gain.still() && self.out.still()) {
            return Some("a gain is gliding");
        }
        if !self.diff.is_zero() || !self.ahead.is_zero() {
            return Some("the detector remembers");
        }
        if (self.zrun as usize) < self.look {
            return Some("the look-ahead holds sound");
        }
        if !self.sections_clear() {
            return Some("a part's compressor or tilt rings");
        }
        None
    }

    pub fn skip(&mut self, frames: usize) {
        self.diff.skip(frames);
        self.quiet.skip(frames);
        self.zrun = self
            .zrun
            .saturating_add(frames.min(u32::MAX as usize) as u32);
        self.cols.skip(frames, [0.0; 3]);
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let s = self.set;
        let (mut t_pk, mut b_pk, mut o_pk, mut m_pk) = (0f32, 0f32, 0f32, 0f32);
        self.t.gr = 0.0;
        self.b.gr = 0.0;
        for i in 0..n {
            let out = self.out.next();
            let (x, y) = (l[i], r[i]);
            let level = x.abs().max(y.abs());
            let (ad, _) = self.diff.run(level, &self.times);
            let m0 = mask_of(ad, s.sensitivity);
            // The look-ahead: the mask the next `look` samples will have, the audio late by
            // as much.
            let (m, a) = if self.look > 0 {
                let m = self.ahead.push(m0, self.look as u32 + 1);
                let k = self.pos;
                let a = [self.line[0][k], self.line[1][k]];
                self.line[0][k] = x;
                self.line[1][k] = y;
                self.pos = if k + 1 >= self.look { 0 } else { k + 1 };
                (m, a)
            } else {
                (m0, [x, y])
            };
            if x.to_bits() == 0 && y.to_bits() == 0 {
                self.zrun = self.zrun.saturating_add(1);
            } else {
                self.zrun = 0;
            }
            if self.quiet.note(level) {
                self.t.settle();
                self.b.settle();
            }
            let (tl, bl) = split(a[0], m);
            let (tr, br) = split(a[1], m);
            let mut tp = [tl, tr];
            let mut bp = [bl, br];
            if !self.t.idle(&s.transient) {
                tp = self.t.run(&s.transient, self.g_lp, tp);
            }
            if !self.b.idle(&s.body) {
                bp = self.b.run(&s.body, self.g_lp, bp);
            }
            let mut o = match s.listen {
                Listen::Both => [tp[0] + bp[0], tp[1] + bp[1]],
                Listen::Transient => tp,
                Listen::Body => bp,
            };
            if out != 1.0 {
                o[0] *= out;
                o[1] *= out;
            }
            l[i] = o[0];
            r[i] = o[1];
            let (tk, bk) = (tp[0].abs().max(tp[1].abs()), bp[0].abs().max(bp[1].abs()));
            t_pk = t_pk.max(tk);
            b_pk = b_pk.max(bk);
            o_pk = o_pk.max(o[0].abs().max(o[1].abs()));
            m_pk = m_pk.max(m);
            self.cols.push([tk, bk, m]);
        }
        self.blocks = (self.blocks + 1) % crate::dynfx_meter::WRAP;
        let window = (0.05 * self.sr) as u32;
        for (h, v) in self
            .holds
            .iter_mut()
            .zip([t_pk, b_pk, o_pk, m_pk, self.t.gr, self.b.gr])
        {
            h.push(v, n, window);
        }
    }

    /// The editor's numbers ([`tel`]).
    pub fn telemetry(&self, out: &mut [f32; 64]) {
        use crate::dynfx_follow::to_db;
        out[tel::SAMPLE_RATE] = self.sr;
        out[tel::BLOCKS] = self.blocks as f32;
        out[tel::T_PEAK] = to_db(self.holds[0].get());
        out[tel::B_PEAK] = to_db(self.holds[1].get());
        out[tel::OUT_PEAK] = to_db(self.holds[2].get());
        out[tel::MASK] = self.holds[3].get();
        out[tel::T_GR] = self.holds[4].get();
        out[tel::B_GR] = self.holds[5].get();
        out[tel::LATENCY] = self.look as f32;
        self.cols.write(out);
    }
}

#[cfg(test)]
#[path = "discrimen_tests.rs"]
mod tests;
