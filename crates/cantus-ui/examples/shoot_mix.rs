//! The desk's VCAs and snapshots (round 36, ADR 0093), screenshotted without a screen:
//! `cargo run --release -p cantus-ui --example shoot_mix -- <out-dir> [filter…]`.
//!
//! Writes `mix-<theme>-<variant>.png` for Slate and Paper at 1280 × 720: `desk` (two VCAs
//! on the desk, the drums and the keys following, a muted strip), `desk-xl` (Extra large),
//! `desk-es` (Spanish), `snap-menu` (the Snapshots menu, a cue listed) and `strip-menu` (a
//! strip's menu with the VCA rows).

use cantus_core::mix_cmd::MixCommand;
use cantus_core::mix_snap::{capture, MixCue, MixCueId, MixSnapId, MixSnapshot};
use cantus_core::vca::StripKind;
use cantus_core::{Color, Command, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

/// The welcome project with named, coloured, balanced strips and two VCAs.
fn project() -> Project {
    let mut p = welcome_project();
    while p.strips.len() < 12 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
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
    p.strips[2].muted = true;
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
    // A snapshot and a cue at bar 3.
    let state = capture(&p, &[], true);
    let sid = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: sid,
        name: "Verse".into(),
        state: state.clone(),
    });
    let chorus = MixSnapId(p.alloc_id());
    p.mix.snapshots.push(MixSnapshot {
        id: chorus,
        name: "Chorus".into(),
        state,
    });
    let cue = MixCueId(p.alloc_id());
    p.mix.cues.push(MixCue {
        id: cue,
        at: Ticks::beats(8),
        snapshot: chorus,
        scope: Vec::new(),
    });
    p
}

fn main() {
    use cantus_ui::i18n::Locale::{En, Es};
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let variants = [
        ("desk", En, None),
        ("desk-xl", En, Some(2u8)),
        ("desk-es", Es, None),
        ("snap-menu", En, None),
        ("snap-menu-es", Es, None),
        ("strip-menu", En, None),
    ];
    for (tn, theme) in &themes {
        for (v, locale, mode) in variants {
            let name = format!("mix-{tn}-{v}");
            if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                continue;
            }
            shoot_one(&dir, &name, theme, v, locale, mode);
        }
    }
}

fn shoot_one(
    dir: &std::path::Path,
    name: &str,
    theme: &cantus_ui::theme::Theme,
    variant: &str,
    locale: cantus_ui::i18n::Locale,
    mode: Option<u8>,
) {
    cantus_ui::i18n::set_locale(locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(theme, &ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(project()), engine, false);
    {
        use cantus_ui::kit::appearance::Mode;
        let tmp = std::env::temp_dir().join("cantus-shoot-mix");
        app.settings_path = tmp.join("settings.json");
        app.layouts_path = tmp.join("layouts.json");
        app.keymap_path = tmp.join("keymap.toml");
        app.appearance_path = tmp.join("appearance.json");
        let (preset, m) = if name.contains("paper") {
            ("paper", Mode::Light)
        } else {
            ("slate", Mode::Dark)
        };
        app.appearance.mode = m;
        app.appearance.assign(m, format!("preset:{preset}"));
    }
    app.central = cantus_ui::app::CentralView::Mixer;
    if let Some(m) = mode {
        cantus_ui::views::mixer::set_width_mode(
            &mut app,
            cantus_ui::views::mixer::layout::WidthMode::from_index(m as usize),
        );
    }
    let kick = app.project().strips[0].id;
    app.select_strip(kick);
    let (w, h) = (1280.0f32, 720.0f32);
    let at = egui::pos2(300.0, 60.0);
    if variant.starts_with("snap-menu") {
        cantus_ui::views::mixer::open_snapshots(&mut app, at);
    } else if variant == "strip-menu" {
        cantus_ui::views::mixer::open_menu(&mut app, Some(kick), egui::pos2(w * 0.1, h * 0.55));
    }
    // A recall before the picture, so the history says it (and the desk is as the snapshot).
    let id = app.project().mix.snapshots[0].id;
    app.exec(Command::Mix(MixCommand::Recall {
        id,
        scope: Vec::new(),
        master: true,
        include_safe: false,
    }));
    app.sync_engine();
    let mut audio = vec![0f32; 800 * 2];
    let path = dir.join(format!("{name}.png"));
    let size = egui::vec2(w, h);
    shoot(&ctx, size, 1.0, 12, Some(&path), |root| {
        processor.process(&mut audio, 2);
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::ui(&mut app, root, &mut frame);
    });
    println!("wrote {}", path.display());
}
