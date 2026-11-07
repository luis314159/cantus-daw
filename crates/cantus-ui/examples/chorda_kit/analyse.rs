//! The measurements the comparison reports (round 37, ADR 0098), on any stereo render:
//! spectra in third-octave bands (the attack's first 50 ms and the tone at 0.5 s), the
//! attack (sub-fundamental "thump", noisiness, rise time), the decay per octave band, the
//! stereo image, and the release (how fast a damper stops a note, and its noise).
//! Everything in `f64`, `cantus_dsp::math` for the transcendentals.

use super::render::{Stereo, SR};
use cantus_dsp::math;

const FS: f64 = SR as f64;

/// Third-octave band centres, 31.5 Hz … 16 kHz (28 bands).
pub fn thirds() -> Vec<f64> {
    (0..28)
        .map(|k| 31.5 * math::powf(2.0f64, k as f64 / 3.0))
        .collect()
}

/// Octave band centres for the decay and the summaries.
pub const OCTAVES: [f64; 8] = [
    125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

pub fn db(x: f64) -> f64 {
    10.0 * math::log10(x.max(1e-30))
}

/// Both channels of a stretch: what every measure reads (the power of the two averaged —
/// a mono sum would comb-filter a spaced pair of microphones).
pub type Seg<'a> = (&'a [f32], &'a [f32]);

/// |X|² of `seg`'s two channels, averaged (Hann-windowed, zero-padded to `n`, a power of
/// two), bins 0 … n/2.
pub fn power(seg: Seg, n: usize) -> Vec<f64> {
    let a = power1(seg.0, n);
    let b = power1(seg.1, n);
    a.iter().zip(&b).map(|(x, y)| 0.5 * (x + y)).collect()
}

fn power1(seg: &[f32], n: usize) -> Vec<f64> {
    let m = seg.len().min(n);
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    let mut wsum = 0.0f64;
    for i in 0..m {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * (i as f64 + 0.5) / m as f64);
        re[i] = seg[i] * w as f32;
        wsum += w * w;
    }
    cantus_dsp::spectrum::fft(&mut re, &mut im);
    (0..=n / 2)
        .map(|k| (re[k] as f64 * re[k] as f64 + im[k] as f64 * im[k] as f64) / wsum.max(1e-12))
        .collect()
}

/// The energy of `p` (from [`power`] at size `n`) between `lo` and `hi` Hz.
pub fn band(p: &[f64], n: usize, lo: f64, hi: f64) -> f64 {
    let bin = FS / n as f64;
    let a = ((lo / bin).ceil() as usize).max(1);
    let b = ((hi / bin).floor() as usize).min(p.len() - 1);
    if b < a {
        // A band narrower than a bin: the nearest bin.
        let k = (((lo + hi) * 0.5) / bin).round() as usize;
        return p.get(k).copied().unwrap_or(0.0);
    }
    p[a..=b].iter().sum()
}

/// Third-octave levels (dB) of `seg`, relative to its total energy (a shape, level-free).
pub fn shape(seg: Seg) -> Vec<f64> {
    let n = seg.0.len().next_power_of_two().max(8192);
    let p = power(seg, n);
    let total: f64 = band(&p, n, 20.0, 20_000.0);
    let k = math::powf(2.0f64, 1.0 / 6.0);
    thirds()
        .iter()
        .map(|c| db(band(&p, n, c / k, c * k) / total.max(1e-30)))
        .collect()
}

/// The spectral centroid (Hz) of `seg` between 20 Hz and 20 kHz.
pub fn centroid(seg: Seg) -> f64 {
    let n = seg.0.len().next_power_of_two().max(4096);
    let p = power(seg, n);
    let bin = FS / n as f64;
    let (mut a, mut b) = (0.0, 0.0);
    for (k, e) in p.iter().enumerate() {
        let f = k as f64 * bin;
        if (20.0..20_000.0).contains(&f) {
            a += f * e;
            b += e;
        }
    }
    a / b.max(1e-30)
}

