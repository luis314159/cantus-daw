//! Spelling: the key signature a song's key writes, and each pitch's letter and
//! accidental — diatonic notes as the key spells them, chromatic ones by their degree in
//! the key and by the line they move in (up a semitone with a sharp, down with a flat).

use crate::model::{KeyMode, KeySig, Spelled};
use cantus_core::scale::{Key, ScaleKind};

/// The natural pitch class of each letter C … B.
const NATURAL: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Sharps are added F C G D A E B; flats B E A D G C F (letters 0 = C … 6 = B).
const SHARP_ORDER: [u8; 7] = [3, 0, 4, 1, 5, 2, 6];
const FLAT_ORDER: [u8; 7] = [6, 2, 5, 1, 4, 0, 3];

/// The fifths of each major key by its tonic's pitch class (D♭, E♭, G♭… as flats; F♯ and
/// B as sharps, as `scale::ROOT_NAMES` names them).
const MAJOR_FIFTHS: [i8; 12] = [0, -5, 2, -3, 4, -1, 6, 1, -4, 3, -2, 5];

/// The key signature a key writes: a mode's is its parent major's; a minor or minor-like
/// scale's its relative major's; the symmetric scales (whole tone, diminished, chromatic)
/// their root's major.
pub fn key_signature(key: Option<Key>) -> KeySig {
    let Some(key) = key else {
        return KeySig::default();
    };
    let r = key.root as i32 % 12;
    use ScaleKind::*;
    let (major_root, minor, mode) = match key.scale {
        Major | MajorPentatonic => (r, false, KeyMode::Major),
        WholeTone | DiminishedHalfWhole | DiminishedWholeHalf | Chromatic => {
            (r, false, KeyMode::None)
        }
        Minor | HarmonicMinor | MelodicMinor | MinorPentatonic | Blues | HungarianMinor
        | Japanese => (r + 3, true, KeyMode::Minor),
        Dorian => (r - 2, true, KeyMode::Dorian),
        Phrygian => (r - 4, true, KeyMode::Phrygian),
        Lydian => (r - 5, false, KeyMode::Lydian),
        Mixolydian => (r - 7, false, KeyMode::Mixolydian),
        Locrian => (r + 1, true, KeyMode::Locrian),
        // The fifth mode of the harmonic minor a fifth below: its relative major.
        PhrygianDominant => (r + 8, false, KeyMode::None),
    };
    KeySig {
        fifths: MAJOR_FIFTHS[major_root.rem_euclid(12) as usize],
        minor,
        mode,
    }
}

/// The alteration the key signature gives each letter.
pub fn key_alters(sig: KeySig) -> [i8; 7] {
    let mut a = [0i8; 7];
    let n = sig.fifths.unsigned_abs().min(7) as usize;
    if sig.fifths > 0 {
        for &l in &SHARP_ORDER[..n] {
            a[l as usize] = 1;
        }
    } else {
        for &l in &FLAT_ORDER[..n] {
            a[l as usize] = -1;
        }
    }
    a
}

/// Which way a chromatic note moves on: up to the next note a semitone higher, down to
/// one a semitone lower, or neither.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Motion {
    Up,
    Down,
    None,
}

/// Spells pitches in a key.
#[derive(Copy, Clone, Debug)]
pub struct Speller {
    alters: [i8; 7],
    /// The key's tonic pitch class (the mode's own root, not the signature's major).
    tonic: i32,
    minor: bool,
    /// The signature has sharps (or none): a chromatic note with no rule of its own leans
    /// to a sharp; with flats, to a flat.
    sharps: bool,
}

impl Speller {
    pub fn new(key: Option<Key>, sig: KeySig) -> Speller {
        Speller {
            alters: key_alters(sig),
            tonic: key.map_or(0, |k| k.root as i32 % 12),
            minor: sig.minor,
            sharps: sig.fifths >= 0,
        }
    }

    /// The same key written `semitones` higher (a transposing instrument's part).
    pub fn transposed(key: Option<Key>, semitones: i32) -> (Speller, KeySig) {
        let k = key.map(|k| Key::new(((k.root as i32 + semitones).rem_euclid(12)) as u8, k.scale));
        let sig = key_signature(k);
        (Speller::new(k, sig), sig)
    }

    fn make(pitch: i32, letter: i32, alter: i32) -> Spelled {
        // The octave follows the letter: B♯3 is C4's pitch, C♭4 is B3's.
        let natural = NATURAL[letter as usize];
        let octave = (pitch - natural - alter).div_euclid(12) - 1;
        Spelled {
            letter: letter as u8,
            alter: alter as i8,
            octave: octave as i8,
        }
    }

