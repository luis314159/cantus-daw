//! The modulation family's editors (ADR 0055) in their device frame, without a screen.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_modfx -- [out-dir] [stem…] [--quick] [--preview]
//! ```
//!
//! Every editor (`concentus flexus phasis tremor vibrans anulus translatio fluctus`) in
//! Cantus Night and Paper, English and Spanish, at the window scales 75 %, 100 % and
//! 200 % — the scales where things collide or blur. `--quick`: Night and Paper in English
//! at 100 %. `--preview`: the inspector's preview strips too. The engine is the dummy one:
//! each editor is fed what a playing engine would, by its own `demo_feed`.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

const ALL: [&str; 8] = [
    "concentus",
    "flexus",
    "phasis",
    "tremor",
    "vibrans",
    "anulus",
    "translatio",
    "fluctus",
];

fn app_with(stem: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(stem);
    if let Some(k) = app.fx.open.map(|o| o.2) {
        views::fx::demo_feed(&mut app, k);
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn kind_of(stem: &str) -> cantus_core::EffectKind {
    use cantus_core::EffectKind as K;
    match stem {
        "concentus" => K::Chorus,
        "flexus" => K::Flanger,
        "phasis" => K::Phaser,
        "tremor" => K::Tremolo,
        "vibrans" => K::Vibrato,
        "anulus" => K::RingMod,
        "translatio" => K::FreqShift,
        _ => K::VolumeShaper,
    }
}

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme_family::cantus_night()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let quick = args.iter().any(|a| a == "--quick");
    let previews = args.iter().any(|a| a == "--preview");
    args.retain(|a| !a.starts_with("--"));
    let dir = if args.first().is_some_and(|a| a.contains(['/', '\\'])) {
        PathBuf::from(args.remove(0))
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("shots")
            .join("modfx")
    };
    let stems: Vec<String> = if args.is_empty() {
        ALL.iter().map(|s| s.to_string()).collect()
    } else {
        args
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    let mut shots: Vec<(bool, Locale, f32)> = Vec::new();
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            if quick && lang == Locale::Es {
                continue;
            }
            for k in [0.75f32, 1.0, 2.0] {
                if quick && k != 1.0 {
                    continue;
                }
                shots.push((dark, lang, k));
            }
        }
    }
    for stem in &stems {
        for &(dark, lang, k) in &shots {
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            let mut app = app_with(stem);
            let fx = kind_of(stem);
            views::fx::frame::shell_of(fx).set_scale(&ctx, k);
            let size = views::fx::frame::designed_size(fx) * k;
            let name = format!(
                "{stem}-{}-{}-k{}.png",
                if dark { "night" } else { "paper" },
                lang.code(),
                (k * 100.0) as u32
            );
            let path = dir.join(name);
            shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
                let ctx = root.ctx().clone();
                views::fx::window(&mut app, &ctx);
            });
            println!("{}", path.display());
        }
        if previews {
            for dark in [true, false] {
                cantus_ui::i18n::set_locale(Locale::En);
                let ctx = context(dark);
                let app = app_with(stem);
                let Some((strip, effect, _)) = app.fx.open else {
                    continue;
                };
                let spec = app
                    .project()
                    .strip(strip)
                    .and_then(|s| s.effect(effect))
                    .map(|e| e.spec.clone())
                    .expect("the demo's effect");
                let path = dir.join(format!(
                    "{stem}-preview-{}.png",
                    if dark { "night" } else { "paper" }
                ));
                shoot(&ctx, egui::vec2(320.0, 80.0), 2.0, 4, Some(&path), |ui| {
                    views::fx::preview(ui, &spec);
                });
                println!("{}", path.display());
            }
        }
    }
}
