//! `shoot <dir> automodes`: round 31's automation modes and lanes (ADR 0068) — the piano
//! roll with its lanes under the notes (velocity, release velocity, a pitch bend, the mod
//! wheel rising, the sustain pedal in steps, a point selected), the arrangement's headers
//! with a track in each mode (armed, then not), and the top bar with the arm on and its
//! options open. Both themes; `CANTUS_SHOOT_LANG=es` for Spanish.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::{AutoMode, AutoPoint, Project, Shape, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::Path;

fn pt(beats: f64, v: f32) -> AutoPoint {
    AutoPoint::new(Ticks((beats * cantus_core::PPQ as f64) as i64), v)
}

/// The welcome song, its first clip given lanes and its notes release velocities, its
/// tracks in every mode.
pub fn project() -> Project {
    let mut p = cantus_ui::welcome_project();
    p.name = "Lanes".into();
    let modes = [
        AutoMode::Touch,
        AutoMode::Latch,
        AutoMode::Write,
        AutoMode::Read,
    ];
    for (t, m) in p.tracks.iter_mut().zip(modes.iter().cycle()) {
        t.auto_mode = *m;
    }
    // The bass synth, which reads the bend over its own range (12 semitones here).
    let mut bass = cantus_core::NativeSpec::new(cantus_core::InstrumentKind::Bass);
    let _ = bass.set("bend_range", 12.0);
    p.tracks[0].instrument = cantus_core::InstrumentSpec::Native(bass);
    let c = p.tracks[0]
        .clips
        .iter_mut()
        .find(|c| c.audio.is_none() && c.automation.is_none())
        .expect("a note clip");
    for (i, n) in c.notes.iter_mut().enumerate() {
        n.release = [30u8, 64, 96, 0, 110, 50][i % 6];
    }
    let mut up = pt(1.0, 0.82);
    up.tension = 0.45;
    let mut back = pt(2.0, 0.5);
    back.shape = Shape::SCurve;
    let mut steps = vec![
        pt(0.0, 0.0),
        pt(0.5, 1.0),
        pt(3.5, 0.0),
        pt(4.0, 1.0),
        pt(7.5, 0.0),
    ];
    steps.iter_mut().for_each(|s| s.shape = Shape::Hold);
    c.lanes = vec![
        ClipLane::new(
            LaneKind::PitchBend,
            vec![
                pt(0.0, 0.5),
                pt(0.5, 0.5),
                up,
                back,
                pt(4.0, 0.5),
                pt(5.0, 0.3),
                pt(6.0, 0.5),
            ],
        ),
        ClipLane::new(LaneKind::Cc(1), vec![pt(0.0, 0.05), pt(8.0, 0.9)]),
        ClipLane::new(LaneKind::Cc(64), steps),
    ];
    p
}

fn context(theme: &cantus_ui::theme::Theme) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(theme, &ctx);
    ctx
}

/// Write the shots into `out`.
pub fn shots(out: &Path, scale: f32) {
    let lang = cantus_ui::i18n::locale().code();
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    for (tn, theme) in &themes {
        let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
        let (t, c) = {
            let tr = &app.project().tracks[0];
            let c = tr
                .clips
                .iter()
                .find(|c| !c.lanes.is_empty())
                .expect("lanes");
            (tr.id, c.id)
        };
        app.select_clip(t, c);
        app.piano_roll.lanes.release = true;
        app.piano_roll.vel_h = 330.0;
        app.piano_roll
            .lanes
            .set_selected(LaneKind::PitchBend, vec![2]);
        app.set_auto_armed(true);
        // The piano roll with its lanes (a first frame fits the view to the clip).
        let ctx = context(theme);
        shoot(&ctx, egui::vec2(1300.0, 760.0), scale, 1, None, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| views::piano_roll::show(&mut app, ui));
        });
        let path = out.join(format!("automodes-pianoroll-{tn}-{lang}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(1300.0, 760.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::piano_roll::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());

        // The headers, armed and not.
        for armed in [true, false] {
            app.set_auto_armed(armed);
            app.arrangement.px_per_beat = 26.0;
            app.arrangement.track_height = 46.0;
            let path = out.join(format!(
                "automodes-headers-{}-{tn}-{lang}.png",
                if armed { "armed" } else { "off" }
            ));
            let ctx = context(theme);
            shoot(
                &ctx,
                egui::vec2(1100.0, 340.0),
                scale,
                4,
                Some(&path),
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| views::arrangement::show(&mut app, ui));
                },
            );
            println!("wrote {}", path.display());
        }

        // The top bar, armed, its options open.
        app.set_auto_armed(true);
        let path = out.join(format!("automodes-topbar-{tn}-{lang}.png"));
        let ctx = context(theme);
        let mut frame = 0;
        shoot(
            &ctx,
            egui::vec2(1600.0, 300.0),
            scale,
            6,
            Some(&path),
            |root| {
                frame += 1;
                if frame == 2 {
                    views::auto_modes::open_options(root.ctx());
                }
                egui::Panel::top(egui::Id::new("toolbar"))
                    .frame(egui::Frame::NONE)
                    .exact_size(views::transport::HEIGHT)
                    .show(root, |ui| views::transport::show(&mut app, ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |_| {});
            },
        );
        println!("wrote {}", path.display());
    }
}
