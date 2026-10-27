//! A track's controller lanes as the engine plays them: one stream of [`CtlEvent`]s on the
//! timeline, sorted, built once per snapshot.
//!
//! **Where.** A lane is in content ticks, shown through its clip's window exactly as notes
//! are (`ClipWindow::timeline_positions`): each repetition of a looping clip shows the
//! content `[offset, offset + period)` again; a window that does not loop shows
//! `[offset, offset + length)`.
//!
//! **What.** At the start of each stretch the curve's value there (so a window that begins
//! inside a bend begins bent), then every point inside it at its own tick, the moving parts
//! between points sampled on the timeline's [`STEP`] grid (only values that changed), and
//! the lane's rest value where the clip ends. Two events of one kind at one tick: the last
//! wins, and a rest never overrides another clip's value at that tick (the halves of a
//! split meet without a dip).

use super::{ClipLane, CtlEvent, LaneKind, STEP};
use crate::model::{Clip, Track};
use crate::time::Ticks;

/// Whether a clip's lanes are heard: a note clip on the playing lane, not muted.
fn heard(clip: &Clip) -> bool {
    !(clip.muted
        || clip.lane != 0
        || clip.audio.is_some()
        || clip.automation.is_some()
        || clip.lanes.is_empty())
}

/// Every controller event of `track`'s heard clips, sorted by tick (at one tick, rests
/// first, then in clip order). Empty — nothing to split a block at — when no clip has a
/// lane.
pub fn compile_track(track: &Track) -> Vec<CtlEvent> {
    let mut out: Vec<(CtlEvent, bool)> = Vec::new();
    for clip in track.clips.iter().filter(|c| heard(c)) {
        for lane in clip.lanes.iter().filter(|l| !l.points.is_empty()) {
            emit_lane(clip, lane, &mut out);
        }
    }
    if out.is_empty() {
        return Vec::new();
    }
    // A rest where one clip ends while another heard clip with a lane of that kind still
    // plays (overlapping clips on one track) is that clip's value there instead: the
    // controller follows the clip that goes on, and rests only when the last one ends.
    let heard_clips: Vec<&Clip> = track.clips.iter().filter(|c| heard(c)).collect();
    for (e, rest) in out.iter_mut().filter(|(_, r)| *r) {
        let going_on = heard_clips.iter().find_map(|c| {
            if c.start > e.tick || c.end() <= e.tick {
                return None;
            }
            let lane = c
                .lanes
                .iter()
                .find(|l| l.kind == e.kind && !l.points.is_empty())?;
            lane.value_at(c.content_shown_at(e.tick)?.0 as f64)
        });
        if let Some(v) = going_on {
            e.value = v;
            *rest = false;
        }
    }
    // By tick (stable: clip order kept), rests first at a tick.
    out.sort_by_key(|(e, rest)| (e.tick, !*rest));
    // A rest that meets a value of its kind at its tick gives way (each tick's group is
    // small: one event per lane and clip at most, besides a jump's).
    let mut kept: Vec<CtlEvent> = Vec::with_capacity(out.len());
    let mut i = 0;
    while i < out.len() {
        let tick = out[i].0.tick;
        let j = i + out[i..].iter().take_while(|(e, _)| e.tick == tick).count();
        let group = &out[i..j];
        for (e, rest) in group {
            let met = *rest && group.iter().any(|(f, r)| !*r && f.kind == e.kind);
            if !met {
                kept.push(*e);
            }
        }
        i = j;
    }
    kept
}

/// The events one clip's lanes send, on the timeline (what `compile_track` merges).
pub fn controls_of_clip(clip: &Clip) -> Vec<CtlEvent> {
    let mut out = Vec::new();
    for lane in clip.lanes.iter().filter(|l| !l.points.is_empty()) {
        emit_lane(clip, lane, &mut out);
    }
    let mut v: Vec<CtlEvent> = out.into_iter().map(|(e, _)| e).collect();
    v.sort_by_key(|e| e.tick);
    v
}

/// One lane through its clip's window, each repetition in turn, then its rest.
fn emit_lane(clip: &Clip, lane: &ClipLane, out: &mut Vec<(CtlEvent, bool)>) {
    if clip.length <= Ticks::ZERO {
        return;
    }
    let period = clip.period();
    let mut rep = clip.start;
    loop {
        let (span, content0) = match period {
            Some(p) => (p.min(clip.end() - rep), clip.offset),
            None => (clip.length, clip.offset),
        };
        let shift = rep - content0; // timeline = content + shift
        stretch(lane, content0, content0 + span, shift, out);
        rep += span;
        if period.is_none() || rep >= clip.end() {
            break;
        }
    }
    out.push((
        CtlEvent {
            tick: clip.end(),
            kind: lane.kind,
            value: lane.kind.rest(),
        },
        true,
    ));
}

/// The curve over content `[ca, cb)`, placed at `content + shift`.
fn stretch(lane: &ClipLane, ca: Ticks, cb: Ticks, shift: Ticks, out: &mut Vec<(CtlEvent, bool)>) {
    if cb <= ca {
        return;
    }
    let kind = lane.kind;
    let pts = &lane.points;
    let mut last: Option<f32> = None;
    let mut push = |t: Ticks, v: f32, last: &mut Option<f32>| {
        if *last == Some(v) {
            return;
        }
        *last = Some(v);
        out.push((
            CtlEvent {
                tick: t + shift,
                kind,
                value: v,
            },
            false,
        ));
    };
    // The value where the stretch begins.
    if let Some(v) = lane.value_at(ca.0 as f64) {
        push(ca, v, &mut last);
    }
    // Then piece by piece: from `ca` (or a point) to the next point (or `cb`).
    let mut i = pts.partition_point(|p| p.tick <= ca);
    let mut from = ca;
    loop {
        let to = pts.get(i).map_or(cb, |p| p.tick.min(cb));
        // The moving part between `from` and `to`, on the timeline's grid.
        if i > 0 && i < pts.len() {
            let (p, q) = (pts[i - 1], pts[i]);
            let flat = p.value == q.value || p.shape == crate::automation::Shape::Hold;
            if !flat {
                let first = (from + shift).0.div_euclid(STEP) * STEP + STEP - shift.0;
                let mut t = first;
                while t < to.0 {
                    let v = crate::automation::segment(p, q, t as f64);
                    push(Ticks(t), v, &mut last);
                    t += STEP;
                }
            }
        }
        if to >= cb {
            break;
        }
        // The point itself (and every point at its tick: a jump).
        while let Some(p) = pts.get(i).filter(|p| p.tick == to) {
            // A jump's first point is heard for no time; its last is the value.
            let v = p.value;
            last = None;
            push(to, v, &mut last);
            i += 1;
        }
        from = to;
    }
}

/// Every kind `events` holds, each once, in first-heard order.
pub fn kinds(events: &[CtlEvent]) -> Vec<LaneKind> {
    let mut out: Vec<LaneKind> = Vec::new();
    for e in events {
        if !out.contains(&e.kind) {
            out.push(e.kind);
        }
    }
    out
}
