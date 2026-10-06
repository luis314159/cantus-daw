//! Loop tooling for the editor (`research/instruments/sampler.md` §4.6): the measured seam
//! error, a search for better loop points nearby, and zero crossings on both channels.
//! Plain functions over the decoded file; the UI calls them, never the audio thread.

/// Frames either side of a seam the error looks at.
use crate::math;

pub const SEAM_WINDOW: usize = 256;

fn frame(data: &[f32], ch: usize, i: i64) -> f32 {
    if i < 0 {
        return 0.0;
    }
    let at = i as usize * ch;
    if at + ch > data.len() {
        return 0.0;
    }
    data[at..at + ch].iter().sum::<f32>() / ch as f32
}

/// How far the join from `le` back to `ls` is from the audio's own continuation, in dB
/// relative to the level around it: the audio after the loop start compared with the
/// audio after the loop end, and the audio before each. A seamless loop reads −120 dB or
/// lower; −40 dB is inaudible on most material; above −20 dB clicks.
pub fn seam_error_db(data: &[f32], channels: usize, ls: i64, le: i64, window: usize) -> f32 {
    seam_error_xf_db(data, channels, ls, le, window, 0)
}

/// What the loop plays just before its end, with a linear crossfade of `xf` frames (the
/// audio one loop earlier fading in), as the engine does.
pub fn played_before(data: &[f32], channels: usize, ls: i64, le: i64, xf: i64, b: i64) -> f32 {
    let ch = channels.max(1);
    let len = le - ls + 1;
    let xf = xf.clamp(0, ls.min(len - 1).max(0));
    if xf > 0 && b > le - xf && b <= le {
        let g = (b - (le - xf)) as f32 / xf as f32;
        (1.0 - g) * frame(data, ch, b) + g * frame(data, ch, b - len)
    } else {
        frame(data, ch, b)
    }
}

/// [`seam_error_db`] for a loop with a crossfade of `xf` frames: the join as it is played.
/// With a crossfade the end of the loop has become the audio before its start, so the
/// error is how far the crossfaded end is from what naturally precedes the loop start.
pub fn seam_error_xf_db(
    data: &[f32],
    channels: usize,
    ls: i64,
    le: i64,
    window: usize,
    xf: i64,
) -> f32 {
    let ch = channels.max(1);
    let frames = (data.len() / ch) as i64;
    if le <= ls || le >= frames || ls < 0 {
        return 0.0;
    }
    let w = window as i64;
    let (mut diff, mut level, mut n) = (0.0f64, 0.0f64, 0usize);
    for j in 0..w {
        // After the seam: what the loop plays (from ls) against the natural continuation
        // of what played before it (the file after le, or — crossfaded — the loop start).
        if le + 1 + j < frames && xf == 0 {
            let a = frame(data, ch, ls + j) as f64;
            let b = frame(data, ch, le + 1 + j) as f64;
            diff += (a - b) * (a - b);
            level += a * a + b * b;
            n += 1;
        }
        // Before the seam: the end of the loop as played against what precedes its start.
        if ls - 1 - j >= 0 {
            let a = played_before(data, ch, ls, le, xf, le - j) as f64;
            let b = frame(data, ch, ls - 1 - j) as f64;
            diff += (a - b) * (a - b);
            level += a * a + b * b;
            n += 1;
        }
    }
    if n == 0 {
        // A loop of the whole file: compare its two ends as a cycle.
        let a = frame(data, ch, le) as f64 - frame(data, ch, ls) as f64;
        let slope = frame(data, ch, ls + 1) as f64 - frame(data, ch, ls) as f64;
        let err = (a - slope).abs();
        return (20.0 * math::log10(err.max(1e-6))).max(-120.0) as f32;
    }
    let level = (level / 2.0).max(1e-12);
    (10.0 * math::log10((diff / level).max(1e-12))).max(-120.0) as f32
}

/// The loop end within `search` frames of `le` (then the loop start within `search` of
/// `ls`) with the lowest seam error. Returns `(ls, le, error dB)`.
pub fn find_best_loop(
    data: &[f32],
    channels: usize,
    ls: i64,
    le: i64,
    search: i64,
) -> (i64, i64, f32) {
    let ch = channels.max(1);
    let frames = (data.len() / ch) as i64;
    let w = 128;
    let mut best = (ls, le, seam_error_db(data, ch, ls, le, w));
    let min_len = 32;
    for cand in (le - search).max(ls + min_len)..=(le + search).min(frames - 2) {
        let e = seam_error_db(data, ch, best.0, cand, w);
        if e < best.2 {
            best = (best.0, cand, e);
        }
    }
    for cand in (ls - search).max(0)..=(ls + search).min(best.1 - min_len) {
        let e = seam_error_db(data, ch, cand, best.1, w);
        if e < best.2 {
            best = (cand, best.1, e);
        }
    }
    (
        best.0,
        best.1,
        seam_error_db(data, ch, best.0, best.1, SEAM_WINDOW),
    )
}

/// The frame nearest `pos` (within `radius`) where the signal crosses zero rising on every
/// channel at once, or where the channels' summed magnitude is smallest when they never
/// agree — Live snaps on the left channel only, which leaves the right one clicking.
pub fn zero_crossing(data: &[f32], channels: usize, pos: i64, radius: i64) -> i64 {
    let ch = channels.max(1);
    let frames = (data.len() / ch) as i64;
    let at = |i: i64, c: usize| data[i as usize * ch + c];
    let mut best = (pos.clamp(0, (frames - 1).max(0)), f32::MAX);
    for d in 0..=radius {
        for i in [pos - d, pos + d] {
            if i < 1 || i >= frames {
                continue;
            }
            let rising = (0..ch).all(|c| at(i - 1, c) <= 0.0 && at(i, c) >= 0.0);
            let mag: f32 = (0..ch).map(|c| at(i, c).abs()).sum();
            let score = if rising { mag * 0.01 } else { mag };
            if score < best.1 {
                best = (i, score);
            }
        }
    }
    best.0
}

/// Peaks of `data` in `buckets` columns over `[from, to)`: (min, max) per column, the first
/// channel and the second averaged — what the waveform view draws.
pub fn peaks(
    data: &[f32],
    channels: usize,
    from: usize,
    to: usize,
    buckets: usize,
) -> Vec<(f32, f32)> {
    let ch = channels.max(1);
    let frames = data.len() / ch;
    let to = to.min(frames);
    let mut out = vec![(0.0f32, 0.0f32); buckets];
    if to <= from || buckets == 0 {
        return out;
    }
    let span = (to - from) as f64 / buckets as f64;
    for (b, o) in out.iter_mut().enumerate() {
        let a = from + (b as f64 * span) as usize;
        let e = (from + ((b + 1) as f64 * span) as usize).clamp(a + 1, to);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        // Long spans: every frame is too many; a stride keeps it cheap and still honest.
        let stride = ((e - a) / 256).max(1);
        let mut i = a;
        while i < e {
            let v = data[i * ch..i * ch + ch].iter().sum::<f32>() / ch as f32;
            lo = lo.min(v);
            hi = hi.max(v);
            i += stride;
        }
        *o = (lo, hi);
    }
    out
}
