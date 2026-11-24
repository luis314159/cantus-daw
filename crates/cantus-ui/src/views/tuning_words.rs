//! The tuning's words (ADR 0155 §5): the one-line summary the top bar shows only when the
//! song is not in the default tuning — "Werckmeister III in Eb · A415" — with keys named as
//! the song's key names them (`scale::ROOT_NAMES`: the fonts have no sharp or flat sign), and
//! the names the window's rows show. r42:tuning-face

use crate::i18n::{t, tf};
use cantus_core::scale::{Key, ROOT_NAMES};
use cantus_core::tuning::{ScaleDegree, TrackTuning, Tuning, TuningSystem, A4_DEFAULT_HZ};

/// A pitch class's name, as the song's key spells it ("Eb", "F#").
pub fn key_name(pc: u8) -> &'static str {
    ROOT_NAMES[usize::from(pc % 12)]
}

/// The natural notes' letters and pitch classes.
const LETTERS: [(char, u8); 7] = [
    ('C', 0),
    ('D', 2),
    ('E', 4),
    ('F', 5),
    ('G', 7),
    ('A', 9),
    ('B', 11),
];

/// The note `semitones` above `root` (spelt as [`key_name`] spells it), named as the interval
/// of `letters` letter steps from it: a major third (4, 2) above E is G#, above Ab is C —
/// sharps in sharp keys, flats in flat keys, `#` and `b` as the fonts have them.
pub fn spell_above(root: u8, semitones: u8, letters: usize) -> String {
    let name = key_name(root);
    let first = name.chars().next().unwrap_or('C');
    let at = LETTERS.iter().position(|(l, _)| *l == first).unwrap_or(0);
    let (letter, natural) = LETTERS[(at + letters) % 7];
    let target = (root % 12 + semitones % 12) % 12;
    // The accidental the letter needs to reach the target, from −2 to +2 semitones.
    let shift = (i16::from(target) - i16::from(natural) + 18).rem_euclid(12) - 6;
    let accidental = match shift {
        -2 => "bb",
        -1 => "b",
        1 => "#",
        2 => "##",
        _ => "",
    };
    format!("{letter}{accidental}")
}

/// The major third above `root`, spelt as a third (E–G#, Ab–C).
pub fn major_third(root: u8) -> String {
    spell_above(root, 4, 2)
}

/// The perfect fifth above `root`, spelt as a fifth (E–B, F#–C#, Ab–Eb).
pub fn fifth_above(root: u8) -> String {
    spell_above(root, 7, 4)
}

/// A key with its octave, its note named as the song's key names it: "C4" (60), "Eb4",
/// "A4" (69).
pub fn key_with_octave(key: u8) -> String {
    format!("{}{}", key_name(key), i32::from(key) / 12 - 1)
}

/// The diapason as the summary writes it: "A415", "A442.5".
pub fn a4_short(hz: f64) -> String {
    tf("tuning.summary.a4", &[("hz", &hz_text(hz))])
}

