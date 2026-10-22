//! Where an audio clip's source plays on the timeline (ADR 0060 §5): the one mapping from a
//! clip's ticks to its source's seconds (and frames) and back, shared by the engine
//! (`add_clips`), the snapshot (a comp seam's room), and every interface site (the
//! waveform, the pitch editor, comping, the inspector), so what is drawn is what plays.
//!
//! **An unwarped clip** plays at its natural speed from its **content origin** — the
//! timeline tick `start − offset` where the source's first second is laid (ADR 0025's
//! origin) — one second of source per second of song through the tempo map, wherever the
//! window shows it. With one tempo this is exactly the old rule. Every window onto one
//! take shares the origin, so the tick arithmetic of every window edit — a split, a trim
//! of either edge, a comp's swipe and seams, glue — keeps what plays exactly, through any
//! tempo map (review 2: anchored at each window's start instead, every such edit made the
//! audio jump across a tempo change). Only a move or a copy to another place re-derives
//! the offset ([`relocate`]); a reversed window, which reads backwards from its end,
//! re-derives it at its edits ([`reanchor_reversed`]). The region's fine offset (`slip`,
//! frames at the source's rate) moves the source against the timeline.
//!
//! **A warped clip** (round 29, ADR 0061) hears its source through its warp markers: see
//! [`clip_frame_at`].

use crate::tempo::TempoMap;
use crate::time::{Ticks, PPQ};

/// Whether one held tempo covers `[a, b]` (either order), both ends included: where a
/// window's constant arithmetic is the map's exactly. Returns that tempo.
fn held_bpm(map: &TempoMap, a: f64, b: f64) -> Option<f64> {
    let (lo, hi) = (a.min(b), a.max(b));
    match map.held_at(lo) {
        Some((bpm, _, to)) if hi < to || (hi == to && map.bpm_at(hi) == bpm) => Some(bpm),
        _ => None,
    }
}

/// Whether one held tempo covers `[a, b]` (either order).
pub fn held_over(map: &TempoMap, a: Ticks, b: Ticks) -> bool {
    held_bpm(map, a.0 as f64, b.0 as f64).is_some()
}

/// Seconds of source before the clip's start: from its content origin to its start.
pub fn offset_seconds(map: &TempoMap, start: Ticks, offset: Ticks) -> f64 {
    let s = start.0 as f64;
    let origin = (start.0 - offset.0) as f64;
    match held_bpm(map, origin, s) {
        Some(bpm) => offset.to_seconds(bpm),
        None => map.seconds_between_f64(origin, s),
    }
}

/// The source's position, in seconds, under timeline tick `tick` of an unwarped clip that
/// starts at `start` with `offset` (the fine `slip` aside): the seconds from its content
/// origin to `tick`. Where one tempo holds over both, `(tick − (start − offset))` at that
/// tempo — the old rule.
pub fn source_seconds(map: &TempoMap, start: Ticks, offset: Ticks, tick: f64) -> f64 {
    let origin = (start.0 - offset.0) as f64;
    match held_bpm(map, origin, tick) {
        Some(bpm) => (tick - origin) / PPQ as f64 * 60.0 / bpm,
        None => map.seconds_between_f64(origin, tick),
    }
}

/// The timeline tick under `seconds` into the source of an unwarped clip: the inverse of
/// [`source_seconds`].
pub fn tick_at_source(map: &TempoMap, start: Ticks, offset: Ticks, seconds: f64) -> f64 {
    let origin = (start.0 - offset.0) as f64;
    if let Some((bpm, from, to)) = map.held_at(origin) {
        let t = origin + seconds * bpm / 60.0 * PPQ as f64;
        if t >= from && t <= to && held_bpm(map, origin, t).is_some() {
            return t;
        }
    }
    map.tick_after_seconds(origin, seconds)
}

/// The source frame (fractional, at `rate` frames a second) under `tick`, the region's
/// fine offset `slip_frames` added.
#[inline]
pub fn frame_at(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    slip_frames: f64,
    rate: f64,
    tick: f64,
) -> f64 {
    source_seconds(map, start, offset, tick) * rate + slip_frames
}

/// The timeline tick under source frame `frame` (the inverse of [`frame_at`]).
#[inline]
pub fn tick_at_frame(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    slip_frames: f64,
    rate: f64,
    frame: f64,
) -> f64 {
    tick_at_source(map, start, offset, (frame - slip_frames) / rate.max(1.0))
}

