//! Arca's factory cabinets: impulse responses **we make ourselves** (CC0 — no recording,
//! no third-party file), from a model of the three things a miked cabinet is:
//!
//! 1. **The speaker in its box**: the cone's resonance in the cabinet (a second-order high
//!    pass at `fs`, `Q` above 1 for a closed back — the low "thump"), an open back's
//!    front-to-back cancellation (a first-order high pass), the cone's break-up peaks
//!    (where a guitar speaker's bite lives, 1.5 – 5 kHz), its steep fall above them (a
//!    fourth-order low pass) and the dip past it.
//! 2. **The microphone**: dynamic (a presence peak, its low end thinned), ribbon (dark,
//!    smooth, more proximity) or condenser (flat, airy).
//! 3. **Where it stands**: on the cap (bright) to the cone's edge (dark) — a shelf and a
//!    low pass that move together — and its distance: proximity effect close up, the floor's
//!    reflection (a comb) further back.
//!
//! The magnitude is designed in dB on a fine grid, normalised to 0 dB over 100 Hz –
//! 5 kHz, and made **minimum phase** through the real cepstrum (the response starts at its
//! first sample: the convolution adds no latency, and nothing rings before the attack),
//! then shortened to 43 ms with a raised-cosine end. Deterministic: `cantus_dsp::math`
//! and Aula's FFT. Allocates: made off the audio thread.

use crate::aula_fft::Fft;
use crate::math;

/// The cabinets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Cabinet {
    /// 1 × 12″, open back (the clean amp's combo).
    OneTwelveOpen,
    /// 2 × 12″, open back, alnico-style (the chime amp's).
    TwoTwelveOpen,
    /// 4 × 12″, closed back, vintage greenback-style (the crunch amp's).
    FourTwelveVintage,
    /// 4 × 12″, closed back, modern (the high-gain amp's).
    FourTwelveModern,
    /// 4 × 10″, open back (the bass amp's).
    FourTenBass,
}

impl Cabinet {
    pub const ALL: [Cabinet; 5] = [
        Cabinet::OneTwelveOpen,
        Cabinet::TwoTwelveOpen,
        Cabinet::FourTwelveVintage,
        Cabinet::FourTwelveModern,
        Cabinet::FourTenBass,
    ];

    pub fn index(self) -> usize {
        Cabinet::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Cabinet {
        Cabinet::ALL[i.min(4)]
    }

    fn voice(self) -> Voice {
        match self {
            Cabinet::OneTwelveOpen => Voice {
                fs: 85.0,
                q: 0.9,
                open: Some(120.0),
                peaks: [
                    (180.0, 1.5, 1.2),
                    (1_000.0, 1.5, 1.0),
                    (2_200.0, 4.0, 1.6),
                    (3_500.0, 3.0, 2.0),
                ],
                lp: 5_500.0,
                dip: (7_500.0, -6.0, 3.0),
            },
            Cabinet::TwoTwelveOpen => Voice {
                fs: 80.0,
                q: 0.85,
                open: Some(100.0),
                peaks: [
                    (150.0, 1.0, 1.0),
                    (1_300.0, 2.0, 1.2),
                    (3_000.0, 6.0, 1.8),
                    (4_500.0, 3.0, 2.5),
                ],
                lp: 6_000.0,
                dip: (8_000.0, -6.0, 3.0),
            },
            Cabinet::FourTwelveVintage => Voice {
                fs: 95.0,
                q: 1.4,
                open: None,
                peaks: [
                    (110.0, 2.0, 1.5),
                    (1_600.0, 2.0, 1.4),
                    (2_500.0, 5.0, 1.8),
                    (3_600.0, 2.0, 2.4),
                ],
                lp: 4_800.0,
                dip: (6_500.0, -8.0, 3.0),
            },
            Cabinet::FourTwelveModern => Voice {
                fs: 90.0,
                q: 1.5,
                open: None,
                peaks: [
                    (450.0, -1.5, 1.0),
                    (2_000.0, 3.0, 1.2),
                    (2_800.0, 6.0, 1.6),
                    (4_500.0, 4.0, 2.2),
                ],
                lp: 5_500.0,
                dip: (7_000.0, -8.0, 3.0),
            },
            Cabinet::FourTenBass => Voice {
                fs: 100.0,
                q: 0.9,
                open: Some(90.0),
                peaks: [
                    (250.0, 1.5, 1.0),
                    (1_800.0, 3.0, 1.5),
                    (3_200.0, 4.0, 2.0),
                    (5_000.0, 1.0, 2.0),
                ],
                lp: 6_500.0,
                dip: (9_000.0, -6.0, 3.0),
            },
        }
    }
}

/// The microphones.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mic {
    Dynamic,
    Ribbon,
    Condenser,
}

