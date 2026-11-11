//! Aula's FFT (ADR 0112): an iterative radix-2 complex transform of any power-of-two size,
//! in `f32`, whose twiddles are computed once with `cantus_dsp::math` — the same bits on
//! every machine. Every piece of it can run alone: the convolver spreads one transform over
//! many audio blocks, a stage or part of a stage at a time ([`Fft::stage`]), so a long
//! partition costs the same in every block instead of all at once.

use crate::math;

/// A transform of size `n` (a power of two, at least 2): its twiddles and its bit-reversal
/// table. Allocates in [`Fft::new`] only.
#[derive(Clone, Debug)]
pub struct Fft {
    n: usize,
    bits: u32,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<u32>,
}

impl Fft {
    pub fn new(n: usize) -> Fft {
        assert!(n >= 2 && n.is_power_of_two(), "an FFT of {n} points");
        let bits = n.trailing_zeros();
        let half = n / 2;
        let mut cos = Vec::with_capacity(half);
        let mut sin = Vec::with_capacity(half);
        for k in 0..half {
            let a = -std::f64::consts::TAU * k as f64 / n as f64;
            cos.push(math::cos(a) as f32);
            sin.push(math::sin(a) as f32);
        }
        let rev = (0..n as u32)
            .map(|i| i.reverse_bits() >> (u32::BITS - bits))
            .collect();
        Fft {
            n,
            bits,
            cos,
            sin,
            rev,
        }
    }

    pub fn size(&self) -> usize {
        self.n
    }

    /// The number of butterfly stages, `log2(n)`.
    pub fn stages(&self) -> u32 {
        self.bits
    }

    /// Where index `i` goes in bit-reversed order.
    #[inline]
    pub fn rev(&self, i: usize) -> usize {
        self.rev[i] as usize
    }

    /// Butterflies `from .. to` (of `n / 2`) of stage `s` (1 ⩽ s ⩽ `stages`), in place, on
    /// data already in bit-reversed order. `inverse` conjugates the twiddles; nothing is
    /// scaled. The butterflies of a stage are independent, so a stage may be done in parts
    /// in any order and gives the same bits.
    #[inline]
    pub fn stage(
        &self,
        re: &mut [f32],
        im: &mut [f32],
        s: u32,
        from: usize,
        to: usize,
        inverse: bool,
    ) {
        let half = 1usize << (s - 1);
        let stride = self.n >> s;
        let sign = if inverse { -1.0 } else { 1.0 };
        let mut b = from;
        while b < to {
            let k = b & (half - 1);
            let group = b >> (s - 1);
            // The run of butterflies in this group from k on.
            let end = (to - b).min(half - k);
            let base = group * 2 * half;
            for j in k..k + end {
                let wr = self.cos[j * stride];
                let wi = sign * self.sin[j * stride];
                let a = base + j;
                let c = a + half;
                let tr = re[c] * wr - im[c] * wi;
                let ti = re[c] * wi + im[c] * wr;
                re[c] = re[a] - tr;
                im[c] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
            b += end;
        }
    }

    /// The whole transform in place, natural order in and out (off the audio thread: the
    /// impulse response's partitions).
    pub fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        for i in 0..self.n {
            let j = self.rev(i);
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        for s in 1..=self.bits {
            self.stage(re, im, s, 0, self.n / 2, inverse);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A direct DFT in f64, the reference.
    fn dft(re: &[f32], im: &[f32]) -> (Vec<f64>, Vec<f64>) {
        let n = re.len();
        let mut or = vec![0.0; n];
        let mut oi = vec![0.0; n];
        for k in 0..n {
            for t in 0..n {
                let a = -std::f64::consts::TAU * (k * t % n) as f64 / n as f64;
                let (s, c) = (math::sin(a), math::cos(a));
                or[k] += re[t] as f64 * c - im[t] as f64 * s;
                oi[k] += re[t] as f64 * s + im[t] as f64 * c;
            }
        }
        (or, oi)
    }

    #[test]
    fn it_matches_a_direct_dft_and_inverts() {
        for n in [2, 4, 8, 64, 256] {
            let fft = Fft::new(n);
            let mut seed = 0x1234_5678_u64 + n as u64;
            let mut next = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5
            };
            let re: Vec<f32> = (0..n).map(|_| next()).collect();
            let im: Vec<f32> = (0..n).map(|_| next()).collect();
            let (wr, wi) = dft(&re, &im);
            let (mut r, mut i) = (re.clone(), im.clone());
            fft.run(&mut r, &mut i, false);
            for k in 0..n {
                assert!(
                    (r[k] as f64 - wr[k]).abs() < 1e-5 * n as f64,
                    "n {n} bin {k}"
                );
                assert!(
                    (i[k] as f64 - wi[k]).abs() < 1e-5 * n as f64,
                    "n {n} bin {k}"
                );
            }
            fft.run(&mut r, &mut i, true);
            for t in 0..n {
                assert!((r[t] / n as f32 - re[t]).abs() < 1e-5, "n {n} at {t}");
                assert!((i[t] / n as f32 - im[t]).abs() < 1e-5, "n {n} at {t}");
            }
        }
    }

    #[test]
    fn a_stage_done_in_pieces_is_the_same_bits() {
        let n = 1024;
        let fft = Fft::new(n);
        let re0: Vec<f32> = (0..n)
            .map(|i| ((i * 37) % 101) as f32 / 50.0 - 1.0)
            .collect();
        let im0: Vec<f32> = (0..n)
            .map(|i| ((i * 53) % 89) as f32 / 44.0 - 1.0)
            .collect();
        let (mut a_re, mut a_im) = (re0.clone(), im0.clone());
        let (mut b_re, mut b_im) = (re0, im0);
        for s in 1..=fft.stages() {
            fft.stage(&mut a_re, &mut a_im, s, 0, n / 2, false);
            let mut from = 0;
            for piece in [1, 7, 30, 100, 250, 512] {
                let to = (from + piece).min(n / 2);
                fft.stage(&mut b_re, &mut b_im, s, from, to, false);
                from = to;
            }
            fft.stage(&mut b_re, &mut b_im, s, from, n / 2, false);
        }
        assert!(a_re
            .iter()
            .zip(&b_re)
            .all(|(x, y)| x.to_bits() == y.to_bits()));
        assert!(a_im
            .iter()
            .zip(&b_im)
            .all(|(x, y)| x.to_bits() == y.to_bits()));
    }
}
