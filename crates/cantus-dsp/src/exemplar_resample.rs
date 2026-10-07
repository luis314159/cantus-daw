//! Exemplar's resampler: octave mip-maps, a polyphase windowed-sinc interpolator running at
//! twice the output rate, and a half-band decimator — L. de Soras, *The Quest For The
//! Perfect Resampler* (2005), `research/instruments/sampler.md` §4.1.
//!
//! A sample is kept at levels 0, 1, 2 … where level *k* is the audio low-passed and
//! decimated by 2^k ([`halve`], built off the audio thread). A voice playing at ratio *r*
//! (source frames per output frame) reads the level *k* whose local ratio r/2^k lies in
//! [1, 2): its content then reaches at most the oversampled domain's Nyquist, so the
//! interpolator — run at 2× the output rate — never folds anything, and the half-band
//! decimator removes everything above the output band before it is kept. The cost per
//! output sample is the same at +1 and at +36 semitones; the memory is at most twice the
//! sample (½ + ¼ + …).
//!
//! Everything here is plain arithmetic on tables built in `new`: no allocation per sample.

/// Mip-map levels a sample can have (level 7 plays 2⁸ = 256× faster than the file).
use crate::math;

pub const MAX_LEVELS: usize = 8;
/// Phases in the interpolator's table; coefficients between two phases are interpolated
/// linearly, which keeps the phase error far below the kernel's own.
pub const PHASES: usize = 256;
/// The longest kernel (the Best tier).
pub const MAX_TAPS: usize = 64;
/// The longest runtime half-band decimator.
pub const MAX_HB: usize = 160;

/// How the voices read their samples. The same in real time and offline (ADR 0023 rule 1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Quality {
    /// Linear interpolation on the file itself: cheap, and it aliases (lo-fi on purpose).
    Draft,
    /// Mip-maps, 16-tap sinc at 2×, 80 dB half-band.
    Standard,
    /// Mip-maps, 32-tap sinc at 2×, 100 dB half-band. The default.
    #[default]
    High,
    /// Mip-maps, 64-tap sinc at 2×, 120 dB half-band.
    Best,
}

impl Quality {
    pub fn from_index(v: f32) -> Quality {
        match v.round() as i32 {
            0 => Quality::Draft,
            1 => Quality::Standard,
            3 => Quality::Best,
            _ => Quality::High,
        }
    }
    /// (kernel taps, Kaiser β, decimator attenuation in dB).
    fn design(self) -> (usize, f64, f64) {
        match self {
            Quality::Draft => (2, 0.0, 0.0),
            Quality::Standard => (16, 6.0, 80.0),
            Quality::High => (32, 9.0, 100.0),
            Quality::Best => (64, 11.5, 120.0),
        }
    }
}

/// The zeroth-order modified Bessel function, for the Kaiser window.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Kaiser window at `x` ∈ [−1, 1].
fn kaiser(x: f64, beta: f64) -> f64 {
    if x.abs() > 1.0 {
        return 0.0;
    }
    bessel_i0(beta * (1.0 - x * x).sqrt()) / bessel_i0(beta)
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        math::sin(px) / px
    }
}

/// A half-band low-pass FIR of length `4m − 1` (cut-off at a quarter of its rate): the
/// centre tap is ½ and every even offset from it is exactly zero. Normalised to unity DC.
pub fn halfband(m: usize, beta: f64) -> Vec<f64> {
    let len = 4 * m - 1;
    let c = (len - 1) as f64 / 2.0;
    let mut h: Vec<f64> = (0..len)
        .map(|n| {
            let k = n as f64 - c;
            let ki = k.round() as i64;
            if ki != 0 && ki % 2 == 0 {
                0.0
            } else {
                0.5 * sinc(0.5 * k) * kaiser(k / (c + 1.0), beta)
            }
        })
        .collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|x| *x /= sum);
    h
}

