//! Arca's voice measured for an A/B between two builds (phase A-3's check): for the amps
//! with followers (crunch, chime, high gain, bass) at the listening files' settings,
//!
//! - the long-term ⅓-octave spectrum of the reference progression through the amp and its
//!   cabinet (Welch, 8 192-point Hann frames, half overlap; 25 Hz … 16 kHz);
//! - the harmonics H1 … H9 of a 1 kHz and a 110 Hz sine (0.3 V), amp only;
//! - the quasi-static transfer: one cycle of a 40 Hz sine at 0.5 V, amp only.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arca_compare -- <guitar-wav-dir> <out.txt>
//! ```
//!
//! One line per measurement, `key value value …`, for a script to diff.

use cantus_dsp::arca::cab::{impulse, CabSettings, Cabinet, Mic};
use cantus_dsp::arca::{AmpModel, Arca, ArcaSettings};
use cantus_dsp::aula_kernel::{Format, Kernel, Layout};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SR: f32 = 48_000.0;

fn read_mono(p: &Path) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(p).unwrap();
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
    (
        raw.chunks(ch)
            .map(|f| f.iter().sum::<f32>() / ch as f32)
            .collect(),
        spec.sample_rate,
    )
}

/// Linear interpolation to 48 kHz (both builds the same: only the amps differ).
fn to_48k(x: &[f32], from: u32) -> Vec<f32> {
    let step = from as f64 / 48_000.0;
    let n = (x.len() as f64 / step) as usize - 1;
    (0..n)
        .map(|j| {
            let p = j as f64 * step;
            let i = p as usize;
            let f = (p - i as f64) as f32;
            x[i] + (x[(i + 1).min(x.len() - 1)] - x[i]) * f
        })
        .collect()
}

fn progression(dir: &Path) -> Vec<f32> {
    let names = [
        "Sample07 (A Minor).wav",
        "Sample15 (F Major).wav",
        "Sample22 (C Major).wav",
        "Sample17 (G Major).wav",
    ];
    let chords: Vec<Vec<f32>> = names
        .iter()
        .map(|n| {
            let (x, sr) = read_mono(&dir.join(n));
            to_48k(&x, sr)
        })
        .collect();
    let beat = (60.0 / 90.0 * SR) as usize;
    let mut out = vec![0.0f32; 8 * 4 * beat + SR as usize];
    for bar in 0..8 {
        for half in 0..2 {
            let start = (bar * 4 + half * 2) * beat;
            let c = &chords[bar % 4];
            for i in 0..(2 * beat).min(c.len()) {
                out[start + i] += c[i];
            }
        }
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    out.iter().map(|v| v * 0.501 / peak).collect()
}

/// In-place radix-2 FFT (f64).
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
        let a = -std::f64::consts::TAU / len as f64;
        for s in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sn, cs) = cantus_dsp::math::sin_cos(a * k as f64);
                let (ur, ui) = (re[s + k], im[s + k]);
                let (vr, vi) = (re[s + k + len / 2], im[s + k + len / 2]);
                let (tr, ti) = (vr * cs - vi * sn, vr * sn + vi * cs);
                re[s + k] = ur + tr;
                im[s + k] = ui + ti;
                re[s + k + len / 2] = ur - tr;
                im[s + k + len / 2] = ui - ti;
            }
        }
        len <<= 1;
    }
}

/// ⅓-octave band levels (dB) of `x`, bands centred 25 Hz … 16 kHz.
fn third_octaves(x: &[f32]) -> Vec<f64> {
    let n = 8_192;
    let mut psd = vec![0.0f64; n / 2];
    let mut frames = 0;
    let mut s = 0;
    while s + n <= x.len() {
        let mut re: Vec<f64> = (0..n)
            .map(|i| {
                let w =
                    0.5 - 0.5 * cantus_dsp::math::cos(std::f64::consts::TAU * i as f64 / n as f64);
                x[s + i] as f64 * w
            })
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        for k in 0..n / 2 {
            psd[k] += re[k] * re[k] + im[k] * im[k];
        }
        frames += 1;
        s += n / 2;
    }
    (0..29)
        .map(|b| {
            let fc = 25.0 * cantus_dsp::math::powf(2.0f64, b as f64 / 3.0);
            let (lo, hi) = (
                fc / cantus_dsp::math::powf(2.0f64, 1.0 / 6.0),
                fc * cantus_dsp::math::powf(2.0f64, 1.0 / 6.0),
            );
            let mut e = 0.0;
            for (k, p) in psd.iter().enumerate() {
                let f = k as f64 * SR as f64 / n as f64;
                if f >= lo && f < hi {
                    e += p;
                }
            }
            10.0 * cantus_dsp::math::log10(e / frames as f64 + 1e-30)
        })
        .collect()
}