/// The length in ticks, from `start`, of `frames` frames of source at `rate` played at
/// natural speed (a recorded take, an imported file): rounded to the nearest tick.
pub fn length_ticks(map: &TempoMap, start: Ticks, frames: f64, rate: f64) -> Ticks {
    let end = tick_at_source(map, start, Ticks::ZERO, frames / rate.max(1.0));
    Ticks((end - start.0 as f64).round() as i64)
}

/// The `offset` (whole ticks) that puts `seconds` of source before a clip starting at
/// `start` (the inverse of [`offset_seconds`]): the ticks back from `start` that last
/// that long. Where the tempo holds, at that tempo — the old rule.
pub fn offset_for(map: &TempoMap, start: Ticks, seconds: f64) -> Ticks {
    let s = start.0 as f64;
    let back = map.tick_after_seconds(s, -seconds);
    match held_bpm(map, back, s) {
        Some(bpm) => Ticks::from_seconds(seconds, bpm),
        None => Ticks((s - back).round() as i64),
    }
}

// --- Editing an unwarped window (review 2: findings 0, 3, 43) --------------------------
//
// A window's edge edits keep its origin (tick arithmetic, exact). A move or a copy to
// another place, and a reversed window's edits, re-derive the offset in seconds: whole
// ticks there, and what they cannot say goes into the fine offset (`slip_frames`), as a
// recorded take's does.

/// The offset and fine offset that put `seconds` of source (the fine offset `slip`
/// aside) before a window starting at `at`: whole ticks, the rest in
/// frames at `rate`, added to `slip`.
pub fn anchor(map: &TempoMap, at: Ticks, seconds: f64, slip: i32, rate: f64) -> (Ticks, i32) {
    // Before the file (a window shown from before its first frame): negative, as it was.
    let offset = offset_for(map, at, seconds);
    let rest = (seconds - offset_seconds(map, at, offset)) * rate;
    let slip = (slip as f64 + rest.round()).clamp(i32::MIN as f64, i32::MAX as f64);
    (offset, slip as i32)
}

/// A **reversed** window (it plays its source backwards from its end) whose start moves
/// to `to` and whose end moves to `end`, playing what it played at the ticks it keeps:
/// its new `(offset, slip_frames)`. The source frame under its start is the one its last
/// sample reads, so it moves by the seconds the end moves (a split's left half, a
/// right-edge trim) and stays when only the start moves (a left-edge trim). Where one
/// tempo holds over all of it, the old tick arithmetic.
#[allow(clippy::too_many_arguments)]
pub fn reanchor_reversed(
    map: &TempoMap,
    start: Ticks,
    old_end: Ticks,
    offset: Ticks,
    slip: i32,
    rate: f64,
    to: Ticks,
    end: Ticks,
) -> (Ticks, i32) {
    let moved = offset + (old_end - end);
    let lo = (start - offset).min(to - moved);
    let hi = old_end.max(end);
    if held_over(map, lo, hi) {
        return (moved, slip);
    }
    let seconds = offset_seconds(map, start, offset)
        + map.seconds_between_f64(end.0 as f64, old_end.0 as f64);
    anchor(map, to, seconds, slip, rate)
}

/// A window's content slipped by `by` ticks under its start (review 3): the source under
/// the window's start moves by the seconds `by` lasts *there* — the content follows the
/// pointer whatever the tempo over its hidden part. Its new `(offset, slip_frames)`; the
/// tick arithmetic (`offset + by`) where one tempo holds over all of it.
pub fn slipped(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    slip: i32,
    rate: f64,
    by: Ticks,
) -> (Ticks, i32) {
    let origin = start - offset;
    let lo = origin.min(origin - by);
    let hi = start.max(start + by);
    if held_over(map, lo, hi) {
        return (offset + by, slip);
    }
    let s = start.0 as f64;
    let seconds = offset_seconds(map, start, offset) + map.seconds_between_f64(s, s + by.0 as f64);
    anchor(map, start, seconds, slip, rate)
}

