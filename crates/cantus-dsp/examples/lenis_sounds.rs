//! Lenis's listening examples (fx side round, wave B; the name is provisional): three harsh
//! sources, each dry, through a gentle and a strong setting, and the strong setting's
//! delta (what it takes away).
//!
//! - **Guitar**: the owner's reference progression (Am, F, C, G at 90 BPM, struck on beats 1
//!   and 3, eight bars) from Carbonthief's CC0 DI strums, through two of Fragor's modelled
//!   pedals in a row (the green overdrive into the op-amp distortion, high gain) and a
//!   crude, honky cabinet of our own (a 2.7 kHz and a 4.6 kHz peak) — a fizzy, harsh tone.
//! - **Voice**: a sung line (formant voice, narrow upper formants, a presence peak) with
//!   whistling "s" sounds — harsh and sibilant, synthesised, no recording.
//! - **Drums**: a synthesised kit whose toms and snare ring boxily (shell modes at 430 Hz
//!   and 1.18 kHz, a snare ping at 900 Hz).
//!
//! ```text
//! cargo run -p cantus-dsp --release --example lenis_sounds -- <dir of the strums as WAV> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. Every processed file is aligned with its dry twin (Lenis's
//! latency taken off) and level-matched to it (RMS), peak under −1 dBFS; the delta is
//! normalised to −6 dBFS peak, its true level said in the log. DSP only: no engine.

use cantus_dsp::fragor::{BoardSettings, Fragor, Model};
use cantus_dsp::lenis::{FocusBand, Lenis, LenisSettings, Mode};
use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{self, Vowel};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const F: f32 = SR as f32;
const BPM: f64 = 90.0;
const BARS: usize = 8;
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];

fn read_mono(path: &Path) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let spec = r.spec();
    let ch = spec.channels as usize;
    let x: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let k = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|v| v.unwrap() as f32 * k).collect()
        }
    };
    (x.chunks(ch).map(|c| c[0]).collect(), spec.sample_rate)
}

/// A windowed-sinc resampler (64 taps, Blackman, the cutoff under both rates' Nyquist).
fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return x.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let cut = ratio.min(1.0) * 0.95;
    let half = 32i64;
    let n = (x.len() as f64 * ratio) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / ratio;
            let c = t.floor() as i64;
            let mut acc = 0.0f64;
            for k in c - half + 1..=c + half {
                if k < 0 || k as usize >= x.len() {
                    continue;
                }
                let d = t - k as f64;
                let sinc = if d.abs() < 1e-9 {
                    cut
                } else {
                    let a = std::f64::consts::PI * d * cut;
                    math::sin(a) / (std::f64::consts::PI * d)
                };
                let w = d / half as f64;
                let win = if w.abs() >= 1.0 {
                    0.0
                } else {
                    let p = std::f64::consts::PI * (w + 1.0);
                    0.42 - 0.5 * math::cos(p) + 0.08 * math::cos(2.0 * p)
                };
                acc += x[k as usize] as f64 * sinc * win;
            }
            acc as f32
        })
        .collect()
}

