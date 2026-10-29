//! How a track's part is written as a score (schema 45, ADR 0075): the clef or *Auto*, one
//! staff or a grand staff and where its hands split, a transposition for a transposing
//! instrument, dynamics read from velocity, the reading grid and the part's name.
//!
//! Plain data on [`Track::score`](crate::model::Track::score): the engraver
//! (`cantus-score`) reads it; nothing here touches what a track plays. Every field's
//! default is what a track without settings meant, so a project before schema 45 opens
//! unchanged and saves nothing new.

use serde::{Deserialize, Serialize};

/// The clef a part is written in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClefChoice {
    /// The clef that needs the fewest ledger lines for the part's range.
    #[default]
    Auto,
    Treble,
    Bass,
    Alto,
    Tenor,
    /// A treble clef an octave down (guitar, tenor voice).
    Treble8vb,
    /// A treble clef an octave up (piccolo, glockenspiel).
    Treble8va,
    /// A bass clef an octave down (double bass, sub bass).
    Bass8vb,
}

impl ClefChoice {
    pub const ALL: [ClefChoice; 8] = [
        ClefChoice::Auto,
        ClefChoice::Treble,
        ClefChoice::Bass,
        ClefChoice::Alto,
        ClefChoice::Tenor,
        ClefChoice::Treble8vb,
        ClefChoice::Treble8va,
        ClefChoice::Bass8vb,
    ];

    /// The `score.clef.*` string naming it.
    pub fn label_key(self) -> &'static str {
        match self {
            ClefChoice::Auto => "score.clef.auto",
            ClefChoice::Treble => "score.clef.treble",
            ClefChoice::Bass => "score.clef.bass",
            ClefChoice::Alto => "score.clef.alto",
            ClefChoice::Tenor => "score.clef.tenor",
            ClefChoice::Treble8vb => "score.clef.treble_8vb",
            ClefChoice::Treble8va => "score.clef.treble_8va",
            ClefChoice::Bass8vb => "score.clef.bass_8vb",
        }
    }
}

/// One staff or a grand staff.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StaffChoice {
    /// A grand staff when the part spans both hands' ranges at once, else one staff.
    #[default]
    Auto,
    Single,
    Grand,
}

impl StaffChoice {
    pub const ALL: [StaffChoice; 3] = [StaffChoice::Auto, StaffChoice::Single, StaffChoice::Grand];

    pub fn label_key(self) -> &'static str {
        match self {
            StaffChoice::Auto => "score.staves.auto",
            StaffChoice::Single => "score.staves.single",
            StaffChoice::Grand => "score.staves.grand",
        }
    }
}

/// The grid a part is read on: what the notes are rounded to *for reading* (the notes
/// themselves never move).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReadingGrid {
    /// Per beat, straight values or triplets (and sextuplets, quintuplets), whichever the
    /// notes fit.
    #[default]
    Auto,
    Eighth,
    Sixteenth,
    ThirtySecond,
    EighthTriplet,
    SixteenthTriplet,
}

impl ReadingGrid {
    pub const ALL: [ReadingGrid; 6] = [
        ReadingGrid::Auto,
        ReadingGrid::Eighth,
        ReadingGrid::Sixteenth,
        ReadingGrid::ThirtySecond,
        ReadingGrid::EighthTriplet,
        ReadingGrid::SixteenthTriplet,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            ReadingGrid::Auto => "score.grid.auto",
            ReadingGrid::Eighth => "score.grid.eighth",
            ReadingGrid::Sixteenth => "score.grid.sixteenth",
            ReadingGrid::ThirtySecond => "score.grid.thirty_second",
            ReadingGrid::EighthTriplet => "score.grid.eighth_triplet",
            ReadingGrid::SixteenthTriplet => "score.grid.sixteenth_triplet",
        }
    }
}

/// The widest transposition a part may be written at, in semitones either way.
pub const MAX_TRANSPOSE: i8 = 36;
/// The longest part name kept.
pub const MAX_NAME: usize = 120;

/// A track's score settings. `Default` is every older track's: everything automatic, no
/// transposition, no dynamics, the track's own name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ScoreSettings {
    #[serde(default, skip_serializing_if = "is_default")]
    pub clef: ClefChoice,
    #[serde(default, skip_serializing_if = "is_default")]
    pub staves: StaffChoice,
    /// Where a grand staff splits: `None` follows the hands; `Some(p)` puts pitches `p` and
    /// above on the upper staff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<u8>,
    /// Written = sounding + `transpose` semitones (a B♭ clarinet is +2, a horn in F +7).
    #[serde(default, skip_serializing_if = "is_default")]
    pub transpose: i8,
    /// Dynamics (pp … ff) written from the notes' velocities.
    #[serde(default, skip_serializing_if = "is_default")]
    pub dynamics: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub grid: ReadingGrid,
    /// The part's name on the score; `None` is the track's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl ScoreSettings {
    pub fn is_default(&self) -> bool {
        *self == ScoreSettings::default()
    }

    /// Whether a command may set these (a hand-edited file's are repaired on load).
    pub fn is_valid(&self) -> bool {
        self.transpose.abs() <= MAX_TRANSPOSE
            && self.split.is_none_or(|p| p <= 127)
            && self
                .name
                .as_ref()
                .is_none_or(|n| n.chars().count() <= MAX_NAME && !n.trim().is_empty())
    }

    /// The nearest valid settings (a hand edit's): the transposition clamped, a split
    /// past the keyboard and an empty or overlong name dropped or cut.
    pub fn repaired(&self) -> ScoreSettings {
        let mut s = self.clone();
        s.transpose = s.transpose.clamp(-MAX_TRANSPOSE, MAX_TRANSPOSE);
        s.split = s.split.filter(|p| *p <= 127);
        s.name = s
            .name
            .map(|n| n.trim().chars().take(MAX_NAME).collect::<String>())
            .filter(|n| !n.is_empty());
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_serialises_to_nothing_and_reads_back() {
        let s = ScoreSettings::default();
        assert_eq!(serde_json::to_string(&s).unwrap(), "{}");
        let back: ScoreSettings = serde_json::from_str("{}").unwrap();
        assert!(back.is_default());
    }

    #[test]
    fn invalid_settings_are_refused_and_repaired() {
        let bad = ScoreSettings {
            transpose: 100,
            name: Some("   ".into()),
            ..Default::default()
        };
        assert!(!bad.is_valid());
        let r = bad.repaired();
        assert!(r.is_valid());
        assert_eq!(r.transpose, MAX_TRANSPOSE);
        assert_eq!(r.name, None);
    }
}
