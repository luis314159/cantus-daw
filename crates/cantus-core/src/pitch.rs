//! **The pitch editor's model** (round 28, ADR 0054): the notes of a recorded voice and
//! what the musician did to them, stored with the audio clip (`AudioRegion::pitch`,
//! schema 27).
//!
//! The analysis of a take (the pitch track, voicing, epochs — `cantus_dsp::pitch::analyse`)
//! is run offline and cached beside the peaks; **its result that edits refer to — the
//! notes, where each one starts and ends in the source, and its sung centre — is stored
//! here** (ADR 0023's principle): an improved analyser never moves an old edit.
//!
//! Every position is in **source frames** at the source file's own rate ([`PitchEdit::rate`]),
//! so a clip's window, its slip, a split of the clip or a second clip over the same file
//! all read the edited take where they read the file. The engine plays a render of the
//! edits (`cantus_engine::pitch`), as long as the source, made off the audio thread.
//!
//! A note's output pitch, at a source time `t` inside it (all in semitones):
//!
//! ```text
//! home  = round(sung)                       the semitone the voice sang nearest
//! shift = (pitch − home)                    moved to another note (snap or free)
//!       + centre · (home − sung)            the centre pulled toward the note, 0 … 1
//!       + (drift − 1) · d(t)                the slow wander, scaled (1 = as sung)
//!       + (vibrato − 1) · v(t)              the vibrato, scaled (1 = as sung)
//! ```
//!
//! where the sung pitch is `sung + d(t) + v(t)`: `d` is the note's pitch smoothed over a
//! vibrato cycle (the drift) and `v` what is left (the vibrato) — [`curves`]. Every field
//! at its default makes `shift` exactly 0: the render is the source, bit for bit.

use serde::{Deserialize, Serialize};

#[path = "pitch_curves.rs"]
pub mod curves;
#[path = "pitch_edit.rs"]
pub mod edit;
#[path = "pitch_midi.rs"]
pub mod midi;
#[cfg(test)]
#[path = "pitch_tests.rs"]
mod tests;

/// The widest a note may be moved, semitones either way.
pub const MAX_MOVE: f32 = 24.0;
/// The formant shift's range, semitones either way.
pub const MAX_FORMANT: f32 = 12.0;
/// The drift and vibrato scales' top (0 flattens, 1 is as sung, 2 doubles).
pub const MAX_SCALE: f32 = 2.0;
/// A note's gain range, dB.
pub const MIN_GAIN_DB: f32 = -48.0;
pub const MAX_GAIN_DB: f32 = 12.0;
/// The shortest note, seconds (a split or a timing edit never makes one shorter).
pub const MIN_NOTE_S: f64 = 0.020;

/// The key the scale snap and *correct pitch* use: detected from the take, editable.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PitchKey {
    /// The tonic's pitch class, 0 = C … 11 = B.
    pub tonic: u8,
    /// The scale, by index in `cantus_dsp::tonus::Scale::ALL` (0 chromatic, 1 major,
    /// 2 minor …) — the same list Tonus offers, so the two speak one language.
    pub scale: u8,
}

impl PitchKey {
    pub fn scale(self) -> cantus_dsp::tonus::Scale {
        cantus_dsp::tonus::Scale::from_index(self.scale as usize)
    }

    /// Whether the semitone `midi` (an integer pitch) is in the key's scale.
    pub fn contains(self, midi: i32) -> bool {
        self.scale()
            .contains(self.tonic % 12, midi.rem_euclid(12) as u8)
    }

    /// The scale note nearest `m` (fractional MIDI); ties go down.
    pub fn nearest(self, m: f32) -> f32 {
        let base = m.round() as i32;
        for d in 0..12 {
            let (lo, hi) = (base - d, base + d);
            let (lo_in, hi_in) = (self.contains(lo), self.contains(hi));
            match (lo_in, hi_in) {
                (true, true) => {
                    return if (m - lo as f32).abs() <= (hi as f32 - m).abs() {
                        lo as f32
                    } else {
                        hi as f32
                    }
                }
                (true, false) => return lo as f32,
                (false, true) => return hi as f32,
                _ => {}
            }
        }
        m.round()
    }

    /// One step of the key from `p`, `dir` up (+1) or down (−1): the next note of the key
    /// beyond `p`'s semitone, its cents kept — so a note sung off the key steps onto it.
    pub fn step(self, p: f32, dir: i32) -> f32 {
        let dir = dir.signum();
        if dir == 0 {
            return p;
        }
        let frac = p - p.round();
        let mut n = p.round() as i32 + dir;
        for _ in 0..12 {
            if self.contains(n) {
                break;
            }
            n += dir;
        }
        n as f32 + frac
    }

