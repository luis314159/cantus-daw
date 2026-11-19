//! Measuring a piano key's tuning as a tuner hears it (the technician's pass, `DSP_VERSION`
//! 10, `docs/round-fx/chorda-tuning.md`): each partial heterodyned to its designed frequency
//! and low-passed, both channels; its frequency the power-weighted mean of its instantaneous
//! frequency over 0.1–1.1 s (a beating pair's pitch is that mean); the key's pitch the partials'
//! offsets weighed by their amplitude and the ear's dominance region; each partial's beat the
//! 3–97 % range of its detrended level over 0.15–3.3 s (the round-40 lab's
//! `s24_beat_depth.py`, so its Salamander numbers compare). Allocates; tests only.

use crate::math;
use std::f64::consts::TAU;

/// One partial as measured.
#[derive(Clone, Copy, Debug)]
pub struct Partial {
    pub n: u32,
    /// Where it was measured from (Hz) and where it is (Hz).
    pub near_hz: f64,
    pub hz: f64,
    /// Its offset from `near_hz` (cents).
    pub cents: f64,
    /// Its mean power over 0.1–1.1 s (dB, relative).
    pub level_db: f64,
    /// Its beat: the 3–97 % range of its level about a quadratic fit (dB), 0.15–3.3 s.
    pub beat_db: f64,
}

/// One key as measured.
#[derive(Clone, Debug)]
pub struct KeyTune {
    pub partials: Vec<Partial>,
    /// The key's pitch against the partials' `near_hz` (cents), as the ear weighs them.
    pub pitch: f64,
    /// The median beat of the (up to eight) strongest partials (the lab's key measure).
    pub beat: f64,
    /// The deepest beat among the three strongest partials.
    pub beat_worst: f64,
}

/// The ear's weight of a partial for pitch: its amplitude, and the dominance region (Plomp
/// 1967, Ritsma 1967: the partials between ~300 Hz and ~2 kHz decide a complex tone's pitch)
/// as a log-normal bump at 700 Hz, 1.5 octaves wide, floored at a tenth.
fn dominance(hz: f64) -> f64 {
    let o = math::log2(hz / 700.0) / 1.5;
    0.1 + 0.9 * math::exp(-0.5 * o * o)
}

/// The complex envelope of `x` at `hz`: frames every `hop` samples, a Hann `len` long (its
/// first null at `2 fs / len`), with the phase of the absolute sample index — so successive
/// frames' phase advance is the partial's frequency offset.
fn envelope(x: &[f64], fs: f64, hz: f64, len: usize, hop: usize) -> Vec<(f64, f64)> {
    let w = TAU * hz / fs;
    let n = x.len();
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    for i in 0..n {
        let ph = (w * i as f64) % TAU;
        let (s, c) = math::sin_cos(ph);
        re[i] = x[i] * c;
        im[i] = -x[i] * s;
    }
    let win: Vec<f64> = (0..len)
        .map(|i| 0.5 - 0.5 * math::cos(TAU * (i as f64 + 0.5) / len as f64))
        .collect();
    let norm = 2.0 / win.iter().sum::<f64>();
    let mut out = Vec::new();
    let mut c = len / 2;
    while c + len / 2 < n {
        let s0 = c - len / 2;
        let (mut a, mut b) = (0.0, 0.0);
        for (k, wk) in win.iter().enumerate() {
            a += re[s0 + k] * wk;
            b += im[s0 + k] * wk;
        }
        out.push((a * norm, b * norm));
        c += hop;
    }
    out
}

