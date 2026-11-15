//! r40:ripple — time itself edited (round 40, ADR 0116): **insert time** and **delete time**
//! over a range of the whole song, and the **section drag** that moves a section's bars on
//! every track ([`crate::ripple_bars`]).
//!
//! Every leader has the two time edits (FL's *Insert / Delete space*, Live's *Insert Silence*
//! and *Delete Time*, Logic's *Insert Silence / Snip*, Cubase's *Insert Silence / Delete
//! Time*, Pro Tools' *Insert / Cut Time*, REAPER's *Insert empty space / Remove contents …
//! moving later items*, Studio One's *Insert Silence / Delete Time*), and they agree on the
//! essentials, which this module follows:
//!
//! - the edit is of **time**, so it is the whole song's: every track, every lane (takes
//!   too, so a comp stays over its takes), the alternate comps' windows, the tempo track and
//!   the metre changes, the markers, the sections, the loop range and the punch range;
//! - a clip across the edit point is **split** there (as the arrangement's *Split*: both
//!   halves keep the content, no note moves) and what follows moves by the length;
//! - delete time removes what lies inside the range (clips cut at its edges) and closes
//!   the gap.
//!
//! **Where we choose differently.** Cubase applies the range to the selected tracks only and
//! leaves the tempo track behind unless it is selected, the most reported trap of its range
//! editing; here the time edits are always the song's (a single track's ripple is the ripple
//! mode's, [`crate::ripple_mode`]). And delete time **heals** a cut it closes: two pieces that
//! meet where the gap was and continue each other (one clip split there, its content going
//! on) are glued back, so inserting time and deleting the same range gives the song back to
//! the tick — no invisible cut left behind.
//!
//! **Each edit is one command.** The builders here return a `Command::Batch` of the model's
//! own commands (splits, bounds, the tempo and metre lists, markers, sections, the loop and
//! punch ranges), worked out on a scratch copy of the project so each step sees the one
//! before; its inverse is the batch's, exact. The bus adds what any edit of the tempo map
//! adds (`tempo::keep`) and the crossfades of new overlaps (ADR 0070).

use crate::clipboard::{self, Ids};
use crate::command::Command;
use crate::model::{Clip, ClipId, Project, TrackId};
use crate::tempo::TempoMap;
use crate::time::Ticks;

pub use crate::ripple_tempo::{delete_metres, delete_tempo, insert_metres, insert_tempo};

/// The undo labels of the time edits.
pub const INSERT_LABEL: &str = "cmd.ripple.insert_time";
pub const DELETE_LABEL: &str = "cmd.ripple.delete_time";

/// How a time edit moves a position of the song.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Shift {
    /// `len` of empty time at `at`: everything at or after `at` moves `len` later.
    Insert { at: Ticks, len: Ticks },
    /// `[from, to)` taken out: everything at or after `to` moves `to − from` earlier; what
    /// was inside collapses onto `from`.
    Delete { from: Ticks, to: Ticks },
}

impl Shift {
    /// Where the start of something (a clip, a marker, a section) lands.
    pub fn point(self, p: Ticks) -> Ticks {
        match self {
            Shift::Insert { at, len } => {
                if p >= at {
                    p + len
                } else {
                    p
                }
            }
            Shift::Delete { from, to } => {
                if p < from {
                    p
                } else if p >= to {
                    p - (to - from)
                } else {
                    from
                }
            }
        }
    }

    /// Where the end of a span lands: an end at the insert point stays (the span ends
    /// before the new time); one past it grows with it.
    pub fn end(self, e: Ticks) -> Ticks {
        match self {
            Shift::Insert { at, len } => {
                if e > at {
                    e + len
                } else {
                    e
                }
            }
            Shift::Delete { .. } => self.point(e),
        }
    }

    /// The point where the song is cut: the insert point, or the start of the deleted range.
    pub fn seam(self) -> Ticks {
        match self {
            Shift::Insert { at, .. } => at,
            Shift::Delete { from, .. } => from,
        }
    }

    /// How far what follows moves (positive later).
    pub fn delta(self) -> Ticks {
        match self {
            Shift::Insert { len, .. } => len,
            Shift::Delete { from, to } => from - to,
        }
    }
}

/// A batch worked out on a scratch copy: each command is applied there before it is kept,
/// so the next one is built against the song as the edit has left it so far.
pub(crate) struct Edit {
    pub scratch: Project,
    pub out: Vec<Command>,
}

impl Edit {
    pub fn new(project: &Project) -> Self {
        Edit {
            scratch: project.clone(),
            out: Vec::new(),
        }
    }

