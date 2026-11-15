//! r40:ripple — the tempo track and the metre changes through a time edit (ADR 0116 §2).
//!
//! **Inserting** `len` at `at`: every point at or after `at` moves `len` later (the tick-0
//! point stays: a tempo track starts at tick 0), and the new time holds the tempo the song
//! had at `at`. Where `at` falls inside a ramp, or a ramp ends at `at`, the ramp is cut there
//! — a point at `at` holding the ramp's value, and (inside a ramp) one at `at + len` that
//! ramps on to where it was going — so the bars before and after play exactly as they did.
//! The metre at `at` holds through the new time.
//!
//! **Deleting** `[from, to)`: the points inside go, the later ones move back, and the song at
//! `from` goes on as it was at `to` — a point is added at `from` with the tempo there (and
//! the ramp it was in) unless the step before already plays it; likewise a metre change.
//! A point at the cut that the music does not need (the step before holds the same tempo, or
//! a ramp passes through it exactly) is left out, so a deletion of what an insertion made
//! gives the tempo track back as it was.

use crate::model::Project;
use crate::tempo::{MetreChange, TempoPoint};
use crate::time::{Ticks, TimeSignature};

/// The tempo on a ramp from `p` toward `n` at `t`, in the map's own arithmetic (its slope
/// times the ticks in), so the point added there is the value the map plays.
fn on_ramp(p: &TempoPoint, n: &TempoPoint, t: Ticks) -> f64 {
    if t == n.tick {
        return n.bpm;
    }
    let k = (n.bpm - p.bpm) / (n.tick.0 - p.tick.0) as f64;
    p.bpm + k * (t.0 - p.tick.0) as f64
}

/// The tempo track after inserting `len` at `at`: `(bpm at tick 0, points)`.
pub fn insert_tempo(p: &Project, at: Ticks, len: Ticks) -> (f64, Vec<TempoPoint>) {
    let pts = &p.tempo_points;
    if pts.is_empty() || len <= Ticks::ZERO {
        return (p.tempo_bpm, pts.clone());
    }
    let shifted = |q: &TempoPoint| TempoPoint {
        tick: q.tick + len,
        ..*q
    };
    let mut out = Vec::with_capacity(pts.len() + 2);
    if at <= Ticks::ZERO {
        // The tick-0 point stays; the new time holds its tempo.
        let p0 = pts[0];
        out.push(TempoPoint { ramp: false, ..p0 });
        if p0.ramp && pts.len() > 1 {
            out.push(TempoPoint { tick: len, ..p0 });
        }
        out.extend(pts[1..].iter().map(shifted));
        return (p.tempo_bpm, out);
    }
    let before: Vec<TempoPoint> = pts.iter().copied().filter(|q| q.tick < at).collect();
    let after: Vec<TempoPoint> = pts.iter().copied().filter(|q| q.tick >= at).collect();
    out.extend(before.iter().copied());
    if let (Some(prev), Some(next)) = (before.last(), after.first()) {
        if prev.ramp {
            let v = on_ramp(prev, next, at);
            out.push(TempoPoint {
                tick: at,
                bpm: v,
                ramp: false,
            });
            if next.tick > at {
                out.push(TempoPoint {
                    tick: at + len,
                    bpm: v,
                    ramp: true,
                });
            }
        }
    }
    out.extend(after.iter().map(shifted));
    (p.tempo_bpm, out)
}

/// What the map plays at `t`: the tempo and whether a ramp runs on from there.
fn state_at(pts: &[TempoPoint], t: Ticks) -> Option<(f64, bool)> {
    let i = pts.iter().rposition(|q| q.tick <= t)?;
    let q = pts[i];
    match pts.get(i + 1) {
        Some(n) if q.ramp => Some((on_ramp(&q, n, t), true)),
        _ => Some((q.bpm, false)),
    }
}

/// Whether the point at index `i` changes nothing the map plays: the step before holds its
/// tempo and it holds too, or a ramp runs through it exactly and on.
fn needless(pts: &[TempoPoint], i: usize) -> bool {
    if i == 0 {
        return false;
    }
    let (p, x) = (pts[i - 1], pts[i]);
    let last = i + 1 == pts.len();
    if !p.ramp {
        return p.bpm == x.bpm && (!x.ramp || last);
    }
    match pts.get(i + 1) {
        Some(n) => x.ramp && on_ramp(&p, n, x.tick) == x.bpm,
        // The last point: a ramp toward its own tempo is a hold, as it is without it.
        None => p.bpm == x.bpm,
    }
}

