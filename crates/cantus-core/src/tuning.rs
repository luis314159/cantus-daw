//! The project's tuning (round 42, ADR 0155): one tuning every pitched instrument follows,
//! with an override per track. Owned by the `tuning` builder; the contract declares the
//! types, the default and the one function every unit reads, [`Tuning::offsets`].
//!
//! A [`Tuning`] is a diapason (`a4_hz`), a [`TuningSystem`] (equal temperament, a named
//! temperament rotated to a key, a Scala scale, or the MTS-ESP master's table) and an
//! optional [`KeyboardMap`] (a Scala `.kbm`). Units never see it: they read a
//! [`TuningTable`] — the semitones each MIDI key sits from 12-TET at A440 — resolved off the
//! audio thread. At the default every entry is `0.0`, so a unit that adds `table[key]` to
//! its pitch keeps its bits (`x + 0.0 == x`).
//!
//! The formats (Scala `.scl`/`.kbm`, the MIDI Tuning Standard) live in `tuning_scala` and
//! `tuning_mts`; the temperaments' tables in `tuning_temper` (the `tuning` builder's files).

use serde::{Deserialize, Serialize};

/// The reference pitch every older song has: A4 = 440 Hz.
pub const A4_DEFAULT_HZ: f64 = 440.0;
/// The diapason's range (ADR 0155): from a low baroque pitch to a bright modern one.
pub const A4_MIN_HZ: f64 = 380.0;
pub const A4_MAX_HZ: f64 = 480.0;
/// The diapason presets the window offers.
pub const A4_PRESETS_HZ: [f64; 6] = [415.0, 430.0, 432.0, 440.0, 442.0, 443.0];

/// A project's or a track's tuning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// The frequency of A4 (MIDI key 69) — or, with a keyboard map, of its reference key.
    #[serde(default = "a4_default")]
    pub a4_hz: f64,
    #[serde(default, skip_serializing_if = "TuningSystem::is_equal")]
    pub system: TuningSystem,
    /// A Scala keyboard map (`.kbm`): which key plays which degree. `None` maps the scale's
    /// degree 0 to MIDI key 60 and repeats it every period (Scala's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<KeyboardMap>,
}

fn a4_default() -> f64 {
    A4_DEFAULT_HZ
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            a4_hz: A4_DEFAULT_HZ,
            system: TuningSystem::Equal,
            map: None,
        }
    }
}

impl Tuning {
    /// Every older song's tuning: 12-TET at 440 Hz, no map. Not written.
    pub fn is_default(&self) -> bool {
        *self == Tuning::default()
    }

    /// The semitones each MIDI key sits from 12-TET at A440 — what every unit reads (ADR
    /// 0155). Exactly `0.0` everywhere at the default.
    ///
    /// **Contract:** complete for [`TuningSystem::Equal`] with any diapason; the other
    /// systems return the diapason's table until the `tuning` builder fills them
    /// (`tuning_temper`, `tuning_scala`). `External` takes the table its caller read from
    /// the MTS-ESP master ([`Tuning::offsets_with`]).
    pub fn offsets(&self) -> TuningTable {
        self.offsets_with(None)
    }

    /// [`Tuning::offsets`], with the MTS-ESP master's table for `External` (`None`: no
    /// master — equal temperament at the diapason).
    pub fn offsets_with(&self, external: Option<&TuningTable>) -> TuningTable {
        if self.is_default() {
            return TuningTable::EQUAL;
        }
        if let (TuningSystem::External, Some(t)) = (&self.system, external) {
            return *t;
        }
        let shift = diapason_semitones(self.a4_hz);
        // r42:contract — the temperaments, Scala scales and keyboard maps are the
        // `tuning` builder's (ADR 0155); until then they sound as equal temperament.
        TuningTable([shift; 128])
    }
}

