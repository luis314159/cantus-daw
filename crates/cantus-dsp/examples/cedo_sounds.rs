//! Cedo's listening examples (fx side round, wave B; DSP only — Cedo's own unit, no engine).
//!
//! - **Kick and pad / kick and bass** at 124 BPM, eight bars, the kick from Pulsus (model 2)
//!   on every beat: the pad ducked by the kick through the sidechain, by the tempo pump (no
//!   audio needed), the pump swung on eighths; the bass ducked in its lows only (minimum and
//!   linear phase), beside the whole bass ducked.
//! - **Guitar under a voice**: the owner's reference progression (Am, F, C, G at 90 BPM,
//!   two strums a bar, eight bars, Carbonthief's CC0 DI strums) under a synthetic spoken
//!   voice (a glottal pulse through three formants, phrases with pauses), the guitar ducked
//!   by the voice — whole, and only above 400 Hz.
//!
//! Every mix has its dry twin. Each set is scaled by one factor (the set's loudest file to
//! −1 dBFS), so a ducked file and its twin keep their real levels.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example cedo_sounds -- <dir of the strums as WAV> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo.

use cantus_dsp::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
use cantus_dsp::cedo::band::{Band, Crossover};
use cantus_dsp::cedo::env::{Detector, KeyFilterMode, ReleaseShape};
use cantus_dsp::cedo::testkit::shape_fn;
use cantus_dsp::cedo::{Cedo, CedoSettings, Trigger, CLASSIC_PUMP};
use cantus_dsp::math;
use cantus_dsp::modfx_clock::Transport;
use cantus_dsp::pulsus::{PadSettings, Pulsus};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const SR: u32 = 48_000;
const SRF: f32 = 48_000.0;

type Mono = Vec<f32>;

fn write(dir: &Path, name: &str, l: &[f32], r: &[f32], gain: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let path: PathBuf = dir.join(name);
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let k = (1 << 23) as f32 - 1.0;
    for (a, b) in l.iter().zip(r) {
        w.write_sample(((a * gain).clamp(-1.0, 1.0) * k) as i32)
            .unwrap();
        w.write_sample(((b * gain).clamp(-1.0, 1.0) * k) as i32)
            .unwrap();
    }
    w.finalize().unwrap();
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

fn read_mono(path: &Path) -> (Mono, u32) {
    let mut r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let spec = r.spec();
    let ch = spec.channels as usize;
    let x: Mono = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let k = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|v| v.unwrap() as f32 * k).collect()
        }
    };
    (x.chunks(ch).map(|c| c[0]).collect(), spec.sample_rate)
}

/// A windowed-sinc resampler (64 taps, Blackman).
fn resample(x: &[f32], from: u32, to: u32) -> Mono {
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
                let pi = std::f64::consts::PI;
                let sinc = if d.abs() < 1e-9 {
                    cut
                } else {
                    math::sin(pi * d * cut) / (pi * d)
                };
                let w = d / half as f64;
                let win = if w.abs() >= 1.0 {
                    0.0
                } else {
                    let p = pi * (w + 1.0);
                    0.42 - 0.5 * math::cos(p) + 0.08 * math::cos(2.0 * p)
                };
                acc += x[k as usize] as f64 * sinc * win;
            }
            acc as f32
        })
        .collect()
}

/// Pulsus's kick (model 2) on every beat.
fn kicks(n: usize, beat: f64) -> Mono {
    let mut p = Pulsus::new(SRF);
    p.set_kit(2, 1, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 36, &PadSettings::kick());
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut at = 0;
    let mut k = 0u64;
    loop {
        let h = (k as f64 * beat).round() as usize;
        if h >= n {
            break;
        }
        if h > at {
            p.process_add(&mut l[at..h], &mut r[at..h]);
            at = h;
        }
        p.note_on(36, 118, k + 1);
        k += 1;
    }
    p.process_add(&mut l[at..], &mut r[at..]);
    l
}