/// The `m` of a half-band reaching `atten_db` with a transition `width` (a fraction of its
/// rate), and the Kaiser β for it — the Kaiser design formulas.
pub fn halfband_size(atten_db: f64, width: f64) -> (usize, f64) {
    let beta = if atten_db > 50.0 {
        0.1102 * (atten_db - 8.7)
    } else {
        0.5842 * math::powf((atten_db - 21.0).max(0.0), 0.4) + 0.07886 * (atten_db - 21.0).max(0.0)
    };
    let len = (atten_db - 7.95) / (2.285 * 2.0 * std::f64::consts::PI * width) + 1.0;
    ((((len + 1.0) / 4.0).ceil() as usize).max(2), beta)
}

/// The mip-map step: low-pass at a quarter of the rate (120 dB from 0.275 of the rate,
/// flat to 0.225) and keep every other frame. `data` is interleaved; the result has
/// `(frames + 1) / 2` frames. Off the audio thread (it allocates).
pub fn halve(data: &[f32], channels: usize) -> Vec<f32> {
    halve_with(data, channels, false)
}

/// [`halve`] for a signal that repeats (a single cycle looped end to end): the filter
/// wraps around instead of seeing silence past the ends, so every level is the same cycle,
/// band-limited, with no seam. A cycle of 2^n frames halves exactly.
pub fn halve_periodic(data: &[f32], channels: usize) -> Vec<f32> {
    halve_with(data, channels, true)
}

fn halve_with(data: &[f32], channels: usize, periodic: bool) -> Vec<f32> {
    let ch = channels.max(1);
    let frames = data.len() / ch;
    if frames == 0 {
        return Vec::new();
    }
    let (m, beta) = halfband_size(120.0, 0.05);
    let h = halfband(m, beta);
    let c = (h.len() / 2) as i64;
    let out_frames = frames.div_ceil(2);
    let mut out = vec![0.0f32; out_frames * ch];
    // Only the centre and the odd offsets are non-zero: sum those.
    let odd: Vec<(i64, f64)> = (1..=c)
        .step_by(2)
        .map(|k| (k, h[(c + k) as usize]))
        .collect();
    let f = frames as i64;
    let at = |i: i64, chn: usize| -> f64 {
        let i = if periodic { i.rem_euclid(f) } else { i };
        if i < 0 || i >= f {
            0.0
        } else {
            data[i as usize * ch + chn] as f64
        }
    };
    for n in 0..out_frames {
        let centre = 2 * n as i64;
        let inside = centre - c >= 0 && centre + c < f;
        for chn in 0..ch {
            let mut acc;
            if inside {
                let x = |i: i64| data[i as usize * ch + chn] as f64;
                acc = 0.5 * x(centre);
                for &(k, coef) in &odd {
                    acc += coef * (x(centre - k) + x(centre + k));
                }
            } else {
                acc = 0.5 * at(centre, chn);
                for &(k, coef) in &odd {
                    acc += coef * (at(centre - k, chn) + at(centre + k, chn));
                }
            }
            out[n * ch + chn] = acc as f32;
        }
    }
    out
}

/// Every mip level of a sample above level 0, down to 16 frames or `max` levels.
pub fn mip_chain(data: &[f32], channels: usize, max: usize, periodic: bool) -> Vec<Vec<f32>> {
    let ch = channels.max(1);
    let mut out: Vec<Vec<f32>> = Vec::new();
    for _ in 1..=max.min(MAX_LEVELS - 1) {
        let prev = out.last().map(Vec::as_slice).unwrap_or(data);
        if prev.len() / ch < 32 {
            break;
        }
        out.push(halve_with(prev, ch, periodic));
    }
    out
}

/// The windowed-sinc interpolator's table: `PHASES + 1` rows of `taps` coefficients, and
/// each row's difference to the next, so a coefficient at any fraction is one multiply-add.
#[derive(Clone, Debug)]
pub struct Kernel {
    pub taps: usize,
    table: Vec<f32>,
    delta: Vec<f32>,
}