impl Mic {
    pub const ALL: [Mic; 3] = [Mic::Dynamic, Mic::Ribbon, Mic::Condenser];

    pub fn from_index(i: usize) -> Mic {
        Mic::ALL[i.min(2)]
    }
}

/// A miked cabinet.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CabSettings {
    pub cabinet: Cabinet,
    pub mic: Mic,
    /// From the cone's centre (0, on the cap) to its edge (1).
    pub position: f32,
    /// From the grille cloth, cm (1 … 30).
    pub distance_cm: f32,
}

impl Default for CabSettings {
    fn default() -> Self {
        CabSettings {
            cabinet: Cabinet::OneTwelveOpen,
            mic: Mic::Dynamic,
            position: 0.3,
            distance_cm: 3.0,
        }
    }
}

/// A speaker in its cabinet: resonance, open back, break-up peaks `(Hz, dB, Q)`, the
/// fall's corner and the dip past it.
#[derive(Copy, Clone, Debug)]
struct Voice {
    fs: f32,
    q: f32,
    open: Option<f32>,
    peaks: [(f32, f32, f32); 4],
    lp: f32,
    dip: (f32, f32, f32),
}

fn db(x: f32) -> f32 {
    20.0 * math::log10(x.max(1e-12))
}

/// Analog magnitudes at `f`, in dB.
fn hp2(f: f32, f0: f32, q: f32) -> f32 {
    let x = f / f0;
    db(x * x / ((1.0 - x * x) * (1.0 - x * x) + (x / q) * (x / q)).sqrt())
}
fn lp2(f: f32, f0: f32, q: f32) -> f32 {
    let x = f / f0;
    db(1.0 / ((1.0 - x * x) * (1.0 - x * x) + (x / q) * (x / q)).sqrt())
}
fn hp1(f: f32, f0: f32) -> f32 {
    let x = f / f0;
    db(x / (1.0 + x * x).sqrt())
}
fn peak(f: f32, f0: f32, g_db: f32, q: f32) -> f32 {
    let a = math::powf(10.0f32, g_db / 40.0);
    let x = f / f0;
    let re = 1.0 - x * x;
    let num = re * re + (x * a / q) * (x * a / q);
    let den = re * re + (x / (a * q)) * (x / (a * q));
    10.0 * math::log10(num / den)
}
fn low_shelf(f: f32, fc: f32, g_db: f32) -> f32 {
    let x = f / fc;
    g_db / (1.0 + x * x)
}
fn high_shelf(f: f32, fc: f32, g_db: f32) -> f32 {
    let x = f / fc;
    g_db * x * x / (1.0 + x * x)
}

