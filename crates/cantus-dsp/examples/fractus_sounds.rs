//! Fractus's listening examples — the DSP alone, no engine: a drum loop played by Pulsus
//! (Cantus's own drum instrument, model 2), the owner's guitar progression (Carbonthief's
//! CC0 DI strums: Am – F – C – G at 90 BPM, struck on beats 1 and 3, eight bars) and a
//! synthesized pad (detuned band-limited saws through a low-pass, the same chords), each
//! through a Fractus pattern on the song's clock, written beside its dry twin. Then the
//! cloud's cost at high density, timed.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example fractus_sounds -- <guitar wav dir> <out dir>
//! ```
//!
//! 48 kHz, 24-bit WAV, stereo. Every dry twin peaks at −6 dBFS; each processed file is
//! matched to its twin's RMS (within 0.1 dB), its peak kept under −1 dBFS (said in the
//! README written beside them when that lowers it).

use cantus_dsp::fractus::{
    CloudSettings, FilterKind, FilterSettings, Fractus, FractusSettings, GateSettings, GrainWindow,
    Module, Note, Pattern, PitchSettings, RepeatSettings, ReverseSettings, Scale, ShuffleSettings,
    TapeKind, TapeSettings, ThrowSettings, Trigger,
};
use cantus_dsp::math;
use cantus_dsp::modfx_clock::Transport;
use cantus_dsp::pulsus::{Hit, PadSettings, Pulsus};
use std::path::Path;
use std::time::Instant;

const SR: f32 = 48_000.0;

type Stereo = (Vec<f32>, Vec<f32>);

/// Samples in a beat at `bpm`.
fn beat(bpm: f64) -> f64 {
    60.0 / bpm * f64::from(SR)
}

fn peak(l: &[f32], r: &[f32]) -> f32 {
    l.iter()
        .chain(r)
        .fold(0.0f32, |m, v| m.max(v.abs()))
        .max(1e-9)
}

fn rms(l: &[f32], r: &[f32]) -> f64 {
    let s: f64 = l
        .iter()
        .chain(r)
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum();
    (s / (l.len() + r.len()) as f64).sqrt().max(1e-12)
}

fn scale(x: &mut Stereo, g: f32) {
    x.0.iter_mut().chain(x.1.iter_mut()).for_each(|v| *v *= g);
}

/// A dry twin at −6 dBFS peak.
fn normalise(mut x: Stereo) -> Stereo {
    let g = math::db_to_gain(-6.0) / peak(&x.0, &x.1);
    scale(&mut x, g);
    x
}

/// Pulsus: a two-bar loop of kick, snare (with ghosts), closed and open hats, played
/// `bars` bars at `bpm`.
fn drums(bpm: f64, bars: usize) -> Stereo {
    let mut p = Pulsus::new(SR);
    p.set_kit(2, 11, 4, false);
    p.set_pad_count(4);
    p.set_pad(0, 36, &PadSettings::kick());
    p.set_pad(1, 38, &PadSettings::snare());
    p.set_pad(
        2,
        42,
        &PadSettings {
            pan: -0.2,
            ..PadSettings::hat(0.05)
        },
    );
    p.set_pad(
        3,
        46,
        &PadSettings {
            pan: 0.2,
            ..PadSettings::hat(0.35)
        },
    );
    let six = beat(bpm) / 4.0;
    // (sixteenth in the two-bar loop, pitch, velocity)
    let mut loop2: Vec<(usize, u8, u8)> = vec![];
    for k in [0usize, 6, 10, 16, 22, 26, 29] {
        loop2.push((k, 36, 120));
    }
    for (k, v) in [
        (4usize, 115u8),
        (12, 118),
        (20, 115),
        (28, 120),
        (15, 38),
        (31, 52),
    ] {
        loop2.push((k, 38, v));
    }
    for k in (0..32).step_by(2) {
        if k != 14 && k != 30 {
            loop2.push((k, 42, if k % 4 == 0 { 92 } else { 64 }));
        }
    }
    for k in [13usize, 27] {
        loop2.push((k, 42, 50));
    }
    for k in [14usize, 30] {
        loop2.push((k, 46, 80));
    }
    let mut hits: Vec<(usize, u8, u8)> = vec![];
    for rep in 0..bars.div_ceil(2) {
        for &(k, pitch, v) in &loop2 {
            let at = ((rep * 32 + k) as f64 * six).round() as usize;
            hits.push((at, pitch, v));
        }
    }
    hits.sort();
    let n = (bars as f64 * 4.0 * beat(bpm)) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut at = 0usize;
    for (i, &(when, pitch, v)) in hits.iter().enumerate() {
        let when = when.min(n);
        if when > at {
            p.process_add(&mut l[at..when], &mut r[at..when]);
            at = when;
        }
        let hit = Hit {
            id: i as u64,
            tick: when as i64,
            ordinal: 0,
            sequenced: true,
        };
        p.trigger(pitch, v, hit);
    }
    p.process_add(&mut l[at..], &mut r[at..]);
    normalise((l, r))
}

