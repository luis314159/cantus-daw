//! Adapted from `demucs/htdemucs.py` (`_spec`, `_ispec`), `demucs/spec.py` and
//! `demucs/apply.py` of <https://github.com/facebookresearch/demucs> @ e976d93e — MIT,
//! Copyright (c) Meta Platforms, Inc. and affiliates (the licence:
//! `crates/cantus-content/models/LICENSE`, and THIRD-PARTY-NOTICES.md).
//!
//! HT-Demucs's spectrogram and its inverse, as `demucs.htdemucs.HTDemucs._spec` and
//! `_ispec` compute them through `torch.stft` / `torch.istft` (n_fft 4096, hop 1024, a
//! periodic Hann window, `normalized=True`, `center=True`, reflection at the edges, the
//! Nyquist bin dropped and two frames trimmed at each end). The network was trained on
//! exactly these numbers, so they are reproduced, not approximated: `tests/stft_reference.rs`
//! holds both to PyTorch's own output (`tools/reference_stft.py`).
//!
//! A stereo pair goes through one complex FFT (left + j·right) and is split by symmetry;
//! the inverse puts two channels' spectra into one transform the same way. Nothing here
//! touches the audio thread.

use cantus_dsp::aequalis::fft::Fft;

pub const NFFT: usize = 4096;
pub const HOP: usize = NFFT / 4;
/// Bins the network sees: 2 049 less the Nyquist bin.
pub const BINS: usize = NFFT / 2;

pub struct Stft {
    fft: Fft,
    window: Vec<f32>,
}

impl Default for Stft {
    fn default() -> Self {
        Self::new()
    }
}

/// Frames `_spec` gives for `len` samples.
pub fn frames(len: usize) -> usize {
    len.div_ceil(HOP)
}

/// `x` extended by reflection (`numpy`'s and `F.pad`'s "reflect": the edge sample is not
/// repeated), `left` before and `right` after. `x` must be longer than either.
fn reflect(x: &[f32], left: usize, right: usize) -> Vec<f32> {
    let n = x.len();
    assert!(
        n > left && n > right,
        "reflect padding wider than the signal"
    );
    let mut y = Vec::with_capacity(n + left + right);
    y.extend((0..left).map(|p| x[left - p]));
    y.extend_from_slice(x);
    y.extend((0..right).map(|q| x[n - 2 - q]));
    y
}

impl Stft {
    pub fn new() -> Self {
        // torch.hann_window(4096), periodic: 0.5 − 0.5·cos(2πn/N), in double precision.
        let window = (0..NFFT)
            .map(|n| {
                let a = 2.0 * std::f64::consts::PI * n as f64 / NFFT as f64;
                (0.5 - 0.5 * cantus_dsp::math::cos(a)) as f32
            })
            .collect();
        Stft {
            fft: Fft::new(NFFT),
            window,
        }
    }

