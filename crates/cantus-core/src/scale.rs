//! Keys and scales (round 31, ADR 0069): the project's key, what the piano roll lights,
//! folds and snaps to, and what the note effects' *Scale* and diatonic *Chord* read.
//!
//! A [`Key`] is a root pitch class (0 = C … 11 = B) and a [`ScaleKind`]. Everything here
//! is integer arithmetic on MIDI pitches: no table is built at run time, nothing
//! allocates, so the engine's note effects call the same functions the UI does.

use crate::model::Pitch;
use serde::{Deserialize, Serialize};

/// A scale, as the semitones above its root that belong to it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScaleKind {
    #[default]
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
    WholeTone,
    DiminishedHalfWhole,
    DiminishedWholeHalf,
    PhrygianDominant,
    HungarianMinor,
    Japanese,
    Chromatic,
}

impl ScaleKind {
    /// Every scale, in the order the menus list them.
    pub const ALL: [ScaleKind; 19] = [
        ScaleKind::Major,
        ScaleKind::Minor,
        ScaleKind::HarmonicMinor,
        ScaleKind::MelodicMinor,
        ScaleKind::Dorian,
        ScaleKind::Phrygian,
        ScaleKind::Lydian,
        ScaleKind::Mixolydian,
        ScaleKind::Locrian,
        ScaleKind::MajorPentatonic,
        ScaleKind::MinorPentatonic,
        ScaleKind::Blues,
        ScaleKind::WholeTone,
        ScaleKind::DiminishedHalfWhole,
        ScaleKind::DiminishedWholeHalf,
        ScaleKind::PhrygianDominant,
        ScaleKind::HungarianMinor,
        ScaleKind::Japanese,
        ScaleKind::Chromatic,
    ];

    /// The semitones above the root, ascending, starting at 0.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ScaleKind::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleKind::Minor => &[0, 2, 3, 5, 7, 8, 10],
            ScaleKind::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleKind::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            ScaleKind::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleKind::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleKind::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleKind::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleKind::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            ScaleKind::MajorPentatonic => &[0, 2, 4, 7, 9],
            ScaleKind::MinorPentatonic => &[0, 3, 5, 7, 10],
            ScaleKind::Blues => &[0, 3, 5, 6, 7, 10],
            ScaleKind::WholeTone => &[0, 2, 4, 6, 8, 10],
            ScaleKind::DiminishedHalfWhole => &[0, 1, 3, 4, 6, 7, 9, 10],
            ScaleKind::DiminishedWholeHalf => &[0, 2, 3, 5, 6, 8, 9, 11],
            ScaleKind::PhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
            ScaleKind::HungarianMinor => &[0, 2, 3, 6, 7, 8, 11],
            ScaleKind::Japanese => &[0, 1, 5, 7, 8],
            ScaleKind::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    /// The i18n key of its name (`notefx` area).
    pub fn label_key(self) -> &'static str {
        match self {
            ScaleKind::Major => "notefx.scale.major",
            ScaleKind::Minor => "notefx.scale.minor",
            ScaleKind::HarmonicMinor => "notefx.scale.harmonic_minor",
            ScaleKind::MelodicMinor => "notefx.scale.melodic_minor",
            ScaleKind::Dorian => "notefx.scale.dorian",
            ScaleKind::Phrygian => "notefx.scale.phrygian",
            ScaleKind::Lydian => "notefx.scale.lydian",
            ScaleKind::Mixolydian => "notefx.scale.mixolydian",
            ScaleKind::Locrian => "notefx.scale.locrian",
            ScaleKind::MajorPentatonic => "notefx.scale.major_pentatonic",
            ScaleKind::MinorPentatonic => "notefx.scale.minor_pentatonic",
            ScaleKind::Blues => "notefx.scale.blues",
            ScaleKind::WholeTone => "notefx.scale.whole_tone",
            ScaleKind::DiminishedHalfWhole => "notefx.scale.dim_hw",
            ScaleKind::DiminishedWholeHalf => "notefx.scale.dim_wh",
            ScaleKind::PhrygianDominant => "notefx.scale.phrygian_dominant",
            ScaleKind::HungarianMinor => "notefx.scale.hungarian_minor",
            ScaleKind::Japanese => "notefx.scale.japanese",
            ScaleKind::Chromatic => "notefx.scale.chromatic",
        }
    }

    /// Its place in [`Self::ALL`] (what a stepped parameter stores).
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The scale at `i` of [`Self::ALL`], clamped.
    pub fn from_index(i: usize) -> ScaleKind {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    /// The pitch classes it holds above its root, as a 12-bit mask (bit 0 = the root).
    pub fn mask(self) -> u16 {
        self.intervals().iter().fold(0u16, |m, i| m | 1 << i)
    }
}

