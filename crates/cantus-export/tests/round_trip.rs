//! Every format written and read back with the repository's decoders (claxon, lewton,
//! hound; symphonia for MP3): the length, the level and the spectrum, the tags — and FLAC
//! and integer WAV to the bit.

mod common;

use cantus_export::process::quantise;
use cantus_export::{
    export_rendered, Channels, Control, Depth, Dither, ExportSettings, Format, Mp3Mode, Rate,
    Tags,
};
use common::*;

fn tags() -> Tags {
    Tags {
        title: "Aleluya".into(),
        artist: "Luis Ángel".into(),
        album: "Salmos".into(),
        year: "2026".into(),
        comment: "Colosenses 3:23".into(),
    }
}

fn settings(format: Format) -> ExportSettings {
    ExportSettings {
        format,
        rate: Rate::Session,
        ..Default::default()
    }
}

fn write(a: &cantus_export::Audio, s: &ExportSettings, name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    let ctl = Control::new();
    let done = export_rendered(a.clone(), s, &tags(), &path, &ctl).unwrap();
    assert_eq!(done.files, vec![path.clone()]);
    assert!(path.exists());
    assert!(
        std::fs::read_dir(dir.path()).unwrap().count() == 1,
        "nothing but the file is left in the folder"
    );
    (dir, path)
}

/// Level within `db` and every octave band from 63 Hz to 8 kHz within `band_db`.
fn assert_sounds_alike(src: &[f32], back: &[f32], ch: usize, rate: u32, db: f64, band_db: f64) {
    let (a, b) = (rms_db(src), rms_db(back));
    assert!((a - b).abs() < db, "level {a:.2} dB → {b:.2} dB");
    let (sa, sb) = (octave_bands(src, ch, rate), octave_bands(back, ch, rate));
    for (i, (x, y)) in sa.iter().zip(&sb).enumerate() {
        assert!((x - y).abs() < band_db, "band {i}: {x:.2} → {y:.2} dB");
    }
}

#[test]
fn flac_is_bit_exact_at_16_and_24_bits() {
    let a = music(48_000, 4.0);
    for (depth, dither) in [
        (Depth::Int24, Dither::Off),
        (Depth::Int24, Dither::Tpdf),
        (Depth::Int16, Dither::Tpdf),
        (Depth::Int16, Dither::Shaped),
    ] {
        let s = ExportSettings {
            depth,
            dither,
            ..settings(Format::Flac)
        };
        let (_d, path) = write(&a, &s, "song.flac");
        let (rate, ch, bits, ints, _) = decode_flac(&path);
        assert_eq!((rate, ch, bits), (48_000, 2, depth.bits() as u32));
        let expected = quantise(&a.samples, 2, depth.bits(), dither);
        assert_eq!(ints.len(), expected.len());
        assert!(ints == expected, "FLAC {depth:?} {dither:?} decodes to the same integers");
    }
}

#[test]
fn wav_is_exact_in_every_depth() {
    let a = music(44_100, 2.0);
    for depth in [Depth::Int16, Depth::Int24, Depth::Float32] {
        let s = ExportSettings {
            depth,
            dither: Dither::Tpdf,
            ..settings(Format::Wav)
        };
        let (_d, path) = write(&a, &s, "song.wav");
        let mut r = hound::WavReader::open(&path).unwrap();
        let spec = r.spec();
        assert_eq!((spec.sample_rate, spec.channels, spec.bits_per_sample), (44_100, 2, depth.bits()));
        if depth == Depth::Float32 {
            let back: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
            assert!(back == a.samples, "32-bit float is the engine's samples");
        } else {
            let back: Vec<i32> = r.samples::<i32>().map(|s| s.unwrap()).collect();
            assert!(back == quantise(&a.samples, 2, depth.bits(), Dither::Tpdf));
        }
    }
}

#[test]
fn mp3_comes_back_at_its_length_level_and_spectrum() {
    let a = music(44_100, 5.0);
    for mode in [Mp3Mode::Cbr(320), Mp3Mode::Cbr(128), Mp3Mode::Vbr(0), Mp3Mode::Vbr(4)] {
        let s = ExportSettings {
            mp3: mode,
            ..settings(Format::Mp3)
        };
        let (_d, path) = write(&a, &s, "song.mp3");
        let (rate, ch, back) = decode_mp3(&path);
        assert_eq!((rate, ch), (44_100, 2));
        assert_eq!(
            back.len(),
            a.samples.len(),
            "{mode:?}: gapless — the LAME tag's delay and padding give the exact length"
        );
        assert_sounds_alike(&a.samples, &back, 2, 44_100, 0.25, 1.0);
    }
}

#[test]
fn mono_mp3_at_48k_and_its_bitrate() {
    let a = music(48_000, 4.0);
    let s = ExportSettings {
        channels: Channels::Mono,
        mp3: Mp3Mode::Cbr(128),
        ..settings(Format::Mp3)
    };
    let (_d, path) = write(&a, &s, "talk.mp3");
    let (rate, ch, back) = decode_mp3(&path);
    assert_eq!((rate, ch), (48_000, 1));
    assert_eq!(back.len(), a.frames());
    let kbps = std::fs::metadata(&path).unwrap().len() as f64 * 8.0 / 4.0 / 1000.0;
    assert!((kbps - 128.0).abs() < 8.0, "{kbps:.1} kbps");
}

#[test]
fn ogg_comes_back_at_its_length_level_and_spectrum() {
    let a = music(48_000, 5.0);
    for q in [3, 6, 10] {
        let s = ExportSettings {
            ogg_quality: q,
            ..settings(Format::Ogg)
        };
        let (_d, path) = write(&a, &s, "song.ogg");
        let (rate, ch, back, _) = decode_ogg(&path);
        assert_eq!((rate, ch), (48_000, 2));
        assert_eq!(back.len(), a.samples.len(), "q{q}: the granule position trims the end");
        assert_sounds_alike(&a.samples, &back, 2, 48_000, 0.25, 1.0);
    }
}

#[test]
fn a_rate_change_keeps_the_length_and_the_sound() {
    let a = music(48_000, 3.0);
    let s = ExportSettings {
        rate: Rate::Hz(44_100),
        depth: Depth::Float32,
        ..settings(Format::Wav)
    };
    let (_d, path) = write(&a, &s, "song.wav");
    let mut r = hound::WavReader::open(&path).unwrap();
    assert_eq!(r.spec().sample_rate, 44_100);
    let back: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
    assert_eq!(back.len() / 2, 3 * 44_100);
    assert_sounds_alike(&a.samples, &back, 2, 44_100, 0.05, 0.2);
}

#[test]
fn an_mp3_at_a_rate_it_cannot_carry_takes_the_nearest_it_can() {
    let s = ExportSettings {
        rate: Rate::Hz(96_000),
        ..settings(Format::Mp3)
    };
    assert_eq!(s.file_rate(96_000), 48_000);
    assert_eq!(ExportSettings { rate: Rate::Hz(88_200), ..s.clone() }.file_rate(48_000), 44_100);
    assert_eq!(ExportSettings { rate: Rate::Session, ..s }.file_rate(44_100), 44_100);
}
