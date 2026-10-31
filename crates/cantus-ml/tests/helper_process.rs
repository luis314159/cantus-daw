//! The helper as Cantus runs it: `cantus-ml` started by the client on its own thread, jobs
//! over its pipes, progress, a cancel, and the helper crashing, hanging, killed from
//! outside, missing or too slow to start — each reported as what it is, and the next job
//! served by a fresh helper.

mod common;

use cantus_ml::client::{ClientConfig, Failure, MlClient, Outcome, Update};
use cantus_ml::proto::JobError;
use cantus_ml::spec::SeparateResult;
use common::*;
use std::time::{Duration, Instant};

fn config() -> ClientConfig {
    let mut c = ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml"));
    c.test_hooks = true;
    c
}

fn track(dir: &std::path::Path, seconds: usize) -> (std::path::PathBuf, (Vec<f32>, Vec<f32>)) {
    let x = music(seconds * RATE as usize);
    let input = dir.join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    (input, x)
}

fn done(o: Option<Outcome>) -> SeparateResult {
    match o {
        Some(Outcome::Done(v)) => serde_json::from_value(v).unwrap(),
        other => panic!("{other:?}"),
    }
}

const WAIT: Duration = Duration::from_secs(60);

#[test]
fn a_job_runs_end_to_end_in_the_helper() {
    let dir = tempfile::tempdir().unwrap();
    let (input, x) = track(dir.path(), 10);
    let mut c = MlClient::new(config());
    assert_eq!(c.helper_pid(), 0, "no helper before a job needs one");
    let args = fixture_args(
        input,
        dir.path().join("out"),
        [0.0; 4],
        [0.1, 0.2, 0.3, 0.4],
    );
    let t = Instant::now();
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    // Starting never waits for the process.
    assert!(t.elapsed() < Duration::from_millis(200));
    let r = done(c.wait(id, WAIT));
    let updates = c.poll();
    assert!(updates.contains(&Update::Started(id)), "{updates:?}");
    let progress: Vec<f32> = updates
        .iter()
        .filter_map(|u| match u {
            Update::Progress { id: j, done, .. } if *j == id => Some(*done),
            _ => None,
        })
        .collect();
    assert!(!progress.is_empty() && progress.windows(2).all(|p| p[0] <= p[1]));
    let mean = mono_mean(&x.0, &x.1);
    for (s, f) in r.stems.iter().enumerate() {
        let stem = read_wav(&f.path);
        let gain = [0.1, 0.2, 0.3, 0.4][s];
        assert!(worst(&stem, &x, gain, mean, 0..x.0.len()) < 2e-6);
    }
    // The helper stays for the next job, and serves it.
    let pid = c.helper_pid();
    assert_ne!(pid, 0);
    let args = fixture_args(
        dir.path().join("in.wav"),
        dir.path().join("out2"),
        [0.0; 4],
        [0.25; 4],
    );
    let id2 = c.start("separate", serde_json::to_value(&args).unwrap());
    done(c.wait(id2, WAIT));
    assert_eq!(c.helper_pid(), pid);
}

#[test]
fn a_cancel_stops_the_job_and_leaves_no_stems() {
    let dir = tempfile::tempdir().unwrap();
    let (input, _) = track(dir.path(), 30);
    let mut c = MlClient::new(config());
    let out = dir.path().join("out");
    let args = with_fixture(
        fixture_args(input.clone(), out.clone(), [0.0; 4], [0.25; 4]),
        400,
        None,
        None,
    );
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    // Cancel once it is under way.
    let t = Instant::now();
    loop {
        let u = c.poll();
        if u.iter()
            .any(|u| matches!(u, Update::Progress { done, .. } if *done > 0.05))
        {
            break;
        }
        assert!(t.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(20));
    }
    let asked = Instant::now();
    c.cancel(id);
    assert_eq!(c.wait(id, WAIT), Some(Outcome::Cancelled));
    assert!(
        asked.elapsed() < Duration::from_secs(2),
        "{:?}",
        asked.elapsed()
    );
    for s in SOURCES {
        assert!(!out.join(format!("{s}.wav")).exists(), "{s} left behind");
    }
    // A queued job cancelled before it starts never reaches the helper.
    let slow = with_fixture(
        fixture_args(input.clone(), dir.path().join("o2"), [0.0; 4], [0.25; 4]),
        200,
        None,
        None,
    );
    let a = c.start("separate", serde_json::to_value(&slow).unwrap());
    let b = c.start("separate", serde_json::to_value(&slow).unwrap());
    c.cancel(b);
    assert_eq!(c.wait(b, WAIT), Some(Outcome::Cancelled));
    done(c.wait(a, WAIT));
}

