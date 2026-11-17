//! Caelum's listening examples (fx side round, wave A; the name is provisional): the
//! owner's reference guitar progression — Am, F, C, G at 90 BPM, struck on beats 1 and 3,
//! eight bars, from Carbonthief's CC0 DI strums — through the octave shimmer, the fifth
//! cathedral, the bloom pad and the infinite freeze swell; a synthesized sung line (the
//! pitch base's formant voice, no recording) through the ducked vocal space, with and
//! without the ducking; and a synthesized pad (detuned saws, our own) through the octave
//! shimmer and the endless ascent, and Infinite thrown on its tail at Sustain 100 % and
//! 40 %. Every set has its dry twin; each processed file is
//! level-matched to it (RMS over the dry's length) with its peak under −1 dBFS.
//!
//! ```text
//! cargo run -p cantus-dsp --example caelum_sounds -- <dir of the strums as WAV> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. DSP only: Caelum's own unit, no engine.

use cantus_dsp::caelum::{presets, Caelum, CaelumSettings};
use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{self, Vowel};
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

fn beat() -> usize {
    (60.0 / BPM * SR as f64) as usize
}

/// The progression: two strums a bar, each faded over 20 ms before the next; peak −6 dBFS.
fn progression(dir: &Path) -> Stereo {
    let beat = beat();
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
    (l.clone(), l)
}

/// A sung line (the formant voice on /a/, then /u/, then /i/): four phrases of three
/// notes in A minor with a little vibrato, each phrase followed by a rest — the gaps the
/// ducked space blooms into. Peak −6 dBFS.
fn sung_line() -> Stereo {
    let phrases: [[f32; 3]; 4] = [
        [440.0, 523.25, 587.33],
        [659.26, 587.33, 523.25],
        [493.88, 523.25, 440.0],
        [392.0, 440.0, 440.0],
    ];
    let vowels = [Vowel::A, Vowel::U, Vowel::I, Vowel::A];
    let note = 0.55f32;
    let gap = 1.6f32;
    let total = phrases.len() as f32 * (3.0 * note + gap);
    let n = (total * SR as f32) as usize;
    let mut x = vec![0.0f32; n];
    for (p, notes) in phrases.iter().enumerate() {
        let at = p as f32 * (3.0 * note + gap);
        for (k, f) in notes.iter().enumerate() {
            let f = *f;
            let v = testkit::voice(SR as f32, note, vowels[p], 0.5, move |t| {
                f * (1.0 + 0.006 * math::sin(std::f64::consts::TAU * 5.5 * t) as f32)
            });
            let start = ((at + k as f32 * note) * SR as f32) as usize;
            let len = v.len();
            let edge = (0.03 * SR as f32) as usize;
            for (i, s) in v.iter().enumerate() {
                let g = (i.min(len - 1 - i) as f32 / edge as f32).min(1.0);
                if start + i < n {
                    x[start + i] += s * g;
                }
            }
        }
    }
    scale_peak(&mut x, -6.0);
    (x.clone(), x)
}

/// A pad: Am, F, C, G as detuned saws (three a note, a slow attack and release), two bars
/// each at 90 BPM, a little wider on the right. Peak −6 dBFS.
fn pad() -> Stereo {
    let chords: [[f32; 3]; 4] = [
        [220.0, 261.63, 329.63],
        [174.61, 220.0, 261.63],
        [196.0, 261.63, 329.63],
        [196.0, 246.94, 293.66],
    ];
    let len = 8 * beat();
    let n = 4 * len;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for (c, notes) in chords.iter().enumerate() {
        for (k, f) in notes.iter().enumerate() {
            for (d, cents) in [-7.0f32, 0.0, 6.0].iter().enumerate() {
                let hz = f * math::powf(2f32, cents / 1200.0);
                let mut ph = (k * 3 + d) as f32 * 0.137;
                for i in 0..len {
                    let t = i as f32 / SR as f32;
                    let env = (t / 0.6).min(1.0) * ((len - i) as f32 / (0.5 * SR as f32)).min(1.0);
                    ph = (ph + hz / SR as f32).fract();
                    // A gentle saw: the naive one through a soft one-pole later.
                    let v = (2.0 * ph - 1.0) * env * 0.2;
                    let at = c * len + i;
                    l[at] += v * if d == 2 { 0.6 } else { 1.0 };
                    r[at] += v * if d == 0 { 0.6 } else { 1.0 };
                }
            }
        }
    }
    for x in [&mut l, &mut r] {
        let mut z = 0.0f32;
        for v in x.iter_mut() {
            z += 0.18 * (*v - z);
            *v = z;
        }
    }
    let peak = l.iter().chain(&r).fold(0f32, |m, v| m.max(v.abs()));
    let g = math::db_to_gain(-6.0) / peak;
    for v in l.iter_mut().chain(r.iter_mut()) {
        *v *= g;
    }
    (l, r)
}

