//! Screenshots of the interface without a screen: `cargo run -p cantus-ui --example shoot
//! -- <out-dir> [scale]`. Renders the mixer with each plug-in editor open over it, through
//! the software renderer in `cantus_ui::raster`, into PNG files — for the design review,
//! and for machines whose display is locked or absent.
//!
//! The engine is the dummy one, so the analyser and the meters are fed here: a mix of
//! tones and noise through the very EQ the editor shows, and a compressor history made by
//! running the real compressor over a drum-like signal.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The test CLAP plug-in's library (`crates/cantus-test-plugins/clap-gain`), built from
/// source into `target/test-plugins` the first time — where the host's tests build it.
fn test_plugin() -> PathBuf {
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

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);

    // `shoot <dir> [scale] [kind…]`: the kinds named, or every editor and the mixer.
    let kinds: Vec<String> = args.collect();
    let kinds: Vec<String> = if kinds.is_empty() {
        [
            "eq",
            "comp",
            "limiter",
            "space",
            "multiband",
            "deesser",
            "gate",
            "",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    } else {
        kinds
    };
    let shots: Vec<(String, String, bool)> = kinds
        .iter()
        .flat_map(|k| {
            let base = if k.is_empty() {
                "mixer".to_string()
            } else {
                k.clone()
            };
            [
                (format!("{base}-light"), k.clone(), false),
                (format!("{base}-dark"), k.clone(), true),
            ]
        })
        .collect();
    for (name, kind, dark) in shots {
        let kind = kind.as_str();
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let theme = if dark {
            cantus_ui::theme::ink_and_gold()
        } else {
            cantus_ui::theme::default_theme()
        };
        cantus_ui::theme::apply(&theme, &ctx);
        let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        app.central = cantus_ui::app::CentralView::Mixer;
        // `gate-midi`: the gate played by the first track's notes; `space-cut`: a hall too
        // long for the measure's capture, so its low octaves are marked cut (ADR 0022).
        let midi_gate = kind == "gate-midi";
        let cut_space = kind == "space-cut";
        let kind = match kind {
            "gate-midi" => "gate",
            "space-cut" => "space",
            k => k,
        };
        // `plugin`: the generic editor of a hosted plug-in (the test plug-in, built from
        // source); `plugins`: the plug-in manager of Settings.
        let hosted = matches!(kind, "plugin" | "plugins");
        if hosted {
            cantus_ui::views::plugins::demo_setup(&mut app, &test_plugin(), kind == "plugin");
        } else if !kind.is_empty() {
            app.demo_editor(kind);
        }
        if let (true, Some((strip, effect, _))) = (cut_space, app.fx.open) {
            app.exec(cantus_core::Command::SetEffectParam {
                strip,
                effect,
                key: "decay".into(),
                value: 9.0,
            });
        }
        if let (true, Some((strip, effect, _))) = (midi_gate, app.fx.open) {
            use cantus_core::Command;
            for (key, value) in [("trig", 1.0), ("vel", 18.0)] {
                app.exec(Command::SetEffectParam {
                    strip,
                    effect,
                    key: key.into(),
                    value,
                });
            }
            let source = app.project().tracks.first().map(|t| t.id);
            app.exec(Command::SetEffectMidiSource {
                strip,
                effect,
                source,
            });
        }
        // Feed the editors what a playing engine would.
        if kind == "eq" {
            if let Some((strip, effect)) = app.eq_editor.open {
                let spec = app
                    .project()
                    .strip(strip)
                    .unwrap()
                    .effect(effect)
                    .unwrap()
                    .spec
                    .clone();
                let mut eq = cantus_dsp::Aequalis::new(48_000.0);
                eq.set(&cantus_engine::eq_bands(&spec), 0.0);
                let mut seed = 7u32;
                let n = 8192;
                let mut pre: Vec<f32> = (0..n)
                    .map(|i| {
                        let t = i as f32 / 48_000.0;
                        0.3 * (std::f32::consts::TAU * 110.0 * t).sin()
                            + 0.15 * (std::f32::consts::TAU * 440.0 * t).sin()
                            + 0.05 * (std::f32::consts::TAU * 1_760.0 * t).sin()
                            + 0.08 * noise(&mut seed)
                    })
                    .collect();
                let mut r = pre.clone();
                let mut post = pre.clone();
                eq.process(&mut post, &mut r);
                let pairs: Vec<f32> = pre.drain(..).zip(post).flat_map(|(a, b)| [a, b]).collect();
                app.eq_editor.feed(&pairs);
            }
            views::fx::demo_feed(&mut app, cantus_core::EffectKind::Eq);
        }
        if !matches!(kind, "" | "eq" | "comp" | "plugin" | "plugins") {
            if let Some((_, _, k)) = app.fx.open {
                views::fx::demo_feed(&mut app, k);
            }
        }
        if kind == "comp" {
            let mut c = cantus_dsp::Comprimo::new(48_000.0);
            if let Some((strip, effect)) = app.comp_editor.open {
                let spec = app
                    .project()
                    .strip(strip)
                    .unwrap()
                    .effect(effect)
                    .unwrap()
                    .spec
                    .clone();
                c.set(&cantus_engine::comp_settings(&spec));
            }
            let mut seed = 3u32;
            for frame in 0..420 {
                let beat = frame % 30 < 6;
                let amp = if beat { 0.9 } else { 0.08 };
                let mut l: Vec<f32> = (0..800).map(|_| amp * noise(&mut seed)).collect();
                let mut r = l.clone();
                c.process(&mut l, &mut r, None);
                let m = c.meter();
                app.comp_editor.feed(m.input, m.output, m.gr_db);
            }
        }
        let path = dir.join(format!("{name}.png"));
        shoot(
            &ctx,
            egui::vec2(1500.0, 940.0),
            scale,
            6,
            Some(&path),
            |root| {
                let ctx = root.ctx().clone();
                egui::CentralPanel::default().show(root, |ui| views::mixer::show(&mut app, ui));
                views::eq_editor::window(&mut app, &ctx);
                views::comp_editor::window(&mut app, &ctx);
                views::fx::window(&mut app, &ctx);
                if kind == "plugins" {
                    egui::Window::new(cantus_ui::i18n::t("host.manager.title"))
                        .default_size([640.0, 420.0])
                        .show(&ctx, |ui| views::plugins::manager(&mut app, ui));
                }
            },
        );
        println!("wrote {}", path.display());
    }
}