fn kernel(c: Cabinet) -> Arc<Kernel> {
    let h = impulse(
        &CabSettings {
            cabinet: c,
            mic: Mic::Dynamic,
            position: 0.3,
            distance_cm: 3.0,
        },
        SR,
    );
    Arc::new(Kernel::new(
        Format::Mono,
        std::slice::from_ref(&h),
        Layout::for_taps(h.len()),
    ))
}

fn run(x: &[f32], s: &ArcaSettings, cab: Option<Cabinet>) -> Vec<f32> {
    let mut a = match cab {
        Some(c) => Arca::with_cab(SR, kernel(c)),
        None => Arca::new(SR),
    };
    a.set(s);
    a.reset();
    let (mut l, mut r) = (x.to_vec(), x.to_vec());
    for (bl, br) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        a.process(bl, br);
    }
    l
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(&args[1]);
    let di = progression(&dir);
    let mut out = String::new();
    let models = [
        (
            AmpModel::Crunch,
            Cabinet::FourTwelveVintage,
            [(0.15, 0.45), (0.7, 1.0)],
        ),
        (
            AmpModel::Chime,
            Cabinet::TwoTwelveOpen,
            [(0.25, 0.6), (0.8, 1.0)],
        ),
        (
            AmpModel::HighGain,
            Cabinet::FourTwelveModern,
            [(0.12, 0.5), (0.75, 0.7)],
        ),
        (
            AmpModel::Bass,
            Cabinet::FourTenBass,
            [(0.3, 1.0), (0.8, 1.0)],
        ),
    ];
    for (m, cab, voicings) in models {
        for (which, (gain, master)) in ["clean", "driven"].iter().zip(voicings) {
            let s = ArcaSettings {
                model: Some(m),
                gain,
                master,
                cab: true,
                ..ArcaSettings::default()
            };
            let key = format!("{m:?}-{which}");
            let y = run(&di, &s, Some(cab));
            let bands = third_octaves(&y);
            let _ = writeln!(
                out,
                "{key}-spectrum {}",
                bands
                    .iter()
                    .map(|v| format!("{v:.3}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let amp_only = ArcaSettings { cab: false, ..s };
            for hz in [1_000.0f64, 110.0] {
                let n = 48_000;
                let x: Vec<f32> = (0..2 * n)
                    .map(|i| {
                        (0.3 * cantus_dsp::math::sin(
                            std::f64::consts::TAU * hz * i as f64 / SR as f64,
                        )) as f32
                    })
                    .collect();
                let y = run(&x, &amp_only, None);
                let tail = &y[n..];
                let h: Vec<String> = (1..=9)
                    .map(|k| {
                        let w = std::f64::consts::TAU * hz * k as f64 / SR as f64;
                        let (mut re, mut im) = (0.0, 0.0);
                        for (i, v) in tail.iter().enumerate() {
                            let (sn, cs) = cantus_dsp::math::sin_cos(w * i as f64);
                            re += *v as f64 * cs;
                            im += *v as f64 * sn;
                        }
                        format!(
                            "{:.3}",
                            20.0 * cantus_dsp::math::log10(
                                2.0 * (re * re + im * im).sqrt() / n as f64 + 1e-30
                            )
                        )
                    })
                    .collect();
                let _ = writeln!(out, "{key}-harmonics-{hz} {}", h.join(" "));
            }
            // One cycle of 40 Hz at 0.5 V, after two seconds.
            let n = 2 * SR as usize + 1_200;
            let x: Vec<f32> = (0..n)
                .map(|i| {
                    (0.5 * cantus_dsp::math::sin(
                        std::f64::consts::TAU * 40.0 * i as f64 / SR as f64,
                    )) as f32
                })
                .collect();
            let y = run(&x, &amp_only, None);
            let cyc: Vec<String> = y[n - 1_200..]
                .iter()
                .step_by(20)
                .map(|v| format!("{v:.5}"))
                .collect();
            let _ = writeln!(out, "{key}-transfer {}", cyc.join(" "));
            eprintln!("{key}");
        }
    }
    std::fs::write(&args[2], out).unwrap();
}
