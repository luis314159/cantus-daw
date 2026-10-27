//! Crossfades between overlapping audio clips (schema 44, ADR 0070).
//!
//! Two audio clips on a track's playing lane **overlap** when the later one starts before
//! the earlier one ends. With the later clip's [`AudioRegion::cross`] set, the engine
//! crossfades them over the whole overlap: the earlier fades out over exactly the samples
//! the later fades in on, on the chosen [`XfadeCurve`] — rendered by the very path that
//! renders a comp seam (ADR 0058: the snapshot's `xin`/`xout` stretches), not a second one.
//! Without it (every clip before schema 44, and an overlap whose crossfade was removed) the
//! two play summed over the overlap, as they always did, to the bit.
//!
//! The crossfade's length is the overlap: there is nothing in the model to keep in step
//! with the clips' edges. Its length is edited by moving those edges (the overlap's
//! handles, the crossfade editor's length — [`resize`]).
//!
//! [`AudioRegion::cross`]: crate::audio::AudioRegion::cross

use crate::command::Command;
use crate::model::{Clip, ClipId, Project, Track, TrackId};
use crate::time::Ticks;
use cantus_dsp::math;
use serde::{Deserialize, Serialize};

/// The curve a crossfade between two clips follows. The first is the default: it keeps
/// the loudness of two different recordings (uncorrelated) constant across the overlap;
/// *Linear* and *S* keep their sum at unity, for two takes of the same thing (correlated).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum XfadeCurve {
    #[default]
    EqualPower,
    Linear,
    SCurve,
}

impl XfadeCurve {
    pub const ALL: [XfadeCurve; 3] = [
        XfadeCurve::EqualPower,
        XfadeCurve::Linear,
        XfadeCurve::SCurve,
    ];

    /// The gains `(in, out)` at `u` (0…1) of the way across: the later clip's and the
    /// earlier one's ([`XfadeCurve::gain_in`], [`XfadeCurve::gain_out`]).
    #[inline]
    pub fn gains(self, u: f32) -> (f32, f32) {
        (self.gain_in(u), self.gain_out(u))
    }

    /// What the later clip is multiplied by at `u` of the way across. Equal power is the
    /// comp seam's law (ADR 0058) to the bit — `sin(π/2·u)` — so a seam renders exactly as
    /// it did; linear and S sum to one with [`XfadeCurve::gain_out`].
    #[inline]
    pub fn gain_in(self, u: f32) -> f32 {
        match self {
            XfadeCurve::EqualPower => math::sin(std::f32::consts::FRAC_PI_2 * u),
            XfadeCurve::Linear => u.clamp(0.0, 1.0),
            XfadeCurve::SCurve => smooth(u),
        }
    }

    /// What the earlier clip is multiplied by at `u` of the way across (the seam's
    /// `cos(π/2·u)` for equal power).
    #[inline]
    pub fn gain_out(self, u: f32) -> f32 {
        match self {
            XfadeCurve::EqualPower => math::cos(std::f32::consts::FRAC_PI_2 * u.min(1.0)),
            XfadeCurve::Linear => 1.0 - u.clamp(0.0, 1.0),
            XfadeCurve::SCurve => 1.0 - smooth(u),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            XfadeCurve::EqualPower => "equal_power",
            XfadeCurve::Linear => "linear",
            XfadeCurve::SCurve => "s_curve",
        }
    }

    /// The i18n key of its name (area `clipfx`).
    pub fn label_key(self) -> &'static str {
        match self {
            XfadeCurve::EqualPower => "clipfx.curve.equal_power",
            XfadeCurve::Linear => "clipfx.curve.linear",
            XfadeCurve::SCurve => "clipfx.curve.s_curve",
        }
    }
}

/// The S: slow out of each end, `3u² − 2u³`.
#[inline]
fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// Whether a clip takes part in overlap crossfades: an audio clip on the playing lane,
/// not muted, with a length. Reversed, looping and warped clips cross like any other —
/// the crossfade is laid on the timeline, whatever the window reads.
pub fn window(c: &Clip) -> bool {
    c.lane == 0 && !c.muted && c.audio.is_some() && c.length > Ticks::ZERO
}

