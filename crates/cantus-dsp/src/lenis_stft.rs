//! Lenis's short-time Fourier transform: a **low-delay asymmetric window pair** (after
//! Mauler & Martin, "A low delay, variable resolution, perfect reconstruction spectral
//! analysis-synthesis system for speech enhancement", EUSIPCO 2007), so the analysis sees
//! a long frame (frequency resolution) while only its last `2M` samples are synthesised
//! (delay).
//!
//! ```text
//!  analysis  wa: ╱‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾╲     √Hann rising over N−M, √Hann falling over M
//!  synthesis ws:                          ╱‾‾‾‾‾‾‾‾╲      Hann(2M) / wa over the last 2M, 0 before
//!  wa·ws       :                          ╱‾‾‾‾‾‾‾‾╲      = Hann(2M): at a hop of M/2 they sum to 2
//!               0                      N−2M   N−M  N
//! ```
//!
//! `M = N/2` is the ordinary symmetric √Hann pair at 75 % overlap (a latency of `N − 1`);
//! Lenis uses `M = N/4` (a latency of `N/2 − 1`: 10.6 ms at 48 kHz for `N = 1024`), measured
//! against the symmetric pair in `lenis_tests_measure.rs`. The hop is `M/2` either way.
//!
//! **Latency.** Frame `k` covers the input `[kH + H − N, kH + H)` and is computed when its
//! last sample arrives; it writes the output samples `[kH + H − 2M, kH + H)`. A sample's last
//! frame is therefore done at most `2M − 1` samples after it arrived, and it is emitted then:
//! the latency is exactly `2M − 1`, whatever the block size.
//!
//! **Two channels in one transform.** The left channel is the real part and the right the
//! imaginary part of one complex FFT; the conjugate symmetry of a real signal's spectrum
//! takes them apart and puts them back together ([`unpack`], [`repack`]).

use crate::math;

/// The transform's shape: frame `n`, synthesis half-length `m`, hop `m/2`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub n: usize,
    pub m: usize,
}

impl Geometry {
    /// `n` a power of two ≥ 16, `m` a power of two with `4 ≤ m ≤ n/2`.
    pub fn new(n: usize, m: usize) -> Geometry {
        assert!(n.is_power_of_two() && n >= 16, "frame {n}");
        assert!(m.is_power_of_two() && m >= 4 && m * 2 <= n, "half {m}");
        Geometry { n, m }
    }

    #[inline]
    pub fn hop(&self) -> usize {
        self.m / 2
    }

    /// Bins `0..=n/2`.
    #[inline]
    pub fn bins(&self) -> usize {
        self.n / 2 + 1
    }

    /// Samples from an input to the output it makes.
    #[inline]
    pub fn latency(&self) -> usize {
        2 * self.m - 1
    }

    /// The overlap-add region of a frame: its last `2m` samples.
    #[inline]
    pub fn span(&self) -> usize {
        2 * self.m
    }
}

/// The periodic Hann window of length `len` at `i`, in double precision.
fn hann(i: usize, len: usize) -> f64 {
    0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / len as f64)
}

/// The window pair, written into slices at least `g.n` long:
/// - `wa`: the analysis window;
/// - `ws`: the synthesis window for a frame that went through the FFT and back (the IFFT's
///   `1/n` and the overlap's `1/2` folded in — both powers of two, so exact);
/// - `wp`: `wa·ws` without the `1/n`: what a frame nobody changed adds, straight from the
///   input, with no transform at all.
///
/// Returns the analysis window's sum (a sine of amplitude `A` reads `A·Σwa/2` in its bin).
pub fn windows(g: Geometry, wa: &mut [f32], ws: &mut [f32], wp: &mut [f32]) -> f64 {
    let (n, m) = (g.n, g.m);
    let mut sum = 0.0f64;
    for i in 0..n {
        let a = if i < n - m {
            hann(i, 2 * (n - m)).sqrt()
        } else {
            hann(i - (n - 2 * m), 2 * m).sqrt()
        };
        let p = if i < n - 2 * m {
            0.0
        } else {
            hann(i - (n - 2 * m), 2 * m)
        };
        // The synthesis window where the analysis window is zero (only i = 0, symmetric
        // case) is zero: the product there is zero too.
        let s = if a > 0.0 { p / a } else { 0.0 };
        wa[i] = a as f32;
        ws[i] = (s * 0.5 / n as f64) as f32;
        wp[i] = (p * 0.5) as f32;
        sum += a;
    }
    sum
}