/// The tempo track after deleting `[from, to)`: `(bpm at tick 0, points)`.
pub fn delete_tempo(p: &Project, from: Ticks, to: Ticks) -> (f64, Vec<TempoPoint>) {
    let pts = &p.tempo_points;
    if pts.is_empty() || to <= from {
        return (p.tempo_bpm, pts.clone());
    }
    let len = to - from;
    let Some((v_out, ramp_out)) = state_at(pts, to) else {
        return (p.tempo_bpm, pts.clone());
    };
    let mut out: Vec<TempoPoint> = pts.iter().copied().filter(|q| q.tick < from).collect();
    let at_seam = pts.iter().any(|q| q.tick == to);
    if !at_seam {
        out.push(TempoPoint {
            tick: from,
            bpm: v_out,
            ramp: ramp_out,
        });
    }
    out.extend(pts.iter().filter(|q| q.tick >= to).map(|q| TempoPoint {
        tick: q.tick - len,
        ..*q
    }));
    if let Some(i) = out.iter().position(|q| q.tick == from) {
        if from > Ticks::ZERO && needless(&out, i) {
            out.remove(i);
        }
    }
    // The last point's ramp has nothing to ramp to: it holds (as the map plays it).
    let bpm = out.first().map_or(p.tempo_bpm, |q| q.bpm);
    (bpm, out)
}

/// The metre in effect at `t` in a metre list (`sig` before any change).
fn metre_at(sig: TimeSignature, ms: &[MetreChange], t: Ticks) -> TimeSignature {
    ms.iter()
        .rev()
        .find(|m| m.tick <= t)
        .map_or(sig, |m| m.signature)
}

/// The metre changes after inserting `len` at `at`: `(the metre at tick 0, changes)`. The
/// tick-0 change stays; the rest at or after `at` move.
pub fn insert_metres(p: &Project, at: Ticks, len: Ticks) -> (TimeSignature, Vec<MetreChange>) {
    let ms = &p.metre_changes;
    let out = ms
        .iter()
        .map(|m| {
            if m.tick >= at && m.tick > Ticks::ZERO {
                MetreChange {
                    tick: m.tick + len,
                    ..*m
                }
            } else {
                *m
            }
        })
        .collect();
    (p.time_signature, out)
}

/// The metre changes after deleting `[from, to)`: the ones inside go, the later ones move
/// back, and the metre at `to` goes on from `from` (a change added there when the bars
/// before have another).
pub fn delete_metres(p: &Project, from: Ticks, to: Ticks) -> (TimeSignature, Vec<MetreChange>) {
    let ms = &p.metre_changes;
    if ms.is_empty() || to <= from {
        return (p.time_signature, ms.clone());
    }
    let len = to - from;
    let m_out = metre_at(p.time_signature, ms, to);
    let mut out: Vec<MetreChange> = ms.iter().copied().filter(|m| m.tick < from).collect();
    let before = out.last().map(|m| m.signature);
    let at_seam = ms.iter().any(|m| m.tick == to);
    if !at_seam && before != Some(m_out) {
        out.push(MetreChange {
            tick: from,
            signature: m_out,
        });
    }
    out.extend(ms.iter().filter(|m| m.tick >= to).map(|m| MetreChange {
        tick: m.tick - len,
        ..*m
    }));
    // A change at the cut that the bars before already have says nothing.
    if from > Ticks::ZERO {
        if let Some(i) = out.iter().position(|m| m.tick == from) {
            if i > 0 && out[i - 1].signature == out[i].signature {
                out.remove(i);
            }
        }
    }
    let sig = out.first().map_or(p.time_signature, |m| m.signature);
    (sig, out)
}

// --- a section's clock carried by an inserting drag (ADR 0116 §3) ------------------------

