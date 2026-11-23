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
            &[("name", &t(id.name_key())), ("key", key_name(*key))],
        ),
        TuningSystem::Scale(s) => scale_name(s),
        TuningSystem::External => t("tuning.system.external_name"),
    }
}

/// A Scala scale's name: its description's first words, or "Scala scale, 31 notes".
pub fn scale_name(s: &cantus_core::tuning::ScalaScale) -> String {
    let d = s.description.trim();
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

/// A track's override, as its inspector row reads it: "Follows the project", "Equal
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
        ScaleDegree::Cents(c) => format!("{c:.3}"),
        ScaleDegree::Ratio(n, d) if d == 1 => format!("{n}"),
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
        assert!((degree_cents(&ScaleDegree::Ratio(3, 2)) - 701.955).abs() < 1e-3);
    }
}
