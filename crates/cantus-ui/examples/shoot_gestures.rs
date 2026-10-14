//! FL's quick gestures (round 27, ADR 0051), screenshotted without a screen — the whole
//! window, 1280 × 720 pixels:
//!
//! - `name-hover`: the pointer on a track name's letters (an I-beam on screen; the hint
//!   bar says a click renames);
//! - `name-open`: those letters clicked — the kit field in place, the name selected;
//! - `header-hover`: the pointer on the header off the letters (the hint bar says what a
//!   double-click, a drag and a right-click do);
//! - `header-open`: what a double-click on a header opens — the track's instrument;
//! - `clip-roll`, `clip-rack`: what a double-click on a clip opens — the piano roll on a
//!   melody, the channel rack on a step pattern.
//!
//! (A double-click is two clicks within 0.3 s and these pictures' clock moves a quarter
//! second a frame, so the `*-open` and `clip-*` scenes run what the double-click runs; the
//! gestures themselves are driven in `tests/quick_gestures.rs`.)
//!
//! Each in Cantus Night and Paper, English and Spanish, at 75, 100 and 200 %.
//!
//! `cargo run --release -p cantus-ui --example shoot_gestures -- <out-dir> [filter…]`
//! writes `gesture-<what>-<theme>-<lang>-<scale>.png`; a filter keeps the names holding it.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project};
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::raster::shoot_events;
use cantus_ui::{welcome_project, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2};
use std::cell::Cell;
use std::path::{Path, PathBuf};

