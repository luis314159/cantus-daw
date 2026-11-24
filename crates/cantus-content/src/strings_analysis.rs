//! r42:strings-rec — what the recorded strings' packs need measured offline (ADR 0158), never
//! on the audio thread: each sample's **pitch** (the octave its file name gets wrong, and the
//! cents it sits off its key), the **steady part** of a sustain, the **loop** found in it, and
//! the **click check** of a loop as Exemplar plays it.
//!
//! Two pitch estimators measure the same thing — the mean of the pitch's trajectory in cents,
//! the vibrato averaged out — by different means, so a test of one by the other is not
//! circular: [`yin_cents`] (de Cheveigné & Kawahara's YIN, the difference function's dip) sets
//! the zones' tuning; [`nsdf_cents`] (McLeod's normalised square difference, its first peak
//! near the maximum) checks what Exemplar plays.

use cantus_dsp::math;

/// A sample's channels mixed to one.
pub fn mono(samples: &[f32], channels: usize) -> Vec<f32> {
    let ch = channels.max(1);
    samples
        .chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Cents above MIDI key 0 (A440 at 6 900) of a frequency.
pub fn cents_of(hz: f64) -> f64 {
    6_900.0 + 1_200.0 * math::log2(hz / 440.0)
}

/// The frequency of a key in 12-TET at A440.
pub fn hz_of_key(key: f64) -> f64 {
    440.0 * math::exp2((key - 69.0) / 12.0)
}

/// RMS per frame of `hop` samples.
pub fn envelope(x: &[f32], hop: usize) -> Vec<f32> {
    x.chunks(hop.max(1))
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len().max(1) as f32).sqrt())
        .collect()
}

/// The steady part of a sustained note, in samples: the longest run of 50 ms frames within
/// 6 dB of the loud half's median level (a dip of one frame forgiven), its first and last
/// 50 ms left out. `None` for a note that never settles (shorter than half a second).
pub fn steady_span(x: &[f32], rate: f32) -> Option<(usize, usize)> {
    let hop = (rate * 0.05) as usize;
    let env = envelope(x, hop);
    if env.len() < 10 {
        return None;
    }
    let mut sorted = env.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median_loud = sorted[sorted.len() * 3 / 4];
    let floor = median_loud * 0.5;
    let (mut best, mut start, mut miss) = ((0usize, 0usize), None::<usize>, 0usize);
    for (i, e) in env.iter().enumerate() {
        if *e >= floor {
            start.get_or_insert(i);
            miss = 0;
        } else if let Some(s) = start {
            miss += 1;
            if miss > 1 {
                let end = i - miss;
                if end - s > best.1 - best.0 {
                    best = (s, end);
                }
                start = None;
                miss = 0;
            }
        }
    }
    if let Some(s) = start {
        let end = env.len() - 1 - miss;
        if end > s && end - s > best.1 - best.0 {
            best = (s, end);
        }
    }
    let (a, b) = (best.0 + 1, best.1.saturating_sub(1));
    (b > a + 10).then_some((a * hop, (b + 1) * hop))
}

/// Frames of analysis tiling `[from, to)`: each `len` long, `hop` apart — every part of a
/// vibrato's cycle weighed alike, so its mean is the note's centre (frames spread thinly
/// would sample a regular vibrato at a few of its phases and lean to them).
fn frames(from: usize, to: usize, len: usize, hop: usize) -> Vec<usize> {
    if to <= from + len {
        return Vec::new();
    }
    (from..=to - len).step_by(hop.max(1)).collect()
}

/// The trimmed mean (the middle half) of `v`.
fn trimmed_mean(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let q = v.len() / 4;
    let mid = &v[q..v.len() - q];
    Some(mid.iter().sum::<f64>() / mid.len() as f64)
}

/// Parabolic refinement of a dip or peak at `i` of `f`.
fn refine(f: &[f64], i: usize) -> f64 {
    if i == 0 || i + 1 >= f.len() {
        return i as f64;
    }
    let (a, b, c) = (f[i - 1], f[i], f[i + 1]);
    let d = a - 2.0 * b + c;
    if d.abs() < 1e-18 {
        i as f64
    } else {
        i as f64 + 0.5 * (a - c) / d
    }
}

