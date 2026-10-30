//! A track's per-note expression as the engine plays it (ADR 0080): one stream of
//! [`ExprEvent`]s on the timeline, sorted, built once per snapshot beside the notes
//! (`TrackSnapshot::expr`). Empty — nothing to split a block at, nothing sent — when no
//! note of the track has a curve, which is every project before schema 47.
//!
//! **Which notes.** Exactly those `snapshot::compile_events` plays: heard clips (the
//! playing lane, not muted, not audio or automation), notes not muted nor on a muted pad,
//! at each position the clip's window shows them, with the clip's groove; each event names
//! the note by the same voice tag its note-on carries (a copy that does not own the id
//! plays it as `snapshot::voice_tag`).
//!
//! **What.** For each curve of a heard note: its value at the note-on's tick (so the voice
//! starts where the player's finger was), every point inside the note at its own tick, the
//! moving parts between points on the timeline's [`STEP`](crate::clip_lanes::STEP) grid
//! (only values that changed), nothing at or after the note-off: the last value is held
//! into the release.

use super::{raw_at, ExprKind, NoteExpr};
use crate::model::{Clip, NoteId, Track};
use crate::time::Ticks;

/// One expression value for one sounding note, at an absolute tick.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ExprEvent {
    pub tick: Ticks,
    /// The voice tag of the note it addresses (its note-on's `NoteEvent::note`).
    pub note: NoteId,
    pub kind: ExprKind,
    /// In the kind's unit: semitones for the bend, 0…1 for pressure and timbre.
    pub value: f32,
}

/// Whether a clip's notes are heard (what `snapshot::compile_events` plays).
fn heard(clip: &Clip) -> bool {
    !(clip.muted || clip.lane != 0 || clip.audio.is_some() || clip.automation.is_some())
}

/// Every expression event of `track`, sorted by tick (then note, then kind).
pub fn compile_track(track: &Track, pool: &[crate::groove::Groove]) -> Vec<ExprEvent> {
    let any = track
        .clips
        .iter()
        .filter(|c| heard(c))
        .any(|c| c.notes.iter().any(|n| n.expr.is_some() && !n.muted));
    if !any {
        return Vec::new();
    }
    let muted_pads = track.instrument.pulsus().map(|k| {
        k.pads
            .iter()
            .filter(|p| p.muted)
            .map(|p| p.pitch)
            .collect::<Vec<_>>()
    });
    let plays = |n: &crate::model::Note| {
        !n.muted
            && !muted_pads
                .as_ref()
                .is_some_and(|pads| pads.contains(&n.pitch))
    };
    let clips: Vec<&Clip> = track.clips.iter().filter(|c| heard(c)).collect();
    let mut at = Vec::new();
    // Which (note id, clip id) pairs play at least once: the lowest clip of each id owns
    // it, as `snapshot::tell_copies_apart` decides.
    let mut pairs: Vec<(u64, u64)> = Vec::new();
    if clips.len() > 1 {
        for clip in &clips {
            let groove = groove_of(clip, pool);
            for n in clip.notes.iter().filter(|n| plays(n)) {
                if !heard_spans(clip, n, groove.as_ref(), &mut at).is_empty() {
                    pairs.push((n.id.0, clip.id.0));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
    }
    let owns = |note: u64, clip: u64| {
        let i = pairs.partition_point(|p| p.0 < note);
        pairs.get(i).is_none_or(|p| p.0 != note || p.1 == clip)
    };
    let mut out = Vec::new();
    for clip in &clips {
        let groove = groove_of(clip, pool);
        for n in clip.notes.iter().filter(|n| plays(n)) {
            let Some(e) = n.expr.as_deref() else {
                continue;
            };
            let tag = if owns(n.id.0, clip.id.0) {
                n.id
            } else {
                NoteId(crate::snapshot::voice_tag(n.id.0, clip.id.0))
            };
            for (on, off) in heard_spans(clip, n, groove.as_ref(), &mut at) {
                emit(e, tag, on, off, &mut out);
            }
        }
    }
    out.sort_by_key(|e: &ExprEvent| (e.tick, e.note, e.kind));
    out
}

type ResolvedGroove = (crate::groove::Groove, crate::groove::ClipGroove);

fn groove_of(clip: &Clip, pool: &[crate::groove::Groove]) -> Option<ResolvedGroove> {
    clip.groove
        .and_then(|g| crate::groove::resolve(pool, &g.source).map(|x| (x, g)))
}

/// Where a note sounds, `(on, off)` per position the window shows it — the arithmetic of
/// `snapshot::compile_events`, kept in step by the tests.
fn heard_spans(
    clip: &Clip,
    n: &crate::model::Note,
    groove: Option<&ResolvedGroove>,
    at: &mut Vec<Ticks>,
) -> Vec<(Ticks, Ticks)> {
    let groove = groove.map(|(g, a)| (g, a));
    clip.timeline_positions(n.start, at);
    let mut spans = Vec::new();
    for heard_at in at.iter().copied() {
        let (on, _, length) = crate::groove::heard(groove, clip, heard_at, n.velocity, n.length);
        let mut off = (on + length).min(clip.end());
        if let Some(period) = clip.period() {
            off = off.min(heard_at + period);
        }
        if off > on {
            spans.push((on, off));
        }
    }
    spans
}

/// One heard note's curves as events in `[on, off)`.
fn emit(e: &NoteExpr, note: NoteId, on: Ticks, off: Ticks, out: &mut Vec<ExprEvent>) {
    let step = crate::clip_lanes::STEP;
    let len = off.0 - on.0;
    for kind in ExprKind::ALL {
        let c = e.curve(kind);
        let Some(v0) = raw_at(c, 0) else {
            continue;
        };
        let mut push = |rel: i64, raw: i64, last: &mut i64| {
            out.push(ExprEvent {
                tick: Ticks(on.0 + rel),
                note,
                kind,
                value: kind.read(raw) as f32,
            });
            *last = raw;
        };
        let mut last = v0;
        push(0, v0, &mut last);
        // Each stretch between two points inside the note: its grid ticks, then the point.
        let inside: Vec<_> = c.iter().filter(|p| p.0 > 0 && p.0 < len).copied().collect();
        let mut prev_at = 0i64;
        let mut next = inside.iter().peekable();
        loop {
            let seg_end = next.peek().map_or(len, |p| p.0);
            // Grid ticks of the timeline strictly between `prev_at` and `seg_end`.
            let first = (on.0 + prev_at).div_euclid(step) * step + step;
            let mut g = first;
            while g < on.0 + seg_end {
                let rel = g - on.0;
                if let Some(v) = raw_at(c, rel) {
                    if v != last {
                        push(rel, v, &mut last);
                    }
                }
                g += step;
            }
            match next.next() {
                Some(p) => {
                    if p.1 != last {
                        push(p.0, p.1, &mut last);
                    }
                    prev_at = p.0;
                }
                None => break,
            }
        }
    }
}
