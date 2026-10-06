//! Latitudo's grid (the glides and what they derive) and its sample loop.

use super::*;
use crate::multiplex::filters::shelf_params;

impl Latitudo {
    /// One grid point: every glide steps toward its target, the stages fade in or out
    /// (switching structure only while silent), and the coefficients are derived again.
    pub(super) fn grid(&mut self) {
        let s = self.s;
        if self.settled == Some(s) {
            return;
        }
        let (ag, af, ah) = (self.a_gain, self.a_freq, self.a_haas);
        let lg = |hz: f32| hz.max(1.0).log2();
        let mut moving = false;
        let mut glide = |g: &mut Glide, t: f32, a: f32| {
            g.t = t;
            g.step(a);
            moving |= g.moving();
        };
        glide(&mut self.width, s.width, ag);
        glide(&mut self.mid_g, db_to_gain(s.mid_db), ag);
        glide(&mut self.side_g, db_to_gain(s.side_db), ag);
        glide(&mut self.bal, s.balance.clamp(-1.0, 1.0), ag);
        glide(&mut self.rot, s.rotation_deg, ag);
        glide(&mut self.swap, if s.swap { 1.0 } else { 0.0 }, ag);
        glide(&mut self.pol_l, if s.invert_l { -1.0 } else { 1.0 }, ag);
        glide(&mut self.pol_r, if s.invert_r { -1.0 } else { 1.0 }, ag);
        let lm = s.listen.matrix();
        for (g, t) in self.lis.iter_mut().zip(lm) {
            glide(g, t, ag);
        }
        glide(&mut self.out_g, db_to_gain(s.out_db), ag);

        // Mono below: a new slope waits until the stage has faded out.
        if self.mono_slope != s.mono_slope && self.mono_fade.v == 0.0 {
            self.mono_slope = s.mono_slope;
            self.x_mono = [Xover::default(); 2];
        }
        let mono_on = s.mono_on() && self.mono_slope == s.mono_slope;
        self.mono_fade.t = if mono_on { 1.0 } else { 0.0 };
        if s.mono_on() {
            self.mono_f.t = lg(s.mono_hz);
        }
        if self.mono_fade.v == 0.0 {
            // Silent: start (or wait) at the target, with empty filters.
            self.mono_f.v = self.mono_f.t;
            if !mono_on {
                self.x_mono = [Xover::default(); 2];
            }
        }
        let t = self.mono_f.t;
        glide(&mut self.mono_f, t, af);

        // Width by band.
        let bands_on = s.bands_on();
        self.band_fade.t = if bands_on { 1.0 } else { 0.0 };
        let (x_lo, x_hi) = s.crossovers();
        let widths = [s.w_lo, s.w_mid, s.w_hi];
        if self.band_fade.v == 0.0 {
            self.x_lo.v = lg(x_lo);
            self.x_hi.v = lg(x_hi);
            if bands_on {
                for (g, w) in self.w.iter_mut().zip(widths) {
                    g.v = w;
                }
            } else {
                self.x_band = [Xover::default(); 5];
            }
        }
        glide(&mut self.x_lo, lg(x_lo), af);
        glide(&mut self.x_hi, lg(x_hi), af);
        for (g, w) in self.w.iter_mut().zip(widths) {
            glide(g, w, ag);
        }

        // Tone: each shelf runs while its gain is not 0; a new slope waits for silence.
        let gains = [s.mid_lo_db, s.mid_hi_db, s.side_lo_db, s.side_hi_db];
        let freqs = [s.mid_lo_hz, s.mid_hi_hz, s.side_lo_hz, s.side_hi_hz];
        let any_live = (0..4).any(|i| self.tone_db[i].v != 0.0 || gains[i] != 0.0);
        if self.tone_slope != s.tone_slope {
            if !any_live || self.tone_fade.v == 0.0 {
                self.tone_slope = s.tone_slope;
                self.shelves = [Shelf::default(); 4];
                self.tone_fade.t = 1.0;
                if any_live {
                    // Come back in from silence, so the new shelves start without a jump.
                    self.tone_fade.v = 0.0;
                }
            } else {
                self.tone_fade.t = 0.0;
            }
        } else {
            self.tone_fade.t = 1.0;
        }
        for i in 0..4 {
            if self.tone_db[i].v == 0.0 && gains[i] == 0.0 {
                self.tone_f[i].v = lg(freqs[i]);
                self.shelves[i] = Shelf::default();
            }
            glide(&mut self.tone_db[i], gains[i], ag);
            glide(&mut self.tone_f[i], lg(freqs[i]), af);
        }

        // Haas: a new mode waits for silence; silent, the delay jumps to where it goes.
        let haas_on = s.haas_on();
        if self.haas_mode != s.haas_mode && self.haas_fade.v == 0.0 {
            self.haas_mode = s.haas_mode;
        }
        self.haas_fade.t = if haas_on && self.haas_mode == s.haas_mode {
            1.0
        } else {
            0.0
        };
        // `ms · sr / 1000`, in that order: 10 ms at 48 kHz is exactly 480 samples.
        let delay = s.haas_ms.clamp(0.0, HAAS_MAX_MS) * self.sr / 1_000.0;
        if haas_on {
            self.haas_d.t = delay;
        }
        if self.haas_fade.v == 0.0 {
            self.haas_d.v = self.haas_d.t;
        }
        let t = self.haas_d.t;
        glide(&mut self.haas_d, t, ah);
        glide(&mut self.haas_amt, s.haas_amount.clamp(0.0, 1.0), ag);

        self.derive();
        let fades_rest = [
            self.mono_fade,
            self.band_fade,
            self.tone_fade,
            self.haas_fade,
        ]
        .iter()
        .all(|f| f.v == f.t);
        let structure = self.mono_slope == s.mono_slope
            && self.tone_slope == s.tone_slope
            && self.haas_mode == s.haas_mode;
        self.settled = (!moving && fades_rest && structure).then_some(s);
    }