    /// The spectrogram of a stereo segment, laid out as the network's `mag` input:
    /// `[left re, left im, right re, right im][BINS][frames(len)]`.
    pub fn spec(&self, left: &[f32], right: &[f32]) -> Vec<f32> {
        let len = left.len();
        assert_eq!(right.len(), len);
        let le = frames(len);
        // `_spec`: pad by 3/8 of n_fft each side (and up to whole hops on the right)...
        let pad = HOP / 2 * 3;
        let (l1, r1) = (
            reflect(left, pad, pad + le * HOP - len),
            reflect(right, pad, pad + le * HOP - len),
        );
        // ...then `torch.stft(center=True)` pads n_fft / 2 more, by reflection too.
        let (l2, r2) = (
            reflect(&l1, NFFT / 2, NFFT / 2),
            reflect(&r1, NFFT / 2, NFFT / 2),
        );
        let total = 1 + (l2.len() - NFFT) / HOP;
        debug_assert_eq!(total, le + 4);
        let scale = 1.0 / (NFFT as f32).sqrt();
        let mut out = vec![0f32; 4 * BINS * le];
        let mut re = vec![0f32; NFFT];
        let mut im = vec![0f32; NFFT];
        // Frames 2 .. 2 + le are the ones `_spec` keeps.
        for t in 0..le {
            let start = (t + 2) * HOP;
            for n in 0..NFFT {
                re[n] = l2[start + n] * self.window[n];
                im[n] = r2[start + n] * self.window[n];
            }
            self.fft.run(&mut re, &mut im, false);
            // Z = L + jR: L[k] = (Z[k] + conj Z[N−k]) / 2, R[k] = (Z[k] − conj Z[N−k]) / 2j.
            for k in 0..BINS {
                let (zr, zi) = (re[k], im[k]);
                let m = (NFFT - k) % NFFT;
                let (wr, wi) = (re[m], -im[m]);
                let (lr, li) = (0.5 * (zr + wr), 0.5 * (zi + wi));
                let (rr, ri) = (0.5 * (zi - wi), -0.5 * (zr - wr));
                let at = k * le + t;
                out[at] = lr * scale;
                out[BINS * le + at] = li * scale;
                out[2 * BINS * le + at] = rr * scale;
                out[3 * BINS * le + at] = ri * scale;
            }
        }
        out
    }

