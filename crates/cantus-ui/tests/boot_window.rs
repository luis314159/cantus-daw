//! What the real window starts beside itself (ADR 0050, second review #4, #11, #12): the
//! desktop app goes `run` → `boot::Boot` → its stages → `CantusApp::assemble`, never
//! `CantusApp::new`; so the sample cache's folder, the limit Settings chose, the start
//! sweep and loading beside the window must be wired there. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder and the stream hub is per process.

use cantus_core::InstrumentSpec;
use cantus_core::TrackIcon;
use cantus_engine::EngineHandle;
use cantus_ui::app::{data_dir, Files};
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::views::inst::sampler::sfz;
use cantus_ui::views::samples_content::sounds;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

fn wav(path: &std::path::Path, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let n = (48_000.0 * seconds) as usize;
    for i in 0..n {
        w.write_sample(((i % 97) as i16 - 48) * 100).unwrap();
    }
    w.finalize().unwrap();
}

/// The boot's assemble stage, driven without a window: the cache in the data folder, the
/// Settings limit on the hub, the project opened at start loading beside the window (no
/// device here, as on a machine whose interface is unplugged).
#[test]
fn the_boot_path_starts_the_sample_cache_and_loads_beside_the_window() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    assert_eq!(data_dir(), data.path(), "never the user's own folder");
    // A crash's stray part in the test's cache folder: the start sweep tidies it there.
    let cache = data.path().join("cache").join("samples");
    std::fs::create_dir_all(&cache).unwrap();
    let stray = cache.join("dead.part1");
    std::fs::write(&stray, b"half").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&stray)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(3_600))
        .unwrap();
    let pack = data.path().join("pack");
    std::fs::create_dir_all(&pack).unwrap();
    wav(&pack.join("a.wav"), 4.0);
    let none = |_: &str| None;
    let meta = |_: &str| None;
    let imp = sfz::parse(
        "<region> sample=a.wav lokey=0 hikey=127 pitch_keycenter=60\n",
        &pack,
        &none,
        &meta,
    );
    let spec = InstrumentSpec::Native(sounds::spec_of(&imp));
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut s = Song::new("boot", 120.0, 1);
    let part = s.part("piano", spec, colors::BASS, TrackIcon::Keys);
    s.feel(part, Feel::TIGHT);
    let project = s.finish();

    let mut files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    files.settings.sample_cache_gb = Some(3.0);
    // The decoder is held until the test lets it go: assemble must return while it is —
    // proved by the logic, not by a clock (a loaded machine made a time bound flaky).
    cantus_engine::samples::assets::hold_loads(true);
    let ctx = egui::Context::default();
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(project),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    let loading = app.engine.samples.loading();
    cantus_engine::samples::assets::hold_loads(false);
    let hub = cantus_engine::inst::stream::hub();
    assert_eq!(
        hub.cache_dir(),
        data.path().join("cache").join("samples"),
        "the cache lives in the data folder"
    );
    assert_eq!(
        hub.cache_limit(),
        3 << 30,
        "the Settings limit reaches the cache"
    );
    assert!(
        app.engine.samples.streaming(),
        "Exemplar streams in the window"
    );
    assert!(
        app.engine.samples.background(),
        "loads go beside the window"
    );
    assert!(
        loading,
        "the project opened at start loads beside the window, not in the boot's frame"
    );
    assert!(hub.sweeps_started() >= 1, "the start sweep ran");
    let t = Instant::now();
    while stray.exists() && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!stray.exists(), "the sweep ran on the test's folder");
    // Let go, the samples arrive.
    let mut app = app;
    let t = Instant::now();
    while app.engine.samples.loading() && t.elapsed() < Duration::from_secs(20) {
        app.engine.samples.poll_loaded();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !app.engine.samples.loading(),
        "released: the samples arrived"
    );
}

/// Second review: a test that builds the application as the desktop does starts its
/// sample cache and sweeps it — so every such test points `CANTUS_DATA_DIR` at a folder of
/// its own, and the services read the data folder through `data_dir` (which honours it).
#[test]
fn tests_that_build_the_real_app_never_touch_the_users_data_folder() {
    let app = include_str!("../src/app.rs").replace("\r\n", "\n");
    let services = &app[app.find("fn start_window_services(").expect("the services")..];
    let services = &services[..services.find("\n    }\n").unwrap()];
    assert!(services.contains("set_cache_dir(data_dir()"), "{services}");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let builds = [
                "CantusApp::assemble(",
                "CantusApp::new(",
                "boot::Boot::new(",
            ]
            .iter()
            .any(|c| text.contains(c));
            if builds && p.file_name() != Some(std::ffi::OsStr::new("boot_window.rs")) {
                assert!(
                    text.contains("CANTUS_DATA_DIR"),
                    "{} builds the real app without a data folder of its own",
                    p.display()
                );
            }
        }
    }
}

/// The backstop: the boot's assemble stage calls `assemble`, and `assemble` starts what the
/// window needs before the first snapshot (the boot's own font-stage test is the pattern).
#[test]
fn the_window_services_live_on_the_boot_path_not_only_in_new() {
    let boot = include_str!("../src/boot.rs");
    let stage = &boot[boot
        .find("Stage::Assemble => {")
        .expect("the assemble stage")..];
    assert!(stage.contains("CantusApp::assemble("), "{stage}");
    let app = include_str!("../src/app.rs").replace("\r\n", "\n");
    let body = &app[app.find("pub fn assemble(").expect("assemble")..];
    let body = &body[..body.find("\n    }\n").unwrap()];
    let services = body
        .find("start_window_services(")
        .expect("assemble starts the window's services");
    // `from_parts` or `from_parts_with` (round 29: the isolation default goes in first).
    let parts = body.find("Self::from_parts").expect("from_parts");
    assert!(
        services < parts,
        "before the first snapshot (from_parts syncs the engine)"
    );
    let new = &app[app.find("pub fn new(").expect("new")..];
    let new = &new[..new.find("\n    }\n").unwrap()];
    for only_new in [
        "set_cache_dir",
        "load_beside_the_window",
        "cantus-warm-builtins",
    ] {
        assert!(
            !new.contains(only_new),
            "`{only_new}` in `new` alone would never run in the desktop app"
        );
    }
}
