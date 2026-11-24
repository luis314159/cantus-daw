//! Scala files in and out of the Tuning window (ADR 0155 §2–3): *Import…* reads a `.scl`
//! (and the `.kbm` beside it, or one picked with it) into one `SetProjectTuning` — the song
//! keeps the scale, never a path — and *Export…* writes the tuning as a `.scl` and, when the
//! scale alone would not sound as it does (a keyboard map, a diapason that is not 440, a
//! temperament's A anchor), a `.kbm` (r42 fix2-model, review 3). An Ableton `.ascl`'s
//! `@ABL REFERENCE_PITCH` is its map when no `.kbm` comes with it. r42:tuning-face
//!
//! The files are read and written by `cantus_core::tuning_scala` (every rule of the Scala
//! help, tested on the whole archive); this module is the window's side of them.

use super::Target;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::tuning::{KeyboardMap, ScalaScale, ScaleDegree, Tuning, TuningSystem};
use cantus_core::tuning_scala;
use std::path::{Path, PathBuf};

/// Why a Scala file was refused, in the interface's language (the reason
/// `cantus_core::tuning_scala` states, each in its own words — r42 fix-ui, review ui#16).
pub type ScalaError = String;

/// The words of a field a Scala error names (`tuning.scala.what.*`).
fn what_words(what: &str) -> String {
    let key = match what {
        "description" => "tuning.scala.what.description",
        "number of notes" => "tuning.scala.what.count",
        "pitch value (a blank line)" => "tuning.scala.what.pitch_blank",
        "pitch value" => "tuning.scala.what.pitch",
        "mapping entry (a degree or x)" => "tuning.scala.what.entry",
        "map size" => "tuning.scala.what.map_size",
        "first key" => "tuning.scala.what.first_key",
        "last key" => "tuning.scala.what.last_key",
        "middle key" => "tuning.scala.what.middle_key",
        "reference key" => "tuning.scala.what.reference_key",
        "reference frequency" => "tuning.scala.what.reference_hz",
        "formal octave degree" => "tuning.scala.what.octave_degree",
        "reference pitch (@ABL REFERENCE_PITCH)" => "tuning.scala.what.reference_pitch",
        other => return other.to_string(),
    };
    t(key)
}

/// A Scala error in the interface's language.
pub fn scala_why(e: &tuning_scala::ScalaError) -> String {
    use tuning_scala::ScalaError as E;
    match e {
        E::NotUtf8 => t("tuning.scala.not_utf8"),
        E::Missing { what, line } => tf(
            "tuning.scala.missing",
            &[("what", &what_words(what)), ("line", &line.to_string())],
        ),
        E::Bad { what, line, text } => tf(
            "tuning.scala.bad",
            &[
                ("what", &what_words(what)),
                ("line", &line.to_string()),
                ("text", text),
            ],
        ),
        E::TooMany { count } => tf(
            "tuning.scala.too_many",
            &[
                ("n", &count.to_string()),
                ("max", &cantus_core::tuning_cmd::MAX_DEGREES.to_string()),
            ],
        ),
        E::FewerPitches { count, found } => tf(
            "tuning.scala.fewer",
            &[("n", &count.to_string()), ("found", &found.to_string())],
        ),
        E::ShortMap { size, found } => tf(
            "tuning.scala.short_map",
            &[("n", &size.to_string()), ("found", &found.to_string())],
        ),
        E::ReferenceUnmapped => t("tuning.scala.reference_unmapped"),
    }
}

/// Why `tuning_cmd::check` refused a tuning, in the interface's language.
fn check_why(e: &cantus_core::CommandError) -> String {
    let raw = e.to_string();
    let key = [
        ("diapason out of range", "tuning.check.a4"),
        ("temperament key", "tuning.check.key"),
        ("scale degrees", "tuning.check.degrees"),
        ("scale degree", "tuning.check.degree"),
        ("keyboard map keys", "tuning.check.map_keys"),
        ("keyboard map reference", "tuning.check.map_reference"),
        ("keyboard map size", "tuning.check.map_size"),
    ]
    .into_iter()
    .find(|(what, _)| raw.ends_with(what));
    match key {
        Some((_, k)) => t(k),
        None => raw,
    }
}

