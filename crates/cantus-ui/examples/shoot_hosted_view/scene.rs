//! `shoot <dir> [scale] hostedview`: a hosted plug-in in Cantus's view (ADR 0073, BACKLOG
//! §6h point 2), in Cantus Night and Cantus Paper — `CANTUS_SHOOT_LANG=es` for Spanish:
//! - `hosted-view-<theme>`: the test gain on the first strip, described as a synth's 38
//!   parameters in eight pages (two of them meters, in *Output*), over the mixer: the switch *Plug-in
//!   window | Cantus view*, *As default*, the search, the cards;
//! - `hosted-view-search-<theme>`: the same with "env" typed — two pages left, the count;
//! - `hosted-view-inst-<theme>`: the test synth on Keys in its Cantus view (its two meters,
//!   *Tempo seen* and *Playing seen*), the inspector beside it with the *Editor* and
//!   *Remember* rows.

use cantus_engine::EngineHandle;
use cantus_ui::hosted_view::HostedView;
use cantus_ui::raster::shoot;
use cantus_ui::views::plugin_notice::Target;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

/// A page of a synth: (group, name, min, max, value, stepped, read-only).
type Row = (&'static str, &'static str, f64, f64, f64, bool, bool);

const SYNTH: &[Row] = &[
    ("Oscillator A", "Wave", 0.0, 3.0, 2.0, true, false),
    ("Oscillator A", "Octave", -3.0, 3.0, 0.0, true, false),
    ("Oscillator A", "Fine", -100.0, 100.0, 7.0, false, false),
    ("Oscillator A", "Level", 0.0, 1.0, 0.8, false, false),
    ("Oscillator B", "Wave", 0.0, 3.0, 1.0, true, false),
    ("Oscillator B", "Octave", -3.0, 3.0, -1.0, true, false),
    ("Oscillator B", "Fine", -100.0, 100.0, -7.0, false, false),
    ("Oscillator B", "Level", 0.0, 1.0, 0.55, false, false),
    ("Filter", "Cutoff", 20.0, 20_000.0, 2_400.0, false, false),
    ("Filter", "Resonance", 0.0, 1.0, 0.35, false, false),
    ("Filter", "Drive", 0.0, 1.0, 0.2, false, false),
    ("Filter", "Env amount", -1.0, 1.0, 0.45, false, false),
    ("Filter", "Key track", 0.0, 1.0, 0.5, false, false),
    ("Output", "Volume", -60.0, 6.0, -3.0, false, false),
    ("Output", "Pan", -1.0, 1.0, 0.0, false, false),
    ("Output", "Width", 0.0, 2.0, 1.2, false, false),
    ("Output", "Voices", 1.0, 16.0, 8.0, true, false),
    ("Output", "Glide", 0.0, 1.0, 0.0, false, false),
    ("Output", "Bend range", 0.0, 24.0, 2.0, true, false),
    ("Output", "Velocity", 0.0, 1.0, 0.75, false, false),
    ("Output", "Output level", 0.0, 1.0, 0.68, false, true),
    ("Output", "Gain reduction", 0.0, 1.0, 0.22, false, true),
    ("Amp Envelope", "Attack", 0.0, 1.0, 0.02, false, false),
    ("Amp Envelope", "Decay", 0.0, 1.0, 0.3, false, false),
    ("Amp Envelope", "Sustain", 0.0, 1.0, 0.7, false, false),
    ("Amp Envelope", "Release", 0.0, 1.0, 0.4, false, false),
    ("Filter Envelope", "Attack", 0.0, 1.0, 0.0, false, false),
    ("Filter Envelope", "Decay", 0.0, 1.0, 0.45, false, false),
    ("Filter Envelope", "Sustain", 0.0, 1.0, 0.2, false, false),
    ("Filter Envelope", "Release", 0.0, 1.0, 0.3, false, false),
    ("LFO 1", "Rate", 0.0, 1.0, 0.25, false, false),
    ("LFO 1", "Depth", 0.0, 1.0, 0.6, false, false),
    ("LFO 1", "Shape", 0.0, 4.0, 1.0, true, false),
    ("LFO 1", "Delay", 0.0, 1.0, 0.1, false, false),
    ("Effects", "Chorus", 0.0, 1.0, 0.3, false, false),
    ("Effects", "Delay mix", 0.0, 1.0, 0.18, false, false),
    ("Effects", "Delay time", 0.0, 1.0, 0.5, false, false),
    ("Effects", "Reverb", 0.0, 1.0, 0.25, false, false),
];

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme_family::cantus_paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