/// Where a window of a block moved together lands (review 3): the block's anchor (its
/// earliest start) goes from `from_anchor` to `to_anchor`, and an unwarped window starting
/// at `start` lands the same seconds after it — so windows that met still meet, and the
/// windows of one take keep their content origin. Where the tempo over the block is the
/// same function at both places (`same`), the ticks exactly.
pub fn block_start(
    src: &TempoMap,
    from_anchor: Ticks,
    dst: &TempoMap,
    to_anchor: Ticks,
    start: Ticks,
    same: bool,
) -> Ticks {
    if same {
        return start + (to_anchor - from_anchor);
    }
    let secs = src.seconds_between(from_anchor, start);
    Ticks(dst.tick_after_seconds(to_anchor.0 as f64, secs).round() as i64)
}

// --- Moving several windows together (reviews 3–4) -----------------------------------

/// One clip of a block moved or pasted together.
#[derive(Copy, Clone, Debug)]
pub struct Member<'a> {
    pub clip: &'a crate::model::Clip,
    /// Its row (a track): the windows of one take are on one row.
    pub row: u64,
    /// Where it starts before the move (under the source map).
    pub from: Ticks,
    /// Its file's rate.
    pub rate: f64,
}

/// Where a window of a chain lands: its start, its window and fades, its fine offset.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Laid {
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    pub fade_in: Ticks,
    pub fade_out: Ticks,
    pub slip_frames: i32,
}

/// A window that can be in a chain: unwarped audio (or a warp that does not follow the
/// tempo) that does not loop.
fn chains_with(c: &crate::model::Clip) -> bool {
    c.audio
        .as_ref()
        .is_some_and(|a| !a.warp.as_ref().is_some_and(|w| w.follow))
        && !c.content_length.is_some_and(|p| p < c.length)
}

