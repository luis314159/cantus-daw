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

const WHAT: [&str; 6] = [
    "name-hover",
    "name-open",
    "header-hover",
    "header-open",
    "clip-roll",
    "clip-rack",
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
                    window(&path, what, theme, scale);
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

/// The welcome song with Unda on the keys: an instrument with a window of its own.
fn project() -> Project {
    let mut p = welcome_project();
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    p
}

fn click(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn window(path: &Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(project(), theme);
    let size = vec2(1280.0, 720.0) / scale;
    // Where the first track's name was drawn, and a point of its header off the letters.
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
                "name-hover" | "name-open" => letters.get(),
                "header-hover" => body.get(),
                _ => None,
            };
            let Some(at) = at.filter(|_| frame >= 4) else {
                return Vec::new();
            };
            match (what, frame) {
                ("name-open", 5) => vec![click(at, true)],
                ("name-open", 6) => vec![click(at, false)],
                ("name-open", 7) => vec![Event::PointerGone],
                ("name-open", _) => Vec::new(),
                _ => vec![Event::PointerMoved(at)],
            }
        },
        |root| {
            frames += 1;
            if frames == 3 {
                let t = app.project().tracks[0].id;
                match what {
                    "header-open" => app.open_track_instrument(t),
                    "clip-roll" | "clip-rack" => {
                        let i = if what == "clip-roll" { 0 } else { 1 };
                        let tr = &app.project().tracks[i];
                        let (t, c) = (tr.id, tr.clips[0].id);
                        app.open_clip_editor(t, c);
                    }
                    _ => {}
                }
            }
            app.frame(root);
            if let Some(h) = app.arrangement.header_hits().first() {
                letters.set(h.letters.map(|l| l.center()));
                body.set(h.letters.map(|l| pos2(l.right() + 10.0, l.center().y)));
            }
        },
    );
}
