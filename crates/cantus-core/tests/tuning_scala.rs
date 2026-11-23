//! Round 42 (ADR 0155): the Scala reader and writer against outside bytes — the Scala
//! format page's own vectors, Surge's MIT `tuning-library` test files (committed under
//! `tests/data/tuning-library`, with its must-fail set) — and our own fixtures for every
//! quirk the archive's 5 401 files show (`microtuning-verified.md` §1.3; none of the
//! archive's files is copied: it has no licence). The whole archive is swept by
//! `tuning_scala_archive.rs` (`#[ignore]`, `tools/test/scala-archive.sh`).

use cantus_core::tuning::{ScalaScale, ScaleDegree};
use cantus_core::tuning_scala::{
    parse_degree, read_kbm, read_scl, write_kbm, write_scl, ScalaError,
};

fn data(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/tuning-library")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn cents(s: &ScalaScale) -> Vec<f64> {
    s.degrees.iter().map(|d| d.cents()).collect()
}

fn close(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!((x - y).abs() <= tol, "degree {i}: {x} vs {y}");
    }
}

/// The format page's list of valid pitch lines, byte for byte (leading spaces included).
#[test]
fn the_spec_s_valid_pitch_lines_read_as_it_says() {
    use ScaleDegree::*;
    let want = [
        ("81/64", Ratio(81, 64)),
        ("408.0", Cents(408.0)),
        ("408.", Cents(408.0)),
        ("5", Ratio(5, 1)),
        ("-5.0", Cents(-5.0)),
        ("10/20", Ratio(10, 20)),
        ("100.0 cents", Cents(100.0)),
        (" 100.0 C#", Cents(100.0)),
        (" 5/4   E\\", Ratio(5, 4)),
    ];
    for (line, d) in want {
        assert_eq!(parse_degree(line), Some(d), "{line:?}");
    }
    // A ratio below 1/1 is valid (−1200 c); a negative ratio is "meaningless".
    assert!((Ratio(10, 20).cents() + 1200.0).abs() < 1e-9);
    for bad in ["-5/4", "-5", "What is this", "", "0/1", "3/0", "/4", "."] {
        assert_eq!(parse_degree(bad), None, "{bad:?}");
    }
}

/// The format page's example file (`meanquar.scl` as the spec prints it).
#[test]
fn the_spec_s_example_file_reads() {
    let text = "! meanquar.scl\r\n!\r\n1/4-comma meantone scale. Pietro Aaron's temperament (1523)\r\n 12\r\n!\r\n 76.04900\r\n 193.15686\r\n 310.26471\r\n 5/4\r\n 503.42157\r\n 579.47057\r\n 696.57843\r\n 25/16\r\n 889.73529\r\n 1006.84314\r\n 1082.89214\r\n 2/1\r\n";
    let s = read_scl(text.as_bytes()).unwrap();
    assert_eq!(
        s.description,
        "1/4-comma meantone scale. Pietro Aaron's temperament (1523)"
    );
    assert_eq!(s.degrees.len(), 12);
    assert_eq!(s.degrees[3], ScaleDegree::Ratio(5, 4));
    assert_eq!(s.degrees[11], ScaleDegree::Ratio(2, 1));
    assert!((s.degrees[0].cents() - 76.049).abs() < 1e-9);
}