/// The **chains** of a block moved by `delta` from under `src` to under `dst` (review 4):
/// the windows of one take moved together — on one row, one file, one direction, sharing
/// their content origin and fine offset, or meeting end to start and playing on there
/// within a frame. A chain keeps its own shape through a tempo change: its first window
/// lands at its start plus `delta`, the others the same *seconds* after it (both edges, so
/// windows that met still meet), each playing the second of its file it played (windows
/// that shared an origin and fine offset share them still, exactly). `Some` for each window
/// of a chain of two or more whose tempo differs where it lands; `None` for every other
/// clip — it moves by `delta` and is relocated on its own ([`relocate`]), MIDI and audio
/// alike, so clips that do not belong to one take keep their bars.
pub fn lay_chains(
    src: &TempoMap,
    dst: &TempoMap,
    delta: Ticks,
    members: &[Member],
) -> Vec<Option<Laid>> {
    let n = members.len();
    let mut out = vec![None; n];
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    fn join(p: &mut [usize], a: usize, b: usize) {
        let (x, y) = (find(p, a), find(p, b));
        if x != y {
            p[x.max(y)] = x.min(y);
        }
    }
    // Each window's take: (row, file, direction).
    let take = |m: &Member| {
        let a = m.clip.audio.as_ref().expect("an audio window");
        (m.row, a.source, a.reverse)
    };
    let slip = |m: &Member| m.clip.audio.as_ref().map_or(0, |a| a.slip_frames);
    let eligible: Vec<usize> = (0..n).filter(|&i| chains_with(members[i].clip)).collect();
    // Sharing an origin and a fine offset (forwards).
    let mut shared: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
    // Meeting: by where a window starts.
    let mut starts: std::collections::HashMap<_, Vec<usize>> = std::collections::HashMap::new();
    for &i in &eligible {
        let m = &members[i];
        let (row, source, reverse) = take(m);
        if !reverse {
            let key = (row, source, m.from - m.clip.offset, slip(m));
            match shared.get(&key) {
                Some(&j) => join(&mut parent, i, j),
                None => {
                    shared.insert(key, i);
                }
            }
        }
        starts
            .entry((row, source, reverse, m.from))
            .or_default()
            .push(i);
    }
    // The frame a window plays at `tick` before the move (backwards when reversed).
    let frame = |m: &Member, tick: Ticks| {
        let a = m.clip.audio.as_ref().expect("an audio window");
        let s = f64::from(a.slip_frames);
        let o = m.clip.offset;
        if a.reverse {
            frame_at(src, m.from, o, s, m.rate, m.from.0 as f64)
                + src.seconds_between(tick, m.from + m.clip.length) * m.rate
        } else {
            frame_at(src, m.from, o, s, m.rate, tick.0 as f64)
        }
    };
    // A linked window sits within half a tick of its take's line (review 5) — the tick
    // rounded at its content origin, or at its start, the longer (review 6).
    let slack = |m: &Member| {
        if m.clip.link.is_none() {
            return 0.0;
        }
        let half = |t: f64| 0.5 * src.seconds_between_f64(t, t + 1.0) * m.rate;
        let origin = (m.from - m.clip.offset).0.max(0) as f64;
        half(origin).max(half(m.from.0 as f64))
    };
    for &i in &eligible {
        let m = &members[i];
        let (row, source, reverse) = take(m);
        let end = m.from + m.clip.length;
        if let Some(next) = starts.get(&(row, source, reverse, end)) {
            for &j in next {
                let reach = 1.0 + slack(m).max(slack(&members[j]));
                if (frame(m, end) - frame(&members[j], end)).abs() <= reach {
                    join(&mut parent, i, j);
                }
            }
        }
    }
    // A linked window that no longer shares its take's origin exactly (a re-anchor moved
    // it by whole ticks) joins the take whose line it reads within its reach, met or not
    // (review 6: a take with a gap lost it).
    // One sorted pass a take (final review: each linked window against every window was
    // quadratic, on every frame a drag's landing changed): windows next to each other on
    // the line chain when a linked one is within its reach.
    let mut by_line: Vec<(usize, f64, f64)> = eligible
        .iter()
        .copied()
        .filter(|&i| !take(&members[i]).2)
        .map(|i| {
            let m = &members[i];
            let line = frame(m, m.from) - src.seconds_between(Ticks::ZERO, m.from) * m.rate;
            (i, line, slack(m))
        })
        .collect();
    by_line.sort_by(|x, y| {
        let (a, b) = (take(&members[x.0]), take(&members[y.0]));
        (a.0, a.1).cmp(&(b.0, b.1)).then(x.1.total_cmp(&y.1))
    });
    for pair in by_line.windows(2) {
        let ((i, li, si), (j, lj, sj)) = (pair[0], pair[1]);
        let linked = members[i].clip.link.is_some() || members[j].clip.link.is_some();
        if linked && take(&members[i]) == take(&members[j]) && lj - li <= 1.0 + si.max(sj) {
            join(&mut parent, i, j);
        }
    }
    let mut chains: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for &i in &eligible {
        let r = find(&mut parent, i);
        chains.entry(r).or_default().push(i);
    }
    for chain in chains.into_values().filter(|c| c.len() > 1) {
        let lo = chain
            .iter()
            .map(|&i| members[i].from - members[i].clip.offset.max(Ticks::ZERO))
            .min()
            .expect("a chain");
        let hi = chain
            .iter()
            .map(|&i| members[i].from + members[i].clip.length)
            .max()
            .expect("a chain");
        // The same tempo function over the whole chain where it lands: the ticks exactly.
        if src.span_key(lo, hi) == dst.span_key(lo + delta, hi + delta) {
            continue;
        }
        let head = chain
            .iter()
            .copied()
            .min_by_key(|&i| members[i].from)
            .expect("a chain");
        let (from_h, to_h) = (members[head].from, members[head].from + delta);
        let at = |t: Ticks| block_start(src, from_h, dst, to_h, t, false);
        let place = |i: usize| {
            let m = &members[i];
            let a = m.clip.audio.as_ref().expect("an audio window");
            let (start, end) = (at(m.from), at(m.from + m.clip.length));
            let end = end.max(start + Ticks(1));
            let fi = src.seconds_between(m.from, m.from + a.fade_in);
            let fo =
                src.seconds_between(m.from + m.clip.length - a.fade_out, m.from + m.clip.length);
            let (s, e) = (start.0 as f64, end.0 as f64);
            let round = |x: f64| Ticks(x.round() as i64).max(Ticks::ZERO);
            (
                start,
                end - start,
                round(dst.tick_after_seconds(s, fi) - s),
                round(e - dst.tick_after_seconds(e, -fo)),
            )
        };
        // The windows sharing an origin and a fine offset share them where they land.
        let mut done = vec![false; n];
        let mut by_start = chain.clone();
        by_start.sort_by_key(|&i| members[i].from);
        for &s in &by_start {
            if done[s] {
                continue;
            }
            let m = &members[s];
            let a = m.clip.audio.as_ref().expect("an audio window");
            let (start, length, fade_in, fade_out) = place(s);
            let seconds = offset_seconds(src, m.from, m.clip.offset);
            let (offset, slip_frames) = anchor(dst, start, seconds, a.slip_frames, m.rate);
            out[s] = Some(Laid {
                start,
                length,
                offset,
                fade_in,
                fade_out,
                slip_frames,
            });
            done[s] = true;
            if a.reverse {
                continue;
            }
            let key = (take(m), m.from - m.clip.offset, a.slip_frames);
            for &k in &by_start {
                let o = &members[k];
                if done[k] || (take(o), o.from - o.clip.offset, slip(o)) != key {
                    continue;
                }
                let (start_k, length, fade_in, fade_out) = place(k);
                out[k] = Some(Laid {
                    start: start_k,
                    length,
                    offset: offset + (start_k - start),
                    fade_in,
                    fade_out,
                    slip_frames,
                });
                done[k] = true;
            }
        }
    }
    out
}

