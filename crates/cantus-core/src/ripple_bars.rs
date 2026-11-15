//! r40:ripple — the section drag (ADR 0116 §3; des-arrange's *Sections lane*: "Dragging it
//! moves its bars on every track; Ctrl copies, Shift inserts").
//!
//! A section dragged in the sections lane takes **its bars on every track** with it: the
//! clips (cut at its edges, every lane), the alternate comps' windows, its markers, the
//! mixer's cues in it and the section itself. Four gestures, one command each:
//!
//! | Gesture | What happens | The song's length |
//! |---|---|---|
//! | drag | the bars move; where they land is replaced, where they were is left silent | same |
//! | Ctrl drag | a copy of the bars replaces where it lands | same |
//! | Shift drag | the bars are taken out and put in where they land: what was there moves on (the song rearranged — Logic's arrangement markers, Studio One's arranger) | same |
//! | Ctrl+Shift drag | a copy is put in where it lands, the rest moves on | longer by the section |
//!
//! The inserting gestures move **time**: the section's tempo and metre go with its bars and
//! the song's clock opens and closes around them ([`crate::ripple`]'s passes). The replacing
//! gestures keep the clock: the bars land under the tempo and metre of where they land, as a
//! clip moved does (an unwarped audio window still plays the same second of its file).

use crate::arrange::{ArrangeCommand, Section, SectionId};
use crate::clipboard::{self, Ids};
use crate::command::Command;
use crate::mix_cmd::MixCommand;
use crate::mix_snap::{MixCue, MixCueId};
use crate::model::{Clip, ClipId, Project, TrackId};
use crate::ripple::{self, Edit, Maps, Shift};
use crate::tempo::{Marker, MarkerId};
use crate::time::Ticks;

/// How a section drag lands: Ctrl copies, Shift inserts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Landing {
    pub copy: bool,
    pub insert: bool,
}

impl Landing {
    /// The undo label of this kind of drag.
    pub fn label(self) -> &'static str {
        match (self.copy, self.insert) {
            (false, false) => "cmd.ripple.move_bars",
            (true, false) => "cmd.ripple.copy_bars",
            (false, true) => "cmd.ripple.insert_bars",
            (true, true) => "cmd.ripple.insert_copy_bars",
        }
    }

    /// Where the bars `[from, to)` dropped at `dest` begin once the drag is done: an
    /// inserting move dropped after its own bars lands earlier by their length (they are
    /// taken out of the song before they go in).
    pub fn lands_at(self, from: Ticks, to: Ticks, dest: Ticks) -> Ticks {
        if self.insert && !self.copy && dest > to {
            dest - (to - from)
        } else {
            dest
        }
    }

    /// Whether dropping `[from, to)` at `dest` changes nothing.
    pub fn is_still(self, from: Ticks, to: Ticks, dest: Ticks) -> bool {
        match (self.copy, self.insert) {
            (false, false) => dest == from,
            (false, true) => dest >= from && dest <= to,
            _ => false,
        }
    }
}

/// What a section drag carries, taken from the song before anything moves.
struct Carried {
    clips: Vec<(TrackId, Clip)>,
    comps: Vec<(TrackId, usize, Clip)>,
    markers: Vec<Marker>,
    sections: Vec<Section>,
    cues: Vec<MixCue>,
    tempo: Vec<crate::tempo::TempoPoint>,
    metres: Vec<crate::tempo::MetreChange>,
}

