//! Oversampling for Comprimo's gain stage and detector: linear-phase half-band FIRs, 2× or
//! 4×, with a latency that is a whole number of samples at the base rate — so delay
//! compensation lines the strip up exactly, and the dry path is matched with a plain
//! delay.
//!
//! Why a compressor wants it: a fast attack multiplies the audio by a gain that moves
//! quickly, and the sidebands that multiplication makes reach past Nyquist and fold back
//! as aliasing. Applied at 2× or 4× the rate, with the gain interpolated between the base
//! samples, those sidebands land where the down-sampling filter removes them. The key is
//! up-sampled through the same filter, so the detector hears the peaks *between* samples
//! (a true-peak detector) and stays exactly in step with the audio it controls.
//!
//! The filters are half-bands with a Kaiser window: the first stage 63 taps, flat to
//! 20 kHz and at least 80 dB down from 28 kHz at 48 kHz; the second (for 4×) 19 taps.
//! Every other tap of a half-band is zero and one phase of each up-sampler is a pure
//! delay, so each stage costs about a quarter of a plain FIR.

/// Taps of the first (1×↔2×) and the second (2×↔4×) stage: both 4k+3, as a half-band
/// with its centre tap on an odd index needs.
const TAPS_A: usize = 63;
const TAPS_B: usize = 19;
/// Kaiser β for about 80 dB of stop band.
const BETA: f64 = 7.86;

/// The oversampling factors, by the `os` parameter's index.
pub const FACTORS: [usize; 3] = [1, 2, 4];

/// The latency oversampling adds at `factor`, in base-rate samples: the first stage's up
/// and down filters (its centre, `c_a`, samples at the base rate), and the second's plus
/// half a sample of padding at 4× (so the sum is whole).
pub fn latency(factor: usize) -> usize {
    let c_a = (TAPS_A - 1) / 2;
    let c_b = (TAPS_B - 1) / 2;
    match factor {
        2 => c_a,
        4 => c_a + (2 * c_b + PAD_4X) / 4,
        _ => 0,
    }
}

/// Samples of delay at 4× that round the second stage up to whole base samples.
const PAD_4X: usize = 2;

/// A half-band low-pass at a quarter of its (high) rate, windowed by Kaiser.
fn half_band(taps: usize) -> Vec<f64> {
    let c = (taps - 1) as f64 / 2.0;
    let i0 = |x: f64| {
        // The modified Bessel function of the first kind, order 0, by its series.
        let (mut sum, mut term) = (1.0f64, 1.0f64);
        let q = x * x / 4.0;
        for k in 1..100 {
            term *= q / (k * k) as f64;
            sum += term;
            if term < 1e-17 * sum {
                break;
            }
        }
        sum
    };
    let norm = i0(BETA);
    let h: Vec<f64> = (0..taps)
        .map(|n| {
            let t = n as f64 - c;
            let ideal = if t == 0.0 {
                0.5
            } else {
                (std::f64::consts::PI * t / 2.0).sin() / (std::f64::consts::PI * t)
            };
            let r = t / c;
            ideal * i0(BETA * (1.0 - r * r).max(0.0).sqrt()) / norm
        })
        .collect();
    // Normalise the DC gain to exactly one: the centre stays 0.5, the side taps share the
    // other half.
    let side: f64 = h
        .iter()
        .enumerate()
        .filter(|(n, _)| (*n as f64 - c) != 0.0)
        .map(|(_, v)| v)
        .sum();
    h.iter()
        .enumerate()
        .map(|(n, v)| if n as f64 == c { 0.5 } else { v * 0.5 / side })
        .collect()
}

/// A half-band's non-zero side taps, from the centre outward: `h[c−1], h[c−3], … h[0]`.
#[derive(Clone, Debug)]
struct HalfBand {
    side: Vec<f32>,
    /// `K`, where the filter has `4K + 3` taps.
    k: usize,
}

impl HalfBand {
    fn new(taps: usize) -> Self {
        let h = half_band(taps);
        let c = (taps - 1) / 2;
        HalfBand {
            side: (0..=c / 2).map(|j| h[c - (2 * j + 1)] as f32).collect(),
            k: c / 2,
        }
    }

    /// `Σ side[j] · (w[K+1+j] + w[K−j])` over a window of the last `2K+2` samples,
    /// oldest first: the even phase of both the up- and the down-sampler.
    #[inline]
    fn side_sum(&self, w: &[f32]) -> f32 {
        // `w[K+1+j]` walks up from the centre and `w[K−j]` down from it: two slices, no
        // index arithmetic, no bounds checks in the loop.
        let (down, up) = w.split_at(self.k + 1);
        self.side
            .iter()
            .zip(up)
            .zip(down.iter().rev())
            .fold(0.0f32, |acc, ((h, a), b)| acc + h * (a + b))
    }
}