/// Spectral flatness (0 … 1: a pure tone … white noise) between `lo` and `hi`.
pub fn flatness(seg: Seg, lo: f64, hi: f64) -> f64 {
    let n = seg.0.len().next_power_of_two().max(4096);
    let p = power(seg, n);
    let bin = FS / n as f64;
    let (a, b) = ((lo / bin) as usize, ((hi / bin) as usize).min(p.len() - 1));
    let mut lg = 0.0;
    let mut ar = 0.0;
    let mut c = 0;
    for e in &p[a..=b] {
        lg += math::ln(e.max(1e-30));
        ar += e;
        c += 1;
    }
    if c == 0 || ar <= 0.0 {
        return 0.0;
    }
    math::exp(lg / c as f64) / (ar / c as f64)
}

pub fn energy(x: &[f32]) -> f64 {
    x.iter().map(|v| *v as f64 * *v as f64).sum()
}

pub fn rms_db(x: Seg) -> f64 {
    db((energy(x.0) + energy(x.1)) / (2 * x.0.len().max(1)) as f64)
}

pub fn secs(s: f64) -> usize {
    (s * FS).round() as usize
}

/// The first sample at or after `from` where the signal reaches 5 % of its peak in the
/// next `within` seconds, less a millisecond: where a note really starts.
pub fn onset(x: &[f32], from: usize, within: f64) -> usize {
    let end = (from + secs(within)).min(x.len());
    let peak = x[from..end].iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let at = x[from..end]
        .iter()
        .position(|v| v.abs() >= 0.05 * peak)
        .unwrap_or(0);
    (from + at).saturating_sub(secs(0.001))
}

/// The first partial (Hz) of key `m` in equal temperament (the bands only need it near).
pub fn f0(m: u8) -> f64 {
    440.0 * math::powf(2.0f64, (m as f64 - 69.0) / 12.0)
}

/// What one note measures.
#[derive(Clone, Debug, Default)]
pub struct NoteM {
    pub key: u8,
    pub vel: u8,
    /// RMS over the first second (dBFS) and the peak (dBFS).
    pub level: f64,
    pub peak: f64,
    /// Third-octave shapes: the first 50 ms, and 0.4–0.6 s.
    pub early: Vec<f64>,
    pub mid: Vec<f64>,
    pub centroid_early: f64,
    pub centroid_mid: f64,
    /// The share of the first 60 ms below 0.7·f₀ (dB of the window's energy): the knock.
    pub thump: f64,
    /// Flatness of 1–8 kHz in the first 30 ms: the strike's noise.
    pub noise: f64,
    /// 10 → 90 % of the 1 ms envelope's first peak (ms).
    pub rise_ms: f64,
    /// Decay per octave band (dB/s, positive = falling) from 0.15 s to the release.
    pub decay: Vec<f64>,
    /// The broadband decay early (0.1–0.6 s) and late (1.5 s – release), dB/s.
    pub decay_early: f64,
    pub decay_late: f64,
    /// Stereo over the first second: correlation, balance (L/R dB), side under mid (dB).
    pub corr: f64,
    pub balance: f64,
    pub side: f64,
    /// After the key comes up: ms until 30 dB down, and 2 kHz+ energy in the first 100 ms
    /// against the 100 ms before (dB).
    pub release_ms: f64,
    pub release_hf: f64,
}

fn envelope_db(x: Seg, win: usize) -> Vec<f64> {
    x.0.chunks(win).zip(x.1.chunks(win)).map(rms_db).collect()
}

