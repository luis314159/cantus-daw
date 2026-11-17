//! Lapsus's editor in its device frame, without a screen (fx side round, ADR 0129):
//! the design sheet's pictures.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_lapsus -- [out-dir] [scales…]
//! ```
//!
//! Three states — *Tape stop ¼* with *Fade the last beat* at rest (nothing playing), the
//! same playing (a drum loop's telemetry and analyser pairs fed as a playing engine would),
//! and *Reverse ½* under *Tremolo ⅛* playing with the volume bank on the pads (the slip at
//! its longest) — in Slate and Paper, English and Spanish, at the scales given
//! (75, 100, 150 and 200 % unless named; every state in Slate English at each, the rest at
//! 100 %); plus the inspector's preview.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::TimeShaper;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("lapsus");
    let (strip, effect, _) = app.fx.open.expect("Lapsus is open");
    if state == "reverse" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "lapsus")
            .set_many(&[("tslot", 18.0), ("vslot", 24.0)], "cmd.fx_preset");
    }
    if state != "rest" {
        views::fx::demo_feed(&mut app, KIND);
    }
    if state == "reverse" {
        // The volume bank on the pads: the switch's other side.
        app.fx.lapsus.bank = cantus_core::fx::lapsus::Lane::Vol;
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn context(paper: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if paper {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme::slate()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/lapsus"));
    let scales: Vec<f32> = args
        .iter()
        .skip(1)
        .filter_map(|a| a.parse::<f32>().ok())
        .collect();
    let scales = if scales.is_empty() {
        vec![0.75, 1.0, 1.5, 2.0]
    } else {
        scales
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    for state in ["rest", "playing", "reverse"] {
        for paper in [false, true] {
            for lang in [Locale::En, Locale::Es] {
                for &k in &scales {
                    let main = !paper && lang == Locale::En;
                    if !main && k != 1.0 {
                        continue;
                    }
                    if state == "reverse" && paper {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(paper);
                    let mut app = app_with(state);
                    views::fx::frame::shell_of(KIND).set_scale(&ctx, k);
                    let size = views::fx::frame::designed_size(KIND) * k;
                    let name = format!(
                        "lapsus-{state}-{}-{}-k{}.png",
                        if paper { "paper" } else { "slate" },
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
    for paper in [false, true] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(paper);
        let mut spec = cantus_core::EffectSpec::new(KIND);
        for (k, v) in views::fx::lapsus::DEMO {
            spec.set(k, *v);
        }
        let path = dir.join(format!(
            "lapsus-preview-{}.png",
            if paper { "paper" } else { "slate" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::lapsus::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
