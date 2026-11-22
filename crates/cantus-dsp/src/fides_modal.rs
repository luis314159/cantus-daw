//! Fides's modal resonator: a bar, a tine, a plate, a membrane, a drumhead, a bell, a
//! glass, a pan (a child of `fides`). Each partial of the object's table
//! ([`super::tables::partials`]) is a complex one-pole `y ← r·e^{jω}·y + g·x` — exact in
//! frequency at any pitch (its angle is set, not a cosine's rounding: a fundamental is
//! within 10⁻⁴ cent), decaying in its own T60 — struck where its shape says
//! ([`super::tables::shape`]) and heard from the microphone's point. Modes run in chunks of
//! eight and are summed lane by lane in a fixed order, so the compiler can run a chunk at
//! once and the bits never depend on it.
//!
//! **A bow** on a modal object (a bowed vibraphone, a glass harmonica) meets the object's
//! velocity at the bow — the modes' sum weighted by their shapes there — through the same
//! friction curve as a string's.

use super::body::sum8;
use super::exciter::friction;
use super::tables::{partials, shape, Partial, ResKind, MAX_MODES};
use crate::math;

const LANES: usize = 8;
const CHUNKS: usize = MAX_MODES / LANES;
/// A state below this is still.
const STILL: f32 = 1e-9;

/// What [`Modal::design`] needs each control step.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalStep {
    pub f0: f64,
    pub t60: f64,
    pub p: f64,
}

#[derive(Clone, Debug)]
pub struct Modal {
    sr: f64,
    kind: ResKind,
    n: usize,
    ratio: [f64; MAX_MODES],
    /// Each mode's T60 over the fundamental's, for the exponent `rel_p`.
    rel_t: [f64; MAX_MODES],
    rel_p: f64,
    pr: [[f32; LANES]; CHUNKS],
    pi: [[f32; LANES]; CHUNKS],
    gin: [[f32; LANES]; CHUNKS],
    gout: [[f32; LANES]; CHUNKS],
    gbow: [[f32; LANES]; CHUNKS],
    gbin: [[f32; LANES]; CHUNKS],
    re: [[f32; LANES]; CHUNKS],
    im: [[f32; LANES]; CHUNKS],
    /// The strike's and the microphone's weights, normalised.
    w_in: [f32; MAX_MODES],
    w_out: [f32; MAX_MODES],
    last: Option<ModalStep>,
    live: bool,
    /// Something was injected since the last grid point.
    moved: bool,
    /// The displacement at the bow a sample ago, and 1/ω₀: the bow meets the velocity (a
    /// displacement held still is no velocity — the stick must end, as on a string).
    bow_d: f32,
    bow_w: f32,
}