/// A pad on Am – F – C – G, a bar each: detuned saws, gently low-passed, stereo.
fn pad(n: usize, bar: usize) -> (Mono, Mono) {
    let chords: [[f32; 4]; 4] = [
        [220.0, 261.63, 329.63, 440.0],
        [174.61, 220.0, 261.63, 349.23],
        [196.0, 261.63, 329.63, 392.0],
        [196.0, 246.94, 293.66, 392.0],
    ];
    let mut ph = [[0.0f32; 4]; 4];
    let (mut lp_l, mut lp_r) = (0.0f32, 0.0f32);
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    for i in 0..n {
        let c = &chords[(i / bar) % 4];
        let (mut a, mut b) = (0.0f32, 0.0f32);
        for (j, f) in c.iter().enumerate() {
            for (d, det) in [0.996f32, 1.0, 1.004, 1.007].iter().enumerate() {
                let p = &mut ph[j][d];
                *p += f * det / SRF;
                *p -= p.floor();
                let s = 2.0 * *p - 1.0;
                if d % 2 == 0 {
                    a += s;
                } else {
                    b += s;
                }
            }
        }
        lp_l += 0.05 * (a * 0.06 - lp_l);
        lp_r += 0.05 * (b * 0.06 - lp_r);
        l[i] = lp_l;
        r[i] = lp_r;
    }
    (l, r)
}

/// A bass on the roots, eighth notes, a saw through a moving low-pass.
fn bass(n: usize, beat: f64, bar: usize) -> Mono {
    let roots = [55.0f32, 43.65, 65.41, 49.0];
    let eighth = beat / 2.0;
    let mut ph = 0.0f32;
    let mut lp = [0.0f32; 2];
    (0..n)
        .map(|i| {
            let f = roots[(i / bar) % 4];
            ph += f / SRF;
            ph -= ph.floor();
            let t = (i as f64 % eighth) / eighth;
            let env = (1.0 - t as f32 * 0.6)
                * if t > 0.92 {
                    ((1.0 - t) / 0.08) as f32
                } else {
                    1.0
                };
            let cut = 0.02 + 0.06 * (1.0 - t as f32);
            let s = 2.0 * ph - 1.0;
            lp[0] += cut * (s - lp[0]);
            lp[1] += cut * (lp[0] - lp[1]);
            0.5 * env * lp[1]
        })
        .collect()
}

/// One biquad cascade, f64.
struct Bq {
    n: usize,
    c: [Coefs; MAX_SECTIONS],
    z: [[f64; 2]; MAX_SECTIONS],
}

impl Bq {
    fn new(shape: EqShape, hz: f32, q: f32, slope: f32) -> Bq {
        let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
        let n = design(shape, SRF, hz, 0.0, q, slope, &mut c);
        Bq {
            n,
            c,
            z: [[0.0; 2]; MAX_SECTIONS],
        }
    }
    fn set(&mut self, shape: EqShape, hz: f32, q: f32, slope: f32) {
        self.n = design(shape, SRF, hz, 0.0, q, slope, &mut self.c);
    }
    fn tick(&mut self, x: f32) -> f32 {
        let mut y = x as f64;
        for (c, z) in self.c[..self.n].iter().zip(self.z.iter_mut()) {
            let o = c.b0 as f64 * y + z[0];
            z[0] = c.b1 as f64 * y - c.a1 as f64 * o + z[1];
            z[1] = c.b2 as f64 * y - c.a2 as f64 * o;
            y = o;
        }
        y as f32
    }
}

fn hash(i: u64) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

