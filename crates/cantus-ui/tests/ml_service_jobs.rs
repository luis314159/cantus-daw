//! The service's job table stays bounded (round 34's review): a job its starter forgot is
//! not made again by its late end, and a starter that never forgets keeps only the last
//! [`KEPT_ENDED`] ended jobs.

mod ml_support;

use cantus_content::models::{ModelManifest, Models};
use cantus_ml::client::{ClientConfig, Update};
use cantus_ui::ml_service::{MlJob, MlService, KEPT_ENDED};
use std::time::Duration;

fn service(dir: &std::path::Path) -> MlService {
    MlService::new(
        Models::new(dir.join("Models"), ModelManifest::bundled()),
        ClientConfig::new(ml_support::helper_exe()),
        true,
    )
}

/// A separation of `input` with the fixture family (no runtime, no model file).
fn separate(ml: &MlService, input: &std::path::Path, out: &std::path::Path) -> MlJob {
    ml.start(
        "separate",
        serde_json::json!({
            "runtime": {"library": "unused"},
            "model": {"graph": "unused", "family": {"kind": "fixture", "spec": [0.0, 0.0, 0.0, 0.0],
                      "wave": [0.25, 0.25, 0.25, 0.25]},
                      "sources": ["drums", "bass", "other", "vocals"], "sample_rate": 44100, "segment": 343980},
            "input": input, "out_dir": out,
        }),
    )
}

fn ended(job: &MlJob) {
    loop {
        match job.updates.recv_timeout(Duration::from_secs(120)) {
            Ok(Update::Ended(..)) => return,
            Ok(_) => {}
            Err(e) => panic!("job {:?}: {e:?}", job.id),
        }
    }
}

#[test]
fn a_forgotten_job_is_not_made_again_by_its_end() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    ml_support::write_track(&input, 3.0);
    let ml = service(dir.path());
    let a = separate(&ml, &input, &dir.path().join("a"));
    // Forgotten while it runs (a stop, an agent's tool that moved on).
    ml.forget(a.id);
    ended(&a);
    // The router handles updates in order: once a later job has ended, `a`'s end has been
    // handled too.
    let b = separate(&ml, &dir.path().join("missing.wav"), &dir.path().join("b"));
    ended(&b);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(ml.job(a.id), None, "the end made the forgotten job again");
    ml.forget(b.id);
    assert_eq!(ml.kept(), 0);
}

#[test]
fn a_starter_that_never_forgets_keeps_only_the_last_ended_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let ml = service(dir.path());
    let missing = dir.path().join("missing.wav");
    let jobs: Vec<MlJob> = (0..KEPT_ENDED + 8)
        .map(|i| separate(&ml, &missing, &dir.path().join(format!("o{i}"))))
        .collect();
    for j in &jobs {
        ended(j);
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(ml.kept(), KEPT_ENDED);
    assert!(ml.job(jobs[0].id).is_none(), "the oldest went first");
    assert!(ml.job(jobs.last().unwrap().id).unwrap().outcome.is_some());
}
