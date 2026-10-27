//! Edits of controller lanes as pure functions over points (ADR 0068): what the piano
//! roll's pencil, line and curve tools write, what moving, copying and pasting notes carry
//! with them, and what a glue makes of several clips' lanes. The caller wraps the result
//! in `Command::SetClipLanes` — one undo step.

use super::{ClipLane, LaneKind};
use crate::automation::{segment, AutoPoint, Shape};
use crate::model::Clip;
use crate::time::Ticks;

/// The curve's value just before `t` (the last point before it, or the value it holds
/// there): what is heard an instant before an overwrite begins.
fn before(points: &[AutoPoint], t: Ticks) -> Option<f32> {
    let first = points.first()?;
    let i = points.partition_point(|p| p.tick < t);
    if i == 0 {
        return Some(first.value);
    }
    let p = points[i - 1];
    Some(match points.get(i) {
        // Between a point before `t` and one at or after it: the curve arriving there.
        Some(q) => crate::automation::segment(p, *q, t.0 as f64),
        None => p.value,
    })
}

/// The curve's value just after `t` (the last point at `t`, or the value there).
fn after(points: &[AutoPoint], t: Ticks) -> Option<f32> {
    ClipLane {
        kind: LaneKind::Pressure,
        points: points.to_vec(),
    }
    .value_at(t.0 as f64)
}

/// `points` with `[a, b]` replaced by `new` (sorted, inside `[a, b]`): what was heard
/// outside the stretch stays — the old value is kept up to `a` and comes back at `b`, each
/// as a jump, so drawing over two beats changes those two beats only.
pub fn overwrite(points: &[AutoPoint], a: Ticks, b: Ticks, new: &[AutoPoint]) -> Vec<AutoPoint> {
    let (a, b) = (a.min(b), a.max(b));
    // A point at `a` is where the segment before it arrives (a pulse or stairs do not
    // reach it by themselves): it stays its target.
    let old_in = match points.iter().find(|p| p.tick == a) {
        Some(p) => Some(p.value),
        None => before(points, a),
    };
    let old_out = after(points, b);
    let had = !points.is_empty();
    let mut out: Vec<AutoPoint> = points.iter().copied().filter(|p| p.tick < a).collect();
    if had {
        if let Some(v) = old_in {
            if out.last().is_none_or(|p| p.value != v || p.tick != a) {
                let mut p = AutoPoint::new(a, v);
                p.shape = Shape::Hold;
                out.push(p);
            }
        }
    }
    out.extend(new.iter().copied().filter(|p| p.tick >= a && p.tick <= b));
    let tail: Vec<AutoPoint> = points.iter().copied().filter(|p| p.tick > b).collect();
    if had && !tail.is_empty() {
        if let Some(v) = old_out {
            if let Some(last) = out.last_mut() {
                last.shape = Shape::Hold;
            }
            out.push(AutoPoint::new(b, v));
        }
    }
    out.extend(tail);
    out.sort_by_key(|p| p.tick);
    dedupe(out)
}