    /// Apply `c` to the scratch and keep it; `None` (nothing kept) when the model refuses it.
    pub fn run(&mut self, c: Command) -> Option<()> {
        c.clone().apply(&mut self.scratch).ok()?;
        self.out.push(c);
        Some(())
    }

    pub fn run_all(&mut self, cs: Vec<Command>) -> Option<()> {
        cs.into_iter().try_for_each(|c| self.run(c))
    }

    /// The batch, or `None` when the edit changed nothing.
    pub fn finish(self, label: &str) -> Option<Command> {
        (!self.out.is_empty()).then(|| Command::Batch {
            label_key: label.to_string(),
            commands: self.out,
        })
    }
}

/// Insert `len` of empty time at `at` over the whole song: one command, `None` when there is
/// nothing to insert (`len` not positive, `at` before the song) or the model refuses a step
/// (a frozen track's clips are the bus's to refuse, ADR 0064).
pub fn insert_time(project: &Project, at: Ticks, len: Ticks, ids: Ids) -> Option<Command> {
    if len <= Ticks::ZERO || at < Ticks::ZERO {
        return None;
    }
    let mut e = Edit::new(project);
    apply_shift(&mut e, Shift::Insert { at, len }, ids)?;
    // What followed plays under its scene (R2-B2): the cues moved with it already do.
    let after = crate::ripple_cues::Piece {
        new: at + len,
        old: at,
    };
    crate::ripple_cues::keep_scenes(&mut e, project, &[after], ids)?;
    e.finish(INSERT_LABEL)
}

/// Delete the time `[from, to)` over the whole song: what lies inside goes (clips cut at the
/// edges), what follows moves `to − from` earlier, and a cut the gap closes is healed. One
/// command; `None` for an empty range or a refused step.
pub fn delete_time(project: &Project, from: Ticks, to: Ticks, ids: Ids) -> Option<Command> {
    if to <= from || from < Ticks::ZERO {
        return None;
    }
    let mut e = Edit::new(project);
    apply_shift(&mut e, Shift::Delete { from, to }, ids)?;
    // What followed the range plays under the scene it had there (R2-B2): a cue inside the
    // range that was still in effect at its end is kept, as cues of its own at the seam.
    let after = crate::ripple_cues::Piece { new: from, old: to };
    crate::ripple_cues::keep_scenes(&mut e, project, &[after], ids)?;
    e.finish(DELETE_LABEL)
}

/// What a shift reaches: a time edit's is the whole song, cutting what runs across
/// ([`Scope::SONG`]); the ripple mode's is a track's clips, or every track's with the song's
/// marks and clock, moving only what starts after the edit ([`crate::ripple_mode`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    /// The tracks whose clips move (`None`: every track).
    pub tracks: Option<Vec<TrackId>>,
    /// Whether clips across the edit are cut (a time edit) or left where they are.
    pub cut: bool,
    /// Whether the markers, sections, loop, punch, tempo and metre move too.
    pub marks: bool,
    /// Clips the gesture itself moves: left to it.
    pub exclude: Vec<ClipId>,
    /// Cues inside deleted time that stay where they are: another track still plays there
    /// (the ripple mode's every-track follow, R2-B2).
    pub keep_cues: Vec<crate::mix_snap::MixCueId>,
}

impl Scope {
    /// A time edit: the whole song, clips cut at its edges.
    pub fn song() -> Self {
        Scope {
            tracks: None,
            cut: true,
            marks: true,
            exclude: Vec::new(),
            keep_cues: Vec::new(),
        }
    }
}

/// Every part of a time edit, in order, onto `e`: the clips of every track (and the
/// alternate comps), then the markers, the sections, the loop and punch ranges, and last the
/// tempo and metre lists (the clips are moved against the map as it was and as it will be).
pub(crate) fn apply_shift(e: &mut Edit, shift: Shift, ids: Ids) -> Option<()> {
    apply_shift_in(e, shift, ids, &Scope::song())
}

