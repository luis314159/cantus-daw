//! The patch's graph (which operators are heard, which feed them, and how), and the alias
//! guard: the bandwidth a voice can reach, by Carson's rule, and what that decides — the
//! voice's oversampling factor (Auto) and how far its indices are scaled back (Tame).
//!
//! Carson's rule puts most of a PM spectrum within `f_c + (I + 1)·f_m`. For a network the
//! estimate runs in evaluation order: an operator's bandwidth is its own frequency plus,
//! for each operator modulating it, `(I + 1)` times *that* operator's bandwidth (a
//! modulated modulator is wider than its frequency). Feedback widens an operator by about
//! four harmonics per radian. Content folds back into the audible band only if it lies
//! beyond `(N − ½)·fs` at N-times oversampling (below that, it folds above the host's
//! Nyquist, where the half-bands remove it), so a voice needs the smallest N with its
//! bandwidth under `(N − ½)·fs·0.9`.

use super::{target, Patch, GUARD_TAME, OPS, OUT_OFF, QUALITY_AUTO};
use crate::math;

/// What the audio loop needs to know about the patch's topology, worked out when the patch
/// changes, never per sample.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Graph {
    /// The operators computed, in evaluation order (H first).
    pub order: [u8; OPS],
    pub n: usize,
    /// Heard: on, and its output above off (or modulated there).
    pub carrier: [bool; OPS],
    pub needed: [bool; OPS],
    /// `ins[t][..ins_n[t]]`: the operators modulating *t*.
    pub ins: [[u8; OPS]; OPS],
    pub ins_n: [u8; OPS],
    pub has_fb: [bool; OPS],
    /// The most the modulation routes can add to a cell, a feedback, the brightness.
    pub extra_m: [[f32; OPS]; OPS],
    pub extra_fb: [f32; OPS],
    pub extra_bright: f32,
}

pub fn graph(p: &Patch) -> Graph {
    let mut g = Graph::default();
    let mut routed_out = [false; OPS];
    for r in p.routes.routes() {
        let d = r.dst as usize;
        let depth = r.depth.abs();
        if (target::M..target::FB).contains(&d) {
            let i = d - target::M;
            g.extra_m[i / OPS][i % OPS] += depth;
        } else if (target::FB..target::OUT).contains(&d) {
            g.extra_fb[d - target::FB] += depth;
        } else if (target::OUT..target::PAN).contains(&d) {
            routed_out[d - target::OUT] = true;
        } else if d == target::BRIGHT {
            g.extra_bright += depth;
        }
    }
    for (o, s) in p.ops.iter().enumerate() {
        g.carrier[o] = s.on && (s.out_db > OUT_OFF || routed_out[o]);
        g.needed[o] = g.carrier[o];
        g.has_fb[o] = s.fb != 0.0 || g.extra_fb[o] != 0.0;
    }
    let edge = |s: usize, t: usize| {
        s != t && p.ops[s].on && p.ops[t].on && (p.m[s][t] != 0.0 || g.extra_m[s][t] != 0.0)
    };
    // Everything that reaches a heard operator is needed.
    for _ in 0..OPS {
        for s in 0..OPS {
            if !g.needed[s] && (0..OPS).any(|t| g.needed[t] && edge(s, t)) {
                g.needed[s] = true;
            }
        }
    }
    for o in (0..OPS).rev() {
        if g.needed[o] {
            g.order[g.n] = o as u8;
            g.n += 1;
        }
    }
    for t in 0..OPS {
        for s in (0..OPS).rev() {
            if g.needed[t] && g.needed[s] && edge(s, t) {
                g.ins[t][g.ins_n[t] as usize] = s as u8;
                g.ins_n[t] += 1;
            }
        }
    }
    g
}

/// The Carson bandwidth (Hz) of a voice whose operators run at `hz` and reach at most
/// `peak` (their envelope's highest level times their level), with every index scaled by
/// `scale`.
pub fn bandwidth(p: &Patch, g: &Graph, hz: &[f32; OPS], peak: &[f32; OPS], scale: f32) -> f32 {
    estimate(p, g, hz, peak, scale, false)
}

