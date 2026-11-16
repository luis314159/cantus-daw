//! Fractus's read modules — repeat, reverse, tape stop, shuffle — and the joins between
//! reads. Each module says, for its event and a song sample, a [`Rd`]: how far behind the
//! live signal to read (`d`, samples), at what speed, at what gain, and which segment of
//! the event this is (a repeat, a slice: a new segment may jump). Pure functions of the
//! event, the sample and the settings.

use super::seq::{roll, unit};
use super::Ev;
use crate::math;

/// A step in the read larger than this, in samples, beyond what its speed predicts, is a
/// jump: a join's crossfade starts.
pub const JUMP: f64 = 0.5;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RepeatSettings {
    /// The slice repeated, quarter notes ([`super::NOTE_BEATS`]).
    pub size: f64,
    /// Halvings of the slice over the event (0 … 3): the event in `ramp + 1` equal parts,
    /// each repeating half the last one's slice — the stutter build.
    pub ramp: u8,
    /// Semitones each repeat adds (−12 … 12).
    pub pitch_st: f32,
    /// dB each repeat adds (−24 … 0).
    pub decay_db: f32,
    /// How much of each repeat sounds (0.05 … 1).
    pub duty: f32,
}

impl Default for RepeatSettings {
    fn default() -> Self {
        RepeatSettings {
            size: 0.125,
            ramp: 0,
            pitch_st: 0.0,
            decay_db: 0.0,
            duty: 1.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ReverseSettings {
    /// The slice played backwards, quarter notes; 0: the grid's step.
    pub slice: f64,
}

impl Default for ReverseSettings {
    fn default() -> Self {
        ReverseSettings { slice: 0.0 }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TapeKind {
    /// The tape slows to a stop.
    #[default]
    Stop,
    /// From still, the tape comes up to speed.
    Start,
}

impl TapeKind {
    pub fn from_index(v: f32) -> TapeKind {
        if v.round() as i32 == 1 {
            TapeKind::Start
        } else {
            TapeKind::Stop
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TapeSettings {
    pub kind: TapeKind,
    /// How long the stop (or the start) takes, quarter notes; 0: the whole event.
    pub time: f64,
    /// 0: the speed falls in a line; 1: it falls fast and lingers.
    pub curve: f32,
}

impl Default for TapeSettings {
    fn default() -> Self {
        TapeSettings {
            kind: TapeKind::Stop,
            time: 0.0,
            curve: 0.5,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShuffleSettings {
    /// A slice, quarter notes.
    pub slice: f64,
    /// How many slices back a slice may come from (1 … 8).
    pub range: u8,
    /// The chance a slice plays backwards (0 … 1).
    pub reverse: f32,
}

impl Default for ShuffleSettings {
    fn default() -> Self {
        ShuffleSettings {
            slice: 0.25,
            range: 4,
            reverse: 0.0,
        }
    }
}

/// One read: `d` samples behind the live signal, at `speed`, scaled by `gain`, mixed with
/// the live signal by `amt`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rd {
    pub d: f64,
    pub speed: f64,
    pub gain: f32,
    pub amt: f32,
    pub seg: i64,
    /// From the repeat's capture rather than the history.
    pub cap: bool,
}

impl Rd {
    /// The live signal.
    pub const DRY: Rd = Rd {
        d: 0.0,
        speed: 1.0,
        gain: 1.0,
        amt: 1.0,
        seg: 0,
        cap: false,
    };

    pub fn is_dry(&self) -> bool {
        self.d == 0.0 && self.gain == 1.0 && !self.cap
    }
}

/// What a read module needs of the grid.
#[derive(Copy, Clone, Debug)]
pub struct Ctx {
    /// Samples in a quarter note.
    pub spb: f64,
    /// Samples in the grid's step.
    pub step: f64,
    pub xf: u32,
    pub seed: u32,
}

/// The equal-gain raised cosine: 0 at 0, 1 at 1.
#[inline]
pub fn rc(x: f64) -> f32 {
    (0.5 - 0.5 * math::cos(std::f64::consts::PI * x.clamp(0.0, 1.0))) as f32
}

/// Boundary `i` of a grid of `len`-sample cells, rounded.
#[inline]
fn cell(i: i64, len: f64) -> i64 {
    (i as f64 * len).round() as i64
}

/// The cell of a grid of `len` holding `t`: its index and its start.
#[inline]
fn cell_of(t: i64, len: f64) -> (i64, i64) {
    let mut i = (t as f64 / len).floor() as i64;
    while cell(i, len) > t {
        i -= 1;
    }
    while cell(i + 1, len) <= t {
        i += 1;
    }
    (i, cell(i, len))
}

/// Cells of `len` starting before `span`.
fn cells_in(span: i64, len: f64) -> i64 {
    let mut c = (span as f64 / len).ceil() as i64;
    while c > 0 && cell(c - 1, len) >= span {
        c -= 1;
    }
    while cell(c, len) < span {
        c += 1;
    }
    c
}

/// A gain falling to 0 at `end` over the `x` samples before it.
#[inline]
fn fall(tau: i64, end: i64, x: u32) -> f32 {
    if tau >= end {
        0.0
    } else if tau >= end - i64::from(x) {
        rc((end - tau) as f64 / (f64::from(x) + 1.0))
    } else {
        1.0
    }
}

/// The repeat's shortest slice, samples.
const MIN_SLICE: f64 = 16.0;

/// Samples the capture keeps writing past the slice, so the interpolator's taps at its
/// end are the signal's.
pub const CAPTURE_PAD: i64 = 64;

/// How long the capture is written from a repeat's start, samples: its slice and a pad.
pub fn repeat_slice(s: &RepeatSettings, spb: f64) -> i64 {
    (s.size * spb).max(MIN_SLICE).round() as i64 + CAPTURE_PAD
}

/// A repeat: the event's first slice plays live and is captured; every repeat after it
/// reads the capture again, `pitch_st` higher and `decay_db` lower each time, gated by
/// `duty`; with a ramp, each part of the event halves the slice.
pub fn repeat(e: &Ev, so: i64, s: &RepeatSettings, c: &Ctx) -> Rd {
    let t = so - e.start;
    let parts: i64 = if e.end == i64::MAX {
        1
    } else {
        i64::from(s.ramp.min(3)) + 1
    };
    let lf = (e.end - e.start) as f64;
    let pb = |j: i64| -> i64 {
        if j == 0 {
            0
        } else {
            (j as f64 * lf / parts as f64).round() as i64
        }
    };
    let mut j = 0;
    while j + 1 < parts && pb(j + 1) <= t {
        j += 1;
    }
    let r0 = (s.size * c.spb).max(MIN_SLICE);
    let mut n_before = 0;
    for jj in 0..j {
        n_before += cells_in(pb(jj + 1) - pb(jj), (r0 / (1 << jj) as f64).max(MIN_SLICE));
    }
    let r = (r0 / (1 << j) as f64).max(MIN_SLICE);
    let tau0 = t - pb(j);
    let (i, ri) = cell_of(tau0, r);
    let tau = tau0 - ri;
    let rlen = cell(i + 1, r) - ri;
    let n = n_before + i;
    let v = if s.pitch_st == 0.0 || n == 0 {
        1.0
    } else {
        math::exp2(f64::from(s.pitch_st) * n as f64 / 12.0).clamp(0.25, 4.0)
    };
    let (pos, wraps) = if v == 1.0 {
        (tau as f64, 0)
    } else {
        let x = tau as f64 * v;
        let w = (x / r).floor();
        (x - w * r, w as i64)
    };
    let mut gain = if s.decay_db == 0.0 || n == 0 {
        1.0
    } else {
        math::db_to_gain(s.decay_db * n as f32)
    };
    if s.duty < 1.0 {
        let on = (f64::from(s.duty.max(0.05)) * rlen as f64).round() as i64;
        gain *= fall(tau, on.max(2), c.xf.min((on / 2).max(1) as u32));
    }
    Rd {
        d: t as f64 - pos,
        speed: v,
        gain,
        amt: e.amount,
        seg: n * 1024 + wraps,
        cap: true,
    }
}

/// A reverse: each slice of the event plays the slice before it backwards.
pub fn reverse(e: &Ev, so: i64, s: &ReverseSettings, c: &Ctx) -> Rd {
    let t = so - e.start;
    let len = if s.slice > 0.0 {
        s.slice * c.spb
    } else {
        c.step
    }
    .max(MIN_SLICE);
    let (j, sj) = cell_of(t, len);
    let tau = t - sj;
    Rd {
        d: 2.0 * tau as f64,
        speed: -1.0,
        gain: 1.0,
        amt: e.amount,
        seg: j,
        cap: false,
    }
}

/// A tape stop (or start): the speed falls from 1 to 0 over `time` (or rises from 0), the
/// level with it.
pub fn tape(e: &Ev, so: i64, s: &TapeSettings, c: &Ctx) -> Rd {
    let t = (so - e.start) as f64;
    let span = if s.time > 0.0 {
        s.time * c.spb
    } else if e.end == i64::MAX {
        4.0 * c.spb
    } else {
        (e.end - e.start) as f64
    }
    .max(1.0);
    let p = 1.0 + 3.0 * f64::from(s.curve.clamp(0.0, 1.0));
    let u = (t / span).min(1.0);
    let (v, moved) = match s.kind {
        TapeKind::Stop => {
            let v = math::powf(1.0 - u, p);
            let m = span * (1.0 - math::powf(1.0 - u, p + 1.0)) / (p + 1.0);
            (v, m)
        }
        TapeKind::Start => {
            let v = math::powf(u, p);
            let m = span * math::powf(u, p + 1.0) / (p + 1.0) + (t - span).max(0.0);
            (if t >= span { 1.0 } else { v }, m)
        }
    };
    let gain = if v >= 1.0 {
        1.0
    } else {
        math::powf(v, 0.5) as f32
    };
    Rd {
        d: (t - moved).max(0.0),
        speed: v,
        gain,
        amt: e.amount,
        seg: 0,
        cap: false,
    }
}

/// A shuffle: each slice of the event plays a slice up to `range` back (rolled from the
/// seed, the event and the slice), some backwards.
pub fn shuffle(e: &Ev, so: i64, s: &ShuffleSettings, c: &Ctx) -> Rd {
    let t = so - e.start;
    let len = (s.slice * c.spb).max(MIN_SLICE);
    let (j, sj) = cell_of(t, len);
    let tau = t - sj;
    let n = e.seed.wrapping_mul(4096).wrapping_add(j as u64);
    let range = u64::from(s.range.clamp(1, 8));
    let mut k = (roll(c.seed, 3, n, 2) % (range + 1)) as i64;
    let back = s.reverse > 0.0 && unit(roll(c.seed, 3, n, 3)) < s.reverse;
    if back && k == 0 {
        k = 1;
    }
    let src = cell(j - k, len);
    let d = if back {
        let src_len = cell(j - k + 1, len) - src;
        (sj - src - src_len + 1) as f64 + 2.0 * tau as f64
    } else {
        (sj - src) as f64
    };
    Rd {
        d,
        speed: if back { -1.0 } else { 1.0 },
        gain: 1.0,
        amt: e.amount,
        seg: j,
        cap: false,
    }
}

/// Who a read is: a lane's event, or the live signal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Id {
    pub lane: u8,
    pub start: i64,
}

impl Id {
    pub const DRY: Id = Id {
        lane: u8::MAX,
        start: 0,
    };
}

/// The read that fades out where the read jumped: the old one carrying on at its speed.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ghost {
    pub on: bool,
    pub rd: Rd,
    pub left: u32,
    pub len: u32,
}

/// The joins' state: the main read of the last sample, what was heard then (the main
/// read, or the one its end was fading into), and the read fading out.
#[derive(Copy, Clone, Debug)]
pub struct Reader {
    pub prev: (Id, Rd),
    pub heard: (Id, Rd),
    pub ghost: Ghost,
}

impl Reader {
    pub fn new() -> Reader {
        Reader {
            prev: (Id::DRY, Rd::DRY),
            heard: (Id::DRY, Rd::DRY),
            ghost: Ghost {
                on: false,
                rd: Rd::DRY,
                left: 0,
                len: 0,
            },
        }
    }

    /// Back to the live signal, nothing fading (a seek, a reset).
    pub fn restart(&mut self) {
        *self = Reader::new();
    }

    pub fn fading(&self) -> bool {
        self.ghost.on
    }

    /// Whether `rd` carries on from `p` without a step: the same place its speed
    /// predicts, the same level and mix, the same direction.
    fn carries(rd: &Rd, p: &Rd) -> bool {
        let pred = p.d + 1.0 - p.speed;
        (rd.d - pred).abs() <= JUMP
            && (rd.gain - p.gain).abs() <= 1e-3
            && (rd.amt - p.amt).abs() <= 1e-3
            && (rd.speed - p.speed).abs() < 0.5
            && (rd.cap == p.cap || (rd.d == 0.0 && p.d == 0.0))
    }

    /// This sample's read is `rd` (of `id`), fading towards `fade` (its id, read and
    /// weight) before its end. Where it does not carry on from what was heard last, that
    /// carries on at its speed and fades out over `xf`.
    pub fn join(&mut self, id: Id, rd: &Rd, fade: Option<(Id, Rd, f32)>, xf: u32) {
        let (pid, p) = self.prev;
        // The same event: a new segment may jump. Another: from what was heard.
        let from = if pid == id {
            (p.seg != rd.seg).then_some(p)
        } else {
            let (hid, h) = self.heard;
            (hid != id || h.seg != rd.seg).then_some(h)
        };
        if let Some(p) = from {
            if !Reader::carries(rd, &p) && xf > 0 {
                self.ghost = Ghost {
                    on: true,
                    rd: Rd {
                        d: p.d + 1.0 - p.speed,
                        ..p
                    },
                    left: xf,
                    len: xf,
                };
            }
        }
        self.prev = (id, *rd);
        self.heard = match fade {
            Some((fid, frd, w)) if w >= 0.5 => (fid, frd),
            _ => (id, *rd),
        };
    }

    /// The fading read's weight now (before [`Reader::ghost_on`]).
    #[inline]
    pub fn ghost_weight(&self) -> f32 {
        let g = &self.ghost;
        let len = f64::from(g.len);
        1.0 - rc((len - f64::from(g.left) + 1.0) / (len + 1.0))
    }

    /// The fading read moves on a sample.
    #[inline]
    pub fn ghost_on(&mut self) {
        let g = &mut self.ghost;
        if g.on {
            g.rd.d += 1.0 - g.rd.speed;
            g.left -= 1;
            g.on = g.left > 0;
        }
    }
}
