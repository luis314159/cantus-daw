//! The MusicXML round trip (ADR 0075): every golden passage written as MusicXML and read
//! back gives the notes it was written from — start, length, pitch and velocity — with
//! its metres, tempo and key; a played (unquantised) part reads back as it was read.

use cantus_score::build::{bars_between, build, Song};
use cantus_score::musicxml::{self, XmlOptions};
use cantus_score::musicxml_in;
use cantus_score::read::{self, SourceNote};
use cantus_score::samples::{self, Passage};

fn xml_of(p: &Passage) -> String {
    let song = Song {
        title: p.title.into(),
        composer: "Cantus tests".into(),
        tempo: &p.tempo,
        key: p.key,
    };
    let score = build(&song, &p.parts, None);
    musicxml::write(
        &score,
        &XmlOptions {
            software: "Cantus".into(),
            date: None,
        },
    )
}

fn key(n: &SourceNote) -> (i64, i64, u8, u8) {
    (n.start, n.len, n.pitch, n.velocity)
}

#[test]
fn every_passage_reads_back_as_written() {
    for p in samples::all() {
        let xml = xml_of(&p);
        let back = musicxml_in::read(&xml).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        assert_eq!(back.title, p.title, "{}", p.name);
        assert_eq!(back.composer, "Cantus tests");
        assert_eq!(back.parts.len(), p.parts.len(), "{}", p.name);
        for (want, got) in p.parts.iter().zip(&back.parts) {
            let mut w: Vec<_> = want.notes.iter().map(key).collect();
            w.sort();
            let mut g: Vec<_> = got
                .notes
                .iter()
                .map(|n| (n.start, n.len, n.pitch, n.velocity))
                .collect();
            g.sort();
            assert_eq!(g, w, "{}: the notes read back", p.name);
            assert_eq!(got.name, want.name);
        }
        // The metres and the tempo as the map holds them.
        let metres: Vec<_> = p
            .tempo
            .metres()
            .iter()
            .map(|m| (m.tick.0, m.signature))
            .collect();
        let got: Vec<_> = back.metres.clone();
        if metres.is_empty() {
            assert_eq!(got.len(), 1, "{}", p.name);
            assert_eq!(got[0].1, p.tempo.signature_at(cantus_core::Ticks(0)));
        } else {
            assert_eq!(got, metres, "{}", p.name);
        }
        let bpm0 = p.tempo.bpm_at(0.0);
        assert_eq!(back.tempo[0], (0, bpm0), "{}", p.name);
        for pt in p.tempo.points().iter().skip(1) {
            assert!(
                back.tempo.contains(&(pt.tick.0, pt.bpm)),
                "{}: {pt:?}",
                p.name
            );
        }
        let sig = cantus_score::spell::key_signature(p.key);
        assert_eq!(back.key, Some((sig.fifths, sig.mode)), "{}", p.name);
    }
}

#[test]
fn a_played_part_reads_back_as_it_was_read() {
    let p = samples::melody();
    // Played: every onset and end off by up to ±35 ticks, the velocities varied.
    let mut seed = 7u64;
    let mut jitter = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) % 71) as i64 - 35
    };
    let played: Vec<SourceNote> = p.parts[0]
        .notes
        .iter()
        .map(|n| SourceNote {
            start: (n.start + jitter()).max(0),
            len: n.len + jitter(),
            velocity: (n.velocity as i64 + jitter() / 2).clamp(1, 127) as u8,
            ..*n
        })
        .collect();
    let mut part = p.parts[0].clone();
    part.notes = played.clone();
    let passage = Passage {
        parts: vec![part],
        ..p
    };
    let xml = xml_of(&passage);
    let back = musicxml_in::read(&xml).unwrap();
    let last = played.iter().map(|n| n.start + n.len).max().unwrap();
    let bars = bars_between(&passage.tempo, 0, last);
    let (read, _) = read::read(&played, &bars, Default::default());
    let mut want: Vec<_> = read
        .iter()
        .map(|n| (n.start, n.end - n.start, n.pitch, n.velocity))
        .collect();
    want.sort();
    let mut got: Vec<_> = back.parts[0]
        .notes
        .iter()
        .map(|n| (n.start, n.len, n.pitch, n.velocity))
        .collect();
    got.sort();
    assert_eq!(got, want);
    // And reading put it back on the grid it was written on.
    let mut grid: Vec<_> = p_notes().iter().map(|n| (n.start, n.pitch)).collect();
    grid.sort();
    let mut onsets: Vec<_> = got.iter().map(|n| (n.0, n.2)).collect();
    onsets.sort();
    assert_eq!(onsets, grid);
}

