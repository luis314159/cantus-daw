//! Rhythm: slices between transients, each played at its own speed from where the map puts
//! its transient (Ableton's *Beats*, FL's slicing). A slice is cut where the next begins
//! (a short fade before it, so the next transient starts clean), or — when the clip is
//! slowed and the slice runs out first — ends by its [`Tail`].

use super::read::Fast;
use super::{Request, Tail};

/// The fade that ends a slice, seconds.
const FADE_S: f64 = 0.003;
/// A looped tail's loop: at most this long, seconds…
const LOOP_S: f64 = 0.060;
/// …and its cross-fade, at most this long.
const XFADE_S: f64 = 0.005;

fn get(x: &[f32], i: i64) -> f32 {
    if i >= 0 && (i as usize) < x.len() {
        x[i as usize]
    } else {
        0.0
    }
}

/// The source read at `pos`: whole frames as they are, fractions by cubic interpolation.
fn read(x: &[f32], pos: f64) -> f32 {
    if pos.fract() == 0.0 {
        get(x, pos as i64)
    } else {
        crate::psola::read_at(x, pos)
    }
}

/// The slices of a render: `(source start, output start)`, the first from wherever output
/// 0 reads — and the cut that started it, when a transient before the output did.
fn slices(req: &Request, ratio: f64, src_len: f64) -> (Vec<(f64, f64)>, Option<f64>) {
    let map = req.map;
    let len = req.len;
    // The cuts: every transient and every hard breakpoint (a warp marker, a window's end),
    // in the source — never a tempo ramp's samples (soft points): a slice rides through
    // them at its own speed.
    let mut cuts: Vec<f64> = req
        .onsets
        .iter()
        .copied()
        .chain(map.hard_points().map(|p| p.1))
        .map(f64::round)
        .filter(|s| *s > 0.0 && *s < src_len)
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|b, a| (*b - *a).abs() < 1.0);
    let mut slices: Vec<(f64, f64)> = vec![(map.source(0.0).round(), 0.0)];
    let mut first = None;
    for &s in &cuts {
        let o = map.output(s).round();
        if o <= slices.last().map_or(f64::NEG_INFINITY, |l| l.1) || o >= len as f64 {
            if o <= 0.0 {
                // A transient before the output starts: the first slice starts there.
                slices[0] = (s + (0.0 - o) * ratio, 0.0);
                first = Some(s);
            }
            continue;
        }
        slices.push((s, o));
    }
    (slices, first)
}

/// The source frames `[lo, hi]` the slices read (a looped tail's cross-fade aside, which
/// reaches a fifth of a slice before it at most).
fn read_span(slices: &[(f64, f64)], len: usize, ratio: f64, src_len: f64) -> (f64, f64) {
    let lo = slices.iter().map(|s| s.0).fold(f64::MAX, f64::min);
    let hi = slices
        .iter()
        .enumerate()
        .map(|(i, &(s0, o0))| {
            let (s1, o1) = slices
                .get(i + 1)
                .map_or((src_len, len as f64), |&(s, o)| (s, o));
            (s0 + (o1 - o0) * ratio).min(s1)
        })
        .fold(lo, f64::max);
    (lo, hi)
}

/// The span of a source `src_len` frames long a Rhythm render reads (before the readers'
/// own reach): from the first slice's start — or the transient before the output that
/// starts it — to the furthest a slice reads (round 29's second review: it took the whole
/// file for every window).
pub(super) fn reach(req: &Request, ratio: f64, src_len: usize) -> (f64, f64) {
    let (s, first) = slices(req, ratio, src_len as f64);
    let (lo, hi) = read_span(&s, req.len, ratio, src_len as f64);
    (first.map_or(lo, |f| f.min(lo)), hi)
}

pub(super) fn render(channels: &[&[f32]], rate: f32, req: &Request, ratio: f64) -> Vec<Vec<f32>> {
    let len = req.len;
    let src_len = channels.iter().map(|c| c.len()).max().unwrap_or(0) as f64;
    let (slices, _) = slices(req, ratio, src_len);
    let fade = (FADE_S * rate as f64).max(1.0);
    // Transposed up, a slice is read faster than it was recorded: through the band-limited
    // reader (nothing above the output's band folds back), over the span the slices read.
    let fast: Vec<Option<Fast>> = channels
        .iter()
        .map(|c| {
            (ratio > 1.0).then(|| {
                let (lo, hi) = read_span(&slices, len, ratio, src_len);
                Fast::new(c, lo, hi, ratio)
            })
        })
        .collect();
    let at = |c: usize, ch: &[f32], q: f64| -> f32 {
        match &fast[c] {
            Some(f) => f.read(q, ratio),
            None => read(ch, q),
        }
    };
    let mut out: Vec<Vec<f32>> = channels.iter().map(|_| vec![0.0; len]).collect();
    for (i, &(s0, o0)) in slices.iter().enumerate() {
        let (s1, o1) = slices
            .get(i + 1)
            .map_or((src_len, len as f64), |&(s, o)| (s, o));
        let room = s1 - s0; // source frames the slice owns
        let span = o1 - o0; // output frames it fills
        if span <= 0.0 || room <= 0.0 {
            continue;
        }
        let used = span * ratio;
        let continuous = (used - room).abs() < 0.5;
        // Read position (source) after `u` source frames of the slice.
        let lp = (LOOP_S * rate as f64).min(0.6 * room).max(1.0);
        let xf = (XFADE_S * rate as f64).min(lp / 3.0).max(1.0);
        let ls = s1 - lp;
        let pos = |u: f64| -> (f64, Option<(f64, f32)>) {
            if u < room - lp || req.tail == Tail::Fade || used <= room {
                return (s0 + u, None);
            }
            let v = u - (room - lp);
            match req.tail {
                Tail::PingPong => {
                    let ph = v.rem_euclid(2.0 * lp);
                    (if ph < lp { ls + ph } else { ls + 2.0 * lp - ph }, None)
                }
                _ => {
                    let ph = v.rem_euclid(lp);
                    if ph >= lp - xf {
                        let w = ((ph - (lp - xf)) / xf) as f32;
                        (ls + ph, Some((ls + ph - lp, w)))
                    } else {
                        (ls + ph, None)
                    }
                }
            }
        };
        // Where the slice ends: at the next slice, or (Fade) where its source runs out.
        let end = if used > room && req.tail == Tail::Fade {
            o0 + room / ratio
        } else {
            o1
        };
        let fade_len = if continuous {
            0.0
        } else {
            fade.min((end - o0) / 2.0)
        };
        let t_start = o0.max(0.0) as i64;
        let t_end = (end.ceil() as i64).min(len as i64);
        for t in t_start..t_end {
            let u = (t as f64 - o0) * ratio;
            let (p, x) = pos(u);
            let left = end - t as f64;
            let g = if fade_len > 0.0 && left < fade_len {
                (left / fade_len).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            for (c, (o, ch)) in out.iter_mut().zip(channels).enumerate() {
                let mut v = at(c, ch, p);
                if let Some((q, w)) = x {
                    v = v * (1.0 - w) + at(c, ch, q) * w;
                }
                o[t as usize] += v * g;
            }
        }
    }
    out
}