/// The search range of periods (samples) for a note near `hint_hz`: from two and a half
/// times higher to two and a half times lower, so an octave off either way is found.
fn period_range(rate: f32, hint_hz: f64) -> (usize, usize) {
    let t = rate as f64 / hint_hz.max(10.0);
    (
        (t / 2.5).floor().max(2.0) as usize,
        (t * 2.5).ceil() as usize,
    )
}

/// YIN over `[from, to)` of `x`: the mean pitch in cents ([`cents_of`]) of frames tiling it,
/// the trimmed mean of the voiced ones, searching near `hint_hz`.
pub fn yin_cents(x: &[f32], rate: f32, from: usize, to: usize, hint_hz: f64) -> Option<f64> {
    let (tmin, tmax) = period_range(rate, hint_hz);
    let win = (2 * tmax).max(1_024);
    let mut out = Vec::new();
    for at in frames(from, to.min(x.len()), win + tmax + 2, win / 2) {
        let w = &x[at..at + win + tmax + 2];
        let mut d = vec![0.0f64; tmax + 2];
        for (tau, dv) in d.iter_mut().enumerate().skip(1) {
            let mut s = 0.0f64;
            for j in 0..win {
                let e = (w[j] - w[j + tau]) as f64;
                s += e * e;
            }
            *dv = s;
        }
        // The cumulative mean normalised difference.
        let mut cm = vec![1.0f64; d.len()];
        let mut run = 0.0;
        for tau in 1..d.len() {
            run += d[tau];
            cm[tau] = if run > 0.0 {
                d[tau] * tau as f64 / run
            } else {
                1.0
            };
        }
        let mut pick = None;
        let mut tau = tmin;
        while tau <= tmax {
            if cm[tau] < 0.15 {
                while tau < tmax && cm[tau + 1] < cm[tau] {
                    tau += 1;
                }
                pick = Some(tau);
                break;
            }
            tau += 1;
        }
        if let Some(t) = pick {
            let p = refine(&d, t);
            out.push(cents_of(rate as f64 / p));
        }
    }
    trimmed_mean(out)
}

/// McLeod's NSDF over `[from, to)`: the mean pitch in cents of frames tiling it, searching
/// near `hint_hz` — the key the first peak above 0.85 of the highest gives.
pub fn nsdf_cents(x: &[f32], rate: f32, from: usize, to: usize, hint_hz: f64) -> Option<f64> {
    let (tmin, tmax) = period_range(rate, hint_hz);
    let win = (3 * tmax).max(2_048);
    let mut out = Vec::new();
    for at in frames(from, to.min(x.len()), win, win / 3) {
        let w = &x[at..at + win];
        let mut n = vec![0.0f64; tmax + 2];
        for (tau, nv) in n.iter_mut().enumerate() {
            let (mut r, mut m) = (0.0f64, 0.0f64);
            for j in 0..win - tau {
                let (a, b) = (w[j] as f64, w[j + tau] as f64);
                r += a * b;
                m += a * a + b * b;
            }
            *nv = if m > 0.0 { 2.0 * r / m } else { 0.0 };
        }
        let top = n[tmin..=tmax].iter().copied().fold(f64::MIN, f64::max);
        if top < 0.5 {
            continue;
        }
        let mut tau = tmin;
        while tau < tmax {
            if n[tau] >= 0.85 * top && n[tau] >= n[tau - 1] && n[tau] >= n[tau + 1] {
                break;
            }
            tau += 1;
        }
        let p = refine(&n, tau);
        out.push(cents_of(rate as f64 / p));
    }
    trimmed_mean(out)
}

/// A loop found in a sustain: its start and end (inclusive) and crossfade, in frames, and
/// the seam's error as Exemplar plays it ([`cantus_dsp::exemplar::analysis`], dB).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LoopPoints {
    pub ls: i64,
    pub le: i64,
    pub xf: i64,
    pub err_db: f32,
}

/// The loop of a sustained sample (interleaved `data`): inside its steady part, as long as
/// the part allows (at most 60 % of it, starting 30 % in), a whole number of periods of
/// `hz`, its end and start moved within a period to the lowest seam error, crossfaded over a
/// third of its length (at most 400 ms). `None` when the note has no steady part a loop of
/// half a second fits.
pub fn find_loop(data: &[f32], channels: usize, rate: f32, hz: f64) -> Option<LoopPoints> {
    find_loop_at(data, channels, rate, hz, 0.3, 0.6)
}