/// The tempo over `[from, to)` relative to `from`: the first point at 0 is what plays at
/// `from` (its tempo, and whether a ramp runs on), then every point inside. Empty without a
/// tempo track.
pub fn tempo_span(p: &Project, from: Ticks, to: Ticks) -> Vec<TempoPoint> {
    let pts = &p.tempo_points;
    let Some((bpm, ramp)) = state_at(pts, from) else {
        return Vec::new();
    };
    let mut out = vec![TempoPoint {
        tick: Ticks::ZERO,
        bpm,
        ramp,
    }];
    out.extend(
        pts.iter()
            .filter(|q| q.tick > from && q.tick < to)
            .map(|q| TempoPoint {
                tick: q.tick - from,
                ..*q
            }),
    );
    out
}

/// The tempo track with `seg` (a [`tempo_span`]) laid over `[at, at + len)`: what plays
/// before `at` and from `at + len` on is kept. `(bpm at tick 0, points)`.
pub fn paste_tempo(
    p: &Project,
    at: Ticks,
    len: Ticks,
    seg: &[TempoPoint],
) -> (f64, Vec<TempoPoint>) {
    let pts = &p.tempo_points;
    if pts.is_empty() || seg.is_empty() || len <= Ticks::ZERO {
        return (p.tempo_bpm, pts.clone());
    }
    let end = at + len;
    let after = state_at(pts, end);
    let mut out: Vec<TempoPoint> = pts.iter().copied().filter(|q| q.tick < at).collect();
    out.extend(seg.iter().filter(|q| q.tick < len).map(|q| TempoPoint {
        tick: q.tick + at,
        ..*q
    }));
    if !pts.iter().any(|q| q.tick == end) {
        if let Some((bpm, ramp)) = after {
            out.push(TempoPoint {
                tick: end,
                bpm,
                ramp,
            });
        }
    }
    out.extend(pts.iter().copied().filter(|q| q.tick >= end));
    for t in [end, at] {
        if t > Ticks::ZERO {
            if let Some(i) = out.iter().position(|q| q.tick == t) {
                if needless(&out, i) {
                    out.remove(i);
                }
            }
        }
    }
    let bpm = out.first().map_or(p.tempo_bpm, |q| q.bpm);
    (bpm, out)
}

/// The metre over `[from, to)` relative to `from`: the metre at `from` at 0, then every
/// change inside. Empty when the song has no metre changes.
pub fn metre_span(p: &Project, from: Ticks, to: Ticks) -> Vec<MetreChange> {
    let ms = &p.metre_changes;
    if ms.is_empty() {
        return Vec::new();
    }
    let mut out = vec![MetreChange {
        tick: Ticks::ZERO,
        signature: metre_at(p.time_signature, ms, from),
    }];
    out.extend(
        ms.iter()
            .filter(|m| m.tick > from && m.tick < to)
            .map(|m| MetreChange {
                tick: m.tick - from,
                ..*m
            }),
    );
    out
}

/// The metre changes with `seg` (a [`metre_span`]) laid over `[at, at + len)`.
pub fn paste_metres(
    p: &Project,
    at: Ticks,
    len: Ticks,
    seg: &[MetreChange],
) -> (TimeSignature, Vec<MetreChange>) {
    let ms = &p.metre_changes;
    if ms.is_empty() || seg.is_empty() || len <= Ticks::ZERO {
        return (p.time_signature, ms.clone());
    }
    let end = at + len;
    let after = metre_at(p.time_signature, ms, end);
    let mut out: Vec<MetreChange> = ms.iter().copied().filter(|m| m.tick < at).collect();
    out.extend(seg.iter().filter(|m| m.tick < len).map(|m| MetreChange {
        tick: m.tick + at,
        ..*m
    }));
    if !ms.iter().any(|m| m.tick == end) {
        out.push(MetreChange {
            tick: end,
            signature: after,
        });
    }
    out.extend(ms.iter().copied().filter(|m| m.tick >= end));
    // A change at either seam that repeats the metre before it says nothing.
    for t in [end, at] {
        if let Some(i) = out.iter().position(|m| m.tick == t) {
            if i > 0 && out[i].signature == out[i - 1].signature {
                out.remove(i);
            }
        }
    }
    let sig = out.first().map_or(p.time_signature, |m| m.signature);
    (sig, out)
}