/// Drag the bars `[from, to)` of the whole song so they begin at `dest` (in the song as it
/// is before the drag), as `how` says. One command; `None` when it changes nothing or the
/// model refuses a step.
pub fn move_bars(
    project: &Project,
    from: Ticks,
    to: Ticks,
    dest: Ticks,
    how: Landing,
    ids: Ids,
) -> Option<Command> {
    if to <= from || from < Ticks::ZERO || dest < Ticks::ZERO || how.is_still(from, to, dest) {
        return None;
    }
    let len = to - from;
    let orig_map = project.tempo_map();
    let mut e = Edit::new(project);
    let tracks: Vec<TrackId> = project.tracks.iter().map(|t| t.id).collect();
    // The bars cut out on every track (and in every alternate comp), then taken.
    for &t in &tracks {
        ripple::cut_at(&mut e, t, from, ids)?;
        ripple::cut_at(&mut e, t, to, ids)?;
        ripple::with_comps(&mut e, t, &mut |sub| {
            ripple::cut_at(sub, t, from, ids)?;
            ripple::cut_at(sub, t, to, ids)
        })?;
    }
    let carried = carry(&e.scratch, from, to);
    let land = how.lands_at(from, to, dest);
    if how.insert {
        if !how.copy {
            ripple::apply_shift(&mut e, Shift::Delete { from, to }, ids)?;
        }
        ripple::apply_shift(&mut e, Shift::Insert { at: land, len }, ids)?;
        paste_clock(&mut e, &carried, land, len)?;
        split_sections(&mut e, land, land + len, ids)?;
    } else {
        if !how.copy {
            clear_bars(&mut e, &tracks, from, to, ids)?;
        }
        clear_bars(&mut e, &tracks, land, land + len, ids)?;
    }
    let maps_new = e.scratch.tempo_map();
    let maps = Maps {
        old: &orig_map,
        new: &maps_new,
        tempo_track: !project.tempo_points.is_empty() || !e.scratch.tempo_points.is_empty(),
    };
    paste_clips(&mut e, project, &maps, &carried, from, land, how.copy, ids)?;
    paste_marks(&mut e, &carried, from, land, how.copy, ids)?;
    crate::ripple_cues::keep_scenes(&mut e, project, &pieces(from, to, land, how), ids)?;
    e.finish(how.label())
}

/// Where a section drag's music went (R2-B2): the bars at `land`, playing what began at
/// `from`; the music after them (where they replaced, the song's own at that place; where
/// they went in, what was there before); and, a move that leaves its place, the music after
/// the place it left. Each starts under the scene it had ([`crate::ripple_cues`]).
fn pieces(from: Ticks, to: Ticks, land: Ticks, how: Landing) -> Vec<crate::ripple_cues::Piece> {
    use crate::ripple_cues::Piece;
    let len = to - from;
    let end = land + len;
    let mut out = vec![Piece {
        new: land,
        old: from,
    }];
    match (how.insert, how.copy) {
        // Replaced: the song's own after the bars, and after the silence they left.
        (false, copy) => {
            if !(from..to).contains(&end) || copy {
                out.push(Piece { new: end, old: end });
            }
            if !copy && !(land..end).contains(&to) {
                out.push(Piece { new: to, old: to });
            }
        }
        // Moved in: what followed the bars' place closes up there, and what was at the
        // place they went in follows them.
        (true, false) => {
            let closed = Shift::Delete { from, to };
            let opened = Shift::Insert { at: land, len };
            out.push(Piece {
                new: opened.point(from),
                old: to,
            });
            // The old tick that now sits at `land`, before the bars went in.
            let at = if land >= from { land + len } else { land };
            debug_assert_eq!(closed.point(at), land);
            out.push(Piece { new: end, old: at });
        }
        // A copy put in: what was at `land` follows it.
        (true, true) => out.push(Piece {
            new: end,
            old: land,
        }),
    }
    out
}