/// A synthetic spoken voice: phrases of syllables with pauses — a glottal pulse with a
/// falling pitch through three formants of a vowel each, a breath of noise at each onset.
fn voice(n: usize) -> Mono {
    // (F1, F2, F3) of a, e, i, o, u.
    let vowels = [
        (730.0f32, 1_090.0f32, 2_440.0f32),
        (530.0, 1_840.0, 2_480.0),
        (270.0, 2_290.0, 3_010.0),
        (570.0, 840.0, 2_410.0),
        (300.0, 870.0, 2_240.0),
    ];
    let mut out = vec![0.0f32; n];
    let mut f = [
        Bq::new(EqShape::BandPass, 700.0, 6.0, 12.0),
        Bq::new(EqShape::BandPass, 1_200.0, 9.0, 12.0),
        Bq::new(EqShape::BandPass, 2_500.0, 12.0, 12.0),
    ];
    let mut hiss = Bq::new(EqShape::BandPass, 4_500.0, 1.5, 12.0);
    let mut ph = 0.0f32;
    // Phrases: start (s), syllables.
    let phrases = [(1.2f32, 9usize), (4.6, 7), (8.4, 10), (12.6, 6), (15.6, 9)];
    let syl = (0.19 * SRF) as usize;
    for (pi, &(start, count)) in phrases.iter().enumerate() {
        let s0 = (start * SRF) as usize;
        for k in 0..count {
            let v = vowels[(pi * 3 + k * 2) % 5];
            for (j, (fi, hz)) in f.iter_mut().zip([v.0, v.1, v.2]).enumerate() {
                fi.set(EqShape::BandPass, hz, [6.0, 9.0, 12.0][j], 12.0);
            }
            let at = s0 + k * syl + (hash((pi * 31 + k) as u64).abs() * 0.03 * SRF) as usize;
            let len = (syl as f32 * (0.8 + 0.2 * hash((pi * 17 + k) as u64).abs())) as usize;
            for i in 0..len {
                let idx = at + i;
                if idx >= n {
                    break;
                }
                let t = i as f32 / len as f32;
                let env = math::powf(math::sin(std::f32::consts::PI * t.min(1.0)).max(0.0), 0.6);
                let progress = (k as f32 + t) / count as f32;
                let f0 = 135.0 - 30.0 * progress + 6.0 * math::sin(7.0 * progress);
                ph += f0 / SRF;
                ph -= ph.floor();
                // A glottal pulse: a skewed saw with a soft corner.
                let g = if ph < 0.6 { ph / 0.6 } else { (1.0 - ph) / 0.4 } - 0.5;
                let src = g * env;
                let mut y = 0.0f32;
                for (j, fi) in f.iter_mut().enumerate() {
                    y += fi.tick(src) * [1.0, 0.6, 0.3][j];
                }
                let breath = if i < (0.035 * SRF) as usize {
                    hiss.tick(hash(idx as u64 + 99) * 0.25)
                } else {
                    hiss.tick(0.0)
                };
                out[idx] += 1.4 * y + breath;
            }
        }
    }
    out
}

/// The reference progression: Am, F, C, G at 90 BPM, two strums a bar, eight bars.
fn progression(dir: &Path) -> Mono {
    let names = [
        "Sample07 (A Minor)",
        "Sample15 (F Major)",
        "Sample22 (C Major)",
        "Sample17 (G Major)",
    ];
    let beat = (60.0 / 90.0 * SRF as f64) as usize;
    let n = 8 * 4 * beat;
    let mut l = vec![0.0f32; n];
    let fade = (0.02 * SRF) as usize;
    let strums: Vec<Mono> = names
        .iter()
        .map(|c| {
            let (x, rate) = read_mono(&dir.join(format!("{c}.wav")));
            resample(&x, rate, SR)
        })
        .collect();
    for bar in 0..8 {
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
    let p = peak(&l);
    let k = math::db_to_gain(-6.0) / p;
    l.iter_mut().for_each(|v| *v *= k);
    l
}

/// Runs Cedo over a stereo signal (the transport playing at `bpm` from the start) in blocks
/// of 256; the latency taken off so it lines up with its dry twin. Returns it and the most
/// reduction seen, dB.
fn duck(
    s: &CedoSettings,
    shape: &[cantus_dsp::fluctus::CurvePoint],
    l: &[f32],
    r: &[f32],
    key: Option<&[f32]>,
    bpm: f64,
) -> (Mono, Mono, f32, usize) {
    let mut c = Cedo::new(SRF, shape_fn);
    c.set_shape(shape);
    c.set(s);
    c.reset();
    let lat = c.latency();
    let n = l.len();
    let mut a: Mono = l
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, lat))
        .collect();
    let mut b: Mono = r
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0.0, lat))
        .collect();
    let k: Option<Mono> = key.map(|k| {
        k.iter()
            .copied()
            .chain(std::iter::repeat_n(0.0, lat))
            .collect()
    });
    let mut gr = 0.0f32;
    let mut i = 0;
    while i < a.len() {
        let end = (i + 256).min(a.len());
        c.set_transport(&Transport {
            playing: true,
            tempo_bpm: bpm,
            samples: i as i64,
        });
        let kk = k.as_ref().map(|k| (&k[i..end], &k[i..end]));
        c.process(&mut a[i..end], &mut b[i..end], kk);
        gr = gr.max(c.meter().gr_db);
        i = end;
    }
    (a[lat..lat + n].to_vec(), b[lat..lat + n].to_vec(), gr, lat)
}

