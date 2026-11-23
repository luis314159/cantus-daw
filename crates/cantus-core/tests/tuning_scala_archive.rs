//! Round 42 (ADR 0155): the whole Scala archive (version 94) through the reader — run by
//! `tools/test/scala-archive.sh`, which downloads it (it has no licence: never committed),
//! checks its SHA-256 and points `CANTUS_SCALA_ARCHIVE` at its `scl` folder.
//!
//! Every one of the 5 401 files parses or is refused for a stated reason; each one read
//! writes back to the same cents within 1e-9 (and the same description); and the files the
//! temperaments' table cites (`microtuning-verified.md` §5.2) hold Cantus's tables.

use cantus_core::tuning::Temperament;
use cantus_core::tuning_scala::{latin1, read_scl, write_scl};
use std::collections::BTreeMap;

#[test]
#[ignore = "needs the archive: bash tools/test/scala-archive.sh"]
fn every_archive_file_reads_or_is_refused_with_a_reason_and_round_trips() {
    let dir = std::env::var("CANTUS_SCALA_ARCHIVE")
        .expect("CANTUS_SCALA_ARCHIVE: the archive's scl folder (tools/test/scala-archive.sh)");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the archive's folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("scl")))
        .collect();
    // Sorted: never the file system's order (RELEASING L4).
    files.sort();
    assert_eq!(files.len(), 5401, "version 94 has 5 401 files");
    let mut read = 0usize;
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut degrees = 0usize;
    let mut most = (0usize, String::new());
    for p in &files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(p).unwrap();
        match read_scl(&bytes) {
            Ok(s) => {
                read += 1;
                degrees += s.degrees.len();
                if s.degrees.len() > most.0 {
                    most = (s.degrees.len(), name.clone());
                }
                let back = read_scl(&latin1(&write_scl(&s)))
                    .unwrap_or_else(|e| panic!("{name}: our own file refused: {e}"));
                assert_eq!(back.description, s.description, "{name}");
                assert_eq!(back.degrees.len(), s.degrees.len(), "{name}");
                for (i, (a, b)) in s.degrees.iter().zip(&back.degrees).enumerate() {
                    assert!(
                        (a.cents() - b.cents()).abs() < 1e-9,
                        "{name} degree {i}: {a:?} vs {b:?}"
                    );
                }
            }
            Err(e) => refused.entry(e.to_string()).or_default().push(name),
        }
    }
    println!(
        "{read} of {} files read ({degrees} degrees; the most, {}, in {}); refused:",
        files.len(),
        most.0,
        most.1
    );
    for (why, names) in &refused {
        println!("  {} × {why}: {}", names.len(), names.join(", "));
    }
    // The archive is Scala's own: every file is one Scala reads.
    assert!(refused.is_empty(), "refused: {refused:?}");

    // The historical files hold the temperaments' tables (C … B, three decimals).
    let path = |n: &str| std::path::Path::new(&dir).join(n);
    for (file, id) in [
        ("pyth_12.scl", Temperament::Pythagorean),
        ("meanquar.scl", Temperament::MeantoneQuarter),
        ("meansixth.scl", Temperament::MeantoneSixth),
        ("werck3.scl", Temperament::WerckmeisterIii),
        ("werck4.scl", Temperament::WerckmeisterIv),
        ("kirnberger.scl", Temperament::KirnbergerIii),
        ("vallotti.scl", Temperament::Vallotti),
        ("young.scl", Temperament::Young1799),
        ("kellner.scl", Temperament::Kellner),
        ("neidhardt3.scl", Temperament::Neidhardt),
        ("lehman1.scl", Temperament::BachLehman),
        ("malcolm.scl", Temperament::JustMajor),
        ("duodene.scl", Temperament::JustMinor),
        ("ji_12a.scl", Temperament::Barbershop7),
    ] {
        let s = read_scl(&std::fs::read(path(file)).unwrap()).unwrap();
        let mut c: Vec<f64> = s.degrees.iter().map(|d| d.cents()).collect();
        assert_eq!(c.len(), 12, "{file}");
        assert!(
            (c.pop().unwrap() - 1200.0).abs() < 1e-9,
            "{file}: an octave"
        );
        c.sort_by(f64::total_cmp);
        let ours = cantus_core::tuning_temper::cents(id);
        for pc in 1..12 {
            assert!(
                (c[pc - 1] - ours[pc]).abs() < 0.002,
                "{file} vs {id:?}, pc {pc}: {} vs {}",
                c[pc - 1],
                ours[pc]
            );
        }
    }
}
