//! Nitor's window without a screen (round 41, ADR 0151): its design sheet.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_nitor -- [out-dir]
//! ```
//!
//! Each design state playing, in Slate English and Paper Spanish at 100 %; at rest; the
//! starting state at 75 %, 150 % and 200 % (and the Spanish at 75 and 200 %); the inspector's
//! line. Every shot runs the kit's text probe and prints any word cut. `NITOR_ONLY=a,b` shoots
//! only the names holding one of those words. // r41:nitor

use cantus_engine::EngineHandle;
use cantus_ui::kit::text::probe;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::Enhancer;

#[derive(Copy, Clone)]
struct Shot {
    state: &'static str,
    playing: bool,
    dark: bool,
    lang: cantus_ui::i18n::Locale,
    k: f32,
}

fn settings(state: &str) -> &'static [(&'static str, f32)] {
    views::fx::nitor::DESIGN_STATES
        .iter()
        .find(|(n, _)| *n == state)
        .map(|(_, kv)| *kv)
        .unwrap_or(&[])
}

fn app_with(sh: &Shot) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("nitor");
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    let kv = settings(sh.state);
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, "nitor").set_many(kv, "cmd.fx_preset");
    }
    if sh.playing {
        views::fx::demo_feed(&mut app, KIND);
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn name(sh: &Shot) -> String {
    format!(
        "nitor-{}-{}-{}-{}-k{}.png",
        sh.state,
        if sh.playing { "playing" } else { "rest" },
        if sh.dark { "slate" } else { "paper" },
        sh.lang.code(),
        (sh.k * 100.0).round() as u32
    )
}

fn take(dir: &Path, sh: &Shot) {
    cantus_ui::i18n::set_locale(sh.lang);
    let ctx = context(sh.dark);
    let mut app = app_with(sh);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, sh.k);
    let size = views::fx::frame::designed_size(KIND) * sh.k;
    let path = dir.join(name(sh));
    probe::start_clipped();
    shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
        let ctx = root.ctx().clone();
        views::fx::window(&mut app, &ctx);
    });
    let cuts = probe::take();
    println!("{}", path.display());
    for c in cuts {
        println!("  CUT {:?} -> {:?} ({})", c.full, c.shown, c.how);
    }
}

fn main() {
    use cantus_ui::i18n::Locale;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/nitor"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let only = std::env::var("NITOR_ONLY").ok();
    let sh = |state, playing, dark, lang, k| Shot {
        state,
        playing,
        dark,
        lang,
        k,
    };
    let mut shots = vec![
        sh("start", false, true, Locale::En, 1.0),
        sh("start", false, false, Locale::Es, 1.0),
    ];
    for (state, _) in views::fx::nitor::DESIGN_STATES {
        shots.push(sh(state, true, true, Locale::En, 1.0));
        shots.push(sh(state, true, false, Locale::Es, 1.0));
    }
    for k in [0.75, 1.5, 2.0] {
        shots.push(sh("start", true, true, Locale::En, k));
    }
    for k in [0.75, 2.0] {
        shots.push(sh("start", true, false, Locale::Es, k));
    }
    let wanted = |n: &str| {
        only.as_ref()
            .is_none_or(|o| o.split(',').any(|p| n.contains(p)))
    };
    for s in &shots {
        if wanted(&name(s)) {
            take(&dir, s);
        }
    }
    for (dark, lang) in [(true, Locale::En), (false, Locale::Es)] {
        if !wanted("preview") {
            break;
        }
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(dark);
        let spec = cantus_core::starting::starting_spec(KIND);
        let path = dir.join(format!(
            "nitor-preview-{}.png",
            if dark { "slate" } else { "paper" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::nitor::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
