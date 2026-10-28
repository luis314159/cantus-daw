//! What the hosted-view tests share (ADR 0073): the test CLAP plug-ins built from source,
//! copied under a name of the test's own and scanned into an app's database, and the
//! plug-in put on a strip or a track as the user's insert and pick do.
#![allow(dead_code)]

use cantus_core::*;
use cantus_host::{Backend, DbEntry, PluginInfo, ScanStatus};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// `cargo build -p <package>` into `target/test-plugins` (its own target folder: the
/// test's cargo holds the workspace's), and the library or program it made.
fn build(package: &str, file: &str) -> PathBuf {
    let target = root().join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", package, "--target-dir"])
            .arg(&target)
            .current_dir(root())
            .status()
            .expect("cargo");
    assert!(status.success(), "{package} builds");
    target.join("debug").join(file)
}

fn library(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

/// The test gain (`cantus-test-clap`).
pub fn gain_library() -> PathBuf {
    build("cantus-test-clap", &library("cantus_test_clap"))
}

/// The test synth (`cantus-test-clap-synth`).
pub fn synth_library() -> PathBuf {
    build("cantus-test-clap-synth", &library("cantus_test_clap_synth"))
}

/// This build's plug-in helper (`cantus-sandbox`), for the out-of-process tests.
pub fn sandbox_exe() -> PathBuf {
    let exe = if cfg!(windows) {
        "cantus-sandbox.exe"
    } else {
        "cantus-sandbox"
    };
    build("cantus-sandbox", exe)
}

/// One build and copy at a time in a process: the tests run in parallel.
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// `library` copied into `dir` as `name` (a name with `nogui` has no editor), scanned,
/// and put in the app's database as a scan leaves it. Its first plug-in.
pub fn know(app: &mut CantusApp, library: &Path, dir: &Path, name: &str) -> PluginInfo {
    app.plugins.db_path = None;
    let file = dir.join(name);
    std::fs::copy(library, &file).expect("the test plug-in copies");
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .expect("the test plug-in scans");
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    let info = plugins[0].clone();
    app.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    info
}

/// Insert `info` on `strip` as the insert menu and *Add effect* do: in its view. The new
/// effect.
pub fn insert(app: &mut CantusApp, strip: StripId, info: &PluginInfo) -> EffectId {
    assert!(app.add_picked_effect(
        strip,
        &cantus_ui::picker::PickChoice::Hosted(Box::new(info.clone()))
    ));
    app.sync_engine();
    app.project()
        .effects_of(strip)
        .and_then(|c| c.last())
        .map(|e| e.id)
        .expect("inserted")
}

/// The first strip of the starter project.
pub fn first_strip(app: &CantusApp) -> StripId {
    app.project().tracks[0].strip.expect("a strip")
}

/// Run the app's per-frame host work (its plug-ins' news, the opens waiting for them) until
/// `done`, at most `secs` seconds.
pub fn pump_until(
    app: &mut CantusApp,
    secs: f32,
    mut done: impl FnMut(&CantusApp) -> bool,
) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs_f32(secs);
    loop {
        let _ = app.plugins.idle(&mut app.bus, &mut app.engine);
        app.sync_engine();
        app.pump_hosted_opens();
        if done(app) {
            return true;
        }
        if std::time::Instant::now() > until {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