/// [`find_loop`] starting `start` of the way into the steady part and `len` of it long (the
/// next try when a loop clicks: another place, a shorter loop).
pub fn find_loop_at(
    data: &[f32],
    channels: usize,
    rate: f32,
    hz: f64,
    start: f64,
    len: f64,
) -> Option<LoopPoints> {
    use cantus_dsp::exemplar::analysis::{find_best_loop, seam_error_xf_db};
    let m = mono(data, channels);
    let (a, b) = steady_span(&m, rate)?;
    let span = (b - a) as f64;
    let ls = (a as f64 + span * start) as i64;
    let period = rate as f64 / hz.max(10.0);
    let want = (span * len).min(span * (0.95 - start));
    if want < rate as f64 * 0.5 {
        return None;
    }
    let len = ((want / period).floor().max(1.0) * period).round() as i64;
    let le = ls + len;
    let search = period.ceil() as i64;
    let (ls, le, _) = find_best_loop(&m, 1, ls, le, search);
    let xf = ((le - ls) / 3).min((rate * 0.4) as i64).min(ls);
    let err_db = seam_error_xf_db(&m, 1, ls, le, 256, xf);
    Some(LoopPoints { ls, le, xf, err_db })
}

/// The click check: in `out` (mono, as rendered), the energy of the second difference —
/// a high-pass rising 12 dB an octave, what a click is made of — in 5 ms frames, at the
/// frames that hold a seam against the 90th percentile of the others. Returns the worst
/// seam's excess in dB (0 or less: no louder than the loop's own playing).
pub fn seam_excess_db(out: &[f32], rate: f32, seams: &[usize]) -> f32 {
    let hop = ((rate * 0.005) as usize).max(16);
    let hp: Vec<f32> = (2..out.len())
        .map(|i| out[i] - 2.0 * out[i - 1] + out[i - 2])
        .collect();
    let env = envelope(&hp, hop);
    let at_seam = |f: usize| {
        seams
            .iter()
            .any(|s| s / hop == f || (s / hop).abs_diff(f) <= 1)
    };
    let mut rest: Vec<f32> = env
        .iter()
        .enumerate()
        .filter(|(f, _)| !at_seam(*f))
        .map(|(_, e)| *e)
        .collect();
    if rest.is_empty() {
        return 0.0;
    }
    rest.sort_by(|a, b| a.total_cmp(b));
    let p90 = rest[rest.len() * 9 / 10].max(1e-9);
    seams
        .iter()
        .filter_map(|s| env.get(s / hop))
        .map(|e| 20.0 * math::log10((*e).max(1e-9) / p90))
        .fold(f32::MIN, f32::max)
}

/// How far either side of a zone's mapped root its pitch is searched, in cents: wide enough
/// for any player's intonation, too narrow for an octave or a fifth (the root is known: the
/// note the zone maps, after the octave fix).
pub const NEAR_CENTS: f64 = 60.0;

/// The mean of `v`: the pitch's trajectory averaged over time, every frame alike — what two
/// measures of it agree on however long their frames (a trimmed mean would cut a vibrato's
/// or a tremolo's extremes differently for each).
fn trimmed_mean80(v: Vec<f64>) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The power of the windowed frame `w` at frequency `f` (cycles per sample): one DFT bin at
/// any frequency, by a rotating phasor.
fn power_at(w: &[f64], f: f64) -> f64 {
    let (s, c) = math::sin_cos(-std::f64::consts::TAU * f);
    let (mut re, mut im) = (0.0f64, 0.0f64);
    let (mut pr, mut pi) = (1.0f64, 0.0f64);
    for (n, x) in w.iter().enumerate() {
        re += x * pr;
        im += x * pi;
        let r = pr * c - pi * s;
        pi = pr * s + pi * c;
        pr = r;
        // Renormalise now and then: the phasor stays on the circle.
        if n % 1024 == 1023 {
            let m = (pr * pr + pi * pi).sqrt();
            pr /= m;
            pi /= m;
        }
    }
    re * re + im * im
}