/// The last `n` samples of a stream, readable as one slice, oldest first (each sample is
/// written twice, so the window never wraps).
#[derive(Clone, Debug)]
struct Window {
    buf: Vec<f32>,
    pos: usize,
    n: usize,
}

impl Window {
    fn new(n: usize) -> Self {
        Window {
            buf: vec![0.0; 2 * n],
            pos: 0,
            n,
        }
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.buf[self.pos + self.n] = x;
        self.pos = (self.pos + 1) % self.n;
    }

    #[inline]
    fn get(&self) -> &[f32] {
        &self.buf[self.pos..self.pos + self.n]
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
    }
}

/// A 2× up-sampler: one sample in, two out (gain 2 on the zero-stuffed stream, so the
/// level is kept).
#[derive(Clone, Debug)]
struct Up {
    w: Window,
}

impl Up {
    fn new(hb: &HalfBand) -> Self {
        Up {
            w: Window::new(2 * hb.k + 2),
        }
    }

    #[inline]
    fn tick(&mut self, hb: &HalfBand, x: f32) -> [f32; 2] {
        self.w.push(x);
        let w = self.w.get();
        [2.0 * hb.side_sum(w), w[hb.k + 1]]
    }
}

/// A 2× down-sampler: two samples in, one out.
#[derive(Clone, Debug)]
struct Down {
    even: Window,
    odd: Window,
}

impl Down {
    fn new(hb: &HalfBand) -> Self {
        Down {
            even: Window::new(2 * hb.k + 2),
            odd: Window::new(hb.k + 2),
        }
    }

    #[inline]
    fn tick(&mut self, hb: &HalfBand, pair: [f32; 2]) -> f32 {
        self.even.push(pair[0]);
        self.odd.push(pair[1]);
        0.5 * self.odd.get()[0] + hb.side_sum(self.even.get())
    }
}

/// Up- and down-sampling for two channels at 1×, 2× or 4×.
#[derive(Clone, Debug)]
pub struct Oversampler {
    factor: usize,
    a: HalfBand,
    b: HalfBand,
    up_a: [Up; 2],
    up_b: [Up; 2],
    down_b: [Down; 2],
    down_a: [Down; 2],
    pad: [[f32; PAD_4X]; 2],
}

impl Oversampler {
    pub fn new() -> Self {
        let a = HalfBand::new(TAPS_A);
        let b = HalfBand::new(TAPS_B);
        Oversampler {
            factor: 1,
            up_a: [Up::new(&a), Up::new(&a)],
            up_b: [Up::new(&b), Up::new(&b)],
            down_b: [Down::new(&b), Down::new(&b)],
            down_a: [Down::new(&a), Down::new(&a)],
            pad: [[0.0; PAD_4X]; 2],
            a,
            b,
        }
    }

    pub fn factor(&self) -> usize {
        self.factor
    }

    /// 1, 2 or 4. A change starts the filters from silence.
    pub fn set_factor(&mut self, factor: usize) {
        let f = if factor >= 4 {
            4
        } else if factor >= 2 {
            2
        } else {
            1
        };
        if f != self.factor {
            self.factor = f;
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            self.up_a[ch].w.clear();
            self.up_b[ch].w.clear();
            self.down_b[ch].even.clear();
            self.down_b[ch].odd.clear();
            self.down_a[ch].even.clear();
            self.down_a[ch].odd.clear();
        }
        self.pad = [[0.0; PAD_4X]; 2];
    }

    /// One base-rate sample of channel `ch` up to `factor` samples in `out`.
    #[inline]
    pub fn up(&mut self, ch: usize, x: f32, out: &mut [f32; 4]) {
        match self.factor {
            2 => {
                let [p, q] = self.up_a[ch].tick(&self.a, x);
                out[0] = p;
                out[1] = q;
            }
            4 => {
                let [p, q] = self.up_a[ch].tick(&self.a, x);
                let [r, s] = self.up_b[ch].tick(&self.b, p);
                let [t, u] = self.up_b[ch].tick(&self.b, q);
                *out = [r, s, t, u];
            }
            _ => out[0] = x,
        }
    }

    /// `factor` samples of channel `ch` back down to one.
    #[inline]
    pub fn down(&mut self, ch: usize, v: &[f32; 4]) -> f32 {
        match self.factor {
            2 => self.down_a[ch].tick(&self.a, [v[0], v[1]]),
            4 => {
                // Half a base sample of delay at 4×, so the latency is whole.
                let mut d = [0f32; 4];
                for (i, x) in v.iter().enumerate() {
                    d[i] = self.pad[ch][0];
                    self.pad[ch].copy_within(1.., 0);
                    self.pad[ch][PAD_4X - 1] = *x;
                }
                let p = self.down_b[ch].tick(&self.b, [d[0], d[1]]);
                let q = self.down_b[ch].tick(&self.b, [d[2], d[3]]);
                self.down_a[ch].tick(&self.a, [p, q])
            }
            _ => v[0],
        }
    }
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new()
    }
}