/// The bandwidth the guard decides on: sidebands down to about −60 dB rather than Carson's
/// 98 % of the power (`I + 2.5·∛I + 2` of them rather than `I + 1`), and feedback four
/// times wider.
pub fn guard_bandwidth(
    p: &Patch,
    g: &Graph,
    hz: &[f32; OPS],
    peak: &[f32; OPS],
    scale: f32,
) -> f32 {
    estimate(p, g, hz, peak, scale, true)
}

/// Carson for every operator; with `strict`, the heard operators' own sidebands are counted
/// down to about −60 dB instead (the margin is applied once, at the output, so it does not
/// compound through a stack).
fn estimate(
    p: &Patch,
    g: &Graph,
    hz: &[f32; OPS],
    peak: &[f32; OPS],
    scale: f32,
    strict: bool,
) -> f32 {
    let mut b = [0f32; OPS];
    let mut widest = 0f32;
    for &o in &g.order[..g.n] {
        let t = o as usize;
        let f = hz[t].abs();
        let own = if p.ops[t].wave == 0 { f } else { f * 8.0 };
        let (mut carson, mut wide) = (own, own);
        for &s in &g.ins[t][..g.ins_n[t] as usize] {
            let s = s as usize;
            let i = (p.m[s][t].abs() + g.extra_m[s][t]) * scale * peak[s];
            if i > 1e-4 {
                // A back edge's source is not worked out yet: its own frequency stands in.
                let fm = if s > t { b[s] } else { hz[s].abs() };
                carson += (i + 1.0) * fm;
                wide += (i + 2.5 * math::cbrt(i) + 2.0) * fm;
            }
        }
        let beta = (p.ops[t].fb.abs() + g.extra_fb[t]) * scale * peak[t];
        if beta > 1e-4 {
            carson += 4.0 * beta * f;
            wide += 16.0 * beta * f;
        }
        b[t] = carson;
        if g.carrier[t] {
            widest = widest.max(if strict { wide } else { carson });
        }
    }
    widest
}
/// What the guard decides for one voice at its note-on.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Decision {
    /// Oversampling factor: 1, 2, 4 or 8.
    pub factor: usize,
    /// Every index of the voice is multiplied by this (1 unless taming).
    pub tame: f32,
    /// The bandwidth the voice can reach, after taming, Hz.
    pub bandwidth: f32,
}

/// The widest bandwidth factor `n` carries without audible folding.
pub fn limit(n: usize, fs: f32) -> f32 {
    (n as f32 - 0.5) * fs * 0.9
}

pub fn decide(p: &Patch, g: &Graph, hz: &[f32; OPS], peak: &[f32; OPS], fs: f32) -> Decision {
    let bright = math::exp2(2.0 * (p.bright + g.extra_bright).clamp(-1.0, 1.0));
    let b = guard_bandwidth(p, g, hz, peak, bright);
    let factor = match super::fixed_factor(p.quality) {
        Some(f) => f,
        None => {
            debug_assert_eq!(p.quality, QUALITY_AUTO);
            [1usize, 2, 4, 8]
                .into_iter()
                .find(|&n| b <= limit(n, fs))
                .unwrap_or(8)
        }
    };
    let lim = limit(factor, fs);
    if p.guard != GUARD_TAME || b <= lim {
        return Decision {
            factor,
            tame: 1.0,
            bandwidth: bandwidth(p, g, hz, peak, bright),
        };
    }
    // The largest scale that fits, by bisection (the estimate grows with the scale).
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..16 {
        let mid = 0.5 * (lo + hi);
        if guard_bandwidth(p, g, hz, peak, bright * mid) <= lim {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Decision {
        factor,
        tame: lo,
        bandwidth: bandwidth(p, g, hz, peak, bright * lo),
    }
}