/// The voiced frames tiling `[from, to)`: their starts, `len` long and `hop` apart, those whose
/// level is within 20 dB of the loudest.
fn voiced(x: &[f32], from: usize, to: usize, len: usize, hop: usize) -> Vec<usize> {
    let at = frames(from, to.min(x.len()), len, hop);
    let rms: Vec<f32> = at
        .iter()
        .map(|a| (x[*a..a + len].iter().map(|v| v * v).sum::<f32>() / len as f32).sqrt())
        .collect();
    let top = rms.iter().copied().fold(0.0f32, f32::max);
    at.into_iter()
        .zip(rms)
        .filter(|(_, r)| *r > 0.0 && *r >= top * 0.1)
        .map(|(a, _)| a)
        .collect()
}

/// **The partials' fit** — the pitch whose first harmonics (up to eight, below 0.45 of the
/// rate) hold the most power, frame by frame over `[from, to)` (Hann windows of at least
/// eight periods, half overlapping), searched within [`NEAR_CENTS`] of `root_hz`: a coarse
/// grid of 8 cents on four partials, then a golden-section search on eight to 0.02 cent. The mean of the voiced frames, in cents ([`cents_of`]) — the note's mean pitch, its vibrato kept.
/// What a zone's tuning is set from.
pub fn harmonic_cents(x: &[f32], rate: f32, from: usize, to: usize, root_hz: f64) -> Option<f64> {
    harmonic_cents_k(x, rate, from, to, root_hz, PARTIALS)
}

/// The partials the fit weighs: the low ones that set a string's pitch, before its stiffness
/// stretches the upper ones sharp.
pub const PARTIALS: usize = 8;

/// [`harmonic_cents`] over the first `partials` harmonics.
pub fn harmonic_cents_k(
    x: &[f32],
    rate: f32,
    from: usize,
    to: usize,
    root_hz: f64,
    partials: usize,
) -> Option<f64> {
    let period = rate as f64 / root_hz.max(10.0);
    let len = ((8.0 * period).ceil() as usize).max(4_096);
    let win: Vec<f64> = (0..len)
        .map(|n| 0.5 - 0.5 * math::cos(std::f64::consts::TAU * n as f64 / len as f64))
        .collect();
    let mut out = Vec::new();
    for at in voiced(x, from, to, len, len / 2) {
        let w: Vec<f64> = x[at..at + len]
            .iter()
            .zip(&win)
            .map(|(v, h)| *v as f64 * h)
            .collect();
        let score = |cents: f64, most: usize| -> f64 {
            let f = root_hz * math::exp2(cents / 1_200.0) / rate as f64;
            let k_max = ((0.45 / f) as usize).clamp(1, most);
            (1..=k_max).map(|k| power_at(&w, f * k as f64)).sum()
        };
        // Coarse: four partials on an 8-cent grid; then all eight, golden-section.
        let mut best = (f64::MIN, 0.0);
        let mut c = -NEAR_CENTS;
        while c <= NEAR_CENTS {
            let v = score(c, partials.min(4));
            if v > best.0 {
                best = (v, c);
            }
            c += 8.0;
        }
        let score = |cents: f64| score(cents, partials);
        // Golden section within a grid step either side.
        let g = 0.618_033_988_75;
        let (mut a, mut b) = (best.1 - 8.0, best.1 + 8.0);
        let (mut c1, mut c2) = (b - g * (b - a), a + g * (b - a));
        let (mut f1, mut f2) = (score(c1), score(c2));
        while b - a > 0.02 {
            if f1 > f2 {
                b = c2;
                c2 = c1;
                f2 = f1;
                c1 = b - g * (b - a);
                f1 = score(c1);
            } else {
                a = c1;
                c1 = c2;
                f1 = f2;
                c2 = a + g * (b - a);
                f2 = score(c2);
            }
        }
        out.push(cents_of(root_hz) + 0.5 * (a + b));
    }
    trimmed_mean80(out)
}

/// **The held-out check** — McLeod's normalised square difference over `[from, to)`, its
/// highest peak among the periods within [`NEAR_CENTS`] (and a margin) of `root_hz`,
/// parabolically refined; frames of at least four periods tiling the span a third apart, the
/// voiced ones; their mean in cents. Never used to set a tuning: what
/// checks one.
pub fn nsdf_near_cents(x: &[f32], rate: f32, from: usize, to: usize, root_hz: f64) -> Option<f64> {
    nsdf_near_cents_w(x, rate, from, to, root_hz, 4.0, 2_048)
}