/// What lies in `[from, to)` of `p` (already cut at both edges).
fn carry(p: &Project, from: Ticks, to: Ticks) -> Carried {
    let inside = |c: &Clip| c.start >= from && c.end() <= to;
    let mut clips = Vec::new();
    let mut comps = Vec::new();
    for t in &p.tracks {
        clips.extend(
            t.clips
                .iter()
                .filter(|c| inside(c))
                .map(|c| (t.id, c.clone())),
        );
        for (i, comp) in t.comps.iter().enumerate() {
            comps.extend(
                comp.windows
                    .iter()
                    .filter(|c| inside(c))
                    .map(|c| (t.id, i, c.clone())),
            );
        }
    }
    let markers = p
        .markers
        .iter()
        .filter(|m| m.tick >= from && m.tick < to)
        .cloned()
        .collect();
    let cues = p
        .mix
        .cues
        .iter()
        .filter(|c| c.at >= from && c.at < to)
        .cloned()
        .collect();
    let sections = p
        .sections
        .iter()
        .filter_map(|s| {
            let (a, b) = (s.start.max(from), s.end().min(to));
            (b > a).then(|| Section {
                start: a,
                length: b - a,
                ..s.clone()
            })
        })
        .collect();
    Carried {
        clips,
        comps,
        markers,
        sections,
        cues,
        tempo: crate::ripple_tempo::tempo_span(p, from, to),
        metres: crate::ripple_tempo::metre_span(p, from, to),
    }
}

/// Clear `[from, to)` without moving anything: clips (every track, lane and alternate comp),
/// markers, cues and sections there go; a section across an edge keeps what lies outside.
fn clear_bars(e: &mut Edit, tracks: &[TrackId], from: Ticks, to: Ticks, ids: Ids) -> Option<()> {
    for &t in tracks {
        ripple::clear(e, t, from, to, ids)?;
        ripple::with_comps(e, t, &mut |sub| ripple::clear(sub, t, from, to, ids))?;
    }
    for m in e.scratch.markers.clone() {
        if m.tick >= from && m.tick < to {
            e.run(Command::RemoveMarker { id: m.id })?;
        }
    }
    for c in e.scratch.mix.cues.clone() {
        if c.at >= from && c.at < to {
            e.run(Command::Mix(MixCommand::RemoveCue { id: c.id }))?;
        }
    }
    for s in e.scratch.sections.clone() {
        if s.end() <= from || s.start >= to {
            continue;
        }
        let (left, right) = (s.start < from, s.end() > to);
        if left {
            e.run(Command::Arrange(ArrangeCommand::SetSection {
                section: Section {
                    length: from - s.start,
                    ..s.clone()
                },
            }))?;
        } else if right {
            e.run(Command::Arrange(ArrangeCommand::SetSection {
                section: Section {
                    start: to,
                    length: s.end() - to,
                    ..s.clone()
                },
            }))?;
        } else {
            e.run(Command::Arrange(ArrangeCommand::RemoveSection { id: s.id }))?;
        }
        if left && right {
            // Across both edges: what follows the cleared bars is a section of its own.
            let index = e.scratch.sections.iter().filter(|o| o.start < to).count();
            e.run(Command::Arrange(ArrangeCommand::InsertSection {
                index,
                section: Section {
                    id: SectionId(ids()),
                    start: to,
                    length: s.end() - to,
                    ..s.clone()
                },
            }))?;
        }
    }
    Some(())
}

/// A section the inserted bars went into (it grew around them) is cut around them: what it
/// held before them keeps its id, what it held after is a section of its own of the same
/// name — the bars carry their own section.
fn split_sections(e: &mut Edit, from: Ticks, to: Ticks, ids: Ids) -> Option<()> {
    for s in e.scratch.sections.clone() {
        if s.start >= from || s.end() <= from {
            continue;
        }
        e.run(Command::Arrange(ArrangeCommand::SetSection {
            section: Section {
                length: from - s.start,
                ..s.clone()
            },
        }))?;
        if s.end() > to {
            let index = e.scratch.sections.iter().filter(|o| o.start < to).count();
            e.run(Command::Arrange(ArrangeCommand::InsertSection {
                index,
                section: Section {
                    id: SectionId(ids()),
                    start: to,
                    length: s.end() - to,
                    ..s
                },
            }))?;
        }
    }
    Some(())
}

