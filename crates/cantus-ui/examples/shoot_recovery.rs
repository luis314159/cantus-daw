//! Screenshots of *Recover your work* (round 22's crash recovery, `views::recovery`) over
//! the whole window, without a screen: in the light and the dark theme, in English and
//! Spanish, for the three kinds of offer —
//!
//! - `crash`: a crash, an autosave twelve minutes old, three takes recorded since the last
//!   save, a project file with a long path;
//! - `closed`: a close with unsaved changes kept in the autosave, a project never saved;
//! - `bare`: a crash before any autosave, nothing recorded.
//!
//! `cargo run -p cantus-ui --example shoot_recovery -- <out-dir> [scale]`

use cantus_core::audio::TakeMode;
use cantus_core::{Ticks, TrackId};
use cantus_engine::{EngineHandle, FinishedTake};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster::shoot;
use cantus_ui::recovery::{Journal, JournalPass, Orphan};
use cantus_ui::recovery_restore::{Item, Offer, RecoveredPass};
use cantus_ui::{welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const W: f32 = 1400.0;
const H: f32 = 820.0;
const KINDS: [&str; 3] = ["crash", "closed", "bare"];

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn journal(name: &str, path: Option<PathBuf>) -> Journal {
    Journal {
        version: 1,
        pid: 4242,
        started_at: now() - 3_600,
        app_version: "0.1.0-beta.1".into(),
        project_name: name.into(),
        project_path: path,
        dirty: true,
        autosave: None,
        autosaved_at: None,
        closed: false,
        passes: Vec::new(),
    }
}

fn takes(n: usize) -> RecoveredPass {
    let pass = JournalPass {
        record_start: Ticks::beats(16),
        transport_start: Ticks::beats(12),
        punch: None,
        loop_region: None,
        loop_takes: false,
        mode: TakeMode::Stack,
        tempo_bpm: 120.0,
        sample_rate: 48_000,
        latency_frames: 0,
        takes: Vec::new(),
        finished: false,
    };
    let takes = (0..n)
        .map(|i| FinishedTake {
            track: TrackId(i as u64 + 1),
            path: PathBuf::from(format!("C:/Music/Cantus/Recordings/Take {i}.wav")),
            sample_rate: 48_000,
            channels: 1,
            frames: 480_000,
            start_tick: 0.0,
            wraps: Vec::new(),
            dropped: 0,
        })
        .collect();
    RecoveredPass {
        journal: pass,
        takes,
    }
}

fn item(kind: &str) -> Item {
    let (name, path, crashed, autosaved, n) = match kind {
        "crash" => (
            "Sunday Strut (final mix)",
            Some(PathBuf::from(
                "C:/Users/Luis/Music/Cantus Projects/2026/Sunday Strut/Sunday Strut (final mix).cantus",
            )),
            true,
            Some(now() - 12 * 60),
            3,
        ),
        "closed" => ("Untitled idea", None, false, Some(now() - 40), 0),
        _ => ("Paper Lanterns", None, true, None, 0),
    };
    let autosave = autosaved.map(|_| (PathBuf::from("autosave.cantus"), welcome_project()));
    Item {
        orphan: Orphan {
            journal: journal(name, path.clone()),
            json: PathBuf::from("session.json"),
        },
        name: name.into(),
        project_path: path,
        autosave,
        autosaved_at: autosaved,
        crashed,
        passes: if n > 0 { vec![takes(n)] } else { Vec::new() },
    }
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

fn window(ctx: &egui::Context, a: &mut CantusApp, path: &Path, scale: f32) {
    shoot(ctx, egui::vec2(W, H), scale, 8, Some(path), |root| {
        a.frame(root)
    });
    println!("wrote {}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    std::fs::create_dir_all(&dir).expect("output dir");
    let scratch = std::env::temp_dir().join("cantus-shoot-recovery");
    let _ = std::fs::create_dir_all(&scratch);
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        for kind in KINDS {
            for dark in [false, true] {
                let ctx = context();
                let mut a =
                    CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
                // Never touch the user's files.
                a.settings_path = scratch.join("settings.json");
                a.layouts_path = scratch.join("layouts.json");
                a.keymap_path = scratch.join("keymap.toml");
                a.appearance_path = scratch.join("appearance.json");
                a.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
                a.verse = None;
                a.attach(&ctx);
                a.recovery = Some(Offer {
                    items: vec![item(kind)],
                });
                let theme = if dark { "dark" } else { "light" };
                let path = dir.join(format!("recovery-{kind}-{theme}-{}.png", locale.code()));
                window(&ctx, &mut a, &path, scale);
            }
        }
    }
}