/// A windowed-sinc resampler (offline, f64, 64 taps, Kaiser β 9), stereo interleaved in,
/// two channels out.
fn resample(x: &[f32], ch: usize, from: u32, to: u32) -> Stereo {
    let frames = x.len() / ch;
    let ratio = f64::from(from) / f64::from(to);
    let cut = ratio.max(1.0).recip() * 0.95;
    let out_n = (frames as f64 / ratio) as usize;
    let at = |i: i64, c: usize| -> f64 {
        if i < 0 || i as usize >= frames {
            0.0
        } else {
            f64::from(x[i as usize * ch + c.min(ch - 1)])
        }
    };
    let i0 = |b: f64| {
        let (mut s, mut t) = (1.0, 1.0);
        for k in 1..40 {
            t *= (b / 2.0) * (b / 2.0) / (k as f64 * k as f64);
            s += t;
        }
        s
    };
    let beta = 9.0;
    let ib = i0(beta);
    let (mut l, mut r) = (vec![0.0f32; out_n], vec![0.0f32; out_n]);
    for o in 0..out_n {
        let p = o as f64 * ratio;
        let base = p.floor() as i64;
        let (mut a, mut b) = (0.0, 0.0);
        for k in -31..=32i64 {
            let t = p - (base + k) as f64;
            let u = t / 32.0;
            if u.abs() >= 1.0 {
                continue;
            }
            let w = i0(beta * (1.0 - u * u).sqrt()) / ib;
            let arg = std::f64::consts::PI * cut * t;
            let s = if arg == 0.0 {
                1.0
            } else {
                math::sin(arg) / arg
            };
            let c = cut * s * w;
            a += c * at(base + k, 0);
            b += c * at(base + k, 1);
        }
        l[o] = a as f32;
        r[o] = b as f32;
    }
    (l, r)
}

/// One strum: a 32-bit float WAV read with hound, resampled to 48 kHz.
fn strum(dir: &Path, stem: &str) -> Stereo {
    let mut rd = hound::WavReader::open(dir.join(format!("{stem}.wav"))).expect("a strum");
    let spec = rd.spec();
    let x: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let m = (1i64 << (spec.bits_per_sample - 1)) as f32;
            rd.samples::<i32>().map(|s| s.unwrap() as f32 / m).collect()
        }
    };
    let ch = usize::from(spec.channels);
    if spec.sample_rate == SR as u32 {
        let l: Vec<f32> = x.iter().step_by(ch).copied().collect();
        let r: Vec<f32> = x.iter().skip(ch - 1).step_by(ch).copied().collect();
        (l, r)
    } else {
        resample(&x, ch, spec.sample_rate, SR as u32)
    }
}