/// [`apply_shift`] over `scope`.
pub(crate) fn apply_shift_in(e: &mut Edit, shift: Shift, ids: Ids, scope: &Scope) -> Option<()> {
    let old_map = e.scratch.tempo_map();
    let (bpm, points) = match (scope.marks, shift) {
        (false, _) => (e.scratch.tempo_bpm, e.scratch.tempo_points.clone()),
        (true, Shift::Insert { at, len }) => insert_tempo(&e.scratch, at, len),
        (true, Shift::Delete { from, to }) => delete_tempo(&e.scratch, from, to),
    };
    let (sig, metres) = match (scope.marks, shift) {
        (false, _) => (e.scratch.time_signature, e.scratch.metre_changes.clone()),
        (true, Shift::Insert { at, len }) => insert_metres(&e.scratch, at, len),
        (true, Shift::Delete { from, to }) => delete_metres(&e.scratch, from, to),
    };
    let new_map = TempoMap::new(bpm, sig, &points, &metres);
    let maps = Maps {
        old: &old_map,
        new: &new_map,
        tempo_track: !e.scratch.tempo_points.is_empty() || !points.is_empty(),
    };
    let tracks: Vec<TrackId> = match &scope.tracks {
        Some(t) => t.clone(),
        None => e.scratch.tracks.iter().map(|t| t.id).collect(),
    };
    for t in tracks {
        clips_of_track(e, t, shift, &maps, ids, scope)?;
        with_comps(e, t, &mut |sub| {
            clips_of_track(sub, t, shift, &maps, ids, scope)
        })?;
    }
    if !scope.marks {
        return Some(());
    }
    crate::ripple_marks::shift_marks(e, shift, &scope.keep_cues)?;
    if bpm != e.scratch.tempo_bpm || points != e.scratch.tempo_points {
        e.run(Command::SetTempoTrack { bpm, points })?;
    }
    if sig != e.scratch.time_signature || metres != e.scratch.metre_changes {
        e.run(Command::SetMetreTrack {
            signature: sig,
            metres,
        })?;
    }
    Some(())
}

/// The tempo maps before and after a time edit, for the audio windows it moves.
pub(crate) struct Maps<'a> {
    pub old: &'a TempoMap,
    pub new: &'a TempoMap,
    /// Whether either has a tempo track: without one, a move keeps every tick as it is.
    pub tempo_track: bool,
}

/// The clips of track `t` (every lane): split at the seam (and, deleting, at the range's
/// end), the inside removed, the rest moved, a closed cut healed. Without `scope.cut`, only
/// what starts at or after the edit moves (after the range, deleting).
fn clips_of_track(
    e: &mut Edit,
    t: TrackId,
    shift: Shift,
    maps: &Maps,
    ids: Ids,
    scope: &Scope,
) -> Option<()> {
    if scope.cut {
        match shift {
            Shift::Insert { at, .. } => cut_at(e, t, at, ids)?,
            Shift::Delete { from, to } => clear(e, t, from, to, ids)?,
        }
    }
    let first = match (scope.cut, shift) {
        (false, Shift::Delete { to, .. }) => to,
        _ => shift.seam(),
    };
    // Moved latest first: a clip moved later never lands where one still to move sits
    // (nothing refuses that, but the order is the same every time, so undo is exact).
    let mut later: Vec<Clip> = clips_of(e, t)
        .into_iter()
        .filter(|c| c.start >= first && !scope.exclude.contains(&c.id))
        .collect();
    match shift {
        Shift::Insert { .. } => later.sort_by_key(|c| std::cmp::Reverse((c.start, c.id))),
        Shift::Delete { .. } => later.sort_by_key(|c| (c.start, c.id)),
    }
    for c in &later {
        let to = shift.point(c.start);
        if to != c.start {
            let cmds = move_clip(&e.scratch, maps, t, c, to);
            e.run_all(cmds)?;
        }
    }
    if let (true, Shift::Delete { from, .. }) = (scope.cut, shift) {
        heal(e, t, from)?;
    }
    Some(())
}

/// The clips of `t` (every lane) as the edit has left them so far.
pub(crate) fn clips_of(e: &Edit, t: TrackId) -> Vec<Clip> {
    e.scratch
        .track(t)
        .map(|tr| tr.clips.clone())
        .unwrap_or_default()
}

/// Split every clip of `t` that runs across `at` ([`split_any`]).
pub(crate) fn cut_at(e: &mut Edit, t: TrackId, at: Ticks, ids: Ids) -> Option<()> {
    for c in clips_of(e, t)
        .iter()
        .filter(|c| c.start < at && at < c.end())
    {
        let cmds = split_any(&e.scratch, t, c, at, ids)?;
        e.run_all(cmds)?;
    }
    Some(())
}

/// Take every clip of `t` out of `[from, to)`, cutting those across its edges; nothing
/// moves.
pub(crate) fn clear(e: &mut Edit, t: TrackId, from: Ticks, to: Ticks, ids: Ids) -> Option<()> {
    cut_at(e, t, from, ids)?;
    cut_at(e, t, to, ids)?;
    for c in clips_of(e, t)
        .iter()
        .filter(|c| c.start >= from && c.end() <= to)
    {
        e.run(Command::RemoveClip {
            track: t,
            clip: c.id,
        })?;
    }
    Some(())
}