/// The carried tempo and metre laid over `[land, land + len)` (the inserting drags).
fn paste_clock(e: &mut Edit, c: &Carried, land: Ticks, len: Ticks) -> Option<()> {
    let (bpm, points) = crate::ripple_tempo::paste_tempo(&e.scratch, land, len, &c.tempo);
    if bpm != e.scratch.tempo_bpm || points != e.scratch.tempo_points {
        e.run(Command::SetTempoTrack { bpm, points })?;
    }
    let (signature, metres) = crate::ripple_tempo::paste_metres(&e.scratch, land, len, &c.metres);
    if signature != e.scratch.time_signature || metres != e.scratch.metre_changes {
        e.run(Command::SetMetreTrack { signature, metres })?;
    }
    Some(())
}

/// A carried clip as it goes back in: the same clip moved, or a copy — a clip of its own
/// (fresh ids), or another placement of the same clip when it is linked (FL's pattern clips:
/// a copied section plays the same patterns).
fn landed(clip: &Clip, copy: bool, ids: Ids) -> Clip {
    if !copy {
        return clip.clone();
    }
    if clip.link.is_some() {
        return Clip {
            id: ClipId(ids()),
            comp: false,
            ..clip.clone()
        };
    }
    clipboard::copy_of(clip, clip.start, ids)
}

#[allow(clippy::too_many_arguments)]
fn paste_clips(
    e: &mut Edit,
    project: &Project,
    maps: &Maps,
    c: &Carried,
    from: Ticks,
    land: Ticks,
    copy: bool,
    ids: Ids,
) -> Option<()> {
    for (t, clip) in &c.clips {
        let piece = landed(clip, copy, ids);
        let laid = ripple::moved_clip(project, maps, &piece, land + (clip.start - from));
        e.run(Command::InsertClip {
            track: *t,
            clip: Box::new(laid),
        })?;
    }
    let tracks: Vec<TrackId> = project.tracks.iter().map(|t| t.id).collect();
    for t in tracks {
        let mine: Vec<&(TrackId, usize, Clip)> = c.comps.iter().filter(|w| w.0 == t).collect();
        if mine.is_empty() {
            continue;
        }
        let Some(tr) = e.scratch.track(t) else {
            continue;
        };
        let (mut comps, active) = (tr.comps.clone(), tr.active_comp);
        for (_, i, w) in mine {
            let mut piece = landed(w, copy, ids);
            piece.comp = true;
            let laid = ripple::moved_clip(project, maps, &piece, land + (w.start - from));
            if let Some(comp) = comps.get_mut(*i) {
                comp.windows.push(laid);
                comp.windows.sort_by_key(|x| (x.start, x.id));
            }
        }
        e.run(Command::SetTrackComps {
            track: t,
            comps,
            active,
        })?;
    }
    Some(())
}

fn paste_marks(
    e: &mut Edit,
    c: &Carried,
    from: Ticks,
    land: Ticks,
    copy: bool,
    ids: Ids,
) -> Option<()> {
    for m in &c.markers {
        let id = if copy { MarkerId(ids()) } else { m.id };
        e.run(Command::AddMarker {
            marker: Marker {
                id,
                tick: land + (m.tick - from),
                ..m.clone()
            },
        })?;
    }
    // The cues in the bars land with them (a copy is a cue of its own), at the list's end.
    for q in &c.cues {
        let id = if copy { MixCueId(ids()) } else { q.id };
        e.run(Command::Mix(MixCommand::InsertCue {
            index: e.scratch.mix.cues.len(),
            cue: MixCue {
                id,
                at: land + (q.at - from),
                ..q.clone()
            },
        }))?;
    }
    for s in &c.sections {
        // A copy is a section of its own; so is the part of a section the bars took when
        // the rest of it stayed behind (the section's id is the rest's).
        let id = if copy || e.scratch.section(s.id).is_some() {
            SectionId(ids())
        } else {
            s.id
        };
        let start = land + (s.start - from);
        let index = e
            .scratch
            .sections
            .iter()
            .filter(|o| o.start < start)
            .count();
        e.run(Command::Arrange(ArrangeCommand::InsertSection {
            index,
            section: Section {
                id,
                start,
                ..s.clone()
            },
        }))?;
    }
    Some(())
}

#[cfg(test)]
#[path = "ripple_bars_tests.rs"]
mod tests;
