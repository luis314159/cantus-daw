//! Every choice of the export dialog against the file it writes (round 23, BACKLOG §1):
//! for each option the written file is opened with an independent decoder and its
//! format, rate, channels, sample format, length and bitrate are compared with what the
//! dialog said. `round_trip.rs` checks the sound; this checks that each choice lands.

mod common;

use cantus_export::{
    export_rendered, Audio, Channels, Control, Depth, Dither, ExportSettings, Format, Mp3Mode,
    Preset, Rate, Scope, Tags, MP3_BITRATES, MP3_VBR, OGG_QUALITIES, RATES,
};
use std::path::{Path, PathBuf};

/// What a file holds, read back.
#[derive(Debug, PartialEq)]
struct Probe {
    rate: u32,
    channels: usize,
    /// Integer bits for WAV/FLAC, 32 for a float WAV, `None` for MP3 and Ogg.
    bits: Option<u32>,
    frames: usize,
}

fn probe(path: &Path, format: Format) -> Probe {
    let head = std::fs::read(path).unwrap();
    match format {
        Format::Wav => {
            assert_eq!(&head[..4], b"RIFF");
            assert_eq!(&head[8..12], b"WAVE");
            let r = hound::WavReader::open(path).unwrap();
            let spec = r.spec();
            Probe {
                rate: spec.sample_rate,
                channels: spec.channels as usize,
                bits: Some(spec.bits_per_sample as u32),
                frames: r.duration() as usize,
            }
        }
        Format::Flac => {
            assert_eq!(&head[..4], b"fLaC");
            let (rate, channels, bits, samples, _) = common::decode_flac(path);
            Probe {
                rate,
                channels,
                bits: Some(bits),
                frames: samples.len() / channels,
            }
        }
        Format::Mp3 => {
            assert_eq!(&head[..3], b"ID3", "the tags come first");
            let (rate, channels, samples) = common::decode_mp3(path);
            Probe {
                rate,
                channels,
                bits: None,
                frames: samples.len() / channels.max(1),
            }
        }
        Format::Ogg => {
            assert_eq!(&head[..4], b"OggS");
            let (rate, channels, _, _) = common::decode_ogg(path);
            Probe {
                rate,
                channels,
                bits: None,
                frames: ogg_length(&head),
            }
        }
    }
}

/// An Ogg Vorbis file's length in frames: the granule position of its last page — what
/// the format says the stream holds, the end trimmed to it (a decoder may hand back the
/// whole last block).
fn ogg_length(bytes: &[u8]) -> usize {
    let mut at = 0;
    let mut granule = 0u64;
    while at + 27 <= bytes.len() && &bytes[at..at + 4] == b"OggS" {
        granule = u64::from_le_bytes(bytes[at + 6..at + 14].try_into().unwrap());
        let segments = bytes[at + 26] as usize;
        let body: usize = bytes[at + 27..at + 27 + segments]
            .iter()
            .map(|b| *b as usize)
            .sum();
        at += 27 + segments + body;
    }
    assert_eq!(at, bytes.len(), "the pages cover the file");
    granule as usize
}

/// What the dialog's choices promise for `input`.
fn promised(s: &ExportSettings, input: &Audio) -> Probe {
    let rate = s.file_rate(input.rate);
    let bits = match s.format {
        Format::Wav => Some(s.depth.bits() as u32),
        Format::Flac => Some(s.flac_depth().bits() as u32),
        _ => None,
    };
    let frames = if rate == input.rate {
        input.frames()
    } else {
        (input.frames() as u128 * rate as u128).div_ceil(input.rate as u128) as usize
    };
    Probe {
        rate,
        channels: s.channel_count() as usize,
        bits,
        frames,
    }
}

fn write(a: &Audio, s: &ExportSettings, dir: &Path) -> PathBuf {
    let path = dir.join(format!("song.{}", s.format.extension()));
    let done = export_rendered(a.clone(), s, &Tags::default(), &path, &Control::new()).unwrap();
    assert_eq!(done.files, vec![path.clone()]);
    path
}