/// The designed magnitude at `f` Hz, in dB, before normalisation and before the
/// distance's reflection.
pub fn design_db(s: &CabSettings, f: f32) -> f32 {
    let v = s.cabinet.voice();
    let f = f.max(1.0);
    let mut g = hp2(f, v.fs, v.q);
    if let Some(o) = v.open {
        g += hp1(f, o);
    }
    for &(f0, gain, q) in &v.peaks {
        g += peak(f, f0, gain, q);
    }
    // The fall: fourth order (two sections, Butterworth's Qs).
    g += lp2(f, v.lp, 0.541) + lp2(f, v.lp, 1.307);
    g += peak(f, v.dip.0, v.dip.1, v.dip.2);
    // Where the microphone points: the cap is bright, the edge dark.
    let p = s.position.clamp(0.0, 1.0);
    g += high_shelf(f, 2_500.0, 3.0 - 10.0 * p);
    g += lp2(f, 9_000.0 * (1.0 - p) + 3_500.0 * p, 0.7);
    // Proximity: a low shelf that grows as the microphone comes close.
    let d = s.distance_cm.clamp(1.0, 30.0);
    let near = 1.0 / (1.0 + (d - 1.0) / 3.0);
    match s.mic {
        Mic::Dynamic => {
            g += peak(f, 5_500.0, 4.0, 1.5);
            g += hp2(f, 60.0, 0.7);
            g += lp2(f, 14_000.0, 0.7);
            g += low_shelf(f, 200.0, 6.0 * near);
        }
        Mic::Ribbon => {
            g += lp2(f, 9_000.0, 0.6);
            g += hp2(f, 30.0, 0.7);
            g += low_shelf(f, 220.0, 9.0 * near);
        }
        Mic::Condenser => {
            g += peak(f, 10_000.0, 2.0, 0.8);
            g += hp2(f, 20.0, 0.7);
            g += low_shelf(f, 200.0, 2.0 * near);
        }
    }
    g
}

/// The response's length at 48 kHz (43 ms), and the FFT it is designed on.
const TAPS_48K: usize = 2_048;

/// The minimum-phase impulse response of `s` at `rate` (taps ≈ 43 ms). Allocates.
pub fn impulse(s: &CabSettings, rate: f32) -> Vec<f32> {
    let taps = ((TAPS_48K as f32 * rate / 48_000.0).round() as usize).max(256);
    let n = (4 * taps).next_power_of_two();
    let half = n / 2;
    // The magnitude on the grid, normalised over 100 Hz – 5 kHz (a log-spaced mean).
    let mut mean = 0.0f32;
    let pts = 64;
    for k in 0..pts {
        let f = 100.0 * math::powf(50.0f32, k as f32 / (pts - 1) as f32);
        mean += design_db(s, f);
    }
    mean /= pts as f32;
    let fft = Fft::new(n);
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    let ln10 = core::f32::consts::LN_10;
    for k in 0..=half {
        let f = k as f32 * rate / n as f32;
        let g = (design_db(s, f) - mean).max(-120.0);
        re[k] = g / 20.0 * ln10;
        if k > 0 && k < half {
            re[n - k] = re[k];
        }
    }
    // The real cepstrum, folded onto the causal side: the minimum phase's log spectrum.
    fft.run(&mut re, &mut im, true);
    let inv = 1.0 / n as f32;
    for k in 0..n {
        let c = re[k] * inv;
        re[k] = if k == 0 || k == half {
            c
        } else if k < half {
            2.0 * c
        } else {
            0.0
        };
        im[k] = 0.0;
    }
    fft.run(&mut re, &mut im, false);
    for k in 0..n {
        let m = math::exp(re[k]);
        let (sn, cs) = math::sin_cos(im[k]);
        re[k] = m * cs;
        im[k] = m * sn;
    }
    fft.run(&mut re, &mut im, true);
    let mut h: Vec<f32> = re[..taps].iter().map(|x| x * inv).collect();
    // The floor's reflection at a distance: a later, quieter copy (the path from the
    // cone down to the floor and up again, 35 cm below).
    let d = s.distance_cm.clamp(1.0, 30.0) * 0.01;
    let extra = (d * d + 0.7 * 0.7).sqrt() - d;
    let lag = (extra / 343.0 * rate).round() as usize;
    let g = 0.35 * (d / 0.3);
    if lag > 0 && lag < taps {
        for i in (lag..taps).rev() {
            h[i] += g * h[i - lag];
        }
        let norm = 1.0 / (1.0 + g);
        for x in &mut h {
            *x *= norm;
        }
    }
    // A raised-cosine end over the last 30 %.
    let fade = taps * 3 / 10;
    for i in 0..fade {
        let w = 0.5 + 0.5 * math::cos(core::f32::consts::PI * (i + 1) as f32 / fade as f32);
        h[taps - fade + i] *= w;
    }
    h
}
