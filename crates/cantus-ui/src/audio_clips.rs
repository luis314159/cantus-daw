//! Editing audio clips (ADR 0025): fades, clip gain, normalise, reverse, and the rules
//! that keep split, glue, trim and slip honest about the file underneath.
//!
//! Free functions over a [`Project`], like `clips.rs`, so every rule is tested without a
//! window. Everything that changes how a clip plays goes through **one command**,
//! `SetClipAudio`, which merges per clip: a fade or gain drag is one undo step however
//! many frames it lasted.

use crate::clips::ClipRef;
use cantus_core::{AudioRegion, AudioSource, Clip, Command, FadeCurve, Project, Ticks, TrackId};

/// Where normalise puts a clip's loudest sample (dBFS): a hair under full scale, so a
/// resampler's overshoot does not clip.
pub const NORMALIZE_DB: f32 = -0.1;
/// The clip gain's range in dB.
pub const GAIN_MIN_DB: f32 = -48.0;
pub const GAIN_MAX_DB: f32 = 24.0;

/// A clip and what makes it audio.
pub fn region(project: &Project, r: ClipRef) -> Option<(&Clip, &AudioRegion)> {
    let c = project.track(r.0)?.clip(r.1)?;
    Some((c, c.audio.as_ref()?))
}

/// The source an audio clip plays.
pub fn source_of<'p>(project: &'p Project, clip: &Clip) -> Option<&'p AudioSource> {
    project.audio_source(clip.audio.as_ref()?.source)
}

/// The whole file's length on the timeline, at the project's tempo (ADR 0025 §1: audio
/// plays at its natural speed whatever the tempo).
pub fn source_ticks(project: &Project, s: &AudioSource) -> Ticks {
    let seconds = s.frames as f64 / s.sample_rate.max(1) as f64;
    Ticks::from_seconds(seconds, project.tempo_bpm)
}

/// The frame of the source at content tick `c` (fractional).
pub fn frame_at(project: &Project, rate: u32, c: Ticks) -> f64 {
    c.to_seconds(project.tempo_bpm) * rate as f64
}

/// The command that gives a clip `audio`.
pub fn set(track: TrackId, clip: &Clip, audio: AudioRegion) -> Command {
    Command::SetClipAudio {
        track,
        clip: clip.id,
        audio: Some(audio),
    }
}

/// `a` with its fade in set to `len`, kept inside the clip and clear of the fade out.
pub fn with_fade_in(clip: &Clip, a: &AudioRegion, len: Ticks) -> AudioRegion {
    let room = (clip.length - a.fade_out).max(Ticks::ZERO);
    AudioRegion {
        fade_in: len.clamp(Ticks::ZERO, room),
        ..a.clone()
    }
}

/// `a` with its fade out set to `len`, kept inside the clip and clear of the fade in.
pub fn with_fade_out(clip: &Clip, a: &AudioRegion, len: Ticks) -> AudioRegion {
    let room = (clip.length - a.fade_in).max(Ticks::ZERO);
    AudioRegion {
        fade_out: len.clamp(Ticks::ZERO, room),
        ..a.clone()
    }
}

/// `a` with its gain set, within the range.
pub fn with_gain(a: &AudioRegion, db: f32) -> AudioRegion {
    AudioRegion {
        gain_db: if db.is_finite() {
            db.clamp(GAIN_MIN_DB, GAIN_MAX_DB)
        } else {
            0.0
        },
        ..a.clone()
    }
}

/// Which fade.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Fade {
    In,
    Out,
}

/// `a` with one fade's curve set.
pub fn with_curve(a: &AudioRegion, which: Fade, curve: FadeCurve) -> AudioRegion {
    let mut out = a.clone();
    match which {
        Fade::In => out.fade_in_curve = curve,
        Fade::Out => out.fade_out_curve = curve,
    }
    out
}

/// The clip gain that brings `peak` (linear) to [`NORMALIZE_DB`]; `None` for silence.
pub fn normalize_gain(peak: f32) -> Option<f32> {
    if peak.is_nan() || peak <= 1e-6 {
        return None;
    }
    let db = NORMALIZE_DB - 20.0 * peak.log10();
    Some(db.clamp(GAIN_MIN_DB, GAIN_MAX_DB))
}

