//! r39:el-arrange — the whole window with the arrangement in Slate, rendered headless:
//! the demo song with sections, a marker, a folder of its drums, the selected track lit;
//! a closed folder; the empty song — in Slate Night and Paper, at 1280 × 720 (100 %) and
//! 1920 × 1080 (125 %).
//!
//! `cargo run -p cantus-ui --example shoot_arrange -- <out_dir> [case…]`

use cantus_core::arrange::{FolderId, Section, SectionId, TrackFolder};
use cantus_core::{Marker, MarkerId, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::{raster, CantusApp};
use std::path::{Path, PathBuf};

fn bar(n: i64) -> Ticks {
    Ticks(Ticks::beats(4).0 * n)
}

/// The demo song with its structure: three sections, a marker, the drums in a folder.
fn song(open: bool) -> Project {
    let mut p = cantus_ui::demos::startup_project();
    let add = |p: &mut Project, from: i64, len: i64, name: &str| {
        let id = SectionId(p.alloc_id());
        p.sections.push(Section {
            id,
            start: bar(from),
            length: bar(len),
            name: name.into(),
            color: None,
        });
    };
    add(&mut p, 0, 4, "Intro");
    add(&mut p, 4, 8, "Drop");
    add(&mut p, 12, 4, "Outro");
    let id = MarkerId(p.alloc_id());
    p.markers.push(Marker {
        id,
        tick: bar(6),
        name: "Vox in".into(),
        color: None,
    });
    let f = FolderId(p.alloc_id());
    let color = p
        .tracks
        .first()
        .map_or(cantus_core::Color::PALETTE[0], |t| t.color);
    p.folders.push(TrackFolder {
        id: f,
        name: "Drums".into(),
        color,
        open,
    });
    for t in p.tracks.iter_mut().take(4) {
        t.folder = Some(f);
    }
    if let Some(t) = p.tracks.get_mut(7) {
        t.muted = true;
    }
    if let Some(t) = p.tracks.get_mut(5) {
        t.glyph = Some("star".into());
    }
    p
}

struct Case {
    name: &'static str,
    preset: &'static str,
    dark: bool,
    size: (f32, f32),
    zoom: f32,
    project: fn() -> Project,
}

fn open_song() -> Project {
    song(true)
}
fn closed_song() -> Project {
    song(false)
}
fn empty() -> Project {
    Project::new("New song")
}

/// The details: tall rows, two automation lanes of one track in their own modes, an armed
/// audio track, a frozen-looking one, the drums' folder closed.
fn detail() -> Project {
    use cantus_core::{AutoMode, AutoPoint, AutoTarget, LaneMode};
    let mut p = song(false);
    let pt = |beats: i64, v: f32| AutoPoint::new(Ticks::beats(beats), v);
    let chords = p.tracks[5].id;
    let strip = p.tracks[5].strip.expect("a strip");
    let a = p.make_automation_clip(
        "Chords · Volume",
        AutoTarget::StripGain(strip),
        Ticks::ZERO,
        Ticks::beats(64),
        vec![pt(0, 0.4), pt(16, 0.8), pt(32, 0.6), pt(48, 0.75)],
    );
    let b = p.make_automation_clip(
        "Chords · Pan",
        AutoTarget::TrackPan(chords),
        Ticks::ZERO,
        Ticks::beats(64),
        vec![pt(0, 0.5), pt(24, 0.3), pt(40, 0.7)],
    );
    let t = &mut p.tracks[5];
    t.clips.push(a);
    t.clips.push(b);
    t.automation_open = true;
    t.lane_modes.push(LaneMode {
        target: AutoTarget::TrackPan(chords),
        mode: AutoMode::Touch,
    });
    p.tracks[4].height = 64.0 / 44.0;
    p.tracks[6].height = 96.0 / 44.0;
    if let Some(t) = p.tracks.get_mut(8) {
        t.input = Some(cantus_core::audio::TrackInput::mono(0));
        t.armed = true;
    }
    p
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for (name, preset, dark) in [
        ("night", "slate_night", true),
        ("paper", "slate_paper", false),
    ] {
        for (tag, size, zoom) in [
            ("100", (1280.0, 720.0), 1.0),
            ("125", (1920.0, 1080.0), 1.25),
        ] {
            let _ = tag;
            v.push(Case {
                name: Box::leak(format!("song-{name}-{tag}").into_boxed_str()),
                preset,
                dark,
                size,
                zoom,
                project: open_song,
            });
            v.push(Case {
                name: Box::leak(format!("closed-{name}-{tag}").into_boxed_str()),
                preset,
                dark,
                size,
                zoom,
                project: closed_song,
            });
            v.push(Case {
                name: Box::leak(format!("detail-{name}-{tag}").into_boxed_str()),
                preset,
                dark,
                size,
                zoom,
                project: detail,
            });
            v.push(Case {
                name: Box::leak(format!("empty-{name}-{tag}").into_boxed_str()),
                preset,
                dark,
                size,
                zoom,
                project: empty,
            });
        }
    }
    v
}

fn shoot(dir: &Path, case: &Case) {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let tmp = std::env::temp_dir().join("cantus-shoot-arrange");
    let mut app = CantusApp::from_parts(Some((case.project)()), EngineHandle::dummy(), false);
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if case.dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance
        .assign(mode, format!("preset:{}", case.preset));
    app.attach(&ctx);
    // Chords selected, as after a click on its header.
    if let Some(t) = app.project().tracks.get(5).map(|t| t.id) {
        app.select_track(t);
    }
    // The details: a wide header column, the mixer hidden for room.
    if case.name.starts_with("detail") {
        app.arrangement.header_width = 280.0;
        app.workspace.hide(cantus_ui::views::workspace::Pane::Mixer);
    }
    let (w, h) = case.size;
    let points = egui::vec2(w / case.zoom, h / case.zoom);
    let path = dir.join(format!(
        "window-{}-{}x{}.png",
        case.name, w as u32, h as u32
    ));
    raster::shoot(&ctx, points, case.zoom, 8, Some(&path), |root| {
        app.frame(root)
    });
    println!("{}", path.display());
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-arrange"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for case in cases() {
        if only.is_empty() || only.iter().any(|o| case.name.starts_with(o.as_str())) {
            shoot(&dir, &case);
        }
    }
}