impl Modal {
    pub fn new(sample_rate: f32) -> Self {
        Modal {
            sr: sample_rate as f64,
            kind: ResKind::Bar,
            n: 0,
            ratio: [0.0; MAX_MODES],
            rel_t: [1.0; MAX_MODES],
            rel_p: f64::NAN,
            pr: [[0.0; LANES]; CHUNKS],
            pi: [[0.0; LANES]; CHUNKS],
            gin: [[0.0; LANES]; CHUNKS],
            gout: [[0.0; LANES]; CHUNKS],
            gbow: [[0.0; LANES]; CHUNKS],
            gbin: [[0.0; LANES]; CHUNKS],
            re: [[0.0; LANES]; CHUNKS],
            im: [[0.0; LANES]; CHUNKS],
            w_in: [0.0; MAX_MODES],
            w_out: [0.0; MAX_MODES],
            last: None,
            live: false,
            moved: false,
            bow_d: 0.0,
            bow_w: 1.0,
        }
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    pub fn clear(&mut self) {
        self.re = [[0.0; LANES]; CHUNKS];
        self.im = [[0.0; LANES]; CHUNKS];
        self.live = false;
        self.moved = false;
        self.bow_d = 0.0;
    }

    /// The modes in use.
    pub fn modes(&self) -> usize {
        self.n
    }

    /// Mode `k`'s amplitude now (the editor lights its partials with these).
    pub fn level(&self, k: usize) -> f32 {
        if k >= self.n {
            return 0.0;
        }
        let (c, l) = (k / LANES, k % LANES);
        math::hypot(self.re[c][l], self.im[c][l]) * self.w_out[k].abs()
    }

    /// Mode `k`'s frequency over the fundamental's.
    pub fn ratio(&self, k: usize) -> f64 {
        self.ratio.get(k).copied().unwrap_or(0.0)
    }

    /// A note starts: the object's partials (`modes` of them at most, stretched by
    /// `inharm`), its strike and microphone points, then the first step at once. `keep`: it
    /// is struck again while ringing.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        kind: ResKind,
        inharm: f64,
        modes: usize,
        pos: f64,
        listen: f64,
        step: &ModalStep,
        keep: bool,
    ) {
        if !keep || kind != self.kind {
            self.clear();
        }
        self.kind = kind;
        let mut table = [Partial {
            ratio: 0.0,
            m: 0,
            n: 0,
        }; MAX_MODES];
        let count = partials(kind, &mut table).min(modes.clamp(1, MAX_MODES));
        // Inharmonicity stretches (or squeezes) each partial's ratio as a power.
        let e = 1.0 + 0.12 * inharm.clamp(-1.0, 1.0);
        for (k, (r, t)) in self.ratio.iter_mut().zip(table.iter()).enumerate() {
            *r = if k < count {
                math::powf(t.ratio, e)
            } else {
                0.0
            };
        }
        self.n = count;
        self.rel_p = f64::NAN;
        self.place(&table, pos, listen);
        self.last = None;
        self.design(step);
        self.live = true;
    }

    /// Where it is struck and heard: the weights, normalised so the modes' products have
    /// a power of one (every object, every point, about the same loudness).
    pub fn place(&mut self, table: &[Partial; MAX_MODES], pos: f64, listen: f64) {
        let mut power = 0.0f64;
        let mut wi = [0.0f64; MAX_MODES];
        let mut wo = [0.0f64; MAX_MODES];
        for k in 0..self.n {
            wi[k] = shape(self.kind, &table[k], pos);
            wo[k] = shape(self.kind, &table[k], listen);
            power += (wi[k] * wo[k]) * (wi[k] * wo[k]);
        }
        let norm = if power > 1e-12 {
            1.0 / power.sqrt()
        } else {
            0.0
        };
        // The power shared out evenly between the two weights.
        let s = norm.sqrt();
        for k in 0..MAX_MODES {
            self.w_in[k] = (wi[k] * s) as f32;
            self.w_out[k] = (wo[k] * s) as f32;
        }
    }

    /// The next [`Self::design`] designs every mode again (its points moved).
    pub fn design_again(&mut self) {
        self.last = None;
    }

