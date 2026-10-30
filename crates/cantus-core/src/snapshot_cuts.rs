//! The notes a snapshot ends sooner than the one playing (ADR 0076 §7): worked out where
//! the snapshot is sent, off the audio thread, so an install while playing only walks a
//! list — a note the new snapshot ends behind the playhead is ended at once (its note-off
//! was passed: it would ring on), and nothing else is (a note pasted behind the playhead
//! was never started: no note-off for it, which would cut a held note of that pitch on a
//! hosted instrument).

use super::{NoteEvent, Snapshot, TrackSnapshot};
use crate::model::{NoteId, Pitch, TrackId};
use crate::time::{Ticks, PPQ};
use std::collections::HashMap;

/// How far past where the snapshot is cut the playhead may be when it installs it: four
/// beats (a snapshot reaches the audio thread within a lead — tens of milliseconds).
pub const CUT_AHEAD: Ticks = Ticks(PPQ * 4);

/// At most this many cuts a snapshot carries, the soonest kept (the install walks them all:
/// a comparison or three each).
const MAX_CUTS: usize = 16_384;

/// A note the old snapshot started and the new one ends sooner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CutNote {
    /// The track's index in the new snapshot.
    pub track: usize,
    pub pitch: Pitch,
    /// The voice tag (`NoteEvent::note`).
    pub note: NoteId,
    pub on: Ticks,
    /// Where the new snapshot ends it.
    pub off: Ticks,
    /// Where the old one did.
    pub was_off: Ticks,
    pub release: u8,
}

impl CutNote {
    /// Whether an install ends it whose playhead's first tick still ahead is `x` (the
    /// events before `x` were played): the old snapshot had started it and not yet ended
    /// it, and the new one's end is behind.
    pub fn due(&self, x: Ticks) -> bool {
        self.on < x && self.off < x && x <= self.was_off
    }
}

/// A note of a track's events: (voice tag, on) → (pitch, off, release velocity).
type Notes = HashMap<(NoteId, Ticks), (Pitch, Ticks, u8)>;

/// The notes of `events` that start at or before `to` and end after `after` (one never
/// ended ends at `Ticks::MAX`).
fn notes(events: &[NoteEvent], to: Ticks, after: Ticks) -> Notes {
    let mut open: HashMap<NoteId, (Ticks, Pitch)> = HashMap::new();
    let mut out = Notes::new();
    for e in events {
        if e.tick > to && open.is_empty() {
            break;
        }
        if e.is_on() {
            if e.tick <= to {
                open.insert(e.note, (e.tick, e.pitch));
            }
        } else if let Some((on, pitch)) = open.remove(&e.note) {
            if e.tick > after {
                out.insert((e.note, on), (pitch, e.tick, e.release));
            }
        }
    }
    for (note, (on, pitch)) in open {
        out.insert((note, on), (pitch, Ticks(i64::MAX), 0));
    }
    out
}

impl Snapshot {
    /// The notes `old` (the snapshot playing when this one is sent, its playhead at `at`)
    /// started that this one ends sooner, for an install up to [`CUT_AHEAD`] later: what
    /// the engine ends at once when it installs this while playing ([`CutNote::due`]).
    /// Linear in the events of the tracks whose notes changed (the others are skipped); a
    /// note this snapshot no longer has at all (or has under another tag) is left as the
    /// install has always left it.
    pub fn cut_from(&mut self, old: &Snapshot, at: Ticks) {
        self.cuts.clear();
        // Sounding at `at` or after it, in the old.
        let from = at - Ticks(1);
        let to = at + CUT_AHEAD;
        let olds: HashMap<TrackId, &TrackSnapshot> = old.tracks.iter().map(|t| (t.id, t)).collect();
        for (ti, t) in self.tracks.iter().enumerate() {
            let Some(o) = olds.get(&t.id) else {
                continue;
            };
            if o.events == t.events {
                continue;
            }
            let was = notes(&o.events, to, from);
            if was.is_empty() {
                continue;
            }
            let now = notes(&t.events, to, Ticks(i64::MIN));
            let mut cuts: Vec<CutNote> = was
                .iter()
                .filter_map(|(&(note, on), &(pitch, was_off, _))| {
                    let &(_, off, release) = now.get(&(note, on))?;
                    (off < was_off && off <= to).then_some(CutNote {
                        track: ti,
                        pitch,
                        note,
                        on,
                        off,
                        was_off,
                        release,
                    })
                })
                .collect();
            self.cuts.append(&mut cuts);
        }
        // In a fixed order (a map's is not), the soonest first.
        self.cuts.sort_by_key(|c| (c.on, c.track, c.note));
        self.cuts.truncate(MAX_CUTS);
    }
}

#[cfg(test)]
mod tests {
    use crate::model::Project;
    use crate::snapshot::Snapshot;
    use crate::time::Ticks;

    fn song() -> Project {
        let mut p = Project::new("cuts");
        let t = p.make_track("Keys");
        p.tracks.push(t);
        let mut c = p.make_clip("held", Ticks::ZERO, Ticks::beats(8));
        let n = p.make_note(Ticks::ZERO, Ticks::beats(8), 60, 100);
        c.notes.push(n);
        p.tracks[0].clips.push(c);
        p
    }

    /// N3: a note pasted behind the playhead was never started by the snapshot playing: no
    /// note-off for it (on a hosted instrument it would end the held note of its pitch).
    #[test]
    fn a_note_pasted_behind_the_playhead_is_not_cut() {
        let mut p = song();
        let old = Snapshot::compile(&p, 1);
        let n = p.make_note(Ticks::beats(1), Ticks::QUARTER, 60, 90);
        p.tracks[0].clips[0].notes.push(n);
        let mut new = Snapshot::compile(&p, 2);
        new.cut_from(&old, Ticks::beats(3));
        assert!(new.cuts.is_empty(), "{:?}", new.cuts);
    }

    /// A held note shortened to end behind the playhead is cut there, and only once the
    /// playhead has passed its new end and not its old one.
    #[test]
    fn a_note_ended_behind_the_playhead_is_cut_where_it_was_playing() {
        let mut p = song();
        let old = Snapshot::compile(&p, 1);
        p.tracks[0].clips[0].length = Ticks::beats(2);
        let mut new = Snapshot::compile(&p, 2);
        new.cut_from(&old, Ticks::beats(3));
        assert_eq!(new.cuts.len(), 1);
        let c = new.cuts[0];
        assert_eq!(
            (c.on, c.off, c.was_off),
            (Ticks::ZERO, Ticks::beats(2), Ticks::beats(8))
        );
        assert!(c.due(Ticks::beats(3)));
        assert!(!c.due(Ticks::beats(2)), "its new end is still to be played");
        assert!(!c.due(Ticks::beats(9)), "the old one ended it");
        // Unchanged: nothing to cut.
        let mut same = Snapshot::compile(&p, 3);
        same.cut_from(&new, Ticks::beats(3));
        assert!(same.cuts.is_empty());
    }
}
