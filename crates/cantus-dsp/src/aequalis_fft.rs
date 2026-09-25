//! A radix-2 complex FFT for Aequalis's phase modes, with its twiddle factors computed once
//! in double precision (`spectrum::fft` builds them by recurrence, which is fine for a
//! picture at 4096 points and not for a 65 536-point filter design).
//!
//! One table serves every size up to the one it was made for: a transform of size `n`
//! reads every `max / n`-th entry. Nothing here allocates after [`Fft::new`].

/// The transform. `run` is in place, unscaled; the inverse is the conjugate transform, so
/// a forward and an inverse multiply the signal by `n`.
#[derive(Clone, Debug)]
pub struct Fft {
    max: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
}

impl Fft {
    /// A transform for every power-of-two size up to `max`.
    pub fn new(max: usize) -> Self {
        let max = max.next_power_of_two().max(2);
        let half = max / 2;
        let (cos, sin) = (0..half)
            .map(|k| {
                let a = -2.0 * std::f64::consts::PI * k as f64 / max as f64;
                (a.cos() as f32, a.sin() as f32)
            })
            .unzip();
        Fft { max, cos, sin }
    }

    pub fn max(&self) -> usize {
        self.max
    }

    /// Transform `re` + j·`im` in place; `n = re.len()` must be a power of two no larger
    /// than [`Fft::max`]. `inverse` conjugates the twiddles (no 1/n scaling).
    pub fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        let n = re.len();
        debug_assert!(n.is_power_of_two() && n <= self.max && im.len() == n);
        if n < 2 {
            return;
        }
        let bits = n.trailing_zeros();
        for i in 0..n {
            let j = i.reverse_bits() >> (usize::BITS - bits);
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let sign = if inverse { -1.0 } else { 1.0 };
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let stride = self.max / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let wr = self.cos[k * stride];
                    let wi = sign * self.sin[k * stride];
                    let a = start + k;
                    let b = a + half;
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_then_inverse_is_the_signal_times_n() {
        let fft = Fft::new(4096);
        for n in [2usize, 8, 512, 4096] {
            let src: Vec<f32> = (0..n)
                .map(|i| ((i * 37 % 101) as f32 / 50.0) - 1.0)
                .collect();
            let mut re = src.clone();
            let mut im = vec![0.0; n];
            fft.run(&mut re, &mut im, false);
            fft.run(&mut re, &mut im, true);
            for (a, b) in re.iter().zip(&src) {
                assert!((a / n as f32 - b).abs() < 1e-5, "size {n}");
            }
            assert!(im.iter().all(|x| (x / n as f32).abs() < 1e-5));
        }
    }

    #[test]
    fn a_cosine_lands_in_its_bin() {
        let n = 1024;
        let fft = Fft::new(n);
        let mut re: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 17.0 * i as f32 / n as f32).cos())
            .collect();
        let mut im = vec![0.0; n];
        fft.run(&mut re, &mut im, false);
        assert!((re[17] - n as f32 / 2.0).abs() < 1e-2);
        assert!((re[n - 17] - n as f32 / 2.0).abs() < 1e-2);
        assert!(re[16].abs() < 1e-2 && im[18].abs() < 1e-2);
    }
}
