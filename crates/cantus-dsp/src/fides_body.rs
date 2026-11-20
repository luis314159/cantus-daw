//! Fides's body and output stage: the voices' sum radiated through a body's modes (a
//! guitar's air and top plate, a violin's signature modes and bridge hill, a harp's
//! soundboard, a box, a shell) into two microphones, then the tremolo, the low and high
//! cuts and the gain. One per instance, after the voices (a child of `fides`).
//!
//! **The method** is Chorda's in spirit (its radiation responses are measured modes) at a
//! fraction of the cost: a bank of resonant modes, each a complex one-pole — exact in
//! frequency, continuous in size and ring, so `body.size` and `body.decay` glide with no
//! response to rebuild. Its power gain to white noise is 1 at every setting.

use crate::math;

/// Modes in the largest body.
pub const BODY_MODES: usize = 40;

/// Which body, in `body.type`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum BodyKind {
    None,
    #[default]
    Guitar,
    Violin,
    Cello,
    Harp,
    Box,
    Shell,
}

impl BodyKind {
    pub const ALL: [BodyKind; 7] = [
        BodyKind::None,
        BodyKind::Guitar,
        BodyKind::Violin,
        BodyKind::Cello,
        BodyKind::Harp,
        BodyKind::Box,
        BodyKind::Shell,
    ];

    pub fn from_index(i: f32) -> BodyKind {
        let k = (i.round().max(0.0) as usize).min(Self::ALL.len() - 1);
        Self::ALL[k]
    }
}

/// One mode of a body: Hz, Q, and its weight in the left and right microphones.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct BodyMode {
    pub hz: f64,
    pub q: f64,
    pub left: f64,
    pub right: f64,
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

/// What a body is made of: its measured low modes (Hz, Q, weight) and the cloud of modes
/// above them (from, to, how many, Q from … to, the weight's slope in dB/oct, and a hill's
/// centre in Hz, width in octaves and height in dB).
struct Recipe {
    low: &'static [(f64, f64, f64)],
    cloud: (f64, f64, usize),
    q: (f64, f64),
    slope_db: f64,
    hill: (f64, f64, f64),
    salt: u64,
}

/// The bodies. Low modes: a classical guitar's air (A0) and top-plate modes (Fletcher &
/// Rossing, *The Physics of Musical Instruments*, §9.4, and Caldersmith's measurements); a
/// violin's signature modes A0, CBR, B1−, B1+ and its bridge hill near 2.5 kHz (Bissinger,
/// "Structural acoustics of good and bad violins", JASA 124, 2008; Jansson); a cello's the
/// violin's scaled to its size (A0 near 103 Hz); a concert harp's soundbox and soundboard
/// (Le Carrou et al., "Modelling of the concert harp's soundboard", Acta Acustica 2005); a
/// small wooden box's Helmholtz mode and walls (a kalimba's box); a metal shell's rings.
fn recipe(kind: BodyKind) -> Recipe {
    match kind {
        BodyKind::None | BodyKind::Guitar => Recipe {
            low: &[
                (98.0, 13.0, 1.0),
                (204.0, 30.0, 1.2),
                (390.0, 25.0, 0.8),
                (550.0, 30.0, 0.6),
                (660.0, 35.0, 0.5),
                (840.0, 35.0, 0.45),
                (970.0, 40.0, 0.4),
            ],
            cloud: (1100.0, 6500.0, 33),
            q: (40.0, 25.0),
            slope_db: -3.5,
            hill: (2500.0, 1.0, 0.0),
            salt: 1,
        },
        BodyKind::Violin => Recipe {
            low: &[
                (275.0, 15.0, 0.9),
                (405.0, 30.0, 0.5),
                (460.0, 25.0, 1.1),
                (540.0, 25.0, 1.2),
                (700.0, 30.0, 0.6),
                (850.0, 30.0, 0.55),
            ],
            cloud: (950.0, 7000.0, 34),
            q: (35.0, 20.0),
            slope_db: -2.5,
            hill: (2500.0, 0.7, 7.0),
            salt: 2,
        },
        BodyKind::Cello => Recipe {
            low: &[
                (103.0, 14.0, 0.9),
                (152.0, 28.0, 0.5),
                (172.0, 24.0, 1.1),
                (202.0, 24.0, 1.2),
                (262.0, 28.0, 0.6),
                (318.0, 30.0, 0.55),
                (390.0, 30.0, 0.5),
            ],
            cloud: (450.0, 5000.0, 33),
            q: (35.0, 22.0),
            slope_db: -3.0,
            hill: (1000.0, 0.8, 6.0),
            salt: 3,
        },
        BodyKind::Harp => Recipe {
            low: &[
                (70.0, 8.0, 0.8),
                (145.0, 20.0, 1.0),
                (230.0, 25.0, 0.9),
                (320.0, 25.0, 0.8),
                (410.0, 28.0, 0.7),
                (520.0, 30.0, 0.6),
            ],
            cloud: (620.0, 5000.0, 34),
            q: (35.0, 25.0),
            slope_db: -4.0,
            hill: (1500.0, 1.0, 0.0),
            salt: 4,
        },
        BodyKind::Box => Recipe {
            low: &[
                (210.0, 18.0, 1.1),
                (480.0, 30.0, 0.8),
                (720.0, 35.0, 0.6),
                (950.0, 35.0, 0.5),
            ],
            cloud: (1200.0, 6000.0, 36),
            q: (45.0, 30.0),
            slope_db: -4.5,
            hill: (2000.0, 1.0, 0.0),
            salt: 5,
        },
        BodyKind::Shell => Recipe {
            low: &[
                (180.0, 60.0, 0.9),
                (340.0, 70.0, 0.8),
                (520.0, 80.0, 0.7),
                (760.0, 80.0, 0.6),
                (1050.0, 90.0, 0.5),
            ],
            cloud: (1300.0, 8000.0, 35),
            q: (90.0, 60.0),
            slope_db: -3.0,
            hill: (3000.0, 1.0, 0.0),
            salt: 6,
        },
    }
}

