//! The offline shifter: a whole take, any ratio and formant curve, any time map, on the
//! exact epochs of the offline analysis.

use super::{lattice, Grain, Ola, FADE_S, MAX_FORMANT, MAX_RATIO, NOISE_PERIOD_S};
use crate::math;
use crate::pitch::Analysis;

/// Where each output sample reads the source: a piecewise-linear map from output time to
/// source time (samples), by breakpoints `(output, source)` with both non-decreasing.
/// Empty: the identity. Before the first breakpoint and after the last the map continues
/// at slope 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimeMap {
    points: Vec<(f64, f64)>,
}

impl TimeMap {
    pub fn identity() -> Self {
        TimeMap::default()
    }

    /// A map through `points`; `None` unless both coordinates are non-decreasing.
    pub fn new(points: Vec<(f64, f64)>) -> Option<Self> {
        let ok = points.windows(2).all(|w| w[1].0 > w[0].0 && w[1].1 >= w[0].1);
        ok.then_some(TimeMap { points })
    }

    /// The breakpoint segment holding output time `t`: its start and its slope.
    fn segment(&self, t: f64) -> ((f64, f64), f64) {
        let p = &self.points;
        if p.is_empty() {
            return ((0.0, 0.0), 1.0);
        }
        if t <= p[0].0 {
            return (p[0], 1.0);
        }
        let i = p.partition_point(|q| q.0 <= t);
        if i >= p.len() {
            return (p[p.len() - 1], 1.0);
        }
        let (a, b) = (p[i - 1], p[i]);
        (a, (b.1 - a.1) / (b.0 - a.0))
    }

    /// The source time output time `t` reads.
    pub fn source(&self, t: f64) -> f64 {
        let (a, k) = self.segment(t);
        a.1 + (t - a.0) * k
    }

    /// Whether output time `t` reads the source sample for sample with no interpolation:
    /// slope 1 at a whole-sample offset.
    fn is_plain(&self, t: f64) -> bool {
        let (a, k) = self.segment(t);
        k == 1.0 && (a.1 - a.0).fract() == 0.0
    }
}

/// What to do to a take.
pub struct Plan<'a> {
    /// The pitch ratio at a source time (samples); 1 leaves the pitch.
    pub ratio: &'a dyn Fn(f64) -> f32,
    /// The formant shift at a source time, semitones; 0 keeps the formants.
    pub formant: &'a dyn Fn(f64) -> f32,
    /// Where each output sample reads the source.
    pub time: TimeMap,
    /// The output's length, samples.
    pub len: usize,
}

/// Render `plan` over the take's channels (each as long as the analysed take): the output
/// per channel, `plan.len` samples. Where the ratio is 1, the formant 0 and the time map
/// plain (slope 1, a whole-sample offset), the output **is** the source, bit for bit; the
/// shifter fades in and out over 5 ms around everything else. Deterministic; allocates.
pub fn render(channels: &[&[f32]], analysis: &Analysis, plan: &Plan) -> Vec<Vec<f32>> {
    let sr = analysis.sample_rate;
    let mut out: Vec<Vec<f32>> = vec![vec![0.0; plan.len]; channels.len()];
    let mut ola = Ola::new();
    let mut mix = 0.0f32;
    let step = 1.0 / (FADE_S * sr).max(1.0);
    let noise = NOISE_PERIOD_S * sr;
    // The fade must be complete before a change: look ahead by the fade's length.
    let fade_len = (FADE_S * sr).ceil() as usize + 1;
    let changes = |t: usize| -> bool {
        let tf = t as f64;
        let src = plan.time.source(tf);
        !plan.time.is_plain(tf) || (plan.ratio)(src) != 1.0 || (plan.formant)(src) != 0.0
    };
    // The next change at or after each sample (a reverse scan).
    let mut next = vec![usize::MAX; plan.len + 1];
    for t in (0..plan.len).rev() {
        next[t] = if changes(t) { t } else { next[t + 1] };
    }
    let mut buf = vec![0.0f32; channels.len()];
    for t in 0..plan.len {
        let engage = next[t] <= t + fade_len;
        mix = if engage {
            (mix + step).min(1.0)
        } else {
            (mix - step).max(0.0)
        };
        let tf = t as f64;
        let src = plan.time.source(tf);
        let plain = plan.time.is_plain(tf);
        for (ch, x) in channels.iter().enumerate() {
            buf[ch] = if plain {
                let i = src as i64;
                if i >= 0 && (i as usize) < x.len() {
                    x[i as usize]
                } else {
                    0.0
                }
            } else {
                super::read_at(x, src)
            };
        }
        if mix == 0.0 {
            ola.clear();
            for (o, v) in out.iter_mut().zip(&buf) {
                o[t] = *v;
            }
            continue;
        }
        grow(&mut ola, tf, analysis, plan, noise);
        ola.retire(tf);
        for (ch, x) in channels.iter().enumerate() {
            let wet = ola
                .sample::<1>(tf, |_, at| super::read_at(x, at))
                .map(|w| w[0])
                .unwrap_or(buf[ch]);
            out[ch][t] = if mix == 1.0 {
                wet
            } else {
                buf[ch] + (wet - buf[ch]) * mix
            };
        }
    }
    out
}

