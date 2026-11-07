//! Every click target on a mixer strip is a real one (round 37, the owner: "los tres botones
//! … son muy chicos y es difícil darles click"): at least 24 × 24 pt in every width mode and
//! at every zoom from 75 % to 200 %, and no two of a strip's targets overlap.

use cantus_engine::EngineHandle;
use cantus_ui::views::mixer::{self, layout::WidthMode, Control};
use cantus_ui::{views, welcome_project, CantusApp};
use egui::{Pos2, Rect};

const FLOOR: f32 = 24.0;

fn frame(ctx: &egui::Context, app: &mut CantusApp, px: egui::Vec2, n: u64) {
    let z = ctx.zoom_factor();
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, px / z)),
        time: Some(n as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(app, ui));
    });
    out.textures_delta.clear();
}

#[test]
fn every_strip_target_is_at_least_24_pt_and_none_overlaps() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut p = welcome_project();
    while p.strips.len() < 6 {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    let strips: Vec<_> = p.strips.iter().map(|s| s.id).collect();
    // A strip with a send, and another selected: the sends' rows show their targets.
    p.strips[1].sends.push(cantus_core::Send {
        to: strips[3],
        level: 0.5,
        enabled: true,
        tap: Default::default(),
    });
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    app.select_strip(strips[2]);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut bad = Vec::new();
    let mut seen = 0usize;
    for mode in WidthMode::ALL {
        mixer::set_width_mode(&mut app, mode);
        for zoom in [0.75f32, 1.0, 1.25, 1.5, 2.0] {
            ctx.set_zoom_factor(zoom);
            // Tall enough that no row gives way.
            let px = egui::vec2(2400.0, 2400.0) * zoom;
            for n in 0..3 {
                frame(&ctx, &mut app, px, n);
            }
            for &id in &strips[1..5] {
                let mut controls = vec![
                    Control::Mute,
                    Control::Solo,
                    Control::Phase,
                    Control::Swap,
                    Control::Fx,
                    Control::Record,
                    Control::Send,
                ];
                if mode != WidthMode::Compact {
                    controls.push(Control::Mono);
                }
                let rects: Vec<(Control, Rect)> = controls
                    .iter()
                    .filter_map(|c| mixer::control_rect(&ctx, Some(id), *c).map(|r| (*c, r)))
                    .collect();
                for (c, r) in &rects {
                    seen += 1;
                    if r.width() + 1e-3 < FLOOR || r.height() + 1e-3 < FLOOR {
                        bad.push(format!(
                            "{mode:?} {zoom}: {c:?} is {:.1} × {:.1}",
                            r.width(),
                            r.height()
                        ));
                    }
                }
                for (i, (a, ra)) in rects.iter().enumerate() {
                    for (b, rb) in &rects[i + 1..] {
                        let o = ra.intersect(*rb);
                        if o.width() > 0.01 && o.height() > 0.01 {
                            bad.push(format!("{mode:?} {zoom}: {a:?} overlaps {b:?}"));
                        }
                    }
                }
            }
        }
    }
    ctx.set_zoom_factor(1.0);
    assert!(seen > 100, "the targets were found ({seen})");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
