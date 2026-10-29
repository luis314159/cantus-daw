//! The PDF (ADR 0075): written for A4 and Letter, opened back with an independent reader
//! (lopdf) — the pages and their size, every font embedded as a tagged subset (Bravura's
//! CFF, the text faces' TrueType), the title and composer in its info, the words found
//! again through the ToUnicode maps — and many pages for a long part.

use cantus_score::build::{build, Song};
use cantus_score::layout::{engrave, Format, LayoutOptions, Paper};
use cantus_score::pdf;
use cantus_score::samples;
use cantus_score::text::{TextFaces, TextMeasure};
use lopdf::{Document, Object};

static REGULAR: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-SemiBold.ttf");
static ITALIC: &[u8] =
    include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSansCondensed-Italic.ttf");

fn faces() -> TextFaces {
    TextFaces {
        regular: REGULAR,
        bold: BOLD,
        italic: ITALIC,
    }
}

fn make(p: &samples::Passage, paper: Paper, repeat: usize) -> Vec<u8> {
    let mut parts = p.parts.clone();
    // A longer part: the passage again and again, one after another.
    let span = parts[0]
        .notes
        .iter()
        .map(|n| n.start + n.len)
        .max()
        .unwrap_or(0);
    let span = (span + 3839) / 3840 * 3840;
    let one = parts[0].notes.clone();
    for r in 1..repeat {
        parts[0]
            .notes
            .extend(one.iter().map(|n| cantus_score::SourceNote {
                id: n.id + 10_000 * r as u64,
                start: n.start + span * r as i64,
                ..*n
            }));
    }
    let song = Song {
        title: p.title.into(),
        composer: "Ana Compositora".into(),
        tempo: &p.tempo,
        key: p.key,
    };
    let score = build(&song, &parts, None);
    let e = engrave(
        &score,
        &LayoutOptions {
            format: Format::Pages(paper),
            text: TextMeasure::new(Some(faces())),
            header: true,
        },
    );
    pdf::write(&e, &paper, faces(), &score.title, &score.composer).expect("a PDF")
}

fn name(o: &Object) -> String {
    match o {
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    }
}

#[test]
fn an_a4_page_with_every_font_embedded_and_subset() {
    let bytes = make(&samples::piano(), Paper::A4, 1);
    assert!(bytes.starts_with(b"%PDF-1."));
    let doc = Document::load_mem(&bytes).expect("lopdf reads it");
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 1);
    let page = doc.get_dictionary(pages[&1]).unwrap();
    let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
    let w = mb[2].as_float().unwrap();
    let h = mb[3].as_float().unwrap();
    assert!(
        (w - 595.28).abs() < 0.1 && (h - 841.89).abs() < 0.1,
        "{w} × {h}"
    );
    let fonts = doc.get_page_fonts(pages[&1]).unwrap();
    assert_eq!(fonts.len(), 4);
    let mut embedded = 0;
    for f in fonts.values() {
        assert_eq!(name(f.get(b"Subtype").unwrap()), "Type0");
        let base = name(f.get(b"BaseFont").unwrap());
        assert!(
            base.len() > 7 && base.as_bytes()[6] == b'+',
            "{base} is a tagged subset"
        );
        let desc_kids = f.get(b"DescendantFonts").unwrap().as_array().unwrap();
        let cid = doc
            .get_dictionary(desc_kids[0].as_reference().unwrap())
            .unwrap();
        let desc = doc
            .get_dictionary(cid.get(b"FontDescriptor").unwrap().as_reference().unwrap())
            .unwrap();
        let file = desc
            .get(b"FontFile3")
            .or_else(|_| desc.get(b"FontFile2"))
            .expect("the font's program is in the file");
        let stream = doc
            .get_object(file.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        let data = stream.decompressed_content().unwrap();
        // A subset: far smaller than the face it came from.
        let whole = if base.ends_with("Bravura") {
            cantus_score::BRAVURA.len()
        } else {
            REGULAR.len()
        };
        assert!(
            data.len() * 4 < whole,
            "{base}: {} of {whole} bytes",
            data.len()
        );
        assert!(f.get(b"ToUnicode").is_ok());
        embedded += 1;
    }
    assert_eq!(embedded, 4);
    let info = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = doc.get_dictionary(info).unwrap();
    let title = info.get(b"Title").unwrap();
    assert!(matches!(title, Object::String(..)));
    let text = doc.extract_text(&[1]).unwrap_or_default();
    assert!(text.contains("Sonatina"), "{text:?}");
    assert!(text.contains("Ana Compositora"), "{text:?}");
}

