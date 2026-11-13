//! Cribrum's editor in its device frame, without a screen (fx side round, ADR 0130
//! provisional): the design sheet's pictures.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_cribrum -- [out-dir] [scales…]
//! ```
//!
//! Three states — the demo at rest (nothing playing), the demo playing (its telemetry and
//! the analyser's spectrum fed as a playing engine would), a vowel filter talking with the
//! curve's lane shown — in Slate and Paper, English and Spanish, at the scales given
//! (100 % and 150 % unless named); plus the inspector's preview.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::AutoFilter;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("cribrum");
    let (strip, effect, _) = app.fx.open.expect("Cribrum is open");
    if state == "vowel" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "cribrum").set_many(
            &[
                ("a.mode", 5.0),
                ("a.vowel", 1.0),
                ("a.res", 0.45),
                ("a.cut", 1_000.0),
                ("lfo.cut", 0.0),
                ("lfo.vowel", 1.5),
                ("lfo.div", 5.0),
                ("cur.cut", 0.0),
                ("cur.vowel", 1.0),
                ("b.on", 0.0),
            ],
            "cmd.fx_preset",
        );
    }
    if state != "rest" {
        views::fx::demo_feed(&mut app, KIND);
    }
    if state == "vowel" {
        app.fx.cribrum.source = 2;
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
        .unwrap_or_else(|| PathBuf::from("target/shots/cribrum"));
    let scales: Vec<f32> = args
        .iter()
        .skip(1)
        .filter_map(|a| a.parse::<f32>().ok())
        .collect();
    let scales = if scales.is_empty() {
        vec![1.0, 1.5]
    } else {
        scales
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    if args.iter().any(|a| a == "--colourways") {
        colourways(&dir);
        return;
    }
    use cantus_ui::i18n::Locale;
    for state in ["rest", "playing", "vowel"] {
        for paper in [false, true] {
            for lang in [Locale::En, Locale::Es] {
                for &k in &scales {
                    // Every state in Slate English at every scale; the rest at 100 %.
                    let main = !paper && lang == Locale::En;
                    if !main && k != 1.0 {
                        continue;
                    }
                    if state == "vowel" && (paper || lang == Locale::Es) && k != 1.0 {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let ctx = context(paper);
                    let mut app = app_with(state);
                    views::fx::frame::shell_of(KIND).set_scale(&ctx, k);
                    let size = views::fx::frame::designed_size(KIND) * k;
                    let name = format!(
                        "cribrum-{state}-{}-{}-k{}.png",
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
        for (k, v) in views::fx::cribrum::DEMO {
            spec.set(k, *v);
        }
        let path = dir.join(format!(
            "cribrum-preview-{}.png",
            if paper { "paper" } else { "slate" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::cribrum::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}

/// The colourways as the user chooses them (ADR 0130 §Colourways): the demo playing in each,
/// Slate English and Paper Spanish at 100 %, into `<dir>/colour/final/`.
fn colourways(dir: &std::path::Path) {
    use cantus_ui::i18n::Locale;
    use cantus_ui::kit::skin;
    let dir = dir.join("colour").join("final");
    std::fs::create_dir_all(&dir).expect("output dir");
    let unit = skin::Unit::Effect(KIND);
    for w in skin::colourways(unit) {
        for (paper, lang) in [(false, Locale::En), (true, Locale::Es)] {
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(paper);
            skin::set_colourway(skin::colourway_unit(unit).expect("a unit"), w.id);
            let mut app = app_with("playing");
            views::fx::frame::shell_of(KIND).set_scale(&ctx, 1.0);
            let size = views::fx::frame::designed_size(KIND);
            let path = dir.join(format!(
                "cribrum-{}-{}-{}-k100.png",
                w.id,
                if paper { "paper" } else { "slate" },
                lang.code()
            ));
            shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
                let ctx = root.ctx().clone();
                views::fx::window(&mut app, &ctx);
            });
            println!("{}", path.display());
        }
    }
    skin::set_colourways(std::iter::empty());
}