/// Bin `k`'s two spectra from the packed transform `z = FFT(l + j·r)`:
/// `(l_re, l_im, r_re, r_im)`. `k` in `0..=n/2`.
#[inline]
pub fn unpack(re: &[f32], im: &[f32], k: usize, n: usize) -> (f32, f32, f32, f32) {
    let j = (n - k) & (n - 1);
    let (a, b) = (re[k], im[k]);
    let (c, d) = (re[j], im[j]);
    (0.5 * (a + c), 0.5 * (b - d), 0.5 * (b + d), 0.5 * (c - a))
}

/// Write bin `k` (and its mirror) of the packed spectrum back from the two channels'
/// spectra, each scaled by its own gain: `z[k] = gl·L + j·gr·R`,
/// `z[n−k] = gl·conj(L) + j·gr·conj(R)`.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn repack(
    re: &mut [f32],
    im: &mut [f32],
    k: usize,
    n: usize,
    l: (f32, f32),
    r: (f32, f32),
    gl: f32,
    gr: f32,
) {
    let j = (n - k) & (n - 1);
    let (a, b) = (gl * l.0, gl * l.1);
    let (c, d) = (gr * r.0, gr * r.1);
    re[k] = a - d;
    im[k] = b + c;
    if j != k {
        re[j] = a + d;
        im[j] = c - b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aequalis::fft::Fft;

    /// Overlap-adding `wa·ws` at the hop gives exactly one (to rounding) everywhere a
    /// whole set of frames covers, for the shipped and the symmetric shapes.
    #[test]
    fn the_window_pair_reconstructs() {
        for (n, m) in [
            (1024, 256),
            (1024, 512),
            (512, 128),
            (2048, 512),
            (4096, 1024),
        ] {
            let g = Geometry::new(n, m);
            let (mut wa, mut ws, mut wp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
            windows(g, &mut wa, &mut ws, &mut wp);
            let h = g.hop();
            let mut acc = vec![0.0f64; 8 * n];
            for k in 0..(8 * n - n) / h {
                for i in 0..n {
                    acc[k * h + i] += wp[i] as f64;
                    let viaf = wa[i] as f64 * ws[i] as f64 * n as f64;
                    assert!((viaf - wp[i] as f64).abs() < 1e-6, "{n}/{m} at {i}");
                }
            }
            for v in &acc[2 * n..6 * n] {
                assert!((v - 1.0).abs() < 1e-6, "{n}/{m}: {v}");
            }
        }
    }

    #[test]
    fn two_channels_share_one_transform() {
        let n = 256;
        let fft = Fft::new(n);
        let l: Vec<f32> = (0..n)
            .map(|i| ((i * 37 % 101) as f32 / 50.0) - 1.0)
            .collect();
        let r: Vec<f32> = (0..n)
            .map(|i| ((i * 53 % 89) as f32 / 44.0) - 1.0)
            .collect();
        let (mut re, mut im) = (l.clone(), r.clone());
        fft.run(&mut re, &mut im, false);
        let (mut lr, mut li) = (l.clone(), vec![0.0; n]);
        fft.run(&mut lr, &mut li, false);
        let (mut rr, mut ri) = (r.clone(), vec![0.0; n]);
        fft.run(&mut rr, &mut ri, false);
        for k in 0..=n / 2 {
            let (a, b, c, d) = unpack(&re, &im, k, n);
            assert!(
                (a - lr[k]).abs() < 1e-3 && (b - li[k]).abs() < 1e-3,
                "left {k}"
            );
            assert!(
                (c - rr[k]).abs() < 1e-3 && (d - ri[k]).abs() < 1e-3,
                "right {k}"
            );
        }
        // Gains of 0.5 on the left and 2 on the right come back as 0.5·l and 2·r.
        let mut z = (vec![0.0f32; n], vec![0.0f32; n]);
        for k in 0..=n / 2 {
            let (a, b, c, d) = unpack(&re, &im, k, n);
            repack(&mut z.0, &mut z.1, k, n, (a, b), (c, d), 0.5, 2.0);
        }
        fft.run(&mut z.0, &mut z.1, true);
        for i in 0..n {
            assert!((z.0[i] / n as f32 - 0.5 * l[i]).abs() < 1e-4, "l {i}");
            assert!((z.1[i] / n as f32 - 2.0 * r[i]).abs() < 1e-4, "r {i}");
        }
    }
}