impl Kernel {
    /// A kernel of `taps` (even) taps with a Kaiser window of `beta`, cut off at the
    /// source's Nyquist. Row 0 is exactly the identity, so an integral position returns
    /// the sample itself.
    pub fn new(taps: usize, beta: f64) -> Kernel {
        let half = taps as f64 / 2.0;
        let mut table = vec![0.0f32; (PHASES + 1) * taps];
        for p in 0..=PHASES {
            let frac = p as f64 / PHASES as f64;
            let row = &mut table[p * taps..(p + 1) * taps];
            if p == 0 || p == PHASES {
                row[taps / 2 - 1 + usize::from(p == PHASES)] = 1.0;
                continue;
            }
            let mut sum = 0.0;
            let mut vals = [0.0f64; MAX_TAPS];
            for (j, v) in vals.iter_mut().enumerate().take(taps) {
                let t = j as f64 - (half - 1.0) - frac;
                *v = sinc(t) * kaiser(t / half, beta);
                sum += *v;
            }
            for (r, v) in row.iter_mut().zip(&vals[..taps]) {
                *r = (v / sum) as f32;
            }
        }
        let mut delta = vec![0.0f32; (PHASES + 1) * taps];
        for p in 0..PHASES {
            for j in 0..taps {
                delta[p * taps + j] = table[(p + 1) * taps + j] - table[p * taps + j];
            }
        }
        Kernel { taps, table, delta }
    }

    /// The coefficients for a position `frac` ∈ [0, 1) past a sample, into `out[..taps]`.
    /// Tap `j` weighs the frame `floor(pos) − (taps/2 − 1) + j`.
    #[inline]
    pub fn coefs(&self, frac: f64, out: &mut [f32; MAX_TAPS]) {
        let x = frac * PHASES as f64;
        let p = (x as usize).min(PHASES - 1);
        let a = (x - p as f64) as f32;
        let t = self.taps;
        let row = &self.table[p * t..p * t + t];
        let d = &self.delta[p * t..p * t + t];
        for ((o, r), dd) in out[..t].iter_mut().zip(row).zip(d) {
            *o = r + a * dd;
        }
    }
}

/// The runtime half-band decimator's coefficients (the voice keeps its own history).
#[derive(Clone, Debug)]
pub struct Decimator {
    /// Coefficients at offsets ±1, ±3, … from the centre (the centre is ½).
    pub odd: Vec<f32>,
    /// The same, last first (for the half of the window before the centre).
    pub rev: Vec<f32>,
    /// Length of the filter, `4m − 1`.
    pub len: usize,
}

impl Decimator {
    /// A decimator from 2·`fs` to `fs` keeping `pass` Hz and reaching `atten_db` by
    /// `fs − pass` (anything above folds back above `pass`, out of the kept band).
    pub fn new(fs: f64, pass: f64, atten_db: f64) -> Decimator {
        let width = ((fs - 2.0 * pass) / (2.0 * fs)).max(0.02);
        let (m, beta) = halfband_size(atten_db, width);
        let m = m.min(MAX_HB.div_ceil(4));
        let h = halfband(m, beta);
        let c = h.len() / 2;
        let odd: Vec<f32> = (1..=c).step_by(2).map(|k| h[c + k] as f32).collect();
        let rev = odd.iter().rev().copied().collect();
        Decimator {
            odd,
            rev,
            len: h.len(),
        }
    }

    /// Group delay in oversampled samples.
    pub fn delay(&self) -> usize {
        self.len / 2
    }
}

/// Everything a voice needs to read with one quality: the kernel and the decimator.
#[derive(Clone, Debug)]
pub struct Reader {
    pub quality: Quality,
    pub kernel: Kernel,
    pub dec: Decimator,
}

impl Reader {
    pub fn new(quality: Quality, fs: f32) -> Reader {
        let (taps, beta, atten) = quality.design();
        let fs = fs as f64;
        let pass = (0.42 * fs).min(20_000.0);
        Reader {
            quality,
            kernel: Kernel::new(taps.max(2), beta),
            dec: Decimator::new(fs, pass, atten.max(60.0)),
        }
    }
}
