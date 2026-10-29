//! The score as an engraver reads it before anything is placed: parts, staves, bars,
//! voices and events with their written values, ties, tuplets, beams, spelled pitches and
//! accidentals. MusicXML is written from this; the layout draws it.

use cantus_core::time::TimeSignature;

/// Ticks per whole note (`PPQ` × 4).
pub const WHOLE: i64 = cantus_core::time::PPQ * 4;

/// A written value: `log` 0 is a whole note, 1 a half, 2 a quarter … 6 a 64th (−1 a
/// breve); `dots` 0, 1 or 2.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dur {
    pub log: i8,
    pub dots: u8,
}

impl Dur {
    pub const fn new(log: i8, dots: u8) -> Dur {
        Dur { log, dots }
    }

    /// Its length in ticks, written (before a tuplet's ratio).
    pub fn ticks(self) -> i64 {
        let base = if self.log < 0 {
            WHOLE << (-self.log)
        } else {
            WHOLE >> self.log
        };
        let mut t = base;
        let mut add = base;
        for _ in 0..self.dots {
            add /= 2;
            t += add;
        }
        t
    }

    /// The plain, dotted or double-dotted value exactly `ticks` long (written), if any.
    pub fn of(ticks: i64) -> Option<Dur> {
        for log in -1..=7i8 {
            for dots in 0..=2u8 {
                let d = Dur::new(log, dots);
                if d.ticks() == ticks && (WHOLE >> log.max(0)) >> dots > 0 {
                    return Some(d);
                }
            }
        }
        None
    }

    /// How many beams (or flags) it carries: an eighth one, a sixteenth two.
    pub fn beams(self) -> u8 {
        (self.log - 2).max(0) as u8
    }

    /// MusicXML's `<type>`.
    pub fn type_name(self) -> &'static str {
        match self.log {
            -1 => "breve",
            0 => "whole",
            1 => "half",
            2 => "quarter",
            3 => "eighth",
            4 => "16th",
            5 => "32nd",
            6 => "64th",
            _ => "128th",
        }
    }
}

/// A clef as written.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Clef {
    Treble,
    Bass,
    Alto,
    Tenor,
    Treble8vb,
    Treble8va,
    Bass8vb,
}

impl Clef {
    /// The diatonic step (C0 = 0, D0 = 1 … B0 = 6, C1 = 7 …) on the staff's middle line.
    pub fn middle_step(self) -> i32 {
        match self {
            Clef::Treble => 4 * 7 + 6,    // B4
            Clef::Bass => 3 * 7 + 1,      // D3
            Clef::Alto => 4 * 7,          // C4
            Clef::Tenor => 3 * 7 + 5,     // A3
            Clef::Treble8vb => 3 * 7 + 6, // B3
            Clef::Treble8va => 5 * 7 + 6, // B5
            Clef::Bass8vb => 2 * 7 + 1,   // D2
        }
    }

    /// MusicXML's sign, line and octave change.
    pub fn musicxml(self) -> (&'static str, u8, i8) {
        match self {
            Clef::Treble => ("G", 2, 0),
            Clef::Bass => ("F", 4, 0),
            Clef::Alto => ("C", 3, 0),
            Clef::Tenor => ("C", 4, 0),
            Clef::Treble8vb => ("G", 2, -1),
            Clef::Treble8va => ("G", 2, 1),
            Clef::Bass8vb => ("F", 4, -1),
        }
    }
}

/// An octave line over a passage: notes under it are written an octave (or two) from
/// where they sound.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ottava {
    /// The first and last event under it: `(bar, voice, event)`.
    pub from: (usize, usize, usize),
    pub to: (usize, usize, usize),
    /// +1: 8va (written an octave down); −1: 8vb (written an octave up).
    pub shift: i8,
}

/// A spelled pitch: a letter (0 = C … 6 = B), its alteration and octave.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Spelled {
    pub letter: u8,
    pub alter: i8,
    pub octave: i8,
}

impl Spelled {
    /// The diatonic step: C0 = 0, B0 = 6, C1 = 7.
    pub fn step(self) -> i32 {
        self.octave as i32 * 7 + self.letter as i32
    }

    pub fn letter_name(self) -> char {
        b"CDEFGAB"[self.letter as usize] as char
    }

