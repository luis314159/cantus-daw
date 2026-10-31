//! A 60-minute take through *Convert to MIDI…* (round 34's review: long files), for
//! measuring memory — ignored, run by hand:
//!
//! ```text
//! CANTUS_LONG_DIR=<a folder with 2 GB free>
//! CANTUS_ORT_LIBRARY=<onnxruntime.dll>  CANTUS_BASIC_PITCH=<nmp.onnx>
//! CANTUS_OAF_DRUMS=<oaf_drums.cantus.onnx>
//! cargo test -p cantus-ui --test transcribe_long -- --ignored --nocapture --test-threads 1
//! ```
//!
//! Each step prints when it starts and ends, so an outside sampler of the processes' peak
//! working sets (`docs/round34/transcribe.md` has the numbers) can tell them apart.

mod ml_support;

use cantus_ml::client::{ClientConfig, MlClient, Outcome};
use cantus_ml::spec::{Device, RuntimeSpec};
use cantus_ml::transcribe::spec::*;
use cantus_ui::transcribe_ui as tr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MINUTES: u64 = 60;
const RATE: u32 = 48_000;

fn var(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
}

/// An hour of 16-bit stereo at 48 kHz (a recorded take): a note every half second.
fn take(dir: &Path) -> PathBuf {
    let path = dir.join("hour.wav");
    if path.is_file() {
        return path;
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let n = MINUTES * 60 * RATE as u64;
    for i in 0..n {
        let t = (i % (RATE as u64 / 2)) as f64 / RATE as f64;
        let k = (i / (RATE as u64 / 2)) % 12;
        let hz = 220.0 * 2f64.powf(k as f64 / 12.0);
        let v = (0.3 * (-t * 6.0).exp() * (std::f64::consts::TAU * hz * t).sin() * 32_767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
    path
}

fn source(path: &Path) -> tr::Source {
    tr::Source {
        track: cantus_core::TrackId(1),
        clip: cantus_core::ClipId(2),
        clip_name: "hour".into(),
        track_name: "take".into(),
        path: path.to_path_buf(),
        rate: RATE,
        frames: (0, MINUTES * 60 * RATE as u64),
        pieces: Vec::new(),
        start: cantus_core::Ticks::ZERO,
        end: cantus_core::Ticks::ZERO,
    }
}

fn step<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    };
    println!("STEP {name} begins at {:.1}", now());
    let out = f();
    println!(
        "STEP {name} ends at {:.1} after {:.1} s",
        now(),
        t0.elapsed().as_secs_f64()
    );
    std::thread::sleep(Duration::from_secs(3));
    out
}

fn job(mode: Mode, input: PathBuf, out: PathBuf, model: NoteModel) -> Outcome {
    // A helper of its own for each job: its peak is that job's.
    let mut c = MlClient::new(ClientConfig::new(ml_support::helper_exe()));
    let a = TranscribeArgs {
        mode,
        input,
        out,
        runtime: Some(RuntimeSpec {
            library: var("CANTUS_ORT_LIBRARY"),
            threads: 0,
            device: Device::Cpu,
            sha256: None,
        }),
        model: Some(model),
        notes: NoteSettings::default(),
        drums: DrumSettings::default(),
    };
    let id = c.start("transcribe", serde_json::to_value(&a).unwrap());
    c.wait(id, Duration::from_secs(3_600)).expect("ended")
}

#[test]
#[ignore = "an hour of audio and both models (see the file's header)"]
fn an_hour_converts_in_bounded_memory() {
    let dir = var("CANTUS_LONG_DIR");
    std::fs::create_dir_all(&dir).unwrap();
    let src = source(&take(&dir));
    let notes_in = dir.join("notes.wav");
    let drums_in = dir.join("drums.wav");
    step("read-22050", || {
        tr::write_input(&src, 22_050, &notes_in, &|| false).unwrap()
    });
    step("read-44100", || {
        tr::write_input(&src, 44_100, &drums_in, &|| false).unwrap()
    });
    let o = step("polyphonic", || {
        job(
            Mode::Polyphonic,
            notes_in.clone(),
            dir.join("notes.json"),
            NoteModel {
                graph: var("CANTUS_BASIC_PITCH"),
                family: NoteFamily::BasicPitch,
                check: Vec::new(),
            },
        )
    });
    assert!(matches!(o, Outcome::Done(_)), "{o:?}");
    let o = step("drums", || {
        job(
            Mode::Drums,
            drums_in.clone(),
            dir.join("drums.json"),
            NoteModel {
                graph: var("CANTUS_OAF_DRUMS"),
                family: NoteFamily::OafDrums,
                check: Vec::new(),
            },
        )
    });
    assert!(matches!(o, Outcome::Done(_)), "{o:?}");
}
