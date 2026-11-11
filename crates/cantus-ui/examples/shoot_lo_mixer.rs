//! Round 39's close-out of the mixer and its inspector (`lo-mixer`), the **whole window**
//! headless on the demo song (Neon Tide), playing, at 1920 × 1080 and 125 %, in Slate Night
//! and Slate Paper: the mixer maximised in Compact, Wide and Large with the tracks wearing
//! their own icons and Chords' strip EQ shaped (the Large band dots on real data), and the
//! inspector on a track's strip, a bus and the master — its sections folded as they start,
//! and every one of them open (`-open`), Chords carrying des-mixer's three inserts (a keyed
//! compressor with its mini view open, an EQ, a bypassed drive).
//!
//! `cargo run --release -p cantus-ui --example shoot_lo_mixer -- <out-dir> [filter…]`
//!
//! Writes `lomix-<case>-<theme>.png` (the `-open` cases 1920 × 2700).

use cantus_core::{Command, EffectKind, EqField, StripId};
use cantus_engine::EngineHandle;
use cantus_ui::app::CentralView;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::workspace::Pane;
use cantus_ui::{raster, CantusApp};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, PartialEq)]
enum Pick {
    /// Chords' strip, chosen on the desk.
    Chords,
    /// Chords' track, chosen in the arrangement (the inspector's track sheet).
    ChordsTrack,
    Bus,
    Master,
}

#[derive(Copy, Clone)]
struct Case {
    name: &'static str,
    mode: WidthMode,
    pick: Pick,
    /// Every folded section opened, the compressor's mini view open.
    open: bool,
    /// The arrangement folded: the mixer takes the window.
    maximise: bool,
    /// The first insert row lifted and dragged below the second (its drop line showing).
    drag: bool,
    /// Chords' strip frozen first (its render made, as Freeze makes it).
    frozen: bool,
}

const UNFOLD: [&str; 6] = [
    "kit.inspector.routing",
    "inspector.sidechain",
    "rec.recording",
    "inspector.automation",
    "inspector.level",
    "freeze.section",
];

fn strip_named(app: &CantusApp, name: &str) -> Option<StripId> {
    let p = app.project();
    p.strips
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .or_else(|| {
            p.tracks
                .iter()
                .find(|t| t.name == name)
                .and_then(|t| t.strip)
        })
}

/// A few tracks wear their own icons, as a user would set them.
fn icons(app: &mut CantusApp) {
    let picks = [
        ("Kick", "kick"),
        ("Clap", "clap"),
        ("Hats", "hihat"),
        ("Percussion", "congas"),
        ("Bass", "bass"),
        ("Chords", "piano"),
        ("Pluck", "harp"),
        ("Lead", "lead"),
        ("Choir", "choir"),
    ];
    for (name, g) in picks {
        let id = app
            .project()
            .tracks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id);
        if let Some(id) = id {
            cantus_ui::views::track_glyphs::set(app, id, Some(g));
        }
    }
}

/// Chords as des-mixer drew it: a compressor keyed from the kick, an EQ, a bypassed drive;
/// its strip EQ a low shelf up, a bell down, a high shelf up.
fn chords(app: &mut CantusApp) -> Option<StripId> {
    let chords = strip_named(app, "Chords")?;
    let kick = strip_named(app, "Kick");
    if app
        .project()
        .strip(chords)
        .is_some_and(|s| s.effects.is_empty())
    {
        for kind in [
            EffectKind::Compressor,
            EffectKind::Eq,
            EffectKind::Saturator,
        ] {
            app.add_effect(chords, kind);
        }
        let fx: Vec<_> = app
            .project()
            .strip(chords)
            .map(|s| s.effects.iter().map(|e| e.id).collect())
            .unwrap_or_default();
        if let (Some(&comp), Some(kick)) = (fx.first(), kick) {
            app.exec(Command::SetEffectSidechain {
                strip: chords,
                effect: comp,
                source: Some(kick),
            });
        }
        if let Some(&drive) = fx.get(2) {
            app.exec(Command::SetEffectBypass {
                strip: chords,
                effect: drive,
                bypass: true,
            });
        }
    }
    for (band, gain) in [(0u8, 2.0f32), (1, -2.5), (2, 1.6)] {
        app.exec(Command::SetStripEq {
            id: chords,
            band,
            field: EqField::Gain,
            value: gain,
        });
    }
    Some(chords)
}