/// The names of the twelve roots, as a key is usually spelled (the flats a key signature
/// writes: E♭ major, not D♯ major).
pub const ROOT_NAMES: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// A key: a root pitch class and a scale.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Key {
    /// 0 = C … 11 = B.
    pub root: u8,
    pub scale: ScaleKind,
}

impl Key {
    pub fn new(root: u8, scale: ScaleKind) -> Key {
        Key {
            root: root % 12,
            scale,
        }
    }

    /// Whether the key is well formed (a hand-edited file's root may not be).
    pub fn is_valid(&self) -> bool {
        self.root < 12
    }

    /// The pitch classes of the key, as a 12-bit mask over C … B.
    pub fn mask(&self) -> u16 {
        let m = self.scale.mask() as u32;
        let r = (self.root % 12) as u32;
        (((m << r) | (m >> (12 - r))) & 0xFFF) as u16
    }

    /// Whether `pitch` belongs to the key.
    #[inline]
    pub fn contains(&self, pitch: Pitch) -> bool {
        self.mask() & (1 << (pitch % 12)) != 0
    }

    /// Whether `pitch` is the key's root (any octave).
    #[inline]
    pub fn is_root(&self, pitch: Pitch) -> bool {
        pitch % 12 == self.root % 12
    }

    /// The nearest pitch of the key to `pitch` (itself if it belongs); ties go down, the
    /// way a sung note falls back into the key. Clamped to 0…127.
    pub fn snap(&self, pitch: Pitch) -> Pitch {
        if self.contains(pitch) {
            return pitch;
        }
        for d in 1..12i16 {
            let down = pitch as i16 - d;
            if (0..=127).contains(&down) && self.contains(down as u8) {
                return down as u8;
            }
            let up = pitch as i16 + d;
            if (0..=127).contains(&up) && self.contains(up as u8) {
                return up as u8;
            }
        }
        pitch
    }

    /// The pitch of the key nearest `pitch` on the side of `dir` (> 0 up, < 0 down,
    /// 0 nearest): what a drag snaps to, so the note follows the hand.
    pub fn snap_toward(&self, pitch: Pitch, dir: i32) -> Pitch {
        if dir == 0 || self.contains(pitch) {
            return self.snap(pitch);
        }
        let step = if dir > 0 { 1i16 } else { -1 };
        let mut p = pitch as i16;
        for _ in 0..12 {
            p += step;
            if !(0..=127).contains(&p) {
                break;
            }
            if self.contains(p as u8) {
                return p as u8;
            }
        }
        self.snap(pitch)
    }

    /// `pitch` moved `steps` degrees of the key (from its nearest degree): a transposition
    /// that stays in the key. `None` past 0…127.
    pub fn step(&self, pitch: Pitch, steps: i32) -> Option<Pitch> {
        let mut p = self.snap(pitch) as i32;
        let dir = steps.signum();
        for _ in 0..steps.unsigned_abs() {
            loop {
                p += dir;
                if !(0..=127).contains(&p) {
                    return None;
                }
                if self.contains(p as u8) {
                    break;
                }
            }
        }
        Some(p as u8)
    }

    /// The degree (0-based) of `pitch` in the key, if it belongs.
    pub fn degree(&self, pitch: Pitch) -> Option<usize> {
        let rel = (pitch as i32 - self.root as i32).rem_euclid(12) as u8;
        self.scale.intervals().iter().position(|i| *i == rel)
    }