/// What [`relocate`] gives a moved audio window: its offset, length, fades and fine
/// offset where it lands.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Relocated {
    pub offset: Ticks,
    pub length: Ticks,
    pub fade_in: Ticks,
    pub fade_out: Ticks,
    pub slip_frames: i32,
}

/// An unwarped audio window moved from `from` (under `src`) to `to` (under `dst`) — a
/// move, a drag-copy, a paste, a duplicate — playing the same seconds of its source: the
/// same second under its start, the same length and fades in seconds, the rounding in
/// `slip_frames`. `None` when nothing changes: the tempo over the window and its hidden
/// part before it is the same function at both places (the ticks are kept exactly), or
/// the clip follows the tempo through a warp (content ticks), or it has no audio.
pub fn relocate(
    src: &TempoMap,
    from: Ticks,
    clip: &crate::model::Clip,
    dst: &TempoMap,
    to: Ticks,
    rate: f64,
) -> Option<Relocated> {
    let a = clip.audio.as_ref()?;
    if a.warp.as_ref().is_some_and(|w| w.follow) {
        return None;
    }
    let (length, offset) = (clip.length, clip.offset.max(Ticks::ZERO));
    if src.span_key(from - offset, from + length) == dst.span_key(to - offset, to + length) {
        return None;
    }
    // A looping window repeats its period in ticks: kept as it is (review 3).
    if clip.content_length.is_some_and(|p| p < length) {
        return None;
    }
    let secs = |x: Ticks, y: Ticks| src.seconds_between(x, y);
    let start_s = offset_seconds(src, from, clip.offset);
    let len_s = secs(from, from + length);
    let fi_s = secs(from, from + a.fade_in);
    let fo_s = secs(from + length - a.fade_out, from + length);
    let t = to.0 as f64;
    let end = dst.tick_after_seconds(t, len_s);
    let round = |x: f64| Ticks(x.round() as i64);
    let (offset, slip_frames) = anchor(dst, to, start_s, a.slip_frames, rate);
    Some(Relocated {
        offset,
        length: round(end - t).max(Ticks(1)),
        fade_in: round(dst.tick_after_seconds(t, fi_s) - t).max(Ticks::ZERO),
        fade_out: round(end - dst.tick_after_seconds(end, -fo_s)).max(Ticks::ZERO),
        slip_frames,
    })
}

// --- Warped clips (round 29, warp — ADR 0061) ---------------------------------------
//
// A warped clip that follows the tempo hears, at timeline tick `T`, its content tick
// `T − (start − offset)` — wherever the tempo map puts that tick in time — and the source
// frame its warp markers pair with that content tick (`Warp::frame_at`): the markers hold
// content ticks, so the clip stays on the beat through every tempo change. A warp that does
// not follow the tempo plays at natural speed, as above. The engine renders the window
// through exactly this mapping (`cantus_engine::warp::render`), and the interface draws
// and edits through it, so what is drawn is what plays.

/// The source frame (at `rate`, the source's own) under timeline tick `tick` of an audio
/// clip that starts at `start` with `offset` and region `region` — warped or not.
pub fn clip_frame_at(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    region: &crate::audio::AudioRegion,
    rate: f64,
    tick: f64,
) -> f64 {
    frame_through(
        map,
        start,
        offset,
        region.warp.as_deref(),
        region.slip_frames as f64,
        rate,
        tick,
    )
}

