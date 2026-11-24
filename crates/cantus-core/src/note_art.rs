//! The note's articulation and bow (round 42, ADR 0158): how a note is played — sustained,
//! legato, staccato, spiccato, pizzicato, tremolo — and, for a bowed string, which way the
//! bow moves. Held as `Note::art`; `None` (every older note) lets the instrument decide.
//! Owned by the `strings-rec` builder; the contract declares the types and the command.
//!
//! `Command::SetNoteArt` sets the articulation of the notes it names; its inverse — the same
//! command with the old values — gives them back exactly.

use crate::command::{edit_shared, Command, CommandError};
use crate::model::{Clip, ClipId, NoteId, Project, TrackId};
use serde::{Deserialize, Serialize};

/// How a note is played. **Append only, never reorder** (a song stores the name).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    Sustain,
    Legato,
    Staccato,
    Spiccato,
    Pizzicato,
    Tremolo,
}

impl Articulation {
    pub const ALL: [Articulation; 6] = [
        Articulation::Sustain,
        Articulation::Legato,
        Articulation::Staccato,
        Articulation::Spiccato,
        Articulation::Pizzicato,
        Articulation::Tremolo,
    ];

    /// Its locale key (`areas/strings_rec.*`).
    pub fn name_key(self) -> &'static str {
        match self {
            Articulation::Sustain => "art.sustain",
            Articulation::Legato => "art.legato",
            Articulation::Staccato => "art.staccato",
            Articulation::Spiccato => "art.spiccato",
            Articulation::Pizzicato => "art.pizzicato",
            Articulation::Tremolo => "art.tremolo",
        }
    }
}

/// Which way the bow moves on a bowed note.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bow {
    Down,
    Up,
}

/// A note's articulation, and its bow when it is bowed (`None`: the instrument alternates).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoteArt {
    pub articulation: Articulation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bow: Option<Bow>,
}

/// Apply `Command::SetNoteArt`: refused whole (nothing changed) for a note that is not in the
/// clip or a note named twice. The inverse holds the old values.
pub(crate) fn apply(
    project: &mut Project,
    track: TrackId,
    clip: ClipId,
    arts: Vec<(NoteId, Option<NoteArt>)>,
) -> Result<Command, CommandError> {
    for (i, (id, _)) in arts.iter().enumerate() {
        if arts[..i].iter().any(|(o, _)| o == id) {
            return Err(CommandError::DuplicateId(id.0));
        }
    }
    let old = edit_shared(project, track, clip, |c: &mut Clip| {
        for (id, _) in &arts {
            if c.note(*id).is_none() {
                return Err(CommandError::NoteNotFound(*id));
            }
        }
        let mut old = Vec::with_capacity(arts.len());
        for (id, a) in &arts {
            if let Some(n) = c.note_mut(*id) {
                old.push((*id, std::mem::replace(&mut n.art, *a)));
            }
        }
        Ok(old)
    })?;
    Ok(Command::SetNoteArt {
        track,
        clip,
        arts: old,
    })
}

/// How far before a note marked [`Articulation::Legato`] the note it follows may end and
/// still be tied to it: an eighth note (ADR 0158's look-ahead).
pub const LEGATO_TIE: crate::time::Ticks = crate::time::Ticks(crate::time::PPQ / 2);

