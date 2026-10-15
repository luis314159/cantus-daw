//! Modulators (round 28, ADR 0057), screenshotted without a screen:
//!
//! - `section`: the inspector on a track carrying one modulator of every kind — LFO,
//!   envelope, steps, random on the track, a follower on its channel — each driving
//!   parameters, one folded, one muted, one mapping;
//! - `popup`: the *Modulate…* list open beside a parameter;
//! - `knobs`: controls modulated — rings by kind, the live dot, the marks — as knobs and as
//!   cells, Alt-drag and drop targets drawn as they are while a handle is carried.
//!
//! Each at 75, 100 and 200 %, in English and Spanish, in Cantus Night and Paper.
//!
//! `cargo run --release -p cantus-ui --example shoot_modulators -- <out-dir> [filter…]`
//! writes `mods-<what>-<theme>-<lang>-<scale>.png`; a filter keeps the names holding it.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::kit::automate::ModOffer;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in ["section", "popup", "knobs"] {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [0.75f32, 1.0, 2.0] {
                    let name = format!(
                        "mods-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    shoot(&path, what, theme, scale);
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme_family::cantus_night()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

/// The welcome song with modulators of every kind on its first track and channel.
fn project() -> Project {
    let mut p = welcome_project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    let other = p.tracks.get(1).and_then(|t| t.strip).unwrap_or(s);
    let add = |p: &mut Project, host, kind, routes: Vec<(AutoTarget, f32)>| {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
        m.routes = routes
            .into_iter()
            .map(|(target, depth)| ModRoute { target, depth })
            .collect();
        let id = m.id;
        p.modulators_mut(host).unwrap().push(m);
        id
    };
    let lfo = add(
        &mut p,
        ModHost::Track(t),
        ModKind::Lfo,
        vec![(AutoTarget::StripPan(s), 0.35), (AutoTarget::TrackGain(t), -0.2)],
    );
    let env = add(
        &mut p,
        ModHost::Track(t),
        ModKind::Envelope,
        vec![(
            AutoTarget::Firmus {
                track: t,
                param: FirmusParam::Cutoff,
            },
            0.6,
        )],
    );
    add(
        &mut p,
        ModHost::Track(t),
        ModKind::Step,
        vec![(AutoTarget::StripWidth(s), 0.5)],
    );
    let rnd = add(
        &mut p,
        ModHost::Track(t),
        ModKind::Random,
        vec![(AutoTarget::ModAmount(lfo), -0.4)],
    );
    add(
        &mut p,
        ModHost::Strip(s),
        ModKind::Follower,
        vec![(AutoTarget::StripGain(other), -0.45)],
    );
    if let Some((h, i, _)) = p.modulator(rnd) {
        p.modulators_mut(h).unwrap()[i].muted = true;
    }
    if let Some((h, i, _)) = p.modulator(env) {
        if let ModSource::Envelope(e) = &mut p.modulators_mut(h).unwrap()[i].source {
            e.attack_ms = 40.0;
            e.decay_ms = 220.0;
            e.sustain = 0.35;
        }
    }
    p
}

fn app_for(theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-mods");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        ("paper", Mode::Light)
    } else {
        ("cantus_night", Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app
}

fn shoot(path: &std::path::Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(theme);
    app.attach(&ctx);
    let t = app.project().tracks[0].id;
    app.select_track(t);
    let third = app.project().all_modulators().nth(2).map(|(_, m)| m.id);
    app.modulators.mapping = third;
    let size = match what {
        "section" => vec2(360.0, 2000.0),
        "popup" => vec2(720.0, 560.0),
        _ => vec2(620.0, 260.0),
    };
    let mut frame = 0;
    cantus_ui::raster::shoot(&ctx, size, scale, 6, Some(path), |root| {
        frame += 1;
        match what {
            "section" | "popup" => {
                if what == "popup" && frame == 2 {
                    let s = app.project().tracks[0].strip.unwrap();
                    let r = egui::Rect::from_min_size(egui::pos2(300.0, 60.0), vec2(36.0, 36.0));
                    app.modulators.popup = Some((AutoTarget::StripPan(s), r));
                }
                egui::Panel::left(egui::Id::new("insp"))
                    .exact_size(360.0)
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| cantus_ui::views::inspector::show(&mut app, ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                    .show(root, |_| {});
                cantus_ui::views::modulator::popup::frame(&mut app, &root.ctx().clone());
            }
            _ => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                    .show(root, |ui| knobs(ui));
            }
        }
    });
}

/// Controls as a modulated parameter draws them.
fn knobs(ui: &mut egui::Ui) {
    use cantus_ui::kit::knob::KnobSize;
    let s = cantus_ui::kit::Style::current();
    let hue = |f| s.t.hue(f);
    use cantus_ui::kit::Function as F;
    let sets: [(f32, &[(f32, egui::Color32)], ModOffer); 4] = [
        (
            0.5,
            &[(0.3, hue(F::Mod))],
            ModOffer {
                depth: Some(0.3),
                mapping: false,
                live: Some(0.68),
            },
        ),
        (
            0.35,
            &[(-0.25, hue(F::Dynamics)), (0.4, hue(F::Source))],
            ModOffer {
                depth: Some(-0.25),
                mapping: false,
                live: Some(0.52),
            },
        ),
        (
            0.7,
            &[(0.2, hue(F::Space))],
            ModOffer {
                depth: Some(0.2),
                mapping: true,
                live: Some(0.82),
            },
        ),
        (0.2, &[], ModOffer::default()),
    ];
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        for (i, (v, rings, m)) in sets.iter().enumerate() {
            let mut x = *v;
            let _ = cantus_ui::kit::widgets::Param::new(&mut x, 0.0..=1.0, 0.5)
                .size(if i % 2 == 0 { KnobSize::L } else { KnobSize::M })
                .rings(rings)
                .automation(i == 1)
                .modulation(*m)
                .show(ui);
            ui.add_space(18.0);
        }
    });
    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let names = ["Drive", "Cutoff", "Mix", "Width"];
        for (i, (v, rings, m)) in sets.iter().enumerate() {
            let fmt = |x: f32, b: &mut String| b.push_str(&format!("{:.0} %", x * 100.0));
            let _ = cantus_ui::views::fx::controls::Cell::new(*v, 0.0..=1.0, 0.5)
                .label(cantus_ui::kit::text::Src::Str(names[i]))
                .name(names[i])
                .format(&fmt)
                .rings(rings)
                .automation(i == 2)
                .modulation(*m)
                .show(ui, &s);
            ui.add_space(12.0);
        }
    });
}
