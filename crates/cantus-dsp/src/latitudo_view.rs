//! What Latitudo tells its editor: the telemetry (meters and the vectorscope's points)
//! and the responses the displays draw — from the same filters the audio runs.

use super::*;
use crate::multiplex::filters::{shelf_params, shelf_response, xover_response, C};

/// The telemetry's layout (64 numbers, refreshed after every block).
///
/// | Slot | What |
/// |---|---|
/// | 0 | vectorscope points written since the start, modulo 2²⁴ |
/// | 1 | the sample rate, Hz |
/// | 2, 3 | the output's correlation (−1 … +1, 0 when silent) and balance (−1 left … +1 right), over 300 ms |
/// | 4, 5 | the same of the input |
/// | 6, 7 | the last block's output peaks, left and right, dB |
/// | 8, 9 | the side's level against the mid, dB, of the output and of the input (−60 floor) |
/// | 10–12 | the output's correlation in three bands: below `x_lo`, between, above `x_hi` |
/// | 13, 14 | the output's and the input's level, dB RMS |
/// | 15 | samples between two vectorscope points |
/// | 16–63 | the last 48 points of the output, oldest first, packed (`unpack_point`) |
pub mod telemetry {
    pub const SEQ: usize = 0;
    pub const SAMPLE_RATE: usize = 1;
    pub const CORR: usize = 2;
    pub const BAL: usize = 3;
    pub const IN_CORR: usize = 4;
    pub const IN_BAL: usize = 5;
    pub const PEAK_L: usize = 6;
    pub const PEAK_R: usize = 7;
    pub const SIDE_DB: usize = 8;
    pub const IN_SIDE_DB: usize = 9;
    pub const BAND_CORR: usize = 10;
    pub const LEVEL_DB: usize = 13;
    pub const IN_LEVEL_DB: usize = 14;
    pub const STRIDE: usize = 15;
    pub const POINTS_AT: usize = 16;
    /// Counters wrap here, so they stay exact in an `f32`.
    pub const WRAP: u32 = 1 << 24;

    /// How many came since `last`, across a wrap.
    pub fn since(now: f32, last: f32) -> u32 {
        let (now, last) = (now.max(0.0) as u32, last.max(0.0) as u32);
        (now + WRAP - last) % WRAP
    }
}

/// The floor of the side-against-mid readout, dB.
pub const SIDE_FLOOR_DB: f32 = -60.0;
/// A point's coordinates are kept within ±this (beyond it they sit on the edge).
pub const POINT_RANGE: f32 = 2.0;
const Q: f32 = 4_095.0;

/// A point (left, right) packed into one `f32`: 12 bits each, exact.
pub fn pack_point(l: f32, r: f32) -> f32 {
    let q = |v: f32| {
        let v = if v.is_finite() { v } else { 0.0 };
        (((v / POINT_RANGE).clamp(-1.0, 1.0) * 0.5 + 0.5) * Q).round() as u32
    };
    (q(l) * 4_096 + q(r)) as f32
}

/// The point [`pack_point`] packed, within 1/2048 of [`POINT_RANGE`].
pub fn unpack_point(p: f32) -> (f32, f32) {
    let v = p.max(0.0) as u32;
    let d = |q: u32| ((q.min(4_095) as f32 / Q) * 2.0 - 1.0) * POINT_RANGE;
    (d(v / 4_096), d(v % 4_096))
}

/// Correlation and balance of integrated (LL, RR, LR).
fn corr_bal(a: &[f32; 3]) -> (f32, f32) {
    let (ll, rr, lr) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let corr = if ll * rr > 1e-20 {
        (lr / (ll * rr).sqrt()).clamp(-1.0, 1.0) as f32
    } else {
        0.0
    };
    let bal = if ll + rr > 1e-12 {
        ((rr - ll) / (rr + ll)).clamp(-1.0, 1.0) as f32
    } else {
        0.0
    };
    (corr, bal)
}

/// The side's level against the mid, dB, from integrated (LL, RR, LR).
fn side_db(a: &[f32; 3]) -> f32 {
    let (ll, rr, lr) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let mm = (ll + rr + 2.0 * lr) * 0.25;
    let ss = (ll + rr - 2.0 * lr).max(0.0) * 0.25;
    if ss <= 1e-15 {
        return SIDE_FLOOR_DB;
    }
    if mm <= 1e-15 {
        return 40.0;
    }
    ((10.0 * (ss / mm).log10()) as f32).clamp(SIDE_FLOOR_DB, 40.0)
}

fn level_db(a: &[f32; 3]) -> f32 {
    let p = (a[0] as f64 + a[1] as f64) * 0.5;
    if p > 1e-14 {
        (10.0 * p.log10()) as f32
    } else {
        -140.0
    }
}

fn peak_db(v: f32) -> f32 {
    if v > 1e-7 {
        20.0 * v.log10()
    } else {
        -140.0
    }
}