/// `12·log2(a4 / 440)`, deterministically (`cantus_dsp::math`), `0.0` at 440.
pub fn diapason_semitones(a4_hz: f64) -> f32 {
    if a4_hz == A4_DEFAULT_HZ {
        return 0.0;
    }
    let a = a4_hz.clamp(A4_MIN_HZ, A4_MAX_HZ);
    (12.0 * cantus_dsp::math::log2(a / A4_DEFAULT_HZ)) as f32
}

/// How the octave (or a scale's period) is divided.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TuningSystem {
    /// 12-tone equal temperament.
    #[default]
    Equal,
    /// A named historical or just table, rotated so its best key sits on `key` (0 = C …
    /// 11 = B). With *follow the song's key*, the song's key rotates it.
    Temperament {
        id: Temperament,
        #[serde(default)]
        key: u8,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        follow_song_key: bool,
    },
    /// A Scala scale, as its file said it (the song keeps the scale, never a path).
    Scale(ScalaScale),
    /// The MTS-ESP master's table (ADR 0156); equal temperament when there is none.
    External,
}

impl TuningSystem {
    pub fn is_equal(&self) -> bool {
        matches!(self, TuningSystem::Equal)
    }
}

/// The named temperaments (ADR 0155; their tables in `tuning_temper`, cited in
/// `research/instruments/microtuning-verified.md` §5). **Append only**: a song stores the name.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Temperament {
    Pythagorean,
    MeantoneQuarter,
    MeantoneSixth,
    WerckmeisterIii,
    WerckmeisterIv,
    KirnbergerIi,
    KirnbergerIii,
    Vallotti,
    Young1799,
    Kellner,
    Neidhardt,
    BachLehman,
    JustMajor,
    JustMinor,
    Barbershop7,
}

impl Temperament {
    /// Every temperament, in the order the window lists them.
    pub const ALL: [Temperament; 15] = [
        Temperament::Pythagorean,
        Temperament::MeantoneQuarter,
        Temperament::MeantoneSixth,
        Temperament::WerckmeisterIii,
        Temperament::WerckmeisterIv,
        Temperament::KirnbergerIi,
        Temperament::KirnbergerIii,
        Temperament::Vallotti,
        Temperament::Young1799,
        Temperament::Kellner,
        Temperament::Neidhardt,
        Temperament::BachLehman,
        Temperament::JustMajor,
        Temperament::JustMinor,
        Temperament::Barbershop7,
    ];

    /// Its locale key (`areas/tuning.*`).
    pub fn name_key(self) -> &'static str {
        match self {
            Temperament::Pythagorean => "tuning.temper.pythagorean",
            Temperament::MeantoneQuarter => "tuning.temper.meantone_quarter",
            Temperament::MeantoneSixth => "tuning.temper.meantone_sixth",
            Temperament::WerckmeisterIii => "tuning.temper.werckmeister_iii",
            Temperament::WerckmeisterIv => "tuning.temper.werckmeister_iv",
            Temperament::KirnbergerIi => "tuning.temper.kirnberger_ii",
            Temperament::KirnbergerIii => "tuning.temper.kirnberger_iii",
            Temperament::Vallotti => "tuning.temper.vallotti",
            Temperament::Young1799 => "tuning.temper.young_1799",
            Temperament::Kellner => "tuning.temper.kellner",
            Temperament::Neidhardt => "tuning.temper.neidhardt",
            Temperament::BachLehman => "tuning.temper.bach_lehman",
            Temperament::JustMajor => "tuning.temper.just_major",
            Temperament::JustMinor => "tuning.temper.just_minor",
            Temperament::Barbershop7 => "tuning.temper.barbershop_7",
        }
    }
}

/// One degree of a Scala scale, kept as its file wrote it.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleDegree {
    /// A line with a period: cents above the scale's degree 0.
    Cents(f64),
    /// A line without one: a ratio (an integer `n` is `n/1`).
    Ratio(u64, u64),
}

/// A Scala `.scl` scale: its description and its degrees above 1/1, the last the period.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ScalaScale {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub degrees: Vec<ScaleDegree>,
}

