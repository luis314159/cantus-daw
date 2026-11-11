//! r39:lo-arrange — the arrangement and the piano roll as the close-out left them, the whole
//! window at 1920 × 1080 and 125 %, in Slate Night and Slate Paper: the tool row, the clip
//! states (lit, dim, a clip being moved), automation values at the close zoom, the Draw
//! tool in hand, the maximised arrangement with its docks' row, the piano roll's head with a
//! selection.
//!
//! `cargo run -p cantus-ui --example shoot_lo_arrange [-- out_dir [case…]]`

use cantus_core::{AutoPoint, AutoTarget, Automation, Command, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

type Setup = fn(&mut CantusApp);

fn chords(a: &CantusApp) -> Option<(usize, cantus_core::TrackId, cantus_core::ClipId)> {
    a.project().tracks.iter().enumerate().find_map(|(i, t)| {
        t.name
            .to_lowercase()
            .contains("chord")
            .then(|| {
                t.clips
                    .iter()
                    .find(|c| !c.notes.is_empty())
                    .map(|c| (i, t.id, c.id))
            })
            .flatten()
    })
}

/// The song as a user leaves it: the chords' track selected (its clips lit), the lead
/// soloed (the rest dim) for the states case.
fn arrangement(a: &mut CantusApp) {
    if let Some((_, t, _)) = chords(a) {
        a.select_track(t);
    }
}

fn states(a: &mut CantusApp) {
    arrangement(a);
    // A solo: every other track's clips dim.
    if let Some(t) = a.project().tracks.get(2).map(|t| t.id) {
        a.exec(Command::SetTrackSolo {
            id: t,
            soloed: true,
        });
    }
    // A clip being moved two bars on: its slot dashed, a shadow under it.
    if let Some((_, t, c)) = chords(a) {
        let start = a
            .project()
            .track(t)
            .and_then(|tr| tr.clip(c))
            .map(|c| c.start);
        if let Some(start) = start {
            a.select_clip(t, c);
            a.arrangement.shoot_move((t, c), start, Ticks::beats(8));
        }
    }
}

fn close(a: &mut CantusApp) {
    arrangement(a);
    if let Some((_, t, _)) = chords(a) {
        let mut clip = a.bus.new_clip("Volume", Ticks::ZERO, Ticks::beats(16));
        clip.automation = Some(Automation::new(
            AutoTarget::TrackGain(t),
            vec![
                AutoPoint::new(Ticks::ZERO, 0.35),
                AutoPoint::new(Ticks::beats(3), 0.8),
                AutoPoint::new(Ticks::beats(6), 0.55),
                AutoPoint::new(Ticks::beats(10), 0.9),
                AutoPoint::new(Ticks::beats(14), 0.4),
            ],
        ));
        a.exec(Command::InsertClip {
            track: t,
            clip: Box::new(clip),
        });
    }
    a.arrangement.px_per_beat = 90.0;
    // The chords' row and its new lane at the top of the lanes.
    a.arrangement.scroll_y = 5.0 * a.arrangement.track_height;
    a.arrangement.tool = views::arrangement::tools::Tool::Draw;
}

fn maximised(a: &mut CantusApp) {
    arrangement(a);
    a.workspace.toggle_maximise(Pane::Arrangement);
}

fn piano_roll(a: &mut CantusApp) {
    if let Some((i, t, c)) = chords(a) {
        a.selected_track = Some(t);
        a.selected_clip = Some(c);
        a.piano_roll.key_height = 12.0;
        views::piano_roll::fit_clip(a, t, c);
        a.selected_notes = a.project().tracks[i]
            .clip(c)
            .map(|c| c.notes.iter().skip(2).take(6).map(|n| n.id).collect())
            .unwrap_or_default();
    }
    a.show_central(CentralView::PianoRoll);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-lo-arrange")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let cases: [(&str, Setup); 5] = [
        ("arrangement", arrangement),
        ("states", states),
        ("close", close),
        ("maximised", maximised),
        ("pianoroll", piano_roll),
    ];
    let themes = [("slate_night", true), ("slate_paper", false)];
    let (size, zoom) = ((1920.0f32, 1080.0f32), 1.25f32);
    for (case, setup) in cases {
        for (preset, dark) in themes {
            let name = format!("{case}-{}-1920x1080", preset.trim_start_matches("slate_"));
            if !only.is_empty() && !only.iter().any(|o| name.starts_with(o.as_str())) {
                continue;
            }
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-lo-arrange");
            let _ = std::fs::remove_dir_all(&tmp);
            let project = cantus_ui::demos::startup_project();
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.layouts_path = tmp.join("layouts.json");
            app.keymap_path = tmp.join("keymap.toml");
            app.appearance_path = tmp.join("appearance.json");
            app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
            let mode = app.appearance.mode;
            app.appearance.assign(mode, format!("preset:{preset}"));
            app.attach(&ctx);
            setup(&mut app);
            let points = egui::vec2(size.0 / zoom, size.1 / zoom);
            let path = dir.join(format!("{name}.png"));
            raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
            println!("{}", path.display());
        }
    }
}