/// A least-squares slope (dB per second) of `(t, dB)` points.
fn slope(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return f64::NAN;
    }
    let n = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
    let (mx, my) = (sx / n, sy / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in points {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    -num / den.max(1e-12)
}

/// Band levels over time: frames of 2048 hopped by 1024, dB per octave band.
fn band_tracks(x: Seg, from: f64, to: f64) -> Vec<(f64, Vec<f64>)> {
    let (n, hop) = (2048usize, 1024usize);
    let mut out = Vec::new();
    let mut i = secs(from);
    let k = 2.0f64.sqrt();
    while i + n <= secs(to).min(x.0.len()) {
        let p = power((&x.0[i..i + n], &x.1[i..i + n]), n);
        let lv = OCTAVES
            .iter()
            .map(|c| db(band(&p, n, c / k, c * k)))
            .collect();
        out.push(((i + n / 2) as f64 / FS, lv));
        i += hop;
    }
    out
}

/// Measure one note of `s` struck at `start` (samples) and let go `held` seconds later.
pub fn note(s: &Stereo, key: u8, vel: u8, start: usize, held: f64) -> NoteM {
    let mono = s.mono();
    let on = onset(&mono, start, 0.25);
    let off = start + secs(held);
    let x: Seg = (&s.l[on..], &s.r[on..]);
    let len = x.0.len();
    let w = |a: f64, b: f64| -> Seg {
        let (i, j) = (secs(a).min(len), secs(b).min(len));
        (&x.0[i..j], &x.1[i..j])
    };
    let mut m = NoteM {
        key,
        vel,
        level: rms_db(w(0.0, 1.0)),
        ..Default::default()
    };
    m.peak = db(w(0.0, 1.0)
        .0
        .iter()
        .chain(w(0.0, 1.0).1)
        .fold(0.0f64, |a, v| a.max(*v as f64 * *v as f64)));
    m.early = shape(w(0.0, 0.05));
    m.mid = shape(w(0.4, 0.6));
    m.centroid_early = centroid(w(0.0, 0.05));
    m.centroid_mid = centroid(w(0.4, 0.6));
    {
        let seg = w(0.0, 0.06);
        let n = 16_384;
        let p = power(seg, n);
        let lim = 0.7 * f0(key);
        m.thump = if lim > 30.0 {
            db(band(&p, n, 20.0, lim) / band(&p, n, 20.0, 20_000.0).max(1e-30))
        } else {
            f64::NAN
        };
    }
    m.noise = flatness(w(0.0, 0.03), 1_000.0, 8_000.0);
    {
        let seg = w(0.0, 0.2);
        let env: Vec<f64> = seg
            .0
            .chunks(secs(0.001))
            .zip(seg.1.chunks(secs(0.001)))
            .map(|(a, b)| ((energy(a) + energy(b)) / (2 * a.len().max(1)) as f64).sqrt())
            .collect();
        let peak = env.iter().cloned().fold(0.0, f64::max);
        let a = env.iter().position(|v| *v >= 0.1 * peak).unwrap_or(0);
        let b = env.iter().position(|v| *v >= 0.9 * peak).unwrap_or(0);
        m.rise_ms = (b as f64 - a as f64).max(0.0);
    }
    let end = (held - (on - start) as f64 / FS - 0.05).max(0.3);
    // The floor: what is left just before the next note (a recording's noise, a room's
    // last tail); a decay is read only where it stands 12 dB clear of it.
    let floor = band_tracks(x, 5.5, 5.95);
    let floor_b: Vec<f64> = (0..OCTAVES.len())
        .map(|b| floor.iter().map(|(_, l)| l[b]).fold(-300.0, f64::max))
        .collect();
    let floor_all = if len > secs(5.95) {
        rms_db(w(5.5, 5.95))
    } else {
        -300.0
    };
    let tracks = band_tracks(x, 0.15, end);
    m.decay = (0..OCTAVES.len())
        .map(|b| {
            let top = tracks.iter().map(|(_, l)| l[b]).fold(-300.0, f64::max);
            let pts: Vec<(f64, f64)> = tracks
                .iter()
                .filter(|(_, l)| l[b] > top - 50.0 && l[b] > -110.0 && l[b] > floor_b[b] + 12.0)
                .map(|(t, l)| (*t, l[b]))
                .collect();
            slope(&pts)
        })
        .collect();
    let env = envelope_db(x, secs(0.01));
    let pts = |a: f64, b: f64| -> Vec<(f64, f64)> {
        env.iter()
            .enumerate()
            .map(|(i, v)| (i as f64 * 0.01, *v))
            .filter(|(t, v)| *t >= a && *t < b && *v > -120.0 && *v > floor_all + 12.0)
            .collect()
    };
    m.decay_early = slope(&pts(0.1, 0.6));
    m.decay_late = slope(&pts(1.5, end));
    {
        let (l, r) = (
            &s.l[on..(on + secs(1.0)).min(s.len())],
            &s.r[on..(on + secs(1.0)).min(s.len())],
        );
        let (el, er) = (energy(l), energy(r));
        let lr: f64 = l.iter().zip(r).map(|(a, b)| *a as f64 * *b as f64).sum();
        m.corr = lr / (el * er).sqrt().max(1e-30);
        m.balance = db(el / er.max(1e-30));
        let mid: f64 = l
            .iter()
            .zip(r)
            .map(|(a, b)| ((a + b) as f64 * 0.5).powi(2))
            .sum();
        let side: f64 = l
            .iter()
            .zip(r)
            .map(|(a, b)| ((a - b) as f64 * 0.5).powi(2))
            .sum();
        m.side = db(side / mid.max(1e-30));
    }
    if off + secs(2.4) <= mono.len() {
        let part = |a: usize, b: usize| -> Seg { (&s.l[a..b], &s.r[a..b]) };
        let before = part(off - secs(0.1), off);
        let lvl = rms_db(before);
        let env: Vec<f64> = envelope_db(part(off, off + secs(2.4)), secs(0.005));
        m.release_ms = env
            .iter()
            .position(|v| *v < lvl - 30.0)
            .map(|i| i as f64 * 5.0)
            .unwrap_or(2_400.0);
        let n = 8192;
        let pa = power(part(off, off + secs(0.1)), n);
        let pb = power(before, n);
        m.release_hf =
            db(band(&pa, n, 2_000.0, 20_000.0) / band(&pb, n, 2_000.0, 20_000.0).max(1e-30));
    } else {
        m.release_ms = f64::NAN;
        m.release_hf = f64::NAN;
    }
    m
}

/// What a whole phrase measures.
#[derive(Clone, Debug, Default)]
pub struct PhraseM {
    pub lufs: f64,
    pub peak: f64,
    /// The long-term spectrum (third octaves, dB of the total).
    pub ltas: Vec<f64>,
    pub centroid: f64,
    pub corr: f64,
    pub side: f64,
    /// Peak to loudness (dB): how much the attacks stand out.
    pub plr: f64,
}

pub fn lufs(s: &Stereo) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(SR as f32);
    m.process(&s.l, &s.r);
    m.integrated().unwrap_or(-120.0)
}

