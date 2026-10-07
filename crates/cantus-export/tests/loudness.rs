//! Loudness-normalised export lands within ±0.5 LU of the target, measured on the file
//! as decoded, under its true-peak ceiling.

mod common;

use cantus_dsp::terminus::loudness::LoudnessMeter;
use cantus_export::{
    export_rendered, Audio, Control, Depth, ExportSettings, Format, Loudness, Preset, Rate,
    Scope, Tags,
};
use common::*;

/// Integrated loudness and true peak of interleaved audio (mono as dual mono).
fn measure(x: &[f32], ch: usize, rate: u32) -> (f64, f32) {
    let mut m = LoudnessMeter::new(rate as f32);
    let l: Vec<f32> = x.iter().step_by(ch).copied().collect();
    let r: Vec<f32> = x.iter().skip(ch - 1).step_by(ch).copied().collect();
    m.process(&l, &r);
    (m.integrated().expect("loud enough to gate"), m.true_peak_db())
}

fn scaled(a: &Audio, db: f32) -> Audio {
    let g = 10f32.powf(db / 20.0);
    Audio {
        samples: a.samples.iter().map(|x| x * g).collect(),
        ..a.clone()
    }
}

fn float_wav(target: f32, ceiling: f32, limit: bool) -> ExportSettings {
    ExportSettings {
        format: Format::Wav,
        depth: Depth::Float32,
        rate: Rate::Session,
        loudness: Loudness {
            target_lufs: Some(target),
            ceiling_dbtp: ceiling,
            limit,
        },
        ..Default::default()
    }
}

fn run(a: &Audio, s: &ExportSettings) -> (cantus_export::Done, Vec<f32>, usize, u32) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("x.{}", s.format.extension()));
    let done = export_rendered(a.clone(), s, &Tags::default(), &path, &Control::new()).unwrap();
    let (rate, ch, back) = match s.format {
        Format::Mp3 => decode_mp3(&path),
        _ => {
            let mut r = hound::WavReader::open(&path).unwrap();
            let spec = r.spec();
            let v: Vec<f32> = r.samples::<f32>().map(|s| s.unwrap()).collect();
            (spec.sample_rate, spec.channels as usize, v)
        }
    };
    (done, back, ch, rate)
}

#[test]
fn a_quiet_mix_is_raised_to_the_target() {
    let a = scaled(&music(48_000, 8.0), -12.0);
    let (done, back, ch, rate) = run(&a, &float_wav(-14.0, -1.0, true));
    let (lufs, tp) = measure(&back, ch, rate);
    assert!((lufs + 14.0).abs() <= 0.5, "{lufs:.2} LUFS");
    assert!(tp <= -1.0 + 0.05, "{tp:.2} dBTP");
    let n = done.loudness.unwrap();
    assert!(n.gain_db > 0.0);
}

#[test]
fn a_loud_target_limits_the_peaks_and_still_lands() {
    let a = music(48_000, 8.0);
    let (done, back, ch, rate) = run(&a, &float_wav(-9.0, -1.0, true));
    let (lufs, tp) = measure(&back, ch, rate);
    assert!((lufs + 9.0).abs() <= 0.5, "{lufs:.2} LUFS");
    assert!(tp <= -1.0 + 0.1, "true peak {tp:.2} dBTP under the −1 dBTP ceiling");
    assert!(done.loudness.unwrap().limited);
}

#[test]
fn without_the_limiter_the_gain_stops_at_the_ceiling() {
    let a = music(48_000, 8.0);
    let (done, back, ch, rate) = run(&a, &float_wav(-6.0, -1.0, false));
    let (lufs, tp) = measure(&back, ch, rate);
    assert!((tp + 1.0).abs() < 0.1, "peak at the ceiling: {tp:.2}");
    assert!(lufs < -6.5, "quieter than asked, as said: {lufs:.2}");
    let n = done.loudness.unwrap();
    assert!(!n.limited);
    assert!((n.lufs.unwrap() - lufs).abs() < 0.05, "the report says what was written");
}

#[test]
fn the_streaming_preset_lands_on_minus_14_as_an_mp3() {
    let a = scaled(&music(48_000, 8.0), -8.0);
    let s = Preset::Streaming.apply(&ExportSettings::default());
    assert_eq!(s.format, Format::Mp3);
    let (_, back, ch, rate) = run(&a, &s);
    assert_eq!(rate, 44_100);
    let (lufs, _) = measure(&back, ch, rate);
    assert!((lufs + 14.0).abs() <= 0.5, "{lufs:.2} LUFS");
}

#[test]
fn the_podcast_preset_is_mono_at_minus_16_heard_as_dual_mono() {
    let a = scaled(&music(48_000, 8.0), -6.0);
    let s = Preset::Podcast.apply(&ExportSettings::default());
    let (_, back, ch, rate) = run(&a, &s);
    assert_eq!((ch, rate), (1, 44_100));
    let (lufs, tp) = measure(&back, ch, rate);
    assert!((lufs + 16.0).abs() <= 0.5, "{lufs:.2} LUFS");
    assert!(tp < 0.0);
}

#[test]
fn silence_is_left_alone() {
    let a = Audio {
        rate: 48_000,
        channels: 2,
        samples: vec![0.0; 96_000],
    };
    let (done, back, ..) = run(&a, &float_wav(-14.0, -1.0, true));
    assert!(back.iter().all(|x| *x == 0.0));
    assert_eq!(done.loudness.unwrap().gain_db, 0.0);
}

#[test]
fn stems_are_never_normalised() {
    let s = ExportSettings {
        scope: Scope::Stems,
        ..float_wav(-14.0, -1.0, true)
    };
    assert!(!s.normalises());
}
