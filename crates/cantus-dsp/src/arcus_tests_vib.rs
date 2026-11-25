//! r42:arcus-p5 — the vibrato measured as the study measures string players'
//! (`tools/arcus/pass5/vibrato.py`, the same steps in Rust): the study's tracker over two
//! periods every 2.5 ms, the cents against the note's median, the drift taken out over two
//! vibrato periods, the cycles cut at alternate extrema — each cycle's rate and its extent
//! peak to trough; the onset (the first cycle at a quarter of the full extent), the rise
//! (to the first at 90 %), the cycle-to-cycle jitter of the period and of the extent.

use super::tests::cents;
use super::tests_p5::yin_frames_p;
use crate::math;

/// One note's vibrato as `vibrato.py` reports it.
#[derive(Clone, Debug, Default)]
pub(super) struct Vib {
    pub rate_hz: f64,
    pub extent_pp: f64,
    pub onset_s: f64,
    pub rise_s: f64,
    pub rate_jitter: f64,
    pub extent_jitter: f64,
    pub drift_hz_s: f64,
    pub cycles: usize,
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        f64::NAN
    } else {
        v[v.len() / 2]
    }
}

/// `detrend_at`: `v` less its moving average two vibrato periods long (edges held).
fn detrend(v: &[f64], dt: f64, rate: f64) -> Vec<f64> {
    let k = ((2.0 / rate / dt).round() as usize).max(3);
    let (l, r) = (k / 2, k - 1 - k / 2);
    let n = v.len();
    let at = |i: isize| v[i.clamp(0, n as isize - 1) as usize];
    (0..n)
        .map(|i| {
            let s: f64 = (-(l as isize)..=r as isize)
                .map(|j| at(i as isize + j))
                .sum();
            v[i] - s / k as f64
        })
        .collect()
}

