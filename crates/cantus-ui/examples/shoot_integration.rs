//! Round 39's integration: the whole window on the demo song at 1920 × 1080 and 125 %, in
//! Slate Night (and Slate Lapis where this build has it) — the arrangement, the mixer with
//! its Flow cables shown, the piano roll, the channel rack, the browser, Settings and the
//! track icon picker — for the integrator to look at before the owner does.
//!
//! `cargo run -p cantus-ui --example shoot_integration [-- out_dir [case…]]`
//! Writes `<view>-<theme>-1920x1080.png`; a case on the command line renders only the cases
//! whose name starts with it.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views;
use cantus_ui::views::mixer::layout::CableView;
use cantus_ui::views::workspace::Pane;
use std::path::PathBuf;

type Setup = fn(&mut CantusApp);

fn chords(a: &CantusApp) -> Option<(cantus_core::TrackId, cantus_core::ClipId)> {
    a.project().tracks.iter().find_map(|t| {
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
    })
}

/// A few tracks wear their own icons, as a user would set them.
fn icons(a: &mut CantusApp) {
    let picks = [
        "kick", "snare", "hihat", "bass", "piano", "pad", "lead", "mic",
    ];
    let ids: Vec<_> = a.project().tracks.iter().map(|t| t.id).collect();
    for (id, g) in ids.into_iter().zip(picks) {
        views::track_glyphs::set(a, id, Some(g));
    }
}

fn arrangement(a: &mut CantusApp) {
    icons(a);
    if let Some((t, _)) = chords(a) {
        a.select_track(t);
    }
}
fn mixer(a: &mut CantusApp) {
    icons(a);
    a.show_central(CentralView::Mixer);
    let mut prefs = a.mixer.prefs();
    prefs.cables = CableView::All;
    a.mixer.set_prefs(prefs);
}
fn piano_roll(a: &mut CantusApp) {
    if let Some((t, c)) = chords(a) {
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
}
fn settings(a: &mut CantusApp) {
    a.show_settings = true;
}
fn picker(a: &mut CantusApp) {
    icons(a);
    // The picker open from the sixth track's header, where its icon sits.
    if let Some(t) = a.project().tracks.get(5).map(|t| t.id) {
        a.select_track(t);
        let at = egui::pos2(260.0, 330.0);
        let ctx = egui::Context::default();
        a.arrangement.glyphs.open_for(t, at, &ctx);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-integration")
    });
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let views: [(&str, Setup); 7] = [
        ("arrangement", arrangement),
        ("mixer", mixer),
        ("pianoroll", piano_roll),
        ("rack", rack),
        ("browser", browser),
        ("settings", settings),
        ("icon-picker", picker),
    ];
    let themes: Vec<(&str, bool)> = ["slate_night", "slate_lapis_night"]
        .into_iter()
        .filter(|p| {
            cantus_ui::kit::appearance::PRESETS
                .iter()
                .any(|t| t.0 == *p)
        })
        .map(|p| (p, true))
        .collect();
    let (size, zoom) = ((1920.0f32, 1080.0f32), 1.25f32);
    for (view, setup) in views {
        for &(preset, dark) in &themes {
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
            let tmp = std::env::temp_dir().join("cantus-shoot-integration");
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