    /// The matrices, the ramps' ends and the filters' coefficients from the glides as
    /// they are now.
    pub(super) fn derive(&mut self) {
        let sr = self.sr;
        // P: polarity, then swap, on left and right.
        let sw = self.swap.v as f64;
        let (pl, pr) = (self.pol_l.v as f64, self.pol_r.v as f64);
        let p = [(1.0 - sw) * pl, sw * pr, sw * pl, (1.0 - sw) * pr];
        // Q: mid and side gains with the width, what is heard, out to left and right,
        // rotation, balance, output.
        let [l00, l01, l11] = self.lis.map(|g| g.v as f64);
        let gm = self.mid_g.v as f64;
        let gs = self.side_g.v as f64 * self.width.v as f64;
        let a = [l00 * gm, (l01 + l11) * gs, l00 * gm, (l01 - l11) * gs];
        let th = (self.rot.v as f64).to_radians();
        let (sn, c) = if th == 0.0 { (0.0, 1.0) } else { th.sin_cos() };
        let rot = [c, -sn, sn, c];
        let b = self.bal.v as f64;
        let go = self.out_g.v as f64;
        let (gl, gr) = ((1.0 - b).min(1.0) * go, (1.0 + b).min(1.0) * go);
        let ra = mul(rot, a);
        let q = [gl * ra[0], gl * ra[1], gr * ra[2], gr * ra[3]];
        let t = mul(mul(q, [0.5, 0.5, 0.5, -0.5]), p);
        let f = |m: [f64; 4]| m.map(|v| v as f32);
        self.p.retarget(f(p));
        self.q.retarget(f(q));
        self.t.retarget(f(t));
        self.wr
            .retarget([self.w[0].v, self.w[1].v, self.w[2].v, self.haas_amt.v]);
        self.dr.retarget([self.haas_d.v, 0.0, 0.0, 0.0]);

        self.mono_c = XoverCoefs::new(self.mono_slope, self.mono_f.v.exp2(), sr);
        self.band_c = [
            XoverCoefs::new(Slope::S24, self.x_lo.v.exp2(), sr),
            XoverCoefs::new(Slope::S24, self.x_hi.v.exp2(), sr),
        ];
        for (g, c) in self.an_g.iter_mut().zip(&self.band_c) {
            *g = c.g / (1.0 + c.g);
        }
        for i in 0..4 {
            let db = self.tone_db[i].v;
            self.shelf_live[i] = db != 0.0 || self.tone_db[i].t != 0.0;
            if !self.shelf_live[i] {
                continue;
            }
            let r = db_to_gain(db);
            let hz = self.tone_f[i].v.exp2();
            // Even shelves are low shelves: r below, 1 above — a high shelf to 1/r, times r.
            self.shelf_c[i] = if i % 2 == 0 {
                let (g, rho) = shelf_params(self.tone_slope, hz, 1.0 / r, sr);
                (g, rho, r)
            } else {
                let (g, rho) = shelf_params(self.tone_slope, hz, r, sr);
                (g, rho, 1.0)
            };
        }
        self.full = self.mono_fade.live()
            || self.band_fade.live()
            || self.haas_fade.live()
            || self.shelf_live.iter().any(|l| *l);
    }
}

