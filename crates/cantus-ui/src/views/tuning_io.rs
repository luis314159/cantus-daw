//! Scala files in and out of the Tuning window (ADR 0155 §2–3): *Import…* reads a `.scl`
//! (and the `.kbm` beside it, or one picked with it) into one `SetProjectTuning` — the song
//! keeps the scale, never a path — and *Export…* writes the tuning as a `.scl` and, when it
//! has a keyboard map, a `.kbm`. r42:tuning-face
//!
//! **Interim** (r42:tuning-face interim): the four readers and writers below follow the
//! Scala help's grammar (`research/instruments/microtuning-verified.md` §1) until the
//! `tuning` builder's `cantus_core::tuning_scala::{read_scl, read_kbm, write_scl, write_kbm}`
//! land — the merge swaps these bodies (and `latin1`) for calls to theirs (the signatures
//! are theirs).

use super::Target;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::tuning::{KeyboardMap, ScalaScale, ScaleDegree, Tuning, TuningSystem};
use std::path::{Path, PathBuf};

/// Why a Scala file was refused.
pub type ScalaError = String;

/// The lines that are not comments (`!` first), each without its line end.
fn lines(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    // Scala files are Latin-1 (the verified note, §1.1): each byte its own character.
    bytes
        .split(|b| *b == b'\n')
        .map(|l| l.iter().map(|b| char::from(*b)).collect::<String>())
        .map(|l| l.trim_end_matches('\r').to_string())
        .filter(|l| !l.starts_with('!'))
}

fn first_word(l: &str) -> &str {
    l.split_whitespace().next().unwrap_or("")
}

/// A `.scl`: its description and its degrees, as written.
pub fn read_scl(bytes: &[u8]) -> Result<ScalaScale, ScalaError> {
    let mut it = lines(bytes);
    let description = it.next().ok_or("no description line")?.trim().to_string();
    let count: usize = it
        .by_ref()
        .map(|l| l.trim().to_string())
        .next()
        .and_then(|l| first_word(&l).parse().ok())
        .ok_or("no note count")?;
    let mut degrees = Vec::with_capacity(count);
    for l in it {
        if degrees.len() == count {
            break;
        }
        let w = first_word(&l);
        if w.is_empty() {
            continue;
        }
        degrees.push(degree(w).ok_or_else(|| format!("bad pitch {w:?}"))?);
    }
    if degrees.len() < count {
        return Err(format!("{} pitches of {count}", degrees.len()));
    }
    Ok(ScalaScale {
        description,
        degrees,
    })
}

fn degree(w: &str) -> Option<ScaleDegree> {
    if w.contains('.') {
        return w
            .parse::<f64>()
            .ok()
            .filter(|c| c.is_finite())
            .map(ScaleDegree::Cents);
    }
    let (n, d) = match w.split_once('/') {
        Some((n, d)) => (n.parse::<u64>().ok()?, d.parse::<u64>().ok()?),
        None => (w.parse::<u64>().ok()?, 1),
    };
    (n > 0 && d > 0).then_some(ScaleDegree::Ratio(n, d))
}

/// A `.kbm`: its seven numbers and its mapping (`x` an unmapped key; a short tail is `x`).
pub fn read_kbm(bytes: &[u8]) -> Result<KeyboardMap, ScalaError> {
    let words: Vec<String> = lines(bytes)
        .map(|l| first_word(&l).to_string())
        .filter(|w| !w.is_empty())
        .collect();
    let num = |i: usize, what: &str| -> Result<f64, ScalaError> {
        words
            .get(i)
            .and_then(|w| w.parse::<f64>().ok())
            .ok_or_else(|| format!("no {what}"))
    };
    let size = num(0, "map size")? as u32;
    let key = |i: usize, what: &str| -> Result<u8, ScalaError> {
        let v = num(i, what)?;
        if (0.0..=127.0).contains(&v) {
            Ok(v as u8)
        } else {
            Err(format!("{what} out of range"))
        }
    };
    let mut mapping = Vec::with_capacity(size as usize);
    for i in 0..size as usize {
        match words.get(7 + i).map(String::as_str) {
            None | Some("x") | Some("X") => mapping.push(None),
            Some(w) => mapping.push(Some(
                w.parse::<u32>().map_err(|_| format!("bad degree {w:?}"))?,
            )),
        }
    }
    Ok(KeyboardMap {
        size,
        first_key: key(1, "first key")?,
        last_key: key(2, "last key")?,
        middle_key: key(3, "middle key")?,
        reference_key: key(4, "reference key")?,
        reference_hz: num(5, "reference frequency")?,
        octave_degree: num(6, "formal octave")? as u32,
        mapping,
    })
}

