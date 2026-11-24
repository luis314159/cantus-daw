//! What is measured of a note (a reference's or Arcus's), the same way for both.

use cantus_dsp::math;

/// The steady window, s, and the FFT that holds it at 48 kHz zero-padded.
pub const WIN_S: f64 = 0.6;
const NFFT: usize = 1 << 16;
pub const HARMS: usize = 40;
/// Third-octave bands from 50 Hz to 12.5 kHz.
pub const BANDS: usize = 25;

pub fn band_hz(i: usize) -> f64 {
    50.0 * math::exp2(i as f64 / 3.0)
}

#[derive(Clone, Debug, Default)]
pub struct Features {
    pub f0: f64,
    /// Harmonics 1…40, dB against their energy sum (`None` above the rate's reach).
    pub harm: Vec<Option<f64>>,
    /// Harmonics against the fundamental, dB (printed by `probe`).
    #[allow(dead_code)]
    pub harm_f0: Vec<Option<f64>>,
    /// Third-octave levels against their sum, dB.
    pub ltas: Vec<f64>,
    pub centroid_hz: f64,
    /// The noise between harmonics 1…10 against the harmonics, dB.
    pub noise_db: f64,
    /// 10 % → 90 % of the steady level, ms.
    pub attack_ms: f64,
    /// Vibrato rate (Hz) and depth (± cents), if any.
    pub vib: Option<(f64, f64)>,
    /// Pizzicato: each partial's T60 (s) for partials 1…8, and the thump (dB under the
    /// note's first 60 ms: what lies under 0.7·f0).
    pub t60: Vec<Option<f64>>,
    pub thump_db: f64,
    /// The steady window's RMS (dBFS): a silent note shows here.
    pub rms_db: f64,
}

fn fft(re: &mut [f64], im: &mut [f64]) {
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
        let (s, c) = math::sin_cos(-std::f64::consts::TAU / len as f64);
        for start in (0..n).step_by(len) {
            let (mut wr, mut wi) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
                let t = wr * c - wi * s;
                wi = wr * s + wi * c;
                wr = t;
            }
        }
        len <<= 1;
    }
}

/// The magnitude spectrum of `x` (Hann, zero-padded to `n`).
fn spectrum(x: &[f32], n: usize) -> Vec<f64> {
    let m = x.len().min(n);
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    for (i, v) in x.iter().take(m).enumerate() {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / m as f64);
        re[i] = w * *v as f64;
    }
    fft(&mut re, &mut im);
    (0..n / 2)
        .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
        .collect()
}

fn db(x: f64) -> f64 {
    20.0 * math::log10(x.max(1e-15))
}

/// The peak magnitude within ±2 % of `hz` (at least ±3 bins), and its bin.
fn peak(sp: &[f64], hz: f64, sr: f64, n: usize) -> (f64, usize) {
    let bin = hz * n as f64 / sr;
    let w = (0.02 * bin).max(3.0);
    let (a, b) = (
        ((bin - w).max(1.0)) as usize,
        ((bin + w) as usize).min(sp.len() - 1),
    );
    let mut best = (0.0, a);
    for (i, v) in sp.iter().enumerate().take(b + 1).skip(a) {
        if *v > best.0 {
            best = (*v, i);
        }
    }
    best
}

/// The fundamental near `near` (within ±2 %), refined at the first four harmonics' peaks;
/// with `octaves`, an octave either way is taken instead when it holds clearly more: the
/// octave up when its harmonics carry 1.5× `near`'s first eight, the octave down when its
/// odd harmonics (which `near` lacks) carry at least a third of its even ones.
pub fn find_f0(x: &[f32], sr: f64, near: f64, octaves: bool) -> f64 {
    let sp = spectrum(x, NFFT);
    let at = |f: f64, k: usize| {
        if k as f64 * f < 0.45 * sr {
            peak(&sp, k as f64 * f, sr, NFFT).0
        } else {
            0.0
        }
    };
    let score = |f: f64| -> f64 { (1..=8).map(|k| at(f, k)).sum() };
    let mut f = near;
    if octaves {
        let here = score(near);
        let down_odd: f64 = [1, 3, 5, 7].iter().map(|k| at(near / 2.0, *k)).sum();
        let down_even: f64 = [2, 4, 6, 8].iter().map(|k| at(near / 2.0, *k)).sum();
        if score(2.0 * near) > 1.5 * here {
            f = 2.0 * near;
        } else if down_odd > down_even / 3.0 {
            f = near / 2.0;
        }
    }
    let mut acc = (0.0, 0.0);
    for k in 1..=4 {
        let (m, i) = peak(&sp, k as f64 * f, sr, NFFT);
        if i == 0 || i + 1 >= sp.len() || m <= 0.0 {
            continue;
        }
        let (l, r) = (sp[i - 1], sp[i + 1]);
        if m < l || m < r {
            continue;
        }
        let den = l - 2.0 * m + r;
        let d = if den < 0.0 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let hz = (i as f64 + d) * sr / NFFT as f64 / k as f64;
        acc = (acc.0 + m * hz, acc.1 + m);
    }
    if acc.1 > 0.0 {
        acc.0 / acc.1
    } else {
        f
    }
}

