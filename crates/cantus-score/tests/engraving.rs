//! The engraving's golden tests (ADR 0075): each passage — a melody, a two-hand piano
//! piece, triplets, syncopation, long tied notes, a metre change — engraved wrapped to a
//! pane's width and on A4, its display list compared line by line with the blessed one
//! (`CANTUS_SCORE_BLESS=1` writes them anew, on purpose, in the commit that means it), and
//! held to rules a good engraving keeps whatever the golden says: nothing collides,
//! every stem is long enough, every note is drawn once.

use cantus_score::build::{build, Song};
use cantus_score::display::{DisplayList, Item};
use cantus_score::layout::{engrave, Engraved, Format, LayoutOptions, Paper};
use cantus_score::text::{TextFaces, TextMeasure};
use cantus_score::{samples, Glyph};
use std::path::PathBuf;

static REGULAR: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-SemiBold.ttf");
static ITALIC: &[u8] =
    include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSansCondensed-Italic.ttf");

fn measure() -> TextMeasure {
    TextMeasure::new(Some(TextFaces {
        regular: REGULAR,
        bold: BOLD,
        italic: ITALIC,
    }))
}

fn engraved(p: &samples::Passage, format: Format) -> Engraved {
    let song = Song {
        title: p.title.into(),
        composer: "Cantus".into(),
        tempo: &p.tempo,
        key: p.key,
    };
    let score = build(&song, &p.parts, None);
    engrave(
        &score,
        &LayoutOptions {
            format,
            text: measure(),
            header: matches!(format, Format::Pages(_)),
        },
    )
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

#[test]
fn every_passage_matches_its_golden() {
    let bless = std::env::var_os("CANTUS_SCORE_BLESS").is_some();
    let mut failed = Vec::new();
    for p in samples::all() {
        for (kind, format) in [
            ("wrap", Format::Wrap { width: 120.0 }),
            ("a4", Format::Pages(Paper::A4)),
        ] {
            let got = engraved(&p, format).dump();
            let path = golden_dir().join(format!("{}-{kind}.txt", p.name));
            if bless {
                std::fs::create_dir_all(golden_dir()).unwrap();
                std::fs::write(&path, &got).unwrap();
                continue;
            }
            let want = std::fs::read_to_string(&path)
                .unwrap_or_else(|_| {
                    panic!(
                        "{} missing: bless with CANTUS_SCORE_BLESS=1",
                        path.display()
                    )
                })
                .replace("\r\n", "\n");
            if want != got {
                let line = want
                    .lines()
                    .zip(got.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(want.lines().count().min(got.lines().count()));
                failed.push(format!(
                    "{}: first difference at line {}\n  want {:?}\n  got  {:?}",
                    path.display(),
                    line + 1,
                    want.lines().nth(line),
                    got.lines().nth(line)
                ));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

fn is_head(g: Glyph) -> bool {
    matches!(
        g,
        Glyph::NoteheadBlack
            | Glyph::NoteheadHalf
            | Glyph::NoteheadWhole
            | Glyph::NoteheadDoubleWhole
    )
}

fn is_accidental(g: Glyph) -> bool {
    matches!(
        g,
        Glyph::AccidentalFlat
            | Glyph::AccidentalSharp
            | Glyph::AccidentalNatural
            | Glyph::AccidentalDoubleFlat
            | Glyph::AccidentalDoubleSharp
            | Glyph::AccidentalParensLeft
            | Glyph::AccidentalParensRight
    )
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    w * h
}

fn glyphs(list: &DisplayList) -> Vec<(Glyph, [f32; 4], Option<u64>)> {
    list.items
        .iter()
        .filter_map(|i| match i {
            Item::Glyph { glyph, owner, .. } => Some((*glyph, i.bounds(), *owner)),
            _ => None,
        })
        .collect()
}

#[test]
fn nothing_collides() {
    for p in samples::all() {
        for format in [
            Format::Wrap { width: 120.0 },
            Format::Pages(Paper::A4),
            Format::Continuous,
        ] {
            let e = engraved(&p, format);
            for page in &e.pages {
                let gs = glyphs(&page.list);
                for (i, a) in gs.iter().enumerate() {
                    for b in &gs[i + 1..] {
                        // An accidental never touches a notehead or another accidental of
                        // a different note.
                        let clash = (is_accidental(a.0) && is_head(b.0))
                            || (is_head(a.0) && is_accidental(b.0))
                            || (is_accidental(a.0) && is_accidental(b.0) && a.2 != b.2);
                        // Noteheads of different notes never overlap (a second's heads sit
                        // side by side).
                        let heads = is_head(a.0) && is_head(b.0) && a.2 != b.2;
                        if (clash || heads) && overlap(a.1, b.1) > 0.02 {
                            panic!(
                                "{}: {:?} {:?} overlaps {:?} {:?}",
                                p.name, a.0, a.1, b.0, b.1
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_note_is_drawn_and_stems_are_long_enough() {
    for p in samples::all() {
        let e = engraved(&p, Format::Wrap { width: 120.0 });
        let list = &e.pages[0].list;
        for n in &p.parts[0].notes {
            assert!(
                list.items.iter().any(|i| matches!(i, Item::Glyph { glyph, owner: Some(id), .. } if is_head(*glyph) && *id == n.id)),
                "{}: note {} has no head",
                p.name,
                n.id
            );
        }
        // Every stem is at least 2.5 spaces (a beamed group's shortest) and at most 9.
        for it in &list.items {
            if let Item::Line {
                x0,
                y0,
                x1,
                y1,
                w,
                owner: Some(_),
            } = it
            {
                if (x0 - x1).abs() < 1e-4 && (*w - 0.12).abs() < 1e-4 {
                    let len = (y1 - y0).abs();
                    assert!(
                        (2.5..9.0).contains(&len),
                        "{}: a stem {len} long at {x0}",
                        p.name
                    );
                }
            }
        }
    }
}

#[test]
fn the_page_and_the_maps_agree() {
    let p = samples::piano();
    let e = engraved(&p, Format::Pages(Paper::A4));
    assert_eq!(e.pages.len(), 1);
    let page = &e.pages[0];
    let (w, h, _) = Paper::A4.spaces();
    assert_eq!((page.list.width, page.list.height), (w, h));
    // Every bar's downbeat has a place, in order along each system.
    for s in &page.systems {
        assert!(s
            .ticks
            .windows(2)
            .all(|t| t[0].0 < t[1].0 && t[0].1 <= t[1].1));
        assert_eq!(s.staff_tops.len(), 2);
        let mid = (s.ticks[0].0 + s.ticks[s.ticks.len() - 1].0) / 2;
        let x = s.x_of(mid).unwrap();
        let back = s.tick_at(x).unwrap();
        assert!((back - mid).abs() <= 1, "{mid} → {x} → {back}");
    }
    assert!(e.locate(0).is_some());
    // Nothing is drawn off the paper.
    for it in &page.list.items {
        let b = it.bounds();
        assert!(
            b[0] >= 0.0 && b[2] <= w && b[1] >= 0.0 && b[3] <= h,
            "{it:?}"
        );
    }
}

/// Review, finding 8: a note in the last thirty-second of the part's last bar is read
/// onto that bar, never past its end (where no staff shows it).
#[test]
fn a_note_at_the_very_end_of_the_last_bar_is_drawn() {
    use cantus_score::read::SourceNote;
    let p = samples::melody();
    let mut part = p.parts[0].clone();
    part.notes = vec![
        SourceNote {
            id: 1,
            start: 0,
            len: 960,
            pitch: 67,
            velocity: 80,
        },
        SourceNote {
            id: 2,
            start: 3790,
            len: 50,
            pitch: 69,
            velocity: 80,
        },
    ];
    let song = Song {
        title: String::new(),
        composer: String::new(),
        tempo: &p.tempo,
        key: p.key,
    };
    let score = build(&song, &[part], Some((0, 3840)));
    let e = engrave(
        &score,
        &LayoutOptions {
            format: Format::Wrap { width: 120.0 },
            text: measure(),
            header: false,
        },
    );
    let heads = e.pages[0]
        .list
        .items
        .iter()
        .filter(|i| matches!(i, Item::Glyph { glyph, owner: Some(_), .. } if is_head(*glyph)))
        .count();
    assert_eq!(heads, 2);
}
