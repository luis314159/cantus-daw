//! Vetus's editor in its device frame, without a screen (fx side round, ADR 0133
//! provisional): the design sheet's pictures.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_vetus -- [out-dir] [scales…] [--colourways]
//! ```
//!
//! Two states — at rest (nothing playing) and playing (its telemetry, the analyser's pairs
//! and the groove fed as a playing engine would) — in Slate and Paper, English and Spanish,
//! at the scales given (100 % unless named; the other scales Slate English only); a third,
//! the telephone, playing; the inspector's preview; with `--colourways`, each colourway
//! playing in Slate English and Paper Spanish.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::LoFi;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("vetus");
    let (strip, effect, _) = app.fx.open.expect("Vetus is open");
    if state == "telephone" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "vetus").set_many(
            &[
                ("tone.medium", 2.0),
                ("tone.lo", 300.0),
                ("tone.hi", 3_400.0),
                ("tone.res", 0.45),
                ("bits.mode", 2.0),
                ("rate.hz", 8_000.0),
                ("rate.filter", 1.0),
                ("char", 0.6),
                ("wow", 8.0),
                // A module off: its card dimmed, its controls still answering.
                ("age.on", 0.0),
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
        "vetus-{tag}{state}-{}-{}-k{}.png",
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
        .unwrap_or_else(|| PathBuf::from("target/shots/vetus"));
    let scales: Vec<f32> = args
        .iter()
        .skip(1)
        .filter_map(|a| a.parse::<f32>().ok())
        .collect();
    let scales = if scales.is_empty() { vec![1.0] } else { scales };
    std::fs::create_dir_all(&dir).expect("output dir");
    // The object: the cassette unless `--record`.
    let record = args.iter().any(|a| a == "--record");
    if record {
        views::fx::unit_view::set_view("vetus", "record");
    }
    let obj = if record { "record-" } else { "cassette-" };
    if args.iter().any(|a| a == "--colourways") {
        use cantus_ui::kit::skin;
        let unit = skin::Unit::Effect(KIND);
        for w in skin::colourways(unit) {
            skin::set_colourway("vetus", w.id);
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
    one(&dir, "telephone", false, Locale::En, 1.0, obj);
    one(&dir, "telephone", true, Locale::Es, 1.0, obj);
    for paper in [false, true] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(paper);
        let mut spec = cantus_core::EffectSpec::new(KIND);
        for (k, v) in views::fx::vetus::DEMO {
            spec.set(k, *v);
        }
        let path = dir.join(format!(
            "vetus-preview-{}.png",
            if paper { "paper" } else { "slate" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::vetus::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
