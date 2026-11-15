//! r40:ripple — insert and delete time and the section drag, rendered headless (ADR 0116):
//! the time selection's menu with *Insert time* and *Delete time*; the song after two bars
//! were inserted (the new time selected, the sections and the marker moved); and a section
//! being dragged — moved, copied (Ctrl), moved in (Shift), copied in (Ctrl+Shift) — in
//! Slate Night and Paper, the whole window at 1920 × 1080 (125 %).
//!
//! `cargo run -p cantus-ui --example shoot_ripple -- <out_dir> [case…]`

use cantus_core::arrange::{Section, SectionId};
use cantus_core::ripple_bars::Landing;
use cantus_core::{Marker, MarkerId, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::ripple_ui::BarsDrop;
use cantus_ui::{raster, CantusApp};
use std::path::{Path, PathBuf};

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

/// The demo song with three sections and a marker.
fn song() -> Project {
    let mut p = cantus_ui::demos::startup_project();
    for (from, len, name) in [(0, 4, "Intro"), (4, 8, "Drop"), (12, 4, "Outro")] {
        let id = SectionId(p.alloc_id());
        p.sections.push(Section {
            id,
            start: bar(from),
            length: bar(len),
            name: name.into(),
            color: None,
        });
    }
    let id = MarkerId(p.alloc_id());
    p.markers.push(Marker {
        id,
        tick: bar(6),
        name: "Vox in".into(),
        color: None,
    });
    p
}

#[derive(Copy, Clone)]
enum Scene {
    /// The time selection over bars 5–6 and the empty timeline's menu open.
    Menu,
    /// Two bars inserted at bar 5: the new time selected.
    Inserted,
    /// Two bars deleted at bar 5.
    Deleted,
    /// A section being dragged: which (0 Intro, 1 Drop), where to, how.
    Drag(usize, i64, Landing),
    /// The ripple mode on this track, after the Kick's first clip was deleted.
    RippleTrack,
    /// The ripple mode on every track (its key and word).
    RippleAll,
    /// Settings ▸ Editing with *Section drag*.
    Settings,
}

fn cases() -> Vec<(String, &'static str, bool, Scene)> {
    let land = |copy, insert| Landing { copy, insert };
    let scenes = [
        ("menu", Scene::Menu),
        ("inserted", Scene::Inserted),
        ("deleted", Scene::Deleted),
        ("drag-replace", Scene::Drag(0, 12, land(false, false))),
        ("drag-copy", Scene::Drag(0, 12, land(true, false))),
        ("drag-insert", Scene::Drag(0, 12, land(false, true))),
        ("drag-insert-copy", Scene::Drag(1, 16, land(true, true))),
        ("mode-track", Scene::RippleTrack),
        ("mode-all", Scene::RippleAll),
        ("settings", Scene::Settings),
    ];
    let mut v = Vec::new();
    for (light, preset, dark) in [
        ("night", "slate_night", true),
        ("paper", "slate_paper", false),
    ] {
        for (name, scene) in scenes {
            v.push((format!("{name}-{light}"), preset, dark, scene));
        }
    }
    v
}

fn shoot(dir: &Path, name: &str, preset: &str, dark: bool, scene: Scene) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-ripple");
    let mut app = CantusApp::from_parts(Some(song()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    app.workspace.hide(cantus_ui::views::workspace::Pane::Mixer);
    app.workspace
        .hide(cantus_ui::views::workspace::Pane::PianoRoll);
    app.workspace
        .hide(cantus_ui::views::workspace::Pane::ChannelRack);
    app.arrangement.px_per_beat = 15.0;
    if let Some(t) = app.project().tracks.get(5).map(|t| t.id) {
        app.select_track(t);
    }
    match scene {
        Scene::Menu => app.set_time_selection(bar(4), bar(6)),
        Scene::Inserted => {
            app.set_time_selection(bar(4), bar(6));
            app.insert_time();
        }
        Scene::Deleted => {
            app.set_time_selection(bar(4), bar(6));
            app.delete_time();
        }
        Scene::RippleTrack => {
            app.arrangement.ripple = cantus_core::ripple_mode::RippleMode::Track;
            let t = app.project().tracks[0].id;
            let c = app.project().tracks[0].clips[0].id;
            app.select_clip(t, c);
            app.delete_clip_selection();
        }
        Scene::RippleAll => app.arrangement.ripple = cantus_core::ripple_mode::RippleMode::All,
        Scene::Settings => {
            use cantus_ui::views::workspace_settings::{open, SettingsPage};
            app.settings.settings_page = Some(SettingsPage::Editing.key().into());
            open(&mut app, SettingsPage::Editing);
        }
        Scene::Drag(i, to, how) => {
            let s = app.project().sections[i].clone();
            app.arrangement.sections.selected = Some(s.id);
            app.arrangement.sections.landing = Some(BarsDrop {
                section: s.id,
                from: s.start,
                to: s.end(),
                dest: bar(to),
                how,
            });
        }
    }
    let (w, h, zoom) = (1920.0, 1080.0, 1.25);
    let points = egui::vec2(w / zoom, h / zoom);
    let path = dir.join(format!("ripple-{name}.png"));
    // The menu: a right-click on the empty timeline under the tracks, at bar 6.
    let menu_at: std::cell::Cell<Option<egui::Pos2>> = std::cell::Cell::new(None);
    raster::shoot_events(
        &ctx,
        points,
        zoom,
        10,
        Some(&path),
        |frame, _| {
            let Some(at) = menu_at.get() else {
                return Vec::new();
            };
            let b = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            match frame {
                3 => vec![egui::Event::PointerMoved(at)],
                4 => vec![b(true)],
                5 => vec![b(false)],
                _ => Vec::new(),
            }
        },
        |root| {
            app.frame(root);
            if matches!(scene, Scene::Menu) && menu_at.get().is_none() {
                if let Some(g) = app.arrangement.geometry() {
                    let below = g.rows.iter().map(|r| r.top + r.height).fold(0.0, f32::max);
                    let y = (below + 30.0).min(g.lanes_bottom - 10.0);
                    menu_at.set(Some(egui::pos2(g.x(22.0), y)));
                }
            }
        },
    );
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-ripple"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for (name, preset, dark, scene) in cases() {
        if only.is_empty() || only.iter().any(|o| name.starts_with(o.as_str())) {
            shoot(&dir, &name, preset, dark, scene);
        }
    }
}