/// Whether `r` goes on from `l` as one stretch of one file cut in two — a split's halves,
/// which play as the window they were: the same source and direction, `r` starting where
/// `l` ends with the content going on, neither looping (review 1 of ADR 0070: a split
/// inside a crossed overlap must not change what plays).
pub fn continues(l: &Clip, r: &Clip) -> bool {
    let (Some(a), Some(b)) = (l.audio.as_ref(), r.audio.as_ref()) else {
        return false;
    };
    let looped = |c: &Clip| c.period().is_some_and(|p| p < c.length);
    a.source == b.source
        && a.reverse == b.reverse
        && l.end() == r.start
        && !looped(l)
        && !looped(r)
        && if a.reverse {
            r.offset + r.length == l.offset
        } else {
            r.offset == l.offset + l.length
        }
}

/// One overlap of a track's playing lane (ADR 0070 §1): the later window starts after the
/// earlier one starts and before it ends, and ends at or after it ends. A *window* is a
/// clip, or the pieces of one split (they [`continues`] one into the next) taken as the
/// clip they were.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlap {
    /// The earlier window's pieces, in order.
    pub left: Vec<ClipId>,
    /// The later window's pieces, in order.
    pub right: Vec<ClipId>,
    /// The piece whose end ends the overlap (the earlier window's last).
    pub left_last: ClipId,
    /// The piece whose start starts it, and carries the curve (the later window's first).
    pub right_first: ClipId,
    /// The overlap: from the later window's start to the earlier one's end.
    pub from: Ticks,
    pub to: Ticks,
    /// `right_first`'s crossfade (`None`: summed).
    pub curve: Option<XfadeCurve>,
}

impl Overlap {
    /// Whether `id` is one of the earlier window's pieces.
    pub fn in_left(&self, id: ClipId) -> bool {
        self.left.contains(&id)
    }

    /// Whether `id` is one of the later window's pieces.
    pub fn in_right(&self, id: ClipId) -> bool {
        self.right.contains(&id)
    }
}

/// A window: a clip, or a split's pieces.
struct Chain<'a> {
    pieces: Vec<&'a Clip>,
}

impl Chain<'_> {
    fn start(&self) -> Ticks {
        self.pieces[0].start
    }
    fn end(&self) -> Ticks {
        self.pieces[self.pieces.len() - 1].end()
    }
    fn key(&self) -> (Ticks, u64) {
        (self.start(), self.pieces[0].id.0)
    }
}

/// Every overlap of a track's playing lane — the windows by start in one sweep (the
/// windows still reaching past each one's start kept aside), the latest starting earlier
/// window crossing each later one, so a track of thousands of clips costs what sorting
/// them does. A window lying wholly inside another is layered over it, not crossed; two
/// that only meet are a butt join.
pub fn overlaps_of_track(track: &Track) -> Vec<Overlap> {
    let mut w: Vec<&Clip> = track.clips.iter().filter(|c| window(c)).collect();
    if w.len() < 2 {
        return Vec::new();
    }
    w.sort_by_key(|c| (c.start, c.id.0));
    // The pieces of a split, joined: a chain's last piece by where it ends and its file.
    let mut chains: Vec<Chain<'_>> = Vec::with_capacity(w.len());
    let mut open: std::collections::HashMap<(Ticks, u64), usize> = std::collections::HashMap::new();
    for c in w {
        let src = c.audio.as_ref().map_or(0, |a| a.source.0);
        let joined = open
            .get(&(c.start, src))
            .copied()
            .filter(|&k| continues(chains[k].pieces[chains[k].pieces.len() - 1], c));
        match joined {
            Some(k) => {
                open.remove(&(c.start, src));
                chains[k].pieces.push(c);
                open.insert((c.end(), src), k);
            }
            None => {
                open.insert((c.end(), src), chains.len());
                chains.push(Chain { pieces: vec![c] });
            }
        }
    }
    chains.sort_by_key(|c| c.key());
    let mut active: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for (i, r) in chains.iter().enumerate() {
        active.retain(|&k| chains[k].end() > r.start());
        if let Some(&k) = active
            .iter()
            .filter(|&&k| chains[k].start() < r.start() && chains[k].end() <= r.end())
            .max_by_key(|&&k| chains[k].key())
        {
            let l = &chains[k];
            let first = r.pieces[0];
            out.push(Overlap {
                left: l.pieces.iter().map(|c| c.id).collect(),
                right: r.pieces.iter().map(|c| c.id).collect(),
                left_last: l.pieces[l.pieces.len() - 1].id,
                right_first: first.id,
                from: r.start(),
                to: l.end(),
                curve: first.audio.as_ref().and_then(|a| a.cross),
            });
        }
        active.push(i);
    }
    out
}

