//! Libra, the utility (ADR 0130): a channel's everyday corrections.
//!
//! In order: **DC removal** (a 5 Hz first-order high-pass), the **channel operation** (swap,
//! mono, one side to both, mid or side to both), **polarity** per channel, a **delay** per
//! channel in whole samples (two microphones aligned to the sample), **width** (the side's
//! share, mid/side), the **balance** (ADR 0012's: unity at the centre and on the near side —
//! `cantus_core`'s `balance_gains`, passed in as a function), the **gain** (±36 dB) and
//! **mute**. The gain, balance, width and mute glide over 20 ms; the delays, the channel
//! operation, the polarity and the DC switch take effect at once (as the console's buttons
//! do).
//!
//! **Null**: at the defaults every sample comes back to the bit — every stage at its identity
//! is skipped, not multiplied by one. **At rest** when every glide has landed, the delay
//! lines hold only zeros and the DC filter's state is +0.0. The editor's meters (the
//! correlation and the goniometer's points) come in the telemetry.

use crate::modfx_clock::{peak_db, peak_of, tel};
use crate::modfx_line::Glide;
use crate::rest::ZeroRun;

/// The channel operations (`libra.chan.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Chan {
    #[default]
    Stereo,
    Swap,
    Mono,
    LeftBoth,
    RightBoth,
    Mid,
    Side,
}

impl Chan {
    pub const ALL: [Chan; 7] = [
        Chan::Stereo,
        Chan::Swap,
        Chan::Mono,
        Chan::LeftBoth,
        Chan::RightBoth,
        Chan::Mid,
        Chan::Side,
    ];

    pub fn from_index(v: f32) -> Chan {
        let i = if v.is_finite() { v.round() as i64 } else { 0 };
        Chan::ALL[i.clamp(0, 6) as usize]
    }

