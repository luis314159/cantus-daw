//! Round 37 (the integrator: "edit views overflow instead of scrolling"): at a 1500 × 940
//! editor, every instrument's Edit view keeps each section at its own height and
//! scrolls — every label the view paints when it has the room (an editor 1800 tall) is seen
//! whole at the top or at the bottom of its scroll, never cut by the bands under it.

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
use cantus_engine::EngineHandle;
use cantus_ui::views::inst::View;
use cantus_ui::{welcome_project, CantusApp};
use egui::{Event, Pos2, Rect};

/// One frame of the editor alone at `size`; every text painted with the part of it seen.
fn frame(
    ctx: &egui::Context,
    a: &mut CantusApp,
    size: egui::Vec2,
    events: Vec<Event>,
    n: u64,
) -> Vec<(String, Rect, Rect)> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        time: Some(n as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        cantus_ui::views::inst::window_in_sized(a, ui, size);
    });
    out.textures_delta.clear();
    let mut v = Vec::new();
    for c in &out.shapes {
        if let egui::Shape::Text(t) = &c.shape {
            let r = t.galley.rect.translate(t.pos.to_vec2());
            v.push((t.galley.text().to_string(), r, r.intersect(c.clip_rect)));
        }
    }
    v
}

fn seen_whole(texts: &[(String, Rect, Rect)]) -> std::collections::HashSet<String> {
    texts
        .iter()
        .filter(|(s, r, seen)| {
            !s.trim().is_empty()
                && (seen.width() - r.width()).abs() < 1.0
                && (seen.height() - r.height()).abs() < 1.0
        })
        .map(|(s, _, _)| s.clone())
        .collect()
}

#[test]
fn every_instruments_views_scroll_rather_than_crush_a_section() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut bad = Vec::new();
    let mut n = 0u64;
    for kind in InstrumentKind::ALL {
        if kind == InstrumentKind::Dancer {
            continue; // the stage, not a body of sections
        }
        let mut a = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        a.open_inst_editor(track);
        for view in [View::Edit] {
            a.inst.view = view;
            let tall = egui::vec2(1500.0, 1800.0);
            let mut texts = Vec::new();
            for _ in 0..3 {
                n += 1;
                texts = frame(&ctx, &mut a, tall, vec![], n);
            }
            let want = seen_whole(&texts);
            let short = egui::vec2(1500.0, 940.0);
            let mut got = std::collections::HashSet::new();
            for _ in 0..3 {
                n += 1;
                got.extend(seen_whole(&frame(&ctx, &mut a, short, vec![], n)));
            }
            // To the bottom of the view's scroll: the pointer over the body, the wheel down.
            let over = Pos2::new(750.0, 300.0);
            for _ in 0..12 {
                n += 1;
                let ev = vec![
                    Event::PointerMoved(over),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -400.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
                got.extend(seen_whole(&frame(&ctx, &mut a, short, ev, n)));
            }
            let missing: Vec<&String> = want.iter().filter(|w| !got.contains(*w)).collect();
            if !missing.is_empty() {
                bad.push(format!("{kind:?} {view:?}: never seen whole: {missing:?}"));
            }
            // Back to the top for the next view.
            for _ in 0..6 {
                n += 1;
                let ev = vec![
                    Event::PointerMoved(over),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 4000.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
                frame(&ctx, &mut a, short, ev, n);
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
