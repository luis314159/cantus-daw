//! The website's and the README's pictures of round 36 (the recording mode, the desk's VCAs
//! and snapshots), headless, as a machine with an interface plugged in shows them:
//!
//! ```text
//! cargo run --release -p cantus-ui --example shoot_showcase -- <out-dir>
//! ```
//!
//! Like `shoot_readme`, the program runs with the demo interface of the audio setup (an ASIO
//! driver at 48 kHz), so no picture carries the "no audio device" line; the engine is the
//! manual one, run in 64-frame blocks so the bar's buffer reads what the mode is for. Points
//! 1280 × 720 at 150 % (1920 × 1080 pixels), sharp when a page shows them large.
//!
//! - `record-on-<theme>-<lang>`: *Neon Tide* in the recording mode — every strip that feeds
//!   the master baked, a vocal track armed and live, the bar across the window. The CPU
//!   figures in the bar are the shot's (`record_mode::stage_for_shot`), not a measurement.
//! - `record-ask-<theme>-<lang>`: *Prepare to record*'s question on the same song.
//! - `mix-snap-<theme>-<lang>`: two VCAs on the desk (the drums and the music following), the
//!   *Snapshots* menu open with a cue listed.
//!
//! PNGs are 8-bit RGB; run `oxipng` over them when publishing.

use cantus_core::mix_cmd::MixCommand;
use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
use cantus_core::vca::StripKind;
use cantus_core::{Color, Command, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::{demos, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

#[path = "shots_common/studio.rs"]
mod studio;

const POINTS: (f32, f32) = (1280.0, 720.0);
const ZOOM: f32 = 1.5;
/// The block the manual engine runs: what the bar reads as the buffer.
const BLOCK: usize = 64;

/// The app on `project` in `preset`, with the demo interface, attached to `ctx`.
fn studio(ctx: &egui::Context, project: Project, dark: bool, engine: EngineHandle) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-showcase");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    let preset = if dark { "slate_night" } else { "slate_paper" };
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(ctx);
    // The demo interface, and no dropout on the status line.
    studio::studio(&mut app, BLOCK as u32);
    app.samples.content_root = Some(tmp.join("Content"));
    app.settings.sample_folders.clear();
    // Renders (the mode's, made in the background) never in the user's cache.
    app.engine.frozen.set_app_dir(tmp.join("renders"));
    app.set_status(String::new());
    app
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The song the program opens on, with a vocal track armed for a take.
fn song() -> Project {
    let mut p = demos::startup_project();
    let mut vocal = p.make_track("Vocal take");
    vocal.armed = true;
    p.tracks.insert(0, vocal);
    p
}

fn record(dir: &Path, dark: bool, lang: Locale, ask: bool) {
    i18n::set_locale(lang);
    let ctx = context();
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = studio(&ctx, song(), dark, engine);
    // The song's dancer stays off the picture (Alt+D), and the armed take is the selection.
    app.settings.stage.hidden = true;
    app.selected_track = Some(app.project().tracks[0].id);
    app.sync_engine();
    let mut audio = vec![0f32; BLOCK * 2];
    // The question is asked before anything is baked: the whole song live, which on a busy
    // build machine overruns 64-frame blocks and puts a dropout line in the status bar. The
    // engine is not run for that picture (the question shows no meter or buffer).
    let run = !ask;
    for _ in 0..(if run { 8 } else { 0 }) {
        processor.process(&mut audio, 2);
    }
    if ask {
        app.record_mode_ask();
    } else {
        cantus_ui::record_mode::stage_for_shot(&mut app, false);
    }
    let theme = if dark { "night" } else { "paper" };
    let what = if ask { "ask" } else { "on" };
    let path = dir.join(format!("record-{what}-{theme}-{}.png", lang.code()));
    let points = egui::vec2(POINTS.0, POINTS.1);
    raster::shoot(&ctx, points, ZOOM, 10, Some(&path), |root| {
        if run {
            processor.process(&mut audio, 2);
        }
        app.frame(root)
    });
    println!("{}", path.display());
}

/// The welcome project with named, coloured strips and two VCAs (as `shoot_mix` builds it).
fn desk() -> Project {
    let mut p = welcome_project();
    while p.strips.len() < 9 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    // The empty slots go: the VCAs stand right after the named strips.
    p.strips.truncate(9);
    let names = [
        "Kick", "Snare", "Hats", "Bass", "Keys", "Pad", "Lead", "Vox", "Reverb",
    ];
    for (i, n) in names.iter().enumerate() {
        let s = &mut p.strips[i];
        s.name = (*n).into();
        s.gain_db = [0.0, -2.5, -6.0, -1.0, -4.0, -8.0, -3.0, -1.5, -5.0][i];
        s.pan = [0.0, 0.1, -0.3, 0.0, 0.2, -0.5, 0.4, 0.0, 0.0][i];
        s.color = Some(Color::PALETTE[(i * 3) % Color::PALETTE.len()]);
    }
    for (name, db, members) in [
        ("Drums VCA", -3.0f32, [0usize, 1, 2]),
        ("Music VCA", 0.0, [3, 4, 5]),
    ] {
        let mut v = p.make_strip(name);
        v.kind = StripKind::Vca;
        v.gain_db = db;
        let id = v.id;
        p.strips.push(v);
        for m in members {
            p.strips[m].vcas.push(id);
        }
    }
    let state = capture(&p, &[], true);
    for name in ["Verse", "Chorus", "Bridge"] {
        let id = MixSnapId(p.alloc_id());
        p.mix.snapshots.push(MixSnapshot {
            id,
            name: name.into(),
            state: state.clone(),
        });
    }
    let chorus = p.mix.snapshots[1].id;
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(8),
        snapshot: chorus,
        scope: Vec::new(),
    });
    p
}

fn mix(dir: &Path, dark: bool, lang: Locale) {
    i18n::set_locale(lang);
    let ctx = context();
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = studio(&ctx, desk(), dark, engine);
    app.central = cantus_ui::app::CentralView::Mixer;
    cantus_ui::views::mixer::set_width_mode(
        &mut app,
        cantus_ui::views::mixer::layout::WidthMode::from_index(1),
    );
    let kick = app.project().strips[0].id;
    app.select_strip(kick);
    let id = app.project().mix.snapshots[0].id;
    app.exec(Command::Mix(MixCommand::Recall {
        id,
        scope: Vec::new(),
        master: true,
        include_safe: false,
    }));
    cantus_ui::views::mixer::open_snapshots(&mut app, egui::pos2(300.0, 60.0));
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; BLOCK * 2];
    let theme = if dark { "night" } else { "paper" };
    let path = dir.join(format!("mix-snap-{theme}-{}.png", lang.code()));
    raster::shoot(
        &ctx,
        egui::vec2(POINTS.0, POINTS.1),
        ZOOM,
        14,
        Some(&path),
        |root| {
            for _ in 0..20 {
                processor.process(&mut audio, 2);
            }
            app.frame(root)
        },
    );
    println!("{}", path.display());
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
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/showcase"));
    std::fs::create_dir_all(&dir).expect("output dir");
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            record(&dir, dark, lang, false);
            record(&dir, dark, lang, true);
            mix(&dir, dark, lang);
        }
    }
}