    #[inline]
    fn apply(self, a: f32, b: f32) -> (f32, f32) {
        match self {
            Chan::Stereo => (a, b),
            Chan::Swap => (b, a),
            Chan::Mono | Chan::Mid => {
                let m = (a + b) * 0.5;
                (m, m)
            }
            Chan::LeftBoth => (a, a),
            Chan::RightBoth => (b, b),
            Chan::Side => {
                let s = (a - b) * 0.5;
                (s, s)
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LibraSettings {
    pub gain_db: f32,
    /// −1 … 1.
    pub pan: f32,
    /// 0 … 2: the side's share.
    pub width: f32,
    pub chan: Chan,
    pub invert_l: bool,
    pub invert_r: bool,
    pub dc: bool,
    pub delay_l_ms: f32,
    pub delay_r_ms: f32,
    pub mute: bool,
}

impl Default for LibraSettings {
    fn default() -> Self {
        LibraSettings {
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            chan: Chan::Stereo,
            invert_l: false,
            invert_r: false,
            dc: false,
            delay_l_ms: 0.0,
            delay_r_ms: 0.0,
            mute: false,
        }
    }
}

/// The longest delay, seconds (`cantus_core::fx::libra::MAX_DELAY_MS`).
pub const MAX_DELAY_S: f32 = 0.1;
/// The goniometer's points a block, in the telemetry from [`t::POINTS`].
pub const POINTS: usize = 24;
const SNAP: f32 = 1e-6;
/// DC states under this are flushed to +0.0.
const FLUSH: f32 = 1e-15;

/// Libra's own telemetry, after the family's header.
pub mod t {
    use crate::modfx_clock::tel::OWN;
    /// The output's correlation over the last block (−1 … 1; 0 for silence).
    pub const CORR: usize = OWN;
    /// Each output channel's peak over the last block, dBFS.
    pub const PEAK_L: usize = OWN + 1;
    pub const PEAK_R: usize = OWN + 2;
    /// The delays in force, samples.
    pub const DELAY_L: usize = OWN + 3;
    pub const DELAY_R: usize = OWN + 4;
    /// The goniometer's points: `POINTS` pairs (mid, side) of the output, evenly through
    /// the last block.
    pub const POINTS: usize = 16;
}

#[derive(Clone, Debug)]
pub struct Libra {
    sr: f32,
    set: LibraSettings,
    balance: fn(f32) -> (f32, f32),
    gain: Glide,
    bal_l: Glide,
    bal_r: Glide,
    width: Glide,
    level: Glide,
    dc_k: f32,
    dc_s: [f32; 2],
    lines: [Box<[f32]>; 2],
    mask: usize,
    w: usize,
    zeros: [ZeroRun; 2],
    delay: [usize; 2],
    tel: [f32; 64],
    blocks: u32,
}

impl Libra {
    /// `balance` maps a balance (−1 … 1) to the two channels' gains (ADR 0012's balance).
    /// Allocates its delay lines: build it off the audio thread.
    pub fn new(sample_rate: f32, balance: fn(f32) -> (f32, f32)) -> Libra {
        let len = ((MAX_DELAY_S * sample_rate) as usize + 2).next_power_of_two();
        let mut l = Libra {
            sr: sample_rate,
            set: LibraSettings::default(),
            balance,
            gain: Glide::new(1.0, 20.0 / 3.0, sample_rate, SNAP),
            bal_l: Glide::new(1.0, 20.0 / 3.0, sample_rate, SNAP),
            bal_r: Glide::new(1.0, 20.0 / 3.0, sample_rate, SNAP),
            width: Glide::new(1.0, 20.0 / 3.0, sample_rate, SNAP),
            level: Glide::new(1.0, 20.0 / 3.0, sample_rate, SNAP),
            dc_k: 1.0 - crate::math::exp(-std::f32::consts::TAU * 5.0 / sample_rate),
            dc_s: [0.0; 2],
            lines: [
                vec![0.0; len].into_boxed_slice(),
                vec![0.0; len].into_boxed_slice(),
            ],
            mask: len - 1,
            w: 0,
            zeros: [ZeroRun::FULL; 2],
            delay: [0; 2],
            tel: [0.0; 64],
            blocks: 0,
        };
        l.set(&LibraSettings::default());
        l.reset();
        l
    }

    pub fn settings(&self) -> &LibraSettings {
        &self.set
    }

    /// The delay in whole samples for `ms` at this rate.
    pub fn samples_of(&self, ms: f32) -> usize {
        ((ms.max(0.0) * 0.001 * self.sr).round() as usize).min(self.mask - 1)
    }

    pub fn set(&mut self, s: &LibraSettings) {
        self.set = *s;
        self.gain.target = if s.gain_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.gain_db.clamp(-36.0, 36.0))
        };
        let (l, r) = (self.balance)(s.pan);
        self.bal_l.target = l;
        self.bal_r.target = r;
        self.width.target = s.width.clamp(0.0, 2.0);
        self.level.target = if s.mute { 0.0 } else { 1.0 };
        self.delay = [self.samples_of(s.delay_l_ms), self.samples_of(s.delay_r_ms)];
    }

    pub fn reset(&mut self) {
        for g in [
            &mut self.gain,
            &mut self.bal_l,
            &mut self.bal_r,
            &mut self.width,
            &mut self.level,
        ] {
            g.jump(g.target);
        }
        self.dc_s = [0.0; 2];
        for l in &mut self.lines {
            l.fill(0.0);
        }
        self.zeros = [ZeroRun::FULL; 2];
        self.w = 0;
        self.tel = [0.0; 64];
        self.blocks = 0;
    }

    pub fn latency(&self) -> usize {
        0
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        if [
            &self.gain,
            &self.bal_l,
            &self.bal_r,
            &self.width,
            &self.level,
        ]
        .iter()
        .any(|g| !g.still())
        {
            return Some("a glide is moving");
        }
        if !self.zeros.iter().all(|z| z.covers(self.lines[0].len())) {
            return Some("a delay line holds sound");
        }
        if self.dc_s.iter().any(|s| s.to_bits() != 0) {
            return Some("the DC filter remembers");
        }
        None
    }

    /// `frames` of silence skipped at rest: the lines' write position moves on.
    pub fn skip(&mut self, frames: usize) {
        self.w = (self.w + frames) & self.mask;
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let s = self.set;
        let (pol_l, pol_r) = (s.invert_l, s.invert_r);
        for i in 0..n {
            let (mut a, mut b) = (l[i], r[i]);
            if s.dc {
                for (x, st) in [&mut a, &mut b].into_iter().zip(self.dc_s.iter_mut()) {
                    *st += (*x - *st) * self.dc_k;
                    if st.abs() < FLUSH {
                        *st = 0.0;
                    }
                    *x -= *st;
                }
            }
            if s.chan != Chan::Stereo {
                (a, b) = s.chan.apply(a, b);
            }
            // Inverted as `0 − x`, so silence stays +0.0 (a negated +0.0 is −0.0, which the
            // lines would hold as sound).
            if pol_l {
                a = 0.0 - a;
            }
            if pol_r {
                b = 0.0 - b;
            }
            // The delay lines are always written, so a delay set later has its history.
            self.lines[0][self.w] = a;
            self.lines[1][self.w] = b;
            self.zeros[0].push(a);
            self.zeros[1].push(b);
            if self.delay[0] > 0 {
                a = self.lines[0][self.w.wrapping_sub(self.delay[0]) & self.mask];
            }
            if self.delay[1] > 0 {
                b = self.lines[1][self.w.wrapping_sub(self.delay[1]) & self.mask];
            }
            self.w = (self.w + 1) & self.mask;
            let wd = self.width.next();
            if wd != 1.0 {
                let m = (a + b) * 0.5;
                let sd = (a - b) * 0.5 * wd;
                (a, b) = (m + sd, m - sd);
            }
            let (gl, gr) = (self.bal_l.next(), self.bal_r.next());
            if gl != 1.0 {
                a *= gl;
            }
            if gr != 1.0 {
                b *= gr;
            }
            let g = self.gain.next() * self.level.next();
            if g != 1.0 {
                a *= g;
                b *= g;
            }
            l[i] = a;
            r[i] = b;
        }
        if !s.dc {
            // Off, it is made new: switched on again it starts as a new unit's would.
            self.dc_s = [0.0; 2];
        }
        self.blocks = (self.blocks + 1) % (1 << 24);
        self.meters(&l[..n], &r[..n], in_peak);
    }

    fn meters(&mut self, l: &[f32], r: &[f32], in_peak: f32) {
        let (mut lr, mut ll, mut rr) = (0.0f64, 0.0f64, 0.0f64);
        for (a, b) in l.iter().zip(r) {
            let (a, b) = (f64::from(*a), f64::from(*b));
            lr += a * b;
            ll += a * a;
            rr += b * b;
        }
        let den = (ll * rr).sqrt();
        let tl = &mut self.tel;
        tl[tel::SAMPLE_RATE] = self.sr;
        tl[tel::IN_PEAK] = peak_db(in_peak);
        tl[tel::OUT_PEAK] = peak_db(peak_of(l, r));
        tl[tel::BLOCKS] = self.blocks as f32;
        tl[t::CORR] = if den > 1e-20 { (lr / den) as f32 } else { 0.0 };
        tl[t::PEAK_L] = peak_db(l.iter().fold(0.0f32, |m, x| m.max(x.abs())));
        tl[t::PEAK_R] = peak_db(r.iter().fold(0.0f32, |m, x| m.max(x.abs())));
        tl[t::DELAY_L] = self.delay[0] as f32;
        tl[t::DELAY_R] = self.delay[1] as f32;
        let n = l.len();
        for k in 0..POINTS {
            let (m, s) = if n == 0 {
                (0.0, 0.0)
            } else {
                let i = k * n / POINTS;
                ((l[i] + r[i]) * 0.5, (l[i] - r[i]) * 0.5)
            };
            tl[t::POINTS + 2 * k] = m;
            tl[t::POINTS + 2 * k + 1] = s;
        }
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out.copy_from_slice(&self.tel);
    }
}

/// The tuner's reading of a frequency against the reference A (415 … 466 Hz): the MIDI note
/// nearest and the cents off it (−50 … 50). The editor feeds it the streaming tracker's
/// voiced frames (`pitch::PitchTracker`) of the output.
pub fn tuner_reading(f0: f32, reference: f32) -> (i32, f32) {
    let x = 69.0 + 12.0 * crate::math::log2(f0 / reference.clamp(415.0, 466.0));
    let note = x.round();
    (note as i32, (x - note) * 100.0)
}

#[cfg(test)]
#[path = "libra_tests.rs"]
mod tests;