    /// Spell MIDI `pitch`, moving on as `motion` says.
    pub fn spell(&self, pitch: u8, motion: Motion) -> Spelled {
        let p = pitch as i32;
        let pc = p.rem_euclid(12);
        for l in 0..7 {
            let a = self.alters[l as usize] as i32;
            if (NATURAL[l as usize] + a).rem_euclid(12) == pc {
                return Self::make(p, l, a);
            }
        }
        // Chromatic: the letter below raised, or the letter above lowered.
        let mut raise = None;
        let mut lower = None;
        for l in 0..7 {
            let a = self.alters[l as usize] as i32;
            let key_pc = NATURAL[l as usize] + a;
            if (key_pc + 1).rem_euclid(12) == pc {
                raise = Some((l, a + 1));
            }
            if (key_pc - 1).rem_euclid(12) == pc {
                lower = Some((l, a - 1));
            }
        }
        let pick = |c: Option<(i32, i32)>| c.filter(|(_, a)| a.abs() <= 2);
        let (raise, lower) = (pick(raise), pick(lower));
        // A natural on one of the signature's own letters reads before a sharp or flat on
        // its neighbour (review, finding 6: F♮ in C Lydian, not E♯), and a single
        // accidental before a double one: the smaller alteration first.
        if let (Some(r), Some(l)) = (raise, lower) {
            if r.1.abs() != l.1.abs() {
                let c = if r.1.abs() < l.1.abs() { r } else { l };
                return Self::make(p, c.0, c.1);
            }
        }
        let prefer_raise = match motion {
            Motion::Up => true,
            Motion::Down => false,
            Motion::None => {
                let d = (pc - self.tonic).rem_euclid(12);
                if self.minor {
                    // ♯3 (Picardy), ♯4, ♯6 and ♯7 (melodic and harmonic minor) raise; ♭2
                    // (Neapolitan), ♭3, ♭6 and ♭7 (Dorian's B♭, Phrygian's…) lower.
                    match d {
                        4 | 6 | 9 | 11 => true,
                        1 | 3 | 8 | 10 => false,
                        _ => self.sharps,
                    }
                } else {
                    // ♯1, ♯4 raise; ♭3, ♭6, ♭7 lower — the chromatic notes a major key most
                    // often meets.
                    match d {
                        1 | 6 => true,
                        3 | 8 | 10 => false,
                        _ => self.sharps,
                    }
                }
            }
        };
        let chosen = match (raise, lower) {
            (Some(r), Some(l)) => {
                if prefer_raise {
                    r
                } else {
                    l
                }
            }
            (Some(c), None) | (None, Some(c)) => c,
            (None, None) => (0, pc),
        };
        Self::make(p, chosen.0, chosen.1)
    }

    /// The alteration the signature gives `letter`.
    pub fn key_alter(&self, letter: u8) -> i8 {
        self.alters[letter as usize % 7]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::KeyMode;

    fn name(s: Spelled) -> String {
        let acc = match s.alter {
            -2 => "bb",
            -1 => "b",
            1 => "#",
            2 => "x",
            _ => "",
        };
        format!("{}{}{}", s.letter_name(), acc, s.octave)
    }

    /// Review, finding 6: a mode's chromatic notes are spelled by its signature's letters —
    /// F natural in C Lydian (not E♯), B♭ in D Dorian (not A♯).
    #[test]
    fn modes_spell_their_chromatic_notes_by_the_signature() {
        let lyd = Some(Key::new(0, ScaleKind::Lydian));
        let s = Speller::new(lyd, key_signature(lyd));
        assert_eq!(name(s.spell(65, Motion::None)), "F4");
        let dor = Some(Key::new(2, ScaleKind::Dorian));
        let s = Speller::new(dor, key_signature(dor));
        assert_eq!(name(s.spell(70, Motion::None)), "Bb4");
        assert_eq!(key_signature(dor).mode, KeyMode::Dorian);
        assert_eq!(key_signature(lyd).mode, KeyMode::Lydian);
    }

    #[test]
    fn signatures() {
        let k = |r, s| key_signature(Some(Key::new(r, s)));
        assert_eq!(k(0, ScaleKind::Major).fifths, 0);
        assert_eq!(
            k(9, ScaleKind::Minor),
            KeySig {
                fifths: 0,
                minor: true,
                mode: KeyMode::Minor,
            }
        );
        assert_eq!(k(2, ScaleKind::Minor).fifths, -1);
        assert_eq!(k(3, ScaleKind::Major).fifths, -3);
        assert_eq!(k(2, ScaleKind::Dorian).fifths, 0);
        assert_eq!(k(7, ScaleKind::Mixolydian).fifths, 0);
        assert_eq!(k(6, ScaleKind::Major).fifths, 6);
        assert_eq!(key_signature(None).fifths, 0);
    }

    #[test]
    fn spellings_follow_the_key() {
        let key = Some(Key::new(3, ScaleKind::Major)); // E-flat major
        let s = Speller::new(key, key_signature(key));
        assert_eq!(name(s.spell(63, Motion::None)), "Eb4");
        assert_eq!(name(s.spell(70, Motion::None)), "Bb4");
        // A chromatic E natural and F sharp in E-flat.
        assert_eq!(name(s.spell(64, Motion::None)), "E4");
        let a = Some(Key::new(9, ScaleKind::HarmonicMinor));
        let s = Speller::new(a, key_signature(a));
        assert_eq!(name(s.spell(68, Motion::None)), "G#4");
        let c = Some(Key::new(0, ScaleKind::Major));
        let s = Speller::new(c, key_signature(c));
        assert_eq!(name(s.spell(70, Motion::None)), "Bb4");
        assert_eq!(name(s.spell(66, Motion::None)), "F#4");
        assert_eq!(name(s.spell(61, Motion::Down)), "Db4");
        assert_eq!(name(s.spell(61, Motion::Up)), "C#4");
        assert_eq!(name(s.spell(60, Motion::None)), "C4");
        let fs = Some(Key::new(6, ScaleKind::Major));
        let s = Speller::new(fs, key_signature(fs));
        assert_eq!(name(s.spell(65, Motion::None)), "E#4");
        assert_eq!(name(s.spell(71, Motion::None)), "B4");
    }
}