/// The owner's progression: two strums a bar, each faded over 20 ms before the next.
fn guitar(dir: &Path) -> Stereo {
    let chords = [
        "Sample07 (A Minor)",
        "Sample15 (F Major)",
        "Sample22 (C Major)",
        "Sample17 (G Major)",
    ];
    let b = beat(90.0) as usize;
    let n = 8 * 4 * b + SR as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let fade = (0.02 * f64::from(SR)) as usize;
    let strums: Vec<Stereo> = chords.iter().map(|c| strum(dir, c)).collect();
    for bar in 0..8 {
        for half in 0..2 {
            let at = (bar * 4 + half * 2) * b;
            let len = 2 * b;
            let s = &strums[bar % 4];
            for i in 0..len.min(s.0.len()) {
                let g = if i + fade >= len {
                    (len - i) as f32 / fade as f32
                } else {
                    1.0
                };
                l[at + i] += s.0[i] * g;
                r[at + i] += s.1[i] * g;
            }
        }
    }
    normalise((l, r))
}

/// A band-limited saw (polyBLEP) at phase `p` with increment `dt`.
fn saw(p: f64, dt: f64) -> f64 {
    let blep = if p < dt {
        let t = p / dt;
        t + t - t * t - 1.0
    } else if p > 1.0 - dt {
        let t = (p - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    };
    2.0 * p - 1.0 - blep
}

/// A pad: the progression's chords (one a bar at 90 BPM, eight bars), five notes each,
/// three saws a note detuned ±7 cents and spread across the field, a slow attack and a
/// long release, through a two-pole low-pass at 2.4 kHz.
fn pad() -> Stereo {
    let chords: [[u8; 5]; 4] = [
        [45, 52, 57, 60, 64], // Am
        [41, 48, 53, 57, 60], // F
        [48, 55, 60, 64, 67], // C
        [43, 50, 55, 59, 62], // G
    ];
    let bar = (4.0 * beat(90.0)) as usize;
    let n = 8 * bar + SR as usize;
    let (mut l, mut r) = (vec![0.0f64; n], vec![0.0f64; n]);
    let (att, rel) = (0.35 * f64::from(SR), 0.7 * f64::from(SR));
    for b in 0..8 {
        for (j, &note) in chords[b % 4].iter().enumerate() {
            let f0 = 440.0 * math::exp2((f64::from(note) - 69.0) / 12.0);
            for (k, cents) in [-7.0f64, 0.0, 7.0].iter().enumerate() {
                let f = f0 * math::exp2(cents / 1_200.0);
                let dt = f / f64::from(SR);
                let pan = (k as f64 - 1.0) * 0.6 + (j as f64 - 2.0) * 0.1;
                let (gl, gr) = ((1.0 - pan) * 0.5, (1.0 + pan) * 0.5);
                let mut p = ((note as usize * 7 + k * 13) % 97) as f64 / 97.0;
                let start = b * bar;
                let len = bar + rel as usize;
                for i in 0..len.min(n - start) {
                    let t = i as f64;
                    let env = (t / att).min(1.0)
                        * if i > bar {
                            1.0 - (t - bar as f64) / rel
                        } else {
                            1.0
                        };
                    let v = 0.03 * env * saw(p, dt);
                    l[start + i] += v * gl;
                    r[start + i] += v * gr;
                    p += dt;
                    if p >= 1.0 {
                        p -= 1.0;
                    }
                }
            }
        }
    }
    // The low-pass (RBJ, Q 0.7).
    let w = std::f64::consts::TAU * 2_400.0 / f64::from(SR);
    let (s, c) = math::sin_cos(w);
    let a = s / (2.0 * 0.7);
    let (b0, b1, b2) = ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0);
    let (a0, a1, a2) = (1.0 + a, -2.0 * c, 1.0 - a);
    let mut out = (vec![0.0f32; n], vec![0.0f32; n]);
    for (x, y) in [(&l, &mut out.0), (&r, &mut out.1)] {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let o = (b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
            x2 = x1;
            x1 = *v;
            y2 = y1;
            y1 = o;
            y[i] = o as f32;
        }
    }
    normalise(out)
}

