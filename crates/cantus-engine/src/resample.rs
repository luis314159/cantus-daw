//! Sample-rate conversion of whole files, on load (ADR 0025 §9): an audio clip's source is
//! converted once to the engine's rate when the cache first reads it, so the audio thread
//! plays it sample for sample with no interpolation at all.
//!
//! A windowed-sinc interpolator evaluated at the exact rational positions of the
//! conversion: for rates `from` and `to` with `g = gcd(from, to)`, output frame `n` sits at
//! source position `n · M / L` with `L = to / g`, `M = from / g`, so there are only `L`
//! distinct fractional positions and their kernels are computed once, in `f64`, and kept.
//! The kernel is a Kaiser-windowed sinc (β = 10, about 100 dB of rejection) with 32 zero
//! crossings each side at the source's Nyquist, its cutoff lowered to the output's Nyquist
//! when converting down (so nothing folds), at 95 % of the narrower band. Each phase's
//! taps are normalised to sum to one, so DC passes exactly. An odd rate pair whose `L` is
//! too large for a table computes its kernel per frame — slow, and right.
//!
//! The work is split across the machine's cores; it runs on the UI thread's side, never
//! on the audio thread. Same rate in, same samples out: the input is copied, untouched.

/// Zero crossings of the sinc on each side of the centre, at a cutoff of one.
const ZEROS: f64 = 32.0;
/// The Kaiser window's β.
const BETA: f64 = 10.0;
/// The cutoff's margin under the narrower Nyquist.
const ROLL: f64 = 0.95;
/// The most phases a table holds before the kernel is computed per frame instead.
const MAX_TABLE_PHASES: u64 = 4096;

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..200 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// The kernel's geometry for one conversion.
struct Design {
    /// Taps either side: frame `i + k` for `k` in `1 - half ..= half`.
    half: i64,
    /// Cutoff as a fraction of the source's Nyquist.
    fc: f64,
    i0_beta: f64,
}

impl Design {
    fn new(from: u32, to: u32) -> Design {
        let fc = (to as f64 / from as f64).min(1.0) * ROLL;
        Design {
            half: (ZEROS / fc).ceil() as i64,
            fc,
            i0_beta: bessel_i0(BETA),
        }
    }

    fn taps(&self) -> usize {
        (2 * self.half) as usize
    }

    /// The normalised taps for a position `frac` in [0, 1) past source frame `i`; tap `j`
    /// weighs frame `i + 1 - half + j`.
    fn kernel(&self, frac: f64, out: &mut [f64]) {
        let mut sum = 0.0;
        let span = self.half as f64;
        for (j, o) in out.iter_mut().enumerate() {
            let t = (j as i64 + 1 - self.half) as f64 - frac;
            let x = t / span;
            let w = if x.abs() >= 1.0 {
                0.0
            } else {
                bessel_i0(BETA * (1.0 - x * x).sqrt()) / self.i0_beta
            };
            let a = std::f64::consts::PI * self.fc * t;
            let s = if a.abs() < 1e-12 { 1.0 } else { a.sin() / a };
            *o = s * w;
            sum += *o;
        }
        if sum != 0.0 {
            out.iter_mut().for_each(|o| *o /= sum);
        }
    }
}

/// How many frames a file of `frames` at `from` has at `to`.
pub fn converted_len(frames: usize, from: u32, to: u32) -> usize {
    if from == to || from == 0 || to == 0 {
        return frames;
    }
    ((frames as u128 * to as u128).div_ceil(from as u128)) as usize
}