/// A body's modes, ascending. `BodyKind::None` has none.
pub fn body_modes(kind: BodyKind, out: &mut [BodyMode; BODY_MODES]) -> usize {
    if kind == BodyKind::None {
        return 0;
    }
    let r = recipe(kind);
    let mut n = 0;
    for (i, (hz, q, w)) in r.low.iter().enumerate() {
        // The low modes radiate nearly alike to both microphones; a small spread.
        let s = 0.85 + 0.3 * hash(i as u64, r.salt + 100);
        out[n] = BodyMode {
            hz: *hz,
            q: *q,
            left: *w,
            right: *w * s,
        };
        n += 1;
    }
    let (from, to, count) = r.cloud;
    let span = math::log2(to / from);
    for i in 0..count {
        if n >= BODY_MODES {
            break;
        }
        let u = (i as f64 + 0.25 + 0.5 * hash(i as u64, r.salt)) / count as f64;
        let hz = from * math::exp2(u * span);
        let oct = math::log2(hz / from);
        let q = r.q.0 + (r.q.1 - r.q.0) * u;
        let hill = r.hill.2 * math::exp(-math::powi(math::log2(hz / r.hill.0) / r.hill.1, 2));
        let w = 0.45 * math::powf(10f64, (r.slope_db * oct + hill) / 20.0);
        let sign = |salt: u64| {
            if hash(i as u64, salt) < 0.5 {
                -1.0
            } else {
                1.0
            }
        };
        let amp = |salt: u64| 0.6 + 0.4 * hash(i as u64, salt);
        out[n] = BodyMode {
            hz,
            q,
            left: w * sign(r.salt + 10) * amp(r.salt + 20),
            right: w * sign(r.salt + 30) * amp(r.salt + 40),
        };
        n += 1;
    }
    n
}

/// The body's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BodySettings {
    pub kind: BodyKind,
    /// 0 … 1: dry … body.
    pub mix: f32,
    /// The body's dimensions over the measured one's: its modes at `hz / size`.
    pub size: f32,
    /// Its modes' ring over the measured one's.
    pub decay: f32,
}

impl Default for BodySettings {
    fn default() -> Self {
        BodySettings {
            kind: BodyKind::Guitar,
            mix: 0.6,
            size: 1.0,
            decay: 1.0,
        }
    }
}

const LANES: usize = 8;
const CHUNKS: usize = BODY_MODES / LANES;
/// A body whose every mode is below this is still: its state is zeroed.
const STILL: f32 = 1e-10;

/// The body on the audio thread: its modes as complex one-poles in chunks of eight (so the
/// compiler can run a chunk at once), summed lane by lane in a fixed order.
#[derive(Clone, Debug)]
pub struct Body {
    sr: f64,
    set: Option<BodySettings>,
    n: usize,
    pr: [[f32; LANES]; CHUNKS],
    pi: [[f32; LANES]; CHUNKS],
    gin: [[f32; LANES]; CHUNKS],
    gl: [[f32; LANES]; CHUNKS],
    gr: [[f32; LANES]; CHUNKS],
    re: [[f32; LANES]; CHUNKS],
    im: [[f32; LANES]; CHUNKS],
    /// The mix's two gains, dry and wet.
    dry: f32,
    wet: f32,
    still: bool,
}