/// The note's vibrato from `x` (the note `want` Hz sounding from `t_on` to `t_end` s), or
/// `None` when it has fewer than three cycles.
pub(super) fn vibrato_of(x: &[f32], want: f64, t_on: f64, t_end: f64) -> Option<Vib> {
    let fr = yin_frames_2(x, want, t_on, t_end);
    if fr.len() < 60 {
        return None;
    }
    let dt = 0.0025;
    let t: Vec<f64> = (0..fr.len()).map(|i| t_on + i as f64 * dt).collect();
    let mut c: Vec<f64> = fr.clone();
    let m = median(c.clone());
    for v in c.iter_mut() {
        *v -= m;
    }
    // The rate: the spectral peak of the second half (a line taken out) in 3–9 Hz.
    let h = c.len() / 2;
    let (ts, cs) = (&t[h..], &c[h..]);
    let n = ts.len() as f64;
    let (mt, mc) = (ts.iter().sum::<f64>() / n, cs.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (a, b) in ts.iter().zip(cs) {
        num += (a - mt) * (b - mc);
        den += (a - mt) * (a - mt);
    }
    let slope = num / den.max(1e-12);
    let v0: Vec<f64> = ts
        .iter()
        .zip(cs)
        .map(|(a, b)| b - mc - slope * (a - mt))
        .collect();
    let (mut r0, mut best) = (6.0, 0.0);
    let mut f = 3.0;
    while f <= 9.0 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, (a, b)) in ts.iter().zip(&v0).enumerate() {
            let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / (n - 1.0));
            let (sn, cs) = math::sin_cos(std::f64::consts::TAU * f * a);
            re += w * b * cs;
            im += w * b * sn;
        }
        if re * re + im * im > best {
            best = re * re + im * im;
            r0 = f;
        }
        f += 0.01;
    }
    let v = detrend(&c, dt, r0);
    // Alternate extrema at least a third of a period apart.
    let gap = ((0.33 / r0 / dt) as usize).max(2);
    let mut ex: Vec<(usize, i32)> = Vec::new();
    let mut i = 1;
    while i + 1 < v.len() {
        let (lo, hi) = (i.saturating_sub(gap), (i + gap + 1).min(v.len()));
        let w = &v[lo..hi];
        let kind = if v[i] >= w.iter().cloned().fold(f64::MIN, f64::max) && v[i] > 0.0 {
            1
        } else if v[i] <= w.iter().cloned().fold(f64::MAX, f64::min) && v[i] < 0.0 {
            -1
        } else {
            i += 1;
            continue;
        };
        match ex.last_mut() {
            Some(last) if last.1 == kind => {
                if (kind == 1 && v[i] > v[last.0]) || (kind == -1 && v[i] < v[last.0]) {
                    *last = (i, kind);
                }
            }
            _ => ex.push((i, kind)),
        }
        i += gap;
    }
    let mut cyc: Vec<(f64, f64, f64)> = Vec::new();
    for k in 0..ex.len().saturating_sub(2) {
        if ex[k].1 == 1 && ex[k + 1].1 == -1 && ex[k + 2].1 == 1 {
            let (i0, i1, i2) = (ex[k].0, ex[k + 1].0, ex[k + 2].0);
            let per = t[i2] - t[i0];
            if per > 0.08 && per < 0.4 {
                cyc.push((0.5 * (t[i0] + t[i2]), per, 0.5 * (v[i0] + v[i2]) - v[i1]));
            }
        }
    }
    if cyc.len() < 3 {
        return None;
    }
    let mut ext: Vec<f64> = cyc.iter().map(|q| q.2).collect();
    ext.sort_by(f64::total_cmp);
    let full = median(ext[ext.len() / 2..].to_vec());
    // The first 100 ms are the attack's own pitch (a glide into the note), not a cycle.
    let late = |q: &(f64, f64, f64)| q.0 >= t_on + 0.1;
    let k25 = cyc.iter().position(|q| late(q) && q.2 >= 0.25 * full)?;
    let k90 = cyc.iter().position(|q| late(q) && q.2 >= 0.9 * full)?;
    let onset = cyc[k25].0 - 0.5 * cyc[k25].1 - t_on;
    let rise = cyc[k90].0 - cyc[k25].0;
    let on: Vec<&(f64, f64, f64)> = cyc.iter().filter(|q| q.0 >= cyc[k90].0).collect();
    let pers: Vec<f64> = on.iter().map(|q| q.1).collect();
    let exts: Vec<f64> = on.iter().map(|q| q.2).collect();
    let jit = |v: &[f64]| -> f64 {
        if v.len() < 3 {
            return f64::NAN;
        }
        let d: f64 = v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (v.len() - 1) as f64;
        d / (v.iter().sum::<f64>() / v.len() as f64)
    };
    let rates: Vec<f64> = pers.iter().map(|p| 1.0 / p).collect();
    let drift = if on.len() >= 4 {
        let n = on.len() as f64;
        let mt = on.iter().map(|q| q.0).sum::<f64>() / n;
        let mr = rates.iter().sum::<f64>() / n;
        let (mut a, mut b) = (0.0, 0.0);
        for (q, r) in on.iter().zip(&rates) {
            a += (q.0 - mt) * (r - mr);
            b += (q.0 - mt) * (q.0 - mt);
        }
        a / b.max(1e-12)
    } else {
        f64::NAN
    };
    Some(Vib {
        rate_hz: median(rates),
        extent_pp: full,
        onset_s: onset,
        rise_s: rise,
        rate_jitter: jit(&pers),
        extent_jitter: jit(&exts),
        drift_hz_s: drift,
        cycles: cyc.len(),
    })
}

/// The study's `_track`: YIN over two periods (here searched ±12 % of the note) every
/// 2.5 ms, the cents of each frame (aperiodic frames held at the last good one).
fn yin_frames_2(x: &[f32], want: f64, from: f64, to: f64) -> Vec<f64> {
    let fr = yin_frames_p(x, want, from, to, 0.0025, 2.0);
    let mut out = Vec::with_capacity(fr.len());
    let mut last = 0.0;
    for (c, ap) in fr {
        if ap < 0.3 && c.is_finite() {
            last = c;
        }
        out.push(last);
    }
    let _ = cents(1.0, 1.0);
    // a median of five, as the study's
    (0..out.len())
        .map(|i| {
            let a = i.saturating_sub(2);
            let b = (i + 3).min(out.len());
            median(out[a..b].to_vec())
        })
        .collect()
}

/// A held note of `s` at `key`, velocity `vel`, `len` s (CC1 at `cc` when given): the left
/// channel.
pub(super) fn vib_note(
    s: &super::ArcusSettings,
    key: u8,
    vel: f32,
    cc: Option<f32>,
    len: f64,
) -> Vec<f32> {
    use super::tests::Ev;
    let mut score = Vec::new();
    if let Some(c) = cc {
        score.push((0.0, Ev::Cc(0, c)));
    }
    score.push((0.01, Ev::On(key, vel)));
    score.push((0.01 + len, Ev::Off(key)));
    super::tests_p5::play(s, &score, len + 0.05)
}