/// The overlap a piece of the later window crosses into, if any (`r` any of its pieces).
pub fn overlap_into(track: &Track, r: ClipId) -> Option<Overlap> {
    overlaps_of_track(track).into_iter().find(|o| o.in_right(r))
}

/// Every overlap of the project, by its track and its two windows' first pieces.
pub fn pairs(project: &Project) -> Vec<(TrackId, ClipId, ClipId)> {
    let mut out = Vec::new();
    for t in &project.tracks {
        // A track with fewer than two audio clips has nothing to cross (the common case:
        // MIDI tracks), and is passed over without a look at each clip's neighbours.
        if t.clips.iter().filter(|c| window(c)).take(2).count() < 2 {
            continue;
        }
        out.extend(
            overlaps_of_track(t)
                .into_iter()
                .map(|o| (t.id, o.left[0], o.right_first)),
        );
    }
    out
}

/// Whether a command can make two clips overlap, or take one out of its overlap — what
/// the bus's crossfade fix looks after ([`crate::bus::CommandBus::set_auto_crossfades`]);
/// everything else is let by without a look at the project.
pub fn may_overlap(cmd: &Command) -> bool {
    use Command as C;
    match cmd {
        C::InsertClip { .. }
        | C::RemoveClip { .. }
        | C::SetClipBounds { .. }
        | C::SetClipMute { .. }
        | C::MoveClipToTrack { .. }
        | C::SetClipLane { .. }
        | C::SetClipLoop { .. } => true,
        C::Batch { commands, .. } => commands.iter().any(may_overlap),
        _ => false,
    }
}

/// What an edit's overlaps ask (ADR 0070 §2): with automatic crossfades on (`curve`),
/// every overlap there now and not before (`before`, [`pairs`] of the project ahead of the
/// edit) whose later window has no crossfade gets `curve` — an overlap that was already
/// there keeps what it has, so a crossfade removed stays removed while the clips stay put.
/// Whatever the setting, a clip that crossed into an overlap before and now starts no
/// overlap loses its stale curve (review 1: it would cross whatever it is next laid over),
/// unless it is a linked copy (its siblings share it). `SetClipAudio`s, one per clip.
pub fn fresh(
    before: &[(TrackId, ClipId, ClipId)],
    project: &Project,
    curve: Option<XfadeCurve>,
) -> Vec<Command> {
    let now = pairs(project);
    let mut out = Vec::new();
    let mut set = |track: TrackId, clip: ClipId, cross: Option<XfadeCurve>| {
        let Some(c) = project.track(track).and_then(|t| t.clip(clip)) else {
            return;
        };
        let Some(a) = c.audio.as_ref().filter(|a| a.cross != cross) else {
            return;
        };
        let mut next = a.clone();
        next.cross = cross;
        out.push(Command::SetClipAudio {
            track,
            clip,
            audio: Some(next),
        });
    };
    if let Some(curve) = curve {
        for &(track, left, right) in &now {
            let had = project
                .track(track)
                .and_then(|t| t.clip(right))
                .and_then(|c| c.audio.as_ref())
                .is_some_and(|a| a.cross.is_some());
            if !before.contains(&(track, left, right)) && !had {
                set(track, right, Some(curve));
            }
        }
    }
    for &(track, _, right) in before {
        if now.iter().any(|&(t, _, r)| t == track && r == right) {
            continue;
        }
        let linked = project
            .track(track)
            .and_then(|t| t.clip(right))
            .is_none_or(|c| c.link.is_some());
        if !linked {
            set(track, right, None);
        }
    }
    out
}