    /// Set the step: every mode's angle and decay. Only what moved is designed again.
    pub fn design(&mut self, s: &ModalStep) {
        if self.last == Some(*s) {
            return;
        }
        if self.rel_p != s.p {
            for k in 0..self.n {
                self.rel_t[k] = math::powf(self.ratio[k].max(1e-6), -s.p);
            }
            self.rel_p = s.p;
        }
        let nyq = 0.45 * self.sr;
        self.bow_w = (self.sr / (std::f64::consts::TAU * s.f0.max(1.0))) as f32;
        let mut bow_norm = 0.0f64;
        for k in 0..CHUNKS * LANES {
            let (c, l) = (k / LANES, k % LANES);
            if k >= self.n {
                self.pr[c][l] = 0.0;
                self.pi[c][l] = 0.0;
                self.gin[c][l] = 0.0;
                self.gout[c][l] = 0.0;
                self.gbow[c][l] = 0.0;
                self.gbin[c][l] = 0.0;
                continue;
            }
            let hz = s.f0 * self.ratio[k];
            // A mode near the top fades out rather than folding or clicking as a bend
            // carries it past.
            let fade = ((nyq - hz) / (0.05 * self.sr)).clamp(0.0, 1.0);
            let t = (s.t60 * self.rel_t[k]).max(1e-3);
            let r = math::exp(-6.907_755 / (t * self.sr));
            let w = std::f64::consts::TAU * hz.min(nyq) / self.sr;
            let (sn, cs) = math::sin_cos(w);
            self.pr[c][l] = (r * cs) as f32;
            self.pi[c][l] = (r * sn) as f32;
            self.gin[c][l] = self.w_in[k] * fade as f32;
            self.gout[c][l] = self.w_out[k];
            // A bow drives and hears the low modes (a glass's (2,0), a bar's first): those
            // under 4.5 × the fundamental, the lower the more. Drawn across every mode, the
            // modes would cancel at the bow while each grew without bound.
            let sel = if self.ratio[k] <= 4.5 {
                1.0 / self.ratio[k].max(0.25)
            } else {
                0.0
            };
            let wb = self.w_in[k] as f64 * fade * sel;
            self.gbin[c][l] = wb as f32;
            self.gbow[c][l] = wb as f32;
            bow_norm += wb * wb;
        }
        // The bow meets the velocity the modes hand back: scaled so one sample's push comes
        // back whole (the string's junction does the same).
        if bow_norm > 0.0 {
            let k = (1.0 / bow_norm) as f32;
            for c in 0..CHUNKS {
                for l in 0..LANES {
                    self.gbow[c][l] *= k;
                }
            }
        }
        self.last = Some(*s);
    }

    /// The fundamental's decay per sample, for a sustained exciter's compensation.
    pub fn fundamental_r(&self) -> f32 {
        math::hypot(self.pr[0][0], self.pi[0][0])
    }

    /// Render `out.len()` samples, **adding** `g.out` × the microphone's signal. The
    /// exciter injects `g.strike × imp + g.sustain × sus`; `bow` is the bow's velocity when
    /// `bowing`. `grid_end`: the slice ends on the control grid, where the gate may close.
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
        let chunks = self.n.div_ceil(LANES);
        let n = out.len().min(imp.len()).min(sus.len()).min(bow.len());
        let (gi, gs, gain) = (g.strike, g.sustain, g.out);
        let mut moved = self.moved;
        for i in 0..n {
            let x = gi * imp[i] + gs * sus[i];
            let mut xb = 0.0f32;
            if bowing {
                let mut acc = [0.0f32; LANES];
                for c in 0..chunks {
                    for (a, (g, m)) in acc.iter_mut().zip(self.gbow[c].iter().zip(&self.im[c])) {
                        *a += g * m;
                    }
                }
                let d = sum8(&acc);
                let vs = (d - self.bow_d) * self.bow_w;
                self.bow_d = d;
                let dv = bow[i] - vs;
                xb = dv * friction(dv, slope);
            }
            moved |= x != 0.0 || xb != 0.0;
            let mut acc = [0.0f32; LANES];
            for c in 0..chunks {
                for (j, a) in acc.iter_mut().enumerate() {
                    let (re, im) = (self.re[c][j], self.im[c][j]);
                    let nre = self.pr[c][j] * re - self.pi[c][j] * im
                        + self.gin[c][j] * x
                        + self.gbin[c][j] * xb;
                    let nim = self.pi[c][j] * re + self.pr[c][j] * im;
                    self.re[c][j] = nre;
                    self.im[c][j] = nim;
                    *a += self.gout[c][j] * nim;
                }
            }
            out[i] += gain * sum8(&acc);
        }
        self.moved = moved;
        if !grid_end {
            return self.live;
        }
        self.moved = false;
        if !moved {
            let mut peak = 0f32;
            for c in 0..chunks {
                for j in 0..LANES {
                    peak = peak.max(self.re[c][j].abs()).max(self.im[c][j].abs());
                }
            }
            if peak < STILL {
                self.clear();
            }
        }
        self.live
    }
}