/// The note's features: `x` the note from its onset, `steady` where the steady window starts
/// (s), `pizz` a plucked note.
pub fn measure(x: &[f32], sr: f64, near: f64, steady: f64, pizz: bool) -> Features {
    let a = ((steady * sr) as usize).min(x.len().saturating_sub(1));
    let b = (a + (WIN_S * sr) as usize).min(x.len());
    let win = &x[a..b];
    let f0 = find_f0(win, sr, near, false);
    let sp = spectrum(win, NFFT);
    let mut amps = Vec::with_capacity(HARMS);
    for k in 1..=HARMS {
        let hz = k as f64 * f0;
        amps.push(if hz < 0.45 * sr && hz < 20_000.0 {
            Some(peak(&sp, hz, sr, NFFT).0)
        } else {
            None
        });
    }
    let total: f64 = amps.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
    let first = amps[0].unwrap_or(1e-15);
    let harm = amps.iter().map(|v| v.map(|v| db(v / total))).collect();
    let harm_f0 = amps.iter().map(|v| v.map(|v| db(v / first))).collect();
    let (mut num, mut den) = (0.0, 0.0);
    for (k, v) in amps.iter().enumerate() {
        if let Some(v) = v {
            num += (k + 1) as f64 * f0 * v;
            den += v;
        }
    }
    // Third-octave bands over the power spectrum.
    let bin_hz = sr / NFFT as f64;
    let mut ltas = [0.0; BANDS];
    for (i, v) in sp.iter().enumerate() {
        let hz = i as f64 * bin_hz;
        if hz < 44.0 {
            continue;
        }
        let bi = (3.0 * math::log2(hz / 50.0) + 0.5).floor();
        if bi >= 0.0 && (bi as usize) < BANDS {
            ltas[bi as usize] += v * v;
        }
    }
    let sum: f64 = ltas.iter().sum();
    let ltas = ltas
        .iter()
        .map(|v| 10.0 * math::log10((v / sum).max(1e-15)))
        .collect();
    // The noise between harmonics 1…10: the median of the bins at 0.3…0.7 between them.
    let mut noise = Vec::new();
    let mut hsum = 0.0;
    let mut hn = 0.0;
    for k in 1..=10 {
        let (lo, hi) = ((k as f64 + 0.3) * f0, (k as f64 + 0.7) * f0);
        if hi > 0.45 * sr {
            break;
        }
        let (i0, i1) = ((lo / bin_hz) as usize, (hi / bin_hz) as usize);
        noise.extend_from_slice(&sp[i0..i1]);
        if let Some(v) = amps[k - 1] {
            hsum += v * v;
            hn += 1.0;
        }
    }
    noise.sort_by(|p, q| p.total_cmp(q));
    let noise_db = if noise.is_empty() {
        -120.0
    } else {
        db(noise[noise.len() / 2] / (hsum / hn).sqrt())
    };
    let rms =
        (win.iter().map(|v| *v as f64 * *v as f64).sum::<f64>() / win.len().max(1) as f64).sqrt();
    Features {
        rms_db: db(rms),
        f0,
        harm,
        harm_f0,
        ltas,
        centroid_hz: if den > 0.0 { num / den } else { 0.0 },
        noise_db,
        attack_ms: attack_ms(x, sr, steady),
        vib: if pizz { None } else { vibrato(x, sr, f0, a, b) },
        t60: if pizz { decays(x, sr, f0) } else { Vec::new() },
        thump_db: if pizz { thump(x, sr, f0) } else { 0.0 },
    }
}

/// 10 % → 90 % of the level the steady window holds, on a 5 ms RMS from the onset.
fn attack_ms(x: &[f32], sr: f64, steady: f64) -> f64 {
    let hop = (0.005 * sr) as usize;
    let env: Vec<f64> = x
        .chunks(hop)
        .map(|c| (c.iter().map(|v| *v as f64 * *v as f64).sum::<f64>() / c.len() as f64).sqrt())
        .collect();
    let s0 = (steady / 0.005) as usize;
    let s1 = (s0 + (WIN_S / 0.005) as usize).min(env.len());
    if s0 >= s1 {
        return 0.0;
    }
    let level = env[s0..s1].iter().sum::<f64>() / (s1 - s0) as f64;
    let t10 = env.iter().position(|e| *e >= 0.1 * level).unwrap_or(0);
    let t90 = env.iter().position(|e| *e >= 0.9 * level).unwrap_or(0);
    (t90.saturating_sub(t10)) as f64 * 5.0
}