#[test]
fn surge_s_files_read_as_its_tests_say() {
    // A blank description.
    let s = read_scl(&data("12-intune-nodesc.scl")).unwrap();
    assert_eq!(s.description, "");
    close(
        &cents(&s),
        &(1..=12).map(|i| f64::from(i) * 100.0).collect::<Vec<_>>(),
        1e-9,
    );
    // 31-EDO in LF and in CRLF: the same tones, the same description.
    let a = read_scl(&data("31edo.scl")).unwrap();
    let b = read_scl(&data("31edo_dos_lineends.scl")).unwrap();
    assert_eq!(a.degrees.len(), 31);
    assert_eq!(b.description, "31 equal divisions of octave");
    assert_eq!(a, b);
    for (i, c) in cents(&a).iter().enumerate() {
        assert!(
            (c - 1200.0 * (i as f64 + 1.0) / 31.0).abs() < 1e-3,
            "31edo {i}"
        );
    }
    // A non-octave period (3/1), exact ratios, a one-step scale.
    let e = read_scl(&data("ED3-17.scl")).unwrap();
    assert_eq!(e.degrees.len(), 17);
    assert_eq!(e.degrees[16], ScaleDegree::Ratio(3, 1));
    let x = read_scl(&data("6-exact.scl")).unwrap();
    assert_eq!(x.degrees[0], ScaleDegree::Ratio(7, 6));
    assert_eq!(x.degrees[5], ScaleDegree::Ratio(2, 1));
    let c = read_scl(&data("carlos-alpha.scl")).unwrap();
    assert_eq!(c.description, "Carlos Alpha 78 cent scale");
    assert_eq!(c.degrees, vec![ScaleDegree::Cents(78.0)]);
    // Shuffled (non-monotonic): the file's order is kept.
    let sh = read_scl(&data("12-shuffled.scl")).unwrap();
    assert_eq!(sh.degrees.len(), 12);
    let cs = cents(&sh);
    assert!(cs.windows(2).any(|w| w[1] < w[0]), "kept as written");
    // Ableton's `.ascl` is a valid `.scl` (UTF-8, `@ABL` lines after the pitches).
    let r = read_scl(&data("rast.ascl")).unwrap();
    assert_eq!(r.degrees.len(), 12);
    let e31 = read_scl(&data("31-edo.ascl")).unwrap();
    assert_eq!(e31.degrees.len(), 31);
}

