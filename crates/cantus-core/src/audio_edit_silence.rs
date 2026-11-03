//! *Remove silence* (ADR 0096): the stretches of a selection that stay below a level for
//! longer than a minimum are cut, each leaving `keep_ms` of its quiet behind (half on each
//! side) so the breath between two phrases survives. The level is the loudest sample of
//! any channel in a 10 ms block, so a block with one click in it is not silence.

use super::{AudioBuf, Sel};
use cantus_dsp::math;

/// How silence is found and what is left of it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Below this, dBFS, a block is silent.
    pub threshold_db: f32,
    /// A quiet stretch must last this long to be cut, ms.
    pub min_ms: f32,
    /// What stays of each cut stretch, ms.
    pub keep_ms: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            threshold_db: -50.0,
            min_ms: 300.0,
            keep_ms: 80.0,
        }
    }
}

/// The quiet stretches `sel` has under `s`, as frame ranges (each at least `min_ms`).
pub fn find(buf: &AudioBuf, sel: Sel, s: &Settings) -> Vec<(usize, usize)> {
    let n = buf.channels.max(1) as usize;
    let sel = sel.clamp(buf.frames());
    let block = ((buf.rate as f32 * 0.010) as usize).max(1);
    let thr = math::db_to_gain(s.threshold_db);
    let min = (buf.rate as f32 * s.min_ms / 1000.0) as usize;
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    let mut at = sel.start;
    while at < sel.end {
        let end = (at + block).min(sel.end);
        let quiet = buf.data[at * n..end * n].iter().all(|v| v.abs() < thr);
        match (quiet, start) {
            (true, None) => start = Some(at),
            (false, Some(a)) => {
                if at - a >= min {
                    runs.push((a, at));
                }
                start = None;
            }
            _ => {}
        }
        at = end;
    }
    if let Some(a) = start {
        if sel.end - a >= min {
            runs.push((a, sel.end));
        }
    }
    runs
}

/// `buf` with the quiet stretches of `sel` cut out, each join crossfaded over `xfade` frames
/// (equal power, [`super::join`]) with the samples on both sides of the cut — never past
/// the selection: the crossfade is as long as the audio after the cut that is still in it.
pub fn remove(
    buf: &AudioBuf,
    sel: Sel,
    s: &Settings,
    xfade: usize,
) -> Result<AudioBuf, &'static str> {
    if !(s.threshold_db.is_finite() && (-120.0..=0.0).contains(&s.threshold_db)) {
        return Err("threshold");
    }
    if !(s.min_ms.is_finite() && (10.0..=60_000.0).contains(&s.min_ms)) {
        return Err("minimum length");
    }
    if !(s.keep_ms.is_finite() && (0.0..=10_000.0).contains(&s.keep_ms)) {
        return Err("kept length");
    }
    let n = buf.channels.max(1) as usize;
    let half = (buf.rate as f32 * s.keep_ms / 2000.0) as usize;
    let cuts: Vec<(usize, usize)> = find(buf, sel, s)
        .into_iter()
        .filter(|(a, b)| b - a > 2 * half)
        .map(|(a, b)| (a + half, b - half))
        .collect();
    if cuts.is_empty() {
        return Ok(buf.clone());
    }
    let mut out = Vec::with_capacity(buf.data.len());
    let mut from = 0usize;
    let mut joins = Vec::new();
    for (a, b) in &cuts {
        out.extend_from_slice(&buf.data[from * n..a * n]);
        joins.push((out.len() / n, *a, *b));
        from = *b;
    }
    out.extend_from_slice(&buf.data[from * n..]);
    for (j, a, b) in joins {
        let len = xfade.min(b - a).min(sel.end.saturating_sub(b));
        super::join::blend(
            &mut out,
            n,
            j,
            len,
            |i, c| buf.data[(a + i) * n + c],
            |i, c| buf.data[(b + i) * n + c],
        );
    }
    Ok(AudioBuf::new(buf.rate, buf.channels, out))
}