/// Measure partials `near` (Hz each, from the first) of a stereo key struck at sample 0, at
/// `fs`. The Hann is `4 fs / f0` long (its first null half a partial's spacing away); frames
/// every 5 ms. A partial whose envelope is under −60 dB of the strongest is dropped.
pub fn measure(l: &[f64], r: &[f64], fs: f64, f0: f64, near: &[f64]) -> KeyTune {
    let hop = (0.005 * fs) as usize;
    let len = ((4.0 * fs / f0) as usize).max(32) & !1;
    let frame_t = |j: usize| (len / 2 + j * hop) as f64 / fs;
    let mut parts = Vec::new();
    for (i, &hz) in near.iter().enumerate() {
        if hz > 0.45 * fs {
            break;
        }
        let el = envelope(l, fs, hz, len, hop);
        let er = envelope(r, fs, hz, len, hop);
        // The power-weighted mean frequency: the phase of the lag-one autocorrelation.
        let (mut ar, mut ai, mut p, mut np) = (0.0, 0.0, 0.0, 0.0);
        for j in 0..el.len().saturating_sub(1) {
            let t = frame_t(j);
            if !(0.1..=1.1).contains(&t) {
                continue;
            }
            for e in [&el, &er] {
                let (x0, y0) = e[j];
                let (x1, y1) = e[j + 1];
                ar += x1 * x0 + y1 * y0;
                ai += y1 * x0 - x1 * y0;
                p += x0 * x0 + y0 * y0;
            }
            np += 1.0;
        }
        if np == 0.0 || p <= 0.0 {
            continue;
        }
        let df = math::atan2(ai, ar) * fs / (TAU * hop as f64);
        let got = hz + df;
        // The beat: the level's swing about its quadratic trend, 0.15–3.3 s.
        let lv: Vec<(f64, f64)> = el
            .iter()
            .zip(&er)
            .enumerate()
            .map(|(j, (a, b))| {
                let pw = a.0 * a.0 + a.1 * a.1 + b.0 * b.0 + b.1 * b.1;
                (frame_t(j), 10.0 * math::log10(pw + 1e-30))
            })
            .filter(|(t, _)| (0.15..=3.3).contains(t))
            .collect();
        parts.push(Partial {
            n: i as u32 + 1,
            near_hz: hz,
            hz: got,
            cents: 1200.0 * math::log2(got / hz),
            level_db: 10.0 * math::log10(p / np + 1e-30),
            beat_db: swing(&lv),
        });
    }
    let top = parts.iter().map(|p| p.level_db).fold(f64::MIN, f64::max);
    parts.retain(|p| p.level_db > top - 60.0);
    summarise(parts)
}

fn summarise(parts: Vec<Partial>) -> KeyTune {
    let (mut sw, mut sc) = (0.0, 0.0);
    for p in parts.iter().filter(|p| p.n <= 8 && p.near_hz < 5_000.0) {
        let w = math::powf(10.0f64, p.level_db / 20.0) * dominance(p.near_hz);
        sw += w;
        sc += w * p.cents;
    }
    let mut by_level: Vec<&Partial> = parts.iter().collect();
    by_level.sort_by(|a, b| b.level_db.total_cmp(&a.level_db));
    let mut beats: Vec<f64> = by_level.iter().take(8).map(|p| p.beat_db).collect();
    beats.sort_by(f64::total_cmp);
    let beat = if beats.is_empty() {
        0.0
    } else if beats.len() % 2 == 1 {
        beats[beats.len() / 2]
    } else {
        0.5 * (beats[beats.len() / 2 - 1] + beats[beats.len() / 2])
    };
    let beat_worst = by_level
        .iter()
        .take(3)
        .map(|p| p.beat_db)
        .fold(0.0, f64::max);
    KeyTune {
        pitch: if sw > 0.0 { sc / sw } else { 0.0 },
        beat,
        beat_worst,
        partials: parts,
    }
}

/// The 3–97 % range of `(t, dB)` about its least-squares quadratic.
fn swing(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 8 {
        return 0.0;
    }
    // Normal equations for y = a + b t + c t², t centred for conditioning.
    let n = pts.len() as f64;
    let tm = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let mut s = [0.0f64; 5];
    let mut sy = [0.0f64; 3];
    for (t, y) in pts {
        let u = t - tm;
        let mut uk = 1.0;
        for (k, sk) in s.iter_mut().enumerate() {
            *sk += uk;
            if k < 3 {
                sy[k] += uk * y;
            }
            uk *= u;
        }
    }
    let m = [[s[0], s[1], s[2]], [s[1], s[2], s[3]], [s[2], s[3], s[4]]];
    let coef = solve3(m, sy);
    let mut res: Vec<f64> = pts
        .iter()
        .map(|(t, y)| {
            let u = t - tm;
            y - (coef[0] + coef[1] * u + coef[2] * u * u)
        })
        .collect();
    res.sort_by(f64::total_cmp);
    let q = |f: f64| res[((res.len() - 1) as f64 * f).round() as usize];
    q(0.97) - q(0.03)
}

fn solve3(mut m: [[f64; 3]; 3], mut y: [f64; 3]) -> [f64; 3] {
    for c in 0..3 {
        let p = (c..3)
            .max_by(|a, b| m[*a][c].abs().total_cmp(&m[*b][c].abs()))
            .unwrap_or(c);
        m.swap(c, p);
        y.swap(c, p);
        for rr in 0..3 {
            if rr != c && m[c][c] != 0.0 {
                let f = m[rr][c] / m[c][c];
                for k in 0..3 {
                    m[rr][k] -= f * m[c][k];
                }
                y[rr] -= f * y[c];
            }
        }
    }
    [y[0] / m[0][0], y[1] / m[1][1], y[2] / m[2][2]]
}