/// A frequency without a needless decimal: "415", "442.5", "430.54".
pub fn hz_text(hz: f64) -> String {
    let r = (hz * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else if ((r * 10.0) - (r * 10.0).round()).abs() < 1e-9 {
        format!("{r:.1}")
    } else {
        format!("{r:.2}")
    }
}

/// The system's name alone: "Equal temperament", "Werckmeister III in Eb", the scale's own
/// description ("31-EDO"), "MTS-ESP".
pub fn system_name(tu: &Tuning, key: Option<Key>) -> String {
    let tu = super::effective(tu, key);
    match &tu.system {
        TuningSystem::Equal => t("tuning.system.equal_name"),
        TuningSystem::Temperament { id, key, .. } => tf(
            "tuning.summary.temper",
            &[("name", &t(id.name_key())), ("key", &key_name(*key))],
        ),
        TuningSystem::Scale(s) => scale_name(s),
        TuningSystem::External => t("tuning.system.external_name"),
    }
}

/// A Scala scale's name: its description's first words, or "Scala scale, 31 notes" — a
/// scale of Cantus's own (Chorda's *Slightly out*, stored by a fixed name) in the
/// interface's language.
pub fn scale_name(s: &cantus_core::tuning::ScalaScale) -> String {
    let d = s.description.trim();
    if d == super::chorda::SLIGHTLY_OUT_ID {
        return t("tuning.chorda.p.slightly_out");
    }
    if d.is_empty() {
        tf(
            "tuning.summary.scale_notes",
            &[("n", &s.degrees.len().to_string())],
        )
    } else {
        // The summary is one line: a long description stops at its first sentence.
        let first = d.split(['\n', ';']).next().unwrap_or(d).trim();
        first.chars().take(48).collect()
    }
}

/// The summary line: the system (when not equal) and the diapason (when not 440), joined by
/// " · ". Empty at the default.
pub fn summary_of(tu: &Tuning, key: Option<Key>) -> String {
    if tu.is_default() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    if !tu.system.is_equal() {
        parts.push(system_name(tu, key));
    } else {
        parts.push(t("tuning.summary.equal"));
    }
    if tu.a4_hz != A4_DEFAULT_HZ || !tu.system.is_equal() {
        parts.push(a4_short(tu.a4_hz));
    }
    if tu.map.is_some() && !matches!(tu.system, TuningSystem::Scale(_)) {
        parts.push(t("tuning.summary.mapped"));
    }
    parts.join(" \u{b7} ")
}

/// The project's summary for the top bar, or `None` when the song is in the default tuning.
pub fn project_summary(p: &cantus_core::Project) -> Option<String> {
    (!p.tuning.is_default()).then(|| summary_of(&p.tuning, p.key))
}

/// What the summary is made from, without making it (no allocation): the top bar's widths
/// are measured again only when it changes.
pub fn summary_key(p: &cantus_core::Project) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let tu = &p.tuning;
    tu.a4_hz.to_bits().hash(&mut h);
    tu.map.is_some().hash(&mut h);
    match &tu.system {
        TuningSystem::Equal => 0u8.hash(&mut h),
        TuningSystem::Temperament {
            id,
            key,
            follow_song_key,
        } => {
            (1u8, *id, *key, *follow_song_key).hash(&mut h);
            if *follow_song_key {
                p.key.map(|k| k.root).hash(&mut h);
            }
        }
        TuningSystem::Scale(s) => (2u8, s.description.as_str(), s.degrees.len()).hash(&mut h),
        TuningSystem::External => 3u8.hash(&mut h),
    }
    h.finish()
}

/// A track's override in the inspector's row, whose value is a narrow slot (98 pt): the
/// short words — "Project's", "Equal" — or its own tuning's summary (r42 fix3-suite: "Follows
/// the project" was elided there). The menu it opens names them whole.
pub fn track_row_text(mode: &TrackTuning, key: Option<Key>) -> String {
    match mode {
        TrackTuning::Follow => t("tuning.track.follow_row"),
        TrackTuning::Equal => t("tuning.track.equal_row"),
        own => track_mode_text(own, key),
    }
}

/// A track's override, as a sentence names it: "Follows the project", "Equal
/// temperament", or its own tuning's summary.
pub fn track_mode_text(mode: &TrackTuning, key: Option<Key>) -> String {
    match mode {
        TrackTuning::Follow => t("tuning.track.follow"),
        TrackTuning::Equal => t("tuning.track.equal"),
        TrackTuning::Own { tuning } => {
            let s = summary_of(tuning, key);
            if s.is_empty() {
                t("tuning.track.own_equal")
            } else {
                s
            }
        }
    }
}

/// A degree as its file wrote it: "3/2", "701.955".
pub fn degree_written(d: &ScaleDegree) -> String {
    match *d {
        // As the file wrote it: the shortest decimal that reads back to the same number.
        ScaleDegree::Cents(c) => format!("{c}"),
        // A ratio stays a ratio ("2/1"): a bare "2" would read as a number of cents.
        ScaleDegree::Ratio(n, d) => format!("{n}/{d}"),
    }
}

/// A degree in cents above 1/1.
pub fn degree_cents(d: &ScaleDegree) -> f64 {
    match *d {
        ScaleDegree::Cents(c) => c,
        ScaleDegree::Ratio(n, d) => 1200.0 * cantus_dsp::math::log2(n as f64 / d as f64),
    }
}