/// `x` through Fractus at `bpm` from bar 1, in blocks of 480: `notes` handed in at their
/// samples, `changes` of the settings at the start of the block that holds them.
fn run(
    set: &FractusSettings,
    pat: &Pattern,
    x: &Stereo,
    bpm: f64,
    notes: &[(usize, Note)],
    changes: &[(usize, FractusSettings)],
) -> Stereo {
    let mut u = Fractus::new(SR);
    u.set(set);
    u.set_pattern(pat);
    let (mut l, mut r) = x.clone();
    let mut at = 0usize;
    while at < l.len() {
        let n = 480.min(l.len() - at);
        for (s, c) in changes {
            if *s >= at && *s < at + n {
                u.set(c);
            }
        }
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: bpm,
            samples: at as i64,
        });
        for &(s, note) in notes {
            if s >= at && s < at + n {
                u.note(s - at, note);
            }
        }
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn write(dir: &Path, name: &str, x: &Stereo) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(dir.join(name), spec).expect("wav");
    for (a, b) in x.0.iter().zip(&x.1) {
        for v in [a, b] {
            w.write_sample((v.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

/// Matched to `dry`'s RMS, its peak kept under −1 dBFS: the gain, and how far the peak
/// held it under the match (dB).
fn level_match(x: &mut Stereo, dry: &Stereo) -> (f64, f64) {
    let want = math::db_to_gain(0.0) as f64 * rms(&dry.0, &dry.1) / rms(&x.0, &x.1);
    let cap = f64::from(math::db_to_gain(-1.0) / peak(&x.0, &x.1));
    let g = want.min(cap);
    scale(x, g as f32);
    (
        20.0 * math::log10(g),
        if cap < want {
            20.0 * math::log10(want / cap)
        } else {
            0.0
        },
    )
}

/// A 1/16 grid index: bar `b` (from 0), sixteenth `s`.
fn at(b: usize, s: usize) -> usize {
    16 * b + s
}

fn sixteenths() -> FractusSettings {
    FractusSettings {
        step: 0.25,
        bars: 4,
        ..FractusSettings::default()
    }
}

/// A note held from sixteenth `from` for `len` sixteenths at `bpm` (bar `b`).
fn held(bpm: f64, b: usize, from: f64, len: f64, key: u8, vel: u8) -> [(usize, Note); 2] {
    let six = beat(bpm) / 4.0;
    let on = ((16.0 * b as f64 + from) * six).round() as usize;
    let off = ((16.0 * b as f64 + from + len) * six).round() as usize;
    [(on, Note::On(key, vel)), (off, Note::Off(key))]
}

fn key(m: Module) -> u8 {
    48 + m as u8
}

/// One sound example: its file, what it is, its source, its settings, its pattern, notes
/// and changes.
struct Example {
    file: &'static str,
    what: &'static str,
    source: usize,
    set: FractusSettings,
    pat: Pattern,
    notes: Vec<(usize, Note)>,
    changes: Vec<(usize, FractusSettings)>,
}

fn examples() -> Vec<Example> {
    let mut v = vec![];
    // 1. The stutter build: a clean bar, a reversed last beat, a gated and shuffled bar,
    // then the build — the repeat halving its slice each beat (1/4, 1/8, 1/16, 1/32) and
    // rising a quarter tone each repeat under a high-pass that opens — into a clean drop.
    let mut p = Pattern::EMPTY;
    p.block(Module::Reverse, at(1, 12), 4, 1.0, 1.0);
    p.block(Module::Gate, at(2, 0), 4, 0.9, 1.0);
    p.block(Module::Shuffle, at(2, 8), 4, 1.0, 1.0);
    p.block(Module::Throw, at(2, 14), 1, 1.0, 1.0);
    p.block(Module::Repeat, at(3, 0), 16, 1.0, 1.0);
    p.block(Module::Filter, at(3, 0), 16, 1.0, 1.0);
    v.push(Example {
        file: "01-drums-stutter-build.wav",
        what: "Drums, 8 bars at 120 BPM (a 4-bar sequence, twice): bar 2's last beat reversed, \
               bar 3 gated (1/32) and shuffled with a throw, bar 4 the stutter build — the \
               repeat halving its slice every beat (1/4 → 1/32), each repeat a quarter tone \
               higher, under a high-pass opening from 30 Hz to 1.5 kHz — into a clean bar 5.",
        source: 0,
        set: FractusSettings {
            repeat: RepeatSettings {
                size: 1.0,
                ramp: 3,
                pitch_st: 0.5,
                decay_db: 0.0,
                duty: 0.85,
            },
            filter: FilterSettings {
                kind: FilterKind::HighPass,
                from_hz: 30.0,
                to_hz: 1_500.0,
                res: 0.35,
                curve: 0.4,
            },
            gate: GateSettings {
                rate: 0.125,
                duty: 0.45,
                depth: 1.0,
            },
            shuffle: ShuffleSettings {
                slice: 0.25,
                range: 3,
                reverse: 0.3,
            },
            ..sixteenths()
        },
        pat: p,
        notes: vec![],
        changes: vec![],
    });
    // 2. The shuffle: two-bar cycles, sixteenths re-ordered from up to four back (a
    // quarter of them backwards), some steps crushed, a tape stop closing bar 2.
    let mut p = Pattern::EMPTY;
    p.block(Module::Shuffle, at(0, 4), 8, 1.0, 0.9);
    p.block(Module::Shuffle, at(1, 0), 12, 1.0, 0.9);
    p.block(Module::Crush, at(0, 12), 2, 0.8, 1.0);
    p.block(Module::Crush, at(1, 6), 2, 0.8, 0.7);
    p.block(Module::TapeStop, at(1, 12), 4, 1.0, 1.0);
    v.push(Example {
        file: "02-drums-shuffle.wav",
        what: "Drums at 120 BPM: two-bar cycles of sixteenths shuffled from up to four back, \
               a quarter of them reversed, two steps crushed (6 bits, 8 kHz), each bar 2 \
               ending in a tape stop. Probability 0.9 on the shuffles: each cycle differs, the \
               same every render.",
        source: 0,
        set: FractusSettings {
            bars: 2,
            shuffle: ShuffleSettings {
                slice: 0.25,
                range: 4,
                reverse: 0.25,
            },
            crush: cantus_dsp::fractus::CrushSettings {
                bits: 6.0,
                rate_hz: 8_000.0,
            },
            tape: TapeSettings {
                kind: TapeKind::Stop,
                time: 0.0,
                curve: 0.6,
            },
            ..sixteenths()
        },
        pat: p,
        notes: vec![],
        changes: vec![],
    });
    // 3. A performance: notes played on the modules, quantised to sixteenths.
    let bpm = 120.0;
    let mut notes = vec![];
    for n in [
        held(bpm, 1, 8.0, 4.0, key(Module::Repeat), 127),
        held(bpm, 2, 12.0, 4.0, key(Module::Reverse), 127),
        held(bpm, 3, 0.0, 8.0, key(Module::Gate), 110),
        held(bpm, 3, 12.0, 4.0, key(Module::TapeStop), 127),
        held(bpm, 5, 4.0, 8.0, key(Module::Pitch), 127),
        held(bpm, 5, 14.0, 1.0, key(Module::Throw), 127),
        held(bpm, 6, 0.0, 6.0, key(Module::Shuffle), 127),
        held(bpm, 6, 8.0, 8.0, key(Module::Repeat), 127),
        held(bpm, 6, 8.0, 8.0, key(Module::Crush), 90),
        held(bpm, 7, 8.0, 8.0, key(Module::Filter), 127),
    ] {
        notes.extend(n);
    }
    notes.sort_by_key(|n| n.0);
    v.push(Example {
        file: "03-drums-glitch-performance.wav",
        what: "Drums at 120 BPM played live from notes (quantised to sixteenths): a held \
               1/16 repeat, a reversed beat, a 1/32 gate, a tape stop, a fifth up for two \
               beats, a throw, a shuffle, a crushed 1/32 repeat, a closing low-pass sweep.",
        source: 0,
        set: FractusSettings {
            trigger: Trigger::Notes,
            quantise: true,
            repeat: RepeatSettings {
                size: 0.25,
                ..RepeatSettings::default()
            },
            gate: GateSettings {
                rate: 0.125,
                duty: 0.5,
                depth: 1.0,
            },
            pitch: PitchSettings {
                semis: 7.0,
                window_ms: 45.0,
                clean: false,
            },
            filter: FilterSettings {
                kind: FilterKind::LowPass,
                from_hz: 14_000.0,
                to_hz: 250.0,
                res: 0.5,
                curve: -0.3,
            },
            ..sixteenths()
        },
        pat: Pattern::EMPTY,
        notes,
        changes: vec![],
    });
    // 4. The guitar, sequenced: a reversed beat, a descending 1/32 stutter, a throw, a
    // fifth up, a closing sweep (two-bar cycles at 90 BPM).
    let mut p = Pattern::EMPTY;
    p.block(Module::Reverse, at(0, 12), 4, 1.0, 1.0);
    p.block(Module::Throw, at(0, 6), 1, 1.0, 1.0);
    p.block(Module::Pitch, at(1, 4), 4, 0.8, 1.0);
    p.block(Module::Repeat, at(1, 12), 4, 1.0, 1.0);
    p.block(Module::Filter, at(1, 8), 8, 1.0, 1.0);
    v.push(Example {
        file: "04-guitar-sequenced.wav",
        what: "The guitar progression at 90 BPM through a two-bar sequence: bar 1's last beat \
               reversed with a dotted-eighth throw before it, bar 2 an octave up for a beat \
               (mixed in at 80 %), then a 1/32 stutter falling a semitone a repeat under a \
               closing low-pass.",
        source: 1,
        set: FractusSettings {
            bars: 2,
            repeat: RepeatSettings {
                size: 0.125,
                pitch_st: -1.0,
                decay_db: -0.7,
                ..RepeatSettings::default()
            },
            reverse: ReverseSettings { slice: 1.0 },
            pitch: PitchSettings {
                semis: 12.0,
                window_ms: 60.0,
                clean: false,
            },
            filter: FilterSettings {
                kind: FilterKind::LowPass,
                from_hz: 12_000.0,
                to_hz: 500.0,
                res: 0.3,
                curve: 0.0,
            },
            throw: ThrowSettings {
                time: 0.75,
                feedback: 0.5,
                tone_hz: 4_000.0,
                pingpong: true,
                level: 0.8,
            },
            ..sixteenths()
        },
        pat: p,
        notes: vec![],
        changes: vec![],
    });
    // 5. The guitar with a cloud beside it: fifths and octaves, half of them backwards.
    v.push(Example {
        file: "05-guitar-cloud.wav",
        what: "The guitar with the grain cloud beside it (level 0.6): grains of 150 ms from \
               up to half a second back, ±12 st quantised to fifths and octaves, spread \
               wide, 40 % backwards.",
        source: 1,
        set: FractusSettings {
            cloud: CloudSettings {
                level: 0.6,
                position_ms: 200.0,
                spray_ms: 300.0,
                size_ms: 150.0,
                density_hz: 30.0,
                pitch_st: 0.0,
                pitch_rand_st: 12.0,
                scale: Scale::Fifths,
                spread: 1.0,
                reverse: 0.4,
                window: GrainWindow::Hann,
                freeze: false,
            },
            ..sixteenths()
        },
        pat: Pattern::EMPTY,
        notes: vec![],
        changes: vec![],
    });
    // 6. The frozen pad: only the cloud is heard; frozen through bars 3–6 (the chords
    // move on underneath, the cloud keeps Am and F), thawed at bar 7.
    let mut p = Pattern::EMPTY;
    p.block(Module::Cloud, 0, 64, 1.0, 1.0);
    let cloud = CloudSettings {
        level: 0.0,
        position_ms: 300.0,
        spray_ms: 900.0,
        size_ms: 400.0,
        density_hz: 24.0,
        pitch_st: 0.0,
        pitch_rand_st: 0.0,
        scale: Scale::Chromatic,
        spread: 1.0,
        reverse: 0.5,
        window: GrainWindow::Gauss,
        freeze: false,
    };
    let set = FractusSettings {
        cloud,
        ..sixteenths()
    };
    let bar = (4.0 * beat(90.0)) as usize;
    let frozen = FractusSettings {
        cloud: CloudSettings {
            freeze: true,
            ..cloud
        },
        ..set
    };
    v.push(Example {
        file: "06-pad-frozen-cloud.wav",
        what: "The pad as a cloud alone (its lane painted through): 400 ms Gauss grains from \
               up to 1.2 s back, half backwards, spread wide; frozen from bar 3 to bar 6 — \
               the chords move on underneath, the cloud holds Am and F — and thawed at bar 7.",
        source: 2,
        set,
        pat: p,
        notes: vec![],
        changes: vec![(2 * bar, frozen), (6 * bar, set)],
    });
    // 7. A pitched cloud over the pad: sparkle in fifths and octaves.
    v.push(Example {
        file: "07-pad-pitched-cloud.wav",
        what: "The pad with a pitched cloud beside it (level 0.8): 80 ms percussive grains \
               (Expodec), 60 a second, one or two octaves up or a fifth, quantised to fifths \
               and octaves, spread wide.",
        source: 2,
        set: FractusSettings {
            cloud: CloudSettings {
                level: 0.8,
                position_ms: 100.0,
                spray_ms: 400.0,
                size_ms: 80.0,
                density_hz: 60.0,
                pitch_st: 12.0,
                pitch_rand_st: 12.0,
                scale: Scale::Fifths,
                spread: 1.0,
                reverse: 0.0,
                window: GrainWindow::Expodec,
                freeze: false,
            },
            ..sixteenths()
        },
        pat: Pattern::EMPTY,
        notes: vec![],
        changes: vec![],
    });
    v
}

/// Seconds of processing a second of audio, best of three.
fn cost(set: &FractusSettings, pat: &Pattern, x: &Stereo, bpm: f64) -> (f64, usize) {
    let secs = x.0.len() as f64 / f64::from(SR);
    let mut best = f64::MAX;
    let mut grains = 0;
    for _ in 0..3 {
        let mut u = Fractus::new(SR);
        u.set(set);
        u.set_pattern(pat);
        let (mut l, mut r) = x.clone();
        let t = Instant::now();
        let mut at = 0usize;
        let mut most = 0usize;
        while at < l.len() {
            let n = 256.min(l.len() - at);
            u.set_transport(&Transport {
                playing: true,
                tempo_bpm: bpm,
                samples: at as i64,
            });
            u.process(&mut l[at..at + n], &mut r[at..at + n]);
            let mut tel = [0.0f32; 64];
            u.telemetry(&mut tel);
            most = most.max(tel[cantus_dsp::fractus::t::GRAINS] as usize);
            at += n;
        }
        best = best.min(t.elapsed().as_secs_f64() / secs);
        grains = most;
    }
    (best, grains)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let guitar_dir = Path::new(
        args.get(1)
            .map_or("C:/projects/daw-fx/tmp/guitar-wav", |s| s),
    );
    let out = Path::new(args.get(2).map_or("fractus-sound", |s| s));
    std::fs::create_dir_all(out).unwrap();
    let sources = [drums(120.0, 8), guitar(guitar_dir), pad()];
    let names = ["00-drums-dry.wav", "00-guitar-di.wav", "00-pad-dry.wav"];
    let bpms = [120.0, 90.0, 90.0];
    let mut readme = String::from(
        "# Fractus — sound examples (DSP only, no engine)\n\n\
         48 kHz, 24-bit WAV, stereo. Every processed file beside its dry twin (`00-…`, peak \
         −6 dBFS), matched to the twin's RMS unless its peak would pass −1 dBFS (then said).\n\n\
         Sources: **drums** — Cantus's own Pulsus (model 2: kick, snare with ghosts, closed and \
         open hats), a two-bar loop at 120 BPM, synthesized in the example; **guitar** — \
         *Strummed Guitar Chords* by Carbonthief (CC0, Renoise forum library), the owner's \
         progression Am – F – C – G at 90 BPM, two strums a bar, eight bars (Guitar: \
         Carbonthief, CC0); **pad** — synthesized in the example (band-limited saws, the same \
         chords).\n\n",
    );
    for (s, n) in sources.iter().zip(names) {
        write(out, n, s);
    }
    for e in examples() {
        let dry = &sources[e.source];
        let mut y = run(&e.set, &e.pat, dry, bpms[e.source], &e.notes, &e.changes);
        let (g, held) = level_match(&mut y, dry);
        write(out, e.file, &y);
        readme.push_str(&format!("- **`{}`** — {}", e.file, e.what));
        readme.push_str(&format!(" *(level {g:+.1} dB"));
        if held > 0.0 {
            readme.push_str(&format!("; {held:.1} dB under the RMS match, for the peak"));
        }
        readme.push_str(")*\n");
        println!("{}", e.file);
    }
    // The cost, measured on the pad (20 s), best of three.
    let heavy = FractusSettings {
        cloud: CloudSettings {
            level: 1.0,
            position_ms: 300.0,
            spray_ms: 800.0,
            size_ms: 500.0,
            density_hz: 200.0,
            pitch_st: 0.0,
            pitch_rand_st: 24.0,
            scale: Scale::Free,
            spread: 1.0,
            reverse: 0.5,
            window: GrainWindow::Hann,
            freeze: false,
        },
        ..sixteenths()
    };
    let busy = examples().remove(0);
    readme.push_str("\n## What it costs (release build, one core, best of three; measured while other builders compiled on this machine, so an upper bound)\n\n");
    for (what, set, pat, src) in [
        (
            "At the defaults (a wire)",
            sixteenths(),
            Pattern::EMPTY,
            2usize,
        ),
        (
            "The stutter build (every read module, filter, gate, throw)",
            busy.set,
            busy.pat,
            0,
        ),
        (
            "The cloud at its defaults beside the chain (24 grains a second of 90 ms)",
            FractusSettings {
                cloud: CloudSettings {
                    level: 0.5,
                    ..CloudSettings::default()
                },
                ..sixteenths()
            },
            Pattern::EMPTY,
            2,
        ),
        (
            "The cloud at 200 grains a second of 500 ms, ±24 st",
            heavy,
            Pattern::EMPTY,
            2,
        ),
    ] {
        let (c, grains) = cost(&set, &pat, &sources[src], bpms[src]);
        let line = format!(
            "- {what}: {:.2} % of a core in real time{}\n",
            100.0 * c,
            if grains > 0 {
                format!(" ({grains} grains at once)")
            } else {
                String::new()
            }
        );
        print!("{line}");
        readme.push_str(&line);
    }
    std::fs::write(out.join("README.md"), readme).unwrap();
}
