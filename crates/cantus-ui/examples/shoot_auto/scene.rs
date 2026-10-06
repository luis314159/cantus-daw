//! `shoot <dir> automation`: automation clips (ADR 0027) — the arrangement with lanes
//! under three tracks (a fader ride bent into the drop, a filter opened by an LFO, a
//! delay's mix in steps, the master's width breathing), the large editor on the ride with
//! points selected, and the mixer with its faders where the automation has them.

use cantus_core::automation::edit::{lfo, Lfo};
use cantus_core::{AutoPoint, AutoTarget, Color, EffectKind, FirmusParam, Project, Shape, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::Path;

fn pt(beats: f64, v: f32) -> AutoPoint {
    AutoPoint::new(Ticks((beats * cantus_core::PPQ as f64) as i64), v)
}

/// A song's worth of tracks with automation in lanes under them.
pub fn project() -> Project {
    let mut p = cantus_ui::welcome_project();
    p.name = "Automation".into();
    let lead = p.tracks[0].id;
    let lead_strip = p.tracks[0].strip.expect("strip");
    // The lead's fader rides up into bar 5, bent, holds, and dips at the end.
    let mut ride = vec![
        pt(0.0, 0.35),
        pt(14.0, 0.8),
        pt(20.0, 0.8),
        pt(24.0, 0.62),
        pt(28.0, 0.7),
    ];
    ride[0].tension = 0.55;
    ride[2].shape = Shape::SCurve;
    ride[2].tension = 0.4;
    let c = p.make_automation_clip(
        "Lead · Volume",
        AutoTarget::StripGain(lead_strip),
        Ticks::ZERO,
        Ticks::beats(32),
        ride,
    );
    p.tracks[0].clips.push(c);
    // Its cutoff opened by an LFO over two bars, looped.
    let cut = p.make_automation_clip(
        "Lead · Cutoff",
        AutoTarget::Firmus {
            track: lead,
            param: FirmusParam::Cutoff,
        },
        Ticks::beats(8),
        Ticks::beats(24),
        lfo(Lfo::Sine, Ticks::ZERO, Ticks::beats(8), 2, 0.25, 0.85, 0),
    );
    let mut cut = cut;
    cut.content_length = Some(Ticks::beats(8));
    p.tracks[0].clips.push(cut);
    // A delay on the second track's strip, its mix in steps.
    if p.tracks.len() > 1 {
        let s2 = p.tracks[1].strip.expect("strip");
        let mut delay = p.make_effect(EffectKind::Delay);
        delay.spec.set("mix", 0.2);
        let id = delay.id;
        p.strip_mut(s2).expect("strip").effects.push(delay);
        let mut steps = vec![
            pt(0.0, 0.1),
            pt(4.0, 0.45),
            pt(8.0, 0.2),
            pt(12.0, 0.7),
            pt(16.0, 0.3),
        ];
        steps.iter_mut().for_each(|x| x.shape = Shape::Hold);
        let c = p.make_automation_clip(
            "Delay mix",
            AutoTarget::Effect {
                effect: id,
                key: "mix".into(),
            },
            Ticks::beats(4),
            Ticks::beats(16),
            steps,
        );
        p.tracks[1].clips.push(c);
        let mut wide = p.make_automation_clip(
            "Master · Width",
            AutoTarget::MasterWidth,
            Ticks::beats(16),
            Ticks::beats(16),
            lfo(
                Lfo::Triangle,
                Ticks::ZERO,
                Ticks::beats(16),
                4,
                0.35,
                0.65,
                0,
            ),
        );
        wide.color = Some(Color::PALETTE[6]);
        p.tracks[1].clips.push(wide);
    }
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
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    for (tn, theme) in &themes {
        let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
        let ride = {
            let t = &app.project().tracks[0];
            let c = t
                .clips
                .iter()
                .find(|c| c.automation.is_some())
                .expect("ride");
            (t.id, c.id)
        };
        app.select_clip(ride.0, ride.1);
        app.arrangement.px_per_beat = 30.0;
        app.arrangement.track_height = 46.0;
        let path = out.join(format!("auto-arrangement-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(1500.0, 620.0),
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

        // The large editor, with the ride's middle points selected (after a first frame,
        // which fits the view to the clip and starts with nothing selected).
        let ctx = context(theme);
        shoot(&ctx, egui::vec2(1300.0, 460.0), scale, 1, None, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| views::piano_roll::show(&mut app, ui));
        });
        app.automation.editor.selected = vec![1, 2];
        let path = out.join(format!("auto-editor-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(1300.0, 460.0),
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
    }
}
