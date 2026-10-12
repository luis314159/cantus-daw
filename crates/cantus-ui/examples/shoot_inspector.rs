//! Screenshots of the inspector without a screen (ADR 0024 §7):
//! `cargo run -p cantus-ui --example shoot_inspector -- <out-dir> [ppp] [locale] [zoom]`.
//!
//! Three selections, each in Slate and Paper:
//! - `strip`: a channel with four inserts (one of them a hosted plug-in), two sends and
//!   its EQ on, chosen on the desk;
//! - `track`: a track header clicked, so the track sits above its channel;
//! - `orphan`: a strip nothing feeds, chosen after a track — which must not show that
//!   track's instrument (`cantus-audit.md` §4.7).
//!
//! `zoom` is the UI scale (0.75 … 2.0) the collision review renders at.

use cantus_core::{Command, EffectKind, EqField, Send};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

/// The test CLAP plug-in, built from source into `target/test-plugins` the first time.
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

/// The welcome song with a channel worth inspecting: Keys' strip carries four inserts
/// (Aequalis, Comprimo, Spatium and the hosted test plug-in), sends to two returns, and
/// an EQ that is not flat; a bus nothing feeds sits at the end of the desk.
fn scene(app: &mut CantusApp, hosted: bool) -> (cantus_core::StripId, cantus_core::StripId) {
    let keys = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.expect("keys has a strip");
    for kind in [EffectKind::Eq, EffectKind::Compressor, EffectKind::Space] {
        app.add_effect(strip, kind);
    }
    if hosted {
        views::plugins::demo_setup(app, &test_plugin(), false);
        if let Some(info) = app.plugins.db.effects().first().map(|p| (*p).clone()) {
            app.add_plugin(strip, &info);
            app.sync_engine();
        }
    }
    let effects: Vec<_> = app.project().strip(strip).unwrap().effects.clone();
    if let Some(space) = effects.iter().find(|e| e.spec.kind == EffectKind::Space) {
        app.exec(Command::SetEffectMix {
            strip,
            effect: space.id,
            mix: 0.35,
        });
        app.exec(Command::SetEffectBypass {
            strip,
            effect: space.id,
            bypass: true,
        });
    }
    if let Some(comp) = effects
        .iter()
        .find(|e| e.spec.kind == EffectKind::Compressor)
    {
        app.exec(Command::SetEffectMix {
            strip,
            effect: comp.id,
            mix: 0.8,
        });
    }
    for (band, field, value) in [
        (0u8, EqField::Gain, 2.5),
        (1, EqField::Gain, -3.5),
        (1, EqField::Freq, 1_200.0),
        (1, EqField::Q, 1.1),
        (2, EqField::Gain, 3.0),
    ] {
        app.exec(Command::SetStripEq {
            id: strip,
            band,
            field,
            value,
        });
    }
    // Two returns and a bus nothing feeds.
    let mut ids = Vec::new();
    for name in ["Reverb", "Delay", "Bus"] {
        let s = app.bus.new_strip(name);
        ids.push(s.id);
        let index = app.project().strips.len();
        app.exec(Command::InsertStrip {
            index,
            strip: Box::new(s),
        });
    }
    app.set_strip_send(
        strip,
        Send {
            level: 0.5,
            ..Send::new(ids[0])
        },
    );
    app.set_strip_send(
        strip,
        Send {
            level: 0.24,
            ..Send::new(ids[1])
        },
    );
    app.end_gesture();
    app.exec(Command::SetStripGain {
        id: strip,
        gain_db: -2.4,
    });
    app.exec(Command::SetTrackGain {
        id: keys,
        gain_db: -3.0,
    });
    (strip, ids[2])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let ppp: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    let zoom: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let hosted = std::env::var_os("CANTUS_SHOOT_NO_PLUGIN").is_none();
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    let tall = std::env::var("CANTUS_SHOOT_HEIGHT")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(900.0);
    let wide = std::env::var("CANTUS_SHOOT_WIDTH")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(290.0);

    let kinds: Vec<String> = std::env::var("CANTUS_SHOOT_KINDS")
        .map(|k| k.split(',').map(str::to_string).collect())
        .unwrap_or_else(|_| {
            ["strip", "track", "orphan", "mini"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        });
    for kind in kinds.iter().map(String::as_str) {
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
            let (strip, orphan) = scene(&mut app, hosted);
            let keys = app.project().tracks[0].id;
            match kind {
                "strip" => {
                    app.select_track(keys);
                    app.select_strip(strip);
                }
                "mini" => {
                    // Every insert's mini view open: curves, values, the hosted one.
                    app.select_track(keys);
                    app.select_strip(strip);
                    let ids: Vec<_> = app
                        .project()
                        .strip(strip)
                        .unwrap()
                        .effects
                        .iter()
                        .map(|e| e.id)
                        .collect();
                    for e in ids {
                        app.inspector.toggle_mini(e);
                    }
                }
                "track" => {
                    app.select_strip(strip);
                    app.select_track(app.project().tracks[1].id);
                    app.select_track(keys);
                }
                _ => {
                    app.select_track(keys);
                    app.select_strip(orphan);
                }
            }
            let path = dir.join(format!(
                "inspector-{kind}-{theme_name}-{code}-{}x{}.png",
                ppp,
                if zoom == 1.0 {
                    String::new()
                } else {
                    format!("-z{zoom}")
                }
            ));
            // The UI scale keeps the panel's points and multiplies its pixels.
            let size = egui::vec2(wide, tall);
            shoot(&ctx, size, ppp * zoom, 6, Some(&path), |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::inspector::show(&mut app, ui));
            });
            println!("wrote {}", path.display());
        }
    }
}
