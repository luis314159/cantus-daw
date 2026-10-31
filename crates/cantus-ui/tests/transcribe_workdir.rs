//! A conversion's files in the cache (round 34's review): its folder goes when it ends or
//! is stopped — while the take is still being read, too — and a folder left by a Cantus
//! that ended mid-conversion is swept at the next start. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ml::transcribe::spec::Mode;
use cantus_ui::transcribe_ui::{self as tr, run};
use cantus_ui::CantusApp;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The tests share the process's data folder and the cache in it.
static ONE: Mutex<()> = Mutex::new(());

fn data_dir() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let d = DIR.get_or_init(|| tempfile::tempdir().unwrap()).path();
    std::env::set_var("CANTUS_DATA_DIR", d);
    d
}

fn folders() -> Vec<std::path::PathBuf> {
    std::fs::read_dir(tr::cache_root())
        .map(|r| r.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

#[test]
fn a_folder_no_cantus_holds_is_swept_and_a_held_one_kept() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    let held = tr::WorkDir::new("").unwrap();
    std::fs::write(held.join("input.wav"), b"held").unwrap();
    // What a Cantus that ended mid-conversion leaves: its folder, its lock file unheld.
    let left = tr::cache_root().join("4242-1");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::write(left.join("held.lock"), b"").unwrap();
    std::fs::write(left.join("input.wav"), vec![0u8; 4096]).unwrap();
    // And one from before the lock file.
    let older = tr::cache_root().join("4242-2");
    std::fs::create_dir_all(&older).unwrap();
    std::fs::write(older.join("input.wav"), b"x").unwrap();
    assert_eq!(tr::sweep(), 2);
    assert!(!left.exists() && !older.exists());
    assert!(held.join("input.wav").is_file(), "a held folder stays");
    let path = held.path.clone();
    drop(held);
    assert!(!path.exists(), "a conversion's folder goes with it");
}

/// A project holding a minute of stereo take (a sine) on a Drums track: long enough to be
/// stopped while it is read.
fn minute_take() -> (Project, TrackId, ClipId, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let rate = 48_000u32;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..60 * rate * 2 {
        // A 220 Hz tone (frames interleaved: each written twice).
        let t = (i / 2) as f32 / rate as f32;
        w.write_sample((std::f32::consts::TAU * 220.0 * t).sin() * 0.2)
            .unwrap();
    }
    w.finalize().unwrap();
    let mut p = Project::new("stop");
    p.tempo_bpm = 120.0;
    let mut v = p.make_track("Drums");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: rate,
        channels: 2,
        frames: 60 * rate as u64,
        sha256: None,
        recorded: true,
    });
    let clip = p.make_audio_clip("Take 1", id, Ticks::ZERO, Ticks::beats(120));
    let (t, c) = (v.id, clip.id);
    v.clips.push(clip);
    p.tracks.push(v);
    (p, t, c, dir)
}

#[test]
fn stop_while_the_take_is_read_leaves_no_file_behind() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (p, t, c, _dir) = minute_take();
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    let before = app.project().clone();
    run::open(&mut app, t, c);
    app.transcribe.dialog.as_mut().unwrap().settings.mode = Mode::Drums;
    run::start(&mut app);
    assert!(app.transcribe.busy());
    assert_eq!(folders().len(), 1, "the conversion's folder");
    run::stop(&mut app);
    assert!(!app.transcribe.busy());
    // The reader stops at its next block and lets the folder go.
    let t0 = Instant::now();
    while !folders().is_empty() {
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "left behind: {:?}",
            folders()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(app.project(), &before, "a stop changes nothing");
}

#[test]
fn a_cancelled_read_writes_nothing() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22_050,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..22_050 * 4 {
        w.write_sample(((i % 100) as i16 - 50) * 300).unwrap();
    }
    w.finalize().unwrap();
    // Frames 1 s … 3 s, mono, as the file holds them.
    let (x, rate) = tr::read_mono_range(&path, 22_050, 3 * 22_050, &|| false).unwrap();
    assert_eq!((x.len(), rate), (2 * 22_050, 22_050));
    for k in [0usize, 1, 37, 44_099] {
        let want = (((22_050 + k) % 100) as f32 - 50.0) * 300.0 / 32_768.0;
        assert!((x[k] - want).abs() < 1e-6, "{k}: {} against {want}", x[k]);
    }
    assert!(tr::read_mono_range(&path, 0, 4 * 22_050, &|| true).is_err());
}

#[test]
fn another_project_open_stops_the_conversion_and_makes_nothing() {
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (p, t, c, _dir) = minute_take();
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    run::open(&mut app, t, c);
    // Melody: Cantus's own tracker, in process — left alone it ends with a new track.
    app.transcribe.dialog.as_mut().unwrap().settings.mode = Mode::Melody;
    run::start(&mut app);
    assert!(app.transcribe.busy());
    app.replace_project(Project::new("other"));
    let other = app.project().clone();
    let steps = app.bus.history().len();
    let ctx = egui::Context::default();
    let t0 = Instant::now();
    while app.transcribe.busy() || !folders().is_empty() {
        run::frame(&mut app, &ctx);
        assert!(t0.elapsed() < Duration::from_secs(60), "it never stopped");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(app.transcribe.dialog.is_none(), "the dialog closes");
    // Give a tracker that was not stopped the time to end and be applied.
    for _ in 0..50 {
        run::frame(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        app.project(),
        &other,
        "nothing made in the project opened since: {:?}",
        app.transcribe.last
    );
    assert_eq!(app.bus.history().len(), steps);
}

#[test]
fn two_conversions_at_once_never_share_a_folder() {
    // Two windows in one process (the tests of a binary are such): each conversion works
    // in a folder of its own, a start's sweep leaves the other's alone, and one stopped
    // takes only its own folder with it (GitHub's Linux runner, beta.9: the second
    // window's folder was the first's, removed under it).
    let _one = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (p1, t1, c1, _d1) = minute_take();
    let (p2, t2, c2, _d2) = minute_take();
    let mut a = CantusApp::from_parts(Some(p1), EngineHandle::dummy(), false);
    let mut b = CantusApp::from_parts(Some(p2), EngineHandle::dummy(), false);
    for (app, t, c) in [(&mut a, t1, c1), (&mut b, t2, c2)] {
        run::open(app, t, c);
        app.transcribe.dialog.as_mut().unwrap().settings.mode = Mode::Drums;
        run::start(app);
        assert!(app.transcribe.busy());
    }
    let both = folders();
    assert_eq!(both.len(), 2, "a folder each: {both:?}");
    run::stop(&mut a);
    // `a`'s folder goes; `b`'s stays, and its reader finishes its input there.
    let t0 = Instant::now();
    loop {
        let now = folders();
        let input = now.iter().any(|f| f.join("input.wav").is_file());
        if now.len() == 1 && input {
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(60),
            "{now:?}: {:?}",
            b.transcribe.last
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(b.transcribe.busy(), "{:?}", b.transcribe.last);
    run::stop(&mut b);
    let t0 = Instant::now();
    while !folders().is_empty() {
        assert!(t0.elapsed() < Duration::from_secs(20), "{:?}", folders());
        std::thread::sleep(Duration::from_millis(20));
    }
}
