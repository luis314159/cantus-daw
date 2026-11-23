//! Arcus beside a recording (round 42, ADR 0157): for each WAV given, its fundamental, the
//! level of its first harmonics against the fundamental, the noise between them (the bow
//! hair's layer: the level halfway between harmonics against the harmonics' own, by octave
//! band), its spectral centroid, and how long its level takes to reach 90 % of its sustain.
//! Run by `tools/arcus/compare.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_compare -- <near-Hz> <a.wav> [b.wav …]
//! ```
//!
//! `near-Hz` is where to look for the fundamental (within a minor third).

use cantus_dsp::math;

fn read(path: &str) -> (Vec<f32>, f64) {
    let mut r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let spec = r.spec();
    let ch = spec.channels as usize;
    let all: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.unwrap() as f32 / full).collect()
        }
    };
    let mono = all.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    (mono, spec.sample_rate as f64)
}

/// The level (RMS of the heterodyned band, Hann-windowed) at `hz` over `x[a..b]`.
fn level(x: &[f32], sr: f64, hz: f64, a: usize, b: usize) -> f64 {
    let w = std::f64::consts::TAU * hz / sr;
    let n = b - a;
    let (mut re, mut im) = (0.0, 0.0);
    for (j, v) in x[a..b].iter().enumerate() {
        let h = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * j as f64 / n as f64);
        let (s, c) = math::sin_cos(w * (a + j) as f64);
        re += h * *v as f64 * c;
        im += h * *v as f64 * s;
    }
    2.0 * (re * re + im * im).sqrt() / (0.5 * n as f64)
}

fn db(x: f64) -> f64 {
    20.0 * math::log10(x.max(1e-12))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let near: f64 = args[0].parse().expect("near-Hz");
    for path in &args[1..] {
        let (x, sr) = read(path);
        // The steady part: the second quarter to the third of the file.
        let (a, b) = (x.len() / 4, (x.len() * 3 / 4).min(x.len() / 4 + (2.0 * sr) as usize));
        // The fundamental: the autocorrelation's peak within a major third of `near`, then
        // the strongest of a fine scan around it.
        let seg = &x[a..(a + (0.4 * sr) as usize).min(b)];
        let (lo_p, hi_p) = ((sr / (near * 1.26)) as usize, (sr / (near / 1.26)) as usize);
        let mut best_lag = (lo_p, f64::MIN);
        for lag in lo_p..=hi_p.min(seg.len() / 2) {
            let c: f64 = seg[..seg.len() - lag]
                .iter()
                .zip(&seg[lag..])
                .map(|(p, q)| (*p as f64) * (*q as f64))
                .sum();
            if c > best_lag.1 {
                best_lag = (lag, c);
            }
        }
        let rough = sr / best_lag.0 as f64;
        let mut f0 = rough;
        let mut best = 0.0;
        for k in -60..=60 {
            let hz = rough * math::exp2(k as f64 / 1200.0);
            let l = level(&x, sr, hz, a, (a + (0.5 * sr) as usize).min(b));
            if l > best {
                best = l;
                f0 = hz;
            }
        }
        let h1 = level(&x, sr, f0, a, b);
        let mut harm = String::new();
        let mut bands = [(0.0f64, 0.0f64, 0usize); 5];
        let (mut cen_num, mut cen_den) = (0.0, 0.0);
        for n in 1usize..=40 {
            let f = f0 * n as f64;
            if f > 0.45 * sr {
                break;
            }
            let h = level(&x, sr, f, a, b);
            let g = level(&x, sr, f + 0.5 * f0, a, b);
            if n <= 10 {
                harm += &format!(" {:+.1}", db(h / h1));
            }
            cen_num += f * h * h;
            cen_den += h * h;
            // Octave bands from the fundamental: 1, 2–3, 4–7, 8–15, 16–31.
            let band = (usize::BITS - n.leading_zeros() - 1) as usize;
            if band < bands.len() {
                bands[band].0 += h * h;
                bands[band].1 += g * g;
                bands[band].2 += 1;
            }
        }
        let noise: Vec<String> = bands
            .iter()
            .filter(|b| b.2 > 0)
            .map(|b| format!("{:+.1}", 10.0 * math::log10((b.1 / b.0).max(1e-24))))
            .collect();
        // The attack: the first time the 10 ms RMS reaches 90 % of the steady RMS.
        let steady = (x[a..b].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / (b - a) as f64)
            .sqrt();
        let win = (0.01 * sr) as usize;
        let onset = x.iter().position(|v| v.abs() > 0.01 * steady as f32).unwrap_or(0);
        let mut t90 = f64::NAN;
        for i in (onset..x.len().saturating_sub(win)).step_by(win / 4) {
            let r = (x[i..i + win].iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / win as f64)
                .sqrt();
            if r >= 0.9 * steady {
                t90 = (i - onset) as f64 / sr;
                break;
            }
        }
        println!(
            "{path}\n  f0 {f0:.2} Hz · harmonics 1–10 (dB re the first):{harm}\n  noise between harmonics by octave band (dB re the harmonics): {}\n  centroid {:.0} Hz · steady RMS {:.1} dBFS · 90 % of it {:.0} ms after the onset",
            noise.join(" "),
            cen_num / cen_den.max(1e-30),
            db(steady),
            1000.0 * t90
        );
    }
}
