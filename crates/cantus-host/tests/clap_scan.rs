//! The scanner: it finds the test plug-in, the database caches it by path and modification
//! time, and a plug-in that crashes or hangs while loading is blocked while Cantus — this
//! process — carries on.

mod support;
use cantus_host::{Format, PluginDb, PluginHost, PluginKind, ScanStatus};
use std::time::{Duration, Instant};
use support::*;

#[test]
fn the_scan_finds_the_plugin_and_the_database_caches_it() {
    let folder = plugin_folder(&["cantus-test.clap"]);
    let db_file = folder.path().join("db").join("plugins.json");
    let mut host = PluginHost::new(Some(db_file.clone()), SR);
    host.scanner = scanner(Duration::from_secs(60));
    host.add_folder(folder.path().to_path_buf());

    let first = host.scan_now(false);
    assert_eq!(
        (first.scanned, first.cached, first.plugins),
        (1, 0, 1),
        "{first:?}"
    );
    let info = host.db.find(Format::Clap, PLUGIN_ID).expect("found");
    assert_eq!(info.name, "Cantus Test Gain");
    assert_eq!(info.vendor, "Cantus Tests");
    assert_eq!(info.kind, PluginKind::Effect);
    assert_eq!((info.main_inputs, info.main_outputs), (2, 2));
    assert!(info.sidechain, "the second input port is the sidechain");
    assert_eq!(
        info.editor,
        cfg!(any(windows, target_os = "linux", target_os = "macos")),
        "an editor: Win32 on Windows, X11 on Linux (ADR 0065), Cocoa on macOS (ADR 0066)"
    );
    assert!(info.features.iter().any(|f| f == "audio-effect"));
    assert_eq!(host.db.effects().len(), 1);

    // Unchanged: a cache read, no process started.
    let second = host.scan_now(false);
    assert_eq!((second.scanned, second.cached), (0, 1), "{second:?}");

    // The database survives on disk.
    let reread = PluginDb::load(&db_file);
    assert_eq!(reread, host.db);

    // A changed file is read again.
    let path = folder.path().join("cantus-test.clap");
    let later = std::time::SystemTime::now() + Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .and_then(|f| f.set_modified(later))
        .expect("touch");
    let third = host.scan_now(false);
    assert_eq!((third.scanned, third.cached), (1, 0), "{third:?}");
}

#[test]
fn a_plugin_that_crashes_or_hangs_is_blocked_and_cantus_survives() {
    let folder = plugin_folder(&["cantus-test.clap", "cantus-crash.clap", "cantus-hang.clap"]);
    let mut host = PluginHost::new(None, SR);
    // Long enough for a good plug-in to load on a machine under load, short enough that
    // the hang does not hold the test up for long.
    host.scanner = scanner(Duration::from_secs(15));
    host.add_folder(folder.path().to_path_buf());

    let started = Instant::now();
    let report = host.scan_now(false);
    assert_eq!(report.blocked, 2, "{report:?}");
    assert_eq!(report.plugins, 1, "the good copy is still found");
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "the hang was cut off"
    );

    let blocked = host.db.blocked();
    assert_eq!(blocked.len(), 2);
    let reason = |name: &str| match &host
        .db
        .entry(&folder.path().join(name))
        .expect("an entry")
        .status
    {
        ScanStatus::Blocked(why) => why.clone(),
        other => panic!("{name}: {other:?}"),
    };
    assert!(reason("cantus-crash.clap").contains("crashed"));
    assert!(reason("cantus-hang.clap").contains("did not answer"));

    // Blocked stays blocked: not retried by an ordinary scan…
    let again = host.scan_now(false);
    assert_eq!((again.scanned, again.blocked), (0, 0), "{again:?}");
    assert_eq!(host.db.blocked().len(), 2);
    // …and a blocked plug-in is never offered.
    assert_eq!(host.db.plugins().len(), 1);

    // Until the user asks: forgotten, read again, blocked again.
    host.db.forget(&folder.path().join("cantus-crash.clap"));
    let retry = host.scan_now(false);
    assert_eq!((retry.scanned, retry.blocked), (1, 1), "{retry:?}");
}