/// The fundamental's frequency every 10 ms (a heterodyne's phase over 50 ms): its rate (the
/// zero crossings of its swing) and depth (√2 × its RMS swing in cents), if it swings more
/// than 3 cents.
fn vibrato(x: &[f32], sr: f64, f0: f64, a: usize, b: usize) -> Option<(f64, f64)> {
    let w = std::f64::consts::TAU * f0 / sr;
    let (mut re, mut im) = (Vec::with_capacity(b - a), Vec::with_capacity(b - a));
    for (n, v) in x[a..b].iter().enumerate() {
        let (s, c) = math::sin_cos(w * n as f64);
        re.push(*v as f64 * c);
        im.push(-(*v as f64) * s);
    }
    // A boxcar of whole periods (at least 60 ms): every other harmonic falls in its nulls.
    let periods = (0.06 * f0).ceil().max(1.0);
    let len = (periods * sr / f0).round() as usize;
    let hop = (0.01 * sr) as usize;
    let mut ph = Vec::new();
    let mut i = 0;
    while i + len <= re.len() {
        let (sr_, si) = (
            re[i..i + len].iter().sum::<f64>(),
            im[i..i + len].iter().sum::<f64>(),
        );
        ph.push(math::atan2(si, sr_));
        i += hop;
    }
    if ph.len() < 10 {
        return None;
    }
    let mut cents = Vec::new();
    for p in ph.windows(2) {
        let mut d = p[1] - p[0];
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        let hz = f0 + d / (std::f64::consts::TAU * 0.01);
        cents.push(1200.0 * math::log2((hz / f0).max(1e-6)));
    }
    let mean = cents.iter().sum::<f64>() / cents.len() as f64;
    let dev: Vec<f64> = cents.iter().map(|c| c - mean).collect();
    let rms = (dev.iter().map(|d| d * d).sum::<f64>() / dev.len() as f64).sqrt();
    if rms * std::f64::consts::SQRT_2 < 3.0 {
        return None;
    }
    let crossings = dev.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    let rate = crossings as f64 / (dev.len() as f64 * 0.01);
    Some((rate, rms * std::f64::consts::SQRT_2))
}

/// Partials 1…8 of a plucked note: each one's level every 10 ms (a heterodyne over 40 ms)
/// from the loudest point, its T60 from the slope over its first 30 dB of fall (or 1.5 s).
fn decays(x: &[f32], sr: f64, f0: f64) -> Vec<Option<f64>> {
    let start = x
        .iter()
        .enumerate()
        .fold(
            (0usize, 0f32),
            |m, (i, v)| if v.abs() > m.1 { (i, v.abs()) } else { m },
        )
        .0;
    let len = (0.04 * sr) as usize;
    let hop = (0.01 * sr) as usize;
    (1..=8)
        .map(|k| {
            let hz = k as f64 * f0;
            if hz > 0.4 * sr {
                return None;
            }
            let w = std::f64::consts::TAU * hz / sr;
            let mut lv = Vec::new();
            let mut i = start;
            while i + len <= x.len() && lv.len() < 150 {
                let (mut re, mut im) = (0.0, 0.0);
                for (j, v) in x[i..i + len].iter().enumerate() {
                    let (s, c) = math::sin_cos(w * (i + j) as f64);
                    re += *v as f64 * c;
                    im += *v as f64 * s;
                }
                lv.push(db((re * re + im * im).sqrt()));
                i += hop;
            }
            let top = lv.iter().cloned().fold(-300.0, f64::max);
            let from = lv.iter().position(|v| *v >= top - 0.01)?;
            let end = lv
                .iter()
                .skip(from)
                .position(|v| *v < top - 30.0)
                .map_or(lv.len(), |p| from + p);
            if end < from + 5 {
                return None;
            }
            let n = (end - from) as f64;
            let ts: Vec<f64> = (from..end).map(|i| i as f64 * 0.01).collect();
            let ys = &lv[from..end];
            let (mt, my) = (ts.iter().sum::<f64>() / n, ys.iter().sum::<f64>() / n);
            let (mut num, mut den) = (0.0, 0.0);
            for (t, y) in ts.iter().zip(ys) {
                num += (t - mt) * (y - my);
                den += (t - mt) * (t - mt);
            }
            let slope = num / den;
            (slope < -0.5).then(|| 60.0 / -slope)
        })
        .collect()
}

/// What lies under 0.7·f0 in a plucked note's first 60 ms, against all of it (dB).
fn thump(x: &[f32], sr: f64, f0: f64) -> f64 {
    let start = x
        .iter()
        .enumerate()
        .fold(
            (0usize, 0f32),
            |m, (i, v)| if v.abs() > m.1 { (i, v.abs()) } else { m },
        )
        .0
        .saturating_sub((0.005 * sr) as usize);
    let n = (0.06 * sr) as usize;
    let seg = &x[start..(start + n).min(x.len())];
    let sp = spectrum(seg, 1 << 14);
    let bin = sr / (1 << 14) as f64;
    let (mut lo, mut all) = (0.0, 0.0);
    for (i, v) in sp.iter().enumerate() {
        let hz = i as f64 * bin;
        if hz < 20.0 {
            continue;
        }
        if hz < 0.7 * f0 {
            lo += v * v;
        }
        all += v * v;
    }
    10.0 * math::log10((lo / all).max(1e-15))
}
