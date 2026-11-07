//! Chorda's second model's room (ADR 0098): early reflections from a tapped line (a pattern
//! per side, darkened), then an eight-line feedback delay network — a Householder matrix,
//! each line's decay set for its length at low and high frequencies by a one-pole (Jot &
//! Chaigne 1991). In and out in `f64`, sample by sample; its states are flushed to zero on
//! the instrument's grid once nothing has gone in for longer than its longest path, so the
//! instrument comes to an exact rest.

use crate::math;

const LINES: usize = 8;
/// The lines' lengths (ms): mutually prime-ish, 30 … 72 ms (a large studio's live room).
const LINE_MS: [f64; LINES] = [29.7, 37.1, 41.1, 43.7, 53.0, 59.3, 61.9, 71.3];
/// Early reflections (ms, gain) per side.
const ER_L: [(f64, f64); 6] = [
    (7.3, 0.55),
    (11.9, 0.42),
    (17.1, 0.35),
    (23.6, 0.28),
    (29.4, 0.22),
    (37.0, 0.17),
];
const ER_R: [(f64, f64); 6] = [
    (8.9, 0.52),
    (13.1, 0.40),
    (19.7, 0.33),
    (25.3, 0.26),
    (31.8, 0.20),
    (41.2, 0.15),
];
/// The late field starts after this (ms) of the pre-delay line.
const LATE_MS: f64 = 12.0;
/// The longest the pre-delay line holds (ms).
const PRE_MS: f64 = 45.0;

/// A delay line read at any tap.
#[derive(Clone, Debug)]
struct Line {
    buf: Vec<f64>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Line {
        Line {
            buf: vec![0.0; len.max(2)],
            pos: 0,
        }
    }
    #[inline]
    fn push(&mut self, x: f64) {
        self.pos = if self.pos == 0 {
            self.buf.len() - 1
        } else {
            self.pos - 1
        };
        self.buf[self.pos] = x;
    }
    /// The value pushed `d` samples ago (0: the newest).
    #[inline]
    fn at(&self, d: usize) -> f64 {
        let n = self.buf.len();
        self.buf[(self.pos + d.min(n - 1)) % n]
    }
    /// The oldest value (read before the next push).
    #[inline]
    fn last(&self) -> f64 {
        self.at(self.buf.len() - 1)
    }
    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

/// The room's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomVoicing {
    /// T60 (s) at low and at high frequencies.
    pub t60_low: f64,
    pub t60_high: f64,
    /// The early reflections' and the late field's levels.
    pub early: f64,
    pub late: f64,
    /// What goes in: high-passed and low-passed (Hz).
    pub hp_hz: f64,
    pub lp_hz: f64,
}

