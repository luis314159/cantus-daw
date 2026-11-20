//! The website tour's pictures (round 41: `web/src/tour.rs`), headless. They show the piano
//! roll with its velocity lane, the channel rack on a drum pattern, and the mixer at its
//! close-up width, all on *Sunday Strut*: the home's program picture plays *Neon Tide*, so the
//! tour shows a second song.
//!
//! The window is the program picture's (`shoot_site`) at 100 %, the browser and the inspector
//! at the same widths and the operating system's caption band on top, but 760 px tall: the
//! arrangement pane is closed, so the view fills the space between them, top to bottom, and the
//! tour shows that space at about the shape of the program picture's own crops.
//! `tools/shots/place.py <dir> tour` writes the WebPs the site serves (`reshoot.sh` runs both).
//!
//! `cargo run --release -p cantus-ui --example shoot_tour -- <out-dir> [night|paper] [en|es]`
//!
//! Writes `tour-{pianoroll,rack,mixer}-<theme>-<lang>.png`.

use cantus_core::{Ticks, TrackId};
use cantus_engine::EngineHandle;
use cantus_ui::app::CentralView;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::{CableView, WidthMode};
use cantus_ui::views::workspace::Pane;
use cantus_ui::{demos, i18n, CantusApp};
use std::path::{Path, PathBuf};

#[path = "shots_common/studio.rs"]
mod studio;

const SIZE: (f32, f32) = (1912.0, 760.0);
/// The height the caption bar of a real window takes.
const CAPTION: f32 = 31.0;
/// The browser's and the inspector's widths, as in the program picture.
const BROWSER_PX: f32 = 277.0;
const INSPECTOR_PX: f32 = 290.0;
/// Beats in a bar at the song's 4/4.
const BAR: i64 = 4;

#[derive(Clone, Copy, PartialEq)]
enum View {
    PianoRoll,
    Rack,
    Mixer,
}

impl View {
    fn name(self) -> &'static str {
        match self {
            View::PianoRoll => "pianoroll",
            View::Rack => "rack",
            View::Mixer => "mixer",
        }
    }
}

/// The track whose name is this demo key's, in the current language.
fn track(app: &CantusApp, key: &str) -> Option<TrackId> {
    let name = i18n::t(key);
    app.project()
        .tracks
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.id)
}

fn main() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

fn run() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let rest: Vec<String> = args.collect();
    let paper = rest.iter().any(|a| a == "paper");
    let spanish = rest.iter().any(|a| a == "es");
    std::fs::create_dir_all(&dir).expect("output dir");
    i18n::set_locale(if spanish {
        i18n::Locale::Es
    } else {
        i18n::Locale::En
    });
    for view in [View::PianoRoll, View::Rack, View::Mixer] {
        shoot(&dir, view, paper, spanish);
    }
}

fn shoot(dir: &Path, view: View, paper: bool, spanish: bool) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let song = (demos::find("funk").expect("Sunday Strut").build)();
    let mut app = CantusApp::from_parts(Some(song), engine, false);
    // Never touch the user's files.
    let tmp = std::env::temp_dir().join("cantus-shoot-tour");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if paper { Mode::Light } else { Mode::Dark };
    app.attach(&ctx);
    // No dancer in any picture of the site (the song has none; the stage stays hidden).
    app.settings.stage.hidden = true;
    studio::studio(&mut app, 256);
    app.set_status(i18n::t("status.autosaved"));
    // The desk ends cleanly: the strips nothing feeds (the empty slots) are removed.
    let empty: Vec<_> = app
        .project()
        .strips
        .iter()
        .filter(|s| s.name.is_empty() && s.effects.is_empty())
        .map(|s| s.id)
        .collect();
    for id in empty {
        app.exec(cantus_core::Command::RemoveStrip { id });
    }
    // ... and the song reads as opened, not as edited (no "•" beside its name).
    app.bus.mark_saved();

    // Where the song plays: the solo (bar 20) for the lead's melody, the first chorus
    // (bar 12) for the groove and the desk.
    let at_bar = if view == View::PianoRoll { 20 } else { 12 };
    match view {
        View::PianoRoll => {
            if let Some(t) = track(&app, "demos.track.lead") {
                let clip = app
                    .project()
                    .track(t)
                    .and_then(|tr| tr.clips.iter().max_by_key(|c| c.notes.len()))
                    .map(|c| c.id);
                if let Some(c) = clip {
                    app.selected_track = Some(t);
                    app.selected_clip = Some(c);
                    app.piano_roll.key_height = 14.0;
                    cantus_ui::views::piano_roll::fit_clip(&mut app, t, c);
                }
            }
        }
        View::Rack => {
            // The snare's clip in the chorus: its ghost notes are the groove's character,
            // and the rack shows every track's clip in the same window.
            if let Some(t) = track(&app, "demos.track.snare") {
                let at = Ticks::beats(at_bar * BAR + 1);
                let clip = app.project().track(t).and_then(|tr| {
                    tr.clips
                        .iter()
                        .find(|c| c.start <= at && at < c.end())
                        .map(|c| c.id)
                });
                if let Some(c) = clip {
                    app.selected_track = Some(t);
                    app.selected_clip = Some(c);
                }
            }
        }
        View::Mixer => {
            let mut prefs = app.mixer.prefs();
            prefs.cables = CableView::Selected;
            app.mixer.set_prefs(prefs);
            // The keys: the reed piano's strip, its sends and its output drawn as cables.
            if let Some(strip) = track(&app, "demos.track.keys")
                .and_then(|t| app.project().track(t))
                .and_then(|t| t.strip)
            {
                app.select_strip(strip);
            }
        }
    }
    let central = match view {
        View::PianoRoll => CentralView::PianoRoll,
        View::Rack => CentralView::ChannelRack,
        View::Mixer => CentralView::Mixer,
    };

    // Playing: the clock, the playhead and the meters move.
    app.sync_engine();
    app.seek(Ticks::beats(at_bar * BAR + 2));
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    for _ in 0..40 {
        processor.process(&mut audio, 2);
    }
    let theme = if paper { "paper" } else { "night" };
    let lang = if spanish { "es" } else { "en" };
    let path = dir.join(format!("tour-{}-{theme}-{lang}.png", view.name()));
    let mut frame = 0;
    raster::shoot(
        &ctx,
        egui::vec2(SIZE.0, SIZE.1),
        1.0,
        16,
        Some(&path),
        |root| {
            // The browser opens once the window's width is known; the arrangement closes,
            // so the view takes the whole height.
            if frame == 1 {
                app.toggle_pane(Pane::Browser);
                app.toggle_pane(Pane::Arrangement);
                app.show_central(central);
                if view == View::Mixer {
                    cantus_ui::views::mixer::set_width_mode(&mut app, WidthMode::from_index(2));
                }
            }
            if frame == 3 {
                let main = app.workspace.dock.main_surface_mut();
                if let egui_dock::Node::Horizontal(split) = &mut main[egui_dock::NodeIndex::root()]
                {
                    split.fraction = BROWSER_PX / SIZE.0;
                }
                app.workspace.impose_sizes(INSPECTOR_PX, 0.0);
            }
            frame += 1;
            processor.process(&mut audio, 2);
            let pal = cantus_ui::theme::p();
            egui::Panel::top(egui::Id::new("tour-caption"))
                .exact_size(CAPTION)
                .resizable(false)
                .frame(egui::Frame::NONE.fill(pal.panel))
                .show(root, |_| {});
            app.frame(root);
        },
    );
    println!("wrote {}", path.display());
}
