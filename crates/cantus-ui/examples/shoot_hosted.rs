//! Screenshots of hosted plug-ins (ADR 0028, ADR 0063), without a screen:
//! `cargo run -p cantus-ui --example shoot_hosted -- <out-dir> [ppp] [locale]`.
//!
//! Two tracks of the welcome song, each in Slate and Paper:
//! - `loaded`: the repository's test instrument (`cantus-test-clap-synth`, built from
//!   source into `target/test-plugins`) put on Keys as the picker does;
//! - `missing`: a hosted instrument installed nowhere — it opens, plays silence, says so.
//!
//! And round 29's isolation, in Cantus Night and Cantus Paper (`cantus-sandbox` built
//! beside the example, or `CANTUS_SANDBOX_EXE`):
//! - `crash`: the whole window, a real crash — the test gain (`cantus-test-clap`) runs in its
//!   helper on the first strip, its hidden *Fault* makes it abort inside `process` while
//!   the song plays, and the corner card says so, in the stack it shares with an audio
//!   notice (an interface unplugged); also at a narrow window (`crash-narrow`);
//! - `manager`: Settings' plug-in manager, *Where third-party plug-ins run*, with the
//!   plug-in that stopped remembered.

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

/// The test gain's library, built from source the first time.
fn test_gain() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    });
    if !file.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    file
}

/// The helper: beside the example's folder (`target/<profile>/`), unless the environment
/// names one.
fn sandbox_exe() -> PathBuf {
    if let Some(p) = std::env::var_os(cantus_host::bridge::sandbox::EXE_ENV) {
        return PathBuf::from(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.parent()
                .and_then(|d| d.parent())
                .map(|d| d.join(cantus_host::bridge::sandbox::EXE))
        })
        .unwrap_or_default()
}

/// The window with a plug-in that really crashed: the card, the slot, the status bar.
fn crash_app() -> CantusApp {
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(welcome_project()), engine, false);
    app.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    app.plugins.default_isolation = cantus_core::PluginIsolation::Together;
    views::plugins::demo_setup(&mut app, &test_gain(), false);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let info = app.plugins.db.effects().first().map(|p| (*p).clone());
    if let Some(info) = info {
        app.add_plugin(strip, &info);
    }
    app.sync_engine();
    let effect = app
        .project()
        .strip(strip)
        .and_then(|s| s.effects.iter().find(|e| e.spec.external.is_some()))
        .map(|e| e.id);
    app.select_strip(strip);
    app.engine.play();
    let mut buf = vec![0f32; 512];
    for _ in 0..8 {
        processor.process(&mut buf, 2);
    }
    if let Some(e) = effect {
        // The hidden Fault parameter (`clap-gain/src/fault.rs`): abort in `process`.
        app.set_plugin_param(strip, e, 4, 1.0, false);
        app.sync_engine();
    }
    for _ in 0..16 {
        processor.process(&mut buf, 2);
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    // The interface's frame tells the host's news to the card.
    for _ in 0..3 {
        let n = app.plugins.idle(&mut app.bus, &mut app.engine);
        for n in n {
            views::plugin_notice::on_notice(&mut app, n);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    app.audio_setup
        .notice(cantus_ui::audio_setup::Notice::Lost("Focusrite USB".into()));
    app
}

fn isolation_shots(dir: &std::path::Path, ppp: f32, code: &str) {
    for (theme_name, theme) in [
        ("night", cantus_ui::theme::cantus_night()),
        ("paper", cantus_ui::theme::cantus_paper()),
    ] {
        for (kind, size) in [
            ("crash", egui::vec2(1280.0, 760.0)),
            ("crash-narrow", egui::vec2(820.0, 620.0)),
        ] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            let mut app = crash_app();
            let path = dir.join(format!("isolation-{kind}-{theme_name}-{code}-{ppp}x.png"));
            shoot(&ctx, size, ppp, 6, Some(&path), |root| app.frame(root));
            println!("wrote {}", path.display());
        }
        // The generic editor of the stopped plug-in: its *Runs* card and Reload.
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&theme, &ctx);
        let mut app = crash_app();
        let strip = app.project().tracks[0].strip.expect("a strip");
        let effect = app
            .project()
            .strip(strip)
            .and_then(|s| s.effects.iter().find(|e| e.spec.external.is_some()))
            .map(|e| e.id);
        if let Some(e) = effect {
            app.open_effect_editor(strip, e);
        }
        let path = dir.join(format!("isolation-editor-{theme_name}-{code}-{ppp}x.png"));
        shoot(
            &ctx,
            egui::vec2(1280.0, 760.0),
            ppp,
            6,
            Some(&path),
            |root| app.frame(root),
        );
        println!("wrote {}", path.display());
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&theme, &ctx);
        let mut app = crash_app();
        let path = dir.join(format!("isolation-manager-{theme_name}-{code}-{ppp}x.png"));
        shoot(
            &ctx,
            egui::vec2(640.0, 1000.0),
            ppp,
            6,
            Some(&path),
            |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| views::plugins::manager(&mut app, ui));
                });
            },
        );
        println!("wrote {}", path.display());
        // A hosted instrument with a long name that stopped working (review #32, #34, #36):
        // its card (the name shortened in its middle, the verb whole), the inspector's
        // danger status and Reload, the rack's outline, the header's sign.
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&theme, &ctx);
        let mut project = welcome_project();
        let plugin = cantus_core::ExternalPlugin::new(
            cantus_core::PluginFormat::Vst3,
            "0123456789ABCDEF0123456789ABCDEF",
            "Spectrasonics Omnisphere Orchestral Edition",
        );
        project.tracks[0].instrument = cantus_core::InstrumentSpec::External(plugin.clone());
        let track = project.tracks[0].id;
        let (engine, _processor) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::from_parts(Some(project), engine, false);
        app.plugins.mark_crashed_for_tests(
            (None, Some((track, cantus_engine::hosted_key(&plugin)))),
            (cantus_host::Format::Vst3, &plugin.id),
            "it crashed: an access violation (0xC0000005)",
            1,
        );
        views::plugin_notice::on_notice(
            &mut app,
            cantus_host::HostNotice::InstrumentCrashed {
                track,
                name: plugin.name.clone(),
                reason: "it crashed: an access violation (0xC0000005)".into(),
            },
        );
        app.select_track(track);
        // (The status line's word on the staged plug-in not being installed is not the shot.)
        app.set_status(String::new());
        let path = dir.join(format!(
            "isolation-instrument-{theme_name}-{code}-{ppp}x.png"
        ));
        shoot(
            &ctx,
            egui::vec2(1280.0, 760.0),
            ppp,
            6,
            Some(&path),
            |root| app.frame(root),
        );
        println!("wrote {}", path.display());
    }
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
    isolation_shots(&dir, ppp, code);
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
                let mut plugin = ExternalPlugin::new(
                    PluginFormat::Vst3,
                    "0123456789ABCDEF0123456789ABCDEF",
                    "Odin 2",
                );
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