    /// The inverse: one source's spectrogram in the network's layout (`[4][BINS][le]`,
    /// as [`Stft::spec`] gives) back to `length` samples per channel, as `_ispec(z,
    /// length)` does: a zero Nyquist bin and two empty frames at each end put back, the
    /// window-squared envelope of every frame divided out, the padding trimmed.
    pub fn ispec(
        &self,
        cac: &[f32],
        le: usize,
        length: usize,
        left: &mut [f32],
        right: &mut [f32],
    ) {
        assert_eq!(cac.len(), 4 * BINS * le);
        assert!(left.len() == length && right.len() == length);
        let pad = HOP / 2 * 3;
        let out_len = HOP * length.div_ceil(HOP) + 2 * pad;
        let total = le + 4;
        let span = NFFT + HOP * (total - 1);
        let mut yl = vec![0f32; span];
        let mut yr = vec![0f32; span];
        let mut env = vec![0f32; span];
        for t in 0..total {
            for n in 0..NFFT {
                env[t * HOP + n] += self.window[n] * self.window[n];
            }
        }
        // `normalized=True` multiplies by √N before the inverse transform, whose own scale
        // is 1/N.
        let scale = (NFFT as f32).sqrt() / NFFT as f32;
        let mut re = vec![0f32; NFFT];
        let mut im = vec![0f32; NFFT];
        for t in 0..le {
            // Z = L + jR from the two Hermitian spectra; the transform's real part is the
            // left channel and its imaginary part the right.
            re.iter_mut().for_each(|v| *v = 0.0);
            im.iter_mut().for_each(|v| *v = 0.0);
            for k in 0..BINS {
                let at = k * le + t;
                let (lr, li) = (cac[at], cac[BINS * le + at]);
                let (rr, ri) = (cac[2 * BINS * le + at], cac[3 * BINS * le + at]);
                // DC: the imaginary parts are dropped, as a real inverse drops them.
                let (li, ri) = if k == 0 { (0.0, 0.0) } else { (li, ri) };
                re[k] += lr - ri;
                im[k] += li + rr;
                if k > 0 {
                    // conj L[k] + j·conj R[k] at N − k.
                    re[NFFT - k] += lr + ri;
                    im[NFFT - k] += -li + rr;
                }
            }
            self.fft.run(&mut re, &mut im, true);
            let base = (t + 2) * HOP;
            for n in 0..NFFT {
                let w = self.window[n] * scale;
                yl[base + n] += re[n] * w;
                yr[base + n] += im[n] * w;
            }
        }
        // `center=True` drops n_fft / 2 at the start; `_ispec` then drops its own padding.
        let from = NFFT / 2 + pad;
        debug_assert!(NFFT / 2 + out_len <= span);
        for i in 0..length {
            let j = from + i;
            let e = env[j];
            let (a, b) = if e > 1e-11 {
                (yl[j] / e, yr[j] / e)
            } else {
                (yl[j], yr[j])
            };
            left[i] = a;
            right[i] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect_matches_numpy() {
        // numpy.pad([1, 2, 3, 4], (2, 3), mode="reflect") = [3, 2, 1, 2, 3, 4, 3, 2, 1]
        let y = reflect(&[1.0, 2.0, 3.0, 4.0], 2, 3);
        assert_eq!(y, [3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0]);
    }

    /// The inverse of the forward is the input away from the segment's ends — for a signal
    /// with nothing near the Nyquist bin, which `_spec` drops. At the ends it is not, in
    /// PyTorch either: `_spec` drops two frames there that `_ispec` puts back empty (the
    /// reference test holds those samples to PyTorch's).
    #[test]
    fn inverse_of_forward_is_the_input_away_from_the_ends() {
        for &len in &[343_980usize, 44_100, 10_000] {
            let wave = |i: usize, p: f32| {
                let t = i as f32;
                0.5 * (t * 0.01 + p).sin() + 0.3 * (t * 0.31 + p).sin()
            };
            let l: Vec<f32> = (0..len).map(|i| wave(i, 0.0)).collect();
            let r: Vec<f32> = (0..len).map(|i| wave(i, 1.0)).collect();
            let st = Stft::new();
            let z = st.spec(&l, &r);
            let le = frames(len);
            assert_eq!(z.len(), 4 * BINS * le);
            let (mut l2, mut r2) = (vec![0f32; len], vec![0f32; len]);
            st.ispec(&z, le, len, &mut l2, &mut r2);
            let edge = HOP / 2 * 3;
            let err = (edge..len - edge)
                .map(|i| (l[i] - l2[i]).abs().max((r[i] - r2[i]).abs()))
                .fold(0f32, f32::max);
            assert!(err < 2e-5, "{len}: {err}");
        }
    }

    /// One bin of a known sine, against the closed form: the forward is torch's, scale and
    /// sign included (a cosine at bin k gives N/4 · w-normalised energy at +k, real).
    #[test]
    fn a_cosine_lands_in_its_bin_with_torchs_scale() {
        let len = 343_980;
        let k = 96usize;
        let x: Vec<f32> = (0..len)
            .map(|n| {
                let a = 2.0 * std::f64::consts::PI * (k * n) as f64 / NFFT as f64;
                cantus_dsp::math::cos(a) as f32
            })
            .collect();
        let st = Stft::new();
        let z = st.spec(&x, &x);
        let le = frames(len);
        // A frame starting at signal sample n0 holds, at bin m, ½·e^{j2πk·n0/N}·W[m − k]
        // (+ the mirror term, nothing near bin k), W the window's transform: W[0] = N/2,
        // W[±1] = −N/4. Frames start at t·hop − 1 536 (`_spec`'s padding); k = 96 makes
        // k·n0/N whole, so the phase is 1: bin k holds N/4 and bin k + 1 holds −N/8, each
        // times 1/√N (`normalized=True`).
        let want = (NFFT as f32 / 4.0) / (NFFT as f32).sqrt();
        let t = le / 2;
        let re = z[k * le + t];
        let im = z[BINS * le + k * le + t];
        assert!((re - want).abs() < 1e-3 * want, "{re} vs {want}");
        assert!(im.abs() < 1e-3 * want, "{im}");
        // The right channel (the same signal) agrees, and a neighbour bin holds half.
        assert!((z[2 * BINS * le + k * le + t] - re).abs() < 1e-4);
        let next = z[(k + 1) * le + t];
        assert!((next + want / 2.0).abs() < 1e-3 * want, "{next}");
    }
}