/// RBJ's peaking filter in place.
fn peaking(x: &mut [f32], hz: f32, q: f32, db: f32) {
    let a = math::powf(10.0f64, db as f64 / 40.0);
    let w = std::f64::consts::TAU * hz as f64 / SR as f64;
    let (sn, cs) = (math::sin(w), math::cos(w));
    let alpha = sn / (2.0 * q as f64);
    let a0 = 1.0 + alpha / a;
    let (b0, b1, b2) = (
        (1.0 + alpha * a) / a0,
        -2.0 * cs / a0,
        (1.0 - alpha * a) / a0,
    );
    let (a1, a2) = (-2.0 * cs / a0, (1.0 - alpha / a) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for v in x.iter_mut() {
        let x0 = *v as f64;
        let y = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y;
        *v = y as f32;
    }
}

/// One-pole low-pass (`hz`) and high-pass (`hz`) in place.
fn low_pass(x: &mut [f32], hz: f32) {
    let a = 1.0 - math::exp(-std::f32::consts::TAU * hz / F);
    let mut z = 0.0f32;
    for v in x.iter_mut() {
        z += a * (*v - z);
        *v = z;
    }
}

fn high_pass(x: &mut [f32], hz: f32) {
    let a = 1.0 - math::exp(-std::f32::consts::TAU * hz / F);
    let mut z = 0.0f32;
    for v in x.iter_mut() {
        z += a * (*v - z);
        *v -= z;
    }
}

fn scale_peak(x: &mut [f32], db: f32) -> f32 {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let g = math::db_to_gain(db) / p;
    x.iter_mut().for_each(|v| *v *= g);
    20.0 * math::log10(g)
}

fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

fn beat() -> usize {
    (60.0 / BPM * SR as f64) as usize
}

/// The DI progression: two strums a bar, each faded over 20 ms before the next; peak −6 dBFS.
fn progression(dir: &Path) -> Vec<f32> {
    let beat = beat();
    let n = BARS * 4 * beat;
    let mut l = vec![0.0f32; n];
    let fade = (0.02 * F) as usize;
    let strums: Vec<Vec<f32>> = CHORDS
        .iter()
        .map(|c| {
            let (x, rate) = read_mono(&dir.join(format!("{c}.wav")));
            resample(&x, rate, SR)
        })
        .collect();
    for bar in 0..BARS {
        for half in 0..2 {
            let at = (bar * 4 + half * 2) * beat;
            let len = 2 * beat;
            let s = &strums[bar % 4];
            for i in 0..len.min(s.len()) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                l[at + i] += s[i] * g;
            }
        }
    }
    scale_peak(&mut l, -6.0);
    l
}

/// The DI through the overdrive into the op-amp distortion, then the honky cabinet.
fn distorted(di: &[f32]) -> Vec<f32> {
    let mut s = BoardSettings::default();
    s.chain[0] = Some(Model::Overdrive);
    s.chain[1] = Some(Model::OpAmp);
    s.knobs[Model::Overdrive.index()] = [0.7, 0.7, 0.5];
    s.knobs[Model::OpAmp.index()] = [0.75, 0.2, 0.5];
    s.mono = true;
    let mut f = Fragor::new(F);
    f.set(&s);
    f.reset();
    let lat = f.latency();
    let mut l: Vec<f32> = di
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, lat))
        .collect();
    let mut r = l.clone();
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        f.process(a, b);
    }
    let mut y = l[lat..].to_vec();
    high_pass(&mut y, 80.0);
    peaking(&mut y, 2_700.0, 3.0, 7.0);
    peaking(&mut y, 4_600.0, 5.0, 6.0);
    low_pass(&mut y, 7_500.0);
    low_pass(&mut y, 9_000.0);
    scale_peak(&mut y, -6.0);
    y
}

