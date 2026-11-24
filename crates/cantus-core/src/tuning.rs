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
/// Middle C (MIDI key 60) of 12-TET at A440: `440 · 2^(−9/12)` = 261.6256 Hz — where Scala
/// and Surge's `tuning-library` put a scale's degree 0 when no `.kbm` says otherwise.
pub const C4_DEFAULT_HZ: f64 = 261.625_565_300_598_6;
/// The diapason's range (ADR 0155): from a low baroque pitch to a bright modern one.
pub const A4_MIN_HZ: f64 = 380.0;
pub const A4_MAX_HZ: f64 = 480.0;
/// The diapason presets the window offers.
pub const A4_PRESETS_HZ: [f64; 6] = [415.0, 430.0, 432.0, 440.0, 442.0, 443.0];

/// A project's or a track's tuning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// The frequency of A4 (MIDI key 69) — the diapason. With a keyboard map, the map's
    /// reference key sounds at its `reference_hz` scaled by `a4_hz / 440` (r42:tuning): a
    /// `.kbm` imported at 440 keeps its numbers, and A415 lowers the whole map alike.
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

    /// r42:tuning — this tuning in a song whose key's root is `song_key_root` (0 = C): a
    /// temperament that follows the song's key takes that root as its key; anything else is
    /// itself. Every resolution of a track's table goes through it, so a change of the song's
    /// key re-rotates with no command of its own; the tuning window calls it too.
    pub fn resolved_for_key(&self, song_key_root: Option<u8>) -> Tuning {
        let mut t = self.clone();
        if let (
            TuningSystem::Temperament {
                key,
                follow_song_key: true,
                ..
            },
            Some(root),
        ) = (&mut t.system, song_key_root)
        {
            *key = root % 12;
        }
        t
    }

    /// The semitones each MIDI key sits from 12-TET at A440 — what every unit reads (ADR
    /// 0155). Exactly `0.0` everywhere at the default.
    ///
    /// Every system (r42:tuning): `Equal` at the diapason; a temperament rotated to its key
    /// with A at the diapason (as Pianoteq and Logic anchor it); a Scala scale through its
    /// keyboard map (none: degree 0 on key 60 at C4 of 12-TET at the diapason — as Scala and
    /// Surge's `tuning-library` anchor a scale with no `.kbm`), any period, an unmapped key
    /// `NAN`. `External` takes the table its caller read from
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
        let shift = diapason_semitones(self.a4_hz);
        match (&self.system, &self.map) {
            // r42:tuning — the master's table is absolute: no diapason on top of it.
            (TuningSystem::External, _) => match external {
                Some(t) => *t,
                None => TuningTable([shift; 128]),
            },
            (TuningSystem::Equal, None) => TuningTable([shift; 128]),
            _ => self.mapped(shift),
        }
    }

    /// r42:tuning — the degrees of the system (cents above degree 0, the last the period)
    /// and their table through the keyboard map (Scala's "octave extension").
    fn mapped(&self, shift: f32) -> TuningTable {
        let degrees = self.system.degree_cents();
        let n = degrees.len() as i64;
        if n == 0 {
            return TuningTable([shift; 128]);
        }
        let period = degrees[degrees.len() - 1];
        let cents_of = |d: i64| -> f64 {
            let q = d.div_euclid(n);
            let r = d.rem_euclid(n);
            q as f64 * period
                + if r == 0 {
                    0.0
                } else {
                    degrees[(r - 1) as usize]
                }
        };
        // r42 fix2-model (review 2): with no map a Scala scale has degree 0 on key 60 at
        // 261.6256 Hz — Scala's and Surge's anchor (`KeyboardMapping()`:
        // `tuningConstantNote(60)` at `MIDI_0_FREQ * 32`); a temperament (and equal
        // temperament's twelve) keeps A on the diapason, as Pianoteq and Logic anchor it.
        let default_map = match self.system {
            TuningSystem::Scale(_) => KeyboardMap::scala_default(),
            _ => KeyboardMap::a4_anchored(),
        };
        let map = self.map.as_ref().unwrap_or(&default_map);
        // The reference key's degree; a map that leaves it unmapped (refused on import)
        // takes degree 0 rather than fail here.
        let ref_degree = map.degree_of(map.reference_key, n).unwrap_or(0);
        let ref_cents = cents_of(ref_degree);
        // The reference key's frequency, as semitones from A440; the diapason scales the
        // whole map (a `.kbm` at 440 keeps its numbers; A415 lowers it all alike).
        // C4's frequency is taken as exactly −9 semitones (never through a `log2` that
        // rounds), so a 12-TET scale with no map is 12-TET to the bit.
        let ref_semis = if map.reference_hz == A4_DEFAULT_HZ {
            0.0
        } else if map.reference_hz == C4_DEFAULT_HZ {
            -9.0
        } else {
            12.0 * cantus_dsp::math::log2(map.reference_hz / A4_DEFAULT_HZ)
        };
        let base = ref_semis + f64::from(shift) + 69.0;
        let mut out = [0.0f32; 128];
        for (key, o) in out.iter_mut().enumerate() {
            let k = key as u8;
            if k < map.first_key || k > map.last_key {
                // Not retuned (the Scala help leaves it to the instrument): 12-TET at the
                // diapason, never silent.
                *o = shift;
                continue;
            }
            let Some(d) = map.degree_of(k, n) else {
                *o = f32::NAN;
                continue;
            };
            // The key's pitch as a fractional MIDI key, kept between 1 Hz and 20 kHz.
            let pitch = ((cents_of(d) - ref_cents) / 100.0 + base).clamp(PITCH_MIN, PITCH_MAX);
            *o = (pitch - f64::from(k)) as f32;
        }
        TuningTable(out)
    }
}

