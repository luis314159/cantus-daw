//! The boot path (ADR 0073; the app never calls `CantusApp::new`: start-up runs through
//! `Files::read` and `CantusApp::assemble`): each plug-in's default view comes from the
//! settings file, and a plug-in inserted after start opens in it. A process of its own: it
//! sets the data folder for the whole process.

mod hosted_support;

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_host::db::{stamp, DbEntry, PluginDb, ScanStatus};
use cantus_host::{Backend, Format};
use cantus_ui::app::Files;
use cantus_ui::hosted_view::HostedView;
use cantus_ui::views::plugin_notice::Target;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_reads_each_plugins_default_view_and_an_insert_opens_in_it() {
    let _one = hosted_support::serial();
    let data = tempfile::tempdir().unwrap();
    let folder = data.path().join("plugins");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("Cantus Test Gain.clap");
    std::fs::copy(hosted_support::gain_library(), &path).unwrap();
    let infos = cantus_host::clap::ClapBackend.scan(&path).expect("scans");
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
    // A settings file of a user who made Cantus's view this plug-in's default.
    let mut settings = cantus_ui::settings::Settings::default();
    settings
        .hosted_views
        .set(PluginFormat::Clap, &info.id, Some(HostedView::Cantus));
    settings
        .save(&cantus_ui::settings::settings_path(data.path()))
        .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let mut files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    assert_eq!(
        files
            .settings
            .hosted_views
            .get(PluginFormat::Clap, &info.id),
        Some(HostedView::Cantus),
        "read from the file"
    );
    files.settings.plugin_isolation = PluginIsolation::Within;
    files.settings.plugin_scan_at_start = false;
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        app.settings.hosted_views.get(PluginFormat::Clap, &info.id),
        Some(HostedView::Cantus),
        "assemble keeps it"
    );
    let strip = hosted_support::first_strip(&app);
    let fx = hosted_support::insert(&mut app, strip, &info);
    assert!(
        app.cantus_view_open(Target::Insert(fx)),
        "opens in its default"
    );
    assert!(!app.native_open(Target::Insert(fx)));
}
