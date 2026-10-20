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
    // Where this computer says plug-ins run, before the first sync (review isolation-3
    // #29: the shots' *Runs* reads true).
    let mut app = CantusApp::from_parts_with(
        Some(welcome_project()),
        engine,
        false,
        Some(cantus_core::PluginIsolation::Together),
    );
    app.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
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
        let mut app = CantusApp::from_parts_with(
            Some(project),
            engine,
            false,
            Some(cantus_core::PluginIsolation::Together),
        );
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

/// The interface's frames after a crash: the host's news to the card.
fn tell(app: &mut CantusApp) {
    for _ in 0..4 {
        let n = app.plugins.idle(&mut app.bus, &mut app.engine);
        for n in n {
            views::plugin_notice::on_notice(app, n);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// A hosted instrument (the test synth, *Together*) that crashed while the song played,
/// still loaded: what the inspector, the rack and the header show (review isolation-3
/// #29).
fn instrument_crash_app() -> CantusApp {
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts_with(
        Some(welcome_project()),
        engine,
        false,
        Some(cantus_core::PluginIsolation::Together),
    );
    app.plugins
        .set_sandboxes(cantus_host::bridge::Sandboxes::new(sandbox_exe()));
    app.plugins.db_path = None;
    let folder = std::env::temp_dir().join("cantus-shoot-synth");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Synth.clap");
    let _ = std::fs::copy(test_synth(), &file);
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .unwrap_or_default();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    let info = plugins.first().cloned();
    app.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    let track = app.project().tracks[0].id;
    if let Some(info) = info {
        app.set_hosted_instrument(track, &info);
    }
    app.sync_engine();
    app.select_track(track);
    app.engine.play();
    let mut buf = vec![0f32; 512];
    for _ in 0..8 {
        processor.process(&mut buf, 2);
    }
    // The synth's hidden Fault parameter (5): abort in `process`.
    app.exec(cantus_core::Command::SetInstrumentPluginParam {
        track,
        id: 5,
        value: Some(1.0),
    });
    app.sync_engine();
    for _ in 0..16 {
        processor.process(&mut buf, 2);
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    tell(&mut app);
    app
}

/// A card that lists two plug-ins, one of them crashing for the third time with a long
/// name: its three buttons (review isolation-3 #27).
fn three_button_app() -> CantusApp {
    let mut p = welcome_project();
    let strip = p.tracks[0].strip.expect("a strip");
    let mut slot = p.make_effect(cantus_core::EffectKind::External);
    slot.spec = cantus_core::EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Vst3,
        "0123456789ABCDEF0123456789ABCDEF",
        "Spectrasonics Omnisphere Orchestral Edition",
    ));
    let id = slot.id;
    p.strip_mut(strip).expect("the strip").effects.push(slot);
    // Its neighbour in the helper, which it took down with it (review isolation-4 #42: the
    // card lists two, so its third button names the one to run apart).
    let mut glacier = p.make_effect(cantus_core::EffectKind::External);
    glacier.spec = cantus_core::EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.example.glacier",
        "Glacier",
    ));
    let neighbour = glacier.id;
    p.strip_mut(strip).expect("the strip").effects.push(glacier);
    let mut app = CantusApp::from_parts_with(
        Some(p),
        EngineHandle::dummy(),
        false,
        Some(cantus_core::PluginIsolation::Together),
    );
    app.plugins.mark_crashed_for_tests(
        (Some(id), None),
        (
            cantus_host::Format::Vst3,
            "0123456789ABCDEF0123456789ABCDEF",
        ),
        "it crashed: an access violation (0xC0000005)",
        3,
    );
    app.plugins.mark_crashed_for_tests(
        (Some(neighbour), None),
        (cantus_host::Format::Clap, "org.example.glacier"),
        "it crashed: an access violation (0xC0000005)",
        0,
    );
    views::plugin_notice::on_notice(
        &mut app,
        cantus_host::HostNotice::PluginCrashed {
            effect: id,
            name: "Spectrasonics Omnisphere Orchestral Edition".into(),
            reason: "it crashed: an access violation (0xC0000005)".into(),
        },
    );
    views::plugin_notice::on_notice(
        &mut app,
        cantus_host::HostNotice::PluginCrashed {
            effect: neighbour,
            name: "Glacier".into(),
            reason: "it crashed: an access violation (0xC0000005)".into(),
        },
    );
    app.audio_setup
        .notice(cantus_ui::audio_setup::Notice::Lost("Focusrite USB".into()));
    app
}