#[test]
fn a_crash_fails_its_job_and_the_next_job_gets_a_new_helper() {
    let dir = tempfile::tempdir().unwrap();
    let (input, _) = track(dir.path(), 20);
    let mut c = MlClient::new(config());
    let crash = with_fixture(
        fixture_args(input.clone(), dir.path().join("a"), [0.0; 4], [0.25; 4]),
        0,
        Some(1),
        None,
    );
    let id = c.start("separate", serde_json::to_value(&crash).unwrap());
    match c.wait(id, WAIT) {
        Some(Outcome::Failed(Failure::Crashed(how))) => assert!(!how.is_empty()),
        other => panic!("{other:?}"),
    }
    let ok = fixture_args(input, dir.path().join("b"), [0.0; 4], [0.25; 4]);
    let id = c.start("separate", serde_json::to_value(&ok).unwrap());
    done(c.wait(id, WAIT));
}

#[test]
fn a_hang_is_ended_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let (input, _) = track(dir.path(), 20);
    let mut cfg = config();
    cfg.silence = Duration::from_secs(2);
    let mut c = MlClient::new(cfg);
    let hang = with_fixture(
        fixture_args(input.clone(), dir.path().join("a"), [0.0; 4], [0.25; 4]),
        0,
        None,
        Some(1),
    );
    let id = c.start("separate", serde_json::to_value(&hang).unwrap());
    let t = Instant::now();
    assert_eq!(c.wait(id, WAIT), Some(Outcome::Failed(Failure::Hung)));
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!(c.helper_pid(), 0, "the hung helper was ended");
    let ok = fixture_args(input, dir.path().join("b"), [0.0; 4], [0.25; 4]);
    let id = c.start("separate", serde_json::to_value(&ok).unwrap());
    done(c.wait(id, WAIT));
}

#[test]
fn a_helper_killed_from_outside_is_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let (input, _) = track(dir.path(), 30);
    let mut c = MlClient::new(config());
    let slow = with_fixture(
        fixture_args(input, dir.path().join("a"), [0.0; 4], [0.25; 4]),
        500,
        None,
        None,
    );
    let id = c.start("separate", serde_json::to_value(&slow).unwrap());
    let t = Instant::now();
    while !c.poll().iter().any(|u| *u == Update::Started(id)) {
        assert!(t.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = c.helper_pid().to_string();
    let killed = if cfg!(windows) {
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid])
            .output()
    } else {
        std::process::Command::new("kill")
            .args(["-9", &pid])
            .output()
    };
    assert!(killed.unwrap().status.success());
    assert!(matches!(
        c.wait(id, WAIT),
        Some(Outcome::Failed(Failure::Crashed(_)))
    ));
}

#[test]
fn a_missing_or_slow_helper_fails_the_job_cleanly() {
    let mut cfg = ClientConfig::new("no-such-cantus-ml-helper");
    cfg.test_hooks = true;
    let mut c = MlClient::new(cfg);
    let id = c.start("separate", serde_json::json!({}));
    assert!(matches!(
        c.wait(id, WAIT),
        Some(Outcome::Failed(Failure::Start(_)))
    ));

    let mut cfg = config();
    cfg.start = Duration::from_secs(1);
    cfg.env
        .push(("CANTUS_ML_START_DELAY_MS".into(), "5000".into()));
    let mut c = MlClient::new(cfg);
    let id = c.start("separate", serde_json::json!({}));
    match c.wait(id, WAIT) {
        Some(Outcome::Failed(Failure::Start(m))) => assert!(m.contains("did not answer"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn unknown_jobs_and_bad_arguments_are_the_jobs_failures() {
    let mut c = MlClient::new(config());
    let id = c.start("no-such-job", serde_json::json!({}));
    assert_eq!(
        c.wait(id, WAIT),
        Some(Outcome::Failed(Failure::Job(JobError::UnknownJob(
            "no-such-job".into()
        ))))
    );
    let id = c.start("separate", serde_json::json!({"input": 3}));
    assert!(matches!(
        c.wait(id, WAIT),
        Some(Outcome::Failed(Failure::Job(JobError::BadArgs(_))))
    ));
}

#[test]
fn dropping_the_client_ends_the_helper_and_cancels_what_waits() {
    let dir = tempfile::tempdir().unwrap();
    let (input, _) = track(dir.path(), 20);
    let mut c = MlClient::new(config());
    let slow = with_fixture(
        fixture_args(input, dir.path().join("a"), [0.0; 4], [0.25; 4]),
        300,
        None,
        None,
    );
    let id = c.start("separate", serde_json::to_value(&slow).unwrap());
    let t = Instant::now();
    while c.helper_pid() == 0 {
        assert!(t.elapsed() < WAIT);
        std::thread::sleep(Duration::from_millis(10));
    }
    let pid = c.helper_pid();
    let _ = id;
    let t = Instant::now();
    drop(c);
    assert!(t.elapsed() < Duration::from_secs(5));
    assert!(!alive(pid), "the helper outlived its client");
}

fn alive(pid: u32) -> bool {
    if cfg!(windows) {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    } else {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
            || std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .is_ok_and(|s| s.success())
    }
}