    /// The MIDI pitch it names.
    pub fn midi(self) -> i32 {
        const PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
        (self.octave as i32 + 1) * 12 + PC[self.letter as usize] + self.alter as i32
    }
}

/// An accidental drawn before a notehead.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Accidental {
    /// −2 … +2; 0 is a natural.
    pub alter: i8,
    /// A courtesy (cautionary) accidental, drawn in parentheses.
    pub courtesy: bool,
}

/// One notehead of a chord.
#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    /// The sounding pitch (what the track plays).
    pub pitch: u8,
    /// The pitch as written (transposition and octave line applied), spelled.
    pub spelled: Spelled,
    /// Staff position: 0 on the middle line, +1 a step up (a space), +2 the next line.
    pub pos: i32,
    pub accidental: Option<Accidental>,
    /// Tied to the same pitch in the next event of this voice.
    pub tie: bool,
    /// The continuation of a tie (no accidental, no new attack).
    pub tied_from: bool,
    /// The notes it shows (one note, or a note repeated by a looping clip).
    pub id: u64,
    pub velocity: u8,
}

/// A stem's direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stem {
    Up,
    Down,
}

/// A dynamic marking.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dynamic {
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
}

impl Dynamic {
    pub const ALL: [Dynamic; 8] = [
        Dynamic::Ppp,
        Dynamic::Pp,
        Dynamic::P,
        Dynamic::Mp,
        Dynamic::Mf,
        Dynamic::F,
        Dynamic::Ff,
        Dynamic::Fff,
    ];

    /// The marking a velocity reads as.
    pub fn of_velocity(v: u8) -> Dynamic {
        match v {
            0..=23 => Dynamic::Ppp,
            24..=39 => Dynamic::Pp,
            40..=55 => Dynamic::P,
            56..=71 => Dynamic::Mp,
            72..=87 => Dynamic::Mf,
            88..=103 => Dynamic::F,
            104..=117 => Dynamic::Ff,
            _ => Dynamic::Fff,
        }
    }

    /// MusicXML's element name.
    pub fn name(self) -> &'static str {
        match self {
            Dynamic::Ppp => "ppp",
            Dynamic::Pp => "pp",
            Dynamic::P => "p",
            Dynamic::Mp => "mp",
            Dynamic::Mf => "mf",
            Dynamic::F => "f",
            Dynamic::Ff => "ff",
            Dynamic::Fff => "fff",
        }
    }
}

/// What an event is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Rest {
        /// A whole-bar rest: centred in the bar whatever its metre.
        whole_bar: bool,
    },
    /// Heads from the lowest to the highest.
    Chord(Vec<Head>),
}

/// A tuplet over consecutive events of one voice.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tuplet {
    /// `actual` notes in the time of `normal`.
    pub actual: u8,
    pub normal: u8,
    /// The written value of one of its units (for MusicXML's `<normal-type>`).
    pub unit: Dur,
    pub first: usize,
    pub last: usize,
    /// Whether it needs a bracket (its events are not one beam).
    pub bracket: bool,
}

/// A beam's state at one event, per level (MusicXML's begin / continue / end / hooks).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BeamMark {
    Begin,
    Continue,
    End,
    HookForward,
    HookBackward,
}

/// One rhythmic event of a voice.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Bar-relative start and length in ticks, as heard.
    pub start: i64,
    pub len: i64,
    /// Its written value (inside a tuplet, the value before the ratio).
    pub dur: Dur,
    pub kind: Kind,
    /// Index into the voice's tuplets.
    pub tuplet: Option<usize>,
    pub stem: Stem,
    /// Beam marks per level (empty: no beam; a flag if it is an eighth or shorter).
    pub beams: Vec<BeamMark>,
    /// The beam group it belongs to (index into the voice's groups).
    pub group: Option<usize>,
    pub dynamic: Option<Dynamic>,
}

impl Event {
    pub fn end(&self) -> i64 {
        self.start + self.len
    }

    pub fn heads(&self) -> &[Head] {
        match &self.kind {
            Kind::Chord(h) => h,
            Kind::Rest { .. } => &[],
        }
    }

    pub fn is_rest(&self) -> bool {
        matches!(self.kind, Kind::Rest { .. })
    }
}

/// One voice of one staff in one bar.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Voice {
    pub events: Vec<Event>,
    pub tuplets: Vec<Tuplet>,
    /// Beam groups: first and last event index, and the stem direction they share.
    pub groups: Vec<(usize, usize)>,
}

