//! Fides's waveguide: a string, a tube closed at one end, a pipe open at both (a child of
//! `fides`). Built on Gravis's tuning maths (`gravis::string`'s public functions: the
//! Lagrange read and its phase delay, the loss filter's design, the dispersion allpasses'
//! fit) and a topology of its own: **two loops meeting at the exciter**.
//!
//! The exciter sits at `β` of the length from the bridge. Waves travel from it to the
//! bridge and back (the bridge loop, `β·P` samples, where the loss and the stiffness are
//! lumped) and to the nut and back (the neck loop, the rest of the period `P`). At the
//! junction the string's velocity is the sum of the two waves arriving, and whatever the
//! exciter injects leaves in both directions — a pluck's pulse, or a bow's friction, which
//! needs that velocity (the bowed string of Smith and the STK, MIT, with our own lines and
//! filters). What is heard is the wave arriving at the bridge, which drives the body.
//!
//! **Pitch.** The two loops' Lagrange reads, the loss filter and the allpasses together
//! delay the fundamental by exactly `P = sr / f0` (half that for a closed tube, whose loop
//! inverts): their phase delays at `f0` are computed and the neck loop is solved for the
//! rest, so the fundamental is right to `f64`'s precision at every bend.

use super::exciter::friction;
use super::tables::ResKind;
use crate::gravis::string::{
    ap_delay, dispersion_fit, lagrange, lagrange_delay, loss_delay, loss_design,
};
use crate::math;

/// The dispersion allpasses' sections (the count `dispersion_fit` designs for).
const AP: usize = 4;
/// The lowest fundamental a waveguide plays, Hz (its lines are sized for it).
pub const MIN_HZ: f64 = 20.0;
/// A sample below this is silence for the gate.
const QUIET: f32 = 1e-8;
/// The loop's DC blocker's corner, as a fraction of the fundamental (see [`dc_pole`]).
const DC_CORNER: f64 = 1.0 / 32.0;
/// Samples the lines hold past the lowest fundamental's period: the Lagrange taps' reach
/// and the DC blocker's phase lead (about 12 samples at 20 Hz), which lengthens the lines'
/// share of the period.
const LINE_SPARE: usize = 48;

/// The pole of the loop's DC blocker for a fundamental `f0`: its corner at `f0 / 32`, far
/// under the first partial. A loss filter is a low-pass whose gain at DC is capped just
/// under 1, so without it a loop holds a DC level (a pluck's released displacement leaves
/// one) for ever — the voice never ends, and its offset thumps when the voice is taken.
fn dc_pole(f0: f64, sr: f64) -> f64 {
    math::exp(-std::f64::consts::TAU * f0 * DC_CORNER / sr)
}

/// The DC blocker's pole, its phase delay and its gain at `w0` — none (`1`, `0`, `1`: the
/// loop exactly as without one) when it is out of use.
fn dc_design(on: bool, f0: f64, sr: f64, w0: f64) -> (f64, f64, f64) {
    if on {
        let r = dc_pole(f0, sr);
        let (d, g) = dc_response(r, w0);
        (r, d, g)
    } else {
        (1.0, 0.0, 1.0)
    }
}

/// The DC blocker `(1 + R)/2 · (1 − z⁻¹)/(1 − R z⁻¹)` (its gain 1 at Nyquist, under 1
/// below): its phase delay at `w` (negative — a lead) and its gain there.
fn dc_response(r: f64, w: f64) -> (f64, f64) {
    let (sn, cs) = math::sin_cos(w);
    let lead = math::atan2(sn, 1.0 - cs) - math::atan2(r * sn, 1.0 - r * cs);
    let gain = 0.5 * (1.0 + r) * (2.0 - 2.0 * cs).sqrt() / (1.0 - 2.0 * r * cs + r * r).sqrt();
    (-lead / w, gain)
}