impl Latitudo {
    /// Process a stereo block in place.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        self.peak = [0.0; 2];
        let c = self.meter_c;
        let step = self.fade_step;
        for i in 0..n {
            let j = (self.n % UPDATE as u64) as usize;
            if j == 0 {
                self.grid();
            }
            self.p.step(j);
            self.q.step(j);
            self.t.step(j);
            self.wr.step(j);
            self.dr.step(j);
            self.mono_fade.tick(step);
            self.band_fade.tick(step);
            self.tone_fade.tick(step);
            self.haas_fade.tick(step);

            let (xl, xr) = (left[i], right[i]);
            acc(&mut self.acc_in, xl, xr, c);
            let p = self.p.cur;
            let (l1, r1) = if p == IDENTITY {
                (xl, xr)
            } else {
                (p[0] * xl + p[1] * xr, p[2] * xl + p[3] * xr)
            };
            let pos = self.pos;
            self.line[0][pos] = l1;
            self.line[1][pos] = r1;
            let (ol, or) = if self.full {
                self.full_path(l1, r1)
            } else {
                let t = self.t.cur;
                if t == IDENTITY {
                    (xl, xr)
                } else {
                    (t[0] * xl + t[1] * xr, t[2] * xl + t[3] * xr)
                }
            };
            self.pos = if pos + 1 == self.cap { 0 } else { pos + 1 };
            left[i] = ol;
            right[i] = or;
            self.meter(ol, or);
            self.n += 1;
        }
        let pos = self.pos;
        for (q, line) in self.line_quiet.iter_mut().zip(&self.line) {
            q.wrote(line, pos, n);
        }
    }

    /// One sample through the mid/side stages (after polarity and swap).
    #[inline]
    fn full_path(&mut self, l1: f32, r1: f32) -> (f32, f32) {
        let (mut l1, mut r1) = (l1, r1);
        let mut extra_side = 0.0;
        let hf = self.haas_fade.v;
        if hf > 0.0 {
            let (dl, dr) = self.read(self.dr.cur[0]);
            match self.haas_mode {
                HaasMode::DelayRight => r1 += hf * (dr - r1),
                HaasMode::DelayLeft => l1 += hf * (dl - l1),
                HaasMode::MonoSafe => extra_side = hf * self.wr.cur[3] * 0.5 * (dl + dr),
            }
        }
        let mut m = 0.5 * (l1 + r1);
        let mut s = 0.5 * (l1 - r1) + extra_side;

        if self.shelf_live.iter().any(|l| *l) {
            let (mut mt, mut st) = (m, s);
            for k in 0..4 {
                if !self.shelf_live[k] {
                    continue;
                }
                let (g, rho, post) = self.shelf_c[k];
                let x = if k < 2 { mt } else { st };
                let y = post * self.shelves[k].tick(self.tone_slope, g, rho, x, true);
                if k < 2 {
                    mt = y;
                } else {
                    st = y;
                }
            }
            let tf = self.tone_fade.v;
            if tf >= 1.0 {
                m = mt;
                s = st;
            } else {
                m += tf * (mt - m);
                s += tf * (st - s);
            }
        }

        if self.band_fade.live() {
            let [wl, wm, wh, _] = self.wr.cur;
            let (lo, rest) = self.x_band[0].split(&self.band_c[0], s);
            let (mid, hi) = self.x_band[1].split(&self.band_c[1], rest);
            let lo = self.x_band[2].allpass(&self.band_c[1], lo);
            let sb = wl * lo + wm * mid + wh * hi;
            let m1 = self.x_band[3].allpass(&self.band_c[0], m);
            let mb = self.x_band[4].allpass(&self.band_c[1], m1);
            let bf = self.band_fade.v;
            s += bf * (sb - s);
            m += bf * (mb - m);
        }

        if self.mono_fade.live() {
            let (_, hi) = self.x_mono[0].split(&self.mono_c, s);
            let mm = self.x_mono[1].allpass(&self.mono_c, m);
            let mf = self.mono_fade.v;
            s += mf * (hi - s);
            m += mf * (mm - m);
        }

        let q = self.q.cur;
        (q[0] * m + q[1] * s, q[2] * m + q[3] * s)
    }

    /// The Haas line `d` samples back (fractional, linear), left and right.
    #[inline]
    fn read(&self, d: f32) -> (f32, f32) {
        let cap = self.cap;
        let d = d.clamp(0.0, (cap - 2) as f32);
        let i0 = d as usize;
        let fr = d - i0 as f32;
        let a = (self.pos + cap - i0) % cap;
        let b = (self.pos + cap - i0 - 1) % cap;
        let at = |ch: usize| {
            let x = self.line[ch][a];
            x + (self.line[ch][b] - x) * fr
        };
        (at(0), at(1))
    }

    /// The output's meters: levels and correlation, per band, peaks, the scope's points.
    #[inline]
    fn meter(&mut self, l: f32, r: f32) {
        let c = self.meter_c;
        acc(&mut self.acc_out, l, r, c);
        let [g_lo, g_hi] = self.an_g;
        let lo_l = self.an[0].tick(g_lo, l);
        let lo_r = self.an[1].tick(g_lo, r);
        let below_l = self.an[2].tick(g_hi, l);
        let below_r = self.an[3].tick(g_hi, r);
        acc(&mut self.acc_band[0], lo_l, lo_r, c);
        acc(&mut self.acc_band[1], below_l - lo_l, below_r - lo_r, c);
        acc(&mut self.acc_band[2], l - below_l, r - below_r, c);
        self.peak[0] = self.peak[0].max(l.abs());
        self.peak[1] = self.peak[1].max(r.abs());
        if self.n.is_multiple_of(self.stride) {
            self.pts[self.pt_head] = view::pack_point(l, r);
            self.pt_head = (self.pt_head + 1) % POINTS;
            self.pts_done = (self.pts_done + 1) % view::telemetry::WRAP;
        }
    }
}

/// One step of the (LL, RR, LR) integrators; what falls below 10⁻³⁰ is flushed.
#[inline]
fn acc(a: &mut [f32; 3], l: f32, r: f32, c: f32) {
    a[0] += (l * l - a[0]) * c;
    a[1] += (r * r - a[1]) * c;
    a[2] += (l * r - a[2]) * c;
    for v in a.iter_mut() {
        if v.abs() < 1e-30 {
            *v = 0.0;
        }
    }
}

/// 2×2 matrices, row-major.
fn mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
    ]
}