    /// How many steps of the key lie from `from`'s semitone to `to` (a note of the key),
    /// signed.
    pub fn steps_to(self, from: f32, to: f32) -> i32 {
        let (a, b) = (from.round() as i32, to.round() as i32);
        if a == b {
            return 0;
        }
        let dir = (b - a).signum();
        let mut n = a as f32;
        let mut k = 0;
        while (n.round() as i32 - b) * dir < 0 && k < 128 {
            n = self.step(n, dir);
            k += 1;
        }
        k * dir
    }

    /// The scale note `steps` scale degrees from the scale note nearest `m` (a third is
    /// two steps, a fifth four, in a seven-note scale).
    pub fn degrees_from(self, m: f32, steps: i32) -> f32 {
        let mut n = self.nearest(m) as i32;
        let dir = steps.signum();
        for _ in 0..steps.abs() {
            n += dir;
            while !self.contains(n) {
                n += dir;
            }
        }
        n as f32
    }
}

/// One note of the take: where the analysis found it, and what was done to it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PitchNote {
    /// Unique within its clip's edit; a split keeps it on the left half.
    pub id: u32,
    /// **The segmentation** (stored, never re-derived): the note's source frames,
    /// half-open.
    pub src_start: u64,
    pub src_end: u64,
    /// Its sung centre, MIDI (fractional), as analysed.
    pub sung: f32,
    /// Its level, dBFS RMS, as analysed (MIDI out's velocity).
    #[serde(default)]
    pub level_db: f32,
    /// The note it sits on (MIDI; fractional with Alt). Default: `round(sung)`.
    pub pitch: f32,
    /// The pitch centre: how far the sung centre is pulled onto `pitch`, 0 … 1.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub centre: f32,
    /// The drift's scale: 1 as sung, 0 flat, 2 doubled.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub drift: f32,
    /// The vibrato's scale: 1 as sung, 0 none, 2 doubled.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub vibrato: f32,
    /// The formant shift, semitones (0 keeps the voice's formants).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub formant: f32,
    /// The note's gain, dB.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gain_db: f32,
    /// **The timing**: where the note plays, in output frames (the edited take is as long
    /// as the source). Default: where it was sung.
    pub start: u64,
    pub end: u64,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_false(v: &bool) -> bool {
    !*v
}

impl PitchNote {
    /// A note as analysed: untouched.
    pub fn sung(id: u32, src_start: u64, src_end: u64, sung: f32, level_db: f32) -> Self {
        PitchNote {
            id,
            src_start,
            src_end,
            sung,
            level_db,
            pitch: sung.round(),
            centre: 0.0,
            drift: 1.0,
            vibrato: 1.0,
            formant: 0.0,
            gain_db: 0.0,
            start: src_start,
            end: src_end,
        }
    }

    /// The semitone the voice sang nearest.
    pub fn home(&self) -> f32 {
        self.sung.round()
    }

    /// The constant part of the shift (semitones): the move and the pulled centre.
    pub fn offset(&self) -> f32 {
        let home = self.home();
        (self.pitch - home) + self.centre * (home - self.sung)
    }

    /// The note's centre as it sounds after the edits (MIDI).
    pub fn out_centre(&self) -> f32 {
        if self.offset() == 0.0 {
            self.sung
        } else {
            self.sung + self.offset()
        }
    }

    /// Whether the note's pitch is touched (its shift is 0 everywhere otherwise).
    pub fn pitch_edited(&self) -> bool {
        self.offset() != 0.0 || self.drift != 1.0 || self.vibrato != 1.0
    }

    /// Whether the note's timing is touched.
    pub fn moved(&self) -> bool {
        self.start != self.src_start || self.end != self.src_end
    }

    /// Whether anything at all is done to the note.
    pub fn edited(&self) -> bool {
        self.pitch_edited() || self.moved() || self.formant != 0.0 || self.gain_db != 0.0
    }

    /// The same note with every edit undone (its segmentation kept).
    pub fn reset(&self) -> Self {
        PitchNote::sung(
            self.id,
            self.src_start,
            self.src_end,
            self.sung,
            self.level_db,
        )
    }
}

/// An audio clip's pitch edits (`AudioRegion::pitch`, schema 27).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PitchEdit {
    /// The rate every frame here counts at: the source file's own.
    pub rate: u32,
    /// The source's length in frames at that rate, when the notes were found.
    pub frames: u64,
    /// The analyser that found the notes (its version, for the record: the notes stay
    /// as they are when a better one ships).
    pub analyser: u32,
    pub key: PitchKey,
    /// In source order; neither their source ranges nor their output ranges overlap.
    pub notes: Vec<PitchNote>,
    /// Compare with the original (A/B): the clip plays the source untouched.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bypass: bool,
}

impl PitchEdit {
    /// The note with `id`.
    pub fn note(&self, id: u32) -> Option<&PitchNote> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn note_mut(&mut self, id: u32) -> Option<&mut PitchNote> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    /// An id no note has.
    pub fn next_id(&self) -> u32 {
        self.notes.iter().map(|n| n.id + 1).max().unwrap_or(0)
    }