/// [`nsdf_near_cents`] with frames of `periods` periods, at least `min` samples.
pub fn nsdf_near_cents_w(
    x: &[f32],
    rate: f32,
    from: usize,
    to: usize,
    root_hz: f64,
    periods: f64,
    min: usize,
) -> Option<f64> {
    let t = rate as f64 / root_hz.max(10.0);
    let span = math::exp2((NEAR_CENTS + 20.0) / 1_200.0);
    let (tmin, tmax) = (
        (t / span).floor().max(2.0) as usize,
        (t * span).ceil() as usize,
    );
    let win = ((periods * t).ceil() as usize).max(min);
    let mut out = Vec::new();
    for at in voiced(x, from, to, win + tmax + 2, win / 3) {
        let w = &x[at..at + win + tmax + 2];
        let mut n = vec![0.0f64; tmax + 2];
        for (tau, nv) in n
            .iter_mut()
            .enumerate()
            .take(tmax + 2)
            .skip(tmin.saturating_sub(1))
        {
            let (mut r, mut m) = (0.0f64, 0.0f64);
            for j in 0..win {
                let (a, b) = (w[j] as f64, w[j + tau] as f64);
                r += a * b;
                m += a * a + b * b;
            }
            *nv = if m > 0.0 { 2.0 * r / m } else { 0.0 };
        }
        let mut best = tmin;
        for tau in tmin..=tmax {
            if n[tau] > n[best] {
                best = tau;
            }
        }
        if n[best] < 0.3 {
            continue;
        }
        let p = refine(&n, best);
        out.push(cents_of(rate as f64 / p));
    }
    trimmed_mean80(out)
}

/// Where a short note's tone is measured: from 20 ms after its loudest moment's rise began
/// to where it has fallen 30 dB below its peak (at most 1.5 s), in samples.
pub fn tone_span(x: &[f32], rate: f32) -> (usize, usize) {
    let hop = (rate * 0.01) as usize;
    let env = envelope(x, hop.max(1));
    let (peak_i, peak) = env
        .iter()
        .copied()
        .enumerate()
        .fold((0, 0.0f32), |m, (i, e)| if e > m.1 { (i, e) } else { m });
    let start = (peak_i * hop).saturating_sub(hop) + (rate * 0.02) as usize;
    let end_i = env
        .iter()
        .enumerate()
        .skip(peak_i)
        .find(|(_, e)| **e < peak * 0.0316)
        .map_or(env.len(), |(i, _)| i);
    let end = (end_i * hop)
        .min(start + (rate * 1.5) as usize)
        .min(x.len());
    (start.min(end), end)
}