impl Body {
    pub fn new(sample_rate: f32) -> Self {
        Body {
            sr: sample_rate as f64,
            set: None,
            n: 0,
            pr: [[0.0; LANES]; CHUNKS],
            pi: [[0.0; LANES]; CHUNKS],
            gin: [[0.0; LANES]; CHUNKS],
            gl: [[0.0; LANES]; CHUNKS],
            gr: [[0.0; LANES]; CHUNKS],
            re: [[0.0; LANES]; CHUNKS],
            im: [[0.0; LANES]; CHUNKS],
            dry: 1.0,
            wet: 0.0,
            still: true,
        }
    }

    /// Take new settings: the modes are designed again only when the kind, size or ring
    /// changed (a mix alone is two gains). No allocation.
    pub fn set(&mut self, s: &BodySettings) {
        let shape_moved = self
            .set
            .is_none_or(|o| o.kind != s.kind || o.size != s.size || o.decay != s.decay);
        if shape_moved {
            self.design(s);
        }
        self.set = Some(*s);
        let mix = if self.n == 0 {
            0.0
        } else {
            s.mix.clamp(0.0, 1.0)
        };
        self.dry = 1.0 - mix;
        self.wet = mix;
    }

    /// The mix alone (a modulated mix moves only the two gains).
    pub fn set_mix(&mut self, mix: f32) {
        let mix = if self.n == 0 {
            0.0
        } else {
            mix.clamp(0.0, 1.0)
        };
        self.dry = 1.0 - mix;
        self.wet = mix;
    }

    fn design(&mut self, s: &BodySettings) {
        let mut modes = [BodyMode::default(); BODY_MODES];
        let n = body_modes(s.kind, &mut modes);
        let nyq = 0.45 * self.sr;
        let mut power = 0.0f64;
        let mut coefs = [(0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64); BODY_MODES];
        let mut used = 0;
        for m in modes.iter().take(n) {
            let hz = m.hz / s.size.max(0.1) as f64;
            if hz >= nyq {
                continue;
            }
            let w = std::f64::consts::TAU * hz / self.sr;
            let q = m.q * s.decay.max(0.05) as f64;
            let r = math::exp(-w / (2.0 * q));
            // The mode's input so its power gain is its weight squared, both microphones.
            let g = (1.0 - r * r).sqrt();
            let (sn, cs) = math::sin_cos(w);
            power += 0.5 * (m.left * m.left + m.right * m.right);
            coefs[used] = (r * cs, r * sn, g, m.left, m.right);
            used += 1;
        }
        // Each mode's Im part carries half its power; the two microphones, one each. A
        // tone right on a mode is held to +10 dB over the dry (a sharp mode would sing
        // 25 dB over it at a power of one: a wolf, not a body).
        let mut peak = 0.0f64;
        for c in coefs.iter().take(used) {
            let r = (c.0 * c.0 + c.1 * c.1).sqrt();
            let w = c.3.abs().max(c.4.abs());
            peak = peak.max(w * c.2 / (1.0 - r).max(1e-9));
        }
        let norm = if power > 0.0 {
            (2.0 / power).sqrt().min(3.16 / peak.max(1e-9))
        } else {
            0.0
        };
        self.n = used;
        for k in 0..CHUNKS * LANES {
            let (c, l) = (k / LANES, k % LANES);
            let (pr, pi, g, gl, gr) = if k < used {
                coefs[k]
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0)
            };
            self.pr[c][l] = pr as f32;
            self.pi[c][l] = pi as f32;
            self.gin[c][l] = g as f32;
            self.gl[c][l] = (gl * norm) as f32;
            self.gr[c][l] = (gr * norm) as f32;
        }
    }

    /// Whether every mode is still (its state exact zeros).
    pub fn is_still(&self) -> bool {
        self.still
    }

    pub fn reset(&mut self) {
        self.re = [[0.0; LANES]; CHUNKS];
        self.im = [[0.0; LANES]; CHUNKS];
        self.still = true;
    }

    /// Radiate `left`/`right` in place. `quiet`: the input is exact zeros, so a still body
    /// passes them untouched.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], quiet: bool) {
        if self.n == 0 || self.wet == 0.0 {
            // The dry gain is exactly 1 with no body.
            return;
        }
        if quiet && self.still {
            return;
        }
        let chunks = self.n.div_ceil(LANES);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let x = 0.5 * (*l + *r);
            let mut accl = [0.0f32; LANES];
            let mut accr = [0.0f32; LANES];
            for c in 0..chunks {
                for j in 0..LANES {
                    let (re, im) = (self.re[c][j], self.im[c][j]);
                    let nre = self.pr[c][j] * re - self.pi[c][j] * im + self.gin[c][j] * x;
                    let nim = self.pi[c][j] * re + self.pr[c][j] * im;
                    self.re[c][j] = nre;
                    self.im[c][j] = nim;
                    accl[j] += self.gl[c][j] * nim;
                    accr[j] += self.gr[c][j] * nim;
                }
            }
            let (bl, br) = (sum8(&accl), sum8(&accr));
            *l = self.dry * *l + self.wet * bl;
            *r = self.dry * *r + self.wet * br;
        }
        if quiet {
            let mut peak = 0f32;
            for c in 0..chunks {
                for j in 0..LANES {
                    peak = peak.max(self.re[c][j].abs()).max(self.im[c][j].abs());
                }
            }
            if peak < STILL {
                self.reset();
            } else {
                self.still = false;
            }
        } else {
            self.still = false;
        }
    }
}