/// A file that could not be read or written: its name, in the interface's language (the
/// system's own words stay out: they are in the system's language).
fn cannot(key: &str, p: &Path) -> String {
    tf(key, &[("file", &file_name(p))])
}

/// A `.scl`: its description and its degrees, as written (`tuning_scala::read_scl`).
pub fn read_scl(bytes: &[u8]) -> Result<ScalaScale, ScalaError> {
    tuning_scala::read_scl(bytes).map_err(|e| scala_why(&e))
}

/// A `.kbm` (`tuning_scala::read_kbm`).
pub fn read_kbm(bytes: &[u8]) -> Result<KeyboardMap, ScalaError> {
    tuning_scala::read_kbm(bytes).map_err(|e| scala_why(&e))
}

/// A `.scl` as Scala writes one (`tuning_scala::write_scl`).
pub fn write_scl(s: &ScalaScale) -> String {
    tuning_scala::write_scl(s)
}

/// A `.kbm` as Scala writes one (`tuning_scala::write_kbm`).
pub fn write_kbm(m: &KeyboardMap) -> String {
    tuning_scala::write_kbm(m)
}

/// `text` as Latin-1 bytes, as a `.scl` is saved (`tuning_scala::latin1`).
pub fn latin1(text: &str) -> Vec<u8> {
    tuning_scala::latin1(text)
}

/// The scale a tuning exports as: its own, or the 12 degrees it plays above C (a
/// temperament, equal temperament, the master's table), in cents.
pub fn scale_for_export(app: &CantusApp, target: Target) -> ScalaScale {
    let tu = super::effective(&super::tuning_of(app, target), app.project().key);
    if let TuningSystem::Scale(s) = &tu.system {
        return s.clone();
    }
    let table = super::table_of(app, target);
    let c4 = f64::from(table.at(60));
    let degrees = (1..=12)
        .map(|p| {
            let off = f64::from(table.at(60 + p as u8)) - c4;
            ScaleDegree::Cents(100.0 * (f64::from(p) + off))
        })
        .collect();
    ScalaScale {
        description: super::words::summary_of(&tu, app.project().key),
        degrees,
    }
}

/// *Import…*: the `.scl` read, and the `.kbm` with it — given, or beside it with the same
/// name — into `target`'s tuning, one undo step. The diapason stays; a `.kbm`'s reference
/// frequency then says what its key sounds. Returns the line the window shows.
pub fn import(
    app: &mut CantusApp,
    target: Target,
    scl: &Path,
    kbm: Option<&Path>,
) -> Result<String, String> {
    let read = |p: &Path| std::fs::read(p).map_err(|_| cannot("tuning.io.cannot_read", p));
    let scale = read_scl(&read(scl)?).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?;
    let beside = scl.with_extension("kbm");
    let kbm_path = kbm
        .map(Path::to_path_buf)
        .or_else(|| beside.is_file().then_some(beside));
    let map = match &kbm_path {
        Some(p) => Some(read_kbm(&read(p)?).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?),
        // An Ableton `.ascl` says its reference in its own line (r42 fix2-model).
        None => tuning_scala::read_ascl_map(&read(scl)?, &scale)
            .map_err(|e| tf("tuning.io.refused", &[("why", &scala_why(&e))]))?,
    };
    let now = super::tuning_of(app, target);
    // With a map, its reference key sounds at `reference_hz × a4_hz / 440` (ADR 0155, the
    // `tuning` builder's reading): the import puts the diapason back to 440, so the map sounds
    // as written and the diapason then raises or lowers it whole. Without one the diapason
    // stays (degree 0 on key 60 at C4 of 12-TET at it — Scala's and Surge's anchor).
    let tu = Tuning {
        a4_hz: if map.is_some() { 440.0 } else { now.a4_hz },
        system: TuningSystem::Scale(scale),
        map,
    };
    if let Err(e) = cantus_core::tuning_cmd::check(&tu) {
        return Err(tf("tuning.io.refused", &[("why", &check_why(&e))]));
    }
    super::set(app, target, tu.clone());
    let name = file_name(scl);
    Ok(match kbm_path {
        Some(k) => tf(
            "tuning.io.imported_map",
            &[("file", &name), ("map", &file_name(&k))],
        ),
        None => tf("tuning.io.imported", &[("file", &name)]),
    })
}