/// A `.scl` as Scala writes one.
pub fn write_scl(s: &ScalaScale) -> String {
    let mut out = String::from("! Written by Cantus\n!\n");
    out.push_str(&s.description);
    out.push('\n');
    out.push_str(&format!(" {}\n!\n", s.degrees.len()));
    for d in &s.degrees {
        out.push(' ');
        out.push_str(&match *d {
            ScaleDegree::Cents(c) => format!("{c:.6}"),
            ScaleDegree::Ratio(n, d) => format!("{n}/{d}"),
        });
        out.push('\n');
    }
    out
}

/// A `.kbm` as Scala writes one.
pub fn write_kbm(m: &KeyboardMap) -> String {
    let mut out = String::from("! Written by Cantus\n");
    for (what, v) in [
        ("Map size", m.size.to_string()),
        ("First MIDI note to retune", m.first_key.to_string()),
        ("Last MIDI note to retune", m.last_key.to_string()),
        (
            "Middle note where the first entry is mapped",
            m.middle_key.to_string(),
        ),
        ("Reference note", m.reference_key.to_string()),
        ("Reference frequency", format!("{:.6}", m.reference_hz)),
        (
            "Scale degree of the formal octave",
            m.octave_degree.to_string(),
        ),
    ] {
        out.push_str(&format!("! {what}\n{v}\n"));
    }
    out.push_str("! Mapping\n");
    for d in &m.mapping {
        match d {
            Some(d) => out.push_str(&format!("{d}\n")),
            None => out.push_str("x\n"),
        }
    }
    out
}

/// `text` as Latin-1 bytes (a `.scl` is Latin-1; a character past it becomes `?`).
pub fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
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
    let read = |p: &Path| std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
    let scale = read_scl(&read(scl)?).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?;
    let beside = scl.with_extension("kbm");
    let kbm_path = kbm
        .map(Path::to_path_buf)
        .or_else(|| beside.is_file().then_some(beside));
    let map = match &kbm_path {
        Some(p) => Some(read_kbm(&read(p)?).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?),
        None => None,
    };
    let now = super::tuning_of(app, target);
    // With a map, its reference key sounds at `reference_hz × a4_hz / 440` (ADR 0155, the
    // `tuning` builder's reading): the import puts the diapason back to 440, so the map sounds
    // as written and the diapason then raises or lowers it whole. Without one the diapason
    // stays (key 69 sounds at it, Scala's template).
    let tu = Tuning {
        a4_hz: if map.is_some() { 440.0 } else { now.a4_hz },
        system: TuningSystem::Scale(scale),
        map,
    };
    if let Err(e) = cantus_core::tuning_cmd::check(&tu) {
        return Err(tf("tuning.io.refused", &[("why", &e.to_string())]));
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

/// *Export…*: `path` (a `.scl`) and, when the tuning has a keyboard map, the `.kbm` beside
/// it. Returns the files written.
pub fn export(app: &CantusApp, target: Target, path: &Path) -> Result<Vec<PathBuf>, String> {
    let scl = path.with_extension("scl");
    let write = |p: &Path, text: &str| {
        std::fs::write(p, latin1(text)).map_err(|e| format!("{}: {e}", p.display()))
    };
    write(&scl, &write_scl(&scale_for_export(app, target)))?;
    let mut out = vec![scl.clone()];
    if let Some(m) = super::tuning_of(app, target).map {
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
        .add_filter(t("tuning.io.filter"), &["scl", "kbm"])
        .pick_files()
    else {
        return;
    };
    let is = |p: &&PathBuf, ext: &str| {
        p.extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
    };
    let scl = files.iter().find(|p| is(p, "scl")).cloned();
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

/// A `.kbm` alone: the tuning keeps its scale, with this keyboard map.
pub fn import_map(app: &mut CantusApp, target: Target, kbm: &Path) -> Result<String, String> {
    let bytes = std::fs::read(kbm).map_err(|e| format!("{}: {e}", kbm.display()))?;
    let map = read_kbm(&bytes).map_err(|e| tf("tuning.io.refused", &[("why", &e)]))?;
    let mut tu = super::tuning_of(app, target);
    tu.map = Some(map);
    tu.a4_hz = 440.0;
    if let Err(e) = cantus_core::tuning_cmd::check(&tu) {
        return Err(tf("tuning.io.refused", &[("why", &e.to_string())]));
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
        let m = read_kbm(b"12\n0\n127\n60\n69\n442.0\n12\n0\n1\nx\n").unwrap();
        assert_eq!(m.reference_hz, 442.0);
        assert_eq!(m.mapping.len(), 12);
        assert_eq!(&m.mapping[..4], &[Some(0), Some(1), None, None]);
        assert_eq!(read_kbm(write_kbm(&m).as_bytes()).unwrap(), m);
    }

    #[test]
    fn a_file_short_of_its_count_is_refused_with_the_reason() {
        let e = read_scl(b"two\n2\n100.0\n").unwrap_err();
        assert!(e.contains("1 pitches of 2"), "{e}");
    }
}
