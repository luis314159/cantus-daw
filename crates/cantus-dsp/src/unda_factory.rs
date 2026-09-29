//! Unda's factory wavetables, generated in code from their spectra or from cycles drawn
//! at 8× and band-limited — so none is copied from anywhere and every one is exactly
//! band-limited. Each frame is normalised to a peak of 1 on its own, so a sweep (a
//! harmonic series growing from a sine) does not start 30 dB down. Built once per process, on first use (off the audio thread: a unit's
//! `new` asks for them), and shared by every unit and voice.
//!
//! Order and names are `cantus_core::inst::wavetable::TABLES`.

use super::table::{spectrum_of, Partial, Table, HARMONICS};
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

/// The factory tables, in `osc{n}.wt` order.
pub fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        vec![
            basic(),
            analog(),
            pwm(),
            harmonics(),
            formant(),
            vocal(),
            digital(),
            sync(),
            fm(),
            organ(),
        ]
    })
}

/// The sub oscillator's shapes: sine, triangle, saw, square, pulse (25 %), one per frame.
pub fn sub_table() -> &'static Table {
    static SUB: OnceLock<Table> = OnceLock::new();
    SUB.get_or_init(|| {
        Table::from_spectra_each(&[sine(), triangle(), saw(), square(), pulse(0.25)])
    })
}

fn partials(f: impl Fn(usize) -> Partial) -> Vec<Partial> {
    (1..=HARMONICS).map(f).collect()
}

fn sine() -> Vec<Partial> {
    partials(|h| if h == 1 { (0.0, 1.0) } else { (0.0, 0.0) })
}

/// A rising ramp.
fn saw() -> Vec<Partial> {
    partials(|h| (0.0, -2.0 / (PI * h as f32)))
}

fn square() -> Vec<Partial> {
    partials(|h| {
        if h % 2 == 1 {
            (0.0, 4.0 / (PI * h as f32))
        } else {
            (0.0, 0.0)
        }
    })
}

fn triangle() -> Vec<Partial> {
    partials(|h| {
        if h % 2 == 1 {
            let s = if (h / 2) % 2 == 0 { 1.0 } else { -1.0 };
            (0.0, s * 8.0 / (PI * PI * (h * h) as f32))
        } else {
            (0.0, 0.0)
        }
    })
}

/// High for the fraction `w` of the cycle.
fn pulse(w: f32) -> Vec<Partial> {
    partials(|h| {
        let k = 2.0 / (PI * h as f32);
        let a = TAU * h as f32 * w;
        (k * a.sin(), k * (1.0 - a.cos()))
    })
}

/// Saw, square, triangle, sine: darker frame by frame.
fn basic() -> Table {
    Table::from_spectra_each(&[saw(), square(), triangle(), sine()])
}

/// A saw whose even harmonics fade out: saw to square in 16 frames.
fn analog() -> Table {
    let s = saw();
    let frames: Vec<Vec<Partial>> = (0..16)
        .map(|k| {
            let m = k as f32 / 15.0;
            s.iter()
                .enumerate()
                .map(|(i, &(a, b))| {
                    let g = if (i + 1) % 2 == 0 { 1.0 - m } else { 1.0 };
                    (a * g, b * g)
                })
                .collect()
        })
        .collect();
    Table::from_spectra_each(&frames)
}

/// Pulse width from 50 % down to 2 %, in 32 frames.
fn pwm() -> Table {
    let frames: Vec<Vec<Partial>> = (0..32)
        .map(|k| pulse(0.5 - 0.48 * k as f32 / 31.0))
        .collect();
    Table::from_spectra_each(&frames)
}

/// Harmonics added one octave-fraction at a time: a sine, then 1/√h partials up to
/// 2^(k·10/31) of them — additive brightness in 32 frames.
fn harmonics() -> Table {
    let frames: Vec<Vec<Partial>> = (0..32)
        .map(|k| {
            let top = 2f32.powf(k as f32 * 10.0 / 31.0).round() as usize;
            partials(|h| {
                if h <= top {
                    (0.0, 1.0 / (h as f32).sqrt())
                } else {
                    (0.0, 0.0)
                }
            })
        })
        .collect();
    Table::from_spectra_each(&frames)
}

/// A saw through one resonant peak that climbs from the 2nd to the 40th harmonic.
fn formant() -> Table {
    let s = saw();
    let frames: Vec<Vec<Partial>> = (0..32)
        .map(|k| {
            let c = 2.0 * 20f32.powf(k as f32 / 31.0);
            let w = 0.25 * c + 1.0;
            s.iter()
                .enumerate()
                .map(|(i, &(a, b))| {
                    let d = (i as f32 + 1.0 - c) / w;
                    let g = 1.0 + 8.0 * (-d * d).exp();
                    (a * g, b * g)
                })
                .collect()
        })
        .collect();
    Table::from_spectra_each(&frames)
}