fn check(a: &Audio, s: &ExportSettings) {
    let dir = tempfile::tempdir().unwrap();
    let path = write(a, s, dir.path());
    assert_eq!(probe(&path, s.format), promised(s, a), "{s:?}");
}

#[test]
fn every_format_rate_and_channel_choice_is_what_the_file_holds() {
    let a = common::music(48_000, 0.6);
    let rates = std::iter::once(Rate::Session).chain(RATES.into_iter().map(Rate::Hz));
    for rate in rates {
        for format in Format::ALL {
            for channels in [Channels::Stereo, Channels::Mono] {
                check(
                    &a,
                    &ExportSettings {
                        format,
                        rate,
                        channels,
                        ..Default::default()
                    },
                );
            }
        }
    }
}

/// The rate policy itself, in numbers (the dialog shows it in its "written at" note).
#[test]
fn each_format_writes_the_rate_it_can_carry() {
    let at = |format, rate, session| {
        ExportSettings {
            format,
            rate,
            ..Default::default()
        }
        .file_rate(session)
    };
    // FLAC: 96 kHz at most, the same family under it.
    assert_eq!(at(Format::Flac, Rate::Hz(192_000), 48_000), 96_000);
    assert_eq!(at(Format::Flac, Rate::Session, 176_400), 88_200);
    assert_eq!(at(Format::Flac, Rate::Hz(96_000), 48_000), 96_000);
    assert_eq!(at(Format::Flac, Rate::Hz(44_100), 48_000), 44_100);
    // WAV and Ogg: what is asked.
    assert_eq!(at(Format::Wav, Rate::Hz(192_000), 48_000), 192_000);
    assert_eq!(at(Format::Ogg, Rate::Hz(192_000), 48_000), 192_000);
    assert_eq!(at(Format::Wav, Rate::Session, 44_100), 44_100);
    // MP3: its three rates, the family kept.
    assert_eq!(at(Format::Mp3, Rate::Hz(96_000), 48_000), 48_000);
    assert_eq!(at(Format::Mp3, Rate::Hz(88_200), 48_000), 44_100);
    assert!(!Format::Flac.carries(192_000) && Format::Flac.carries(96_000));
}

#[test]
fn every_wav_and_flac_depth_is_what_the_file_holds() {
    let a = common::music(44_100, 0.5);
    for format in [Format::Wav, Format::Flac] {
        for depth in [Depth::Int16, Depth::Int24, Depth::Float32] {
            let s = ExportSettings {
                format,
                depth,
                ..Default::default()
            };
            check(&a, &s);
            if format == Format::Wav {
                // The fmt chunk's tag: PCM for integers, IEEE float for 32-bit float.
                let dir = tempfile::tempdir().unwrap();
                let bytes = std::fs::read(write(&a, &s, dir.path())).unwrap();
                let tag = u16::from_le_bytes([bytes[20], bytes[21]]);
                let want = if depth == Depth::Float32 { 3 } else { 1 };
                assert!(tag == want || tag == 0xFFFE, "{depth:?}: format tag {tag}");
            }
        }
    }
}

