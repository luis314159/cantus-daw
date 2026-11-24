//! The upper strings' bodies (round 42, `arcus-upper`; a child of `arcus::upper`): the
//! violin's bridge admittance **fitted to a measured one** and the viola's from it, as a
//! bank of modes the bridge's force drives.
//!
//! **The data.** The CNSM dataset (Pauget Ballesteros, Lalitte, Lostanlen, Fritz, Zenodo
//! 10.5281/zenodo.18696786, **CC BY 4.0**): the input admittance at the bridge of three
//! violins (Klimke, Levaggi, Stoppani) by laser vibrometer, 0 – 25.6 kHz every 1.5625 Hz.
//! `examples/arcus_upper_body.rs` (`tools/arcus/upper-body.sh`) averages one violin's
//! measurements, fits a sum of modes to it and writes `arcus_upper_body_data.rs`.
//!
//! **The model.** Each mode is the admittance of one resonance, `Y_k(s) = A_k·(ω_k/Q_k)·s /
//! (s² + (ω_k/Q_k)·s + ω_k²)` — at its own frequency it is `A_k`, at DC and far above
//! nothing — with `A_k` real (a driving-point admittance's modes are each a positive mass
//! term; the fit lets a sign go where it helps). It runs as a complex one-pole per mode
//! (impulse invariance: `h_k[n] = 2T·Re(R_k·pⁿ)`, `R_k` the pole's residue), so a mode's
//! frequency and ring are exact and continuous in `body.size` and `body.ring`. The two
//! microphones hear the low modes alike and the modes above 1 kHz each with its own weight
//! (a violin's radiation turns from mode to mode up there — Weinreich's directional tone
//! colour, *JASA* 101, 1997, doi:10.1121/1.418150: abstract read).

use crate::math;

/// Modes in the largest body.
pub const MODES: usize = 64;

/// One fitted mode: Hz, Q, and its admittance at its own frequency (the fit's units: the
/// largest mode is 1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Mode {
    pub hz: f32,
    pub q: f32,
    pub a: f32,
}

/// A deterministic hash in [0, 1).
fn hash(i: u64, salt: u64) -> f64 {
    let mut z = i
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(salt.wrapping_mul(0xD1B5_4A32_D192_ED03));
    z ^= z >> 30;
    z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// What the body is set to: its modes, their scale and ring, the mix.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shape {
    pub modes: &'static [Mode],
    /// The body's size over the measured one's: every mode at `hz / size`.
    pub size: f32,
    /// The modes' ring over the measured one's (Q ×).
    pub ring: f32,
    pub mix: f32,
}

/// The body on the audio thread.
#[derive(Clone, Debug)]
pub struct UpperBody {
    sr: f64,
    shape: Option<Shape>,
    n: usize,
    pr: [f32; MODES],
    pi: [f32; MODES],
    /// Each microphone's weights on the state's real and imaginary parts.
    lr: [f32; MODES],
    li: [f32; MODES],
    rr: [f32; MODES],
    ri: [f32; MODES],
    re: [f32; MODES],
    im: [f32; MODES],
    dry: f32,
    wet: f32,
    still: bool,
    fed: bool,
}

/// A body whose every mode is below this is still: its state is zeroed.
const STILL: f32 = 1e-10;

impl UpperBody {
    pub fn new(sample_rate: f32) -> Self {
        UpperBody {
            sr: sample_rate as f64,
            shape: None,
            n: 0,
            pr: [0.0; MODES],
            pi: [0.0; MODES],
            lr: [0.0; MODES],
            li: [0.0; MODES],
            rr: [0.0; MODES],
            ri: [0.0; MODES],
            re: [0.0; MODES],
            im: [0.0; MODES],
            dry: 1.0,
            wet: 0.0,
            still: true,
            fed: false,
        }
    }

    /// In use (an upper string's body is set).
    pub fn in_use(&self) -> bool {
        self.n > 0
    }