    /// Whether anything at all is done to the take: when not, the clip plays its source.
    pub fn edited(&self) -> bool {
        self.notes.iter().any(PitchNote::edited)
    }

    /// Whether the clip plays a render (edits, and not compared with the original).
    pub fn audible(&self) -> bool {
        !self.bypass && self.edited()
    }

    /// The shortest note, frames.
    pub fn min_len(&self) -> u64 {
        ((self.rate as f64 * MIN_NOTE_S).ceil() as u64).max(1)
    }

    /// Whether the edit is one the engine can render: a rate, notes in order whose source
    /// and output ranges are inside the take and do not overlap, every value finite and in
    /// range. A command refuses anything else.
    pub fn is_valid(&self) -> bool {
        if self.rate == 0 || self.key.tonic > 11 || self.key.scale as usize >= 13 {
            return false;
        }
        let mut ids: Vec<u32> = self.notes.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        if ids.windows(2).any(|w| w[0] == w[1]) {
            return false;
        }
        let ok_note = |n: &PitchNote| {
            n.src_start < n.src_end
                && n.src_end <= self.frames
                && n.start < n.end
                && n.end <= self.frames
                && n.sung.is_finite()
                && n.level_db.is_finite()
                && (n.pitch - n.home()).abs() <= MAX_MOVE
                && (0.0..=1.0).contains(&n.centre)
                && (0.0..=MAX_SCALE).contains(&n.drift)
                && (0.0..=MAX_SCALE).contains(&n.vibrato)
                && n.formant.abs() <= MAX_FORMANT
                && (MIN_GAIN_DB..=MAX_GAIN_DB).contains(&n.gain_db)
        };
        if !self.notes.iter().all(ok_note) {
            return false;
        }
        self.notes.windows(2).all(|w| {
            let (a, b) = (&w[0], &w[1]);
            b.src_start >= a.src_end
                && b.start >= a.end
                // A gap the source has may shrink, never vanish: the time map is a
                // function (ADR 0054 §3).
                && (b.src_start == a.src_end || b.start > a.end)
        })
    }
}

/// What a pitch edit did, for the undo menu (`cmd.pitch.<id>`) and for merging a drag
/// into one step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PitchOp {
    /// The analysed notes stored, or the whole edit removed.
    Notes,
    Pitch,
    Centre,
    Drift,
    Vibrato,
    Formant,
    Gain,
    Timing,
    Split,
    Join,
    CorrectPitch,
    CorrectDrift,
    Quantise,
    Key,
    Compare,
    Reset,
}

impl PitchOp {
    pub const ALL: [PitchOp; 16] = [
        PitchOp::Notes,
        PitchOp::Pitch,
        PitchOp::Centre,
        PitchOp::Drift,
        PitchOp::Vibrato,
        PitchOp::Formant,
        PitchOp::Gain,
        PitchOp::Timing,
        PitchOp::Split,
        PitchOp::Join,
        PitchOp::CorrectPitch,
        PitchOp::CorrectDrift,
        PitchOp::Quantise,
        PitchOp::Key,
        PitchOp::Compare,
        PitchOp::Reset,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            PitchOp::Notes => "cmd.pitch.notes",
            PitchOp::Pitch => "cmd.pitch.pitch",
            PitchOp::Centre => "cmd.pitch.centre",
            PitchOp::Drift => "cmd.pitch.drift",
            PitchOp::Vibrato => "cmd.pitch.vibrato",
            PitchOp::Formant => "cmd.pitch.formant",
            PitchOp::Gain => "cmd.pitch.gain",
            PitchOp::Timing => "cmd.pitch.timing",
            PitchOp::Split => "cmd.pitch.split",
            PitchOp::Join => "cmd.pitch.join",
            PitchOp::CorrectPitch => "cmd.pitch.correct_pitch",
            PitchOp::CorrectDrift => "cmd.pitch.correct_drift",
            PitchOp::Quantise => "cmd.pitch.quantise",
            PitchOp::Key => "cmd.pitch.key",
            PitchOp::Compare => "cmd.pitch.compare",
            PitchOp::Reset => "cmd.pitch.reset",
        }
    }

    /// Whether a run of this edit on one clip merges into one undo step (a drag, a
    /// slider): the edits that set a value; splits, joins and the macros never do.
    pub fn merges(self) -> bool {
        matches!(
            self,
            PitchOp::Pitch
                | PitchOp::Centre
                | PitchOp::Drift
                | PitchOp::Vibrato
                | PitchOp::Formant
                | PitchOp::Gain
                | PitchOp::Timing
                | PitchOp::CorrectPitch
                | PitchOp::CorrectDrift
        )
    }
}