/// The frames a clip's window shows: from where to where in the file (the region's fine
/// offset included, scaled from the source's rate to `rate`).
pub fn window_frames(project: &Project, clip: &Clip, rate: u32) -> (f64, f64) {
    let slip = slip_at(project, clip, rate);
    let a = frame_at(project, rate, clip.offset) + slip;
    let b = frame_at(project, rate, clip.offset + clip.length) + slip;
    (a, b)
}

/// An audio clip's fine offset (`AudioRegion::slip_frames`, in its source's frames) in
/// frames at `rate`; 0 for anything else.
pub fn slip_at(project: &Project, clip: &Clip, rate: u32) -> f64 {
    let Some(a) = clip.audio.as_ref().filter(|a| a.slip_frames != 0) else {
        return 0.0;
    };
    let src_rate = source_of(project, clip)
        .map_or(rate, |s| s.sample_rate)
        .max(1);
    a.slip_frames as f64 * rate as f64 / src_rate as f64
}

/// The fade gain at timeline tick `t` inside `clip` (1 outside both fades), shortening
/// both in proportion when they would overlap — as the engine does.
pub fn fade_gain(clip: &Clip, a: &AudioRegion, t: Ticks) -> f32 {
    fade_gain_at(a, clip.length, t - clip.start)
}

/// [`fade_gain`] for a window `length` long, `d` ticks after its start.
pub fn fade_gain_at(a: &AudioRegion, length: Ticks, d: Ticks) -> f32 {
    let len = length.0.max(1) as f64;
    let (mut fi, mut fo) = (a.fade_in.0.max(0) as f64, a.fade_out.0.max(0) as f64);
    if fi + fo > len {
        let k = len / (fi + fo);
        fi *= k;
        fo *= k;
    }
    let d = d.0 as f64;
    let mut g = 1.0f32;
    if fi > 0.0 && d < fi {
        g *= a.fade_in_curve.gain((d / fi) as f32);
    }
    let left = len - d;
    if fo > 0.0 && left < fo {
        g *= a.fade_out_curve.gain((left / fo) as f32);
    }
    g
}

// ---- keeping the window on the file ------------------------------------------------

/// An edit of a clip's window, as a drag makes it (the drag's `delta`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowEdit {
    TrimStart,
    TrimEnd,
    /// The content dragged by `delta` under a window that stays.
    Slip,
}

/// Limit a window edit's `delta` so an audio clip's window stays on its file: never
/// before the first frame, never past the last. A note clip (or a clip whose source is
/// gone) is not limited.
///
/// A reversed clip plays its window backwards, so its left edge is the window's *end*:
/// trimming it from the left shortens the window from the end, and the limits swap.
pub fn clamp_edit(project: &Project, clip: &Clip, edit: WindowEdit, delta: Ticks) -> Ticks {
    let (Some(a), Some(s)) = (clip.audio.as_ref(), source_of(project, clip)) else {
        return delta;
    };
    let len = source_ticks(project, s);
    let (o, l) = (clip.offset, clip.length);
    let room_after = (len - o - l).max(Ticks::ZERO); // file left after the window
    let (lo, hi) = match (edit, a.reverse) {
        // Forward: offset moves with the left edge; the right edge reveals what follows.
        (WindowEdit::TrimStart, false) => (-o, Ticks(i64::MAX)),
        (WindowEdit::TrimEnd, false) => (Ticks(i64::MIN), room_after),
        (WindowEdit::Slip, false) => (o + l - len, o),
        // Reversed: the left edge reveals what follows the window, the right edge what
        // precedes it.
        (WindowEdit::TrimStart, true) => (-room_after, Ticks(i64::MAX)),
        (WindowEdit::TrimEnd, true) => (Ticks(i64::MIN), o),
        (WindowEdit::Slip, true) => (-o, len - l - o),
    };
    if lo > hi {
        return Ticks::ZERO;
    }
    delta.clamp(lo, hi)
}