/// A Scala `.kbm` keyboard map (field names after the Scala help's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyboardMap {
    /// The size of the map's pattern (0: a linear map, key by key).
    pub size: u32,
    pub first_key: u8,
    pub last_key: u8,
    /// The key where degree 0 is.
    pub middle_key: u8,
    /// The key `reference_hz` belongs to.
    pub reference_key: u8,
    pub reference_hz: f64,
    /// The scale degree that is the formal octave (the period the pattern repeats at).
    pub octave_degree: u32,
    /// Each position's degree; `None` (`x` in the file) is an unmapped key — it sounds nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mapping: Vec<Option<u32>>,
}

/// A track's tuning against the project's (ADR 0155).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum TrackTuning {
    /// The project's tuning — every older track. Not written.
    #[default]
    Follow,
    /// 12-TET at 440, whatever the project says (a drum bus, a sampled loop).
    Equal,
    /// A tuning of its own.
    Own { tuning: Tuning },
}

impl TrackTuning {
    pub fn is_follow(&self) -> bool {
        matches!(self, TrackTuning::Follow)
    }

    /// The tuning this track plays in, given the project's.
    pub fn resolve<'a>(&'a self, project: &'a Tuning) -> std::borrow::Cow<'a, Tuning> {
        match self {
            TrackTuning::Follow => std::borrow::Cow::Borrowed(project),
            TrackTuning::Equal => std::borrow::Cow::Owned(Tuning::default()),
            TrackTuning::Own { tuning } => std::borrow::Cow::Borrowed(tuning),
        }
    }
}

/// The semitones each MIDI key sits from 12-TET at A440 (ADR 0155). `f32::NAN` marks a key a
/// keyboard map leaves unmapped: a unit plays nothing for it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TuningTable(pub [f32; 128]);

impl TuningTable {
    /// 12-TET at A440: every entry `0.0`.
    pub const EQUAL: TuningTable = TuningTable([0.0; 128]);

    /// Whether this is 12-TET at A440 to the bit — a unit may skip any work then.
    pub fn is_equal(&self) -> bool {
        self.0.iter().all(|v| v.to_bits() == 0)
    }

    /// The offset of `key` in semitones; `0.0` past 127.
    #[inline]
    pub fn at(&self, key: u8) -> f32 {
        self.0.get(usize::from(key)).copied().unwrap_or(0.0)
    }

    /// Whether `key` sounds (a keyboard map may leave it unmapped).
    #[inline]
    pub fn sounds(&self, key: u8) -> bool {
        !self.at(key).is_nan()
    }
}

impl Default for TuningTable {
    fn default() -> Self {
        TuningTable::EQUAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_equal_to_the_bit() {
        assert!(Tuning::default().is_default());
        let t = Tuning::default().offsets();
        assert!(t.is_equal());
        assert_eq!(t, TuningTable::EQUAL);
        assert!(TrackTuning::default().is_follow());
    }

    #[test]
    fn a_diapason_shifts_every_key_alike() {
        let t = Tuning {
            a4_hz: 415.0,
            ..Tuning::default()
        }
        .offsets();
        let want = 12.0 * cantus_dsp::math::log2(415.0f64 / 440.0);
        for k in 0..128u8 {
            assert!((f64::from(t.at(k)) - want).abs() < 1e-5, "key {k}");
        }
        assert!(!t.is_equal());
    }

    #[test]
    fn a_track_resolves_against_the_project() {
        let p = Tuning {
            a4_hz: 432.0,
            ..Tuning::default()
        };
        assert_eq!(*TrackTuning::Follow.resolve(&p), p);
        assert!(TrackTuning::Equal.resolve(&p).is_default());
    }

    #[test]
    fn the_default_is_not_written() {
        let json = serde_json::to_string(&Tuning::default()).unwrap();
        assert_eq!(json, r#"{"a4_hz":440.0}"#);
        let back: Tuning = serde_json::from_str("{}").unwrap();
        assert!(back.is_default());
    }
}