/// The review's places a stopped plug-in shows: the Wide mixer (the chain line), the
/// insert's menu on the desk, the selected insert (the mini inspector), a loaded
/// instrument that crashed (inspector, rack, header), the three-button card in a narrow
/// window.
fn review3_shots(dir: &std::path::Path, ppp: f32, code: &str) {
    let theme = cantus_ui::theme::cantus_night();
    let shot = |name: &str, size: egui::Vec2, app: &mut CantusApp| {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&theme, &ctx);
        let path = dir.join(format!("isolation-{name}-night-{code}-{ppp}x.png"));
        shoot(&ctx, size, ppp, 6, Some(&path), |root| app.frame(root));
        println!("wrote {}", path.display());
    };
    let size = egui::vec2(1280.0, 760.0);
    // The mixer, Wide: the chain line of the stopped insert in the danger colour.
    let mut app = crash_app();
    app.central = cantus_ui::app::CentralView::Mixer;
    views::mixer::set_width_mode(&mut app, views::mixer::layout::WidthMode::Xl);
    views::mixer::set_row(&mut app, views::mixer::layout::Row::Chain, true);
    shot("mixer", egui::vec2(1280.0, 1200.0), &mut app);
    // The insert's menu on the desk: Reload plug-in, and where it runs.
    let strip = app.project().tracks[0].strip.expect("a strip");
    let effect = app
        .project()
        .strip(strip)
        .and_then(|s| s.effects.iter().find(|e| e.spec.external.is_some()))
        .map(|e| e.id);
    if let Some(e) = effect {
        views::mixer::open_slot_menu(&mut app, strip, e, egui::pos2(420.0, 300.0));
        shot("mixer-menu", size, &mut app);
        // The selected insert: the inspector's tile for it.
        app.inspector.show_insert_values(e);
        app.central = cantus_ui::app::CentralView::PianoRoll;
        shot("insert-selected", size, &mut app);
    }
    // A loaded instrument that crashed: the inspector, and the rack.
    let mut app = instrument_crash_app();
    shot("instrument-loaded", size, &mut app);
    app.central = cantus_ui::app::CentralView::ChannelRack;
    shot("rack", size, &mut app);
    // The three-button card, in a narrow window.
    let mut app = three_button_app();
    shot("three-buttons", egui::vec2(420.0, 600.0), &mut app);
    // A plug-in waiting on a window of its own (review isolation-4 #54), and a window
    // asked for while its helper is busy (#39), in a narrow window.
    let mut app = waiting_app();
    shot("waiting", egui::vec2(420.0, 600.0), &mut app);
}

/// Glacier waits on a window of its own; its window was asked for meanwhile.
fn waiting_app() -> CantusApp {
    let mut p = welcome_project();
    let strip = p.tracks[0].strip.expect("a strip");
    let mut slot = p.make_effect(cantus_core::EffectKind::External);
    slot.spec = cantus_core::EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.example.glacier",
        "Glacier",
    ));
    let id = slot.id;
    p.strip_mut(strip).expect("the strip").effects.push(slot);
    let mut app = CantusApp::from_parts_with(
        Some(p),
        EngineHandle::dummy(),
        false,
        Some(cantus_core::PluginIsolation::Together),
    );
    app.plugins.set_waiting_for_tests(id, true);
    let busy = cantus_host::HostError::Busy("its own window holds it".into());
    views::plugin_notice::window_failed(&mut app, "Glacier", &busy);
    app
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
    review3_shots(&dir, ppp, code);
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
