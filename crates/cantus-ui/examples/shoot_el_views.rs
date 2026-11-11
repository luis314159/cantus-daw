//! Round 39, `el-views`: the whole window with each of des-views' views in front — the
//! piano roll, the channel rack, the browser, the arrangement's clips at four zooms — in
//! Slate Night and Slate Paper, at 1280 × 720 (100 %) and 1920 × 1080 (125 %), on the demo
//! song the program opens on.
//!
//! `cargo run -p cantus-ui --example shoot_el_views [-- out_dir [case…]]`
//! Writes `<view>-<theme>-<w>x<h>.png`; a case on the command line renders only the cases
//! whose name starts with it.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

type Setup = fn(&mut CantusApp);

fn song(_: &mut CantusApp) {}
fn piano_roll(a: &mut CantusApp) {
    // The chords, as des-views draws them: a melodic clip with notes.
    let pick = a
        .project()
        .tracks
        .iter()
        .filter(|t| !matches!(t.instrument, cantus_core::InstrumentSpec::Pulsus(_)))
        .find_map(|t| {
            t.clips
                .iter()
                .find(|c| c.notes.len() > 8)
                .map(|c| (t.id, c.id, t.name.to_lowercase().contains("chord")))
        });
    let chords = a.project().tracks.iter().find_map(|t| {
        t.name
            .to_lowercase()
            .contains("chord")
            .then(|| {
                t.clips
                    .iter()
                    .find(|c| !c.notes.is_empty())
                    .map(|c| (t.id, c.id))
            })
            .flatten()
    });
    if let Some((t, c)) = chords.or(pick.map(|p| (p.0, p.1))) {
        a.selected_track = Some(t);
        a.selected_clip = Some(c);
        a.piano_roll.key_height = 12.0;
        views::piano_roll::fit_clip(a, t, c);
    }
    a.show_central(CentralView::PianoRoll);
}
fn rack(a: &mut CantusApp) {
    a.show_central(CentralView::ChannelRack);
}
fn browser(a: &mut CantusApp) {
    a.toggle_pane(Pane::Browser);
    // A pack open to its sounds, one chosen, as des-views draws it.
    let user = a.settings.sample_folders.clone();
    for root in a.samples.roots(&user) {
        let Ok(rd) = std::fs::read_dir(&root.path) else {
            continue;
        };
        let mut dirs: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        let Some(dir) = dirs.into_iter().find(|d| {
            std::fs::read_dir(d).is_ok_and(|r| {
                r.flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "wav"))
            })
        }) else {
            continue;
        };
        a.samples.open.insert(root.path.clone());
        a.samples.open.insert(dir.clone());
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut wavs: Vec<_> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "wav"))
                .collect();
            wavs.sort();
            a.samples.selected = wavs.first().cloned();
        }
        break;
    }
}
fn zoom(a: &mut CantusApp, px_per_beat: f32) {
    // The arrangement alone, so every track's clips show.
    for p in [Pane::Mixer, Pane::PianoRoll, Pane::ChannelRack] {
        a.toggle_pane(p);
    }
    // The chords chosen: their clip lit, as des-views' sheet shows the selected track.
    if let Some(t) = a
        .project()
        .tracks
        .iter()
        .find(|t| t.name.to_lowercase().contains("chord"))
    {
        a.selected_track = Some(t.id);
    }
    a.arrangement.px_per_beat = px_per_beat;
}
fn far(a: &mut CantusApp) {
    zoom(a, 4.0);
}
fn default_zoom(a: &mut CantusApp) {
    zoom(a, 12.0);
}
fn near(a: &mut CantusApp) {
    zoom(a, 36.0);
}
fn close(a: &mut CantusApp) {
    zoom(a, 90.0);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-el-views")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let views: [(&str, Setup); 8] = [
        ("song", song),
        ("pianoroll", piano_roll),
        ("rack", rack),
        ("browser", browser),
        ("clips-far", far),
        ("clips-default", default_zoom),
        ("clips-near", near),
        ("clips-close", close),
    ];
    for (view, setup) in views {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            // r39:el-views — a narrow window too, where the toolbars fold.
            for (size, zoom) in [
                ((1280.0f32, 720.0f32), 1.0f32),
                ((1920.0, 1080.0), 1.25),
                ((760.0, 560.0), 1.0),
            ] {
                if size.0 < 1000.0 && view != "pianoroll" && view != "rack" {
                    continue;
                }
                let name = format!(
                    "{view}-{}-{}x{}",
                    preset.trim_start_matches("slate_"),
                    size.0 as u32,
                    size.1 as u32
                );
                if !only.is_empty() && !only.iter().any(|o| name.starts_with(o.as_str())) {
                    continue;
                }
                let ctx = egui::Context::default();
                cantus_ui::design::install_fonts(&ctx);
                cantus_ui::design::install_style(&ctx);
                let tmp = std::env::temp_dir().join("cantus-shoot-el-views");
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
                if let Some(strip) = app.project().tracks.get(1).and_then(|t| t.strip) {
                    app.select_strip(strip);
                }
                setup(&mut app);
                let points = egui::vec2(size.0 / zoom, size.1 / zoom);
                let path = dir.join(format!("{name}.png"));
                raster::shoot(&ctx, points, zoom, 8, Some(&path), |root| app.frame(root));
                println!("{}", path.display());
            }
        }
    }
}