/// The test gain on the first strip in its Cantus view, described as the synth above.
fn effect_app() -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.central = cantus_ui::app::CentralView::Mixer;
    views::plugins::demo_setup(&mut app, &super::test_plugin(), true);
    let Some((strip, effect, _)) = app.fx.open else {
        return app;
    };
    let params: Vec<cantus_host::ParamInfo> = SYNTH
        .iter()
        .enumerate()
        .map(|(i, r)| cantus_host::ParamInfo {
            id: 100 + i as u32,
            name: r.1.into(),
            group: r.0.into(),
            min: r.2,
            max: r.3,
            default: r.4,
            stepped: r.5,
            automatable: !r.6,
            hidden: false,
            read_only: r.6,
            steps: 0,
        })
        .collect();
    app.plugins.set_params_for_tests(effect, params);
    for (i, r) in SYNTH.iter().enumerate() {
        app.set_plugin_param(strip, effect, 100 + i as u32, r.4, false);
    }
    app
}

fn draw_effect(app: &mut CantusApp, root: &mut egui::Ui) {
    let ctx = root.ctx().clone();
    egui::CentralPanel::default().show(root, |ui| views::mixer::show(app, ui));
    views::fx::window(app, &ctx);
}

/// The test synth, scanned into the app's database from a copy of its own.
fn know_synth(app: &mut CantusApp) -> Option<cantus_host::PluginInfo> {
    use cantus_host::{Backend, DbEntry, ScanStatus};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let lib = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap_synth.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap_synth.dylib"
    } else {
        "libcantus_test_clap_synth.so"
    });
    if !lib.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    let folder = std::env::temp_dir().join("cantus-shoot-hosted-view");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Synth.clap");
    let _ = std::fs::copy(&lib, &file);
    let plugins = cantus_host::clap::ClapBackend.scan(&file).ok()?;
    let info = plugins.first().cloned()?;
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    app.plugins.db_path = None;
    app.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    Some(info)
}

pub fn shots(dir: &Path, scale: f32) {
    let lang = match std::env::var("CANTUS_SHOOT_LANG").as_deref() {
        Ok("es") => "-es",
        _ => "",
    };
    for (theme, dark) in [("night", true), ("paper", false)] {
        let ctx = context(dark);
        let mut app = effect_app();
        let path = dir.join(format!("hosted-view-{theme}{lang}.png"));
        shoot(
            &ctx,
            egui::vec2(1500.0, 940.0),
            scale,
            6,
            Some(&path),
            |root| draw_effect(&mut app, root),
        );
        println!("wrote {}", path.display());
        let ctx = context(dark);
        app.fx.hosted.filter = "env".into();
        let path = dir.join(format!("hosted-view-search-{theme}{lang}.png"));
        shoot(
            &ctx,
            egui::vec2(1500.0, 940.0),
            scale,
            6,
            Some(&path),
            |root| draw_effect(&mut app, root),
        );
        println!("wrote {}", path.display());
        // A hosted instrument in its Cantus view, the inspector beside it.
        let ctx = context(dark);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        let Some(info) = know_synth(&mut app) else {
            eprintln!("the test synth did not build: no instrument shot");
            continue;
        };
        let track = app.project().tracks[0].id;
        if !app.set_hosted_instrument(track, &info) {
            continue;
        }
        let target = Target::Instrument(track);
        app.choose_hosted_view(target, HostedView::Cantus);
        app.open_inst_editor(track);
        app.selected_track = Some(track);
        let path = dir.join(format!("hosted-view-inst-{theme}{lang}.png"));
        shoot(
            &ctx,
            egui::vec2(1900.0, 940.0),
            scale,
            6,
            Some(&path),
            |root| {
                let ctx = root.ctx().clone();
                let full = root.max_rect();
                let split = full.right() - 400.0;
                let desk = egui::Rect::from_min_max(full.min, egui::pos2(split, full.bottom()));
                let side = egui::Rect::from_min_max(egui::pos2(split, full.top()), full.max);
                root.scope_builder(egui::UiBuilder::new().max_rect(desk), |ui| {
                    views::mixer::show(&mut app, ui)
                });
                root.scope_builder(egui::UiBuilder::new().max_rect(side), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| views::inspector::show(&mut app, ui))
                });
                views::plugin_view_inst::window(&mut app, &ctx);
            },
        );
        println!("wrote {}", path.display());
    }
}