#[test]
fn surge_s_must_fail_set_fails_and_its_lenient_cases_load() {
    for f in [
        "bad/badnote.scl",
        "bad/blanknote.scl",
        "bad/missingnote.scl",
    ] {
        assert!(read_scl(&data(f)).is_err(), "{f} must fail");
    }
    // "Trailing data is OK."
    assert_eq!(
        read_scl(&data("bad/extraline.scl")).unwrap().degrees.len(),
        17
    );
    for f in [
        "bad/blank-line.kbm",
        "bad/empty-bad.kbm",
        "bad/garbage-key.kbm",
        "bad/missing-note.kbm",
    ] {
        assert!(read_kbm(&data(f)).is_err(), "{f} must fail");
    }
    for f in ["bad/empty-extra.kbm", "bad/extraline-long.kbm"] {
        read_kbm(&data(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    // Each refusal states its reason.
    let e = read_scl(&data("bad/missingnote.scl")).unwrap_err();
    assert!(
        matches!(
            e,
            ScalaError::FewerPitches {
                count: 17,
                found: 16
            }
        ),
        "{e}"
    );
    let e = read_kbm(&data("bad/missing-note.kbm")).unwrap_err();
    assert!(
        matches!(
            e,
            ScalaError::ShortMap {
                size: 12,
                found: 11
            }
        ),
        "{e}"
    );
    let e = read_kbm(&data("bad/empty-bad.kbm")).unwrap_err();
    assert!(e.to_string().contains("formal octave"), "{e}");
}

#[test]
fn surge_s_keyboard_maps_read() {
    let m = read_kbm(&data("mapping-whitekeys-c261.kbm")).unwrap();
    assert_eq!((m.size, m.middle_key, m.reference_key), (12, 60, 60));
    assert!((m.reference_hz - 261.625565280).abs() < 1e-9);
    assert_eq!(m.octave_degree, 12);
    let x = None;
    assert_eq!(
        m.mapping,
        vec![
            Some(0),
            x,
            Some(1),
            x,
            Some(2),
            Some(3),
            x,
            Some(4),
            x,
            Some(5),
            x,
            Some(6)
        ]
    );
    let a = read_kbm(&data("mapping-a442-7-to-12.kbm")).unwrap();
    assert_eq!(
        (a.middle_key, a.reference_key, a.octave_degree),
        (59, 69, 7)
    );
    assert_eq!(a.reference_hz, 442.0);
    let p = read_kbm(&data("piano.kbm")).unwrap();
    assert_eq!(
        (p.size, p.first_key, p.last_key, p.middle_key),
        (0, 21, 108, 21)
    );
    let k = read_kbm(&data("128.kbm")).unwrap();
    assert_eq!((k.size, k.middle_key, k.reference_key), (0, 0, 60));
    let e = read_kbm(&data("31-edo.kbm")).unwrap();
    assert_eq!((e.size, e.reference_key, e.octave_degree), (31, 83, 31));
    assert_eq!(e.mapping.len(), 31);
    // An integer frequency (`452`).
    let n = read_kbm(&data("empty-note69.kbm")).unwrap();
    assert_eq!((n.reference_key, n.reference_hz), (69, 452.0));
}

/// Our own fixture with every quirk the archive shows (§1.3), written from the facts — no
/// archive file is copied: CRLF, LF and a lone CR in one file, a Latin-1 byte, a comment
/// between the description and the count, a tab, a `cents` suffix, bare integers, a
/// trailing period, negative and non-ascending degrees, a ratio below 1/1, 25-digit
/// terms, comments after the pitches and no final newline.
fn quirks() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"! quirks.scl\r\n!\r\nGro\xDF: every quirk\r\n! a comment here\n 11 \r");
    b.extend_from_slice(b" -30.5\r\n 7/5\t\t\r\n 300.000 cents\r\n 600.\r\n 5\r\n 4/375\r\n");
    b.extend_from_slice(b"1709671705179880612640625/1208925819614629174706176\r\n");
    b.extend_from_slice(b"156348578434374084375/147573952589676412928\r\n");
    b.extend_from_slice(b" 15 \r\n -2320.15965\r\n 3/1\r\n! 2/1\r\n! an alternative, commented");
    b
}

#[test]
fn every_archive_quirk_reads_and_round_trips_to_the_same_cents() {
    let s = read_scl(&quirks()).unwrap();
    assert_eq!(s.description, "Groß: every quirk");
    assert_eq!(s.degrees.len(), 11);
    assert_eq!(s.degrees[0], ScaleDegree::Cents(-30.5));
    assert_eq!(s.degrees[1], ScaleDegree::Ratio(7, 5));
    assert_eq!(s.degrees[3], ScaleDegree::Cents(600.0));
    assert_eq!(s.degrees[4], ScaleDegree::Ratio(5, 1));
    assert_eq!(s.degrees[5], ScaleDegree::Ratio(4, 375));
    // Terms past 64 bits: the exact cents (both are 3^n·5^m / 2^k, coprime).
    let big = 1200.0 * (1709671705179880612640625f64 / 1208925819614629174706176f64).log2();
    assert!((s.degrees[6].cents() - big).abs() < 1e-9);
    assert_eq!(s.degrees[10], ScaleDegree::Ratio(3, 1), "the period is 3/1");
    let text = write_scl(&s);
    assert!(
        text.contains("\r\n 600.0\r\n"),
        "a cents value keeps its period"
    );
    let back = read_scl(&cantus_core::tuning_scala::latin1(&text)).unwrap();
    assert_eq!(back.description, s.description);
    for (a, b) in s.degrees.iter().zip(&back.degrees) {
        assert!((a.cents() - b.cents()).abs() < 1e-9, "{a:?} {b:?}");
    }
    // A count of zero is valid (the spec; Surge's reader refuses it).
    let z = read_scl(b"!\nnothing but 1/1\n0\n").unwrap();
    assert!(z.degrees.is_empty());
    // Ableton's files must be UTF-8.
    assert_eq!(
        read_scl(b"x\n1\n2/1\n! @ABL NOTE_NAMES \xFF\n"),
        Err(ScalaError::NotUtf8)
    );
}

#[test]
fn maps_round_trip() {
    for f in [
        "mapping-whitekeys-c261.kbm",
        "mapping-a442-7-to-12.kbm",
        "piano.kbm",
        "128.kbm",
        "31-edo.kbm",
        "empty-note69.kbm",
    ] {
        let m = read_kbm(&data(f)).unwrap();
        let back = read_kbm(write_kbm(&m).as_bytes()).unwrap();
        assert_eq!(back, m, "{f}");
    }
    // The reference key unmapped is an error (the Scala help).
    let bad = b"12\n0\n127\n60\n61\n440.0\n12\n0\nx\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n";
    assert_eq!(read_kbm(bad), Err(ScalaError::ReferenceUnmapped));
}
