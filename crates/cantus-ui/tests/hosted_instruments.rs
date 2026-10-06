//! Hosted instruments in the application (ADR 0028): the picker's `Hosted` choice puts a
//! CLAP instrument on a track in one undo step, the inspector shows its card, the track
//! opens the plug-in's own window, and a project whose instrument is missing opens, says
//! so and saves its record back untouched.

mod support;
use cantus_core::*;
use cantus_host::{Backend, DbEntry, PluginInfo, ScanStatus};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use support::*;

/// The test instrument, built from source where the host's tests build it.
fn test_synth() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap_synth.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap_synth.dylib"
    } else {
        "libcantus_test_clap_synth.so"
    })
}

/// The app's plug-in database knows the test instrument, as after a scan.
fn knows_synth(a: &mut CantusApp) -> PluginInfo {
    a.plugins.db_path = None;
    let dir = tempfile::tempdir().unwrap().keep();
    let file = dir.join("Cantus Test Synth.clap");
    std::fs::copy(test_synth(), &file).unwrap();
    let plugins = cantus_host::clap::ClapBackend.scan(&file).unwrap();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap();
    a.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    let list = a.hosted_instruments();
    assert_eq!(list.len(), 1, "one instrument scanned: {list:?}");
    list[0].clone()
}

fn inspector_text(a: &mut CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(420.0, 4000.0), 3, |ui| {
        cantus_ui::views::inspector::show(a, ui)
    })
}

#[test]
fn the_picker_puts_a_hosted_instrument_on_a_track_in_one_step() {
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    let before = a.bus.history().len();
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info.clone())),
    );
    assert_eq!(a.bus.history().len(), before + 1, "one undo step");
    let plugin = a
        .project()
        .track(track)
        .unwrap()
        .instrument
        .external()
        .cloned();
    let plugin = plugin.expect("the track hosts it");
    assert_eq!(plugin.name, "Cantus Test Synth");
    assert!(!plugin.state.is_empty());
    assert!(
        a.plugins.hosted_instrument(track).is_some(),
        "loaded once, kept"
    );
    a.select_track(track);
    let text = inspector_text(&mut a);
    assert!(text.contains("Cantus Test Synth"), "{text}");
    assert!(text.contains("CLAP"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("hosted.status_ok")),
        "{text}"
    );
    // Undo gives the track its old instrument back, and the plug-in goes.
    a.undo();
    assert!(a
        .project()
        .track(track)
        .unwrap()
        .instrument
        .firmus()
        .is_some());
    a.sync_engine();
    assert!(a.plugins.hosted_instrument(track).is_none());
    // `AddInstrument`: a new track that plays it.
    let tracks = a.project().tracks.len();
    a.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Hosted(Box::new(info)),
    );
    assert_eq!(a.project().tracks.len(), tracks + 1);
    let last = a.project().tracks.last().unwrap();
    assert!(last.instrument.external().is_some());
}

#[cfg(windows)]
#[test]
fn opening_its_editor_from_the_track_opens_the_plugin_window() {
    let mut a = app(cantus_ui::starter_project());
    let info = knows_synth(&mut a);
    let track = a.project().tracks[0].id;
    assert!(a.set_hosted_instrument(track, &info));
    a.sync_engine();
    a.open_inst_editor(track);
    assert!(
        a.plugins.instrument_window_open(track),
        "the plug-in's own window"
    );
    assert!(a.inst.open.is_none(), "not the native frame");
    a.toggle_instrument_window(track);
    assert!(!a.plugins.instrument_window_open(track));
}

#[test]
fn a_missing_hosted_instrument_opens_says_so_and_saves_back_untouched() {
    let mut p = cantus_ui::starter_project();
    let mut plugin = ExternalPlugin::new(
        PluginFormat::Vst3,
        "0123456789ABCDEF0123456789ABCDEF",
        "Far Synth",
    );
    plugin.vendor = "Elsewhere Audio".into();
    plugin.state = PluginState::new(vec![9, 8, 7, 6]);
    plugin.params.insert(3, -1.5);
    p.tracks[0].instrument = InstrumentSpec::External(plugin.clone());
    let track = p.tracks[0].id;
    let mut a = app(p);
    a.sync_engine();
    assert!(a.plugins.instrument_missing(track).is_some());
    a.select_track(track);
    let text = inspector_text(&mut a);
    assert!(text.contains("Far Synth"), "{text}");
    assert!(text.contains("VST3"), "{text}");
    assert!(text.contains("Elsewhere Audio"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("hosted.status_missing")),
        "{text}"
    );
    // Its editor cannot open; the status line says why instead.
    a.open_inst_editor(track);
    assert!(!a.plugins.instrument_window_open(track));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.cantus");
    a.save_to_path(&path);
    let back = cantus_project::load(&path, true).unwrap().project;
    assert_eq!(back.tracks[0].instrument, InstrumentSpec::External(plugin));
}