pub fn phrase(s: &Stereo) -> PhraseM {
    let mono = s.mono();
    let mut m = PhraseM {
        lufs: lufs(s),
        peak: 20.0 * math::log10(s.peak().max(1e-9) as f64),
        ..Default::default()
    };
    // The long-term spectrum: the average of 8192-point frames.
    let n = 8192;
    let mut acc = vec![0.0f64; n / 2 + 1];
    let mut i = 0;
    while i + n <= mono.len() {
        for (a, p) in acc
            .iter_mut()
            .zip(power((&s.l[i..i + n], &s.r[i..i + n]), n))
        {
            *a += p;
        }
        i += n / 2;
    }
    let total = band(&acc, n, 20.0, 20_000.0);
    let k = math::powf(2.0f64, 1.0 / 6.0);
    m.ltas = thirds()
        .iter()
        .map(|c| db(band(&acc, n, c / k, c * k) / total.max(1e-30)))
        .collect();
    let bin = FS / n as f64;
    let (mut a, mut b) = (0.0, 0.0);
    for (j, e) in acc.iter().enumerate() {
        let f = j as f64 * bin;
        if (20.0..20_000.0).contains(&f) {
            a += f * e;
            b += e;
        }
    }
    m.centroid = a / b.max(1e-30);
    let (el, er) = (energy(&s.l), energy(&s.r));
    let lr: f64 =
        s.l.iter()
            .zip(&s.r)
            .map(|(a, b)| *a as f64 * *b as f64)
            .sum();
    m.corr = lr / (el * er).sqrt().max(1e-30);
    let mid: f64 =
        s.l.iter()
            .zip(&s.r)
            .map(|(a, b)| ((a + b) as f64 * 0.5).powi(2))
            .sum();
    let side: f64 =
        s.l.iter()
            .zip(&s.r)
            .map(|(a, b)| ((a - b) as f64 * 0.5).powi(2))
            .sum();
    m.side = db(side / mid.max(1e-30));
    m.plr = m.peak - m.lufs;
    m
}
