//! Harmony for tools that write and check music (round 34, ADR 0082): chord symbols, a
//! chart of chords over the song's bars, voicings that move smoothly from chord to chord,
//! parts laid over a chart, and the **music linter** — a cheap static analysis of notes
//! against the chords and the key, the check a reviewer did by ear in round 28 (the demo's
//! Fm7 held under E♭/G).
//!
//! Everything here is plain arithmetic on pitch classes and ticks: no allocation beyond the
//! results, nothing the audio thread calls. Positions are [`Ticks`] from the song's start;
//! bars are counted by the [`TempoMap`](crate::TempoMap) the caller passes, so every metre
//! change is honoured.

#[path = "harmony_chart.rs"]
pub mod chart;
#[path = "harmony_chord.rs"]
pub mod chord;
#[path = "harmony_lint.rs"]
pub mod lint;
#[path = "harmony_part.rs"]
pub mod part;
#[path = "harmony_voice.rs"]
pub mod voice;

#[cfg(test)]
#[path = "harmony_tests.rs"]
mod tests;

pub use chart::{Chart, ChartChord, ChartError};
pub use chord::{parse_chord, spell, Chord, ChordError};
pub use lint::{lint, Finding, Issue, LintNote, LintOptions, Severity};
pub use part::{lay_part, PartNote, PartSpec, Pattern};
pub use voice::{voice_chart, VoiceSpec};

/// The names of the twelve pitch classes as a chart spells them by default (flats, as
/// [`crate::scale::ROOT_NAMES`]).
pub const PC_NAMES: [&str; 12] = crate::scale::ROOT_NAMES;

/// A MIDI pitch's name with its octave, middle C (60) being `C4`: `Eb3`, `G#5`.
pub fn pitch_name(pitch: u8) -> String {
    format!(
        "{}{}",
        PC_NAMES[(pitch % 12) as usize],
        (pitch as i32) / 12 - 1
    )
}

/// A pitch from its name: a letter, any number of `#`/`b` (or `♯`/`♭`), an octave
/// (`-1`…`9`, middle C is `C4`). `None` outside 0…127 or for anything else.
pub fn parse_pitch(text: &str) -> Option<u8> {
    let (letter, shift, rest) = chord::parse_name(text.trim())?;
    let octave: i32 = rest.trim().parse().ok()?;
    // `Cb4` is B3 and `B#3` is C4: the letter's octave, then the accidental.
    let p = (octave + 1) * 12 + letter + shift;
    (0..=127).contains(&p).then_some(p as u8)
}

/// Whether two pitch classes are a semitone apart (a minor second or a major seventh,
/// by inversion a minor ninth): the interval that clashes when both sound.
pub fn semitone_apart(a: u8, b: u8) -> bool {
    let d = (a as i32 - b as i32).rem_euclid(12);
    d == 1 || d == 11
}