/// The clip's look-ahead for a legato note (ADR 0158; Lindemann's expired US 7,718,885): a
/// note marked [`Articulation::Legato`] that starts while no earlier note sounds, at most
/// [`LEGATO_TIE`] after the last one ended, ties that note to itself — its note-off moves to
/// one tick after the legato note's start — so every instrument hears the overlap a legato
/// line is (Exemplar's scripted legato crossfades into it; a synth glides or keeps its
/// envelope). Done off the audio thread, when the snapshot compiles. Notes without an
/// articulation are never moved: a track without one compiles exactly as before.
///
/// `events` holds each note as an on/off pair, the on at an even index and its off right
/// after it (how `Snapshot::compile` pushes them), in any tick order.
pub fn tie_legato(events: &mut [crate::snapshot::NoteEvent]) {
    let legato = |e: &crate::snapshot::NoteEvent| {
        e.art
            .is_some_and(|a| a.articulation == Articulation::Legato)
    };
    if events.len() < 4 || !events.iter().step_by(2).any(legato) {
        return;
    }
    debug_assert!(events
        .chunks(2)
        .all(|p| p.len() == 2 && p[0].is_on() && !p[1].is_on()));
    let mut order: Vec<usize> = (0..events.len() / 2).map(|k| 2 * k).collect();
    order.sort_by_key(|&i| (events[i].tick, events[i].pitch, events[i].note));
    // The latest note-off among the notes that started before the group at hand: its tick
    // and its index.
    let mut last: Option<(crate::time::Ticks, usize)> = None;
    let mut g = 0;
    while g < order.len() {
        let on = events[order[g]].tick;
        let mut end = g;
        while end < order.len() && events[order[end]].tick == on {
            end += 1;
        }
        for &i in &order[g..end] {
            if !legato(&events[i]) {
                continue;
            }
            if let Some((off, oi)) = last {
                // r42 fix2-model (review 1): never tie a repeated pitch (or the same note
                // again, a looping clip's repetition): its off moved past the new on would
                // end the new note on every receiver that matches offs by key.
                let before = &events[oi - 1];
                let same = before.pitch == events[i].pitch || before.note == events[i].note;
                if !same && off <= on && on - off <= LEGATO_TIE {
                    events[oi].tick = on + crate::time::Ticks(1);
                    last = Some((events[oi].tick, oi));
                }
            }
        }
        for &i in &order[g..end] {
            let off = events[i + 1].tick;
            if last.is_none_or(|(t, _)| off > t) {
                last = Some((off, i + 1));
            }
        }
        g = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NoteId, Pitch};
    use crate::snapshot::NoteEvent;
    use crate::time::Ticks;

    fn pair(on: i64, off: i64, pitch: Pitch, id: u64, art: Option<NoteArt>) -> [NoteEvent; 2] {
        let e = |tick: i64, velocity: u8, art| NoteEvent {
            tick: Ticks(tick),
            pitch,
            velocity,
            note: NoteId(id),
            ordinal: 0,
            release: 0,
            art,
        };
        [e(on, 100, art), e(off, 0, None)]
    }

    const LEG: Option<NoteArt> = Some(NoteArt {
        articulation: Articulation::Legato,
        bow: None,
    });

    #[test]
    fn a_legato_note_ties_the_one_before_it_and_nothing_else_moves() {
        // A drawn end to end, B legato after a small gap, C legato after a long one, D no art.
        let mut ev: Vec<NoteEvent> = [
            pair(0, 960, 60, 1, None),
            pair(1_000, 1_960, 62, 2, LEG),
            pair(5_000, 5_960, 64, 3, LEG),
            pair(6_000, 6_960, 65, 4, None),
        ]
        .concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(1_001), "A now overlaps B by a tick");
        assert_eq!(ev[3].tick, Ticks(1_960), "B is not moved");
        assert_eq!(
            ev[5].tick,
            Ticks(5_960),
            "C's gap is too long: nothing tied"
        );
        assert_eq!(ev[7].tick, Ticks(6_960));
        // Without an articulation nothing moves at all.
        let plain: Vec<NoteEvent> =
            [pair(0, 960, 60, 1, None), pair(1_000, 1_960, 62, 2, None)].concat();
        let mut moved = plain.clone();
        tie_legato(&mut moved);
        assert_eq!(moved, plain);
        // A legato note that already overlaps: nothing moves; a chord of legato notes ties
        // the note before once.
        let mut ev: Vec<NoteEvent> = [
            pair(0, 1_100, 60, 1, None),
            pair(1_000, 1_960, 62, 2, LEG),
            pair(2_000, 2_960, 64, 3, LEG),
            pair(2_000, 2_960, 67, 4, LEG),
        ]
        .concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(1_100));
        assert_eq!(ev[3].tick, Ticks(2_001));
    }

    /// r42 fix2-model (review 1): a legato note on the pitch the note before it played is not
    /// tied — the stream would be `on 60 @1000`, `off 60 @1001`, and a receiver matching
    /// offs by key (MIDI gear, MPE, a hosted plug-in) would end the new note.
    #[test]
    fn a_repeated_pitch_is_never_tied() {
        let mut ev: Vec<NoteEvent> =
            [pair(0, 960, 60, 1, None), pair(1_000, 1_960, 60, 2, LEG)].concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(960), "the first 60 still ends before the second");
        // A looping clip: its repetitions share a note id (and the pitch).
        let mut ev: Vec<NoteEvent> =
            [pair(0, 900, 64, 7, LEG), pair(960, 1_860, 64, 7, LEG)].concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(900), "a repetition is not tied to itself");
        // The same id on another pitch would still be its own voice: never tied either.
        let mut ev: Vec<NoteEvent> =
            [pair(0, 900, 64, 7, None), pair(960, 1_860, 65, 7, LEG)].concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(900));
        // Another pitch is still tied.
        let mut ev: Vec<NoteEvent> =
            [pair(0, 960, 60, 1, None), pair(1_000, 1_960, 62, 2, LEG)].concat();
        tie_legato(&mut ev);
        assert_eq!(ev[1].tick, Ticks(1_001));
    }

    #[test]
    fn the_snapshot_carries_the_art_on_note_ons_without_changing_the_order() {
        use crate::model::Project;
        use crate::snapshot::Snapshot;
        let pizz = Some(NoteArt {
            articulation: Articulation::Pizzicato,
            bow: Some(Bow::Up),
        });
        let song = |arts: [Option<NoteArt>; 3]| {
            let mut p = Project::new("arts");
            let t = p.make_track("Strings");
            p.tracks.push(t);
            let mut c = p.make_clip("line", Ticks::ZERO, Ticks::beats(8));
            for (i, art) in arts.into_iter().enumerate() {
                let mut n = p.make_note(
                    Ticks::beats(i as i64),
                    Ticks::QUARTER,
                    60 + 2 * i as u8,
                    100,
                );
                n.art = art;
                c.notes.push(n);
            }
            p.tracks[0].clips.push(c);
            p
        };
        let plain = Snapshot::compile(&song([None; 3]), 1).tracks[0]
            .events
            .clone();
        let marked = Snapshot::compile(&song([pizz, None, pizz]), 1).tracks[0]
            .events
            .clone();
        assert_eq!(plain.len(), 6);
        let second = plain.iter().filter(|e| e.is_on()).nth(1).map(|e| e.pitch);
        for (a, b) in plain.iter().zip(&marked) {
            assert_eq!(
                (a.tick, a.pitch, a.velocity, a.ordinal),
                (b.tick, b.pitch, b.velocity, b.ordinal)
            );
            assert_eq!(a.art, None);
            if b.is_on() && Some(b.pitch) != second {
                assert_eq!(b.art, pizz);
            } else {
                assert_eq!(b.art, None, "offs and the plain note carry none");
            }
        }
    }
}