fn scale_peak(x: &mut [f32], db: f32) {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let g = math::db_to_gain(db) / p;
    for v in x {
        *v *= g;
    }
}

fn rms(l: &[f32], r: &[f32]) -> f32 {
    let e: f64 = l.iter().chain(r).map(|v| (*v as f64) * (*v as f64)).sum();
    (e / (l.len() + r.len()).max(1) as f64).sqrt() as f32
}

/// Level-matched to `reference` (RMS over the reference's length), the peak kept under
/// −1 dBFS; the gain applied, dB.
fn matched(mut y: Stereo, reference: &Stereo) -> (Stereo, f32) {
    let m = reference.0.len().min(y.0.len());
    let mut g = rms(&reference.0, &reference.1) / rms(&y.0[..m], &y.1[..m]).max(1e-12);
    let peak = y.0.iter().chain(&y.1).fold(0f32, |m, v| m.max(v.abs())) * g;
    let ceiling = math::db_to_gain(-1.0);
    if peak > ceiling {
        g *= ceiling / peak;
    }
    for v in y.0.iter_mut().chain(y.1.iter_mut()) {
        *v *= g;
    }
    (y, 20.0 * math::log10(g))
}

/// How bright a file is over time: the share of its power above about 1.5 kHz (a
/// first-order high-pass), in 4 s windows, percent — a shimmer's climb shows here.
fn brightness(s: &Stereo) -> String {
    let k = 1.0 - math::exp(-std::f32::consts::TAU * 1_500.0 / SR as f32);
    let mut lp = 0.0f32;
    let win = 4 * SR as usize;
    let mut out = Vec::new();
    let (mut hi, mut all) = (0.0f64, 0.0f64);
    for (i, v) in s.0.iter().enumerate() {
        lp += k * (v - lp);
        let h = v - lp;
        hi += (h as f64) * (h as f64);
        all += (*v as f64) * (*v as f64);
        if (i + 1) % win == 0 {
            out.push(if all > 1e-12 {
                format!("{:.0}", 100.0 * hi / all)
            } else {
                "-".into()
            });
            hi = 0.0;
            all = 0.0;
        }
    }
    out.join(" ")
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

/// Through Caelum at `s` with `tail` seconds of silence after; `automate` may change the
/// settings at each 10 ms block (by the block's time, seconds).
fn through(
    x: &Stereo,
    s: CaelumSettings,
    tail: f32,
    automate: &dyn Fn(f64, &mut CaelumSettings),
) -> Stereo {
    let mut u = Caelum::new(SR as f32);
    u.set(&s);
    u.reset();
    let extra = (tail * SR as f32) as usize;
    let (mut l, mut r) = (x.0.clone(), x.1.clone());
    l.extend(std::iter::repeat_n(0.0, extra));
    r.extend(std::iter::repeat_n(0.0, extra));
    let block = 480;
    let mut cur = s;
    let mut at = 0;
    while at < l.len() {
        let n = block.min(l.len() - at);
        let mut next = cur;
        automate(at as f64 / SR as f64, &mut next);
        if next != cur {
            cur = next;
            u.set(&cur);
        }
        u.process(&mut l[at..at + n], &mut r[at..at + n], None);
        at += n;
    }
    (l, r)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: caelum_sounds <dir of the strums as WAV> <out dir>");
        std::process::exit(2);
    }
    let strums = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).expect("out dir");
    let mut log = Vec::new();
    let mut save = |name: &str, s: &Stereo, gain: Option<f32>| {
        write(&out.join(name), s);
        let peak = s.0.iter().chain(&s.1).fold(0f32, |m, v| m.max(v.abs()));
        let line = match gain {
            Some(g) => format!(
                "{name}: level-matched by {g:+.1} dB, peak {:.1} dBFS; share above 1.5 kHz every 4 s: {}",
                20.0 * math::log10(peak),
                brightness(s)
            ),
            None => format!("{name}: dry, as made, peak {:.1} dBFS", 20.0 * math::log10(peak)),
        };
        println!("{line}");
        log.push(line);
    };

    let guitar = progression(&strums);
    save("00-guitar-di.wav", &guitar, None);
    let bar = 4.0 * 60.0 / BPM;
    let none = |_: f64, _: &mut CaelumSettings| {};
    let (y, g) = matched(
        through(&guitar, presets::octave_shimmer(), 9.0, &none),
        &guitar,
    );
    save("01-guitar-octave-shimmer.wav", &y, Some(g));
    let (y, g) = matched(
        through(&guitar, presets::fifth_cathedral(), 12.0, &none),
        &guitar,
    );
    save("02-guitar-fifth-cathedral.wav", &y, Some(g));
    let (y, g) = matched(through(&guitar, presets::bloom_pad(), 10.0, &none), &guitar);
    save("03-guitar-bloom-pad.wav", &y, Some(g));
    // The freeze swell: infinite and blooming for four bars; Freeze held from the end of
    // bar 4 to the end of the song (the guitar plays bars 5 to 8 over the held chord);
    // then Freeze off and the decay finite (8 s), so it lets go.
    let end = BARS as f64 * bar;
    let swell = move |t: f64, s: &mut CaelumSettings| {
        s.freeze = t >= 4.0 * bar && t < end;
        if t >= end {
            s.infinite = false;
            s.decay_s = 8.0;
        }
    };
    let (y, g) = matched(
        through(&guitar, presets::freeze_swell(), 14.0, &swell),
        &guitar,
    );
    save("04-guitar-freeze-swell.wav", &y, Some(g));

    let voice = sung_line();
    save("00-voice-dry.wav", &voice, None);
    let (y, g) = matched(through(&voice, presets::ducked_vocal(), 5.0, &none), &voice);
    save("05-voice-ducked-space.wav", &y, Some(g));
    let open = CaelumSettings {
        duck_db: 0.0,
        ..presets::ducked_vocal()
    };
    let (y, g) = matched(through(&voice, open, 5.0, &none), &voice);
    save("05b-voice-same-space-no-ducking.wav", &y, Some(g));

    let pad = pad();
    save("00-pad-dry.wav", &pad, None);
    let (y, g) = matched(through(&pad, presets::octave_shimmer(), 9.0, &none), &pad);
    save("06-pad-octave-shimmer.wav", &y, Some(g));
    // Endless: infinite while the pad plays, then a finite 10 s so the file ends.
    let pad_end = pad.0.len() as f64 / SR as f64;
    let release = move |t: f64, s: &mut CaelumSettings| {
        if t >= pad_end {
            s.infinite = false;
            s.decay_s = 10.0;
        }
    };
    let (y, g) = matched(
        through(&pad, presets::endless_ascent(), 14.0, &release),
        &pad,
    );
    save("07-pad-endless-ascent.wav", &y, Some(g));
    let (y, g) = matched(through(&pad, presets::sub_cloud(), 10.0, &none), &pad);
    save("08-pad-sub-cloud.wav", &y, Some(g));
    // ADR 0141's amendment: Infinite thrown on the Octave shimmer's tail a quarter second
    // after the pad stops — nothing jumps, the wash holds where it was — held twelve
    // seconds at Sustain 100 % (09: its tone kept) and at 40 % (10: the climb carries it
    // away), then let go (finite, 6.5 s) so the file ends.
    for (name, sus) in [
        ("09-pad-infinite-thrown-sustain-100.wav", 1.0f32),
        ("10-pad-infinite-thrown-sustain-40.wav", 0.4),
    ] {
        let thrown = move |t: f64, s: &mut CaelumSettings| {
            s.infinite = t >= pad_end + 0.25 && t < pad_end + 12.25;
            s.sustain = sus;
        };
        let (y, g) = matched(
            through(&pad, presets::octave_shimmer(), 20.0, &thrown),
            &pad,
        );
        save(name, &y, Some(g));
    }
    let _ = log;
}