/// The MPEG-1 Layer III frames of an MP3 after its ID3v2 tag: (bitrate kbps, mono).
fn mp3_frames(path: &Path) -> Vec<(u32, bool)> {
    const KBPS: [u32; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    let b = std::fs::read(path).unwrap();
    let size = |i: usize| b[i] as usize & 0x7f;
    let mut at = 10 + (size(6) << 21 | size(7) << 14 | size(8) << 7 | size(9));
    let mut frames = Vec::new();
    while at + 4 <= b.len() {
        let h = u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        if h >> 21 != 0x7ff {
            break;
        }
        assert_eq!((h >> 19) & 3, 3, "MPEG-1");
        assert_eq!((h >> 17) & 3, 1, "Layer III");
        let index = ((h >> 12) & 0xf) as usize;
        let rate = [44_100, 48_000, 32_000][((h >> 10) & 3) as usize];
        let padding = ((h >> 9) & 1) as usize;
        let kbps = KBPS[index];
        frames.push((kbps, (h >> 6) & 3 == 3));
        at += 144_000 * kbps as usize / rate + padding;
    }
    frames
}

#[test]
fn every_mp3_bitrate_is_the_bitrate_of_every_frame() {
    let a = common::music(44_100, 1.0);
    let dir = tempfile::tempdir().unwrap();
    for k in MP3_BITRATES {
        for channels in [Channels::Stereo, Channels::Mono] {
            let s = ExportSettings {
                format: Format::Mp3,
                mp3: Mp3Mode::Cbr(k),
                channels,
                ..Default::default()
            };
            let frames = mp3_frames(&write(&a, &s, dir.path()));
            assert!(frames.len() > 30, "{k}: {} frames", frames.len());
            // The first frame carries LAME's Info tag; the music is in the rest.
            for (i, (kbps, mono)) in frames.iter().enumerate().skip(1) {
                assert_eq!(*kbps, k as u32, "frame {i} of the {k} kbps file");
                assert_eq!(*mono, channels == Channels::Mono, "frame {i}");
            }
        }
    }
}

#[test]
fn every_mp3_vbr_level_is_variable_and_smaller_as_the_level_rises() {
    let a = common::music(44_100, 3.0);
    let dir = tempfile::tempdir().unwrap();
    let mut sizes = Vec::new();
    for v in MP3_VBR {
        let s = ExportSettings {
            format: Format::Mp3,
            mp3: Mp3Mode::Vbr(v),
            ..Default::default()
        };
        let path = write(&a, &s, dir.path());
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            bytes.windows(4).take(4096).any(|w| w == b"Xing"),
            "V{v}: a VBR file carries a Xing tag"
        );
        let rates: std::collections::HashSet<u32> =
            mp3_frames(&path).iter().skip(1).map(|f| f.0).collect();
        assert!(rates.len() > 1, "V{v}: the bitrate varies ({rates:?})");
        sizes.push(bytes.len());
    }
    assert!(
        sizes.windows(2).all(|w| w[0] > w[1]),
        "V0 > V2 > V4 > V6 in size: {sizes:?}"
    );
}

#[test]
fn every_ogg_quality_is_larger_as_the_quality_rises() {
    let a = common::music(48_000, 3.0);
    let dir = tempfile::tempdir().unwrap();
    let mut sizes = Vec::new();
    for q in OGG_QUALITIES {
        let s = ExportSettings {
            format: Format::Ogg,
            ogg_quality: q,
            ..Default::default()
        };
        check(&a, &s);
        sizes.push(std::fs::metadata(write(&a, &s, dir.path())).unwrap().len());
    }
    assert!(
        sizes.windows(2).all(|w| w[0] < w[1]),
        "q3 < q4 < … < q10 in size: {sizes:?}"
    );
}

/// A sine at −110 dBFS: under half a 16-bit step, so without dither it is all zeros.
fn whisper() -> Audio {
    let n = 48_000;
    let amp = cantus_dsp::math::powf(10.0, -110.0 / 20.0);
    let mut samples = Vec::with_capacity(n * 2);
    for i in 0..n {
        let v =
            amp * cantus_dsp::math::sin(i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / 48_000.0);
        samples.extend([v, v]);
    }
    Audio {
        rate: 48_000,
        channels: 2,
        samples,
    }
}

fn ints(path: &Path, format: Format) -> Vec<i32> {
    match format {
        Format::Wav => hound::WavReader::open(path)
            .unwrap()
            .samples::<i32>()
            .map(|s| s.unwrap())
            .collect(),
        _ => common::decode_flac(path).3,
    }
}

/// A 16-bit file's noise in the 4 kHz octave, dB: the ear's most sensitive region, where
/// noise shaping takes the dither's noise away from (and puts it higher up).
fn noise_at_4k(x: &[i32]) -> f64 {
    let left: Vec<f32> = x.iter().step_by(2).map(|v| *v as f32).collect();
    common::octave_bands(&left, 1, 48_000)[6]
}

