//! Wave B's windows without a screen (the fx side round, ADRs 0142–0144 provisional):
//! Vinculum, Lenis and Cedo — the design sheets.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_fxb -- [out-dir]
//! ```
//!
//! Each unit in every state its module names (`DESIGN_STATES`), playing, in Slate English
//! and Paper Spanish at 100 %; at rest; the start at 75 %, 150 % and 200 % (and the Spanish
//! at 75 and 200 %); its other colourways; the inspector's lines. Every shot runs the kit's
//! clip-aware text probe and prints any word cut. `FXB_ONLY=a,b` shoots only the names
//! holding one of those words.

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::skin;
use cantus_ui::kit::text::probe;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const UNITS: [&str; 3] = ["vinculum", "lenis", "cedo"];

#[derive(Copy, Clone)]
struct Shot {
    unit: &'static str,
    state: &'static str,
    playing: bool,
    dark: bool,
    lang: cantus_ui::i18n::Locale,
    k: f32,
}

fn kind_of(unit: &str) -> EffectKind {
    match unit {
        "vinculum" => EffectKind::BusCompressor,
        "lenis" => EffectKind::ResonanceSuppressor,
        _ => EffectKind::Ducker,
    }
}

type States = &'static [(&'static str, &'static [(&'static str, f32)])];

fn states(unit: &str) -> States {
    match unit {
        "vinculum" => views::fx::vinculum::DESIGN_STATES,
        "lenis" => views::fx::lenis::DESIGN_STATES,
        _ => views::fx::cedo::DESIGN_STATES,
    }
}

fn settings(unit: &str, state: &str) -> &'static [(&'static str, f32)] {
    states(unit)
        .iter()
        .find(|(n, _)| *n == state)
        .map_or(&[], |(_, kv)| *kv)
}

fn app_with(sh: &Shot) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(sh.unit);
    let (strip, effect, _) = app.fx.open.expect("the unit is open");
    let kv = settings(sh.unit, sh.state);
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, sh.unit)
            .set_many(kv, "cmd.fx_preset");
    }
    if sh.playing {
        views::fx::demo_feed(&mut app, kind_of(sh.unit));
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

fn name(sh: &Shot, extra: &str) -> String {
    format!(
        "{}-{}{extra}-{}-{}-{}-k{}.png",
        sh.unit,
        sh.state,
        if sh.playing { "playing" } else { "rest" },
        if sh.dark { "slate" } else { "paper" },
        sh.lang.code(),
        (sh.k * 100.0).round() as u32
    )
}

/// Shoot one window, its clip-aware text probe on; print the path and any word cut.
fn take(dir: &Path, sh: &Shot, extra: &str) {
    cantus_ui::i18n::set_locale(sh.lang);
    let ctx = context(sh.dark);
    let mut app = app_with(sh);
    let kind = kind_of(sh.unit);
    views::fx::frame::shell_of(kind).set_scale(&ctx, sh.k);
    let size = views::fx::frame::designed_size(kind) * sh.k;
    let path = dir.join(name(sh, extra));
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
        .unwrap_or_else(|| PathBuf::from("target/shots/fxb"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let only = std::env::var("FXB_ONLY").ok();
    let sh = |unit, state, playing, dark, lang, k| Shot {
        unit,
        state,
        playing,
        dark,
        lang,
        k,
    };
    let mut shots = Vec::new();
    for unit in UNITS {
        shots.push(sh(unit, "start", false, true, Locale::En, 1.0));
        shots.push(sh(unit, "start", false, false, Locale::Es, 1.0));
        for (state, _) in states(unit) {
            shots.push(sh(unit, state, true, true, Locale::En, 1.0));
            shots.push(sh(unit, state, true, false, Locale::Es, 1.0));
        }
        for k in [0.75, 1.5, 2.0] {
            shots.push(sh(unit, "start", true, true, Locale::En, k));
        }
        for k in [0.75, 2.0] {
            shots.push(sh(unit, "start", true, false, Locale::Es, k));
        }
    }
    let wanted = |n: &str| {
        only.as_ref()
            .is_none_or(|o| o.split(',').any(|p| n.contains(p)))
    };
    for s in &shots {
        if wanted(&name(s, "")) {
            take(&dir, s, "");
        }
    }
    // The colourways: the second and third of each unit, playing, both themes.
    for unit in UNITS {
        let u = skin::Unit::Effect(kind_of(unit));
        for w in skin::colourways(u).iter().skip(1) {
            for (dark, lang) in [(true, Locale::En), (false, Locale::Es)] {
                let s = sh(unit, "start", true, dark, lang, 1.0);
                let extra = format!("-{}", w.id);
                if wanted(&name(&s, &extra)) {
                    skin::set_colourway(skin::colourway_unit(u).expect("a unit"), w.id);
                    take(&dir, &s, &extra);
                    skin::set_colourways(std::iter::empty());
                }
            }
        }
    }
    // The inspector's lines.
    for (dark, lang) in [(true, Locale::En), (false, Locale::Es)] {
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(dark);
        for unit in UNITS {
            let path = dir.join(format!(
                "{unit}-preview-{}.png",
                if dark { "slate" } else { "paper" }
            ));
            if !wanted(&path.display().to_string()) {
                continue;
            }
            let spec = cantus_core::starting::starting_spec(kind_of(unit));
            shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
                match unit {
                    "vinculum" => views::fx::vinculum::preview(ui, &spec),
                    "lenis" => views::fx::lenis::preview(ui, &spec),
                    _ => views::fx::cedo::preview(ui, &spec),
                };
            });
            println!("{}", path.display());
        }
    }
}