/// The vowels a, e, i, o, u (Peterson & Barney 1952, adult male averages) sung at A2:
/// a −6 dB/oct source through three formants, morphing across 25 frames.
fn vocal() -> Table {
    const VOWELS: [[f32; 3]; 5] = [
        [730.0, 1090.0, 2440.0],
        [530.0, 1840.0, 2480.0],
        [270.0, 2290.0, 3010.0],
        [570.0, 840.0, 2410.0],
        [300.0, 870.0, 2240.0],
    ];
    const BW: [f32; 3] = [80.0, 100.0, 120.0];
    const GAIN: [f32; 3] = [1.0, 0.5, 0.3];
    let f0 = 110.0;
    let frames: Vec<Vec<Partial>> = (0..25)
        .map(|k| {
            let x = k as f32 / 6.0;
            let v = (x.floor() as usize).min(3);
            let t = (x - v as f32).min(1.0);
            let f: Vec<f32> = (0..3)
                .map(|i| VOWELS[v][i] + (VOWELS[v + 1][i] - VOWELS[v][i]) * t)
                .collect();
            partials(|h| {
                let hz = f0 * h as f32;
                if hz > 5_000.0 {
                    return (0.0, 0.0);
                }
                let env: f32 = (0..3)
                    .map(|i| {
                        let d = (hz - f[i]) / BW[i];
                        GAIN[i] / (1.0 + d * d).sqrt()
                    })
                    .sum();
                (0.0, env / h as f32)
            })
        })
        .collect();
    Table::from_spectra_each(&frames)
}

/// Samples per drawn cycle for the time-domain tables: 8× the frame.
const DRAW: usize = 16_384;

fn drawn(frames: usize, f: impl Fn(usize, f32) -> f32) -> Table {
    let cycles: Vec<Vec<f32>> = (0..frames)
        .map(|k| (0..DRAW).map(|i| f(k, i as f32 / DRAW as f32)).collect())
        .collect();
    let spectra: Vec<Vec<Partial>> = cycles.iter().map(|c| spectrum_of(c)).collect();
    Table::from_spectra_each(&spectra)
}

/// A sine through a sample-and-hold and a quantiser that both coarsen frame by frame:
/// 64 steps a cycle and 16 levels down to 4 steps and 2 levels.
fn digital() -> Table {
    drawn(32, |k, t| {
        let m = k as f32 / 31.0;
        let steps = (64.0 * (1.0 / 16.0f32).powf(m)).round().max(4.0);
        let levels = (8.0 * (1.0 / 8.0f32).powf(m)).round().max(1.0);
        let held = (t * steps).floor() / steps;
        let y = (TAU * held).sin();
        (y * levels).round() / levels
    })
}

/// A saw hard-synced to the cycle, its own frequency from 1× to 8×.
fn sync() -> Table {
    drawn(32, |k, t| {
        let r = 8f32.powf(k as f32 / 31.0);
        2.0 * (r * t).fract() - 1.0
    })
}

/// Two-operator FM at 1:1, the index from 0 to 8 radians.
fn fm() -> Table {
    drawn(32, |k, t| {
        let index = 8.0 * k as f32 / 31.0;
        (TAU * t + index * (TAU * t).sin()).sin()
    })
}

/// Eight drawbar registrations on a 16′ fundamental (3 dB a step).
fn organ() -> Table {
    // 16′ 5⅓′ 8′ 4′ 2⅔′ 2′ 1⅗′ 1⅓′ 1′ as harmonics of 16′.
    const HARM: [usize; 9] = [1, 3, 2, 4, 6, 8, 10, 12, 16];
    const REG: [[u8; 9]; 8] = [
        [8, 0, 0, 0, 0, 0, 0, 0, 0],
        [8, 8, 8, 0, 0, 0, 0, 0, 0],
        [8, 6, 8, 0, 0, 0, 0, 0, 0],
        [6, 8, 8, 6, 0, 0, 0, 0, 0],
        [8, 8, 8, 8, 0, 0, 0, 0, 0],
        [8, 0, 0, 0, 0, 0, 8, 8, 8],
        [0, 0, 8, 8, 8, 8, 8, 0, 0],
        [8, 8, 8, 8, 8, 8, 8, 8, 8],
    ];
    let frames: Vec<Vec<Partial>> = REG
        .iter()
        .map(|reg| {
            let mut p = vec![(0.0, 0.0); HARMONICS];
            for (bar, &d) in reg.iter().enumerate() {
                if d > 0 {
                    p[HARM[bar] - 1].1 += 10f32.powf((d as f32 - 8.0) * 3.0 / 20.0);
                }
            }
            p
        })
        .collect();
    Table::from_spectra_each(&frames)
}