#[test]
fn every_dither_choice_reaches_the_16_bit_file() {
    let a = whisper();
    let dir = tempfile::tempdir().unwrap();
    for format in [Format::Wav, Format::Flac] {
        let of = |dither| {
            let s = ExportSettings {
                format,
                depth: Depth::Int16,
                dither,
                ..Default::default()
            };
            ints(&write(&a, &s, dir.path()), format)
        };
        let (off, tpdf, shaped) = (of(Dither::Off), of(Dither::Tpdf), of(Dither::Shaped));
        assert!(
            off.iter().all(|v| *v == 0),
            "{format:?}: undithered, it rounds to silence"
        );
        assert!(
            tpdf.iter().any(|v| *v != 0),
            "{format:?}: TPDF keeps it as noise"
        );
        assert!(
            shaped.iter().any(|v| *v != 0),
            "{format:?}: shaped keeps it too"
        );
        let (flat, moved) = (noise_at_4k(&tpdf), noise_at_4k(&shaped));
        assert!(
            moved < flat - 3.0,
            "{format:?}: shaped dither takes the noise out of 4 kHz ({moved:.1} vs {flat:.1} dB)"
        );
    }
}

#[test]
fn dither_touches_nothing_it_does_not_apply_to() {
    let a = common::music(48_000, 0.5);
    let dir = tempfile::tempdir().unwrap();
    for (format, depth) in [
        (Format::Wav, Depth::Float32),
        (Format::Mp3, Depth::Int24),
        (Format::Ogg, Depth::Int24),
    ] {
        let of = |dither| {
            let s = ExportSettings {
                format,
                depth,
                dither,
                ..Default::default()
            };
            std::fs::read(write(&a, &s, dir.path())).unwrap()
        };
        assert_eq!(of(Dither::Off), of(Dither::Shaped), "{format:?} {depth:?}");
    }
}

#[test]
fn every_preset_writes_what_its_card_says() {
    let a = common::music(48_000, 2.0);
    for p in [Preset::Streaming, Preset::Cd, Preset::Podcast] {
        let s = p.apply(&ExportSettings::default());
        assert_eq!(s.preset(), Some(p));
        let dir = tempfile::tempdir().unwrap();
        let path = write(&a, &s, dir.path());
        let got = probe(&path, s.format);
        let want = match p {
            Preset::Streaming => (Format::Mp3, 44_100, 2, None),
            Preset::Cd => (Format::Wav, 44_100, 2, Some(16)),
            _ => (Format::Mp3, 44_100, 1, None),
        };
        assert_eq!((s.format, got.rate, got.channels, got.bits), want, "{p:?}");
        match p {
            Preset::Streaming => assert!(mp3_frames(&path).iter().skip(1).all(|f| f.0 == 320)),
            Preset::Podcast => assert!(mp3_frames(&path).iter().skip(1).all(|f| f.0 == 128)),
            _ => {}
        }
    }
    let stems = Preset::MasteringStems.apply(&ExportSettings::default());
    assert_eq!(
        (stems.scope, stems.format, stems.depth, stems.rate),
        (Scope::Stems, Format::Wav, Depth::Int24, Rate::Session)
    );
}

#[test]
fn a_preset_card_is_lit_exactly_while_its_visible_choices_hold() {
    // Every choice a preset makes, changed, turns its card off.
    let stems = Preset::MasteringStems.apply(&ExportSettings::default());
    let mut tracks = stems.clone();
    tracks.stems.tracks = true;
    assert_ne!(
        tracks.preset(),
        Some(Preset::MasteringStems),
        "stems of tracks"
    );
    let mut through = stems.clone();
    through.stems.through_master = true;
    assert_ne!(
        through.preset(),
        Some(Preset::MasteringStems),
        "through the master"
    );
    // Choices the dialog hides do not keep a card dark: a ceiling set while a target was
    // on, then the target turned off, is not a choice any more.
    let mut cd = Preset::Cd.apply(&ExportSettings::default());
    cd.loudness.target_lufs = Some(-14.0);
    cd.loudness.ceiling_dbtp = -0.3;
    cd.loudness.limit = false;
    cd.loudness.target_lufs = None;
    assert_eq!(
        cd.preset(),
        Some(Preset::Cd),
        "the hidden ceiling is not a choice"
    );
    // A stems export shows no loudness at all.
    let mut quiet = stems;
    quiet.loudness.ceiling_dbtp = -2.0;
    assert_eq!(quiet.preset(), Some(Preset::MasteringStems));
}