fn p_notes() -> Vec<SourceNote> {
    samples::melody().parts[0].notes.clone()
}

#[test]
fn the_file_is_musicxml_4_partwise() {
    let xml = xml_of(&samples::piano());
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\""));
    assert!(xml.contains("<score-partwise version=\"4.0\">"));
    assert!(xml.contains("<staves>2</staves>"));
    assert!(xml.contains("<clef number=\"2\">"));
    // Balanced: every element opened is closed (a well-formed document).
    let doc = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .expect("well formed");
    // Every measure's voices fill it exactly.
    let div: i64 = doc
        .descendants()
        .find(|n| n.has_tag_name("divisions"))
        .and_then(|n| n.text())
        .and_then(|t| t.parse().ok())
        .unwrap();
    for m in doc.descendants().filter(|n| n.has_tag_name("measure")) {
        let mut at = 0i64;
        let mut most = 0i64;
        for el in m.children().filter(|c| c.is_element()) {
            let dur = el
                .children()
                .find(|c| c.has_tag_name("duration"))
                .and_then(|d| d.text())
                .and_then(|t| t.parse::<i64>().ok())
                .unwrap_or(0);
            match el.tag_name().name() {
                "note" if el.children().any(|c| c.has_tag_name("chord")) => {}
                "note" | "forward" => at += dur,
                "backup" => {
                    assert_eq!(at, most.max(at), "a backup from the measure's end");
                    at -= dur;
                }
                _ => {}
            }
            most = most.max(at);
        }
        assert_eq!(most, 4 * div, "measure {:?}", m.attribute("number"));
    }
}

/// Review, finding 6: a modal key is written as its mode (MusicXML 4's `dorian`,
/// `lydian`…) and read back as it.
#[test]
fn a_modal_key_keeps_its_mode() {
    use cantus_core::scale::{Key, ScaleKind};
    let mut p = samples::melody();
    p.key = Some(Key::new(2, ScaleKind::Dorian));
    let xml = xml_of(&p);
    assert!(xml.contains("<mode>dorian</mode>"), "the mode written");
    let back = musicxml_in::read(&xml).unwrap();
    assert_eq!(back.key, Some((0, cantus_score::model::KeyMode::Dorian)));
}

/// Review 2, finding 1: a second-voice note cut short by a later onset stays one tied
/// note — every tie that starts stops, and it reads back as the note it was.
#[test]
fn a_tie_through_a_cut_chord_stays_balanced() {
    let mut p = samples::melody();
    let note = |id: u64, start: i64, len: i64, pitch: u8| SourceNote {
        id,
        start,
        len,
        pitch,
        velocity: 80,
    };
    p.parts[0].notes = vec![
        note(1, 0, 3840, 72),
        note(2, 0, 1920, 64),
        note(3, 0, 960, 60),
        note(4, 480, 480, 55),
    ];
    let xml = xml_of(&p);
    let starts = xml.matches("<tie type=\"start\"/>").count();
    let stops = xml.matches("<tie type=\"stop\"/>").count();
    assert_eq!(starts, stops, "ties balanced");
    let back = musicxml_in::read(&xml).unwrap();
    let got: Vec<_> = back.parts[0]
        .notes
        .iter()
        .map(|n| (n.start, n.len, n.pitch))
        .collect();
    // The tied E4 reads back whole; the whole note and the later G3 as they were (a third
    // line on one staff — the C4 — is shortened where the G3 cuts it, as ADR 0075 says).
    for want in [(0, 1920, 64), (0, 3840, 72), (480, 480, 55)] {
        assert!(got.contains(&want), "{want:?} not in {got:?}");
    }
}
