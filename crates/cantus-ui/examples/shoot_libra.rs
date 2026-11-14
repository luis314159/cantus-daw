//! Libra's editor in its device frame, without a screen (fx side round, ADR 0133
//! provisional): the design sheet's pictures.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_libra -- [out-dir] [scales…] [--colourways]
//! ```
//!
//! Two states — at rest (nothing playing) and playing (its telemetry, the analyser's pairs
//! and the groove fed as a playing engine would) — in Slate and Paper, English and Spanish,
//! at the scales given (100 % unless named; the other scales Slate English only); a third,
//! mono with the right inverted, playing; the inspector's preview; with `--colourways`, each colourway
//! playing in Slate English and Paper Spanish.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::Utility;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("libra");
    let (strip, effect, _) = app.fx.open.expect("Libra is open");
    if state == "mono" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "libra").set_many(
            &[
                ("chan", 2.0),
                ("pol.r", 1.0),
                ("delay.unit", 1.0),
                ("width", 1.0),
                ("pan", 0.35),
            ],
            "cmd.fx_preset",
        );
    }
    if state != "rest" {
        views::fx::demo_feed(&mut app, KIND);
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

fn one(dir: &Path, state: &str, paper: bool, lang: cantus_ui::i18n::Locale, k: f32, tag: &str) {
    cantus_ui::i18n::set_locale(lang);
    let ctx = context(paper);
    let mut app = app_with(state);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(KIND) * k;
    let name = format!(
        "libra-{tag}{state}-{}-{}-k{}.png",
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

fn main() {
    use cantus_ui::i18n::Locale;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/libra"));
    let scales: Vec<f32> = args
        .iter()
        .skip(1)
        .filter_map(|a| a.parse::<f32>().ok())
        .collect();
    let scales = if scales.is_empty() { vec![1.0] } else { scales };
    std::fs::create_dir_all(&dir).expect("output dir");
    let obj = "";
    if args.iter().any(|a| a == "--colourways") {
        use cantus_ui::kit::skin;
        let unit = skin::Unit::Effect(KIND);
        for w in skin::colourways(unit) {
            skin::set_colourway("libra", w.id);
            for (paper, lang) in [(false, Locale::En), (true, Locale::Es)] {
                one(
                    &dir,
                    "playing",
                    paper,
                    lang,
                    1.0,
                    &format!("{obj}{}-", w.id),
                );
            }
        }
        skin::set_colourways(std::iter::empty());
        return;
    }
    for state in ["rest", "playing"] {
        for (paper, lang) in [(false, Locale::En), (true, Locale::Es)] {
            one(&dir, state, paper, lang, 1.0, obj);
        }
    }
    for &k in scales.iter().filter(|k| **k != 1.0) {
        one(&dir, "playing", false, Locale::En, k, obj);
    }
    one(&dir, "mono", false, Locale::En, 1.0, obj);
    one(&dir, "mono", true, Locale::Es, 1.0, obj);
    for paper in [false, true] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(paper);
        let mut spec = cantus_core::EffectSpec::new(KIND);
        for (k, v) in views::fx::libra::DEMO {
            spec.set(k, *v);
        }
        let path = dir.join(format!(
            "libra-preview-{}.png",
            if paper { "paper" } else { "slate" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::libra::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