/// Eight lanes summed in a fixed order (pairs, then pairs of pairs).
#[inline]
pub fn sum8(a: &[f32; 8]) -> f32 {
    ((a[0] + a[1]) + (a[2] + a[3])) + ((a[4] + a[5]) + (a[6] + a[7]))
}

/// The output stage: the tremolo (its phase the stream's clock, so a skip at rest keeps
/// it), the low and high cuts (12 dB/oct; off at 10 Hz and 20 kHz, where they touch no
/// sample) and the gain.
#[derive(Clone, Debug)]
pub struct Output {
    sr: f32,
    lo: [super::exciter::Biquad; 2],
    hi: [super::exciter::Biquad; 2],
    lo_on: bool,
    hi_on: bool,
    gain: f32,
    trem_rate: f32,
    still: bool,
}

impl Output {
    pub fn new(sample_rate: f32) -> Self {
        Output {
            sr: sample_rate,
            lo: Default::default(),
            hi: Default::default(),
            lo_on: false,
            hi_on: false,
            gain: 1.0,
            trem_rate: 5.0,
            still: true,
        }
    }

    pub fn set(&mut self, s: &super::FidesSettings) {
        use super::exciter::Biquad;
        self.lo_on = s.lowcut_hz > 10.0;
        self.hi_on = s.highcut_hz < 20_000.0 && s.highcut_hz < 0.45 * self.sr;
        let lo = Biquad::pass(s.lowcut_hz, true, self.sr);
        let hi = Biquad::pass(s.highcut_hz, false, self.sr);
        for f in self.lo.iter_mut() {
            f.set(lo);
        }
        for f in self.hi.iter_mut() {
            f.set(hi);
        }
        self.gain = math::db_to_gain(s.gain_db);
        self.trem_rate = s.trem_rate_hz;
    }

    pub fn is_still(&self) -> bool {
        self.still
    }

    pub fn reset(&mut self) {
        for f in self.lo.iter_mut().chain(self.hi.iter_mut()) {
            f.clear();
        }
        self.still = true;
    }

    /// In place, from the stream's sample `clock`; `trem` the tremolo's depth now.
    /// `quiet`: the input is exact zeros.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], clock: u64, trem: f32, quiet: bool) {
        let filters = self.lo_on || self.hi_on;
        if quiet && (self.still || !filters) {
            self.still = true;
            return;
        }
        if trem > 0.0 {
            let rate = self.trem_rate as f64 / self.sr as f64;
            for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                let ph = ((clock + i as u64) as f64 * rate).fract() as f32;
                let g = 1.0 - trem * (0.5 - 0.5 * math::cos(std::f32::consts::TAU * ph));
                *a *= g;
                *b *= g;
            }
        }
        if filters {
            for (a, b) in l.iter_mut().zip(r.iter_mut()) {
                if self.lo_on {
                    *a = self.lo[0].tick(*a);
                    *b = self.lo[1].tick(*b);
                }
                if self.hi_on {
                    *a = self.hi[0].tick(*a);
                    *b = self.hi[1].tick(*b);
                }
            }
            if quiet {
                let peak = self
                    .lo
                    .iter()
                    .chain(self.hi.iter())
                    .fold(0f32, |m, f| m.max(f.peak()));
                if peak < 1e-10 {
                    self.reset();
                    for x in l.iter_mut().chain(r.iter_mut()) {
                        *x = 0.0;
                    }
                }
            } else {
                self.still = false;
            }
        }
        if self.gain != 1.0 {
            for x in l.iter_mut().chain(r.iter_mut()) {
                *x *= self.gain;
            }
        }
    }
}
