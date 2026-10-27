//! Note effects, keys and grooves (round 31, ADR 0069), screenshotted without a screen:
//!
//! - `chain`: the inspector on a track whose note effects are chords from the key into an
//!   up-down arpeggiator, then a scale with its own key (switched off);
//! - `choice`: the arpeggiator's rates open;
//! - `add`: the + of the Note effects band open;
//! - `clip`: the inspector on a clip played with a groove from the pool;
//! - `groove`: the same, its groove list open;
//! - `roll`: the piano roll in D minor, the key's rows lit and its root marked;
//! - `fold`: the same rows folded to the key;
//! - `key`: the key's list open over the piano roll's toolbar;
//! - `rack`: the channel rack, its pattern swung (the groove field on its toolbar).
//!
//! Each at 100 and 200 %, in English and Spanish, in Cantus Night and Paper.
//!
//! `cargo run --release -p cantus-ui --example shoot_notefx -- <out-dir> [filter…]` writes
//! `notefx-<what>-<theme>-<lang>-<scale>.png`; a filter keeps the names holding it.

use cantus_core::groove::{ClipGroove, Groove, GrooveId, GrooveSource};
use cantus_core::notefx::*;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::PathBuf;

const KINDS: [&str; 9] = [
    "chain", "choice", "add", "clip", "groove", "roll", "fold", "key", "rack",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in KINDS {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [1.0f32, 2.0] {
                    let name = format!(
                        "notefx-{what}-{theme}-{}-{}",
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

/// The welcome song in D minor; its first track with three note effects, its first clip
/// swung by a groove taken from a played part.
fn project() -> Project {
    let mut p = welcome_project();
    p.key = Some(Key::new(2, ScaleKind::Minor));
    let mut chord = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Chord);
    if let NoteFxKind::Chord(c) = &mut chord.kind {
        c.shape = ChordShape::ScaleSeventh;
        c.spread = 1;
    }
    let mut arp = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Arp);
    if let NoteFxKind::Arp(a) = &mut arp.kind {
        a.mode = ArpMode::UpDown;
        a.octaves = 2;
        a.rate = 14;
        a.swing = 0.58;
    }
    let mut scale = NoteEffect::new(NoteFxId(p.alloc_id()), NoteFxType::Scale);
    if let NoteFxKind::Scale(s) = &mut scale.kind {
        s.mode = ScaleMode::Drop;
        s.key = KeyChoice {
            own: true,
            root: 9,
            scale: ScaleKind::MinorPentatonic,
        };
    }
    scale.enabled = false;
    let gid = GrooveId(p.alloc_id());
    let hits: Vec<_> = (0..32)
        .map(|k| {
            let late = if k % 2 == 1 { 52 } else { 0 };
            (
                Ticks(k * 240 + late),
                if k % 4 == 0 { 118.0 } else { 84.0 },
                Ticks(200),
            )
        })
        .collect();
    let mut g = Groove::extract(gid, String::new(), &hits, Ticks(1920), Ticks(240)).unwrap();
    g.name = "MPC feel".into();
    p.grooves.push(g);
    if let Some(t) = p.tracks.first_mut() {
        t.note_fx = vec![chord, arp, scale];
        if let Some(c) = t.clips.first_mut() {
            c.groove = Some(ClipGroove {
                source: GrooveSource::Pool { id: gid },
                timing: 0.85,
                velocity: 0.5,
                length: 0.0,
            });
        }
    }
    p
}

fn app_for(theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-notefx");
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
    let c = app.project().tracks[0].clips[0].id;
    match what {
        "groove" | "clip" => app.select_clip(t, c),
        "roll" | "fold" | "key" | "rack" => app.select_clip(t, c),
        _ => app.select_track(t),
    }
    if what == "chain" {
        // The scale folded to its title, as a long chain is kept.
        let id = app.project().tracks[0].note_fx[2].id;
        app.notefx.folded.insert(id);
    }
    let size = match what {
        "chain" | "choice" | "add" => vec2(380.0, 1500.0),
        "groove" | "clip" => vec2(380.0, 1100.0),
        "rack" => vec2(1100.0, 420.0),
        _ => vec2(1100.0, 560.0),
    };
    let mut frame = 0;
    cantus_ui::raster::shoot(&ctx, size, scale, 6, Some(path), |root| {
        frame += 1;
        if frame == 2 {
            let r = egui::Rect::from_min_size(egui::pos2(150.0, 70.0), vec2(160.0, 22.0));
            match what {
                "add" => app.notefx.add_menu = Some((t, r)),
                "fold" => cantus_ui::views::piano_roll::scale::set_fold(&mut app, true),
                "choice" => {
                    let id = app.project().tracks[0].note_fx[1].id;
                    let r = egui::Rect::from_min_size(egui::pos2(150.0, 520.0), vec2(220.0, 22.0));
                    app.notefx.choice_menu = Some((t, id, "rate", r));
                }
                "groove" => app.notefx.groove_menu = Some((t, c, r)),
                "key" => {
                    let r = egui::Rect::from_min_size(egui::pos2(560.0, 4.0), vec2(96.0, 22.0));
                    app.notefx.key_menu = Some(r);
                }
                _ => {}
            }
        }
        match what {
            "chain" | "choice" | "add" | "clip" | "groove" => {
                egui::Panel::left(egui::Id::new("insp"))
                    .exact_size(380.0)
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| cantus_ui::views::inspector::show(&mut app, ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                    .show(root, |_| {});
            }
            "rack" => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| {
                        cantus_ui::views::channel_rack::show(&mut app, ui)
                    });
            }
            _ => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| cantus_ui::views::piano_roll::show(&mut app, ui));
            }
        }
        cantus_ui::notefx_menus::frame(&mut app, &root.ctx().clone());
    });
}
