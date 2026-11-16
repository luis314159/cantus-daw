//! Purus's listening examples (fx side round, wave E; the name is provisional).
//!
//! - **Hum**: the owner's reference guitar progression (Am, F, C, G at 90 BPM, struck on
//!   beats 1 and 3, eight bars, from Carbonthief's CC0 DI strums) with mains hum added —
//!   50 Hz drifting ±0.3 Hz, eight harmonics and a rectifier's buzz — then de-hummed by the
//!   notches (tracked, 16 harmonics) and by the gentle mode, each with its delta.
//! - **A bass on the hum**: a synthetic bass line on 50, 100 and 150 Hz (and beside them)
//!   with the same hum, through the notches and the gentle mode: what each does to the bass.
//! - **Clicks and crackle**: the guitar with 60 synthetic clicks and crackle (300 small
//!   impulses a second) added, through the de-click alone and with the de-crackle, with
//!   deltas; and the clean guitar through it (what it takes that is not a click).
//! - **Room**: a synthetic spoken line (the pitch tests' formant voice, no recording) in a
//!   synthetic room (T60 0.9 s, the direct sound as strong as all the reverberation),
//!   de-reverberated at 12 and 24 dB with the decay estimated, and the delta.
//!
//! Every file has its dry twin; processed files are *not* level-matched (what is taken out
//! is the point). The de-reverb's files are aligned to their input (its 21.3 ms taken off).
//!
//! ```text
//! cargo run -p cantus-dsp --example purus_sounds -- <dir of the strums as WAV> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. DSP only: Purus's own unit, no engine.

use cantus_dsp::math;
use cantus_dsp::purus::testkit;
use cantus_dsp::purus::{
    latency_for, ClickSettings, DecayMode, HumMode, HumSettings, Purus, PurusSettings, VerbSettings,
};
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const BPM: f64 = 90.0;
const BARS: usize = 8;
const CHORDS: [&str; 4] = [
    "Sample07 (A Minor)",
    "Sample15 (F Major)",
    "Sample22 (C Major)",
    "Sample17 (G Major)",
];

type Stereo = (Vec<f32>, Vec<f32>);

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

/// The progression: two strums a bar, each faded over 20 ms before the next; peak −6 dBFS.
fn progression(dir: &Path) -> Vec<f32> {
    let beat = (60.0 / BPM * SR as f64) as usize;
    let n = BARS * 4 * beat;
    let mut l = vec![0.0f32; n];
    let fade = (0.02 * SR as f64) as usize;
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

fn scale_peak(x: &mut [f32], db: f32) {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let g = math::db_to_gain(db) / p;
    for v in x {
        *v *= g;
    }
}

fn write(path: &Path, s: &Stereo) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    let k = (1 << 23) as f32 - 1.0;
    for (a, b) in s.0.iter().zip(&s.1) {
        w.write_sample((a.clamp(-1.0, 1.0) * k) as i32).unwrap();
        w.write_sample((b.clamp(-1.0, 1.0) * k) as i32).unwrap();
    }
    w.finalize().unwrap();
}

/// Through Purus at `s`, aligned to the input (the latency taken off), as long as the input.
fn through(x: &[f32], s: &PurusSettings) -> Stereo {
    let lat = latency_for(s, SR as f32);
    let mut l = x.to_vec();
    l.resize(x.len() + lat, 0.0);
    let mut r = l.clone();
    let mut p = Purus::new(SR as f32);
    p.set(s);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        p.process(a, b);
    }
    (l[lat..].to_vec(), r[lat..].to_vec())
}

fn mono(x: &[f32]) -> Stereo {
    (x.to_vec(), x.to_vec())
}

fn db(x: f64) -> f64 {
    20.0 * math::log10(x.max(1e-12))
}

