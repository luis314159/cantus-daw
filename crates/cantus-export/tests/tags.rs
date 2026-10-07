//! The tags are written and read back by other people's readers: symphonia for MP3's
//! ID3v2 and WAV's INFO, claxon for FLAC's and lewton for Ogg's Vorbis comments. Every
//! file carries the Cantus credit.

mod common;

use cantus_export::tags::encoder_credit;
use cantus_export::{export_rendered, Control, ExportSettings, Format, Tags};
use common::*;

fn tags() -> Tags {
    Tags {
        title: "Canción nueva".into(),
        artist: "Luis Ángel Almazán".into(),
        album: "Salmos".into(),
        year: "2026".into(),
        comment: "Hecho con Cantus — 日本語も".into(),
    }
}

fn write(format: Format) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("t.{}", format.extension()));
    let s = ExportSettings {
        format,
        ..Default::default()
    };
    export_rendered(music(44_100, 1.0), &s, &tags(), &path, &Control::new()).unwrap();
    (dir, path)
}

fn has(list: &[(String, String)], key: &str, value: &str) -> bool {
    list.iter()
        .any(|(k, v)| k.eq_ignore_ascii_case(key) && v == value)
}

#[test]
fn mp3_tags_read_back() {
    let (_d, path) = write(Format::Mp3);
    let t = symphonia_tags(&path, "mp3");
    let want = tags();
    assert!(has(&t, "TrackTitle", &want.title), "{t:?}");
    assert!(has(&t, "Artist", &want.artist), "{t:?}");
    assert!(has(&t, "Album", &want.album), "{t:?}");
    assert!(has(&t, "Date", &want.year), "{t:?}");
    assert!(has(&t, "Comment", &want.comment), "{t:?}");
    assert!(has(&t, "Encoder", &encoder_credit()), "{t:?}");
}

#[test]
fn wav_tags_read_back() {
    let (_d, path) = write(Format::Wav);
    let t = symphonia_tags(&path, "wav");
    let want = tags();
    for (k, v) in [
        ("TrackTitle", &want.title),
        ("Artist", &want.artist),
        ("Album", &want.album),
        ("Date", &want.year),
        ("Comment", &want.comment),
    ] {
        assert!(has(&t, k, v), "{k}: {t:?}");
    }
    assert!(has(&t, "Encoder", &encoder_credit()), "{t:?}");
    // And the samples are still where every reader expects them.
    let r = hound::WavReader::open(&path).unwrap();
    assert_eq!(r.duration(), 44_100);
}

#[test]
fn flac_tags_read_back() {
    let (_d, path) = write(Format::Flac);
    let (.., t) = decode_flac(&path);
    let want = tags();
    for (k, v) in [
        ("TITLE", &want.title),
        ("ARTIST", &want.artist),
        ("ALBUM", &want.album),
        ("DATE", &want.year),
        ("COMMENT", &want.comment),
    ] {
        assert!(has(&t, k, v), "{k}: {t:?}");
    }
    assert!(has(&t, "ENCODER", &encoder_credit()), "{t:?}");
}

#[test]
fn ogg_tags_read_back() {
    let (_d, path) = write(Format::Ogg);
    let (.., t) = decode_ogg(&path);
    let want = tags();
    for (k, v) in [
        ("TITLE", &want.title),
        ("ARTIST", &want.artist),
        ("ALBUM", &want.album),
        ("DATE", &want.year),
        ("COMMENT", &want.comment),
    ] {
        assert!(has(&t, k, v), "{k}: {t:?}");
    }
    assert!(has(&t, "ENCODER", &encoder_credit()), "{t:?}");
}

#[test]
fn empty_tags_are_left_out_and_the_credit_stays() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bare.mp3");
    let s = ExportSettings::default();
    export_rendered(music(44_100, 1.0), &s, &Tags::default(), &path, &Control::new()).unwrap();
    let t = symphonia_tags(&path, "mp3");
    assert_eq!(t.len(), 1, "only the credit: {t:?}");
    assert!(has(&t, "Encoder", &encoder_credit()));
}
