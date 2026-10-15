//! What the pitch editor makes from the notes: MIDI notes (the melody to an instrument)
//! and a double (a second voice a third or a fifth above, from the same take).

use super::{PitchEdit, PitchNote};

/// A detected note as MIDI, in the edited take's output frames.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MidiNote {
    pub start: u64,
    pub end: u64,
    pub pitch: u8,
    pub velocity: u8,
}

/// The velocity a note's level gives: −48 dBFS and below is 1, 0 dBFS is 127.
pub fn velocity(level_db: f32) -> u8 {
    let v = ((level_db + 48.0) / 48.0 * 126.0 + 1.0).round();
    v.clamp(1.0, 127.0) as u8
}

/// The notes as they sound after the edits: each on the semitone nearest its centre, where
/// it plays, at a velocity from its level and gain.
pub fn notes(edit: &PitchEdit) -> Vec<MidiNote> {
    edit.notes
        .iter()
        .map(|n: &PitchNote| MidiNote {
            start: n.start,
            end: n.end,
            pitch: n.out_centre().round().clamp(0.0, 127.0) as u8,
            velocity: velocity(n.level_db + n.gain_db),
        })
        .collect()
}

/// The interval a double sings above the lead.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Interval {
    Third,
    Fifth,
}

impl Interval {
    pub const ALL: [Interval; 2] = [Interval::Third, Interval::Fifth];

    /// Scale degrees above (in a seven-note scale), and semitones in a chromatic one.
    fn steps(self) -> (i32, f32) {
        match self {
            Interval::Third => (2, 4.0),
            Interval::Fifth => (4, 7.0),
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Interval::Third => "pitch.double.third",
            Interval::Fifth => "pitch.double.fifth",
        }
    }
}

/// A double of `edit`: every note moved up by `interval` in the key (a diatonic third or
/// fifth; a major third or a fifth when the key is chromatic), every other edit kept.
pub fn double(edit: &PitchEdit, interval: Interval) -> PitchEdit {
    let mut out = edit.clone();
    out.bypass = false;
    let (steps, semis) = interval.steps();
    let chromatic = edit.key.scale().degrees().len() == 12;
    for n in &mut out.notes {
        let from = n.pitch;
        let to = if chromatic {
            from + semis
        } else {
            // From the scale note the lead sits on, keeping a fractional (Alt) offset.
            let base = edit.key.nearest(from);
            edit.key.degrees_from(base, steps) + (from - base)
        };
        n.pitch = to.clamp(n.home() - super::MAX_MOVE, n.home() + super::MAX_MOVE);
    }
    out
}