const WHAT: [&str; 13] = [
    "name-hover",
    "name-open",
    "header-hover",
    "header-open",
    "clip-roll",
    "clip-rack",
    "rack-far",
    "short-hover",
    "short-open",
    "hosted-open",
    "hosted-show",
    "card-open",
    "card-show",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in WHAT {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [0.75f32, 1.0, 2.0] {
                    let name = format!(
                        "gesture-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    if what.starts_with("hosted") {
                        inspector(&path, what, theme, scale);
                    } else {
                        window(&path, what, theme, scale);
                    }
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

/// The app with its saved state kept off the user's files and the picture's theme named
/// in its appearance (the window applies it on its first frame).
fn app_for(project: Project, theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-gestures");
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

/// The welcome song with Unda on the keys: an instrument with a window of its own. For
/// `short-*`, every track short (0.34 of the base row); for `rack-far`, forty synth tracks
/// and a kit with a four-on-the-floor pattern below them.
fn project(what: &str) -> Project {
    if what == "rack-far" {
        let mut p = Project::new("Many");
        for i in 0..40 {
            let t = p.make_track(format!("Synth {}", i + 1));
            p.tracks.push(t);
        }
        let mut kit = p.make_track("Kit");
        kit.instrument = InstrumentSpec::Pulsus(cantus_core::PulsusParams::default());
        let mut clip = p.make_clip(
            "Four",
            cantus_core::Ticks::ZERO,
            cantus_core::Ticks::beats(4),
        );
        for i in 0..4 {
            let n = p.make_note(
                cantus_core::Ticks::QUARTER * i,
                cantus_core::Ticks::SIXTEENTH,
                36,
                110,
            );
            clip.notes.push(n);
        }
        kit.clips.push(clip);
        p.tracks.push(kit);
        return p;
    }
    let mut p = welcome_project();
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    if what.starts_with("short") {
        for t in &mut p.tracks {
            t.height = if what == "short-open" { 0.6 } else { 0.34 };
        }
    }
    p
}

/// A test plug-in's library (`cantus-test-clap` or `cantus-test-clap-synth`), built from
/// source into `target/test-plugins` the first time — where the host's tests build it.
fn test_plugin(package: &str, file: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let lib = target.join("debug").join(if cfg!(windows) {
        format!("{file}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{file}.dylib")
    } else {
        format!("lib{file}.so")
    });
    if !lib.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", package, "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    lib
}

/// The test instrument on the keys, as the picker puts it there.
fn host_synth(app: &mut CantusApp) {
    use cantus_host::{Backend, DbEntry, ScanStatus};
    app.plugins.db_path = None;
    let folder = std::env::temp_dir().join("cantus-shoot-gestures-synth");
    let _ = std::fs::create_dir_all(&folder);
    let file = folder.join("Cantus Test Synth.clap");
    let _ = std::fs::copy(
        test_plugin("cantus-test-clap-synth", "cantus_test_clap_synth"),
        &file,
    );
    let plugins = cantus_host::clap::ClapBackend
        .scan(&file)
        .unwrap_or_default();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap_or((0, 0));
    app.plugins.db.put(DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: ScanStatus::Ok,
        plugins,
    });
    let keys = app.project().tracks[0].id;
    if let Some(info) = app.hosted_instruments().first().cloned() {
        app.set_hosted_instrument(keys, &info);
    }
    app.select_track(keys);
}

fn click(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// The inspector alone, 290 × 900 points (as `shoot_hosted` shows it), so the hosted row is
/// on screen at every scale: the test instrument on the keys, its window closed or open.
fn inspector(path: &Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(project(what), theme);
    host_synth(&mut app);
    let keys = app.project().tracks[0].id;
    if what == "hosted-show" {
        app.open_hosted_window(keys);
    }
    cantus_ui::raster::shoot(&ctx, vec2(290.0, 900.0), scale, 6, Some(path), |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| cantus_ui::views::inspector::show(&mut app, ui));
    });
    app.plugins.close_instrument_window(keys);
}

fn window(path: &Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(project(what), theme);
    match what {
        "short-hover" => app.arrangement.track_height = 40.0,
        "short-open" => app.arrangement.track_height = 22.0,
        "hosted-open" | "hosted-show" => host_synth(&mut app),
        "card-open" | "card-show" => cantus_ui::views::plugins::demo_setup(
            &mut app,
            &test_plugin("cantus-test-clap", "cantus_test_clap"),
            true,
        ),
        _ => {}
    }
    let size = vec2(1280.0, 720.0) / scale;
    // Where the name of the track aimed at was drawn, and a point of its header off the
    // letters (the second track on short rows, whose letters reach into the first's band).
    let aim = usize::from(what.starts_with("short"));
    let letters: Cell<Option<Pos2>> = Cell::new(None);
    let body: Cell<Option<Pos2>> = Cell::new(None);
    let mut frames = 0;
    shoot_events(
        &ctx,
        size,
        scale,
        14,
        Some(path),
        |frame, _| {
            let at = match what {
                "name-hover" | "name-open" | "short-hover" | "short-open" => letters.get(),
                "header-hover" => body.get(),
                _ => None,
            };
            let Some(at) = at.filter(|_| frame >= 4) else {
                return Vec::new();
            };
            match (what, frame) {
                ("name-open" | "short-open", 5) => vec![click(at, true)],
                ("name-open" | "short-open", 6) => vec![click(at, false)],
                ("name-open" | "short-open", 7) => vec![Event::PointerGone],
                ("name-open" | "short-open", _) => Vec::new(),
                _ => vec![Event::PointerMoved(at)],
            }
        },
        |root| {
            frames += 1;
            if frames == 3 {
                let t = app.project().tracks[0].id;
                match what {
                    "header-open" => app.open_track_instrument(t),
                    "clip-roll" | "clip-rack" | "rack-far" => {
                        let i = match what {
                            "clip-roll" => 0,
                            "clip-rack" => 1,
                            _ => app.project().tracks.len() - 1,
                        };
                        let tr = &app.project().tracks[i];
                        let (t, c) = (tr.id, tr.clips[0].id);
                        app.open_clip_editor(t, c);
                    }
                    "hosted-show" => app.open_hosted_window(t),
                    "card-show" => {
                        if let Some((_, e, _)) = app.fx.open {
                            app.show_plugin_window(e);
                        }
                    }
                    _ => {}
                }
            }
            app.frame(root);
            if let Some(h) = app.arrangement.header_hits().get(aim) {
                letters.set(h.letters.map(|l| l.center()));
                body.set(h.letters.map(|l| pos2(l.right() + 10.0, l.center().y)));
            }
        },
    );
    // The plug-ins' own windows, opened for the pictures, closed again.
    let keys = app.project().tracks[0].id;
    app.plugins.close_instrument_window(keys);
    if let Some((_, e, _)) = app.fx.open {
        app.plugins.close_window(e);
    }
}
