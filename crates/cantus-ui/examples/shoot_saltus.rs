//! Saltus's window without a screen (the fx side round, wave 3, ADR 0132): the
//! design sheet.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_saltus -- [out-dir]
//! ```
//!
//! Every mode playing — the twelve-string (the starting preset, poly), a mono shift with two
//! voices, the harmony in a key and on a MIDI chord, the analog octave, the whammy rocked by
//! hand and played by MIDI notes — in Slate English and Paper Spanish at 100 %; the starting
//! preset at rest; the shift in Slate English at 75 %, 150 % and 200 %; the inspector's line;
//! and Saltus as a pedal on Tabula's board. `SALTUS_ONLY=a,b` shoots only the names holding
//! one of those words.

use cantus_core::fx::tabula::{Pedal, PedalId};
use cantus_core::{Command, EffectKind, EffectSpec};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::views::fx::tabula;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

const KIND: EffectKind = EffectKind::PitchShift;

/// Each state's settings over the starting preset's.
fn settings(state: &str) -> &'static [(&'static str, f32)] {
    match state {
        "shift" => &[
            ("engine", 0.0),
            ("st1", 7.0),
            ("lvl1", 0.8),
            ("pan1", -0.3),
            ("st2", -12.0),
            ("lvl2", 0.6),
            ("pan2", 0.3),
            ("dry", 0.8),
        ],
        "fifth" => &[
            ("engine", 0.0),
            ("st1", 7.0),
            ("lvl1", 0.5),
            ("dry", 1.0),
            ("out", -1.0),
        ],
        "harmony" => &[
            ("mode", 1.0),
            ("engine", 0.0),
            ("lvl1", 0.7),
            ("lvl2", 0.5),
            ("pan2", -0.4),
        ],
        "chord" => &[
            ("mode", 1.0),
            ("hsrc", 1.0),
            ("engine", 0.0),
            ("lvl1", 0.7),
            ("lvl2", 0.5),
        ],
        "octave" => &[("mode", 2.0), ("sub1", 0.8), ("sub2", 0.5), ("up1", 0.3)],
        "whammy" => &[
            ("mode", 3.0),
            ("engine", 0.0),
            ("lvl1", 1.0),
            ("dry", 0.0),
            ("pedal", 0.58),
        ],
        "notes" => &[
            ("mode", 3.0),
            ("engine", 0.0),
            ("lvl1", 1.0),
            ("dry", 0.3),
            ("wsrc", 1.0),
            ("toe", -12.0),
            ("glide", 120.0),
        ],
        _ => &[],
    }
}

fn app_with(state: &str, playing: bool) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    if state == "board" {
        app.demo_editor("tabula");
        let (strip, effect, _) = app.fx.open.expect("Tabula is open");
        let mut spec = EffectSpec::new(KIND);
        for (k, v) in views::fx::saltus::DEMO {
            spec.set(k, *v);
        }
        app.exec(Command::InsertPedal {
            strip,
            effect,
            index: 1,
            pedal: Box::new(Pedal::new(PedalId(9), spec)),
            maps: Vec::new(),
        });
        app.fx.tabula.selected = Some(PedalId(9));
        tabula::demo_feed_at(&mut app, 0.4);
        app.fx.tabula.anim_time = Some(0.37);
        app.fx.pos = Some(egui::pos2(0.0, 0.0));
        return app;
    }
    app.demo_editor("saltus");
    let (strip, effect, _) = app.fx.open.expect("Saltus is open");
    let kv = settings(state);
    if !kv.is_empty() {
        views::fx::host::DawHost::new(&mut app, strip, effect, "saltus")
            .set_many(kv, "cmd.fx_preset");
    }
    if playing {
        views::fx::demo_feed(&mut app, KIND);
    }
    if state == "menu" {
        // The preset list open, the twelve-string loaded, the person's start the mono fifth
        // (set for this picture only: the settings file is not written).
        let mut fifth = cantus_core::EffectSpec::new(KIND);
        for (k, v) in settings("fifth") {
            fifth.set(k, *v);
        }
        app.settings
            .starting
            .push(cantus_ui::starting_choice::StartingChoice {
                label: cantus_ui::i18n::t("preset.saltus.fifth_live"),
                spec: fifth,
            });
        app.fx.saltus.chrome.reset_for(effect.0);
        app.fx.saltus.chrome.open_presets_at(
            egui::pos2(510.0, 27.0),
            egui::Rect::from_min_max(egui::pos2(510.0, 4.0), egui::pos2(750.0, 26.0)),
        );
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

fn main() {
    use cantus_ui::i18n::Locale;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/saltus"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let only = std::env::var("SALTUS_ONLY").ok();
    let mut shots: Vec<(&str, bool, bool, Locale, f32)> =
        vec![("start", false, true, Locale::En, 1.0)];
    for state in [
        "start", "fifth", "shift", "harmony", "chord", "octave", "whammy", "notes", "board", "menu",
    ] {
        shots.push((state, true, true, Locale::En, 1.0));
        shots.push((state, true, false, Locale::Es, 1.0));
    }
    for k in [0.75, 1.5, 2.0] {
        shots.push(("shift", true, true, Locale::En, k));
    }
    for (state, playing, dark, lang, k) in shots {
        let name = format!(
            "saltus-{state}-{}-{}-{}-k{}.png",
            if playing { "playing" } else { "rest" },
            if dark { "slate" } else { "paper" },
            lang.code(),
            (k * 100.0).round() as u32
        );
        if only
            .as_ref()
            .is_some_and(|o| !o.split(',').any(|p| name.contains(p)))
        {
            continue;
        }
        cantus_ui::i18n::set_locale(lang);
        let ctx = context(dark);
        let mut app = app_with(state, playing);
        let kind = if state == "board" {
            EffectKind::Board
        } else {
            KIND
        };
        views::fx::frame::shell_of(kind).set_scale(&ctx, k);
        let size = views::fx::frame::designed_size(kind) * k;
        let path = dir.join(name);
        shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
            let ctx = root.ctx().clone();
            views::fx::window(&mut app, &ctx);
        });
        println!("{}", path.display());
    }
    for dark in [true, false] {
        cantus_ui::i18n::set_locale(if dark { Locale::En } else { Locale::Es });
        let ctx = context(dark);
        let path = dir.join(format!(
            "saltus-preview-{}.png",
            if dark { "slate" } else { "paper" }
        ));
        let mut spec = EffectSpec::new(KIND);
        for (k, v) in views::fx::saltus::DEMO {
            spec.set(k, *v);
        }
        if only.as_ref().is_some_and(|o| !o.contains("preview")) {
            continue;
        }
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::saltus::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
