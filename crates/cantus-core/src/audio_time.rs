//! Where an audio clip's source plays on the timeline (ADR 0060 §5): the one mapping from a
//! clip's ticks to its source's seconds (and frames) and back, shared by the engine
//! (`add_clips`), the snapshot (a comp seam's room), and every interface site (the
//! waveform, the pitch editor, comping, the inspector), so what is drawn is what plays.
//!
//! **An unwarped clip** plays at its natural speed, anchored at its start: the source is
//! `offset` in at the clip's start tick — `offset` (ticks, ADR 0025) read at the tempo in
//! force where the clip starts, as Ableton reads an unwarped clip's start marker — and runs
//! one second of source per second of song from there, whatever the tempo map does after
//! it. With one tempo this is exactly the old rule (the content origin `start − offset`
//! at the project's tempo). The region's fine offset (`slip`, frames at the source's rate)
//! moves the source against the timeline.
//!
//! **A warped clip** (round 29, ADR 0061) hears its source through its warp markers: see
//! [`clip_frame_at`].

use crate::tempo::TempoMap;
use crate::time::{Ticks, PPQ};

/// Seconds of source before the clip's start: its `offset` at the tempo where it starts.
#[inline]
pub fn offset_seconds(map: &TempoMap, start: Ticks, offset: Ticks) -> f64 {
    offset.to_seconds(map.bpm_at(start.0 as f64))
}

/// The source's position, in seconds, under timeline tick `tick` of an unwarped clip that
/// starts at `start` with `offset` (the fine `slip` aside). Where one tempo holds from the
/// clip's start to `tick`, `(tick − (start − offset))` at that tempo — the old rule.
pub fn source_seconds(map: &TempoMap, start: Ticks, offset: Ticks, tick: f64) -> f64 {
    let s = start.0 as f64;
    if let Some((bpm, from, to)) = map.held_at(s) {
        if tick >= from && tick <= to {
            return (tick - (s - offset.0 as f64)) / PPQ as f64 * 60.0 / bpm;
        }
    }
    map.seconds_between_f64(s, tick) + offset_seconds(map, start, offset)
}

/// The timeline tick under `seconds` into the source of an unwarped clip: the inverse of
/// [`source_seconds`].
pub fn tick_at_source(map: &TempoMap, start: Ticks, offset: Ticks, seconds: f64) -> f64 {
    let s = start.0 as f64;
    if let Some((bpm, from, to)) = map.held_at(s) {
        let t = (s - offset.0 as f64) + seconds * bpm / 60.0 * PPQ as f64;
        if t >= from && t <= to {
            return t;
        }
    }
    map.tick_after_seconds(s, seconds - offset_seconds(map, start, offset))
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

/// The `offset` (ticks) that puts `seconds` of source at the clip's start (the inverse
/// of [`offset_seconds`]): a slip, a split, a trim of the left edge.
pub fn offset_for(map: &TempoMap, start: Ticks, seconds: f64) -> Ticks {
    Ticks::from_seconds(seconds, map.bpm_at(start.0 as f64))
}

// --- Editing an unwarped window (review 2: findings 0, 3, 43) --------------------------
//
// An edit that moves a window's start — a split's right half, a left-edge trim, a move —
// keeps the source second under it: the new offset is that second in whole ticks at the
// tempo where the window now starts, and what the whole ticks cannot say goes into the
// fine offset (`slip_frames`), as a recorded take's does. Where one tempo holds over the
// whole edit this is the old tick arithmetic exactly (ADR 0025), slip untouched.

/// Whether one held tempo covers `[a, b]` (either order), both ends included: where a
/// window's tick arithmetic is the map's exactly.
pub fn held_over(map: &TempoMap, a: Ticks, b: Ticks) -> bool {
    let (lo, hi) = (a.0.min(b.0) as f64, a.0.max(b.0) as f64);
    match map.held_at(lo) {
        Some((bpm, _, to)) => hi < to || (hi == to && map.bpm_at(hi) == bpm),
        None => false,
    }
}

/// The offset and fine offset that put `seconds` of source (the fine offset `slip`
/// aside) under a window starting at `at`: whole ticks at the tempo there (never
/// negative), the rest in frames at `rate`, added to `slip`.
pub fn anchor(map: &TempoMap, at: Ticks, seconds: f64, slip: i32, rate: f64) -> (Ticks, i32) {
    let offset = offset_for(map, at, seconds).max(Ticks::ZERO);
    let rest = (seconds - offset_seconds(map, at, offset)) * rate;
    let slip = (slip as f64 + rest.round()).clamp(i32::MIN as f64, i32::MAX as f64);
    (offset, slip as i32)
}

/// A window's start moved from `start` to `to` while its source stays where it plays (a
/// split's right half, a left-edge trim): its new `(offset, slip_frames)`.
pub fn reanchor(
    map: &TempoMap,
    start: Ticks,
    offset: Ticks,
    slip: i32,
    rate: f64,
    to: Ticks,
) -> (Ticks, i32) {
    if held_over(map, start, to) {
        return (offset + (to - start), slip);
    }
    anchor(map, to, source_seconds(map, start, offset, to.0 as f64), slip, rate)
}

/// A **reversed** window (it plays its source backwards from its end) whose start moves
/// to `to` and whose end moves to `end`, playing what it played at the ticks it keeps:
/// its new `(offset, slip_frames)`. The source frame under its start is the one its last
/// sample reads, so it moves by the seconds the end moves (a split's left half, a
/// right-edge trim) and stays when only the start moves (a left-edge trim).
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
    let (lo, hi) = (start.min(to), old_end.max(end));
    if held_over(map, lo, hi) {
        return (offset + (old_end - end), slip);
    }
    let seconds =
        offset_seconds(map, start, offset) + map.seconds_between_f64(end.0 as f64, old_end.0 as f64);
    anchor(map, to, seconds, slip, rate)
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
/// move, a drag-copy, a paste — playing the same seconds of its source: the same second
/// under its start, the same length and fades in seconds, the rounding in `slip_frames`.
/// `None` when nothing changes: the tempo under the window is the same function at both
/// places (the ticks are kept exactly), or the clip follows the tempo through a warp
/// (content ticks), or it has no audio.
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
    let length = clip.length;
    if src.span_key(from, from + length) == dst.span_key(to, to + length) {
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