/// A harsh sung line: narrow upper formants, a presence peak, and whistling sibilants
/// between the phrases.
fn voice() -> Vec<f32> {
    let harsh_a = Vowel {
        formants: [
            (760.0, 70.0),
            (1_220.0, 80.0),
            (2_650.0, 60.0),
            (3_500.0, 70.0),
        ],
    };
    let harsh_e = Vowel {
        formants: [
            (530.0, 60.0),
            (1_840.0, 70.0),
            (2_480.0, 60.0),
            (3_300.0, 70.0),
        ],
    };
    let phrases: [[f32; 3]; 4] = [
        [220.0, 261.63, 293.66],
        [329.63, 293.66, 261.63],
        [246.94, 261.63, 220.0],
        [196.0, 220.0, 220.0],
    ];
    let note = 0.6f32;
    let gap = 0.9f32;
    let total = phrases.len() as f32 * (3.0 * note + gap);
    let n = (total * F) as usize;
    let mut x = vec![0.0f32; n];
    for (p, notes) in phrases.iter().enumerate() {
        let at = p as f32 * (3.0 * note + gap);
        for (k, f0) in notes.iter().enumerate() {
            let f0 = *f0;
            let vowel = if (p + k) % 2 == 0 { harsh_a } else { harsh_e };
            let v = testkit::voice(F, note, vowel, 0.5, move |t| {
                f0 * (1.0 + 0.007 * math::sin(std::f64::consts::TAU * 5.3 * t) as f32)
            });
            let start = ((at + k as f32 * note) * F) as usize;
            let edge = (0.025 * F) as usize;
            for (i, s) in v.iter().enumerate() {
                let g = (i.min(v.len() - 1 - i) as f32 / edge as f32).min(1.0);
                if start + i < n {
                    x[start + i] += s * g;
                }
            }
        }
        // An "s" at the end of the phrase.
        let mut s = testkit::sibilant(F, 0.22, 0.18, 77 + p as u64);
        peaking(&mut s, 6_800.0, 6.0, 10.0);
        let start = ((at + 3.0 * note) * F) as usize;
        let len = s.len();
        for (i, v) in s.iter().enumerate() {
            let g = (i.min(len - 1 - i) as f32 / (0.03 * F)).min(1.0);
            if start + i < n {
                x[start + i] += v * g;
            }
        }
    }
    peaking(&mut x, 3_100.0, 5.0, 8.0);
    scale_peak(&mut x, -6.0);
    x
}

/// One decaying mode: `hz` (gliding from `hz·(1 + glide)`), amplitude, time constant.
fn mode(n: usize, hz: f32, glide: f32, amp: f32, tau: f32) -> Vec<f32> {
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / F;
            let f = hz * (1.0 + glide * math::exp(-t / 0.03));
            ph += f / F;
            amp * math::exp(-t / tau) * math::sin(std::f32::consts::TAU * ph.fract())
        })
        .collect()
}

fn add(dst: &mut [f32], src: &[f32], at: usize) {
    for (i, v) in src.iter().enumerate() {
        if at + i < dst.len() {
            dst[at + i] += v;
        }
    }
}

/// A boxy kit at 100 BPM, four bars: kick on 1 and 3, a ringing snare on 2 and 4, a tom
/// fill in the last bar; the shells ring at 430 Hz and 1.18 kHz, the snare pings at 900 Hz.
fn drums() -> Vec<f32> {
    let beat = (60.0 / 100.0 * F) as usize;
    let n = 16 * beat + SR as usize;
    let mut x = vec![0.0f32; n];
    let hit = (0.9 * F) as usize;
    let kick = {
        let mut k = mode(hit, 52.0, 2.5, 0.9, 0.18);
        add(&mut k, &mode(hit, 430.0, 0.0, 0.10, 0.25), 0);
        k
    };
    let snare = {
        let mut s = mode(hit, 190.0, 0.3, 0.45, 0.09);
        add(&mut s, &mode(hit, 900.0, 0.0, 0.22, 0.35), 0);
        add(&mut s, &mode(hit, 1_180.0, 0.0, 0.10, 0.22), 0);
        let mut wires: Vec<f32> = (0..hit)
            .map(|i| 0.5 * hash(9_000 + i as u64) * math::exp(-(i as f32 / F) / 0.12))
            .collect();
        high_pass(&mut wires, 1_500.0);
        add(&mut s, &wires, 0);
        s
    };
    let tom = |hz: f32| {
        let mut t = mode(hit, hz, 0.4, 0.7, 0.3);
        add(&mut t, &mode(hit, 430.0, 0.0, 0.28, 0.4), 0);
        add(&mut t, &mode(hit, 1_180.0, 0.0, 0.12, 0.25), 0);
        t
    };
    for bar in 0..4 {
        let b0 = bar * 4 * beat;
        add(&mut x, &kick, b0);
        add(&mut x, &kick, b0 + 2 * beat);
        add(&mut x, &snare, b0 + beat);
        if bar < 3 {
            add(&mut x, &snare, b0 + 3 * beat);
        } else {
            for (i, hz) in [160.0f32, 140.0, 120.0, 100.0].iter().enumerate() {
                add(&mut x, &tom(*hz), b0 + 3 * beat + i * beat / 4);
            }
        }
    }
    scale_peak(&mut x, -6.0);
    x
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// Through Lenis at `s` (both channels the same source): the output aligned with the input,
/// the mean and the most of the meter's largest reduction.
fn through(x: &[f32], s: &LenisSettings) -> (Vec<f32>, f32, f32) {
    let mut u = Lenis::new(F);
    u.set(s);
    let lat = u.latency();
    let mut l: Vec<f32> = x
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, lat))
        .collect();
    let mut r = l.clone();
    let (mut sum, mut most, mut blocks) = (0.0f32, 0.0f32, 0usize);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(a, b);
        let m = u.meter().max_reduction_db;
        sum += m;
        most = most.max(m);
        blocks += 1;
    }
    (l[lat..].to_vec(), sum / blocks as f32, most)
}