    /// Take a shape (`None`: not in use — a cello's, a bass's). The modes are designed
    /// again only when they, the size or the ring changed. No allocation.
    pub fn set(&mut self, shape: Option<Shape>) {
        let Some(sh) = shape else {
            self.shape = None;
            self.n = 0;
            if !self.still {
                self.reset();
            }
            return;
        };
        let moved = self.shape.is_none_or(|o| {
            !std::ptr::eq(o.modes, sh.modes) || o.size != sh.size || o.ring != sh.ring
        });
        if moved {
            self.design(&sh);
        }
        self.shape = Some(sh);
        let mix = if self.n == 0 {
            0.0
        } else {
            sh.mix.clamp(0.0, 1.0)
        };
        self.dry = 1.0 - mix;
        self.wet = mix;
    }

    fn design(&mut self, sh: &Shape) {
        let t = 1.0 / self.sr;
        let nyq = 0.45 * self.sr;
        let mut used = 0;
        let mut coefs = [(0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64); MODES];
        for (k, m) in sh.modes.iter().take(MODES).enumerate() {
            let hz = m.hz as f64 / sh.size.max(0.1) as f64;
            if hz >= nyq {
                continue;
            }
            let w = std::f64::consts::TAU * hz;
            let q = (m.q * sh.ring.max(0.05)) as f64;
            let sigma = w / (2.0 * q);
            let wd = (w * w - sigma * sigma).max(1e-6).sqrt();
            // The pole, and the bandpass mode's residue there: R = A·(ω/Q)·p / (2j·ω_d).
            let r = math::exp(-sigma * t);
            let (sn, cs) = math::sin_cos(wd * t);
            let (pre, pim) = (-sigma, wd);
            let k0 = m.a as f64 * (w / q) / (2.0 * wd);
            // p / j = (pim, −pre)
            let (rre, rim) = (k0 * pim, -k0 * pre);
            // y = 2T·Re(R·s) = 2T·(Rr·Re s − Ri·Im s)
            let (wr, wi) = (2.0 * t * rre, -2.0 * t * rim);
            let spread = if hz < 1000.0 {
                0.9 + 0.2 * hash(k as u64, 7)
            } else {
                0.55 + 0.9 * hash(k as u64, 11)
            };
            coefs[used] = (r * cs, r * sn, wr, wi, wr * spread, wi * spread);
            used += 1;
        }
        // The body's power gain to white noise is one (each microphone): the sum of the
        // squared impulse responses, from each mode's own and the pairs' cross terms.
        let mut power = [0.0f64; 2];
        for (ch, pw) in power.iter_mut().enumerate() {
            let mut acc = 0.0f64;
            for a in coefs.iter().take(used) {
                for b in coefs.iter().take(used) {
                    acc += cross(a, b, ch);
                }
            }
            *pw = acc.max(1e-30);
        }
        self.n = used;
        let (nl, nr) = (1.0 / power[0].sqrt(), 1.0 / power[1].sqrt());
        for (k, cf) in coefs.iter().enumerate() {
            let c = if k < used {
                *cf
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };
            self.pr[k] = c.0 as f32;
            self.pi[k] = c.1 as f32;
            self.lr[k] = (c.2 * nl) as f32;
            self.li[k] = (c.3 * nl) as f32;
            self.rr[k] = (c.4 * nr) as f32;
            self.ri[k] = (c.5 * nr) as f32;
        }
    }

    pub fn is_still(&self) -> bool {
        self.still
    }

    pub fn reset(&mut self) {
        self.re = [0.0; MODES];
        self.im = [0.0; MODES];
        self.still = true;
        self.fed = false;
    }