/// The map row's *Import a keyboard map…*: the system's file dialog for one `.kbm`.
pub fn import_map_dialog(app: &mut CantusApp, target: Target) {
    let Some(kbm) = rfd::FileDialog::new()
        .add_filter(t("tuning.io.filter"), &["kbm"])
        .pick_file()
    else {
        return;
    };
    app.tuning.last = Some(import_map(app, target, &kbm));
}

/// The `.kbm` written beside an exported `.scl`, so that reading the two back (here, in Scala
/// or in Surge) gives the same pitches (r42 fix2-model, review 3): the tuning's own map with
/// its reference frequency scaled by the diapason; or, when the scale alone would sound
/// elsewhere — a diapason that is not 440, a temperament (A on the diapason, so C4 is not
/// 12-TET's) or the master's table — a linear map with degree 0 on key 60 at what key 60
/// sounds now. `None` when the `.scl` alone says it all (Scala's default anchor).
pub fn map_for_export(app: &CantusApp, target: Target) -> Option<KeyboardMap> {
    let tu = super::effective(&super::tuning_of(app, target), app.project().key);
    let scale = tu.a4_hz / cantus_core::tuning::A4_DEFAULT_HZ;
    if let Some(mut m) = tu.map {
        if scale != 1.0 {
            m.reference_hz *= scale;
        }
        return Some(m);
    }
    let reference_hz = if matches!(tu.system, TuningSystem::Scale(_)) {
        if scale == 1.0 {
            return None;
        }
        cantus_core::tuning::C4_DEFAULT_HZ * scale
    } else {
        let c4 = f64::from(super::table_of(app, target).at(60));
        if c4 == 0.0 {
            return None;
        }
        cantus_core::tuning::C4_DEFAULT_HZ * cantus_dsp::math::exp2(c4 / 12.0)
    };
    Some(KeyboardMap {
        reference_hz,
        ..KeyboardMap::scala_default()
    })
}

