//! Arca's and Rota's windows without a screen (the fx side round, wave 3, ADRs 0133 and 0134),
//! and Phasis's lamp vibe: the design sheets.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_arca -- [out-dir]
//! ```
//!
//! Arca: every amp playing (the starting preset's clean combo, the crunch, the chime, the
//! high gain, the bass) and the cabinet alone, in Slate English and Paper Spanish at 100 %;
//! the clean combo at rest; the crunch in Slate English at 75 %, 150 % and 200 %; its other
//! colourways. Rota: the chorale playing, the switch thrown to fast (the horn there, the drum
//! on its way), the brake, at rest; fast at 75 %, 150 % and 200 %; its colourways. Phasis in
//! its lamp vibe and classic. Both units as pedals on Tabula's board. The inspector's lines.
//! Every shot runs the kit's text probe and prints any word cut. `ARCA_ONLY=a,b` shoots only
//! the names holding one of those words.

use cantus_core::fx::tabula::{Pedal, PedalId};
use cantus_core::{Command, EffectKind, EffectSpec};
use cantus_engine::EngineHandle;
use cantus_ui::kit::skin;
use cantus_ui::kit::text::probe;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::views::fx::tabula;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

/// A shot: the unit, its state, playing, Slate (else Paper), the language, the scale.
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
        "arca" => EffectKind::Amp,
        "rota" => EffectKind::Rotary,
        "phasis" => EffectKind::Phaser,
        _ => EffectKind::Board,
    }
}

/// Each state's settings over the unit's demo (its starting preset).
fn settings(unit: &str, state: &str) -> &'static [(&'static str, f32)] {
    match (unit, state) {
        ("arca", "crunch") => &[
            ("model", 2.0),
            ("gain", 0.7),
            ("treble", 0.62),
            ("cab", 3.0),
            ("mic", 0.0),
            ("micpos", 0.35),
            ("micdist", 2.5),
        ],
        ("arca", "chime") => &[("model", 3.0), ("gain", 0.55), ("cab", 2.0), ("mic", 1.0)],
        ("arca", "highgain") => &[
            ("model", 4.0),
            ("gain", 0.8),
            ("mid", 0.35),
            ("cab", 4.0),
            ("micpos", 0.55),
            ("micdist", 6.0),
        ],
        ("arca", "bass") => &[
            ("model", 5.0),
            ("gain", 0.5),
            ("cab", 5.0),
            ("mic", 2.0),
            ("micdist", 12.0),
        ],
        ("arca", "cabonly") => &[("model", 0.0), ("cab", 3.0), ("mic", 1.0)],
        ("rota", "fast") => &[("speed", 2.0), ("drive", 0.35)],
        ("rota", "stop") => &[("speed", 0.0)],
        ("phasis", "vibe") => &[("style", 1.0), ("intensity", 0.8), ("mix", 0.5)],
        _ => &[],
    }
}

fn app_with(sh: &Shot) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    if sh.unit == "board" {
        app.demo_editor("tabula");
        let (strip, effect, _) = app.fx.open.expect("Tabula is open");
        for (i, (kind, demo)) in [
            (EffectKind::Amp, views::fx::arca::DEMO),
            (EffectKind::Rotary, views::fx::rota::DEMO),
        ]
        .into_iter()
        .enumerate()
        {
            let mut spec = EffectSpec::new(kind);
            for (k, v) in demo {
                spec.set(k, *v);
            }
            app.exec(Command::InsertPedal {
                strip,
                effect,
                index: 1 + i,
                pedal: Box::new(Pedal::new(PedalId(20 + i as u16), spec)),
                maps: Vec::new(),
            });
        }
        app.fx.tabula.selected = Some(PedalId(20));
        tabula::demo_feed_at(&mut app, 0.4);
        app.fx.tabula.anim_time = Some(0.37);
        app.fx.pos = Some(egui::pos2(0.0, 0.0));
        return app;
    }
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

/// Shoot one window, its text probe on; print the path and any word cut.
fn take(dir: &Path, sh: &Shot, extra: &str) {
    cantus_ui::i18n::set_locale(sh.lang);
    let ctx = context(sh.dark);
    let mut app = app_with(sh);
    let kind = kind_of(sh.unit);
    views::fx::frame::shell_of(kind).set_scale(&ctx, sh.k);
    let size = views::fx::frame::designed_size(kind) * sh.k;
    let path = dir.join(name(sh, extra));
    probe::start();
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
        .unwrap_or_else(|| PathBuf::from("target/shots/arca"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let only = std::env::var("ARCA_ONLY").ok();
    let sh = |unit, state, playing, dark, lang, k| Shot {
        unit,
        state,
        playing,
        dark,
        lang,
        k,
    };
    let mut shots = vec![
        sh("arca", "start", false, true, Locale::En, 1.0),
        sh("rota", "start", false, true, Locale::En, 1.0),
        sh("rota", "start", false, false, Locale::Es, 1.0),
    ];
    for state in ["start", "crunch", "chime", "highgain", "bass", "cabonly"] {
        shots.push(sh("arca", state, true, true, Locale::En, 1.0));
        shots.push(sh("arca", state, true, false, Locale::Es, 1.0));
    }
    for state in ["start", "fast", "stop"] {
        shots.push(sh("rota", state, true, true, Locale::En, 1.0));
        shots.push(sh("rota", state, true, false, Locale::Es, 1.0));
    }
    for state in ["vibe", "start"] {
        shots.push(sh("phasis", state, true, true, Locale::En, 1.0));
        shots.push(sh("phasis", state, true, false, Locale::Es, 1.0));
    }
    shots.push(sh("board", "pedals", true, true, Locale::En, 1.0));
    shots.push(sh("board", "pedals", true, false, Locale::Es, 1.0));
    for k in [0.75, 1.5, 2.0] {
        shots.push(sh("arca", "crunch", true, true, Locale::En, k));
        shots.push(sh("rota", "fast", true, true, Locale::En, k));
    }
    // The Spanish at the extremes too (the longest words).
    for k in [0.75, 2.0] {
        shots.push(sh("arca", "highgain", true, false, Locale::Es, k));
        shots.push(sh("rota", "fast", true, false, Locale::Es, k));
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
    for (unit, state) in [("arca", "crunch"), ("rota", "fast")] {
        let u = skin::Unit::Effect(kind_of(unit));
        for w in skin::colourways(u).iter().skip(1) {
            for (dark, lang) in [(true, Locale::En), (false, Locale::Es)] {
                let s = sh(unit, state, true, dark, lang, 1.0);
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
        if !wanted("preview") {
            break;
        }
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(dark);
        for (unit, state) in [("arca", "crunch"), ("rota", "fast")] {
            let mut spec = EffectSpec::new(kind_of(unit));
            let demo = if unit == "arca" {
                views::fx::arca::DEMO
            } else {
                views::fx::rota::DEMO
            };
            for (k, v) in demo.iter().chain(settings(unit, state)) {
                spec.set(k, *v);
            }
            let path = dir.join(format!(
                "{unit}-preview-{}.png",
                if dark { "slate" } else { "paper" }
            ));
            shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
                if unit == "arca" {
                    views::fx::arca::preview(ui, &spec);
                } else {
                    views::fx::rota::preview(ui, &spec);
                }
            });
            println!("{}", path.display());
        }
    }
}