    /// Radiate `left`/`right` in place (the sum of the voices: the bridge's force), from the
    /// stream's sample `clock`; `quiet`, the input is exact zeros. Not in use, it touches
    /// nothing. Whether it is still is asked on the control grid only.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], clock: u64, quiet: bool) {
        if self.n == 0 || self.wet == 0.0 {
            if !self.still {
                self.reset();
            }
            return;
        }
        if quiet && self.still {
            return;
        }
        let n = self.n;
        let grid = super::super::CTRL as u64;
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            if self.still {
                if *l == 0.0 && *r == 0.0 {
                    continue;
                }
                self.still = false;
            }
            let x = 0.5 * (*l + *r);
            self.fed |= x != 0.0;
            let (mut yl, mut yr) = (0.0f32, 0.0f32);
            for k in 0..n {
                let (re, im) = (self.re[k], self.im[k]);
                let nre = self.pr[k] * re - self.pi[k] * im + x;
                let nim = self.pi[k] * re + self.pr[k] * im;
                self.re[k] = nre;
                self.im[k] = nim;
                yl += self.lr[k] * nre + self.li[k] * nim;
                yr += self.rr[k] * nre + self.ri[k] * nim;
            }
            *l = self.dry * *l + self.wet * yl;
            *r = self.dry * *r + self.wet * yr;
            if (clock + i as u64 + 1).is_multiple_of(grid) && !std::mem::take(&mut self.fed) {
                let peak = self.re[..n]
                    .iter()
                    .chain(&self.im[..n])
                    .fold(0f32, |m, v| m.max(v.abs()));
                if peak < STILL {
                    self.reset();
                }
            }
        }
    }
}

/// The cross term `Σₙ y_a[n]·y_b[n]` of two modes' impulse responses on microphone `ch`
/// (`y[n] = wr·Re pⁿ + wi·Im pⁿ`), in closed form.
fn cross(a: &(f64, f64, f64, f64, f64, f64), b: &(f64, f64, f64, f64, f64, f64), ch: usize) -> f64 {
    let (wa, wb) = if ch == 0 {
        ((a.2, a.3), (b.2, b.3))
    } else {
        ((a.4, a.5), (b.4, b.5))
    };
    // y = Re(c·pⁿ) with c = wr − j·wi; Σ Re(ca pa ⁿ)·Re(cb pb ⁿ) =
    // ½ Re(ca·cb / (1 − pa·pb)) + ½ Re(ca·conj(cb) / (1 − pa·conj(pb))).
    let ca = (wa.0, -wa.1);
    let cb = (wb.0, -wb.1);
    let pa = (a.0, a.1);
    let pb = (b.0, b.1);
    let mul = |x: (f64, f64), y: (f64, f64)| (x.0 * y.0 - x.1 * y.1, x.0 * y.1 + x.1 * y.0);
    let conj = |x: (f64, f64)| (x.0, -x.1);
    let div_re = |num: (f64, f64), den: (f64, f64)| {
        let d = den.0 * den.0 + den.1 * den.1;
        (num.0 * den.0 + num.1 * den.1) / d.max(1e-30)
    };
    let one_minus = |x: (f64, f64)| (1.0 - x.0, -x.1);
    let t1 = div_re(mul(ca, cb), one_minus(mul(pa, pb)));
    let t2 = div_re(mul(ca, conj(cb)), one_minus(mul(pa, conj(pb))));
    0.5 * (t1 + t2)
}

#[path = "arcus_upper_body_data.rs"]
mod data;

/// The violin's modes (fitted, `arcus_upper_body_data.rs`).
pub const VIOLIN: &[Mode] = &data::VIOLIN;

/// The viola's body over the violin's: its size, not its tuning — the study's database
/// (`study/bowed-realism.md` §3.13) measured the viola's A0 at 250 Hz (223 – 250) against the
/// violin's 281, a ratio of 1.12 (a 16-inch body over a violin's 356 mm); no measured viola
/// admittance exists in the open (`strings-verified.md` §3.1). Scaled so, its A0 sits about
/// 1.9 × over its C string (the database: 1.91), the viola's veiled lowest octave.
pub const VIOLA_SCALE: f32 = 1.12;

/// The shape an instrument's settings ask of the upper strings' body: `None` for the cello's
/// and the bass's (they keep Fides's).
pub fn shape_of(s: &super::super::ArcusSettings) -> Option<Shape> {
    use super::super::tables::Instrument;
    let scale = match s.instrument {
        Instrument::Violin => 1.0,
        Instrument::Viola => VIOLA_SCALE,
        Instrument::Cello | Instrument::Bass => return None,
    };
    Some(Shape {
        modes: VIOLIN,
        size: scale * s.body_size,
        ring: s.body_ring,
        mix: s.body_mix,
    })
}