/// Split `clip` at `at` the way the arrangement's *Split* does ([`clipboard::split`]); a
/// clip that loops, cut where no repeat begins, by its pieces ([`clipboard::loop_pieces`]:
/// the model has no loop phase, so the piece after the cut plays what the loop played
/// there). The commands keep the left part's id.
pub fn split_any(
    project: &Project,
    track: TrackId,
    clip: &Clip,
    at: Ticks,
    ids: Ids,
) -> Option<Vec<Command>> {
    if at <= clip.start || at >= clip.end() {
        return None;
    }
    let loops = clip.period().is_some_and(|p| p < clip.length);
    if !loops || clip.audio.is_some() {
        return clipboard::split(project, track, clip, at, ids);
    }
    let mut out = vec![Command::SetClipBounds {
        track,
        clip: clip.id,
        start: clip.start,
        length: at - clip.start,
    }];
    for piece in clipboard::loop_pieces(clip, at, clip.end(), ids) {
        out.push(Command::InsertClip {
            track,
            clip: Box::new(piece),
        });
    }
    Some(out)
}

/// The commands that move `clip` of `track` to start at `to`, its content with it. An
/// unwarped audio window is laid through the tempo maps before and after the edit so it
/// plays the same second of its file at its start (`audio_time::relocate`), as a move of a
/// clip does; with no tempo track every tick is kept as it is.
pub(crate) fn move_clip(
    project: &Project,
    maps: &Maps,
    track: TrackId,
    clip: &Clip,
    to: Ticks,
) -> Vec<Command> {
    let moved = moved_clip(project, maps, clip, to);
    changes(track, clip, &moved)
}

/// `clip` laid at `to` (see [`move_clip`]), as a value.
pub(crate) fn moved_clip(project: &Project, maps: &Maps, clip: &Clip, to: Ticks) -> Clip {
    let mut moved = clip.clone();
    moved.start = to;
    if !maps.tempo_track {
        return moved;
    }
    let Some(a) = clip.audio.as_ref() else {
        return moved;
    };
    let rate = project
        .audio_source(a.source)
        .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1)));
    let Some(m) = crate::audio_time::relocate(maps.old, clip.start, clip, maps.new, to, rate)
    else {
        return moved;
    };
    moved.offset = m.offset;
    moved.length = m.length;
    if moved.link.is_none() {
        if let Some(r) = moved.audio.as_mut() {
            r.fade_in = m.fade_in;
            r.fade_out = m.fade_out;
            r.slip_frames = m.slip_frames;
            // The gain envelope stays on its audio (ADR 0070 review 1).
            if let Some(env) = r.gain_env.as_mut() {
                **env = crate::audio_time::remap_env(
                    env,
                    maps.old,
                    clip.start,
                    clip.offset,
                    maps.new,
                    to,
                    m.offset,
                );
            }
        }
    }
    moved
}

/// The commands that turn `clip` into `moved`: its bounds, its offset, its region.
pub(crate) fn changes(track: TrackId, clip: &Clip, moved: &Clip) -> Vec<Command> {
    let mut out = Vec::new();
    if moved.start != clip.start || moved.length != clip.length {
        out.push(Command::SetClipBounds {
            track,
            clip: clip.id,
            start: moved.start,
            length: moved.length,
        });
    }
    if moved.offset != clip.offset {
        out.push(Command::SetClipOffset {
            track,
            clip: clip.id,
            offset: moved.offset,
        });
    }
    if moved.audio != clip.audio {
        out.push(Command::SetClipAudio {
            track,
            clip: clip.id,
            audio: moved.audio.clone(),
        });
    }
    out
}

/// Glue every pair of clips on `t` that meet at `at` and continue each other ([`continues`]),
/// as the halves of one split: the left keeps its id and the right's end and fade out.
fn heal(e: &mut Edit, t: TrackId, at: Ticks) -> Option<()> {
    let Some(tr) = e.scratch.track(t) else {
        return Some(());
    };
    let mut pairs: Vec<(ClipId, Clip)> = Vec::new();
    let mut used: Vec<ClipId> = Vec::new();
    for l in tr.clips.iter().filter(|c| c.end() == at) {
        let r = tr
            .clips
            .iter()
            .find(|r| r.start == at && !used.contains(&r.id) && continues(l, r));
        if let Some(r) = r {
            used.push(r.id);
            pairs.push((l.id, r.clone()));
        }
    }
    for (l, r) in pairs {
        let left = e.scratch.track(t)?.clip(l)?.clone();
        e.run(Command::RemoveClip {
            track: t,
            clip: r.id,
        })?;
        let mut whole = left.clone();
        whole.length = left.length + r.length;
        if left.audio.as_ref().is_some_and(|a| a.reverse) {
            whole.offset = r.offset;
        }
        if let (Some(w), Some(ra)) = (whole.audio.as_mut(), r.audio.as_ref()) {
            w.fade_out = ra.fade_out;
        }
        e.run_all(changes(t, &left, &whole))?;
    }
    Some(())
}

