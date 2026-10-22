//! The **sketch**: a quick render for what plays while the full one is made (the engine's
//! bridge) — WSOLA (waveform-similarity overlap-add, Verhelst and Roelands 1993): Hann
//! grains half overlapping, each read at the transposition from where the map puts it,
//! moved by up to ±10 ms to the place whose waveform best continues the grain before (so
//! the overlaps add in phase and a tone keeps the transposition's pitch, not the speed's).
//! A grain that holds a transient or a marker (`anchors`) is not moved: it is read from the
//! anchor outward, so the anchor lands on its own frame. The search runs on a mixdown, one
//! sample in eight, then in two, then to the sample, each place weighed once: a few
//! milliseconds per second of audio.

use super::read::Fast;
use super::Map;
use crate::math;

/// The coarse search's step and correlation stride (then two, then one).
const COARSE: usize = 8;
/// Faster than this (seven semitones up), the band-limited reader.
const FAST_ABOVE: f64 = 1.5;

fn read(x: &[f32], fast: Option<&Fast>, pos: f64, ratio: f64) -> f32 {
    if let Some(f) = fast {
        return f.read(pos, ratio);
    }
    if pos.fract() == 0.0 {
        let i = pos as i64;
        return if i >= 0 && (i as usize) < x.len() {
            x[i as usize]
        } else {
            0.0
        };
    }
    crate::psola::read_at(x, pos)
}

/// How well `m` from `b` continues `m` from `a`, over `len` frames one in `step`: the
/// correlation, normalised by the candidate's energy.
fn similarity(m: &[f32], a: i64, b: i64, len: usize, step: usize) -> f32 {
    let get = |i: i64| -> f32 {
        if i >= 0 && (i as usize) < m.len() {
            m[i as usize]
        } else {
            0.0
        }
    };
    let (mut xy, mut yy) = (0.0f32, 0.0f32);
    let mut n = 0;
    while n < len {
        let x = get(a + n as i64);
        let y = get(b + n as i64);
        xy += x * y;
        yy += y * y;
        n += step;
    }
    xy / yy.max(1e-12).sqrt()
}

/// See the module docs. `channels` start at source frame 0 of `map` (local frames);
/// `anchors` are in the same frames, in order.
pub(super) fn render(
    channels: &[&[f32]],
    rate: f32,
    map: &Map,
    len: usize,
    ratio: f64,
    anchors: &[f64],
) -> Vec<Vec<f32>> {
    let g = ((0.043 * rate) as usize)
        .next_power_of_two()
        .clamp(256, 4_096);
    let h = g / 2;
    let win: Vec<f32> = (0..g)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / g as f64)) as f32)
        .collect();
    let search = (0.010 * rate) as i64;
    let mono: Vec<f32> = {
        let n = channels.iter().map(|c| c.len()).max().unwrap_or(0);
        (0..n)
            .map(|i| {
                channels
                    .iter()
                    .map(|c| c.get(i).copied().unwrap_or(0.0))
                    .sum()
            })
            .collect()
    };
    let (a, b) = (map.source(0.0), map.source(len as f64));
    let reach = g as f64 * ratio.max(1.0) + search as f64;
    // Read faster than recorded through the band-limited reader only where folding would
    // be heard: up to half again (seven semitones) what folds back lies above 16 kHz, and
    // a cubic read costs a sixteenth (round 29's second review: the sketch is a bridge,
    // made quickly).
    let fast: Vec<Option<Fast>> = channels
        .iter()
        .map(|x| {
            (ratio > FAST_ABOVE).then(|| Fast::new(x, a.min(b) - reach, a.max(b) + reach, ratio))
        })
        .collect();
    let mut out: Vec<Vec<f32>> = channels.iter().map(|_| vec![0.0; len]).collect();
    // Source frames one hop of output reads.
    let hop_src = h as f64 * ratio;
    let overlap = (hop_src.round() as usize).max(1);
    let mut prev: Option<f64> = None;
    let mut c0 = -(h as i64);
    while c0 < len as i64 {
        let centre_out = (c0 + h as i64) as f64;
        let centre = map.source(centre_out);
        let nominal = centre - hop_src;
        let i = anchors.partition_point(|s| *s < centre);
        let anchor = [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|i| anchors.get(i).copied())
            .filter(|s| (s - centre).abs() <= hop_src)
            .min_by(|x, y| (x - centre).abs().total_cmp(&(y - centre).abs()));
        let start = match (anchor, prev) {
            (Some(s), _) => s - (map.output(s) - c0 as f64) * ratio,
            (None, Some(p)) => {
                // The natural continuation of the grain before, and the nominal place moved
                // to where the waveform continues it best.
                let natural = (p + hop_src).round() as i64;
                let base = nominal.round() as i64;
                // Each place weighed once, coarse to fine (round 29's second review: every
                // comparison weighed both again, and a long take took seconds).
                let best = |lo: i64, hi: i64, step: usize| -> i64 {
                    let mut won = (f32::NEG_INFINITY, 0i64);
                    for x in (lo..=hi).step_by(step) {
                        let s = similarity(&mono, natural, base + x, overlap, step);
                        if s > won.0 || (s == won.0 && x.abs() < won.1.abs()) {
                            won = (s, x);
                        }
                    }
                    won.1
                };
                let coarse = best(-search, search, COARSE);
                let c = COARSE as i64;
                let medium = best(coarse - c, coarse + c, 2);
                let fine = best(medium - 2, medium + 2, 1);
                nominal + fine as f64
            }
            (None, None) => nominal,
        };
        for (ci, (o, x)) in out.iter_mut().zip(channels).enumerate() {
            for (k, w) in win.iter().enumerate() {
                let t = c0 + k as i64;
                if t < 0 {
                    continue;
                }
                if t >= len as i64 {
                    break;
                }
                let pos = start + k as f64 * ratio;
                o[t as usize] += read(x, fast[ci].as_ref(), pos, ratio) * w;
            }
        }
        prev = Some(start);
        c0 += h as i64;
    }
    out
}
