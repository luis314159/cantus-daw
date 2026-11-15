//! r40:ripple — what marks the song's time through a time edit (ADR 0116 §2): the markers,
//! the sections, the mixer's cues, the loop range and the punch range.
//!
//! A mark at or after the insert point moves with the music; a span across it grows by the
//! new time (bars inserted inside the *Verse* make the verse longer, as Logic's arrangement
//! markers and Studio One's arranger do). Deleting, a mark inside the range goes with it,
//! a span across an edge loses what was inside, a later one moves back. A loop or punch
//! range that falls wholly inside deleted time is cleared (and switched off): it would
//! otherwise loop over music it never held. A mixer cue (ADR 0093: a snapshot recalled from
//! its place on) is a point of the song as a marker is: it moves with the music, and goes
//! with the time it was in.

use crate::arrange::ArrangeCommand;
use crate::command::Command;
use crate::model::LoopRegion;
use crate::ripple::{Edit, Shift};

/// A range `[start, end)` through `shift`: `None` when nothing of it is left.
pub fn span(
    shift: Shift,
    start: crate::Ticks,
    end: crate::Ticks,
) -> Option<(crate::Ticks, crate::Ticks)> {
    let s = shift.point(start);
    let e = shift.end(end);
    (e > s).then_some((s, e))
}

/// Markers, sections, the mixer's cues, the loop and the punch, through `shift`, onto `e`
/// (`keep`: cues inside deleted time that stay where they are).
pub(crate) fn shift_marks(
    e: &mut Edit,
    shift: Shift,
    keep: &[crate::mix_snap::MixCueId],
) -> Option<()> {
    markers(e, shift)?;
    sections(e, shift)?;
    cues(e, shift, keep)?;
    loop_range(e, shift)?;
    punch_range(e, shift)
}

fn markers(e: &mut Edit, shift: Shift) -> Option<()> {
    let markers = e.scratch.markers.clone();
    for m in markers {
        let gone = matches!(shift, Shift::Delete { from, to } if m.tick >= from && m.tick < to);
        if gone {
            e.run(Command::RemoveMarker { id: m.id })?;
            continue;
        }
        let tick = shift.point(m.tick);
        if tick != m.tick {
            e.run(Command::SetMarker {
                marker: crate::tempo::Marker { tick, ..m },
            })?;
        }
    }
    Some(())
}

/// The mixer's cues (review mid-round 40, B-1): as the markers — one inside deleted time
/// goes, a later one moves with the music. Each keeps its place in the list (a `SetCue`),
/// so inserting and deleting the same time gives the list back as it was. A cue in `keep`
/// stays where it is. The scene each piece of music plays under is the edit's to keep
/// afterwards ([`crate::ripple_cues`], R2-B2).
fn cues(e: &mut Edit, shift: Shift, keep: &[crate::mix_snap::MixCueId]) -> Option<()> {
    use crate::mix_cmd::MixCommand;
    for c in e.scratch.mix.cues.clone() {
        let inside = matches!(shift, Shift::Delete { from, to } if c.at >= from && c.at < to);
        if inside && keep.contains(&c.id) {
            continue;
        }
        if inside {
            e.run(Command::Mix(MixCommand::RemoveCue { id: c.id }))?;
            continue;
        }
        let at = shift.point(c.at);
        if at != c.at {
            e.run(Command::Mix(MixCommand::SetCue {
                cue: crate::mix_snap::MixCue { at, ..c },
            }))?;
        }
    }
    Some(())
}

fn sections(e: &mut Edit, shift: Shift) -> Option<()> {
    let mut list = e.scratch.sections.clone();
    // Gone first; then, inserting, the latest first (each moves into room already made),
    // deleting the earliest first: no step ever overlaps another section.
    for s in &list {
        if span(shift, s.start, s.end()).is_none() {
            e.run(Command::Arrange(ArrangeCommand::RemoveSection { id: s.id }))?;
        }
    }
    list.retain(|s| span(shift, s.start, s.end()).is_some());
    match shift {
        Shift::Insert { .. } => list.sort_by_key(|s| std::cmp::Reverse(s.start)),
        Shift::Delete { .. } => list.sort_by_key(|s| s.start),
    }
    for s in list {
        let Some((start, end)) = span(shift, s.start, s.end()) else {
            continue;
        };
        if start != s.start || end != s.end() {
            e.run(Command::Arrange(ArrangeCommand::SetSection {
                section: crate::arrange::Section {
                    start,
                    length: end - start,
                    ..s
                },
            }))?;
        }
    }
    Some(())
}

fn loop_range(e: &mut Edit, shift: Shift) -> Option<()> {
    let Some(r) = e.scratch.loop_region.clone() else {
        return Some(());
    };
    match span(shift, r.start, r.end) {
        Some((start, end)) if (start, end) != (r.start, r.end) => e.run(Command::SetLoopRegion {
            region: Some(LoopRegion { start, end }),
        }),
        Some(_) => Some(()),
        None => {
            if e.scratch.loop_enabled {
                e.run(Command::SetLoopEnabled { enabled: false })?;
            }
            e.run(Command::SetLoopRegion { region: None })
        }
    }
}

fn punch_range(e: &mut Edit, shift: Shift) -> Option<()> {
    let settings = e.scratch.recording.clone();
    let Some(r) = settings.punch else {
        return Some(());
    };
    let punch = span(shift, r.start, r.end).map(|(start, end)| LoopRegion { start, end });
    if punch == Some(r) {
        return Some(());
    }
    let enabled = settings.punch_enabled && punch.is_some();
    e.run(Command::SetRecordSettings {
        settings: crate::audio::RecordSettings {
            punch,
            punch_enabled: enabled,
            ..settings
        },
    })
}
