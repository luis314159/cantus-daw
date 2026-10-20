//! The boot path and where a start-up project's plug-ins first load (round 29, review
//! #14): this computer's choice is in the host before the first sync, so a hosted plug-in
//! is never loaded within Cantus first and moved after. A process of its own: it sets the
//! data folder and the helper's path for the whole process.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_host::db::{stamp, DbEntry, PluginDb, ScanStatus};
use cantus_host::Format;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::PathBuf;

/// The CLAP test gain, built once into `target/test-plugins` (its own target folder, so
/// this test's cargo does not wait on the one running it).
fn test_gain() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    });
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = std::process::Command::new(cargo)
        .args(["build", "-p", "cantus-test-clap", "--target-dir"])
        .arg(&target)
        .current_dir(&root)
        .status()
        .expect("cargo builds the test gain");
    assert!(status.success());
    file
}

#[test]
fn a_start_up_projects_plugin_first_loads_where_this_computer_says() {
    let data = tempfile::tempdir().unwrap();
    let folder = data.path().join("plugins");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("cantus-test-gain.clap");
    std::fs::copy(test_gain(), &path).unwrap();
    let infos = cantus_host::backend_for(Format::Clap)
        .unwrap()
        .scan(&path)
        .expect("the gain scans");
    let info = infos[0].clone();
    let (modified, size) = stamp(&path).unwrap_or((0, 0));
    let mut db = PluginDb::default();
    db.put(DbEntry {
        path: path.clone(),
        format: Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins: infos,
    });
    db.save(&data.path().join("plugins.json")).unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    // No helper on this machine: plug-ins set apart run within Cantus — and the host says
    // so at the first sync, which it can only do if it already knows they are set apart.
    std::env::set_var(
        cantus_host::bridge::sandbox::EXE_ENV,
        data.path().join("no-such-cantus-sandbox"),
    );
    let mut project = cantus_ui::starter_project();
    let strip = project.tracks[0].strip.unwrap();
    let mut slot = project.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        info.id.clone(),
        info.name.clone(),
    ));
    let effect = slot.id;
    project.strip_mut(strip).unwrap().effects.push(slot);
    let mut files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    files.settings.plugin_isolation = PluginIsolation::Together;
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
    assert_eq!(app.plugins.default_isolation, PluginIsolation::Together);
    assert_eq!(
        app.plugins.isolation(effect),
        Some(PluginIsolation::Within),
        "loaded (within: the helper is missing)"
    );
    assert!(app.status.error, "{}", app.status.text);
    assert_eq!(
        app.status.text,
        cantus_ui::i18n::t("isolation.unavailable"),
        "the first sync already knew it was to run apart"
    );
}
