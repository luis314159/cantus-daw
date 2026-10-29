//! Round 32, the dynamics family's faces (ADR 0074): Comprimo, Terminus, Multiplex,
//! Sibilus, Porta, Culmen, Tonsor, Impetus and Discrimen in their device frame, in Slate
//! and Paper, at the window scales 100 % and 200 % — the dynamics builder's look at its
//! own work.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_dyn_faces -- [out-dir] [unit…] [--k=1,2] [--theme=slate|paper] [--plain] [--night]
//! ```
//!
//! Writes `<unit>-<theme>-<k>.png` (default `target/shots/dyn-faces`). `--plain`: the
//! kit's one look (the user's switch off), for comparison. The engine is the dummy one:
//! each editor is fed what a playing engine would, by its own `demo_feed`.

use cantus_core::EffectKind as K;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

/// The family's units by their names, with the demo's name and their kind.
const UNITS: [(&str, &str, K); 9] = [
    ("comprimo", "comp", K::Compressor),
    ("terminus", "limiter", K::Limiter),
    ("multiplex", "multiband", K::Multiband),
    ("sibilus", "deesser", K::DeEsser),
    ("porta", "gate", K::Gate),
    ("culmen", "culmen", K::Maximizer),
    ("tonsor", "tonsor", K::Clipper),
    ("impetus", "impetus", K::Transient),
    ("discrimen", "discrimen", K::TransientSplit),
];

fn app_with(demo: &str, kind: K) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(demo);
    let k = if kind == K::Compressor {
        Some(kind)
    } else {
        app.fx.open.map(|o| o.2)
    };
    if let Some(k) = k {
        views::fx::demo_feed(&mut app, k);
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = match theme {
        "night" => cantus_ui::theme_family::cantus_night(),
        "paper" => cantus_ui::theme::paper(),
        _ => cantus_ui::theme::slate(),
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(name))
            .map(String::from)
    };
    let ks: Vec<f32> = flag("--k=")
        .map(|z| z.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| vec![1.0, 2.0]);
    let night = args.iter().any(|a| a == "--night");
    let themes: Vec<&str> = match flag("--theme=").as_deref() {
        Some("slate") => vec![if night { "night" } else { "slate" }],
        Some("paper") => vec!["paper"],
        _ => vec![if night { "night" } else { "slate" }, "paper"],
    };
    if args.iter().any(|a| a == "--plain") {
        cantus_ui::kit::skin::set_enabled(false);
    }
    args.retain(|a| !a.starts_with("--"));
    let dir = if args.first().is_some_and(|a| a.contains(['/', '\\'])) {
        PathBuf::from(args.remove(0))
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shots/dyn-faces")
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    for (unit, demo, kind) in UNITS {
        if !args.is_empty() && !args.iter().any(|a| a == unit) {
            continue;
        }
        for theme in &themes {
            for &k in &ks {
                let ctx = context(theme);
                let mut app = app_with(demo, kind);
                views::fx::frame::shell_of(kind).set_scale(&ctx, k);
                let size = views::fx::frame::designed_size(kind) * k;
                let path = dir.join(format!("{unit}-{theme}-{}.png", (k * 100.0) as u32));
                shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
                    let ctx = root.ctx().clone();
                    views::comp_editor::window(&mut app, &ctx);
                    views::fx::window(&mut app, &ctx);
                });
                println!("{}", path.display());
            }
        }
    }
}