#[derive(Clone, Debug)]
pub struct Guide {
    sr: f64,
    neck: Vec<f32>,
    bridge: Vec<f32>,
    wn: usize,
    wb: usize,
    dn: f64,
    db: f64,
    dn_step: f64,
    db_step: f64,
    /// The loss filter `g(1 − a)/(1 − a z⁻¹)`.
    g1a: f32,
    a: f32,
    lz: f32,
    /// The DC blocker: in use, its pole, its gain `(1 + R)/2`, and its states `(x₁, y₁)`.
    dc_on: bool,
    dc_r: f32,
    dc_k: f32,
    dc: (f32, f32),
    /// The dispersion allpasses' coefficient and states `(x₁, y₁)`.
    c: f32,
    ap: [(f32, f32); AP],
    neck_sign: f32,
    /// A tube's or a pipe's open end radiates its lows less than its highs: a first-order
    /// high-pass at 0.7·f0 (`rad_c` its low-pass's step), scaled so the fundamental keeps
    /// its level; a string's bridge passes the wave as it is.
    radiate: bool,
    rad_c: f32,
    rad_g: f32,
    prev: f32,
    quiet: u32,
    live: bool,
    /// The highest fundamental this rate allows (the lines' and filters' least delays).
    pub max_hz: f64,
}

/// What [`Guide::design`] needs each control step.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GuideStep {
    pub f0: f64,
    /// T60 of the fundamental, s.
    pub t60: f64,
    /// The material's exponent: a partial's T60 falls as `(f/f0)^(−p)`.
    pub p: f64,
    /// Where the exciter is, from the bridge, 0 … ½.
    pub beta: f64,
    /// The loop's DC blocker in use: always but while a bow is on the string. The bow's
    /// stick and slip is timed by the Helmholtz corner, which the blocker's phase lead
    /// at the low partials would carry some 7 cents flat; drawn, the bow holds the loop
    /// anyway, and the blocker comes in when it lifts.
    pub dc: bool,
}

