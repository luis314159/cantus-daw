//! Arcus's window without a screen (round 42, ADR 0157): its design sheet — Play with the
//! cello bowed, the double bass bowed and the cello plucked, and Edit (the Schelleng
//! diagram), in Slate and Paper at 100 % and 200 %. `CANTUS_SHOOT_LANG=es` for Spanish.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_arcus -- <dir>
//! ```

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::shell::Shell;
use cantus_ui::raster::shoot;
use cantus_ui::views::inst::View;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/shots/arcus".into()));
    std::fs::create_dir_all(&dir).expect("out dir");
    let es = std::env::var("CANTUS_SHOOT_LANG").is_ok_and(|l| l == "es");
    cantus_ui::i18n::set_locale(if es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    let lang = if es { "es" } else { "en" };
    // (name, view, instrument, technique, the telemetry's string and key)
    let states: [(&str, View, f32, f32, f32, f32); 4] = [
        ("cello-play", View::Play, 0.0, 0.0, 2.0, 57.0),
        ("bass-play", View::Play, 1.0, 0.0, 2.0, 45.0),
        ("cello-pizz", View::Play, 0.0, 1.0, 1.0, 50.0),
        ("cello-edit", View::Edit, 0.0, 0.0, 2.0, 57.0),
    ];
    for (name, view, inst, tech, string, key) in states {
        for dark in [true, false] {
            for k in [1.0f32, 2.0] {
                let ctx = context(dark);
                let mut app =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                app.demo_editor("arcus");
                let Some(t) = app.inst.open else {
                    panic!("no Arcus editor")
                };
                app.set_inst_value(t, "inst", inst, false);
                app.set_inst_value(t, "tech", tech, false);
                views::inst::arcus::demo_feed(&mut app);
                if let Some(tele) = app.inst.injected.as_mut() {
                    tele[cantus_dsp::arcus::tel::STRING] = string;
                    tele[cantus_dsp::arcus::tel::KEY] = key;
                    if inst > 0.5 {
                        tele[cantus_dsp::arcus::tel::FORCE] = 2.6;
                        tele[cantus_dsp::arcus::tel::FMIN] = 0.9;
                        tele[cantus_dsp::arcus::tel::FMAX] = 3.6;
                    }
                }
                app.inst.view = view;
                Shell::new(("inst", t.0)).set_scale(&ctx, k);
                let size = views::inst::editor_size(InstrumentKind::BowedString) * k;
                let theme = if dark { "slate" } else { "paper" };
                let path = dir.join(format!(
                    "arcus-{name}-{theme}-{lang}-{}.png",
                    (k * 100.0) as u32
                ));
                shoot(&ctx, size, 1.0, 8, Some(&path), |ui| {
                    views::inst::window_in(&mut app, ui)
                });
                println!("{}", path.display());
            }
        }
    }
}