/// *Export…*: `path` (a `.scl`) and, when [`map_for_export`] gives one, the `.kbm` beside it.
/// Returns the files written.
pub fn export(app: &CantusApp, target: Target, path: &Path) -> Result<Vec<PathBuf>, String> {
    let scl = path.with_extension("scl");
    let write = |p: &Path, text: &str| {
        std::fs::write(p, latin1(text)).map_err(|_| cannot("tuning.io.cannot_write", p))
    };
    write(&scl, &write_scl(&scale_for_export(app, target)))?;
    let mut out = vec![scl.clone()];
    if let Some(m) = map_for_export(app, target) {
        let kbm = scl.with_extension("kbm");
        write(&kbm, &write_kbm(&m))?;
        out.push(kbm);
    }
    Ok(out)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The *Import…* key: the system's file dialog (one `.scl`, or a `.scl` and a `.kbm`).
pub fn import_dialog(app: &mut CantusApp, target: Target) {
    let Some(files) = rfd::FileDialog::new()
        .add_filter(t("tuning.io.filter"), &["scl", "ascl", "kbm"])
        .pick_files()
    else {
        return;
    };
    let is = |p: &&PathBuf, ext: &str| {
        p.extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
    };
    let scl = files
        .iter()
        .find(|p| is(p, "scl") || is(p, "ascl"))
        .cloned();
    let kbm = files.iter().find(|p| is(p, "kbm")).cloned();
    app.tuning.last = Some(match scl {
        Some(scl) => import(app, target, &scl, kbm.as_deref()),
        // A `.kbm` alone maps the scale the tuning has.
        None => match kbm {
            Some(k) => import_map(app, target, &k),
            None => return,
        },
    });
}

/// A `.kbm` alone: the tuning keeps what it plays, with this keyboard map — a scale its
/// scale; equal temperament, a temperament or the master's table become the twelve degrees
/// they play, as a Scala scale (a map belongs to a scale: the window shows and removes it
/// only there — r42 fix-ui, review ui#4).
pub fn import_map(app: &mut CantusApp, target: Target, kbm: &Path) -> Result<String, String> {
    let bytes = std::fs::read(kbm).map_err(|_| cannot("tuning.io.cannot_read", kbm))?;
    let map = read_kbm(&bytes).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?;
    let mut tu = super::tuning_of(app, target);
    if !matches!(tu.system, TuningSystem::Scale(_)) {
        tu.system = TuningSystem::Scale(scale_for_export(app, target));
    }
    tu.map = Some(map);
    tu.a4_hz = 440.0;
    if let Err(e) = cantus_core::tuning_cmd::check(&tu) {
        return Err(tf("tuning.io.refused", &[("why", &check_why(&e))]));
    }
    super::set(app, target, tu);
    Ok(tf("tuning.io.imported", &[("file", &file_name(kbm))]))
}

/// The *Export…* key: the system's save dialog, named after the song.
pub fn export_dialog(app: &mut CantusApp, target: Target) {
    let name = format!("{}.scl", app.project().name);
    let Some(path) = rfd::FileDialog::new()
        .add_filter(t("tuning.io.filter"), &["scl"])
        .set_file_name(name)
        .save_file()
    else {
        return;
    };
    app.tuning.last = Some(export(app, target, &path).map(|files| {
        let names: Vec<String> = files.iter().map(|p| file_name(p)).collect();
        tf("tuning.io.exported", &[("files", &names.join(", "))])
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_and_a_map_read_back_as_they_were_written() {
        let s = read_scl(b"! a test\nmeantone fifths\n 3\n!\n 193.157\n 5/4\n 2\n").unwrap();
        assert_eq!(s.description, "meantone fifths");
        assert_eq!(
            s.degrees,
            vec![
                ScaleDegree::Cents(193.157),
                ScaleDegree::Ratio(5, 4),
                ScaleDegree::Ratio(2, 1)
            ]
        );
        assert_eq!(read_scl(write_scl(&s).as_bytes()).unwrap(), s);
        let m = read_kbm(b"12\n0\n127\n60\n69\n442.0\n12\n0\n1\nx\nx\n4\n5\n6\n7\n8\n9\n10\n11\n")
            .unwrap();
        assert_eq!(m.reference_hz, 442.0);
        assert_eq!(m.mapping.len(), 12);
        assert_eq!(&m.mapping[..4], &[Some(0), Some(1), None, None]);
        assert_eq!(read_kbm(write_kbm(&m).as_bytes()).unwrap(), m);
    }

    #[test]
    fn a_file_short_of_its_count_is_refused_with_the_reason() {
        let e = read_scl(
            b"two
2
100.0
",
        )
        .unwrap_err();
        assert!(!e.is_empty(), "a stated reason");
    }

    /// r42 fix-ui (review ui#16): the reason is said in the interface's language — no
    /// English left in a Spanish line, for every refusal the reader states.
    #[test]
    fn a_refusal_reads_in_spanish() {
        use cantus_core::tuning_scala::ScalaError as E;
        crate::i18n::set_thread_locale(crate::i18n::Locale::Es);
        let all = [
            E::NotUtf8,
            E::Missing {
                what: "number of notes",
                line: 3,
            },
            E::Bad {
                what: "reference frequency",
                line: 6,
                text: "abc".into(),
            },
            E::TooMany { count: 5000 },
            E::FewerPitches {
                count: 17,
                found: 16,
            },
            E::ShortMap { size: 12, found: 4 },
            E::ReferenceUnmapped,
        ];
        for e in &all {
            let es = scala_why(e);
            for english in [" the ", "line ", "pitches", "should hold", "entries"] {
                assert!(!es.contains(english), "{e:?} -> {es}");
            }
        }
        let short = read_scl(b"two\n2\n100.0\n").unwrap_err();
        assert!(!short.contains("the count says"), "{short}");
        let refused = check_why(&cantus_core::CommandError::InvalidValue(
            "tuning: diapason out of range".into(),
        ));
        assert_eq!(refused, t("tuning.check.a4"));
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
    }
}