/// A probe (ignored): the vibrato every instrument plays now, by `vibrato_of`, at three
/// registers and two dynamics, held 3 s — and, with `ARCUS_DUMP=<folder>`, the renders for
/// `vibrato.py` to measure the same.
#[test]
#[ignore]
fn probe_vibrato() {
    use super::tables::Instrument;
    let dump = std::env::var("ARCUS_DUMP").ok();
    for inst in Instrument::ALL {
        let low = inst.spec().bottom();
        for oct in [0u8, 1, 2] {
            for vel in [0.35f32, 0.85] {
                let key = low + 7 + 12 * oct;
                let s = super::ArcusSettings {
                    instrument: inst,
                    dyn_src: super::DynSource::Velocity,
                    ..super::ArcusSettings::default()
                };
                let x = vib_note(&s, key, vel, None, 3.0);
                let f = super::tests_p5::hz(key as f64);
                match vibrato_of(&x, f, 0.01, 3.0) {
                    Some(v) => println!(
                        "{inst:?} key {key} v {vel}: rate {:.2} Hz, extent {:.1} ct p-p, onset {:.2} s, rise {:.2} s, jitter rate {:.3} extent {:.3}, drift {:+.2} Hz/s, {} cycles",
                        v.rate_hz, v.extent_pp, v.onset_s, v.rise_s, v.rate_jitter, v.extent_jitter, v.drift_hz_s, v.cycles
                    ),
                    None => println!("{inst:?} key {key} v {vel}: no vibrato"),
                }
                if let Some(d) = &dump {
                    let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
                    std::fs::write(
                        std::path::Path::new(d).join(format!("vib-{inst:?}-{key}-{vel}.f32")),
                        bytes,
                    )
                    .unwrap();
                }
            }
        }
    }
}

/// The study's band for a family's long notes (`research/instruments-next/bowed-refdb/
/// vibrato.json`, `long`: notes of 2 s or more, every dynamic; p25 … p75): rate (Hz),
/// extent (cents peak to peak), onset (s), rate and extent jitter.
pub(super) struct Band {
    pub rate: (f64, f64),
    pub extent: (f64, f64),
    pub onset: (f64, f64),
    pub rate_jitter: (f64, f64),
    pub extent_jitter: (f64, f64),
}

pub(super) fn band(inst: super::tables::Instrument) -> Band {
    use super::tables::Instrument::*;
    match inst {
        Cello => Band {
            rate: (4.89, 5.42),
            extent: (17.2, 32.8),
            onset: (0.127, 0.471),
            rate_jitter: (0.079, 0.167),
            extent_jitter: (0.097, 0.164),
        },
        Bass => Band {
            rate: (4.31, 5.42),
            extent: (12.1, 22.2),
            onset: (0.104, 0.555),
            rate_jitter: (0.154, 0.319),
            extent_jitter: (0.191, 0.357),
        },
        Viola => Band {
            rate: (5.28, 5.57),
            extent: (19.5, 32.4),
            onset: (0.100, 0.411),
            rate_jitter: (0.088, 0.237),
            extent_jitter: (0.096, 0.165),
        },
        Violin => Band {
            rate: (4.89, 5.35),
            extent: (17.1, 28.2),
            onset: (0.152, 0.603),
            rate_jitter: (0.106, 0.232),
            extent_jitter: (0.110, 0.183),
        },
    }
}