impl Default for RoomVoicing {
    fn default() -> Self {
        RoomVoicing {
            t60_low: 1.3,
            t60_high: 0.55,
            early: 0.6,
            late: 0.5,
            hp_hz: 90.0,
            lp_hz: 7_000.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Room {
    pre: Line,
    er_l: [(usize, f64); 6],
    er_r: [(usize, f64); 6],
    late_at: usize,
    lines: [Line; LINES],
    /// Each line's one-pole: `y = b0 x + a1 y₋₁`.
    b0: [f64; LINES],
    a1: [f64; LINES],
    /// The size's targets for `b0` and `a1` (the coefficients glide to them, sample by
    /// sample: a size moved while it rings never zips), and whether they are moving.
    b0_to: [f64; LINES],
    a1_to: [f64; LINES],
    moving: bool,
    glide: f64,
    lens: [usize; LINES],
    t60: (f64, f64),
    fs: f64,
    damp: [f64; LINES],
    /// The input's filters.
    hp_a: f64,
    hp_x: f64,
    hp_y: f64,
    lp_a: f64,
    lp_y: f64,
    /// The early reflections' darkening (per side).
    er_a: f64,
    er_y: [f64; 2],
    early: f64,
    late: f64,
    /// Samples since anything above the floor went in or round.
    quiet: usize,
    longest: usize,
    /// Below this a state counts as silent (the instrument's quiet floor).
    pub floor: f64,
}

/// Below this a state counts as silent (−230 dB of full scale at the output's scale).
const FLOOR: f64 = 1.0e-10;

/// Each line's one-pole for its length: `b0` and `a1` for decay times `lo` and `hi` (s).
fn coefficients(lens: &[usize; LINES], lo: f64, hi: f64, fs: f64) -> ([f64; LINES], [f64; LINES]) {
    let mut b0 = [0.0; LINES];
    let mut a1 = [0.0; LINES];
    for i in 0..LINES {
        let n = lens[i] as f64;
        let g_lo = math::powf(10.0f64, -3.0 * n / (lo * fs));
        let g_hi = math::powf(10.0f64, -3.0 * n / (hi * fs));
        let a = (g_lo - g_hi) / (g_lo + g_hi);
        a1[i] = a;
        b0[i] = g_lo * (1.0 - a);
    }
    (b0, a1)
}

impl Room {
    /// The room's size: its decay times scaled by `scale` (1: as voiced). The lines'
    /// coefficients glide to the new ones (50 ms); at rest they take them at once.
    pub fn set_size(&mut self, scale: f64) {
        let (b0, a1) = if scale == 1.0 {
            coefficients(&self.lens, self.t60.0, self.t60.1, self.fs)
        } else {
            coefficients(&self.lens, self.t60.0 * scale, self.t60.1 * scale, self.fs)
        };
        if b0 == self.b0_to && a1 == self.a1_to {
            return;
        }
        self.b0_to = b0;
        self.a1_to = a1;
        if self.at_rest() {
            self.b0 = b0;
            self.a1 = a1;
            self.moving = false;
        } else {
            self.moving = true;
        }
    }

    #[inline]
    fn glide_size(&mut self) {
        let k = self.glide;
        let mut far = 0.0f64;
        for i in 0..LINES {
            self.b0[i] += k * (self.b0_to[i] - self.b0[i]);
            self.a1[i] += k * (self.a1_to[i] - self.a1[i]);
            far = far.max(
                (self.b0_to[i] - self.b0[i])
                    .abs()
                    .max((self.a1_to[i] - self.a1[i]).abs()),
            );
        }
        if far < 1.0e-12 {
            self.b0 = self.b0_to;
            self.a1 = self.a1_to;
            self.moving = false;
        }
    }

    /// Allocates.
    pub fn new(fs: f64, v: &RoomVoicing) -> Room {
        let ms = |x: f64| ((x * 1e-3 * fs).round() as usize).max(1);
        let lens: [usize; LINES] = std::array::from_fn(|i| ms(LINE_MS[i]));
        let (b0, a1) = coefficients(&lens, v.t60_low, v.t60_high, fs);
        let k = |hz: f64| math::exp(-std::f64::consts::TAU * hz / fs);
        Room {
            pre: Line::new(ms(PRE_MS) + 2),
            er_l: std::array::from_fn(|i| (ms(ER_L[i].0), ER_L[i].1)),
            er_r: std::array::from_fn(|i| (ms(ER_R[i].0), ER_R[i].1)),
            late_at: ms(LATE_MS),
            lines: std::array::from_fn(|i| Line::new(lens[i])),
            b0,
            a1,
            b0_to: b0,
            a1_to: a1,
            moving: false,
            glide: 1.0 - math::exp(-1.0 / (0.05 * fs)),
            lens,
            t60: (v.t60_low, v.t60_high),
            fs,
            damp: [0.0; LINES],
            hp_a: k(v.hp_hz),
            hp_x: 0.0,
            hp_y: 0.0,
            lp_a: k(v.lp_hz),
            lp_y: 0.0,
            er_a: k(5_000.0),
            er_y: [0.0; 2],
            early: v.early,
            late: v.late,
            quiet: usize::MAX / 2,
            longest: lens.iter().copied().max().unwrap_or(1) + ms(PRE_MS) + 2,
            floor: FLOOR,
        }
    }

    pub fn at_rest(&self) -> bool {
        self.quiet == usize::MAX / 2
    }

    pub fn reset(&mut self) {
        self.pre.clear();
        for l in self.lines.iter_mut() {
            l.clear();
        }
        self.damp = [0.0; LINES];
        self.hp_x = 0.0;
        self.hp_y = 0.0;
        self.lp_y = 0.0;
        self.er_y = [0.0; 2];
        self.quiet = usize::MAX / 2;
        self.b0 = self.b0_to;
        self.a1 = self.a1_to;
        self.moving = false;
    }

    /// At a grid point: once nothing has gone in or round for longer than the longest path,
    /// every state is cleared (exactly zero: the instrument may rest).
    pub fn flush(&mut self) {
        if self.quiet != usize::MAX / 2 && self.quiet > self.longest {
            self.reset();
        }
    }

    /// One sample: the dry stereo in, the room's stereo out (the wet signal only).
    #[inline]
    pub fn tick(&mut self, l: f64, r: f64) -> (f64, f64) {
        let x = 0.5 * (l + r);
        if x.abs() > self.floor {
            self.quiet = 0;
        } else if self.quiet == usize::MAX / 2 {
            return (0.0, 0.0);
        }
        if self.moving {
            self.glide_size();
        }
        // In: high-passed, low-passed.
        let hp = self.hp_a * (self.hp_y + x - self.hp_x);
        self.hp_x = x;
        self.hp_y = hp;
        self.lp_y = hp + self.lp_a * (self.lp_y - hp);
        self.pre.push(self.lp_y);
        // Early reflections, darkened.
        let mut e = [0.0; 2];
        for (d, g) in self.er_l {
            e[0] += g * self.pre.at(d);
        }
        for (d, g) in self.er_r {
            e[1] += g * self.pre.at(d);
        }
        for (c, v) in e.iter_mut().enumerate() {
            self.er_y[c] = *v + self.er_a * (self.er_y[c] - *v);
            *v = self.er_y[c];
        }
        // The late field: the lines' ends, damped, mixed by Householder, fed back.
        let into = self.pre.at(self.late_at);
        let mut s = [0.0; LINES];
        let mut sum = 0.0;
        for (i, si) in s.iter_mut().enumerate() {
            let y = self.b0[i] * self.lines[i].last() + self.a1[i] * self.damp[i];
            self.damp[i] = y;
            *si = y;
            sum += y;
        }
        let k = 2.0 / LINES as f64 * sum;
        let mut peak = 0.0f64;
        for (i, (si, line)) in s.iter().zip(self.lines.iter_mut()).enumerate() {
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            let w = si - k + sign * 0.35 * into;
            peak = peak.max(w.abs());
            line.push(w);
        }
        let ol = s[0] - s[2] + s[4] - s[6] + 0.5 * (s[1] - s[5]);
        let or = s[1] - s[3] + s[5] - s[7] + 0.5 * (s[2] - s[6]);
        if peak > self.floor || self.lp_y.abs() > self.floor {
            self.quiet = 0;
        } else {
            self.quiet += 1;
        }
        (
            self.early * e[0] + self.late * 0.5 * ol,
            self.early * e[1] + self.late * 0.5 * or,
        )
    }
}