impl Guide {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate as f64;
        let longest = (sr / MIN_HZ).ceil() as usize + LINE_SPARE;
        Guide {
            sr,
            neck: vec![0.0; longest],
            bridge: vec![0.0; longest / 2 + 8],
            wn: 0,
            wb: 0,
            dn: 8.0,
            db: 4.0,
            dn_step: 0.0,
            db_step: 0.0,
            g1a: 0.0,
            a: 0.0,
            lz: 0.0,
            dc_on: false,
            dc_r: 0.0,
            dc_k: 1.0,
            dc: (0.0, 0.0),
            c: 0.0,
            ap: [(0.0, 0.0); AP],
            neck_sign: -1.0,
            radiate: false,
            rad_c: 0.0,
            rad_g: 1.0,
            prev: 0.0,
            quiet: 0,
            live: false,
            max_hz: sr / 12.0,
        }
    }

    /// Whether anything is moving in it.
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Silence at once.
    pub fn clear(&mut self) {
        if self.live || self.lz != 0.0 {
            self.neck.fill(0.0);
            self.bridge.fill(0.0);
        }
        self.lz = 0.0;
        self.dc = (0.0, 0.0);
        self.prev = 0.0;
        self.ap = [(0.0, 0.0); AP];
        self.quiet = 0;
        self.live = false;
    }

    /// A note starts on it: its kind (string, tube, pipe) and stiffness, then the first
    /// step taken at once. `keep`: it is struck again while ringing (its waves stay).
    pub fn start(&mut self, kind: ResKind, inharm: f64, step: &GuideStep, keep: bool) {
        if !keep {
            self.clear();
        }
        self.neck_sign = if kind == ResKind::Tube { 1.0 } else { -1.0 };
        self.radiate = kind != ResKind::String;
        let loop_hz = self.loop_hz(kind);
        // A string's stiffness, B up to 4·10⁻⁴ (a piano's mid-range): its partials stretch
        // as `n·√(1 + B n²)`. Tubes and pipes have none.
        let b = if kind == ResKind::String {
            4e-4 * inharm.max(0.0) * inharm.max(0.0)
        } else {
            0.0
        };
        let f0 = step.f0.clamp(MIN_HZ, self.max_hz);
        let (_, a) = self.loss(f0, step, loop_hz);
        let period = self.sr / f0 / loop_hz;
        let on = step.dc;
        let r = dc_pole(f0, self.sr);
        self.c = dispersion_fit(b, f0, self.sr, period, |w| {
            loss_delay(a, w) + if on { dc_response(r, w).0 } else { 0.0 }
        }) as f32;
        self.design(kind, step, 0);
        self.live = true;
    }

    /// A closed tube's loop inverts, so its loop is half the period; the rest, the whole.
    fn loop_hz(&self, kind: ResKind) -> f64 {
        if kind == ResKind::Tube {
            2.0
        } else {
            1.0
        }
    }

    fn loss(&self, f0: f64, s: &GuideStep, loop_hz: f64) -> (f64, f64) {
        let f_hi = (8.0 * f0).min(0.35 * self.sr);
        let k = loop_hz; // a closed tube passes its loss twice a period
        if f_hi <= 1.5 * f0 {
            let (g, _) = loss_design(f0, s.t60 * k, s.t60 * k, 2.0 * f0, self.sr);
            return (g, 0.0);
        }
        let t_hi = s.t60 * math::powf(f_hi / f0, -s.p);
        loss_design(f0, s.t60 * k, t_hi * k, f_hi, self.sr)
    }

    /// Set the step: the loss, and the two loops' lengths for the pitch, reached over the
    /// next `span` samples (0: at once) — the samples until the next control step, so the
    /// lengths glide from step to step and stand still between equal steps.
    pub fn design(&mut self, kind: ResKind, s: &GuideStep, span: u32) {
        let loop_hz = self.loop_hz(kind);
        let f0 = s.f0.clamp(MIN_HZ, self.max_hz);
        let (g, a) = self.loss(f0, s, loop_hz);
        let w0 = std::f64::consts::TAU * f0 / self.sr;
        // The DC blocker's loss at f0 taken back from the loss filter's, never past the
        // cap: the loop's gain stays under 1 at every frequency.
        let (r, dc_delay, dc_gain) = dc_design(s.dc, f0, self.sr, w0);
        let g = if s.dc { (g / dc_gain).min(0.999_99) } else { g };
        if s.dc && !self.dc_on {
            // Coming in (a bow lifted): from rest, so its first output is the loop's own.
            self.dc = (0.0, 0.0);
        }
        self.dc_on = s.dc;
        self.g1a = (g * (1.0 - a)) as f32;
        self.a = a as f32;
        self.dc_r = r as f32;
        self.dc_k = (0.5 * (1.0 + r)) as f32;
        let fc = 0.7 * f0;
        self.rad_c = (1.0 - math::exp(-std::f64::consts::TAU * fc / self.sr)) as f32;
        let x = f0 / fc;
        self.rad_g = ((1.0 + x * x).sqrt() / x) as f32;
        let period = self.sr / f0 / loop_hz;
        let mut filt = loss_delay(a, w0) + dc_delay;
        if self.c != 0.0 {
            filt += AP as f64 * ap_delay(self.c as f64, w0);
        }
        let avail = (period - filt).max(4.2);
        let db = (s.beta.clamp(0.0, 0.5) * avail).clamp(2.05, avail - 2.1);
        let mut dn = avail - db;
        for _ in 0..3 {
            let total = lagrange_delay(dn, w0) + lagrange_delay(db, w0);
            dn = (dn + (avail - total)).max(2.05);
        }
        let dn = dn.min(self.neck.len() as f64 - 6.0);
        let db = db.min(self.bridge.len() as f64 - 6.0);
        if span == 0 {
            self.dn = dn;
            self.db = db;
            self.dn_step = 0.0;
            self.db_step = 0.0;
        } else {
            self.dn_step = (dn - self.dn) / span as f64;
            self.db_step = (db - self.db) / span as f64;
        }
    }

    #[inline]
    fn read(buf: &[f32], w: usize, d: f64) -> f32 {
        let (m0, h) = lagrange(d);
        let len = buf.len();
        let mut y = 0.0f32;
        for (j, hj) in h.iter().enumerate() {
            // `back` < `len` (the lengths are clamped well inside the lines): no division.
            let back = m0 + j;
            let idx = if w >= back { w - back } else { w + len - back };
            y += hj * buf[idx];
        }
        y
    }

    /// The period now, in samples (the strike's gain is scaled by it: a pulse's harmonics
    /// in a loop of `P` samples are `2/P` of its area).
    pub fn period(&self) -> f64 {
        self.dn + self.db
    }

    /// Render `out.len()` samples, **adding** `g.out` × the wave at the bridge into `out`.
    /// The exciter injects `g.strike × imp + g.sustain × sus`; `bow` is the bow's velocity
    /// (when `bowing`), met through the friction curve of slope `slope`. `grid_end`: the
    /// slice ends on the control grid, where the gate may close. Returns whether it is still
    /// live.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        imp: &[f32],
        sus: &[f32],
        bow: &[f32],
        bowing: bool,
        slope: f32,
        g: &super::voice::Gains,
        out: &mut [f32],
        grid_end: bool,
    ) -> bool {
        if !self.live {
            // Gone still while its exciter waited (sparse grains, a bow not yet moving): it
            // wakes with the first thing that reaches it.
            let fed = bowing && bow.iter().any(|b| *b != 0.0)
                || imp.iter().chain(sus.iter()).any(|x| *x != 0.0);
            if !fed {
                return false;
            }
            self.live = true;
        }
        let n = out.len().min(imp.len()).min(sus.len()).min(bow.len());
        let (gi, gs, gain) = (g.strike, g.sustain, g.out);
        let (lenn, lenb) = (self.neck.len(), self.bridge.len());
        let longest = (self.dn.max(self.db) as u32) + 16;
        for i in 0..n {
            self.dn += self.dn_step;
            self.db += self.db_step;
            let vn = Self::read(&self.neck, self.wn, self.dn);
            let vb = Self::read(&self.bridge, self.wb, self.db);
            let vs = vn + vb;
            let mut inj = gi * imp[i] + gs * sus[i];
            if bowing {
                let dv = bow[i] - vs;
                inj += dv * friction(dv, slope);
            }
            let to_b = vn + inj;
            let to_n = vb + inj;
            // The bridge: its loss, its stiffness, and the inversion of a fixed end.
            let mut y = self.g1a * to_b + self.a * self.lz;
            self.lz = y;
            if self.dc_on {
                let h = self.dc_k * (y - self.dc.0) + self.dc_r * self.dc.1;
                self.dc = (y, h);
                y = h;
            }
            if self.c != 0.0 {
                for s in self.ap.iter_mut() {
                    let o = self.c * y + s.0 - self.c * s.1;
                    s.0 = y;
                    s.1 = o;
                    y = o;
                }
            }
            self.bridge[self.wb] = -y;
            self.wb += 1;
            if self.wb == lenb {
                self.wb = 0;
            }
            self.neck[self.wn] = self.neck_sign * to_n;
            self.wn += 1;
            if self.wn == lenn {
                self.wn = 0;
            }
            if self.radiate {
                self.prev += self.rad_c * (to_b - self.prev);
                out[i] += gain * self.rad_g * (to_b - self.prev);
            } else {
                out[i] += gain * to_b;
            }
            if to_b.abs() < QUIET && vs.abs() < QUIET && inj == 0.0 && bow[i] == 0.0 {
                self.quiet += 1;
            } else {
                self.quiet = 0;
            }
        }
        // Only at the control grid's points, so where it stops is the same at every block
        // size.
        if grid_end && self.quiet > longest {
            // Every wave has passed the junction below the gate: it is still.
            self.clear();
        }
        self.live
    }
}