/// What a held zone plays at its root, untuned, as Exemplar plays it (its loop crossfaded):
/// three turns of its loop, or the whole file; and the span its pitch is read over (the
/// turns, from just after the loop starts; or the steady part).
pub fn held_play(x: &[f32], rate: f32, lp: Option<[i64; 2]>, xf: i64) -> (Vec<f32>, usize, usize) {
    let mut levels: [Option<std::sync::Arc<[f32]>>; cantus_dsp::exemplar::resample::MAX_LEVELS] =
        Default::default();
    levels[0] = Some(std::sync::Arc::from(x.to_vec()));
    let src = cantus_dsp::exemplar::Source {
        levels,
        channels: 1,
        rate,
        frames: x.len(),
        tune_ct: 0.0,
        stream: None,
    };
    let mut e = cantus_dsp::exemplar::Exemplar::new(rate);
    e.set_globals(&cantus_dsp::exemplar::Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 50.0],
        vel_amp: 0.0,
        ..cantus_dsp::exemplar::Globals::default()
    });
    let region = match lp {
        Some([ls, le]) => cantus_dsp::exemplar::Region {
            end: x.len() as i64,
            looping: true,
            ls,
            le,
            xf,
            ..cantus_dsp::exemplar::Region::default()
        },
        None => cantus_dsp::exemplar::Region {
            end: x.len() as i64,
            ..cantus_dsp::exemplar::Region::default()
        },
    };
    e.put_zone(
        0,
        &cantus_dsp::exemplar::ZoneParams {
            root: 60,
            region,
            ..cantus_dsp::exemplar::ZoneParams::default()
        },
        &src,
    );
    e.note_on(60, 1.0, 1);
    let n = match lp {
        Some([ls, le]) => (ls + 3 * (le - ls + 1) + 2_048) as usize,
        None => x.len(),
    };
    let mut out = Vec::with_capacity(n);
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    while out.len() < n {
        l.fill(0.0);
        r.fill(0.0);
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    let (a, b) = match lp {
        Some([ls, le]) => (ls as usize + 256, (ls + 3 * (le - ls + 1)) as usize),
        None => steady_span(&out, rate).unwrap_or((0, out.len())),
    };
    (out, a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, rate: f32, secs: f32, vib_ct: f64) -> Vec<f32> {
        let n = (rate * secs) as usize;
        let mut ph = 0.0f64;
        (0..n)
            .map(|i| {
                let t = i as f64 / rate as f64;
                let f =
                    hz * math::exp2(vib_ct / 1_200.0 * math::sin(std::f64::consts::TAU * 5.5 * t));
                ph += f / rate as f64;
                let p = std::f64::consts::TAU * ph;
                (0.5 * math::sin(p) + 0.25 * math::sin(2.0 * p) + 0.12 * math::sin(3.0 * p)) as f32
            })
            .collect()
    }

    #[test]
    fn both_estimators_find_a_tone_and_its_octave_and_agree_within_a_cent() {
        // 65.4 Hz (C2) named an octave low; a violin's A4 with ±20 cents of vibrato.
        for (hz, hint, vib) in [(65.406, 32.7, 0.0), (440.0 * 1.0029, 440.0, 20.0)] {
            let x = tone(hz, 44_100.0, 3.0, vib);
            let truth = cents_of(hz);
            let y = yin_cents(&x, 44_100.0, 4_410, x.len(), hint).unwrap();
            let n = nsdf_cents(&x, 44_100.0, 4_410, x.len(), hint).unwrap();
            assert!((y - truth).abs() < 1.0, "YIN {y} vs {truth}");
            assert!((n - truth).abs() < 1.0, "NSDF {n} vs {truth}");
        }
    }

    #[test]
    fn a_steady_tone_loops_seamlessly_and_a_click_is_caught() {
        let rate = 44_100.0;
        let mut x = tone(110.0, rate, 4.0, 0.0);
        // An attack and a fade-out around a steady middle.
        let n = x.len();
        for (i, v) in x.iter_mut().enumerate() {
            let a = (i as f32 / (0.2 * rate)).min(1.0);
            let r = ((n - i) as f32 / (0.4 * rate)).min(1.0);
            *v *= a * r;
        }
        let lp = find_loop(&x, 1, rate, 110.0).expect("a loop");
        assert!(lp.err_db < -40.0, "{lp:?}");
        assert!(lp.le - lp.ls > (0.5 * rate) as i64);
        // The checker: a steady signal has no excess; a step in it does.
        let steady = tone(220.0, rate, 1.0, 0.0);
        assert!(seam_excess_db(&steady, rate, &[22_050]) < 3.0);
        let mut clicked = steady.clone();
        for v in &mut clicked[22_050..] {
            *v += 0.3;
        }
        assert!(seam_excess_db(&clicked, rate, &[22_050]) > 6.0);
    }

    #[test]
    fn the_partials_fit_and_the_held_out_check_find_the_mean_pitch_within_half_a_cent() {
        // A cello's C2 whose fundamental is weak (the second partial louder), a few cents
        // sharp, with vibrato; and a violin's A4 flat.
        for (hz, root, vib) in [(65.406 * 1.0021, 36.0, 15.0), (440.0 * 0.9968, 69.0, 25.0)] {
            let n = 3 * 44_100;
            let mut ph = 0.0f64;
            let x: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f64 / 44_100.0;
                    let f =
                        hz * math::exp2(vib / 1_200.0 * math::sin(std::f64::consts::TAU * 5.5 * t));
                    ph += f / 44_100.0;
                    let p = std::f64::consts::TAU * ph;
                    (0.1 * math::sin(p) + 0.5 * math::sin(2.0 * p) + 0.3 * math::sin(3.0 * p))
                        as f32
                })
                .collect();
            let truth = cents_of(hz);
            let r = hz_of_key(root);
            let h = harmonic_cents(&x, 44_100.0, 0, n, r).unwrap();
            let c = nsdf_near_cents(&x, 44_100.0, 0, n, r).unwrap();
            assert!((h - truth).abs() < 0.5, "the fit {h} vs {truth}");
            assert!((c - truth).abs() < 0.5, "the check {c} vs {truth}");
            assert!((h - c).abs() < 0.5, "the fit {h} and the check {c}");
        }
    }
}