fn add(a: &[f32], b: &[f32]) -> Mono {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

fn rms_db(x: &[f32]) -> f32 {
    let m = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64;
    (10.0 * math::log10(m.max(1e-30))) as f32
}

struct Set {
    dir: PathBuf,
    files: Vec<(String, Mono, Mono)>,
    log: String,
}

impl Set {
    fn put(&mut self, name: &str, l: Mono, r: Mono) {
        let _ = writeln!(
            self.log,
            "{name}: peak {:.1} dBFS, RMS {:.1} dB (before the set's scaling)",
            20.0 * math::log10(peak(&l).max(peak(&r)).max(1e-9)),
            rms_db(&l)
        );
        self.files.push((name.to_string(), l, r));
    }
    /// Scale the whole set by one factor (its loudest file to −1 dBFS) and write it.
    fn write(self) -> String {
        let p = self
            .files
            .iter()
            .map(|(_, l, r)| peak(l).max(peak(r)))
            .fold(0.0f32, f32::max);
        let g = math::db_to_gain(-1.0) / p.max(1e-9);
        for (name, l, r) in &self.files {
            write(&self.dir, name, l, r, g);
        }
        format!(
            "{}  (the set scaled by {:+.2} dB)
",
            self.log,
            20.0 * math::log10(g)
        )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() == 3,
        "usage: cedo_sounds <dir of the strums as WAV> <out dir>"
    );
    let strums = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).unwrap();
    let mut log = String::from(
        "Cedo's listening examples — render log

",
    );

    // Kick and pad, kick and bass: 124 BPM, eight bars.
    let bpm = 124.0;
    let beat = 60.0 / bpm * SRF as f64;
    let bar = (4.0 * beat).round() as usize;
    let n = 8 * bar + SR as usize;
    let kick = kicks(n, beat);
    let (pl, pr) = pad(n, bar);
    let mut a = Set {
        dir: out.clone(),
        files: Vec::new(),
        log: String::new(),
    };
    let sc = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 12.0,
        threshold_db: -24.0,
        attack_ms: 2.0,
        hold_ms: 35.0,
        release_ms: 190.0,
        release_shape: ReleaseShape::Exponential,
        key_filter: KeyFilterMode::BandPass,
        key_hz: 60.0,
        key_q: 0.8,
        lookahead_ms: 2.0,
        ..CedoSettings::default()
    };
    let (dl, dr, gr, lat) = duck(&sc, &[], &pl, &pr, Some(&kick), bpm);
    let _ = writeln!(log, "a2 sidechain: depth 12 dB, threshold −24 dB, band-pass key at 60 Hz, attack 2 / hold 35 / release 190 ms exponential, lookahead 2 ms; latency {lat} samples (taken off); most reduction {gr:.1} dB");
    a.put("a1-pad-and-kick-dry.wav", add(&pl, &kick), add(&pr, &kick));
    a.put(
        "a2-pad-ducked-by-the-kick-sidechain.wav",
        add(&dl, &kick),
        add(&dr, &kick),
    );
    let pump = CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: 15.0,
        edge_ms: 1.0,
        ..CedoSettings::default()
    };
    let (tl, tr, gr, _) = duck(&pump, &CLASSIC_PUMP, &pl, &pr, None, bpm);
    let _ = writeln!(log, "a3/a4 tempo pump: the classic shape on 1/4, depth 15 dB, edge 1 ms; most reduction {gr:.1} dB");
    a.put(
        "a3-pad-pumped-by-the-tempo-and-kick.wav",
        add(&tl, &kick),
        add(&tr, &kick),
    );
    a.put("a4-pad-pumped-by-the-tempo-alone.wav", tl, tr);
    a.put("a5-pad-dry-alone.wav", pl.clone(), pr.clone());
    let swung = CedoSettings {
        division: 11,
        swing: 0.5,
        depth_db: 12.0,
        ..pump
    };
    let (sl, sr, _, _) = duck(&swung, &CLASSIC_PUMP, &pl, &pr, None, bpm);
    let _ = writeln!(
        log,
        "a6 tempo pump on 1/8, swing 50 % (the off-beat at 62.5 % of the pair), depth 12 dB"
    );
    a.put(
        "a6-pad-pumped-on-swung-eighths-and-kick.wav",
        add(&sl, &kick),
        add(&sr, &kick),
    );
    log += &a.write();

    let b_line = bass(n, beat, bar);
    let mut b = Set {
        dir: out.clone(),
        files: Vec::new(),
        log: String::new(),
    };
    let bsc = CedoSettings {
        depth_db: 18.0,
        release_ms: 140.0,
        ..sc
    };
    b.put(
        "b1-bass-and-kick-dry.wav",
        add(&b_line, &kick),
        add(&b_line, &kick),
    );
    let (wl, _, _, _) = duck(&bsc, &[], &b_line, &b_line, Some(&kick), bpm);
    b.put("b2-bass-ducked-whole.wav", add(&wl, &kick), add(&wl, &kick));
    for (name, cross) in [
        (
            "b3-bass-ducked-lows-only-minimum-phase.wav",
            Crossover::MinimumPhase,
        ),
        (
            "b4-bass-ducked-lows-only-linear-phase.wav",
            Crossover::LinearPhase,
        ),
    ] {
        let s = CedoSettings {
            band: Band::Lows,
            crossover: cross,
            cross_hz: 110.0,
            ..bsc
        };
        let (ll, _, gr, lat) = duck(&s, &[], &b_line, &b_line, Some(&kick), bpm);
        let _ = writeln!(log, "{name}: lows under 110 Hz, depth 18 dB, latency {lat} samples (taken off); most reduction {gr:.1} dB");
        b.put(name, add(&ll, &kick), add(&ll, &kick));
    }
    log += &b.write();

    // The guitar under a voice: 90 BPM, eight bars.
    let gtr = progression(&strums);
    let n = gtr.len();
    let vo = voice(n);
    let vp = peak(&vo);
    let vo: Mono = vo.iter().map(|v| v * math::db_to_gain(-4.0) / vp).collect();
    let mut c = Set {
        dir: out.clone(),
        files: Vec::new(),
        log: String::new(),
    };
    c.put("00-guitar-di.wav", gtr.clone(), gtr.clone());
    c.put("c1-voice-dry.wav", vo.clone(), vo.clone());
    c.put(
        "c2-guitar-and-voice-dry.wav",
        add(&gtr, &vo),
        add(&gtr, &vo),
    );
    let vsc = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 10.0,
        threshold_db: -30.0,
        attack_ms: 15.0,
        hold_ms: 300.0,
        release_ms: 450.0,
        release_shape: ReleaseShape::SCurve,
        key_filter: KeyFilterMode::HighPass,
        key_hz: 150.0,
        lookahead_ms: 10.0,
        ..CedoSettings::default()
    };
    let (gl, _, gr, lat) = duck(&vsc, &[], &gtr, &gtr, Some(&vo), 90.0);
    let _ = writeln!(log, "c3 voice-over: depth 10 dB, threshold −30 dB, high-passed key at 150 Hz, attack 15 / hold 300 / release 450 ms S-curve, lookahead 10 ms; latency {lat} (taken off); most reduction {gr:.1} dB");
    c.put(
        "c3-guitar-ducked-by-the-voice.wav",
        add(&gl, &vo),
        add(&gl, &vo),
    );
    c.put("c5-guitar-ducked-by-the-voice-alone.wav", gl.clone(), gl);
    let mids = CedoSettings {
        band: Band::Highs,
        cross_hz: 400.0,
        depth_db: 14.0,
        ..vsc
    };
    let (ml, _, gr, _) = duck(&mids, &[], &gtr, &gtr, Some(&vo), 90.0);
    let _ = writeln!(
        log,
        "c4 voice-over above 400 Hz (minimum phase), depth 14 dB; most reduction {gr:.1} dB"
    );
    c.put(
        "c4-guitar-ducked-above-400-hz-by-the-voice.wav",
        add(&ml, &vo),
        add(&ml, &vo),
    );
    let rms = CedoSettings {
        detector: Detector::Rms,
        rms_ms: 30.0,
        threshold_db: -36.0,
        hold_ms: 200.0,
        ..vsc
    };
    let (rl, _, gr, _) = duck(&rms, &[], &gtr, &gtr, Some(&vo), 90.0);
    let _ = writeln!(
        log,
        "c6 voice-over with the RMS detector (30 ms, threshold −36 dB, hold 200 ms), otherwise as c3; most reduction {gr:.1} dB"
    );
    c.put(
        "c6-guitar-ducked-by-the-voice-rms-detector.wav",
        add(&rl, &vo),
        add(&rl, &vo),
    );
    log += &c.write();
    std::fs::write(out.join("render-log.txt"), log).unwrap();
    println!("written to {}", out.display());
}