/// Convert interleaved audio of `channels` from `from` Hz to `to` Hz.
pub fn resample(data: &[f32], channels: usize, from: u32, to: u32) -> Vec<f32> {
    let channels = channels.max(1);
    if from == to || from == 0 || to == 0 || data.is_empty() {
        return data.to_vec();
    }
    let n_in = data.len() / channels;
    let n_out = converted_len(n_in, from, to);
    let g = gcd(from as u64, to as u64);
    let (l, m) = (to as u64 / g, from as u64 / g);
    let design = Design::new(from, to);
    let taps = design.taps();
    // Every phase's kernel, when there are few enough of them.
    let table: Option<Vec<f64>> = (l <= MAX_TABLE_PHASES).then(|| {
        let mut t = vec![0.0; l as usize * taps];
        for (p, row) in t.chunks_exact_mut(taps).enumerate() {
            design.kernel(p as f64 / l as f64, row);
        }
        t
    });
    let mut out = vec![0f32; n_out * channels];
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let per = n_out.div_ceil(workers).max(4096);
    std::thread::scope(|scope| {
        for (w, chunk) in out.chunks_mut(per * channels).enumerate() {
            let (design, table) = (&design, table.as_deref());
            scope.spawn(move || {
                let mut scratch = vec![0.0f64; taps];
                let mut acc = vec![0.0f64; channels];
                let first = w * per;
                for (k, frame) in chunk.chunks_exact_mut(channels).enumerate() {
                    let n = (first + k) as u64;
                    let num = n as u128 * m as u128;
                    let i = (num / l as u128) as i64;
                    let p = (num % l as u128) as u64;
                    let row: &[f64] = match table {
                        Some(t) => &t[p as usize * taps..(p as usize + 1) * taps],
                        None => {
                            design.kernel(p as f64 / l as f64, &mut scratch);
                            &scratch
                        }
                    };
                    acc.iter_mut().for_each(|a| *a = 0.0);
                    let base = i + 1 - design.half;
                    for (j, c) in row.iter().enumerate() {
                        let f = base + j as i64;
                        if f < 0 || f >= n_in as i64 {
                            continue;
                        }
                        let src = &data[f as usize * channels..(f as usize + 1) * channels];
                        for (a, s) in acc.iter_mut().zip(src) {
                            *a += *s as f64 * c;
                        }
                    }
                    for (o, a) in frame.iter_mut().zip(&acc) {
                        *o = *a as f32;
                    }
                }
            });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, rate: u32, frames: usize, channels: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(frames * channels);
        for i in 0..frames {
            let x = (std::f64::consts::TAU * hz * i as f64 / rate as f64).sin() as f32;
            for c in 0..channels {
                v.push(if c == 0 { x } else { -x });
            }
        }
        v
    }

    #[test]
    fn the_same_rate_is_the_same_samples() {
        let s = sine(440.0, 48_000, 1000, 2);
        assert_eq!(resample(&s, 2, 48_000, 48_000), s);
    }

    #[test]
    fn a_sine_converts_cleanly_both_ways() {
        for (from, to, hz) in [
            (44_100u32, 48_000u32, 1_000.0),
            (48_000, 44_100, 1_000.0),
            (96_000, 48_000, 5_000.0),
            (22_050, 48_000, 3_000.0),
            (44_100, 48_000, 15_000.0),
        ] {
            let frames = from as usize / 5;
            let input = sine(hz, from, frames, 2);
            let out = resample(&input, 2, from, to);
            assert_eq!(out.len() / 2, converted_len(frames, from, to));
            let want = sine(hz, to, out.len() / 2, 2);
            // Away from the edges, where the kernel sees only the file.
            let edge = 200;
            let mut err = 0f64;
            for i in edge..out.len() / 2 - edge {
                for c in 0..2 {
                    err = err.max((out[i * 2 + c] - want[i * 2 + c]).abs() as f64);
                }
            }
            let db = 20.0 * err.log10();
            assert!(db < -80.0, "{from} → {to} at {hz} Hz: error {db:.1} dB");
        }
    }

    #[test]
    fn content_above_the_new_nyquist_is_removed() {
        // 30 kHz at 96 kHz would fold to 18 kHz at 48 kHz.
        let input = sine(30_000.0, 96_000, 19_200, 1);
        let out = resample(&input, 1, 96_000, 48_000);
        let peak = out[200..out.len() - 200]
            .iter()
            .fold(0f32, |m, x| m.max(x.abs()));
        assert!(20.0 * (peak as f64).log10() < -80.0, "folded energy {peak}");
    }

    #[test]
    fn dc_passes_exactly_enough() {
        let input = vec![0.5f32; 10_000];
        let out = resample(&input, 1, 44_100, 48_000);
        for x in &out[100..out.len() - 100] {
            assert!((x - 0.5).abs() < 1e-6, "{x}");
        }
    }

    #[test]
    fn an_odd_pair_without_a_table_still_converts() {
        // 48 001 / 44 100: L = 48 001, far beyond the table.
        let input = sine(500.0, 44_100, 4_410, 1);
        let out = resample(&input, 1, 44_100, 48_001);
        let want = sine(500.0, 48_001, out.len(), 1);
        let err = (200..out.len() - 200)
            .map(|i| (out[i] - want[i]).abs())
            .fold(0f32, f32::max);
        assert!(err < 1e-4, "{err}");
    }
}
