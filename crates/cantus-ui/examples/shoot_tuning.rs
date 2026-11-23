//! Round 42's design sheet of the song's tuning (ADR 0155's surface, ADR 0156): Project ▸
//! Tuning in each of its states, the track inspector's override row, and Chorda's Edit ▸
//! Tuning line — the whole window drawn as the app draws it, in Slate Lapis Night and Paper,
//! at 100 % and 200 %:
//!
//! `cargo run -p cantus-ui --example shoot_tuning -- <out-dir> [kind…] [--zoom=1,2]
//! [--theme=night|paper] [--lang=en|es]`
//!
//! Kinds: `equal` (a new song), `werck` (Werckmeister III in Eb at A415), `follow` (Vallotti
//! following a song in D minor), `scale` (31-EDO read from the MIT `tuning-library`'s test
//! file through the window's import), `track` (a track's own tuning), `inspector` (the
//! track's override row), `chorda` (Chorda's Edit view and its tuning line).
//!
//! **Interim, said plainly:** until the `tuning` builder's `Tuning::offsets()` fills the
//! temperaments, `werck` and `follow` draw the circle and the degrees from the published
//! tables (`research/instruments/microtuning-verified.md` §5.2, rotated as §5.4 says, A at
//! the diapason) through `views::tuning::design_preview`; every other state reads the app's
//! own table. r42:tuning-face

use cantus_core::scale::{Key, ScaleKind};
use cantus_core::tuning::{Temperament, TrackTuning, Tuning, TuningSystem, TuningTable};
use cantus_core::Command;
use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::tuning::{self, Target};
use cantus_ui::views::workspace_device as device;
use cantus_ui::{raster, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const KINDS: [&str; 7] = [
    "equal",
    "werck",
    "follow",
    "scale",
    "track",
    "inspector",
    "chorda",
];
const SIZE: egui::Vec2 = egui::vec2(1440.0, 900.0);

/// Werckmeister III and Vallotti from C, cents (§5.2).
const WERCK3: [f64; 12] = [
    0.0, 90.225, 192.180, 294.135, 390.225, 498.045, 588.270, 696.090, 792.180, 888.270, 996.090,
    1092.180,
];
const VALLOTTI: [f64; 12] = [
    0.0, 94.135, 196.090, 298.045, 392.180, 501.955, 592.180, 698.045, 796.090, 894.135, 1000.0,
    1090.225,
];

/// A row from C rotated to `key` (§5.4), A at `a4`: semitones from 12-TET at A440 per key.
fn table(row: &[f64; 12], key: u8, a4: f64) -> TuningTable {
    let k = usize::from(key % 12);
    let dev = |p: usize| {
        let r = row[(p + 12 - k) % 12] + 100.0 * k as f64 - if k > p { 1200.0 } else { 0.0 };
        r - 100.0 * p as f64
    };
    let shift = 12.0 * cantus_dsp::math::log2(a4 / 440.0);
    let a = dev(9);
    TuningTable(std::array::from_fn(|n| {
        ((dev(n % 12) - a) / 100.0 + shift) as f32
    }))
}

fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../research/tuning-library-tests")
}

fn set_up(app: &mut CantusApp, kind: &str) -> Option<Target> {
    let temper = |id, key, follow, a4| Tuning {
        a4_hz: a4,
        system: TuningSystem::Temperament {
            id,
            key,
            follow_song_key: follow,
        },
        map: None,
    };
    let werck = temper(Temperament::WerckmeisterIii, 3, false, 415.0);
    match kind {
        "equal" => Some(Target::Project),
        "werck" | "inspector" | "chorda" => {
            tuning::design_preview(Some(table(&WERCK3, 3, 415.0)));
            tuning::set(app, Target::Project, werck);
            (kind == "werck").then_some(Target::Project)
        }
        "follow" => {
            app.exec(Command::SetProjectKey {
                key: Some(Key::new(2, ScaleKind::Minor)),
            });
            tuning::design_preview(Some(table(&VALLOTTI, 2, 440.0)));
            tuning::set(
                app,
                Target::Project,
                temper(Temperament::Vallotti, 0, true, 440.0),
            );
            Some(Target::Project)
        }
        "scale" => {
            let scl = tests_dir().join("31edo.scl");
            let said = tuning::io::import(app, Target::Project, &scl, None);
            app.tuning.last = Some(said);
            Some(Target::Project)
        }
        "track" => {
            let id = app.project().tracks[0].id;
            app.exec(Command::SetTrackTuning {
                track: id,
                tuning: TrackTuning::Own {
                    tuning: Tuning {
                        a4_hz: 442.0,
                        ..Tuning::default()
                    },
                },
            });
            Some(Target::Track(id))
        }
        _ => None,
    }
}

fn shoot(dir: &Path, kind: &str, dark: bool, zoom: f32) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-tuning");
    let _ = std::fs::create_dir_all(&tmp);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let mode = if dark { Mode::Dark } else { Mode::Light };
    app.appearance.mode = mode;
    let base = if dark {
        "preset:theme.slate_lapis_night"
    } else {
        "preset:theme.slate_lapis_paper"
    };
    app.appearance.assign(mode, base.into());
    app.attach(&ctx);
    tuning::design_preview(None);
    let target = set_up(&mut app, kind);
    let mut maximise = None;
    if kind == "chorda" {
        app.demo_editor("chorda");
        app.inst.view = cantus_ui::views::inst::View::Edit;
        let track = app.selected_track.unwrap_or(app.project().tracks[0].id);
        maximise = app.instrument_window_key(track);
    } else if kind == "inspector" {
        let id = app.project().tracks[0].id;
        app.select_track(id);
    }
    if let Some(t) = target {
        tuning::open(&mut app, t);
    }
    let theme = if dark { "night" } else { "paper" };
    let lang = cantus_ui::i18n::locale().code();
    let path = dir.join(format!(
        "tuning-{kind}-{theme}-{}-{lang}.png",
        (zoom * 100.0).round() as u32
    ));
    let mut frame = 0;
    raster::shoot_at(
        &ctx,
        SIZE,
        zoom,
        12,
        Some(&path),
        |_| None,
        |root| {
            if frame == 6 {
                if let Some(key) = maximise {
                    device::toggle_maximise(root.ctx(), key);
                }
            }
            app.frame(root);
            frame += 1;
        },
    );
    tuning::design_preview(None);
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-tuning"));
    let rest: Vec<String> = args.collect();
    let zooms: Vec<f32> = rest
        .iter()
        .find_map(|a| a.strip_prefix("--zoom="))
        .map(|z| z.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| vec![1.0, 2.0]);
    let themes: Vec<bool> = match rest.iter().find_map(|a| a.strip_prefix("--theme=")) {
        Some("night") => vec![true],
        Some("paper") => vec![false],
        _ => vec![true, false],
    };
    cantus_ui::i18n::set_locale(match rest.iter().find_map(|a| a.strip_prefix("--lang=")) {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    });
    let only: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::views::inst::chorda::scene::set_blocking(true);
    for kind in KINDS {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == kind) {
            continue;
        }
        for &dark in &themes {
            for &zoom in &zooms {
                shoot(&dir, kind, dark, zoom);
            }
        }
    }
}
