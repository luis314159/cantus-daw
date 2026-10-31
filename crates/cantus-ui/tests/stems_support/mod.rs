//! What the stems tests share: a tone, WAV files in and out, a project with a track of
//! clips, an app whose service runs this build's helper with the stand-in network, and a
//! run driven to its end.
#![allow(dead_code)]

use cantus_content::models::{ModelManifest, Models, ReadyModel};
use cantus_core::{AudioSource, AudioSourceId, Command, Origin, Project, Ticks};
use cantus_ml::client::ClientConfig;
use cantus_ui::ml_service::MlService;
use cantus_ui::stems_ui::run;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const PARTS: [&str; 4] = ["drums", "bass", "other", "vocals"];

pub fn tone(rate: u32, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * rate as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let l = 0.3 * (2.0 * std::f32::consts::PI * 220.0 * t).sin();
            let r = 0.2 * (2.0 * std::f32::consts::PI * 330.0 * t).sin();
            (l, r)
        })
        .unzip()
}

/// One test at a time, each with its own data folder: `CANTUS_DATA_DIR` is the process's,
/// and a test ending deletes its folder — under another test still writing there (Linux
/// deletes open files; Windows refuses, which hid it).
pub fn data_folder() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    (one, data)
}

pub fn write(path: &Path, rate: u32, x: &(Vec<f32>, Vec<f32>)) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..x.0.len() {
        w.write_sample(x.0[i]).unwrap();
        w.write_sample(x.1[i]).unwrap();
    }
    w.finalize().unwrap();
}

pub fn read(path: &Path) -> (u32, Vec<f32>, Vec<f32>) {
    let mut r = hound::WavReader::open(path).unwrap();
    let rate = r.spec().sample_rate;
    let s: Vec<f32> = r.samples::<f32>().map(|v| v.unwrap()).collect();
    let (l, rr) = s.chunks(2).map(|c| (c[0], c[1])).unzip();
    (rate, l, rr)
}

/// An app whose service runs this build's helper with the stand-in network.
pub fn app_with(project: Project, data: &Path) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(project), cantus_engine::EngineHandle::dummy(), false);
    let mut cfg = ClientConfig::new(crate::ml_support::helper_exe());
    cfg.test_hooks = true;
    app.ml = MlService::new(
        Models::new(data.join("Models"), ModelManifest::bundled()),
        cfg,
        true,
    );
    app
}

pub fn fixture_model() -> ReadyModel {
    ReadyModel {
        id: "htdemucs".into(),
        family: "fixture".into(),
        graph: PathBuf::from("unused"),
        library: PathBuf::from("unused"),
        library_sha256: "0".repeat(64),
        files: Vec::new(),
        sources: PARTS.iter().map(|s| s.to_string()).collect(),
        sample_rate: 44_100,
        segment: 343_980,
        name: "HT-Demucs — 4 stems".into(),
        version: 1,
        licence: "MIT".into(),
    }
}

/// A project with one track holding a clip of each file.
pub fn project(
    files: &[(PathBuf, u32, u64)],
) -> (Project, cantus_core::TrackId, Vec<cantus_core::ClipId>) {
    let mut bus = cantus_core::CommandBus::default();
    let track = bus.new_track("Mix");
    let tid = track.id;
    let mut cmds = vec![Command::InsertTrack {
        index: 0,
        track: Box::new(track),
    }];
    let mut clips = Vec::new();
    for (i, (path, rate, frames)) in files.iter().enumerate() {
        let id = AudioSourceId(bus.alloc_id());
        cmds.push(Command::InsertAudioSource {
            source: AudioSource {
                id,
                name: format!("Song {i}.wav"),
                path: path.display().to_string(),
                sample_rate: *rate,
                channels: 2,
                frames: *frames,
                sha256: None,
                recorded: false,
            },
        });
        let mut clip = bus.new_clip(
            format!("Song {i}"),
            Ticks::beats(16 * i as i64),
            Ticks::beats(8),
        );
        clip.audio = Some(cantus_core::AudioRegion::of(id));
        clip.offset = Ticks::beats(1);
        clips.push(clip.id);
        cmds.push(Command::InsertClip {
            track: tid,
            clip: Box::new(clip),
        });
    }
    bus.execute(
        Command::Batch {
            label_key: "import".into(),
            commands: cmds,
        },
        Origin::User,
    )
    .unwrap();
    (bus.project().clone(), tid, clips)
}

pub fn until_done(app: &mut CantusApp) {
    let t = Instant::now();
    while app.stems.runs.iter().any(|r| r.running()) {
        run::step(app);
        assert!(
            t.elapsed() < Duration::from_secs(120),
            "{:?}",
            app.stems.runs.last().map(|r| r.phase.clone())
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn cantus_ui_resolve(app: &CantusApp, path: &str) -> PathBuf {
    let s = app
        .project()
        .audio_pool
        .iter()
        .find(|s| s.path == path)
        .unwrap();
    app.audio_file(s)
}
