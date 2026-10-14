//! Exemplar's built-in sounds: samples *computed* when the program starts — single cycles,
//! plucked strings, bells, a pad, a small drum kit — so a zone can play something without
//! a file, the factory presets are real multisampled instruments, and no recorded content
//! ships while its licence is open (`research/instruments/sampler.md` §9.6).
//!
//! Deterministic: the same numbers on every machine (fixed seeds, `f64` arithmetic).
//! Single cycles are mip-mapped as periodic signals, so they stay exact at every octave.

use super::play::Source;
use super::resample::{halve, halve_periodic, MAX_LEVELS};
use crate::math;
use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

/// One built-in sample.
#[derive(Clone, Debug)]
pub struct Builtin {
    /// Locale key suffix and English name (`smp.gen.<key>`).
    pub key: &'static str,
    pub rate: u32,
    pub root: u8,
    /// Cents the sample is sharp of `root` (the editor shows it; the engine corrects it).
    pub tune_ct: f32,
    /// A loop that suits it (inclusive frames), or none.
    pub loop_points: Option<(u32, u32)>,
    pub frames: usize,
    levels: [Option<Arc<[f32]>>; MAX_LEVELS],
}

impl Builtin {
    pub fn source(&self) -> Source {
        Source {
            levels: self.levels.clone(),
            channels: 1,
            rate: self.rate as f32,
            frames: self.frames,
            tune_ct: self.tune_ct,
            stream: None,
        }
    }
    /// Level 0, for the editor's waveform.
    pub fn data(&self) -> &[f32] {
        self.levels[0].as_deref().unwrap_or(&[])
    }

    /// Level 0, shared.
    pub fn level0(&self) -> Option<Arc<[f32]>> {
        self.levels[0].clone()
    }
}

const CYCLE: usize = 2048;
/// C1: single cycles are rooted here and mip-mapped upwards.
const CYCLE_ROOT: u8 = 24;