/// The hum's harmonics' levels (dBFS) in `x`, followed along the hum's phase `theta` (it
/// drifts), as a line.
fn harmonics(x: &[f32], theta: &[f64], k: usize) -> String {
    (1..=k)
        .map(|h| format!("{:.0}", db(testkit::level_following(x, theta, h))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn hum_on(f: impl FnOnce(&mut HumSettings)) -> PurusSettings {
    let mut h = HumSettings {
        on: true,
        mode: HumMode::Track,
        harmonics: 16,
        ..HumSettings::default()
    };
    f(&mut h);
    PurusSettings {
        hum: h,
        ..PurusSettings::default()
    }
}

fn verb_on(f: impl FnOnce(&mut VerbSettings)) -> PurusSettings {
    let mut v = VerbSettings {
        on: true,
        decay: DecayMode::Estimate,
        ..VerbSettings::default()
    };
    f(&mut v);
    PurusSettings {
        verb: v,
        ..PurusSettings::default()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let strums = PathBuf::from(args.get(1).expect("the strums' folder"));
    let out = PathBuf::from(args.get(2).expect("the output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut log = String::new();
    let mut extra = String::new();
    let mut save = |name: &str, s: &Stereo, note: &str| {
        write(&out.join(name), s);
        log += &format!("{name}: {note}\n");
        println!("{name}");
    };
    let sr = SR as f32;
    // The mains: 50 Hz drifting ±0.3 Hz over 20 s, eight harmonics from −30 dBFS, a buzz.
    let amps = [0.0316f32, 0.02, 0.0251, 0.01, 0.0126, 0.005, 0.0063, 0.004];
    let drift = |t: f64| 50.0 + 0.3 * math::sin(std::f64::consts::TAU * t / 20.0);

    // Hum on the guitar.
    let gtr = progression(&strums);
    let secs = gtr.len() as f32 / sr;
    let hum = testkit::hum(sr, secs, &amps, 0.012, 31, drift);
    let theta = testkit::hum_phase(sr, hum.len(), drift);
    let th = &theta[4 * SR as usize..];
    let humming: Vec<f32> = gtr.iter().zip(&hum).map(|(a, b)| a + b).collect();
    let from = 4 * SR as usize;
    save(
        "00-guitar-di.wav",
        &mono(&gtr),
        "the dry DI progression, peak -6 dBFS",
    );
    save(
        "01-guitar-with-hum.wav",
        &mono(&humming),
        &format!(
            "the same with mains hum and buzz added; harmonics (dBFS) {}",
            harmonics(&humming[from..], th, 8)
        ),
    );
    let notch = hum_on(|_| {});
    let y = through(&humming, &notch);
    let left: Vec<f32> = y.0.iter().zip(&gtr).map(|(a, b)| a - b).collect();
    save(
        "02-guitar-dehum-notches.wav",
        &y,
        &format!(
            "de-hum, notches: tracked, 16 harmonics, 40 dB, 2 Hz; what is left of the hum \
             (dBFS, the output less the DI) {}",
            harmonics(&left[from..], th, 8)
        ),
    );
    let d = through(&humming, &hum_on(|h| h.delta = true));
    save(
        "02b-guitar-dehum-notches-delta.wav",
        &d,
        "what the notches took out",
    );
    let gentle = hum_on(|h| h.gentle = true);
    let y = through(&humming, &gentle);
    let left: Vec<f32> = y.0.iter().zip(&gtr).map(|(a, b)| a - b).collect();
    save(
        "03-guitar-dehum-gentle.wav",
        &y,
        &format!(
            "de-hum, gentle: the same, the hum subtracted; what is left {}",
            harmonics(&left[from..], th, 8)
        ),
    );
    let d = through(
        &humming,
        &hum_on(|h| {
            h.gentle = true;
            h.delta = true;
        }),
    );
    save(
        "03b-guitar-dehum-gentle-delta.wav",
        &d,
        "what the gentle mode took out",
    );

    // Clicks and crackle on the guitar.
    let mut clicked = gtr.clone();
    let n_clicks = testkit::add_clicks(&mut clicked, 60, 51).len();
    let mut both = clicked.clone();
    testkit::add_crackle(&mut both, sr, 300.0, 53);
    let energy = |a: &[f32], b: &[f32]| -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| ((x - y) as f64) * ((x - y) as f64))
            .sum()
    };
    let down =
        |x: &[f32], y: &[f32]| 10.0 * math::log10(energy(x, &gtr) / energy(y, &gtr).max(1e-30));
    save(
        "30-guitar-clicks-and-crackle.wav",
        &mono(&both),
        &format!("the DI with {n_clicks} clicks (0.1-0.5 peak, 1-8 samples) and crackle (300 a second, 0.005-0.03 peak)"),
    );
    // The editor's offline declicker (the same detector and rebuilding, whole-take) on the
    // same, for comparison.
    for (what, x) in [("the clicks", &clicked), ("the clicks and crackle", &both)] {
        let mut y = x.clone();
        let len = y.len();
        let n = cantus_dsp::denoise::declick::declick(&mut y, 0, len, 0.5);
        extra += &format!(
            "the audio editor's offline declick at 0.5 on {what}: {n} rebuilt, down {:.1} dB
",
            down(x, &y)
        );
    }
    let click_on = |crackle: f32, delta: bool| PurusSettings {
        click: ClickSettings {
            on: true,
            crackle,
            delta,
            ..ClickSettings::default()
        },
        ..PurusSettings::default()
    };
    for (name, crackle) in [
        ("31-guitar-declick", 0.0f32),
        ("32-guitar-declick-decrackle", 1.0),
    ] {
        let s = click_on(crackle, false);
        let y = through(&both, &s);
        let mut probe = Purus::new(sr);
        probe.set(&s);
        let (mut l, mut r) = (both.clone(), both.clone());
        probe.process(&mut l, &mut r);
        let found = probe.meter().clicks_total / 2;
        let yc = through(&clicked, &s);
        let clean = through(&gtr, &s);
        let mut cprobe = Purus::new(sr);
        cprobe.set(&s);
        let (mut l, mut r) = (gtr.clone(), gtr.clone());
        cprobe.process(&mut l, &mut r);
        let false_found = cprobe.meter().clicks_total / 2;
        let change = 10.0
            * math::log10(
                gtr.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()
                    / energy(&clean.0, &gtr).max(1e-30),
            );
        save(
            &format!("{name}.wav"),
            &y,
            &format!(
                "sensitivity 0.5, crackle {crackle}: {found} rebuilt; the clicks and crackle \
                 down {:.1} dB (the clicks alone {:.1} dB); on the clean DI {false_found} \
                 rebuilt, the change {change:.1} dB under it",
                down(&both, &y.0),
                down(&clicked, &yc.0)
            ),
        );
        let d = through(&both, &click_on(crackle, true));
        save(&format!("{name}-delta.wav"), &d, "what it took out");
    }

    // A bass on the hum's own frequencies.
    let notes = [
        (0.5, 0.9, 50.0),
        (1.5, 0.9, 100.0),
        (2.5, 0.9, 150.0),
        (3.5, 0.45, 55.0),
        (4.0, 0.45, 61.7),
        (4.5, 0.9, 98.0),
        (5.5, 0.45, 73.4),
        (6.0, 1.4, 50.0),
    ];
    let bass = testkit::bass(sr, 8.0, &notes, 0.5);
    let hum = testkit::hum(sr, 8.0, &amps, 0.012, 37, drift);
    let humming: Vec<f32> = bass.iter().zip(&hum).map(|(a, b)| a + b).collect();
    save(
        "10-bass-dry.wav",
        &mono(&bass),
        "a synthetic bass: 50, 100, 150, 55, 62, 98, 73, 50 Hz",
    );
    save(
        "11-bass-with-hum.wav",
        &mono(&humming),
        "the same with the hum",
    );
    for (name, s, what) in [
        ("12-bass-dehum-notches.wav", &notch, "the notches"),
        ("13-bass-dehum-gentle.wav", &gentle, "the gentle mode"),
    ] {
        let y = through(&humming, s);
        let y_hum = through(&hum, s);
        let b_out: Vec<f32> = y.0.iter().zip(&y_hum.0).map(|(a, b)| a - b).collect();
        let notes_db: Vec<String> = notes
            .iter()
            .map(|(s0, len, hz)| {
                let a = (s0 * sr) as usize;
                let e = ((s0 + len) * sr) as usize;
                let g = testkit::energy(&b_out[a..e]) / testkit::energy(&bass[a..e]);
                format!("{hz} Hz {:+.1}", 10.0 * math::log10(g))
            })
            .collect();
        save(
            name,
            &y,
            &format!("{what}; the bass's notes, dB: {}", notes_db.join(", ")),
        );
    }

    // A spoken line in a room.
    let dry = testkit::spoken(sr, 140.0, 26, 1.5, 41);
    let (h, _) = testkit::room(sr, 0.9, 0.0, 50.0, 43);
    let mut wet = testkit::convolve(&dry, &h);
    wet.truncate(dry.len());
    let p = wet.iter().fold(0f32, |m, v| m.max(v.abs()));
    let g = math::db_to_gain(-6.0) / p;
    let dry: Vec<f32> = dry.iter().map(|v| v * g).collect();
    let wet: Vec<f32> = wet.iter().map(|v| v * g).collect();
    let end = dry.len() - (1.5 * sr) as usize;
    let tail = |x: &[f32]| {
        let (a, b) = (end + (0.05 * sr) as usize, end + (0.8 * sr) as usize);
        10.0 * math::log10(testkit::energy(&x[a..b]).max(1e-30) / 0.75 / sr as f64)
    };
    save(
        "20-voice-dry.wav",
        &mono(&dry),
        "a synthetic spoken line (our formant voice), dry",
    );
    save(
        "21-voice-in-room.wav",
        &mono(&wet),
        &format!(
            "the same in a synthetic room: T60 0.9 s, direct = reverberation (0 dB), early \
             part 50 ms; the tail after the line {:.1} dBFS",
            tail(&wet)
        ),
    );
    for (name, amount, direct) in [
        ("22-voice-dereverb-12dB-direct6", 12.0f32, 6.0f32),
        ("23-voice-dereverb-12dB-direct0", 12.0, 0.0),
        ("24-voice-dereverb-24dB-direct0", 24.0, 0.0),
    ] {
        let s = verb_on(|v| {
            v.amount_db = amount;
            v.direct_db = direct;
        });
        let y = through(&wet, &s);
        let mut probe = Purus::new(sr);
        probe.set(&s);
        let (mut l, mut r) = (wet.clone(), wet.clone());
        probe.process(&mut l, &mut r);
        let est = probe.meter().decay_estimated_s;
        save(
            &format!("{name}.wav"),
            &y,
            &format!(
                "de-reverb {amount} dB, direct {direct:+} dB, the decay estimated ({est:.2} s), early 50 ms; the tail after the line {:.1} dBFS",
                tail(&y.0)
            ),
        );
        let d = through(
            &wet,
            &verb_on(|v| {
                v.amount_db = amount;
                v.direct_db = direct;
                v.delta = true;
            }),
        );
        save(&format!("{name}-delta.wav"), &d, "what it took out");
    }
    log += &extra;
    std::fs::write(out.join("render-log.txt"), log).unwrap();
}