/// [`clip_frame_at`] from the parts (the engine's render plan holds them, not a region):
/// the warp when it follows the tempo, else the natural speed with `slip_frames`.
pub fn frame_through(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    warp: Option<&crate::warp::Warp>,
    slip_frames: f64,
    rate: f64,
    tick: f64,
) -> f64 {
    match warp.filter(|w| w.follow) {
        Some(w) => w.frame_at(tick - (start.0 - offset.0) as f64),
        None => frame_at(map, start, offset, slip_frames, rate, tick),
    }
}

/// The timeline tick where source frame `frame` of the clip is heard (the inverse of
/// [`clip_frame_at`]).
pub fn clip_tick_at_frame(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    region: &crate::audio::AudioRegion,
    rate: f64,
    frame: f64,
) -> f64 {
    tick_through(
        map,
        start,
        offset,
        region.warp.as_deref(),
        region.slip_frames as f64,
        rate,
        frame,
    )
}

/// [`clip_tick_at_frame`] from the parts (the inverse of [`frame_through`]).
pub fn tick_through(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    warp: Option<&crate::warp::Warp>,
    slip_frames: f64,
    rate: f64,
    frame: f64,
) -> f64 {
    match warp.filter(|w| w.follow) {
        Some(w) => w.tick_at(frame) + (start.0 - offset.0) as f64,
        None => tick_at_frame(map, start, offset, slip_frames, rate, frame),
    }
}

/// The length in ticks of `frames` frames of source played by a warped clip from content
/// tick `offset` (the whole file from its warp's view): where its last frame is heard.
pub fn warped_length(warp: &crate::warp::Warp, frames: f64) -> Ticks {
    Ticks(warp.tick_at(frames).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tempo::TempoPoint;
    use crate::time::TimeSignature;

    #[test]
    fn one_tempo_is_the_old_rule_to_the_bit() {
        let map = TempoMap::constant(97.0, TimeSignature::default());
        for (start, offset, tick) in [(0, 0, 0.0), (960, 480, 1_000.5), (77_000, 12_345, 99_999.0)]
        {
            let old = (tick - (start - offset) as f64) / PPQ as f64 * 60.0 / 97.0;
            let got = source_seconds(&map, Ticks(start), Ticks(offset), tick);
            assert_eq!(got.to_bits(), old.to_bits(), "{start} {offset} {tick}");
        }
    }

    #[test]
    fn an_unwarped_clip_plays_at_natural_speed_from_its_start() {
        // 120, then a ramp from beat 8 to 140 at beat 16.
        let map = TempoMap::new(
            120.0,
            TimeSignature::default(),
            &[
                TempoPoint {
                    tick: Ticks::ZERO,
                    bpm: 120.0,
                    ramp: false,
                },
                TempoPoint {
                    tick: Ticks::beats(8),
                    bpm: 120.0,
                    ramp: true,
                },
                TempoPoint {
                    tick: Ticks::beats(16),
                    bpm: 140.0,
                    ramp: false,
                },
            ],
            &[],
        );
        // A clip at beat 4 whose source starts one beat in (0.5 s at 120).
        let (start, offset) = (Ticks::beats(4), Ticks::beats(1));
        assert_eq!(source_seconds(&map, start, offset, start.0 as f64), 0.5);
        // Across the ramp the source runs at one second per second of song.
        for beat in [5.0, 9.0, 12.5, 20.0] {
            let t = beat * PPQ as f64;
            let want = map.seconds_between_f64(start.0 as f64, t) + 0.5;
            let got = source_seconds(&map, start, offset, t);
            assert!((got - want).abs() < 1e-12, "{beat}");
            let back = tick_at_source(&map, start, offset, got);
            assert!((back - t).abs() < 1e-6, "{beat}: {back}");
        }
        // A take of 10 s at 48 kHz starting at beat 4 lasts 10 s of song.
        let len = length_ticks(&map, start, 480_000.0, 48_000.0);
        let secs = map.seconds_between(start, start + len);
        assert!((secs - 10.0).abs() < 1e-3, "{secs}");
    }
}