#[test]
fn letter_and_many_pages() {
    let bytes = make(&samples::melody(), Paper::LETTER, 24);
    let doc = Document::load_mem(&bytes).expect("lopdf reads it");
    let pages = doc.get_pages();
    assert!(pages.len() >= 2, "{} pages", pages.len());
    for (_, id) in pages {
        let page = doc.get_dictionary(id).unwrap();
        let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
        assert!((mb[2].as_float().unwrap() - 612.0).abs() < 0.1);
        assert!((mb[3].as_float().unwrap() - 792.0).abs() < 0.1);
    }
}

/// Review, finding 1: the filled shapes go into the page as one nonzero path, so they
/// must all wind the same way — else where a beam crosses a staff line (or a stem meets a
/// beam, or a tie overlaps a line) the two cancel and leave a white stripe.
#[test]
fn every_filled_shape_winds_the_same_way_and_overlaps_stay_black() {
    let p = samples::piano();
    let song = Song {
        title: p.title.into(),
        composer: String::new(),
        tempo: &p.tempo,
        key: p.key,
    };
    let score = build(&song, &p.parts, None);
    let e = engrave(
        &score,
        &LayoutOptions {
            format: Format::Pages(Paper::A4),
            text: TextMeasure::new(Some(faces())),
            header: true,
        },
    );
    let list = &e.pages[0].list;
    let polys = pdf::fill_polygons(list);
    let area = |p: &[[f32; 2]]| {
        let mut a = 0.0;
        for i in 0..p.len() {
            let (q, r) = (p[i], p[(i + 1) % p.len()]);
            a += q[0] * r[1] - r[0] * q[1];
        }
        a / 2.0
    };
    let signs: Vec<f32> = polys
        .iter()
        .map(|p| area(p).signum())
        .filter(|s| *s != 0.0)
        .collect();
    assert!(!signs.is_empty());
    assert!(signs.iter().all(|s| *s == signs[0]), "mixed windings");
    // Rasterised as one nonzero path, a beam over a staff line stays black there.
    let k = 20.0;
    let (w, h) = ((list.width * k) as u32, (list.height * k) as u32);
    let mut pm = tiny_skia::Pixmap::new(w, h).unwrap();
    pm.fill(tiny_skia::Color::WHITE);
    let mut pb = tiny_skia::PathBuilder::new();
    for c in &polys {
        pb.move_to(c[0][0] * k, c[0][1] * k);
        for q in &c[1..] {
            pb.line_to(q[0] * k, q[1] * k);
        }
        pb.close();
    }
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(tiny_skia::Color::BLACK);
    pm.fill_path(
        &pb.finish().unwrap(),
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
    // A beam's middle and a horizontal staff line under it.
    let mut checked = 0;
    for it in &list.items {
        if let cantus_score::Item::Beam {
            x0, y0, x1, y1, h, ..
        } = it
        {
            let x = (x0 + x1) / 2.0;
            let y = (y0 + y1) / 2.0 + h / 2.0;
            let crosses = list.items.iter().any(|l| {
                matches!(l, cantus_score::Item::Line { x0: a, y0: ly, x1: b, y1: ly2, w, .. }
                    if (ly - ly2).abs() < 1e-4 && *a <= x && *b >= x && (ly - y).abs() < h / 2.0 - w)
            });
            if crosses {
                let px = pm.pixel((x * k) as u32, (y * k) as u32).unwrap();
                assert!(
                    px.red() < 60,
                    "white where a beam crosses a staff line at ({x}, {y})"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no beam crosses a staff line in the passage");
}