/// The overlap of `left` and `right` (an [`Overlap`]'s `left_last` and `right_first`):
/// from the right's start to the left's end.
pub fn overlap(left: &Clip, right: &Clip) -> (Ticks, Ticks) {
    (right.start, left.end())
}

/// The commands that give the crossfade between `left` and `right` (on `track`) the
/// length `len`, centred where it is: the right clip's start moves (its content stays
/// put) and the left clip's end, each only as far as its own audio and its other edge
/// allow (`room`: how far each can grow — the right to the left, the left to the right —
/// from the file's edges, `crate::comp`'s measure). `None` when nothing changes.
pub fn resize(
    track: TrackId,
    left: &Clip,
    right: &Clip,
    len: Ticks,
    room: (Ticks, Ticks),
) -> Option<Vec<Command>> {
    let (from, to) = overlap(left, right);
    if to <= from {
        return None;
    }
    let len = len.max(Ticks(1));
    let centre = from.0 + (to.0 - from.0) / 2;
    // The halves wanted, then each limited: the right's start no earlier than its audio
    // and the left's start (a tick after it), no later than the right's end less a tick;
    // the left's end likewise.
    let half = len.0 / 2;
    let mut start = Ticks(centre - half);
    let mut end = Ticks(centre - half + len.0);
    let earliest = (right.start - room.0).max(left.start + Ticks(1));
    let latest_end = (left.end() + room.1).min(right.end());
    start = start.max(earliest).min(right.end() - Ticks(1));
    end = end.min(latest_end).max(start + Ticks(1));
    if start == right.start && end == left.end() {
        return None;
    }
    let mut out = Vec::new();
    if end != left.end() {
        out.push(Command::SetClipBounds {
            track,
            clip: left.id,
            start: left.start,
            length: end - left.start,
        });
        // A reversed window reads back from its end: what lies there stays by its offset
        // moving the other way.
        if left.audio.as_ref().is_some_and(|a| a.reverse) {
            out.push(Command::SetClipOffset {
                track,
                clip: left.id,
                offset: left.offset - (end - left.end()),
            });
        }
    }
    if start != right.start {
        let d = start - right.start;
        out.push(Command::SetClipBounds {
            track,
            clip: right.id,
            start,
            length: right.length - d,
        });
        out.push(Command::SetClipOffset {
            track,
            clip: right.id,
            offset: content_offset(right, d),
        });
    }
    Some(out)
}

/// How far the crossfade between `left` and `right` can grow on each side, from the
/// audio their files hold past the overlap's edges: `(right's start leftwards, left's end
/// rightwards)`. A reversed window grows into the other end of its file.
pub fn room(project: &Project, left: &Clip, right: &Clip) -> (Ticks, Ticks) {
    let rev = |c: &Clip| c.audio.as_ref().is_some_and(|a| a.reverse);
    let (rb, ra) = crate::snapshot::audio_room(project, right);
    let (lb, la) = crate::snapshot::audio_room(project, left);
    (
        if rev(right) { ra } else { rb },
        if rev(left) { lb } else { la },
    )
}

/// A window's offset after its start moved by `d` with its content kept in place: a
/// forward window's content moves with the start; a reversed one reads back from its end,
/// which stays.
fn content_offset(c: &Clip, d: Ticks) -> Ticks {
    if c.audio.as_ref().is_some_and(|a| a.reverse) {
        c.offset
    } else {
        c.offset + d
    }
}

#[cfg(test)]
#[path = "crossfade_tests.rs"]
mod tests;
