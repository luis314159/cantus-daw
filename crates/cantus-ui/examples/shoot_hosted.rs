//! Screenshots of a hosted instrument in the inspector (ADR 0028), without a screen:
//! `cargo run -p cantus-ui --example shoot_hosted -- <out-dir> [ppp] [locale]`.
//!
//! Two tracks of the welcome song, each in Slate and Paper:
//! - `loaded`: the repository's test instrument (`cantus-test-clap-synth`, built from
//!   source into `target/test-plugins`) put on Keys as the picker does;
//! - `missing`: a hosted instrument installed nowhere — it opens, plays silence, says so.

use cantus_core::{ExternalPlugin, InstrumentSpec, PluginFormat, PluginState};
use cantus_engine::EngineHandle;
use cantus_host::{Backend, DbEntry, ScanStatus};
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

/// The test instrument's library, built from source the first time.
fn test_synth() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap_synth.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap_synth.dylib"
    } else {
        "libcantus_test_clap_synth.so"
    });
    if !file.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    file
}

/// The test instrument in the app's plug-in database, as a scan would leave it.
fn know_synth(app: &mut CantusApp) {
    app.plugins.db_path = None;
    let folder = std::env::temp_dir().join("cantus-shoot-instruments");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Synth.clap");
    let _ = std::fs::copy(test_synth(), &file);
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .unwrap_or_default();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    app.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let ppp: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    for kind in ["loaded", "missing"] {
        for (theme_name, theme) in [
            ("slate", cantus_ui::theme::slate()),
            ("paper", cantus_ui::theme::paper()),
        ] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            let keys = app.project().tracks[0].id;
            if kind == "loaded" {
                know_synth(&mut app);
                let info = app.hosted_instruments().first().cloned();
                if let Some(info) = info {
                    app.set_hosted_instrument(keys, &info);
                }
            } else {
                let mut plugin =
                    ExternalPlugin::new(PluginFormat::Vst3, "0123456789ABCDEF0123456789ABCDEF", "Odin 2");
                plugin.vendor = "The Wave Warden".into();
                plugin.state = PluginState::new(vec![1, 2, 3]);
                app.set_instrument(keys, InstrumentSpec::External(plugin));
            }
            app.sync_engine();
            app.select_track(keys);
            let path = dir.join(format!("hosted-{kind}-{theme_name}-{code}-{ppp}x.png"));
            let size = egui::vec2(290.0, 900.0);
            shoot(&ctx, size, ppp, 6, Some(&path), |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::inspector::show(&mut app, ui));
            });
            println!("wrote {}", path.display());
        }
    }
}
