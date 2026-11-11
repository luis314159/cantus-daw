//! Aula's editor in its device frame, without a screen (round 39, ADR 0112).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_aula -- [out-dir]
//! ```
//!
//! Three states — Cantus's large hall (the demo set-up), the cathedral reversed and cut
//! short, a plate stretched and darkened — in Cantus Night and Paper, English and Spanish,
//! at window scales 75 %, 100 % and 150 %; plus the inspector's preview. With
//! `CANTUS_AULA_IR=<a WAV>`, a fourth: that file loaded (at 100 % and 150 %, in Night).

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::aula;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("aula");
    let (strip, effect, _) = app.fx.open.expect("Aula is open");
    let mut host = views::fx::host::DawHost::new(&mut app, strip, effect, "aula");
    match state {
        "reverse" => host.set_many(
            &[
                ("ir", 9.0),
                ("reverse", 1.0),
                ("length", 0.45),
                ("mix", 0.4),
            ],
            "cmd.fx_preset",
        ),
        "plate" => host.set_many(
            &[
                ("ir", 10.0),
                ("stretch", 1.6),
                ("damp", 0.7),
                ("high", 7_000.0),
                ("gate", -60.0),
                ("predelay", 80.0),
                ("mix", 0.3),
            ],
            "cmd.fx_preset",
        ),
        _ => {}
    }
    drop(host);
    if state == "file" {
        let file = std::env::var("CANTUS_AULA_IR").expect("CANTUS_AULA_IR");
        // What a compile does for the editor's outline: the file read into the store.
        let _ = cantus_engine::fx::aula_store::load(&file, std::path::Path::new(&file));
        aula::set_file(&mut app, strip, effect, Some(file));
        views::fx::host::DawHost::new(&mut app, strip, effect, "aula").set_many(
            &[("predelay", 20.0), ("mix", 0.3), ("low", 80.0)],
            "cmd.fx_preset",
        );
    }
    views::fx::demo_feed(&mut app, cantus_core::EffectKind::Aula);
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/aula"));
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    let kind = cantus_core::EffectKind::Aula;
    let mut states = vec!["hall", "reverse", "plate"];
    if std::env::var_os("CANTUS_AULA_IR").is_some() {
        states.push("file");
    }
    for state in states {
        for dark in [true, false] {
            for lang in [Locale::En, Locale::Es] {
                for k in [0.75f32, 1.0, 1.5] {
                    if state == "file" && (k == 0.75 || !dark || lang == Locale::Es) {
                        continue;
                    }
                    if state != "hall" && state != "file" && (k != 1.0 || lang == Locale::Es) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app = app_with(state);
                    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
                    let size = views::fx::frame::designed_size(kind) * k;
                    let name = format!(
                        "aula-{state}-{}-{}-k{}.png",
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
            }
        }
    }
    for dark in [true, false] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(dark);
        let mut spec = cantus_core::EffectSpec::new(kind);
        spec.set("ir", 8.0);
        spec.set("predelay", 30.0);
        spec.set("mix", 0.3);
        let path = dir.join(format!(
            "aula-preview-{}.png",
            if dark { "night" } else { "paper" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            aula::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