fn shoot(dir: &Path, case: Case, preset: &str, dark: bool) {
    // Every section open runs past a 1080-high window: those are shot 2700 high, so the
    // whole inspector is seen at once.
    let (w, h, zoom) = (
        1920.0f32,
        if case.open { 2700.0f32 } else { 1080.0 },
        1.25f32,
    );
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::demos::startup_project()), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-lo-mixer");
    let _ = std::fs::remove_dir_all(&tmp);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.attach(&ctx);
    app.show_central(CentralView::Mixer);
    cantus_ui::views::mixer::set_width_mode(&mut app, case.mode);
    icons(&mut app);
    let chords = chords(&mut app);
    if let (true, Some(id)) = (case.frozen, chords) {
        app.engine.frozen.set_app_dir(tmp.join("renders"));
        app.freeze_target(cantus_core::freeze::FreezeTarget::Strip(id));
        let t0 = std::time::Instant::now();
        loop {
            app.sync_engine();
            cantus_ui::freeze::tick(&mut app, &ctx);
            if !app.freeze.busy() || t0.elapsed().as_secs() > 120 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        cantus_ui::freeze::tick(&mut app, &ctx);
    }
    match case.pick {
        Pick::Chords => {
            if let Some(id) = chords {
                app.select_strip(id);
            }
        }
        Pick::ChordsTrack => {
            let t = app
                .project()
                .tracks
                .iter()
                .find(|t| t.name == "Chords")
                .map(|t| t.id);
            if let Some(t) = t {
                app.select_track(t);
            }
        }
        Pick::Bus => {
            if let Some(id) = strip_named(&app, "Drum bus") {
                app.select_strip(id);
            }
        }
        Pick::Master => app.select_master(),
    }
    if case.open {
        for key in UNFOLD {
            cantus_ui::views::inspector::unfold(&ctx, key);
        }
        let comp = chords
            .and_then(|c| app.project().strip(c))
            .and_then(|s| s.effects.first())
            .map(|e| e.id);
        if let (Some(e), Pick::Chords | Pick::ChordsTrack) = (comp, case.pick) {
            app.inspector.toggle_mini(e);
        }
    }
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    let points = egui::vec2(w / zoom, h / zoom);
    let path = dir.join(format!("lomix-{}-{preset}.png", case.name));
    let mut frames = 0;
    let first = chords
        .and_then(|c| app.project().strip(c))
        .and_then(|s| s.effects.first())
        .map(|e| e.id);
    let drag = case.drag;
    let events = move |frame: usize, ctx: &egui::Context| -> Vec<egui::Event> {
        let (true, Some(e)) = (drag, first) else {
            return Vec::new();
        };
        let Some(row) = ctx.read_response(egui::Id::new(("insp-tile", e))) else {
            return Vec::new();
        };
        let at = row.rect.center();
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        match frame {
            20 => vec![egui::Event::PointerMoved(at)],
            21 => vec![press(true)],
            22..=29 => vec![egui::Event::PointerMoved(
                at + egui::vec2(0.0, 7.0 * (frame - 21) as f32),
            )],
            _ => Vec::new(),
        }
    };
    raster::shoot_events(&ctx, points, zoom, 30, Some(&path), events, |root| {
        frames += 1;
        if frames == 2 && case.maximise {
            app.workspace.toggle_fold(Pane::Arrangement);
        }
        for _ in 0..3 {
            processor.process(&mut audio, 2);
        }
        app.frame(root);
    });
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-lo-mixer"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let base = Case {
        name: "wide",
        mode: WidthMode::Wide,
        pick: Pick::Chords,
        open: false,
        maximise: true,
        drag: false,
        frozen: false,
    };
    let cases = [
        base,
        Case {
            name: "compact",
            mode: WidthMode::Compact,
            ..base
        },
        Case {
            name: "large",
            mode: WidthMode::Xl,
            ..base
        },
        Case {
            name: "docked",
            maximise: false,
            ..base
        },
        Case {
            name: "insp-strip-open",
            open: true,
            ..base
        },
        Case {
            name: "insp-track",
            pick: Pick::ChordsTrack,
            maximise: false,
            ..base
        },
        Case {
            name: "insp-track-open",
            pick: Pick::ChordsTrack,
            open: true,
            maximise: false,
            ..base
        },
        Case {
            name: "insp-drag",
            drag: true,
            ..base
        },
        Case {
            name: "insp-frozen-open",
            open: true,
            frozen: true,
            ..base
        },
        Case {
            name: "insp-bus",
            pick: Pick::Bus,
            ..base
        },
        Case {
            name: "insp-bus-open",
            pick: Pick::Bus,
            open: true,
            ..base
        },
        Case {
            name: "insp-master",
            pick: Pick::Master,
            ..base
        },
        Case {
            name: "insp-master-open",
            pick: Pick::Master,
            open: true,
            ..base
        },
    ];
    for case in cases {
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            let tag = format!("{}-{preset}", case.name);
            if !only.is_empty() && !only.iter().any(|o| tag.contains(o.as_str())) {
                continue;
            }
            shoot(&dir, case, preset, dark);
        }
    }
}
