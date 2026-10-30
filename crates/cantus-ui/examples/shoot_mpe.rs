//! Shots of per-note expression in the piano roll (round 33, ADR 0080): a played MPE phrase
//! — notes sliding in, vibratos, pressure swells, timbre opening — with its curves on the
//! notes and the three expression rows under the roll, in Cantus Night and Paper, English
//! and Spanish, at 100 and 200 %: `cargo run -p cantus-ui --example shoot_mpe -- <dir>`.

use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::CantusApp;
use egui::vec2;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for what in ["roll", "selected", "dense"] {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                for scale in [1.0f32, 2.0] {
                    let name = format!(
                        "mpe-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    shoot(&path, what, theme, scale);
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

fn st(x: f64) -> i64 {
    ExprKind::Bend.store(x)
}

fn u(x: f64) -> i64 {
    ExprKind::Pressure.store(x)
}

/// A phrase as an MPE controller plays it; `dense` a long take of fast notes, each curve
/// hundreds of points (the frame budget's case).
pub fn project(dense: bool) -> Project {
    let mut p = Project::new("MPE");
    p.tempo_bpm = 96.0;
    let mut t = p.make_track("Seaboard");
    t.instrument = InstrumentSpec::Native(cantus_core::model::note_expr::defaults::with_routes(
        NativeSpec::new(InstrumentKind::Wavetable),
    ));
    let mut c = p.make_clip(
        "Phrase",
        Ticks::ZERO,
        Ticks::beats(if dense { 32 } else { 8 }),
    );
    let phrase: &[(f64, f64, u8, f64)] = &[
        (0.0, 1.5, 62, -2.0),
        (1.5, 0.5, 65, 0.0),
        (2.0, 2.0, 69, 0.0),
        (4.0, 1.0, 67, 1.0),
        (5.0, 3.0, 62, 0.0),
        (2.5, 1.5, 57, 0.0),
    ];
    let reps = if dense { 16 } else { 1 };
    for r in 0..reps {
        for &(at, len, key, from) in phrase {
            let start = Ticks::from_beats_f64(at + r as f64 * 2.0);
            let length = Ticks::from_beats_f64(if dense { len.min(1.0) } else { len });
            let mut n = p.make_note(start, length, key, 96);
            let l = length.0;
            let step = if dense { 6 } else { 24 };
            let mut bend = vec![ExprPoint(0, st(from)), ExprPoint(l / 6, 0)];
            bend.extend((1..(l / step)).filter(|i| i * step > l / 3).map(|i| {
                let x = (i * step) as f64 / 120.0;
                ExprPoint(i * step, st(0.35 * cantus_dsp::math::sin(x * 6.0)))
            }));
            n.expr = NoteExpr {
                bend,
                pressure: vec![
                    ExprPoint(0, u(0.15)),
                    ExprPoint(l / 2, u(0.95)),
                    ExprPoint(l, u(0.4)),
                ],
                timbre: vec![ExprPoint(0, u(0.2)), ExprPoint(l, u(0.85))],
            }
            .normalised()
            .boxed();
            c.notes.push(n);
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn shoot(path: &std::path::Path, what: &str, theme: &str, scale: f32) {
    use cantus_ui::kit::appearance::Mode;
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let th = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme_family::cantus_night()
    };
    cantus_ui::theme::apply(&th, &ctx);
    let mut app =
        CantusApp::from_parts(Some(project(what == "dense")), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-mpe");
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
    app.attach(&ctx);
    let t = app.project().tracks[0].id;
    let c = app.project().tracks[0].clips[0].id;
    app.select_clip(t, c);
    app.focus_piano_roll();
    if what == "selected" {
        app.selected_notes = app.project().tracks[0].clips[0].notes[2..4]
            .iter()
            .map(|n| n.id)
            .collect();
    }
    app.piano_roll.lanes.expr = [true, true, true];
    app.piano_roll.vel_h = 300.0;
    let mut frame = 0;
    cantus_ui::raster::shoot(&ctx, vec2(1200.0, 720.0), scale, 6, Some(path), |root| {
        frame += 1;
        if frame == 2 {
            cantus_ui::views::piano_roll::fit_clip(&mut app, t, c);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| cantus_ui::views::piano_roll::show(&mut app, ui));
    });
}
