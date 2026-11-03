//! Joins without clicks (ADR 0096 §3a): where an edit puts two pieces of audio side by side
//! — a cut, a trim's new ends, a paste, inserted or written silence, the cuts of *Remove
//! silence* — the edges are first moved to the nearest zero crossing (within
//! [`SNAP_MS`]), then the two sides are crossfaded over [`Joins::xfade_ms`] (3 ms by
//! default) on an equal-power curve.
//!
//! Why these numbers: a step in the waveform is heard as a click however short; a zero
//! crossing makes the step small and the crossfade makes it smooth. Across a cut the two
//! sides are different moments of a take — uncorrelated — so equal power keeps the level
//! through the join (a linear crossfade dips 3 dB there). 3 ms is longer than one cycle of
//! everything above 333 Hz, where clicks live, and shorter than the attack of a consonant
//! (5–10 ms), so it smooths the seam without smearing a transient next to it; 2 ms left a
//! soft tick on low notes in our listening, 5 ms audibly softened a cut on a drum hit.
//!
//! The crossfade sits *after* a join where the left side's continuation is known (a cut:
//! what followed the cut start) and *before* it where only the right side's lead-in is (a
//! paste's end): each side fades with its own material, never into invented samples. A
//! side that is the edge of the file or silence fades from or to zero.

use super::AudioBuf;
use cantus_dsp::math;

/// How far an edge may move to find a zero crossing, ms.
pub const SNAP_MS: f32 = 5.0;

/// How joins are made.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Joins {
    /// Move edges to the nearest zero crossing.
    pub snap: bool,
    /// The crossfade at each join, ms (0: a butt splice).
    pub xfade_ms: f32,
}

impl Default for Joins {
    fn default() -> Self {
        Joins {
            snap: true,
            xfade_ms: 3.0,
        }
    }
}

impl Joins {
    /// Raw splices: edges where they are asked, no crossfade (what identity tests and
    /// sample-exact work want).
    pub const RAW: Joins = Joins {
        snap: false,
        xfade_ms: 0.0,
    };

    pub fn frames(&self, rate: u32) -> usize {
        (self.xfade_ms.max(0.0) * rate as f32 / 1000.0).round() as usize
    }
}

/// The frame nearest `at` (within [`SNAP_MS`]) where the mono mix crosses zero: the
/// sample of the pair that is nearer zero. The file's ends and a frame with no crossing
/// near it stay where they are.
pub fn snap(buf: &AudioBuf, at: usize, on: bool) -> usize {
    let frames = buf.frames();
    if !on || at == 0 || at >= frames {
        return at;
    }
    let n = buf.channels.max(1) as usize;
    let mono = |f: usize| -> f32 { buf.data[f * n..f * n + n].iter().sum::<f32>() };
    let w = ((buf.rate as f32 * SNAP_MS / 1000.0) as usize).max(1);
    let lo = at.saturating_sub(w).max(1);
    let hi = (at + w).min(frames - 1);
    let mut best: Option<(usize, usize)> = None;
    for f in lo..=hi {
        let (a, b) = (mono(f - 1), mono(f));
        if a == 0.0 || b == 0.0 || (a < 0.0) != (b < 0.0) {
            let pick = if b.abs() <= a.abs() { f } else { f - 1 };
            let d = pick.abs_diff(at);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((pick, d));
            }
        }
    }
    best.map_or(at, |(f, _)| f.max(1).min(frames - 1))
}

/// Blend `len` frames of `out` from frame `at`: `a(i, c)` fading out, `b(i, c)` fading in,
/// equal power.
pub fn blend(
    out: &mut [f32],
    n: usize,
    at: usize,
    len: usize,
    a: impl Fn(usize, usize) -> f32,
    b: impl Fn(usize, usize) -> f32,
) {
    for i in 0..len {
        let x = (i as f32 + 0.5) / len as f32;
        let (s, c) = math::sin_cos(x * std::f32::consts::FRAC_PI_2);
        for ch in 0..n {
            let at = (at + i) * n + ch;
            if at < out.len() {
                out[at] = a(i, ch) * c + b(i, ch) * s;
            }
        }
    }
}

/// Sample `(frame, channel)` of `buf`.
pub fn at(buf: &AudioBuf, f: usize, c: usize) -> f32 {
    let n = buf.channels.max(1) as usize;
    buf.data[f * n + c]
}
