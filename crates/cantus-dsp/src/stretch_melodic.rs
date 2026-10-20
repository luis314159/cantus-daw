//! Melodic: TD-PSOLA on the pitch base's epochs (ADR 0053's offline shifter), the time map
//! its own, the formants kept by the grains themselves (or moved with the pitch when not
//! kept). Where the line is unvoiced, or periodic only loosely (more than one voice, a
//! chord), the Complex render is cross-faded in over 10 ms.

use super::Request;
use crate::pitch::Analysis;
use crate::psola::{self, Plan, TimeMap};

/// A voiced frame this periodic or better is one voice (YIN's aperiodicity).
const ONE_VOICE: f32 = 0.30;
/// The cross-fade between the two renders, seconds.
const BLEND_S: f64 = 0.010;

pub(super) fn render(
    channels: &[&[f32]],
    rate: f32,
    req: &Request,
    ratio: f64,
    a: &Analysis,
) -> Vec<Vec<f32>> {
    let len = req.len;
    let map = req.map;
    // PSOLA's map: the same breakpoints, closed at both ends of the output.
    let mut pts: Vec<(f64, f64)> = vec![(0.0, map.source(0.0))];
    pts.extend(
        map.points()
            .iter()
            .copied()
            .filter(|p| p.0 > 0.0 && p.0 < len as f64),
    );
    pts.push((len.max(1) as f64, map.source(len.max(1) as f64)));
    let time = TimeMap::new(pts).unwrap_or_default();
    let r = ratio as f32;
    let semis = (12.0 * crate::math::log2(ratio)) as f32;
    let formant = if req.formants { 0.0 } else { semis };
    let ratio_at = |_: f64| r;
    let formant_at = |_: f64| formant;
    let voice = psola::render(
        channels,
        a,
        &Plan {
            ratio: &ratio_at,
            formant: &formant_at,
            time,
            len,
        },
    );
    // How much of each output frame is the voice's render.
    let step = 1.0 / (BLEND_S * rate as f64).max(1.0);
    let target: Vec<f64> = (0..len)
        .map(|t| {
            let s = map.source(t as f64);
            match a.frame_at(s) {
                Some(f) if f.voiced && f.aperiodicity <= ONE_VOICE => 1.0,
                _ => 0.0,
            }
        })
        .collect();
    let mut w = vec![0.0f64; len];
    // Slewed both ways, so a switch is a symmetric 10 ms cross-fade around its frame.
    let mut v = 0.0;
    for t in 0..len {
        v = if target[t] > v {
            (v + step).min(target[t])
        } else {
            (v - step).max(target[t])
        };
        w[t] = v;
    }
    let mut v = 0.0;
    for t in (0..len).rev() {
        v = if target[t] > v {
            (v + step).min(target[t])
        } else {
            (v - step).max(target[t])
        };
        w[t] = w[t].max(v).min(1.0);
    }
    if w.iter().all(|x| *x >= 1.0) {
        return voice;
    }
    let other = super::complex(channels, rate, req, ratio);
    voice
        .into_iter()
        .zip(other)
        .map(|(p, c)| {
            p.iter()
                .zip(&c)
                .zip(&w)
                .map(|((p, c), w)| (*w as f32) * p + (1.0 - *w as f32) * c)
                .collect()
        })
        .collect()
}