impl Latitudo {
    /// The editor's numbers (layout in [`telemetry`]).
    pub fn write_telemetry(&self, t: &mut [f32; 64]) {
        use telemetry::*;
        t.fill(0.0);
        t[SEQ] = self.pts_done as f32;
        t[SAMPLE_RATE] = self.sr;
        let (c, b) = corr_bal(&self.acc_out);
        t[CORR] = c;
        t[BAL] = b;
        let (c, b) = corr_bal(&self.acc_in);
        t[IN_CORR] = c;
        t[IN_BAL] = b;
        t[PEAK_L] = peak_db(self.peak[0]);
        t[PEAK_R] = peak_db(self.peak[1]);
        t[SIDE_DB] = side_db(&self.acc_out);
        t[IN_SIDE_DB] = side_db(&self.acc_in);
        for k in 0..3 {
            t[BAND_CORR + k] = corr_bal(&self.acc_band[k]).0;
        }
        t[LEVEL_DB] = level_db(&self.acc_out);
        t[IN_LEVEL_DB] = level_db(&self.acc_in);
        t[STRIDE] = self.stride as f32;
        for j in 0..POINTS {
            t[POINTS_AT + j] = self.pts[(self.pt_head + j) % POINTS];
        }
    }

    /// Whether the whole effect is a wire right now (every stage silent, the matrix the
    /// identity, nothing moving): the block is returned as it came.
    pub fn is_wire(&self) -> bool {
        !self.full && self.t.cur == IDENTITY && self.t.still()
    }

    /// Whether the mid/side stages run (else the effect is one 2×2 matrix).
    pub fn runs_mid_side(&self) -> bool {
        self.full
    }
}

// ---- responses, for the displays and the tests -------------------------------------

/// The bands' combined response on the side at `hz` (magnitude, 1 is unchanged): each
/// band's width over its crossover, as the audio runs it (the global width apart).
pub fn band_width(s: &StereoSettings, sample_rate: f32, hz: f32) -> f32 {
    if !s.bands_on() {
        return 1.0;
    }
    let (x_lo, x_hi) = s.crossovers();
    let c1 = XoverCoefs::new(Slope::S24, x_lo, sample_rate);
    let c2 = XoverCoefs::new(Slope::S24, x_hi, sample_rate);
    let (lp1, hp1, _) = xover_response(&c1, hz, sample_rate);
    let (lp2, hp2, ap2) = xover_response(&c2, hz, sample_rate);
    let lo = lp1.mul(ap2).scale(s.w_lo as f64);
    let mid = hp1.mul(lp2).scale(s.w_mid as f64);
    let hi = hp1.mul(hp2).scale(s.w_hi as f64);
    lo.add(mid).add(hi).abs() as f32
}

/// How much of the side the mono crossover lets through at `hz` (1 above it, 0 far below).
pub fn mono_pass(s: &StereoSettings, sample_rate: f32, hz: f32) -> f32 {
    if !s.mono_on() {
        return 1.0;
    }
    let c = XoverCoefs::new(s.mono_slope, s.mono_hz, sample_rate);
    xover_response(&c, hz, sample_rate).1.abs() as f32
}

/// The effective width at `hz`: the side against the mid as the width, the mid and side
/// gains, the bands and the mono crossover leave it (tone apart). 1 is as it came.
pub fn width_at(s: &StereoSettings, sample_rate: f32, hz: f32) -> f32 {
    let gains = db_to_gain(s.side_db) / db_to_gain(s.mid_db);
    s.width * gains * band_width(s, sample_rate, hz) * mono_pass(s, sample_rate, hz)
}

/// One shelf's response at `hz`; even `i` are low shelves.
fn shelf_at(slope: Slope, low: bool, f: f32, db: f32, sample_rate: f32, hz: f32) -> C {
    if db == 0.0 {
        return C::ONE;
    }
    let r = db_to_gain(db);
    if low {
        let (g, rho) = shelf_params(slope, f, 1.0 / r, sample_rate);
        shelf_response(slope, g, rho, hz, sample_rate).scale(r as f64)
    } else {
        let (g, rho) = shelf_params(slope, f, r, sample_rate);
        shelf_response(slope, g, rho, hz, sample_rate)
    }
}

/// The tone section's response at `hz`, dB: (mid, side).
pub fn tone_db(s: &StereoSettings, sample_rate: f32, hz: f32) -> (f32, f32) {
    let sl = s.tone_slope;
    let m = shelf_at(sl, true, s.mid_lo_hz, s.mid_lo_db, sample_rate, hz).mul(shelf_at(
        sl,
        false,
        s.mid_hi_hz,
        s.mid_hi_db,
        sample_rate,
        hz,
    ));
    let sd = shelf_at(sl, true, s.side_lo_hz, s.side_lo_db, sample_rate, hz).mul(shelf_at(
        sl,
        false,
        s.side_hi_hz,
        s.side_hi_db,
        sample_rate,
        hz,
    ));
    let db = |c: C| (20.0 * c.abs().max(1e-12).log10()) as f32;
    (db(m), db(sd))
}