    /// The diatonic chord whose root is `pitch` (snapped into the key first): `voices`
    /// notes stacked in thirds of the key (every other degree), as semitones above the
    /// snapped root — a triad with 3, a seventh with 4. At most 6 voices.
    pub fn diatonic(&self, pitch: Pitch, voices: usize, out: &mut [i16; 6]) -> usize {
        let root = self.snap(pitch);
        let ivs = self.scale.intervals();
        let n = ivs.len();
        let Some(d) = self.degree(root) else {
            out[0] = 0;
            return 1;
        };
        let voices = voices.clamp(1, 6);
        for (v, o) in out.iter_mut().enumerate().take(voices) {
            let k = d + 2 * v;
            let semis = ivs[k % n] as i16 + 12 * (k / n) as i16 - ivs[d] as i16;
            *o = semis;
        }
        voices
    }

    /// "C major", in English: the file's and the logs' wording (the UI names the scale
    /// through [`ScaleKind::label_key`]).
    pub fn root_name(&self) -> &'static str {
        ROOT_NAMES[(self.root % 12) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_major_holds_the_white_keys() {
        let k = Key::new(0, ScaleKind::Major);
        let white: Vec<u8> = (60..72).filter(|p| k.contains(*p)).collect();
        assert_eq!(white, [60, 62, 64, 65, 67, 69, 71]);
        assert!(k.is_root(48) && !k.is_root(62));
    }

    #[test]
    fn a_key_rotates_its_scale() {
        // D minor: D E F G A Bb C.
        let k = Key::new(2, ScaleKind::Minor);
        let held: Vec<u8> = (60..72).filter(|p| k.contains(*p)).collect();
        assert_eq!(held, [60, 62, 64, 65, 67, 69, 70]);
        // B major wraps past C: B C# D# E F# G# A#.
        let b = Key::new(11, ScaleKind::Major);
        assert_eq!(b.mask().count_ones(), 7);
        assert!(b.contains(61) && b.contains(71) && !b.contains(60));
    }

    #[test]
    fn snapping_goes_to_the_nearest_and_ties_go_down() {
        let k = Key::new(0, ScaleKind::Major);
        assert_eq!(k.snap(61), 60); // C# between C and D: down.
        assert_eq!(k.snap(66), 65); // F# between F and G: down.
        assert_eq!(k.snap(64), 64);
        let pent = Key::new(0, ScaleKind::MajorPentatonic); // C D E G A
        assert_eq!(pent.snap(65), 64); // F: E is 1 away, G 2.
        assert_eq!(pent.snap(66), 67); // F#: G is 1 away.
        assert_eq!(k.snap_toward(61, 1), 62);
        assert_eq!(k.snap_toward(61, -1), 60);
        assert_eq!(k.snap(0), 0);
        // G in C# major: F# below is as near as G# above, and the edge never leaves 0…127.
        assert_eq!(Key::new(1, ScaleKind::Major).snap(127), 126);
    }

    #[test]
    fn stepping_stays_in_the_key() {
        let k = Key::new(0, ScaleKind::Major);
        assert_eq!(k.step(60, 1), Some(62));
        assert_eq!(k.step(64, 1), Some(65));
        assert_eq!(k.step(60, -1), Some(59));
        assert_eq!(k.step(60, 7), Some(72));
        assert_eq!(k.step(61, 1), Some(62)); // C# snaps to C first.
        assert_eq!(k.step(127, 1), None);
    }

    #[test]
    fn diatonic_chords_are_stacked_thirds_of_the_key() {
        let k = Key::new(0, ScaleKind::Major);
        let mut out = [0i16; 6];
        // I: C E G; ii: D F A; viiø7: B D F A (a half-diminished seventh).
        assert_eq!(k.diatonic(60, 3, &mut out), 3);
        assert_eq!(&out[..3], &[0, 4, 7]);
        k.diatonic(62, 3, &mut out);
        assert_eq!(&out[..3], &[0, 3, 7]);
        assert_eq!(k.diatonic(71, 4, &mut out), 4);
        assert_eq!(&out[..4], &[0, 3, 6, 10]);
        // V7 in C: G B D F.
        k.diatonic(67, 4, &mut out);
        assert_eq!(&out[..4], &[0, 4, 7, 10]);
    }

    #[test]
    fn every_scale_starts_on_its_root_and_ascends() {
        for s in ScaleKind::ALL {
            let iv = s.intervals();
            assert_eq!(iv[0], 0, "{s:?}");
            assert!(iv.windows(2).all(|w| w[0] < w[1] && w[1] < 12), "{s:?}");
            assert_eq!(ScaleKind::from_index(s.index()), s);
        }
    }
}