/// r42:tuning — the lowest and highest pitch a tuning gives a key, as fractional MIDI keys
/// (1 Hz and 20 kHz): a far mapping never asks an instrument for a pitch it cannot make.
pub const PITCH_MIN: f64 = -36.0;
pub const PITCH_MAX: f64 = 135.0;

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

    /// r42:tuning — the system's degrees above degree 0 in cents, the last the period:
    /// `Equal` twelve hundreds, a temperament its twelve rotated pitches above C, a scale
    /// its file's. `External` has none (its table comes whole).
    pub fn degree_cents(&self) -> Vec<f64> {
        match self {
            TuningSystem::Equal => (1..=12).map(|i| f64::from(i) * 100.0).collect(),
            TuningSystem::Temperament { id, key, .. } => {
                let c = crate::tuning_temper::cents_in(*id, *key);
                c[1..].iter().copied().chain([1200.0]).collect()
            }
            TuningSystem::Scale(s) => s.degrees.iter().map(|d| d.cents()).collect(),
            TuningSystem::External => Vec::new(),
        }
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

impl ScaleDegree {
    /// r42:tuning — the degree in cents (a ratio's `1200·log2(n/d)`, each term's logarithm
    /// apart so 64-bit terms keep their precision).
    pub fn cents(self) -> f64 {
        match self {
            ScaleDegree::Cents(c) => c,
            ScaleDegree::Ratio(n, d) => {
                1200.0 * (cantus_dsp::math::log2(n as f64) - cantus_dsp::math::log2(d as f64))
            }
        }
    }
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

impl KeyboardMap {
    /// What no map means for a Scala scale (r42 fix2-model, review 2): a linear map with
    /// degree 0 on key 60 at middle C of 12-TET at A440 ([`C4_DEFAULT_HZ`]) — Scala's
    /// default and Surge's `tuning-library`'s `KeyboardMapping()`. The diapason then scales it.
    pub fn scala_default() -> Self {
        KeyboardMap {
            reference_key: 60,
            reference_hz: C4_DEFAULT_HZ,
            ..Self::a4_anchored()
        }
    }

    /// What no map means for a temperament: degree 0 (C) on key 60, key 69 (A) at the
    /// diapason (as Pianoteq and Logic anchor a temperament).
    pub fn a4_anchored() -> Self {
        KeyboardMap {
            size: 0,
            first_key: 0,
            last_key: 127,
            middle_key: 60,
            reference_key: 69,
            reference_hz: A4_DEFAULT_HZ,
            octave_degree: 0,
            mapping: Vec::new(),
        }
    }

    /// r42:tuning — the scale degree `key` plays (`None`: unmapped), for a scale of
    /// `scale_len` degrees: key `middle + k·size + i` plays `mapping[i] + k·octave`. A
    /// formal octave of 0 with a pattern means the scale's own period (as Surge's
    /// `tuning-library` reads it); a size of 0 is a linear map.
    pub fn degree_of(&self, key: u8, scale_len: i64) -> Option<i64> {
        let offset = i64::from(key) - i64::from(self.middle_key);
        if self.size == 0 {
            return Some(offset);
        }
        let size = i64::from(self.size);
        let (k, i) = (offset.div_euclid(size), offset.rem_euclid(size));
        let octave = if self.octave_degree == 0 {
            scale_len
        } else {
            i64::from(self.octave_degree)
        };
        let entry = self.mapping.get(i as usize).copied().flatten()?;
        Some(i64::from(entry) + k * octave)
    }
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
///
/// Two tables are equal when every entry has the same bits (r42 fix-engine, review
/// correctness 15): an unmapped key's `NAN` equals itself, so a snapshot or a published
/// table that did not change is seen as unchanged.
#[derive(Copy, Clone, Debug)]
pub struct TuningTable(pub [f32; 128]);

impl PartialEq for TuningTable {
    fn eq(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(&other.0)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

impl Eq for TuningTable {}

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

    /// r42 fix-engine (review correctness 15): a table with an unmapped key equals itself,
    /// so `same_until` and the master's publish see an unchanged tuning as unchanged.
    #[test]
    fn a_table_with_an_unmapped_key_equals_itself() {
        let mut t = TuningTable::EQUAL;
        t.0[61] = f32::NAN;
        t.0[62] = 0.25;
        let copy = t;
        assert_eq!(t, copy);
        let mut other = t;
        other.0[62] = 0.5;
        assert_ne!(t, other);
        let snap = crate::snapshot::TrackTuningSnap {
            table: std::sync::Arc::new(t),
            ..Default::default()
        };
        assert_eq!(snap.clone(), snap);
    }

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
