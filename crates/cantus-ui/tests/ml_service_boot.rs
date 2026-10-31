//! The machine-learning service from the boot path (round 34, ADR 0085): the desktop app
//! goes `run` → `boot::Boot` → `CantusApp::assemble`, so the one model store and helper
//! client are made there, under the data folder. And a job's updates reach whoever started
//! it without any frame drawn (an agent's job while the window is minimised). Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder.

mod ml_support;

use cantus_content::models::{ModelManifest, Models};
use cantus_core::Project;
use cantus_ml::client::{ClientConfig, Outcome, Update};
use cantus_ui::ml_service::MlService;
use cantus_ui::CantusApp;
use std::time::Duration;

#[test]
fn assemble_makes_the_one_service_under_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = cantus_ui::app::Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(Project::new("boot")),
        cantus_engine::EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert!(app.ml().is_wired(), "assemble makes the ML service");
    assert_eq!(app.ml().models().root(), data.path().join("Models"));
    assert!(
        !CantusApp::from_parts(None, cantus_engine::EngineHandle::dummy(), false)
            .ml()
            .is_wired()
    );
}

#[test]
fn a_jobs_updates_arrive_without_a_frame() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    ml_support::write_track(&input, 3.0);
    let mut cfg = ClientConfig::new(ml_support::helper_exe());
    cfg.test_hooks = true;
    let ml = MlService::new(
        Models::new(dir.path().join("Models"), ModelManifest::bundled()),
        cfg,
        true,
    );
    let args = serde_json::json!({
        "runtime": {"library": "unused"},
        "model": {"graph": "unused", "family": {"kind": "fixture", "spec": [0.0, 0.0, 0.0, 0.0],
                  "wave": [0.25, 0.25, 0.25, 0.25]},
                  "sources": ["drums", "bass", "other", "vocals"], "sample_rate": 44100, "segment": 343980},
        "input": input, "out_dir": dir.path().join("out"),
    });
    let job = ml.start("separate", args);
    // No frame, no poll: a worker thread's wait.
    let mut seen = Vec::new();
    loop {
        match job.updates.recv_timeout(Duration::from_secs(60)) {
            Ok(u) => {
                let end = matches!(u, Update::Ended(..));
                seen.push(u);
                if end {
                    break;
                }
            }
            Err(e) => panic!("{e:?} after {seen:?}"),
        }
    }
    assert!(seen.contains(&Update::Started(job.id)));
    assert!(matches!(
        seen.last(),
        Some(Update::Ended(_, Outcome::Done(_)))
    ));
    // The sender went with the end; the state stays for the views.
    assert!(job
        .updates
        .recv_timeout(Duration::from_millis(200))
        .is_err());
    assert!(ml.job(job.id).unwrap().outcome.is_some());
}
