//! Start-up time of the real executable (`docs/perf/benchmark.md` §2.6).
//!
//! The release `cantus` binary is started with a fresh, empty data folder (so nothing of
//! the user's is read or written, and every start is a first start) and the start-up
//! probe switched on (`cantus_ui::startup_probe`). It opens the splash, reads its files,
//! opens the audio device and MIDI, assembles the application on the demo it opens on,
//! draws two frames, writes how long each stage took, and closes. The wall time from
//! spawning the process to the report appearing is the number a person waits through.
//!
//! It opens a window and the sound card, so it is in the full suite only, never the gate.

use crate::report::{Better, Metric};
use crate::stats::Summary;
use crate::Plan;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The release binary next to this one (`target/release/cantus[.exe]`).
pub fn find_exe() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let name = format!("cantus{}", std::env::consts::EXE_SUFFIX);
    let p = me.parent()?.join(name);
    p.exists().then_some(p)
}

/// One start: (spawn → report, splash, app, second frame), ms.
fn once(exe: &Path, dir: &Path) -> Option<[f64; 4]> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).ok()?;
    let report = dir.join("startup.json");
    let t0 = Instant::now();
    let mut child = std::process::Command::new(exe)
        .env(cantus_ui::startup_probe::VAR, &report)
        .env("LOCALAPPDATA", dir)
        .env("XDG_DATA_HOME", dir)
        .env("HOME", dir)
        .env("RUST_LOG", "warn")
        .spawn()
        .ok()?;
    let mut wall = None;
    while t0.elapsed() < Duration::from_secs(60) {
        if wall.is_none() && report.exists() {
            wall = Some(t0.elapsed().as_secs_f64() * 1e3);
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let text = std::fs::read_to_string(&report).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some([wall?, f("splash_ms"), f("app_ms"), f("second_frame_ms")])
}

pub fn suite(plan: &Plan) -> Vec<Metric> {
    let Some(exe) = find_exe() else {
        eprintln!("start-up: no release `cantus` binary beside this one; build it with `cargo build --release -p cantus`");
        return Vec::new();
    };
    let dir = std::env::temp_dir().join(format!("cantus-startup-{}", std::process::id()));
    let mut runs: Vec<[f64; 4]> = Vec::new();
    // One start first, unmeasured: the executable and its libraries into the file cache.
    let _ = once(&exe, &dir);
    for _ in 0..plan.runs.max(3) {
        if let Some(r) = once(&exe, &dir) {
            runs.push(r);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let col = |i: usize| -> Vec<f64> { runs.iter().map(|r| r[i]).collect() };
    vec![
        Metric::new("startup.ready", "ms", Better::Lower, Summary::of(&col(0)))
            .ungated()
            .note("spawn to the main window's second frame, fresh data folder, warm file cache"),
        Metric::new("startup.splash", "ms", Better::Lower, Summary::of(&col(1))).ungated(),
        Metric::new("startup.app", "ms", Better::Lower, Summary::of(&col(2))).ungated(),
    ]
}