fn midi_hz(key: f64) -> f64 {
    440.0 * math::exp2((key - 69.0) / 12.0)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

fn normalize(mut v: Vec<f64>, peak: f64) -> Vec<f32> {
    let m = v.iter().fold(0f64, |m, x| m.max(x.abs())).max(1e-12);
    v.iter_mut().for_each(|x| *x *= peak / m);
    v.into_iter().map(|x| x as f32).collect()
}

/// A single cycle from harmonic amplitudes (`amp(h)` for h = 1, 2, …), band-limited to
/// 20 kHz at its root.
fn cycle(amp: impl Fn(usize) -> f64) -> Vec<f32> {
    let f0 = midi_hz(CYCLE_ROOT as f64);
    let top = (20_000.0 / f0) as usize;
    let mut v = vec![0.0f64; CYCLE];
    for h in 1..=top.min(CYCLE / 2 - 1) {
        let a = amp(h);
        if a == 0.0 {
            continue;
        }
        for (i, x) in v.iter_mut().enumerate() {
            *x += a * math::sin(TAU * h as f64 * i as f64 / CYCLE as f64);
        }
    }
    normalize(v, 0.5)
}

/// Karplus–Strong: a plucked string at `key`, `secs` long, at 44.1 kHz.
fn pluck(key: u8, secs: f64, seed: u64) -> (Vec<f32>, f32) {
    let fs = 44_100.0;
    let f = midi_hz(key as f64);
    let n = (fs / f - 0.5).round().max(2.0) as usize;
    let actual = fs / (n as f64 + 0.5);
    let mut rng = Rng(seed);
    let mut line: Vec<f64> = (0..n).map(|_| rng.next()).collect();
    // Soften the excitation (a pick, not a hammer).
    for _ in 0..2 {
        for i in 1..n {
            line[i] = 0.5 * (line[i] + line[i - 1]);
        }
    }
    let len = (secs * fs) as usize;
    let mut out = Vec::with_capacity(len);
    let decay = math::powf(0.996f64, 440.0 / f.max(60.0));
    let mut i = 0;
    for t in 0..len {
        let a = line[i];
        let b = line[(i + 1) % n];
        let y = decay * 0.5 * (a + b);
        line[i] = y;
        let fade = if t + 2_000 > len {
            (len - t) as f64 / 2_000.0
        } else {
            1.0
        };
        out.push(a * fade);
        i = (i + 1) % n;
    }
    (
        normalize(out, 0.5),
        (1200.0 * math::log2(actual / f)) as f32,
    )
}

/// An inharmonic bell: partial ratios and amplitudes, each decaying on its own.
fn bell(key: u8, bright: f64) -> Vec<f32> {
    let fs = 44_100.0;
    let f = midi_hz(key as f64);
    let partials = [
        (0.56, 1.0, 1.6),
        (1.0, 0.8, 1.2),
        (2.0, 0.5, 0.8),
        (2.76, 0.45 * bright, 0.55),
        (5.40, 0.3 * bright, 0.35),
        (8.93, 0.2 * bright * bright, 0.2),
        (13.34, 0.1 * bright * bright, 0.12),
    ];
    let len = (3.0 * fs) as usize;
    let v: Vec<f64> = (0..len)
        .map(|t| {
            let s = t as f64 / fs;
            let attack = (s / 0.002).min(1.0);
            partials
                .iter()
                .filter(|p| p.0 * f < 19_000.0)
                .map(|(r, a, d)| a * math::exp(-s / d) * math::sin(TAU * r * f * s))
                .sum::<f64>()
                * attack
        })
        .collect();
    normalize(v, 0.5)
}

/// Seven detuned band-limited saws: an ensemble pad at `key`, `secs` long.
fn pad(key: u8, secs: f64) -> Vec<f32> {
    let fs = 44_100.0;
    let f = midi_hz(key as f64);
    let detune = [-9.0, -4.0, 0.0, 4.5, 10.0];
    let len = (secs * fs) as usize;
    let top = (12_000.0 / f) as usize;
    let mut v = vec![0.0f64; len];
    for (k, ct) in detune.iter().enumerate() {
        let fk = f * math::exp2(ct / 1200.0f64);
        let phase0 = k as f64 * 0.37;
        for h in 1..=top {
            let a = 1.0 / h as f64 * (1.0 - h as f64 / (top as f64 + 1.0));
            // A rotating phasor: one complex multiply per sample instead of a sine.
            let w = TAU * fk * h as f64 / fs;
            let (dc, ds) = (math::cos(w), math::sin(w));
            let (mut c, mut s) = (math::cos(phase0 * h as f64), math::sin(phase0 * h as f64));
            for x in v.iter_mut() {
                *x += a * s;
                let c2 = c * dc - s * ds;
                s = s * dc + c * ds;
                c = c2;
            }
        }
    }
    let attack = (0.3 * fs) as usize;
    for (t, x) in v.iter_mut().enumerate().take(attack) {
        *x *= t as f64 / attack as f64;
    }
    normalize(v, 0.5)
}

/// Drums: a kick, snares, hats and a key click.
fn drum(kind: u8, seed: u64) -> Vec<f32> {
    let fs = 44_100.0;
    let mut rng = Rng(seed);
    let secs = match kind {
        0 => 0.6,
        1 => 0.35,
        2 => 0.08,
        3 => 0.5,
        _ => 0.04,
    };
    let len = (secs * fs) as usize;
    let (mut lp, mut prev) = (0.0f64, 0.0f64);
    let mut phase = 0.0f64;
    let v: Vec<f64> = (0..len)
        .map(|t| {
            let s = t as f64 / fs;
            let n = rng.next();
            match kind {
                0 => {
                    let f = 45.0 + 110.0 * math::exp(-s / 0.04);
                    phase += TAU * f / fs;
                    math::sin(phase) * math::exp(-s / 0.18) + 0.3 * n * math::exp(-s / 0.004)
                }
                1 => {
                    phase += TAU * 185.0 / fs;
                    lp += 0.45 * (n - lp);
                    0.5 * math::sin(phase) * math::exp(-s / 0.05) + lp * math::exp(-s / 0.09)
                }
                2 | 3 => {
                    let hp = n - prev;
                    prev = n;
                    let d = if kind == 2 { 0.018 } else { 0.16 };
                    hp * math::exp(-s / d)
                }
                _ => {
                    lp += 0.2 * (n - lp);
                    lp * math::exp(-s / 0.006)
                }
            }
        })
        .collect();
    normalize(v, 0.5)
}

fn make(
    key: &'static str,
    rate: u32,
    root: u8,
    tune_ct: f32,
    loop_points: Option<(u32, u32)>,
    data: Vec<f32>,
    periodic: bool,
) -> Builtin {
    let frames = data.len();
    let mut levels: [Option<Arc<[f32]>>; MAX_LEVELS] = Default::default();
    let mut cur = data;
    for (k, slot) in levels.iter_mut().enumerate() {
        if k > 0 {
            if cur.len() < 16 {
                break;
            }
            cur = if periodic {
                halve_periodic(&cur, 1)
            } else {
                halve(&cur, 1)
            };
        }
        *slot = Some(Arc::from(cur.clone()));
    }
    Builtin {
        key,
        rate,
        root,
        tune_ct,
        loop_points,
        frames,
        levels,
    }
}

fn cycle_builtin(key: &'static str, data: Vec<f32>) -> Builtin {
    let f0 = midi_hz(CYCLE_ROOT as f64);
    let rate = (f0 * CYCLE as f64).round();
    let tune = 1200.0 * math::log2(rate / CYCLE as f64 / f0);
    make(
        key,
        rate as u32,
        CYCLE_ROOT,
        tune as f32,
        Some((0, CYCLE as u32 - 1)),
        data,
        true,
    )
}

fn build() -> Vec<Builtin> {
    let mut v = vec![
        cycle_builtin("sine", cycle(|h| if h == 1 { 1.0 } else { 0.0 })),
        cycle_builtin("saw", cycle(|h| 1.0 / h as f64)),
        cycle_builtin(
            "square",
            cycle(|h| if h % 2 == 1 { 1.0 / h as f64 } else { 0.0 }),
        ),
        cycle_builtin(
            "triangle",
            cycle(|h| {
                if h % 2 == 1 {
                    let s = if (h / 2) % 2 == 0 { 1.0 } else { -1.0 };
                    s / (h * h) as f64
                } else {
                    0.0
                }
            }),
        ),
    ];
    let mut rng = Rng(0x0A15E);
    let noise: Vec<f64> = (0..88_200).map(|_| rng.next()).collect();
    v.push(make(
        "noise",
        44_100,
        60,
        0.0,
        Some((22_050, 88_199)),
        normalize(noise, 0.5),
        false,
    ));
    for (key, name) in [(36u8, "pluck_c2"), (60, "pluck_c4"), (84, "pluck_c6")] {
        let (d, tune) = pluck(key, 2.4 - key as f64 / 60.0, 0xC0FFEE + key as u64);
        v.push(make(name, 44_100, key, tune, None, d, false));
    }
    v.push(make(
        "bell_soft",
        44_100,
        72,
        0.0,
        None,
        bell(72, 0.35),
        false,
    ));
    v.push(make(
        "bell_hard",
        44_100,
        72,
        0.0,
        None,
        bell(72, 1.0),
        false,
    ));
    v.push(make(
        "pad",
        44_100,
        48,
        0.0,
        Some((44_100, 129_999)),
        pad(48, 3.0),
        false,
    ));
    v.push(make("kick", 44_100, 36, 0.0, None, drum(0, 1), false));
    for (i, name) in ["snare_1", "snare_2", "snare_3"].into_iter().enumerate() {
        v.push(make(
            name,
            44_100,
            38,
            0.0,
            None,
            drum(1, 100 + i as u64),
            false,
        ));
    }
    for (i, name) in ["hat_1", "hat_2"].into_iter().enumerate() {
        v.push(make(
            name,
            44_100,
            42,
            0.0,
            None,
            drum(2, 200 + i as u64),
            false,
        ));
    }
    v.push(make("hat_open", 44_100, 46, 0.0, None, drum(3, 300), false));
    v.push(make("click", 44_100, 60, 0.0, None, drum(4, 400), false));
    let organ = [1.0, 0.8, 0.0, 0.6, 0.0, 0.45, 0.0, 0.35, 0.0, 0.0, 0.0, 0.2];
    v.push(cycle_builtin(
        "organ",
        cycle(|h| organ.get(h - 1).copied().unwrap_or(0.0)),
    ));
    let f0 = midi_hz(CYCLE_ROOT as f64 + 24.0);
    v.push(cycle_builtin(
        "choir",
        cycle(|h| {
            let f = f0 * h as f64;
            let peak = |c: f64, w: f64| math::exp(-math::powi((f - c) / w, 2));
            (peak(700.0, 130.0) + 0.7 * peak(1_150.0, 150.0) + 0.3 * peak(2_600.0, 250.0))
                / (h as f64).sqrt()
        }),
    ));
    v
}

static BUILTINS: OnceLock<Vec<Builtin>> = OnceLock::new();

/// Every built-in sound; `gen` n (1-based) is `builtins()[n − 1]`. The first call computes
/// them (tens of milliseconds): make it off the audio thread (`Exemplar`'s owner does).
pub fn builtins() -> &'static [Builtin] {
    BUILTINS.get_or_init(build)
}

/// The built-in for a zone's `gen` value.
pub fn get(gen: u8) -> Option<&'static Builtin> {
    if gen == 0 {
        return None;
    }
    builtins().get(gen as usize - 1)
}

/// `gen` numbers of the built-ins by key, for presets.
pub fn index_of(key: &str) -> Option<u8> {
    builtins()
        .iter()
        .position(|b| b.key == key)
        .map(|i| i as u8 + 1)
}