/// Whether `r` goes on from `l` as the other half of one split: the same lane, `r` starting
/// where `l` ends, the same content, and the content going on — a forward window's offset
/// advanced by `l`'s length (a loop's by whole repeats), a reversed one's the other way
/// round — with the cut's own hard edges (no fade out on `l`, no fade in on `r`).
pub fn continues(l: &Clip, r: &Clip) -> bool {
    if r.start != l.end()
        || l.lane != r.lane
        || l.muted != r.muted
        || l.content_length != r.content_length
        || l.link != r.link
        || l.comp != r.comp
        || l.name != r.name
        || l.color != r.color
        || l.notes != r.notes
        || l.automation != r.automation
        || l.lanes != r.lanes
        || l.groove != r.groove
        || l.skip_note_fx != r.skip_note_fx
    {
        return false;
    }
    let going_on = match (l.period(), l.audio.as_ref()) {
        (Some(p), None) if p < l.length + r.length => {
            r.offset == l.offset && (r.start - l.start).0 % p.0 == 0
        }
        (_, Some(a)) if a.reverse => l.offset == r.offset + r.length,
        _ => r.offset == l.content_of(r.start),
    };
    if !going_on {
        return false;
    }
    match (l.audio.as_ref(), r.audio.as_ref()) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.fade_out == Ticks::ZERO
                && b.fade_in == Ticks::ZERO
                && b.xfade == Ticks::ZERO
                && b.cross.is_none()
                && a.slip_frames == b.slip_frames
                && crate::AudioRegion {
                    fade_in: Ticks::ZERO,
                    fade_out: Ticks::ZERO,
                    xfade: Ticks::ZERO,
                    cross: None,
                    ..a.clone()
                } == crate::AudioRegion {
                    fade_in: Ticks::ZERO,
                    fade_out: Ticks::ZERO,
                    xfade: Ticks::ZERO,
                    cross: None,
                    ..b.clone()
                }
        }
        _ => false,
    }
}

/// The alternate comps' kept windows of `t` through `f` — each comp's windows as the clips
/// of a track of their own on a side copy of the song, edited by the same passes as the
/// track's clips — then one `SetTrackComps`, so a comp switched to after the edit lies where
/// its takes now are.
pub(crate) fn with_comps(
    e: &mut Edit,
    t: TrackId,
    f: &mut dyn FnMut(&mut Edit) -> Option<()>,
) -> Option<()> {
    let Some(tr) = e.scratch.track(t) else {
        return Some(());
    };
    if tr.comps.iter().all(|c| c.windows.is_empty()) {
        return Some(());
    }
    let (mut comps, active) = (tr.comps.clone(), tr.active_comp);
    for comp in comps.iter_mut() {
        if comp.windows.is_empty() {
            continue;
        }
        let mut side = Project::empty("");
        side.audio_pool = e.scratch.audio_pool.clone();
        side.tempo_bpm = e.scratch.tempo_bpm;
        side.tempo_points = e.scratch.tempo_points.clone();
        side.time_signature = e.scratch.time_signature;
        side.metre_changes = e.scratch.metre_changes.clone();
        side.next_id = e.scratch.next_id;
        let mut track = e.scratch.track(t)?.clone();
        track.clips = std::mem::take(&mut comp.windows);
        track.comps = Vec::new();
        track.active_comp = 0;
        side.tracks = vec![track];
        let mut sub = Edit {
            scratch: side,
            out: Vec::new(),
        };
        f(&mut sub)?;
        comp.windows = sub.scratch.tracks.pop()?.clips;
    }
    if e.scratch.track(t).is_some_and(|tr| tr.comps == comps) {
        return Some(());
    }
    e.run(Command::SetTrackComps {
        track: t,
        comps,
        active,
    })
}

#[cfg(test)]
#[path = "ripple_tests.rs"]
pub(crate) mod tests;
