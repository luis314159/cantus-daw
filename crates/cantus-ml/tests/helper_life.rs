//! The helper's life beyond one job (review 1 of the first delivery): a job that panics is
//! said at once; an idle helper is ended, and `release` ends it now; and the helper ends
//! when the Cantus that started it is killed (Windows' job object, Linux's parent-death
//! signal, macOS's kqueue watch).

mod common;

use cantus_ml::client::{ClientConfig, Failure, MlClient, Outcome, Update};
use cantus_ml::proto::JobError;
use common::*;
use std::time::{Duration, Instant};

fn config() -> ClientConfig {
    let mut c = ClientConfig::new(env!("CARGO_BIN_EXE_cantus-ml"));
    c.test_hooks = true;
    c
}

fn track(dir: &std::path::Path, seconds: usize) -> std::path::PathBuf {
    let x = music(seconds * RATE as usize);
    let input = dir.join("in.wav");
    write_wav(&input, 2, &x.0, &x.1);
    input
}

const WAIT: Duration = Duration::from_secs(60);

fn alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if cfg!(windows) {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    } else {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

fn gone_within(pid: u32, limit: Duration) -> bool {
    let t = Instant::now();
    while t.elapsed() < limit {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn a_panicking_job_fails_at_once_and_the_helper_serves_on() {
    let dir = tempfile::tempdir().unwrap();
    let input = track(dir.path(), 20);
    let mut c = MlClient::new(config());
    let args = with_panic(
        fixture_args(input.clone(), dir.path().join("a"), [0.0; 4], [0.25; 4]),
        1,
    );
    let t = Instant::now();
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    match c.wait(id, WAIT) {
        Some(Outcome::Failed(Failure::Job(JobError::Model(m)))) => {
            assert!(m.contains("panicked"), "{m}")
        }
        other => panic!("{other:?}"),
    }
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    let pid = c.helper_pid();
    let ok = fixture_args(input, dir.path().join("b"), [0.0; 4], [0.25; 4]);
    let id = c.start("separate", serde_json::to_value(&ok).unwrap());
    assert!(matches!(c.wait(id, WAIT), Some(Outcome::Done(_))));
    assert_eq!(c.helper_pid(), pid, "the same helper");
}

#[test]
fn an_idle_helper_is_ended_and_release_ends_it_now() {
    let dir = tempfile::tempdir().unwrap();
    let input = track(dir.path(), 3);
    let mut cfg = config();
    cfg.idle = Duration::from_secs(2);
    let mut c = MlClient::new(cfg);
    let args = fixture_args(input.clone(), dir.path().join("a"), [0.0; 4], [0.25; 4]);
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    assert!(matches!(c.wait(id, WAIT), Some(Outcome::Done(_))));
    let pid = c.helper_pid();
    assert!(alive(pid));
    assert!(gone_within(pid, Duration::from_secs(10)), "idle, ended");
    assert_eq!(c.helper_pid(), 0);
    // A new job starts a new helper; release ends it at once once it is idle.
    let mut c = MlClient::new(config());
    let id = c.start("separate", serde_json::to_value(&args).unwrap());
    assert!(matches!(c.wait(id, WAIT), Some(Outcome::Done(_))));
    let pid = c.helper_pid();
    c.release();
    assert!(gone_within(pid, Duration::from_secs(5)), "released");
    // The stems it wrote can go: nothing holds them.
    std::fs::remove_dir_all(dir.path().join("a")).unwrap();
}

#[test]
fn a_hung_helpers_job_ends_once() {
    let dir = tempfile::tempdir().unwrap();
    let input = track(dir.path(), 20);
    let mut cfg = config();
    cfg.silence = Duration::from_secs(1);
    let mut c = MlClient::new(cfg);
    let hang = with_fixture(
        fixture_args(input, dir.path().join("a"), [0.0; 4], [0.25; 4]),
        0,
        None,
        Some(1),
    );
    let id = c.start("separate", serde_json::to_value(&hang).unwrap());
    assert_eq!(c.wait(id, WAIT), Some(Outcome::Failed(Failure::Hung)));
    std::thread::sleep(Duration::from_millis(500));
    let again = c
        .poll()
        .into_iter()
        .filter(|u| matches!(u, Update::Ended(j, _) if *j == id))
        .count();
    assert_eq!(again, 0, "a second end for the same job");
}

/// Run as a child "Cantus": start a slow job, write the helper's pid, wait to be killed.
fn as_parent() {
    let dir = std::path::PathBuf::from(std::env::var("CANTUS_ML_TEST_PARENT").unwrap());
    let input = track(&dir, 30);
    let mut c = MlClient::new(config());
    let slow = with_fixture(
        fixture_args(input, dir.join("out"), [0.0; 4], [0.25; 4]),
        1000,
        None,
        None,
    );
    let _ = c.start("separate", serde_json::to_value(&slow).unwrap());
    while c.helper_pid() == 0 {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::write(dir.join("pid"), c.helper_pid().to_string()).unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn the_helper_ends_when_cantus_is_killed() {
    if std::env::var("CANTUS_ML_TEST_PARENT").is_ok() {
        as_parent();
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut parent = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "the_helper_ends_when_cantus_is_killed",
            "--nocapture",
        ])
        .env("CANTUS_ML_TEST_PARENT", dir.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pidfile = dir.path().join("pid");
    let t = Instant::now();
    while !pidfile.exists() {
        assert!(t.elapsed() < WAIT, "the child never started its helper");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(100));
    let helper: u32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(helper));
    // Killed hard, as a crash ends it: no drop, no quit.
    parent.kill().unwrap();
    let _ = parent.wait();
    assert!(
        gone_within(helper, Duration::from_secs(10)),
        "the helper outlived Cantus"
    );
}

/// On Windows the helper lives in `ml\` of the install folder, beside its C++ runtime, and
/// is found there first; a development build (or another system) finds it beside the
/// program (integration review #1).
#[test]
fn the_helper_is_found_in_its_own_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = if cfg!(windows) {
        "cantus-ml.exe"
    } else {
        "cantus-ml"
    };
    assert_eq!(ClientConfig::exe_in(dir.path()), dir.path().join(name));
    std::fs::create_dir_all(dir.path().join("ml")).unwrap();
    std::fs::write(dir.path().join("ml").join(name), b"MZ").unwrap();
    let want = if cfg!(windows) {
        dir.path().join("ml").join(name)
    } else {
        dir.path().join(name)
    };
    assert_eq!(ClientConfig::exe_in(dir.path()), want);
}