/// Every instrument's long notes (3 s) at three registers, three dynamics and two seeds,
/// measured: (the medians of rate, extent, onset, rate jitter, extent jitter; the soft
/// notes' and the loud notes' median extent and rate).
pub(super) fn vibrato_medians(
    inst: super::tables::Instrument,
) -> ([f64; 5], (f64, f64), (f64, f64)) {
    let low = inst.spec().bottom();
    let mut rows: Vec<(f32, Vib)> = Vec::new();
    for oct in [0u8, 1, 2] {
        for vel in [0.3f32, 0.55, 0.85] {
            for seed in [0u32, 7, 13, 21] {
                let key = low + 7 + 12 * oct;
                let s = super::ArcusSettings {
                    instrument: inst,
                    dyn_src: super::DynSource::Velocity,
                    seed,
                    ..super::ArcusSettings::default()
                };
                let x = vib_note(&s, key, vel, None, 3.0);
                if let Some(v) = vibrato_of(&x, super::tests_p5::hz(key as f64), 0.01, 3.0) {
                    rows.push((vel, v));
                }
            }
        }
    }
    let med = |f: &dyn Fn(&Vib) -> f64, sel: &dyn Fn(f32) -> bool| {
        median(
            rows.iter()
                .filter(|r| sel(r.0))
                .map(|r| f(&r.1))
                .filter(|x| x.is_finite())
                .collect(),
        )
    };
    let all = |_: f32| true;
    (
        [
            med(&|v| v.rate_hz, &all),
            med(&|v| v.extent_pp, &all),
            med(&|v| v.onset_s, &all),
            med(&|v| v.rate_jitter, &all),
            med(&|v| v.extent_jitter, &all),
        ],
        (
            med(&|v| v.extent_pp, &|d| d < 0.4),
            med(&|v| v.extent_pp, &|d| d > 0.8),
        ),
        (
            med(&|v| v.rate_hz, &|d| d < 0.4),
            med(&|v| v.rate_hz, &|d| d > 0.8),
        ),
    )
}

/// A probe (ignored): each instrument's medians against the study's band.
#[test]
#[ignore]
fn probe_vibrato_band() {
    use super::tables::Instrument;
    for inst in Instrument::ALL {
        let (m, e, r) = vibrato_medians(inst);
        let b = band(inst);
        println!(
            "{inst:?}: rate {:.2} {:?} | extent {:.1} {:?} | onset {:.2} {:?} | jitter rate {:.3} {:?} extent {:.3} {:?} | extent soft {:.1} loud {:.1} | rate soft {:.2} loud {:.2}",
            m[0], b.rate, m[1], b.extent, m[2], b.onset, m[3], b.rate_jitter, m[4], b.extent_jitter, e.0, e.1, r.0, r.1
        );
    }
}

/// The vibrato's gate (pass 5, the owner's ask: a natural vibrato): on every instrument the
/// long notes' vibrato — measured as the study measures the players' — has its rate, its
/// extent, its onset and its cycle-to-cycle jitter of rate and extent inside the study's
/// band (p25 … p75 of the family's long notes), and it widens with the dynamic (the loud
/// notes' extent at least 1.2 × the soft notes', the players' 1.4 – 1.9 ×) and never
/// slows with it. Round 42's model 2 was metronomic (rate jitter 0.02–0.06 on the cello
/// and the viola against the players' 0.08–0.24) and widened only 1.25 × soft to loud.
#[test]
fn the_vibrato_is_the_players() {
    use super::tables::Instrument;
    for inst in Instrument::ALL {
        let (m, e, r) = vibrato_medians(inst);
        let b = band(inst);
        let inside = |x: f64, (lo, hi): (f64, f64)| x >= lo && x <= hi;
        println!(
            "{inst:?}: rate {:.2} {:?}, extent {:.1} {:?}, onset {:.2} {:?}, jitter {:.3} {:?} / {:.3} {:?}; extent soft {:.1} loud {:.1}; rate soft {:.2} loud {:.2}",
            m[0], b.rate, m[1], b.extent, m[2], b.onset, m[3], b.rate_jitter, m[4], b.extent_jitter, e.0, e.1, r.0, r.1
        );
        assert!(inside(m[0], b.rate), "{inst:?}: rate {:.2} Hz", m[0]);
        assert!(inside(m[1], b.extent), "{inst:?}: extent {:.1} cents", m[1]);
        assert!(inside(m[2], b.onset), "{inst:?}: onset {:.2} s", m[2]);
        assert!(
            inside(m[3], b.rate_jitter),
            "{inst:?}: rate jitter {:.3}",
            m[3]
        );
        assert!(
            inside(m[4], b.extent_jitter),
            "{inst:?}: extent jitter {:.3}",
            m[4]
        );
        assert!(
            e.1 >= 1.2 * e.0,
            "{inst:?}: extent soft {:.1} loud {:.1}",
            e.0,
            e.1
        );
        assert!(r.1 >= r.0, "{inst:?}: rate soft {:.2} loud {:.2}", r.0, r.1);
    }
}