/// The commands of a window edit on a **reversed** audio clip, where the window maps
/// backwards (`clips.rs` handles every other clip). `delta` is already clamped.
pub fn reversed_edit(track: TrackId, clip: &Clip, edit: WindowEdit, delta: Ticks) -> Vec<Command> {
    let mut out = Vec::with_capacity(2);
    let (start, length, offset) = match edit {
        // The left edge cuts or reveals the window's end: the offset stays.
        WindowEdit::TrimStart => (clip.start + delta, clip.length - delta, clip.offset),
        // The right edge cuts or reveals the window's beginning.
        WindowEdit::TrimEnd => (clip.start, clip.length + delta, clip.offset - delta),
        // Dragging the music later shows content from later in the file at each point
        // (it is played backwards).
        WindowEdit::Slip => (clip.start, clip.length, clip.offset + delta),
    };
    let start = start.max(Ticks::ZERO);
    let length = length.max(Ticks::SIXTEENTH);
    if start != clip.start || length != clip.length {
        out.push(Command::SetClipBounds {
            track,
            clip: clip.id,
            start,
            length,
        });
    }
    let offset = offset.max(Ticks::ZERO);
    if offset != clip.offset {
        out.push(Command::SetClipOffset {
            track,
            clip: clip.id,
            offset,
        });
    }
    out
}

/// Whether a clip plays backwards.
pub fn is_reversed(clip: &Clip) -> bool {
    clip.audio.as_ref().is_some_and(|a| a.reverse)
}

// ---- split and glue ------------------------------------------------------------------

// `split_audio` (the fades and a reversed clip's windows at a split) is
// `cantus_core::clipboard::split_audio`, beside the split it serves.

/// Glue audio clips: only clips that are **contiguous windows on one source** — each
/// starts where the last ended, on the timeline and in the file — with the same gain and
/// direction. The first keeps its fade in and takes the last one's fade out. `None` when
/// they cannot be glued (a clip of notes among them, two files, a gap).
pub fn glue(track: TrackId, clips: &[&Clip]) -> Option<Vec<Command>> {
    let mut ordered: Vec<&Clip> = clips.to_vec();
    ordered.sort_by_key(|c| (c.start, c.id.0));
    let first = *ordered.first()?;
    let last = *ordered.last()?;
    let a = first.audio.as_ref()?;
    for w in ordered.windows(2) {
        let (p, n) = (w[0], w[1]);
        let na = n.audio.as_ref()?;
        let same = na.source == a.source
            && na.gain_db == a.gain_db
            && na.reverse == a.reverse
            && na.slip_frames == a.slip_frames
            // Round 28 (ADR 0054): one clip, one pitch edit; halves edited apart are not
            // glued (one half's corrections would be lost).
            && na.pitch == a.pitch;
        if !same || n.start != p.end() || n.lane != p.lane {
            return None;
        }
        // Forward, the next window continues the file; reversed, it precedes it.
        let continues = if a.reverse {
            n.offset + n.length == p.offset
        } else {
            n.offset == p.offset + p.length
        };
        if !continues {
            return None;
        }
    }
    let la = last.audio.as_ref()?;
    let mut commands = Vec::with_capacity(ordered.len() + 2);
    for c in &ordered[1..] {
        commands.push(Command::RemoveClip { track, clip: c.id });
    }
    let length = last.end() - first.start;
    commands.push(Command::SetClipBounds {
        track,
        clip: first.id,
        start: first.start,
        length,
    });
    if a.reverse {
        // Reversed, the glued window starts at the last clip's offset.
        commands.push(Command::SetClipOffset {
            track,
            clip: first.id,
            offset: last.offset,
        });
    }
    let glued = AudioRegion {
        fade_out: la.fade_out,
        fade_out_curve: la.fade_out_curve,
        ..a.clone()
    };
    if glued != *a {
        commands.push(Command::SetClipAudio {
            track,
            clip: first.id,
            audio: Some(glued),
        });
    }
    Some(commands)
}

/// A finished drag on a reversed audio clip, as a window edit; `None` for anything else
/// (a move, or a clip that plays forwards), which `clips.rs` handles.
pub fn reversed_drag(
    clip: &Clip,
    result: crate::views::arrangement::ClipDragResult,
) -> Option<(WindowEdit, Ticks)> {
    use crate::views::arrangement::ClipDragResult as R;
    if !is_reversed(clip) {
        return None;
    }
    match result {
        R::TrimStart { delta } => Some((WindowEdit::TrimStart, delta)),
        R::TrimEnd { delta } => Some((WindowEdit::TrimEnd, delta)),
        R::Slip { delta } => Some((WindowEdit::Slip, delta)),
        R::Move { .. } => None,
    }
}
