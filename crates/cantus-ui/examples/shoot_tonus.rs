//! Tonus's editor in its device frame, without a screen (round 28, ADR 0053).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_tonus -- [out-dir]
//! ```
//!
//! Three states — the front row (A minor, E bypassed, G removed), the folded settings
//! open, and the MIDI source — in Cantus Night and Paper, English and Spanish, at window
//! scales 75 %, 100 % and 200 %; plus the inspector's preview. Each editor is fed what a
//! playing engine would: the demo line through the real DSP (`tonus::demo_feed`).

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::views::fx::tonus;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("tonus");
    let (strip, effect, _) = app.fx.open.expect("Tonus is open");
    match state {
        "more" => app.fx.tonus.more = true,
        "midi" => views::fx::host::DawHost::new(&mut app, strip, effect, "tonus")
            .set_many(&[("src", 1.0), ("speed", 0.0)], "cmd.fx_preset"),
        _ => {}
    }
    views::fx::demo_feed(&mut app, cantus_core::EffectKind::AutoTune);
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
        .unwrap_or_else(|| PathBuf::from("target/shots/tonus"));
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    let kind = cantus_core::EffectKind::AutoTune;
    for state in ["front", "more", "midi"] {
        for dark in [true, false] {
            for lang in [Locale::En, Locale::Es] {
                for k in [0.75f32, 1.0, 2.0] {
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(dark);
                    let mut app = app_with(state);
                    views::fx::frame::shell_of(kind).set_scale(&ctx, k);
                    let size = views::fx::frame::designed_size(kind) * k;
                    let name = format!(
                        "tonus-{state}-{}-{}-k{}.png",
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
    // The inspector's preview.
    for dark in [true, false] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(dark);
        let mut spec = cantus_core::EffectSpec::new(kind);
        spec.set("key", 9.0);
        spec.set("scale", 2.0);
        spec.set("n4", 3.0);
        let path = dir.join(format!(
            "tonus-preview-{}.png",
            if dark { "night" } else { "paper" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            tonus::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