/// Drop points that change nothing: a run of three or more at one value keeps its ends,
/// two at one tick and value are one.
fn dedupe(points: Vec<AutoPoint>) -> Vec<AutoPoint> {
    let mut out: Vec<AutoPoint> = Vec::with_capacity(points.len());
    for p in points {
        if let Some(l) = out.last() {
            if l.tick == p.tick && l.value == p.value {
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// A straight line (or, with `tension`, a curve bent towards one end) from `(t0, v0)` to
/// `(t1, v1)` over the stretch between them: the line and curve tools.
pub fn line(
    points: &[AutoPoint],
    t0: Ticks,
    v0: f32,
    t1: Ticks,
    v1: f32,
    tension: f32,
) -> Vec<AutoPoint> {
    let ((ta, va), (tb, vb)) = if t0 <= t1 {
        ((t0, v0), (t1, v1))
    } else {
        ((t1, v1), (t0, v0))
    };
    let mut p = AutoPoint::new(ta, va.clamp(0.0, 1.0));
    p.tension = if t0 <= t1 { tension } else { -tension }.clamp(-1.0, 1.0);
    let q = AutoPoint::new(tb, vb.clamp(0.0, 1.0));
    overwrite(points, ta, tb, &[p, q])
}

/// Freehand samples (`(tick, value)`, in drawing order) written over the stretch they
/// cover, thinned to the bake's tolerance (1/500 of the travel); a switch's lane steps.
pub fn pencil(points: &[AutoPoint], kind: LaneKind, samples: &[(Ticks, f32)]) -> Vec<AutoPoint> {
    if samples.is_empty() {
        return points.to_vec();
    }
    let mut s: Vec<(Ticks, f32)> = samples
        .iter()
        .map(|(t, v)| (*t, v.clamp(0.0, 1.0)))
        .collect();
    s.sort_by_key(|x| x.0);
    // At one tick, the last drawn.
    let mut uniq: Vec<(Ticks, f32)> = Vec::with_capacity(s.len());
    for x in s {
        match uniq.last_mut() {
            Some(l) if l.0 == x.0 => l.1 = x.1,
            _ => uniq.push(x),
        }
    }
    let (a, b) = (uniq[0].0, uniq[uniq.len() - 1].0);
    let new: Vec<AutoPoint> = if kind.is_switch() {
        uniq.iter()
            .map(|(t, v)| {
                let mut p = AutoPoint::new(*t, if *v >= 0.5 { 1.0 } else { 0.0 });
                p.shape = Shape::Hold;
                p
            })
            .collect()
    } else {
        let ticks: Vec<i64> = uniq.iter().map(|x| x.0 .0).collect();
        let values: Vec<f32> = uniq.iter().map(|x| x.1).collect();
        crate::modulator::bake::thin(&ticks, &values, 0)
    };
    overwrite(points, a, b, &dedupe(new))
}

/// The curve over `[a, b)` as points relative to `a`, with its values at both ends (what
/// a copy carries, so a paste plays what was heard). `None` when the lane has no points.
/// [`span_on`] on the content's own grid.
pub fn span(points: &[AutoPoint], a: Ticks, b: Ticks) -> Option<Vec<AutoPoint>> {
    span_on(points, a, b, Ticks::ZERO)
}

/// [`span`], its shapes kept (the fix-round review): a point at `a` is kept as it is; a
/// stretch that begins or ends inside a segment keeps that segment's shape — a flat or
/// held one as a hold, a straight one as a line from the value there — and any other
/// (a curve, an S, stairs, a pulse, a wave) is written out as holds on the timeline's
/// [`super::STEP`] grid, the values the engine samples there (`timeline = content +
/// shift`), so a piece plays what the whole played, to the bit on that grid.
pub fn span_on(points: &[AutoPoint], a: Ticks, b: Ticks, shift: Ticks) -> Option<Vec<AutoPoint>> {
    if points.is_empty() || b <= a {
        return None;
    }
    let mut out: Vec<AutoPoint> = Vec::new();
    let held = |t: Ticks, v: f32| {
        let mut p = AutoPoint::new(t - a, v);
        p.shape = Shape::Hold;
        p
    };
    // The segment `[s, e)` of `p → q` (`p.tick <= s < e <= q.tick`), written out.
    let piece = |out: &mut Vec<AutoPoint>, p: AutoPoint, q: AutoPoint, s: Ticks, e: Ticks| {
        let at = |t: Ticks| {
            if t == p.tick {
                p.value
            } else {
                segment(p, q, t.0 as f64)
            }
        };
        let whole = s == p.tick && e == q.tick;
        let flat = p.value == q.value || p.shape == Shape::Hold;
        let line = p.shape == Shape::Curve && p.tension == 0.0;
        if s == p.tick && (whole || flat || line) {
            out.push(AutoPoint {
                tick: p.tick - a,
                ..p
            });
        } else if flat {
            out.push(held(s, p.value));
        } else if line {
            out.push(AutoPoint::new(s - a, at(s)));
        } else {
            out.push(held(s, at(s)));
            let step = super::STEP;
            let mut g = (s + shift).0.div_euclid(step) * step + step - shift.0;
            while g < e.0 {
                out.push(held(Ticks(g), at(Ticks(g))));
                g += step;
            }
        }
    };
    // Before the first point and after the last, the curve holds its end's value.
    let first = points[0];
    if a < first.tick {
        out.push(held(a, first.value));
    }
    // The points from the last at or before `a`, segment by segment.
    let i0 = points.partition_point(|p| p.tick <= a).saturating_sub(1);
    for i in i0..points.len() {
        let p = points[i];
        if p.tick >= b {
            break;
        }
        // Several points at one tick: a jump, heard as its last.
        if points.get(i + 1).is_some_and(|n| n.tick == p.tick) {
            continue;
        }
        let s = p.tick.max(a);
        match points.get(i + 1) {
            Some(&q) => piece(&mut out, p, q, s, q.tick.min(b)),
            None => out.push(if p.tick >= a {
                AutoPoint {
                    tick: p.tick - a,
                    ..p
                }
            } else {
                held(a, p.value)
            }),
        }
    }
    // The curve up to the end, then nothing after: the value the stretch reaches.
    if let Some(last) = out.last().copied() {
        // A segment that ends on a point at `b` travels to that point (a pulse or stairs
        // do not arrive at it), one cut at `b` to its value there.
        let v = match points.iter().find(|p| p.tick == b) {
            Some(q) => q.value,
            None => before(points, b).unwrap_or(last.value),
        };
        if last.tick < b - a {
            out.push(AutoPoint::new(b - a, v));
        }
    }
    Some(out)
}

/// Every lane's [`span`] over `[a, b)`: what copying notes carries.
pub fn spans(lanes: &[ClipLane], a: Ticks, b: Ticks) -> Vec<ClipLane> {
    spans_on(lanes, a, b, Ticks::ZERO)
}

/// Every lane's [`span_on`] over `[a, b)`, on the timeline's grid (`timeline = content +
/// shift`).
pub fn spans_on(lanes: &[ClipLane], a: Ticks, b: Ticks, shift: Ticks) -> Vec<ClipLane> {
    lanes
        .iter()
        .filter_map(|l| span_on(&l.points, a, b, shift).map(|p| ClipLane::new(l.kind, p)))
        .collect()
}

/// What `clip`'s lanes play over the timeline's `[a, b)` (inside its window), laid out
/// once from `a` for a clip that shows its content from 0 and does not loop: each
/// repetition's stretch in turn, then each kind's rest at `b - a`. What a piece written out
/// of a looping or trimmed clip carries — a time-range cut or copy, *Bake to notes* — so it
/// plays the controllers as they were heard (round 31's integrated review).
pub fn heard(clip: &Clip, a: Ticks, b: Ticks) -> Vec<ClipLane> {
    let (a, b) = (a.max(clip.start), b.min(clip.end()));
    let mut out: Vec<ClipLane> = Vec::new();
    if b <= a || clip.lanes.iter().all(|l| l.points.is_empty()) {
        return out;
    }
    let period = clip.period();
    let mut rep = clip.start;
    loop {
        let len = period.map_or(clip.length, |p| p.min(clip.end() - rep));
        let (s, e) = (rep.max(a), (rep + len).min(b));
        if s < e {
            let c0 = clip.offset + (s - rep);
            out = paste(
                &out,
                &spans_on(&clip.lanes, c0, c0 + (e - s), s - c0),
                s - a,
            );
        }
        rep += len;
        if period.is_none() || rep >= b || len <= Ticks::ZERO {
            break;
        }
    }
    // After the stretch, the rest (as the clip rested where it ended).
    let end = b - a;
    for l in &mut out {
        let rest = l.kind.rest();
        if l.points
            .last()
            .is_some_and(|p| p.tick == end && p.value == rest)
        {
            continue;
        }
        if let Some(p) = l.points.last_mut() {
            p.shape = Shape::Hold;
        }
        l.points.push(AutoPoint::new(end, rest));
    }
    out
}

/// `lanes` with `copied` (relative points, a span each) written at `at`: a paste. A kind
/// the clip has no lane for gets one, resting outside the paste.
pub fn paste(lanes: &[ClipLane], copied: &[ClipLane], at: Ticks) -> Vec<ClipLane> {
    let mut out = lanes.to_vec();
    for c in copied.iter().filter(|c| !c.points.is_empty()) {
        let moved: Vec<AutoPoint> = c
            .points
            .iter()
            .map(|p| AutoPoint {
                tick: p.tick + at,
                ..*p
            })
            .collect();
        let (a, b) = (moved[0].tick, moved[moved.len() - 1].tick);
        match out.iter_mut().find(|l| l.kind == c.kind) {
            Some(l) => l.points = overwrite(&l.points, a, b, &moved),
            None => {
                let rest = c.kind.rest();
                let mut pts = Vec::with_capacity(moved.len() + 2);
                if a > Ticks::ZERO {
                    let mut r = AutoPoint::new(a, rest);
                    r.shape = Shape::Hold;
                    pts.push(r);
                }
                pts.extend(moved.iter().copied());
                if let Some(last) = pts.last_mut() {
                    last.shape = Shape::Hold;
                }
                pts.push(AutoPoint::new(b, rest));
                out.push(ClipLane::new(c.kind, dedupe(pts)));
            }
        }
    }
    super::sort_lanes(&mut out);
    out
}

/// `lanes` with what lies in `[a, b)` moved by `dt` (a drag of notes carrying their
/// controllers, Cubase's *controllers follow events*): the stretch is lifted out — its
/// neighbours meet across the gap — and written at its new place.
pub fn shift(lanes: &[ClipLane], a: Ticks, b: Ticks, dt: Ticks) -> Vec<ClipLane> {
    if dt == Ticks::ZERO || b <= a {
        return lanes.to_vec();
    }
    let lifted = spans(lanes, a, b);
    let mut out: Vec<ClipLane> = lanes
        .iter()
        .map(|l| {
            let kept: Vec<AutoPoint> = l
                .points
                .iter()
                .copied()
                .filter(|p| p.tick < a || p.tick >= b)
                .collect();
            ClipLane::new(l.kind, kept)
        })
        .collect();
    let to = (a + dt).max(Ticks::ZERO);
    for c in &lifted {
        let moved: Vec<AutoPoint> = c
            .points
            .iter()
            .map(|p| AutoPoint {
                tick: p.tick + to,
                ..*p
            })
            .collect();
        if let (Some(l), Some(first), Some(last)) = (
            out.iter_mut().find(|l| l.kind == c.kind),
            moved.first(),
            moved.last(),
        ) {
            l.points = overwrite(&l.points, first.tick, last.tick, &moved);
        }
    }
    out.retain(|l| !l.points.is_empty());
    out
}

/// The lanes a glue of `ordered` (sorted by start; the first is kept) gives the first
/// clip, in its content frame: each later clip's curve inside its window written over that
/// stretch. `None` when nothing changes (the halves of a split: one curve, one frame).
pub fn glue(ordered: &[&Clip]) -> Option<Vec<ClipLane>> {
    let first = *ordered.first()?;
    let mut lanes = first.lanes.clone();
    let mut changed = false;
    for c in &ordered[1..] {
        // A kind the glued clip has and this clip lacks rests over this clip's window, as it
        // did when this clip played (its controller rested where the first clip ended).
        let shift = first.content_of(c.start) - c.start; // content = timeline + shift
        let (a, b) = (c.start + shift, c.end() + shift);
        for l in lanes
            .iter_mut()
            .filter(|l| c.lanes.iter().all(|m| m.kind != l.kind))
        {
            let mut r0 = AutoPoint::new(a, l.kind.rest());
            r0.shape = Shape::Hold;
            l.points = overwrite(&l.points, a, b, &[r0, AutoPoint::new(b, l.kind.rest())]);
            changed = true;
        }
        if c.lanes.is_empty() {
            continue;
        }
        let same_frame = first.content_of(c.start) == c.offset && c.period().is_none();
        if same_frame && c.lanes == first.lanes {
            continue;
        }
        changed = true;
        // What the clip plays, on the timeline, as relative spans per repetition.
        let period = c.period();
        let mut rep = c.start;
        loop {
            let len = period.map_or(c.length, |p| p.min(c.end() - rep));
            let copied = spans_on(&c.lanes, c.offset, c.offset + len, rep - c.offset);
            lanes = paste(&lanes, &copied, rep + shift);
            rep += len;
            if period.is_none() || rep >= c.end() {
                break;
            }
        }
    }
    changed.then_some(lanes)
}

/// MIDI messages of one kind (`(content tick, position)`, in time order) as a lane's
/// points: MIDI's own shape — each value holds until the next — with a value repeated or
/// overridden at its own tick dropped. A tick before the content's start is its start.
pub fn steps(events: &[(Ticks, f32)]) -> Vec<AutoPoint> {
    let mut out: Vec<AutoPoint> = Vec::with_capacity(events.len());
    for &(t, v) in events {
        let tick = t.max(Ticks::ZERO);
        let value = v.clamp(0.0, 1.0);
        match out.last_mut() {
            Some(l) if l.tick == tick => l.value = value,
            Some(l) if l.value == value => {}
            _ => {
                let mut p = AutoPoint::new(tick, value);
                p.shape = Shape::Hold;
                out.push(p);
            }
        }
    }
    // A step overridden at its tick may now repeat the one before it.
    out.dedup_by(|b, a| a.value == b.value);
    out
}

/// Messages of every kind (`(content tick, kind, position)`, in time order) as new lanes:
/// each kind at rest until its first message, then its [`steps`] — what a MIDI file's or a
/// recording's controllers become in a clip that had none.
pub fn lanes_from_messages(events: &[(Ticks, LaneKind, f32)]) -> Vec<ClipLane> {
    let mut kinds: Vec<LaneKind> = events
        .iter()
        .map(|e| e.1)
        .filter(|k| k.is_valid())
        .collect();
    kinds.sort();
    kinds.dedup();
    let mut out = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let mine: Vec<(Ticks, f32)> = events
            .iter()
            .filter(|e| e.1 == kind)
            .map(|e| (e.0, e.2))
            .collect();
        let pts = steps(&mine);
        let Some(first) = pts.first().copied() else {
            continue;
        };
        let mut lane = Vec::with_capacity(pts.len() + 1);
        if first.tick > Ticks::ZERO && first.value != kind.rest() {
            let mut r = AutoPoint::new(Ticks::ZERO, kind.rest());
            r.shape = Shape::Hold;
            lane.push(r);
        }
        lane.extend(pts);
        out.push(ClipLane::new(kind, lane));
    }
    super::sort_lanes(&mut out);
    out
}

/// A lane's curve as straight and held pieces only (what a format with no curves holds —
/// DAWproject's `linear` and `hold`): each point with whether it holds to the next, the
/// bent and patterned pieces sampled every 1/16 of a beat.
pub fn linearised(points: &[AutoPoint]) -> Vec<(Ticks, f32, bool)> {
    let step = crate::time::PPQ / 16;
    let mut out = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        let hold = p.shape == Shape::Hold;
        out.push((p.tick, p.value, hold));
        let Some(q) = points.get(i + 1) else {
            continue;
        };
        let straight = p.shape == Shape::Curve && p.tension == 0.0;
        if hold || straight || q.tick <= p.tick {
            continue;
        }
        let mut t = p.tick.0 + step;
        while t < q.tick.0 {
            out.push((
                Ticks(t),
                crate::automation::segment(*p, *q, t as f64),
                false,
            ));
            t += step;
        }
    }
    out
}
