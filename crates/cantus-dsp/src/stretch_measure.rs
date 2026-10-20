//! Measurements of a stretch (ADR 0061's bench, the tests and `cantus-bench`): transient
//! sharpness, pre-echo, timing, pitch, phasiness. Plain arithmetic on whole buffers.

use crate::math;

/// The index of the largest `|x|` within `radius` frames of `around`.
pub fn peak_near(x: &[f32], around: f64, radius: usize) -> Option<usize> {
    let c = around.round() as i64;
    let lo = (c - radius as i64).max(0) as usize;
    let hi = ((c + radius as i64 + 1).max(0) as usize).min(x.len());
    (lo..hi).max_by(|&a, &b| x[a].abs().total_cmp(&x[b].abs()))
}

/// The first frame at or after `from` (and before `to`) where `|x|` reaches `share` of the
/// stretch's peak: where an attack is heard to start.
pub fn rise_at(x: &[f32], from: usize, to: usize, share: f32) -> Option<usize> {
    let to = to.min(x.len());
    let peak = x[from.min(to)..to]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()));
    if peak <= 0.0 {
        return None;
    }
    (from..to).find(|&i| x[i].abs() >= share * peak)
}

/// The attack time in frames: from 10 % to 90 % of the smoothed envelope's peak in
/// `from..to` (the envelope a 0.5 ms one-pole follower makes of `|x|`).
pub fn attack_frames(x: &[f32], rate: f32, from: usize, to: usize) -> Option<usize> {
    let to = to.min(x.len());
    if from >= to {
        return None;
    }
    let a = math::exp(-1.0 / (0.0005 * rate));
    let mut e = 0.0f32;
    let env: Vec<f32> = x[from..to]
        .iter()
        .map(|v| {
            e = v.abs().max(a * e);
            e
        })
        .collect();
    let peak = env.iter().fold(0.0f32, |m, v| m.max(*v));
    if peak <= 0.0 {
        return None;
    }
    let lo = env.iter().position(|v| *v >= 0.1 * peak)?;
    let hi = env.iter().position(|v| *v >= 0.9 * peak)?;
    Some(hi.saturating_sub(lo))
}

/// Half the vocoder's long frame at `rate` (frames): how far before a hit a frame reaches,
/// so how far back pre-echo is measured (round 29's third review: 25 ms saw a third of it).
pub fn half_frame(rate: f32) -> usize {
    super::pv::frame_len(rate) / 2
}

/// Energy before a transient relative to the transient itself, dB: the stretch
/// `onset − window .. onset − guard` against `onset .. onset + window`. A clean stretch
/// leaves the first far below the second (pre-echo is the smear a long window spreads in
/// front of a hit).
pub fn pre_echo_db(x: &[f32], onset: usize, window: usize, guard: usize) -> f32 {
    let e = |a: usize, b: usize| -> f64 {
        x[a.min(x.len())..b.min(x.len())]
            .iter()
            .map(|v| (*v as f64) * (*v as f64))
            .sum()
    };
    let before = e(onset.saturating_sub(window), onset.saturating_sub(guard));
    let after = e(onset, onset + window);
    (10.0 * math::log10(before.max(1e-20) / after.max(1e-20))) as f32
}

/// Cents from `target` to `measured`.
pub fn cents(measured: f32, target: f32) -> f32 {
    1200.0 * math::log2(measured.max(1e-9) / target.max(1e-9))
}

/// The amplitude (linear) of `x` at `hz` over `x`, Hann-windowed Goertzel.
pub fn amplitude(x: &[f32], rate: f32, hz: f32) -> f32 {
    let n = x.len();
    if n == 0 {
        return 0.0;
    }
    let w = 2.0 * std::f64::consts::PI * hz as f64 / rate as f64;
    let c = 2.0 * math::cos(w);
    let (mut s1, mut s2, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, &v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / n as f64);
        wsum += win;
        let s0 = v as f64 * win + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let p = s1 * s1 + s2 * s2 - c * s1 * s2;
    (2.0 * p.max(0.0).sqrt() / wsum.max(1e-30)) as f32
}

/// **Phasiness** of a stretched steady tone at `hz`: how far its level wanders, dB peak to
/// peak, over 40 ms windows hopped by 10 ms through `x[from..to]` (a phase vocoder that
/// loses its partials' phase relations makes them beat; a clean one holds them flat).
pub fn level_wander_db(x: &[f32], rate: f32, hz: f32, from: usize, to: usize) -> f32 {
    level_wander_db_in(x, rate, hz, from, to, 0.040)
}

/// [`level_wander_db`] over windows of `window_s` seconds (hopped by a quarter of one):
/// longer windows resolve partials that lie close together (a chord's voices), at the
/// cost of the fastest wander.
pub fn level_wander_db_in(
    x: &[f32],
    rate: f32,
    hz: f32,
    from: usize,
    to: usize,
    window_s: f32,
) -> f32 {
    let w = (window_s * rate) as usize;
    let hop = (w / 4).max(1);
    let to = to.min(x.len());
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    let mut i = from;
    while i + w <= to {
        let a = 20.0 * math::log10(amplitude(&x[i..i + w], rate, hz).max(1e-9));
        lo = lo.min(a);
        hi = hi.max(a);
        i += hop;
    }
    if hi < lo {
        0.0
    } else {
        hi - lo
    }
}

/// The correlation of two signals over `from..to` (1: the same shape).
pub fn correlation(a: &[f32], b: &[f32], from: usize, to: usize) -> f32 {
    let to = to.min(a.len()).min(b.len());
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for i in from..to {
        let (x, y) = (a[i] as f64, b[i] as f64);
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    (ab / (aa * bb).sqrt().max(1e-30)) as f32
}

/// The inter-channel phase difference of a stereo tone at `hz` over `x`, radians: how a
/// stretch keeps (or smears) the stereo image.
pub fn channel_phase(l: &[f32], r: &[f32], rate: f32, hz: f32) -> f32 {
    let phase = |x: &[f32]| -> f64 {
        let w = 2.0 * std::f64::consts::PI * hz as f64 / rate as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, &v) in x.iter().enumerate() {
            let (s, c) = math::sin_cos(w * i as f64);
            re += v as f64 * c;
            im -= v as f64 * s;
        }
        math::atan2(im, re)
    };
    let d = phase(l) - phase(r);
    let pi = std::f64::consts::PI;
    ((d + pi).rem_euclid(2.0 * pi) - pi) as f32
}

/// RMS level in dB over `x`.
pub fn rms_db(x: &[f32]) -> f32 {
    let n = x.len().max(1) as f64;
    let e: f64 = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / n;
    (10.0 * math::log10(e.max(1e-20))) as f32
}