/// One staff's bar.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StaffBar {
    /// Voice 1 (stems up when a second voice sounds), then voice 2.
    pub voices: Vec<Voice>,
}

/// One staff of a part.
#[derive(Clone, Debug, PartialEq)]
pub struct Staff {
    pub clef: Clef,
    pub bars: Vec<StaffBar>,
    pub ottavas: Vec<Ottava>,
}

/// A part: one track's music.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    /// One staff, or two (a grand staff: upper, lower).
    pub staves: Vec<Staff>,
    /// Written = sounding + this, in semitones (MusicXML's `<transpose>`).
    pub transpose: i8,
    /// The key signature it is written in (the song's, transposed with it).
    pub key: KeySig,
}

/// A tempo marking.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TempoMark {
    /// The beat it counts: a quarter, or a dotted quarter in compound metres.
    pub unit: Dur,
    /// Beats per minute of `unit`, rounded to what is written.
    pub per_minute: f64,
    /// The tempo in quarter notes per minute (MusicXML's `<sound tempo>`).
    pub quarter_bpm: f64,
    /// A gradual change starts here: `Some(true)` accelerando, `Some(false)` ritardando.
    pub ramp: Option<bool>,
}

/// One bar of the score, shared by every part.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    /// Absolute start tick and length.
    pub start: i64,
    pub len: i64,
    pub sig: TimeSignature,
    /// The time signature is written at this bar (the first, and every change).
    pub show_sig: bool,
    pub tempo: Option<TempoMark>,
    /// Its number as printed (the first bar of the music is 1).
    pub number: u32,
}

/// A key's mode as MusicXML 4 names it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum KeyMode {
    #[default]
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    /// A scale no mode names (whole tone, diminished, chromatic…).
    None,
}

impl KeyMode {
    pub const ALL: [KeyMode; 8] = [
        KeyMode::Major,
        KeyMode::Minor,
        KeyMode::Dorian,
        KeyMode::Phrygian,
        KeyMode::Lydian,
        KeyMode::Mixolydian,
        KeyMode::Locrian,
        KeyMode::None,
    ];

    /// MusicXML's `<mode>`.
    pub fn musicxml(self) -> &'static str {
        match self {
            KeyMode::Major => "major",
            KeyMode::Minor => "minor",
            KeyMode::Dorian => "dorian",
            KeyMode::Phrygian => "phrygian",
            KeyMode::Lydian => "lydian",
            KeyMode::Mixolydian => "mixolydian",
            KeyMode::Locrian => "locrian",
            KeyMode::None => "none",
        }
    }

    /// From MusicXML's `<mode>` (`ionian` is major, `aeolian` minor).
    pub fn from_musicxml(s: &str) -> KeyMode {
        match s {
            "ionian" => KeyMode::Major,
            "aeolian" => KeyMode::Minor,
            _ => KeyMode::ALL
                .into_iter()
                .find(|m| m.musicxml() == s)
                .unwrap_or(KeyMode::Major),
        }
    }
}

/// The key signature: sharps (+) or flats (−), whether the key is minor-like (for
/// spelling), and its mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct KeySig {
    pub fifths: i8,
    pub minor: bool,
    pub mode: KeyMode,
}

/// A whole score.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub title: String,
    pub composer: String,
    pub key: KeySig,
    pub bars: Vec<Bar>,
    pub parts: Vec<Part>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_dots() {
        assert_eq!(Dur::new(2, 0).ticks(), 960);
        assert_eq!(Dur::new(2, 1).ticks(), 1440);
        assert_eq!(Dur::new(1, 2).ticks(), 1920 + 960 + 480);
        assert_eq!(Dur::of(1440), Some(Dur::new(2, 1)));
        assert_eq!(Dur::of(720), Some(Dur::new(3, 1)));
        assert_eq!(Dur::of(1200), None);
        assert_eq!(Dur::of(7680), Some(Dur::new(-1, 0)));
    }

    #[test]
    fn spelled_pitches() {
        let c4 = Spelled {
            letter: 0,
            alter: 0,
            octave: 4,
        };
        assert_eq!(c4.midi(), 60);
        let bb3 = Spelled {
            letter: 6,
            alter: -1,
            octave: 3,
        };
        assert_eq!(bb3.midi(), 58);
    }
}
