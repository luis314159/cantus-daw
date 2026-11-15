//! r40:tymp-ui — how a user's strip group (ADR 0016: right-click ▸ Group ▸ New group from
//! selection) looks on the desk today, for the owner: a Neon Tide-like desk with the group
//! "Voces" (Vox, Choir, Pad) open, the same group folded, and the group's strips docked at
//! the right beside the returns. Slate Night and Slate Paper, 1280 × 720 at 100 %.
//!
//! `cargo run -p cantus-ui --example shoot_groups_today -- <out_dir>`

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq)]
enum Case {
    Open,
    Folded,
    Docked,
}

fn song(case: Case) -> (Project, TrackId) {
    let mut p = Project::new("Neon Tide");
    let names = ["Drums", "Bass", "Piano", "Guitar", "Vox", "Choir", "Pad"];
    let mut first = None;
    for (i, (name, color)) in names.iter().zip([0usize, 2, 1, 5, 4, 7, 6]).enumerate() {
        let mut t = p.make_track(*name);
        t.strip = Some(p.strips[i].id);
        t.color = Color::PALETTE[color];
        first.get_or_insert(t.id);
        p.tracks.push(t);
    }
    // Two returns, docked at the right as a session has them.
    for (i, name) in [(14usize, "Reverb"), (15, "Delay")] {
        p.strips[i].name = name.into();
        p.strips[i].dock = Dock::Right;
    }
    let mut g = p.make_strip_group("Voces", Color::PALETTE[7]);
    g.collapsed = case == Case::Folded;
    let gid = g.id;
    p.strip_groups.push(g);
    for i in [4usize, 5, 6] {
        p.strips[i].group = Some(gid);
        if case == Case::Docked {
            p.strips[i].dock = Dock::Right;
        }
    }
    if case == Case::Docked {
        // The group's strips beside the returns: in the strips' order, before them.
        let moved: Vec<MixerStrip> = [4usize, 5, 6]
            .iter()
            .map(|i| p.strips[*i].clone())
            .collect();
        p.strips.retain(|s| !moved.iter().any(|m| m.id == s.id));
        let at = p
            .strips
            .iter()
            .position(|s| s.name == "Reverb")
            .unwrap_or(p.strips.len());
        for (k, s) in moved.into_iter().enumerate() {
            p.strips.insert(at + k, s);
        }
    }
    (p, first.unwrap())
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-groups"));
    std::fs::create_dir_all(&dir).expect("the output folder");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let tmp = std::env::temp_dir().join("cantus-shoot-groups");
    for (case, label) in [
        (Case::Open, "open"),
        (Case::Folded, "folded"),
        (Case::Docked, "docked-right"),
    ] {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let (project, first) = song(case);
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
            let mode = app.appearance.mode;
            app.appearance.assign(mode, format!("preset:{preset}"));
            app.attach(&ctx);
            app.show_central(CentralView::Mixer);
            app.workspace
                .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
            cantus_ui::views::mixer::set_width_mode(
                &mut app,
                cantus_ui::views::mixer::layout::WidthMode::Wide,
            );
            app.select_track(first);
            let theme = preset.trim_start_matches("slate_");
            let path = dir.join(format!("groups-{label}-{theme}-100.png"));
            raster::shoot(
                &ctx,
                egui::vec2(1280.0, 720.0),
                1.0,
                8,
                Some(&path),
                |root| app.frame(root),
            );
            println!("{}", path.display());
        }
    }
}
