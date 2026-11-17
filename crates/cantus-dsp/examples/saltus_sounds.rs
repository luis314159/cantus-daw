//! Saltus's listening examples (fx side round, wave 3; ADR 0132): the owner's
//! reference guitar progression — Am, F, C, G at 90 BPM, struck on beats 1 and 3, eight
//! bars, from Carbonthief's CC0 DI strums — through the poly engine (octaves, a fifth, a
//! doubler, the whammy swept), a synthesized bass line through the octaver, and a
//! synthesized sung line through the harmoniser and the mono shifter. Every set has its dry
//! twin; each processed file is level-matched to it (RMS), its peak kept under −1 dBFS.
//!
//! ```text
//! cargo run -p cantus-dsp --example saltus_sounds -- <dir of the strums as WAV> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. DSP only: Saltus's own unit, no engine.

use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_dsp::saltus::poly::{Plan, Poly, PolyVoice};
use cantus_dsp::saltus::{
    Engine, HarmonySource, Mode, NoteEvent, OctaveStyle, Saltus, SaltusSettings, VoiceSettings,
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
fn progression(dir: &Path) -> Stereo {
    let beat = (60.0 / BPM * SR as f64) as usize;
    let n = BARS * 4 * beat + SR as usize;
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

fn scale_peak(x: &mut [f32], db: f32) {
    let p = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let g = math::db_to_gain(db) / p;
    for v in x {
        *v *= g;
    }
}

fn rms(s: &Stereo) -> f32 {
    let e: f64 =
        s.0.iter()
            .chain(&s.1)
            .map(|v| (*v as f64) * (*v as f64))
            .sum();
    (e / (s.0.len() + s.1.len()) as f64).sqrt() as f32
}

/// Level-matched to `reference` (RMS), the peak kept under −1 dBFS; the gain applied, dB.
fn matched(mut y: Stereo, reference: &Stereo) -> (Stereo, f32) {
    let mut g = rms(reference) / rms(&y).max(1e-12);
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

/// Through Saltus at `s`, with `automate` setting the settings at each 10 ms block (by
/// the block's time in seconds) and `notes` (sample, event); the latency removed.
fn through(
    x: &Stereo,
    s: SaltusSettings,
    automate: &dyn Fn(f64, &mut SaltusSettings),
    notes: &[(usize, NoteEvent)],
) -> Stereo {
    let mut u = Saltus::new(SR as f32);
    u.set(&s);
    u.reset();
    let lat = u.latency();
    let (mut l, mut r) = (x.0.clone(), x.1.clone());
    l.extend(std::iter::repeat_n(0.0, lat));
    r.extend(std::iter::repeat_n(0.0, lat));
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
        for &(t, e) in notes {
            if t >= at && t < at + n {
                u.note(t - at, e);
            }
        }
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l[lat..].to_vec(), r[lat..].to_vec())
}

fn none(_: f64, _: &mut SaltusSettings) {}

/// The poly vocoder at another frame, for listening: voice 1 at `st` and `level` over the
/// dry (delayed to match).
fn poly_frame(x: &Stereo, n: usize, st: f32, level: f32) -> Stereo {
    let mut p = Poly::with_frame(SR as f32, n);
    let lat = p.latency();
    let plan = Plan {
        voices: [
            PolyVoice {
                shift: st,
                formant: math::exp2(st / 12.0),
                level,
                pan: 0.0,
            },
            PolyVoice::default(),
        ],
    };
    let len = x.0.len();
    let (mut l, mut r) = (Vec::with_capacity(len), Vec::with_capacity(len));
    for i in 0..len + lat {
        let (a, b) = if i < len {
            (x.0[i], x.1[i])
        } else {
            (0.0, 0.0)
        };
        let (wl, wr) = p.tick(a, b, &plan).get();
        if i >= lat {
            l.push(wl + x.0[i - lat]);
            r.push(wr + x.1[i - lat]);
        }
    }
    (l, r)
}

fn voice(semitones: f32, cents: f32, degree: i32, level: f32, pan: f32) -> VoiceSettings {
    VoiceSettings {
        semitones,
        cents,
        degree,
        level,
        pan,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let samples = PathBuf::from(args.get(1).expect("the strums' folder"));
    let out = PathBuf::from(args.get(2).expect("the output folder"));
    let readme = include_str!("saltus_sounds_readme.txt");
    let mut log = String::from(readme);
    let mut save = |dir: &Path, name: &str, s: &Stereo, what: &str, gain: Option<f32>| {
        std::fs::create_dir_all(dir).unwrap();
        write(&dir.join(format!("{name}.wav")), s);
        let g = gain
            .map(|g| format!(" (level-matched: {g:+.1} dB)"))
            .unwrap_or_default();
        log.push_str(&format!("- `{name}.wav`: {what}{g}\n"));
        eprintln!("{name}");
    };
    sets(&samples, &out, &mut save);
    std::fs::write(out.join("README.md"), log).unwrap();
}

type Save<'a> = dyn FnMut(&Path, &str, &Stereo, &str, Option<f32>) + 'a;

fn sets(samples: &Path, out: &Path, save: &mut Save) {
    let d = SaltusSettings::default();
    // ---- The guitar, through the poly engine ----
    let g = out.join("guitar");
    let di = progression(samples);
    save(
        &g,
        "00-guitar-di",
        &di,
        "the dry DI progression (peak −6 dBFS)",
        None,
    );
    let poly = |voices: [VoiceSettings; 2], dry: f32| {
        let mut s = d;
        s.engine = Engine::Poly;
        s.formants = false;
        s.voices = voices;
        s.dry = dry;
        s
    };
    let list: [(&str, SaltusSettings, &str); 5] = [
        ("01-octave-up-twelve-string", poly([voice(12.0, 0.0, 0, 0.6, 0.0), voice(0.0, 0.0, 0, 0.0, 0.0)], 1.0), "Shift, poly: an octave up at 60 % over the dry — the twelve-string"),
        ("02-octave-down", poly([voice(-12.0, 0.0, 0, 0.7, 0.0), voice(0.0, 0.0, 0, 0.0, 0.0)], 1.0), "Shift, poly: an octave down at 70 % over the dry"),
        ("03-fifth-up", poly([voice(7.0, 0.0, 0, 0.5, 0.0), voice(0.0, 0.0, 0, 0.0, 0.0)], 1.0), "Shift, poly: a fifth up at 50 % over the dry"),
        ("04-doubler", poly([voice(0.0, 9.0, 0, 0.7, -0.8), voice(0.0, -9.0, 0, 0.7, 0.8)], 1.0), "Shift, poly: two voices 9 cents up and down, spread left and right (the studio doubler)"),
        ("05-octaves-both", poly([voice(12.0, 0.0, 0, 0.45, -0.5), voice(-12.0, 0.0, 0, 0.6, 0.5)], 1.0), "Shift, poly: an octave up (left) and down (right) over the dry"),
    ];
    for (name, s, what) in list {
        let (y, gain) = matched(through(&di, s, &none, &[]), &di);
        save(&g, name, &y, what, Some(gain));
    }
    // The whammy on chords: heel 0, toe +12, the pedal rocked in bars 3–4 and 7–8.
    let bar = 4.0 * 60.0 / BPM;
    let rock = move |t: f64, s: &mut SaltusSettings| {
        let b = t / bar;
        let phase = b % 4.0;
        s.pedal = if phase >= 2.0 {
            let x = (phase - 2.0) / 2.0;
            (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * x)) as f32
        } else {
            0.0
        };
    };
    let mut wh = poly(
        [voice(0.0, 0.0, 0, 1.0, 0.0), voice(0.0, 0.0, 0, 0.0, 0.0)],
        0.0,
    );
    wh.mode = Mode::Whammy;
    wh.toe = 12.0;
    wh.glide_ms = 40.0;
    let (y, gain) = matched(through(&di, wh, &rock, &[]), &di);
    save(&g, "06-whammy-chords", &y, "Whammy, poly: heel unison, toe an octave up, the pedal rocked through bars 3–4 and 7–8 (no dry)", Some(gain));
    let mut dive = wh;
    dive.toe = -24.0;
    let (y, gain) = matched(through(&di, dive, &rock, &[]), &di);
    save(
        &g,
        "07-whammy-dive",
        &y,
        "Whammy, poly: the dive bomb — toe two octaves down, rocked as above",
        Some(gain),
    );
    // The frame, for the owner's ear: the same octave up at 8 192 points (170 ms).
    let (y, gain) = matched(poly_frame(&di, 8_192, 12.0, 0.6), &di);
    save(&g, "08-octave-up-frame-8192", &y, "as 01 with the vocoder's frame doubled (170 ms instead of 85: chords cleaner, strums softer at the attack) — for comparing with 01", Some(gain));
    bass(out, save);
    voices(out, save);
}

/// A plucked string (as the DSP tests' pluck): plucked at `pos`, with the pick's noise.
fn pluck(hz: f32, secs: f32, pos: f32, seed: u64) -> Vec<f32> {
    let n = (secs * SR as f32) as usize;
    let len = (SR as f32 / hz).round() as usize;
    let mut ns = testkit::Noise::new(seed);
    let peak_at = (len as f32 * pos).max(1.0);
    let mut line: Vec<f32> = (0..len)
        .map(|i| {
            let x = i as f32;
            let tri = if x < peak_at {
                x / peak_at
            } else {
                (len as f32 - x) / (len as f32 - peak_at)
            };
            tri - 0.5 + 0.05 * ns.next()
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    for i in 0..n {
        let a = line[p];
        let b = line[(p + 1) % len];
        line[p] = 0.5 * (a + b) * 0.997;
        let fade = ((n - i) as f32 / (0.01 * SR as f32)).min(1.0);
        out.push(a * fade);
        p = (p + 1) % len;
    }
    out
}

fn bass(out: &Path, save: &mut Save) {
    let b = out.join("bass");
    // A line in E: two bars of eighths at 100 BPM, twice.
    let notes = [
        40, 40, 52, 40, 43, 45, 47, 45, 40, 40, 52, 50, 47, 45, 43, 38,
    ];
    let eighth = 60.0 / 100.0 / 2.0;
    let mut x =
        vec![0.0f32; ((notes.len() * 2) as f64 * eighth * SR as f64) as usize + SR as usize];
    for rep in 0..2 {
        for (i, &m) in notes.iter().enumerate() {
            let at = (((rep * notes.len() + i) as f64) * eighth * SR as f64) as usize;
            let hz = cantus_dsp::pitch::midi_to_hz(m as f32);
            let p = pluck(hz, eighth as f32 * 0.95, 0.2, (rep * 31 + i) as u64);
            for (k, v) in p.iter().enumerate() {
                x[at + k] += v;
            }
        }
    }
    scale_peak(&mut x, -6.0);
    let dry = (x.clone(), x);
    save(
        &b,
        "00-bass-dry",
        &dry,
        "a synthesized bass line (plucked strings of our own, E1–E2): the dry twin",
        None,
    );
    let d = SaltusSettings::default();
    let mut oct = d;
    oct.mode = Mode::Octave;
    oct.dry = 1.0;
    type Row = (&'static str, (f32, f32, f32), OctaveStyle, &'static str);
    let list: [Row; 4] = [
        (
            "01-octave-down-analog",
            (0.8, 0.0, 0.0),
            OctaveStyle::Analog,
            "Octave, analog (the divider, no latency): one down at 80 % over the dry",
        ),
        (
            "02-two-down-analog",
            (0.6, 0.6, 0.0),
            OctaveStyle::Analog,
            "Octave, analog: one and two down over the dry",
        ),
        (
            "03-octave-up-analog",
            (0.0, 0.0, 0.7),
            OctaveStyle::Analog,
            "Octave, analog: the octave up (the fundamental rectified) over the dry",
        ),
        (
            "04-octave-down-clean",
            (0.8, 0.0, 0.0),
            OctaveStyle::Clean,
            "Octave, clean (PSOLA, 2.7 ms): one down at 80 % over the dry",
        ),
    ];
    for (name, (s1, s2, u1), style, what) in list {
        let mut s = oct;
        s.sub1 = s1;
        s.sub2 = s2;
        s.up1 = u1;
        s.octave_style = style;
        let (y, gain) = matched(through(&dry, s, &none, &[]), &dry);
        save(&b, name, &y, what, Some(gain));
    }
}

fn voices(out: &Path, save: &mut Save) {
    let v = out.join("voice");
    // A sung line (the pitch tests' synthetic voice: a glottal source through vowel
    // formants — not a singer): C major, a vibrato on the long notes.
    let melody = [
        (60, 0.5),
        (62, 0.5),
        (64, 1.0),
        (65, 0.5),
        (67, 0.5),
        (69, 1.0),
        (67, 0.5),
        (65, 0.5),
        (64, 1.0),
        (62, 0.5),
        (60, 1.5),
    ];
    let total: f32 = melody.iter().map(|m| m.1).sum();
    let f = move |t: f64| {
        let mut at = 0.0;
        for &(n, d) in &melody {
            if t < at + d as f64 {
                let vib = if d >= 1.0 {
                    25.0 * math::sin(2.0 * std::f64::consts::PI * 5.5 * t)
                } else {
                    0.0
                };
                return cantus_dsp::pitch::midi_to_hz(n as f32 + vib as f32 / 100.0);
            }
            at += d as f64;
        }
        0.0
    };
    let mut x = testkit::voice(SR as f32, total + 0.5, Vowel::A, 0.5, f);
    scale_peak(&mut x, -6.0);
    let dry = (x.clone(), x);
    save(
        &v,
        "00-voice-dry",
        &dry,
        "a synthesized sung line in C major (the pitch tests' voice): the dry twin",
        None,
    );
    let d = SaltusSettings::default();
    let mut h = d;
    h.mode = Mode::Harmony;
    h.dry = 1.0;
    h.key = 0;
    h.voices = [voice(0.0, 0.0, 2, 0.7, -0.3), voice(0.0, 0.0, 4, 0.0, 0.3)];
    let (y, gain) = matched(through(&dry, h, &none, &[]), &dry);
    save(&v, "01-harmony-third", &y, "Harmony in C major: a third above (two degrees) at 70 % over the dry — major or minor as the scale says", Some(gain));
    let mut h2 = h;
    h2.voices[1].level = 0.55;
    let (y, gain) = matched(through(&dry, h2, &none, &[]), &dry);
    save(
        &v,
        "02-harmony-third-and-fifth",
        &y,
        "Harmony in C major: a third and a fifth above, spread left and right",
        Some(gain),
    );
    let mut h3 = h;
    h3.voices[0] = voice(0.0, 0.0, -3, 0.7, 0.0);
    let (y, gain) = matched(through(&dry, h3, &none, &[]), &dry);
    save(
        &v,
        "03-harmony-fourth-below",
        &y,
        "Harmony in C major: three degrees below (a fourth down, diatonic)",
        Some(gain),
    );
    let mut chord = h;
    chord.harmony_source = HarmonySource::Chord;
    chord.voices[0] = voice(0.0, 0.0, 1, 0.7, 0.0);
    let notes = [
        (0usize, NoteEvent::On(48)),
        (0, NoteEvent::On(52)),
        (0, NoteEvent::On(55)),
        (SR as usize * 4, NoteEvent::AllOff),
        (SR as usize * 4, NoteEvent::On(53)),
        (SR as usize * 4, NoteEvent::On(57)),
        (SR as usize * 4, NoteEvent::On(60)),
    ];
    let (y, gain) = matched(through(&dry, chord, &none, &notes), &dry);
    save(&v, "04-harmony-chord-midi", &y, "Harmony from MIDI: C major held, then F major at 4 s; the voice the next chord note above the sung one", Some(gain));
    for (name, st, keep, what) in [
        (
            "05-shift-down-octave-formants-kept",
            -12.0,
            true,
            "Shift, mono: an octave down, formants kept (the same singer, lower)",
        ),
        (
            "06-shift-down-octave-formants-moved",
            -12.0,
            false,
            "Shift, mono: an octave down, formants moved with the pitch (the tape slowed)",
        ),
        (
            "07-shift-up-fifth",
            7.0,
            true,
            "Shift, mono: a fifth up, formants kept",
        ),
    ] {
        let mut s = d;
        s.voices[0].semitones = st;
        s.formants = keep;
        let (y, gain) = matched(through(&dry, s, &none, &[]), &dry);
        save(&v, name, &y, what, Some(gain));
    }
    let mut wh = d;
    wh.mode = Mode::Whammy;
    wh.toe = 12.0;
    let sweep = |t: f64, s: &mut SaltusSettings| {
        s.pedal = (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * t / 4.0)) as f32;
    };
    let (y, gain) = matched(through(&dry, wh, &sweep, &[]), &dry);
    save(
        &v,
        "08-whammy-sweep",
        &y,
        "Whammy, mono: the pedal swept slowly heel to toe (an octave) and back every 4 s",
        Some(gain),
    );
}
