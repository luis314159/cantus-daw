//! Measurements the virtual-analog tests share (and Calor's): an `f64` FFT, a spectrum
//! behind a 7-term Blackman-Harris window (side lobes at −180 dB, so an alias at −120 dB
//! is seen and not a window's leak), the aliasing floor, and a frequency read from zero
//! crossings.

/// In-place radix-2 FFT (`re.len()` a power of two).
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (w_re, w_im) = (
                    crate::math::cos(ang * k as f64),
                    crate::math::sin(ang * k as f64),
                );
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * w_re - im[b] * w_im;
                let t_im = re[b] * w_im + im[b] * w_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
            }
        }
        len <<= 1;
    }
}

/// Magnitudes of the first half of the spectrum of `x` (length a power of two), windowed.
pub fn spectrum(x: &[f32]) -> Vec<f64> {
    const A: [f64; 7] = [
        0.271_051_400_693_42,
        -0.433_297_939_234_48,
        0.218_122_999_543_11,
        -0.065_925_446_388_03,
        0.010_811_742_098_37,
        -0.000_776_584_825_22,
        0.000_013_887_217_35,
    ];
    let n = x.len();
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let p = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let w: f64 = A
                .iter()
                .enumerate()
                .map(|(k, a)| a * crate::math::cos(k as f64 * p))
                .sum();
            *v as f64 * w
        })
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    (0..n / 2)
        .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
        .collect()
}

/// The strongest component in `lo…hi` Hz that is not a harmonic of `f0`, in dB relative
/// to the fundamental (the "aliasing floor"). `x` is a power-of-two length at `fs`.
pub fn aliasing_floor_db(x: &[f32], fs: f32, f0: f32, lo: f32, hi: f32) -> f64 {
    let mag = spectrum(x);
    let n = x.len() as f64;
    let bin = |f: f64| f * n / fs as f64;
    let fund_bin = bin(f0 as f64).round() as usize;
    let fund = mag[fund_bin.saturating_sub(3)..fund_bin + 4]
        .iter()
        .cloned()
        .fold(0.0, f64::max);
    const GUARD: f64 = 12.0;
    let mut worst = 0.0f64;
    for (i, m) in mag.iter().enumerate() {
        let f = i as f64 * fs as f64 / n;
        if f < lo as f64 || f > hi as f64 {
            continue;
        }
        let h = (f / f0 as f64).round().max(1.0);
        if (i as f64 - bin(h * f0 as f64)).abs() <= GUARD {
            continue;
        }
        worst = worst.max(*m);
    }
    20.0 * crate::math::log10(worst.max(1e-30) / fund)
}

/// Frequency from the upward zero crossings of `x` (interpolated), after removing its
/// mean. `None` with fewer than three crossings.
pub fn frequency(x: &[f32], fs: f32) -> Option<f64> {
    let mean = x.iter().map(|v| *v as f64).sum::<f64>() / x.len() as f64;
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in 1..x.len() {
        let (a, b) = (x[i - 1] as f64 - mean, x[i] as f64 - mean);
        if a < 0.0 && b >= 0.0 {
            let t = (i - 1) as f64 + a / (a - b);
            if first.is_none() {
                first = Some(t);
            }
            last = t;
            count += 1;
        }
    }
    let first = first?;
    (count >= 3).then(|| (count - 1) as f64 * fs as f64 / (last - first))
}

/// Cents between two frequencies.
pub fn cents(f: f64, reference: f64) -> f64 {
    1200.0 * crate::math::log2(f / reference)
}