/// Cents with their sign: "+3.91", "−11.73", "0.00" (the minus sign, not a hyphen).
pub fn signed_cents(c: f64) -> String {
    let c = if c.abs() < 0.005 { 0.0 } else { c };
    if c > 0.0 {
        format!("+{c:.2}")
    } else if c < 0.0 {
        format!("\u{2212}{:.2}", -c)
    } else {
        "0.00".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every root's major third is a third — two letters up, four semitones up — with no
    /// double accidental (r42 integration: "E–Ab" read as a diminished fourth).
    #[test]
    fn a_major_third_is_spelt_as_a_third_over_every_root() {
        let pc = |n: &str| {
            let mut ch = n.chars();
            let l = ch.next().unwrap();
            let base = LETTERS.iter().find(|(x, _)| *x == l).unwrap().1 as i16;
            let acc: i16 = ch.map(|c| if c == '#' { 1 } else { -1 }).sum();
            (base + acc).rem_euclid(12) as u8
        };
        let letter = |n: &str| LETTERS.iter().position(|(x, _)| n.starts_with(*x)).unwrap();
        for root in 0..12u8 {
            let third = major_third(root);
            assert_eq!(pc(&third), (root + 4) % 12, "{} {third}", key_name(root));
            assert_eq!(
                (letter(key_name(root)) + 2) % 7,
                letter(&third),
                "{} {third}",
                key_name(root)
            );
            assert!(!third.contains("##") && !third.contains("bb"), "{third}");
        }
        assert_eq!(major_third(4), "G#");
        assert_eq!(major_third(8), "C");
        assert_eq!(major_third(1), "E#");
        assert_eq!(major_third(3), "G");
        assert_eq!(major_third(11), "D#");
    }

    /// r42 fix-ui (review ui#5): every root's fifth is a fifth — four letters up, seven
    /// semitones up, no double accidental (B–F#, never B–Gb; F#–C#, never F#–Db).
    #[test]
    fn a_fifth_is_spelt_as_a_fifth_over_every_root() {
        let letter = |n: &str| LETTERS.iter().position(|(x, _)| n.starts_with(*x)).unwrap();
        let pc = |n: &str| {
            let base = LETTERS[letter(n)].1 as i16;
            let acc: i16 = n[1..].chars().map(|c| if c == '#' { 1 } else { -1 }).sum();
            (base + acc).rem_euclid(12) as u8
        };
        for root in 0..12u8 {
            let f = fifth_above(root);
            assert_eq!(pc(&f), (root + 7) % 12, "{} {f}", key_name(root));
            assert_eq!((letter(key_name(root)) + 4) % 7, letter(&f), "{f}");
            assert!(!f.contains("##") && !f.contains("bb"), "{f}");
        }
        assert_eq!(fifth_above(11), "F#");
        assert_eq!(fifth_above(6), "C#");
        assert_eq!(fifth_above(8), "Eb");
        assert_eq!(key_with_octave(60), "C4");
        assert_eq!(key_with_octave(63), "Eb4");
        assert_eq!(key_with_octave(69), "A4");
    }
    use cantus_core::tuning::Temperament;

    #[test]
    fn the_default_says_nothing_and_a_temperament_says_its_key_and_pitch() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        assert_eq!(summary_of(&Tuning::default(), None), "");
        let w = Tuning {
            a4_hz: 415.0,
            system: TuningSystem::Temperament {
                id: Temperament::WerckmeisterIii,
                key: 3,
                follow_song_key: false,
            },
            map: None,
        };
        assert_eq!(summary_of(&w, None), "Werckmeister III in Eb \u{b7} A415");
        let a432 = Tuning {
            a4_hz: 432.0,
            ..Tuning::default()
        };
        assert_eq!(summary_of(&a432, None), "Equal \u{b7} A432");
    }

    #[test]
    fn following_the_songs_key_names_the_songs_tonic() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let w = Tuning {
            a4_hz: 440.0,
            system: TuningSystem::Temperament {
                id: Temperament::Vallotti,
                key: 0,
                follow_song_key: true,
            },
            map: None,
        };
        let d_minor = Key::new(2, cantus_core::scale::ScaleKind::Minor);
        assert_eq!(summary_of(&w, Some(d_minor)), "Vallotti in D \u{b7} A440");
        assert_eq!(summary_of(&w, None), "Vallotti in C \u{b7} A440");
    }

    #[test]
    fn numbers_read_as_a_musician_writes_them() {
        assert_eq!(hz_text(415.0), "415");
        assert_eq!(hz_text(442.5), "442.5");
        assert_eq!(signed_cents(-11.7301), "\u{2212}11.73");
        assert_eq!(signed_cents(0.001), "0.00");
        assert_eq!(degree_written(&ScaleDegree::Ratio(3, 2)), "3/2");
        // r42 fix-ui (review ui#20): the octave reads as a ratio, never as a bare "2".
        assert_eq!(degree_written(&ScaleDegree::Ratio(2, 1)), "2/1");
        assert!((degree_cents(&ScaleDegree::Ratio(3, 2)) - 701.955).abs() < 1e-3);
    }
}