fn write(path: &Path, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    let k = (1 << 23) as f32 - 1.0;
    for v in x {
        let s = (v.clamp(-1.0, 1.0) * k) as i32;
        w.write_sample(s).unwrap();
        w.write_sample(s).unwrap();
    }
    w.finalize().unwrap();
}

/// The gentle setting: Soft, a little depth, the default selectivity.
fn gentle() -> LenisSettings {
    LenisSettings {
        depth: 0.5,
        sharpness: 0.5,
        selectivity: 0.5,
        attack_ms: 5.0,
        release_ms: 80.0,
        mode: Mode::Soft,
        ..LenisSettings::default()
    }
}

/// The strong setting: Hard, deep, more selective of nothing (a lower threshold), faster.
fn strong() -> LenisSettings {
    LenisSettings {
        depth: 0.9,

        selectivity: 0.3,
        attack_ms: 3.0,
        release_ms: 60.0,
        mode: Mode::Hard,
        ..LenisSettings::default()
    }
}

/// A set: the dry twin, gentle, strong and the strong delta.
fn set(
    dir: &Path,
    prefix: &str,
    dry: &[f32],
    low_hz: f32,
    focus: Option<FocusBand>,
    sharpness: f32,
    log: &mut String,
) {
    write(&dir.join(format!("{prefix}-0-dry.wav")), dry);
    let dry_rms = rms(dry);
    for (name, mut s) in [("1-gentle", gentle()), ("2-strong", strong())] {
        s.low_hz = low_hz;
        s.sharpness = sharpness;
        if let Some(b) = focus {
            s.bands[0] = b;
        }
        let (mut y, mean, most) = through(dry, &s);
        let bands = octaves(dry, &y);
        let mut g = (dry_rms / rms(&y).max(1e-12)) as f32;
        let peak = y.iter().fold(0f32, |m, v| m.max(v.abs())) * g;
        if peak > math::db_to_gain(-1.0) {
            g *= math::db_to_gain(-1.0) / peak;
        }
        y.iter_mut().for_each(|v| *v *= g);
        write(&dir.join(format!("{prefix}-{name}.wav")), &y);
        let _ = writeln!(
            log,
            "{prefix}-{name}.wav: depth {:.0} %, {:?}, selectivity {:.0} %, sharpness {:.0} %, attack {} ms, release {} ms, from {low_hz} Hz{}; the largest reduction {mean:.1} dB on average, {most:.1} dB at most; level-matched {:+.2} dB",
            s.depth * 100.0,
            s.mode,
            s.selectivity * 100.0,
            s.sharpness * 100.0,
            s.attack_ms,
            s.release_ms,
            focus.map_or(String::new(), |b| format!(", focus +{:.0} % at {:.0} Hz", b.amount * 100.0, b.hz)),
            20.0 * math::log10(g)
        );
        let _ = writeln!(log, "  octave bands before matching (dB): {bands}");
        if name == "2-strong" {
            let (mut d, _, _) = through(dry, &LenisSettings { delta: true, ..s });
            let level = 20.0 * math::log10(rms(&d) / dry_rms);
            let gain = scale_peak(&mut d, -6.0);
            write(&dir.join(format!("{prefix}-3-strong-delta.wav")), &d);
            let _ = writeln!(
                log,
                "{prefix}-3-strong-delta.wav: what the strong setting takes away, {level:.1} dB under the dry (RMS); raised {gain:+.1} dB to peak at -6 dBFS"
            );
        }
    }
    log.push('\n');
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let strums = PathBuf::from(args.get(1).expect("the strums' folder"));
    let out = PathBuf::from(args.get(2).expect("the output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut log = String::new();
    let di = progression(&strums);
    write(&out.join("00-guitar-di.wav"), &di);
    let _ = writeln!(log, "00-guitar-di.wav: the DI progression, peak -6 dBFS\n");
    let guitar = distorted(&di);
    set(
        &out,
        "01-guitar",
        &guitar,
        150.0,
        Some(FocusBand {
            on: true,
            hz: 3_500.0,
            amount: 0.5,
            width_oct: 1.5,
        }),
        0.3,
        &mut log,
    );
    set(&out, "02-voice", &voice(), 1_000.0, None, 0.45, &mut log);
    set(
        &out,
        "03-drums",
        &drums(),
        300.0,
        Some(FocusBand {
            on: true,
            hz: 600.0,
            amount: 0.6,
            width_oct: 1.5,
        }),
        0.4,
        &mut log,
    );
    std::fs::write(out.join("render-log.txt"), &log).unwrap();
    print!("{log}");
}

/// The change in each octave band (125 Hz … 16 kHz) from `dry` to `y`, long-term (Welch,
/// 4096-point Hann, hop 1024), before any level matching.
fn octaves(dry: &[f32], y: &[f32]) -> String {
    let n = 4096;
    let fft = cantus_dsp::aequalis::fft::Fft::new(n);
    let w: Vec<f32> = (0..n)
        .map(|i| (0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64)) as f32)
        .collect();
    let power = |x: &[f32]| {
        let mut sum = vec![0.0f64; n / 2 + 1];
        let (mut re, mut im) = (vec![0.0f32; n], vec![0.0f32; n]);
        let mut k = 0;
        while k + n <= x.len() {
            for i in 0..n {
                re[i] = x[k + i] * w[i];
                im[i] = 0.0;
            }
            fft.run(&mut re, &mut im, false);
            for (b, s) in sum.iter_mut().enumerate() {
                *s += (re[b] as f64) * (re[b] as f64) + (im[b] as f64) * (im[b] as f64);
            }
            k += n / 4;
        }
        sum
    };
    let (pd, py) = (power(dry), power(y));
    let hz = F as f64 / n as f64;
    (0..8)
        .map(|o| {
            let c = 125.0 * (1u32 << o) as f64;
            let (lo, hi) = (c / 2f64.sqrt(), c * 2f64.sqrt());
            let band = |p: &[f64]| -> f64 {
                p.iter()
                    .enumerate()
                    .filter(|(b, _)| (*b as f64 * hz) >= lo && (*b as f64 * hz) < hi)
                    .map(|(_, v)| v)
                    .sum()
            };
            let d = 10.0 * math::log10(band(&py).max(1e-30) / band(&pd).max(1e-30));
            let name = if c >= 1000.0 {
                format!("{}k", c / 1000.0)
            } else {
                format!("{c}")
            };
            format!("{name} {d:+.1}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}
