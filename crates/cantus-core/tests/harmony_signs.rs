//! The music signs in chord symbols: read in what an agent writes, never shown. These tests
//! live outside `src/` because `cantus-ui`'s `font_coverage` reads `src/` for text the
//! interface can draw, and the interface fonts have no flat, sharp or triangle (round 34).

use cantus_core::harmony::{parse_chord, spell};

/// The pitch classes of a chord, sorted.
fn pcs(sym: &str) -> Vec<u8> {
    let c = parse_chord(sym).unwrap_or_else(|e| panic!("{e}"));
    let mut v: Vec<u8> = c.tones.iter().map(|t| (c.root + t.interval) % 12).collect();
    v.sort_unstable();
    v
}

#[test]
fn the_signs_read_as_a_lead_sheet_writes_them() {
    assert_eq!(pcs("C\u{394}"), vec![0, 4, 7, 11]);
    assert_eq!(parse_chord("E\u{266d}").unwrap().root, 3);
}

/// Round 34 (`font_coverage`): the interface fonts have no music flat, sharp or triangle, so
/// what Cantus says of a chord spells them `b`, `#` and `maj7` — while the parser still reads
/// them in what an agent writes.
#[test]
fn a_chord_is_shown_in_the_fonts_glyphs_and_read_in_any() {
    assert_eq!(spell("E\u{266d}\u{394}/G"), "Ebmaj7/G");
    assert_eq!(spell("C\u{394}9"), "Cmaj9");
    assert_eq!(spell("C\u{2206}"), "Cmaj7");
    assert_eq!(spell("F\u{266f}m7\u{266d}5"), "F#m7b5");
    assert_eq!(spell("Ebmaj7/G"), "Ebmaj7/G");
    for (music, plain) in [
        ("E\u{266d}\u{394}", "Ebmaj7"),
        ("C\u{394}9", "Cmaj9"),
        ("C\u{2206}", "Cmaj7"),
        ("F\u{266f}m7\u{266d}5", "F#m7b5"),
        ("C7\u{266f}9", "C7#9"),
        ("C7\u{266d}9", "C7b9"),
    ] {
        let (a, b) = (parse_chord(music).unwrap(), parse_chord(plain).unwrap());
        assert_eq!(
            (a.mask(), a.root, a.bass),
            (b.mask(), b.root, b.bass),
            "{music}"
        );
    }
    let e = parse_chord("E\u{266d}\u{394}x\u{266f}")
        .unwrap_err()
        .to_string();
    assert!(
        !e.contains(['\u{266d}', '\u{266f}', '\u{394}', '\u{2206}']),
        "{e}"
    );
    assert!(e.contains("Ebmaj7x#"), "the symbol quoted, spelled: {e}");
}