/// Create grains until one is centred after `t`.
fn grow(ola: &mut Ola, t: f64, a: &Analysis, plan: &Plan, noise: f32) {
    let (s, prev_a) = match ola.last() {
        None => {
            let src = plan.time.source(t);
            let (p_a, _, f) = local(a, plan, src, noise);
            ola.push(Grain {
                s: t,
                a: src,
                hl: 0.0,
                hr: p_a / f,
                f,
            });
            return;
        }
        Some(g) if t < g.s => return,
        Some(g) => (g.s, g.a),
    };
    let src_now = plan.time.source(s);
    let (p_a0, ratio0, _) = local(a, plan, src_now, noise);
    let p_s = p_a0 / ratio0;
    let s_new = s + p_s as f64;
    let src = plan.time.source(s_new);
    let (p_a, _, f) = local(a, plan, src, noise);
    let voiced = a.frame_at(src).is_some_and(|fr| fr.voiced);
    let mark = if voiced {
        nearest_epoch(a, src).unwrap_or(src)
    } else {
        // Unvoiced: grains on a lattice of the noise period around the source time.
        lattice(prev_a, p_a as f64, src, f64::INFINITY)
    };
    let hr = p_a / f;
    ola.push(Grain {
        s: s_new,
        a: mark,
        hl: p_s.min(hr),
        hr,
        f,
    });
}

/// The input period, the ratio and the formant read rate at a source time.
fn local(a: &Analysis, plan: &Plan, src: f64, noise: f32) -> (f32, f32, f32) {
    let f = math::exp2((plan.formant)(src) / 12.0).clamp(1.0 / MAX_FORMANT, MAX_FORMANT);
    match a.frame_at(src) {
        Some(fr) if fr.voiced && fr.period > 1.0 => {
            let p = nearest_epoch_period(a, src).unwrap_or(fr.period);
            (p, (plan.ratio)(src).clamp(1.0 / MAX_RATIO, MAX_RATIO), f)
        }
        _ => (noise, 1.0, f),
    }
}

fn nearest_index(a: &Analysis, src: f64) -> Option<usize> {
    let e = &a.epochs;
    if e.is_empty() {
        return None;
    }
    let i = e.partition_point(|x| x.at < src);
    let best = match (i.checked_sub(1), (i < e.len()).then_some(i)) {
        (Some(p), Some(n)) => {
            if src - e[p].at <= e[n].at - src {
                p
            } else {
                n
            }
        }
        (Some(p), None) => p,
        (None, Some(n)) => n,
        (None, None) => return None,
    };
    // Only a mark within a period of the time counts.
    ((e[best].at - src).abs() <= e[best].period as f64).then_some(best)
}

fn nearest_epoch(a: &Analysis, src: f64) -> Option<f64> {
    nearest_index(a, src).map(|i| a.epochs[i].at)
}

fn nearest_epoch_period(a: &Analysis, src: f64) -> Option<f32> {
    nearest_index(a, src).map(|i| a.epochs[i].period)
}
