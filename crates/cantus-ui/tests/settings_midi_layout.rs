//! Settings ▸ MIDI's inputs table reads whole (round 33's review: merged, its Grid column
//! ran past the page and was clipped, the MPE cell showed a third of itself, and at 200 %
//! neither could be reached). Every column sits inside the page at the window's default
//! size, at 100 % and at 200 %; in a window too narrow for it the table scrolls sideways
//! and every column is still there to scroll to.

use cantus_engine::EngineHandle;
use cantus_ui::views::workspace_settings::{demo_midi, midi_inputs_layout, InputsLayout};
use cantus_ui::{welcome_project, CantusApp};
use egui::{vec2, Pos2, Rect, Vec2};

const PORTS: [&str; 4] = [
    "Keystation 49",
    "MIDIIN2 (LPMiniMK3 MIDI)",
    "Seaboard RISE 2",
    "Launch Control XL",
];

/// The settings window open on MIDI with four ports (MPE marked on one, a Launchpad on
/// another), on a screen of `screen` pixels at UI scale `zoom`: the table's layout and the
/// texts drawn.
fn settings_midi(screen: Vec2, zoom: f32) -> (InputsLayout, Vec<String>) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::views::workspace_style::install(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
    ctx.set_zoom_factor(zoom);
    let (engine, _processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(welcome_project()), engine, false);
    app.settings.ui_scale = zoom;
    app.settings.mpe_ports = vec!["Seaboard RISE 2".into()];
    let mut texts = Vec::new();
    for frame in 0..12 {
        if frame == 4 {
            demo_midi(&mut app, &PORTS, &["Drum Machine"]);
        }
        let input = egui::RawInput {
            // The screen in points: its pixels over the UI scale.
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen / zoom)),
            time: Some(frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            let mut f = eframe::Frame::_new_kittest();
            eframe::App::ui(&mut app, root, &mut f);
        });
        out.textures_delta.clear();
        texts.clear();
        for c in &out.shapes {
            collect(&c.shape, c.clip_rect, &mut texts);
        }
    }
    let layout = midi_inputs_layout(&ctx).expect("the inputs table was drawn");
    (layout, texts)
}

fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(t) if clip.intersects(t.visual_bounding_rect()) => {
            out.push(t.galley.text().to_string());
        }
        egui::Shape::Vec(v) => {
            for s in v {
                collect(s, clip, out);
            }
        }
        _ => {}
    }
}

/// Seven columns, each heading over its cell, side by side without overlapping.
fn columns_read(l: &InputsLayout) {
    assert_eq!(
        l.heads.len(),
        7,
        "port, notes, controls, clock, MTC, MPE, grid"
    );
    assert_eq!(l.cells.len(), 7);
    for (i, (h, c)) in l.heads.iter().zip(&l.cells).enumerate() {
        assert!(
            (h.center().x - c.center().x).abs() < 0.5 && (h.width() - c.width()).abs() < 0.5,
            "column {i}: heading {h:?} over its cell {c:?}"
        );
        if i > 0 {
            assert!(
                c.left() >= l.cells[i - 1].right() - 0.01,
                "column {i} overlaps the one before"
            );
        }
    }
}

#[test]
fn the_inputs_table_fits_the_default_window_at_100_and_200_percent() {
    for zoom in [1.0, 2.0] {
        let (l, texts) = settings_midi(vec2(1920.0, 1080.0), zoom);
        columns_read(&l);
        for (i, c) in l.cells.iter().enumerate() {
            assert!(
                l.viewport.expand(0.5).contains_rect(*c),
                "at {zoom}×, column {i} {c:?} lies inside the page {:?}",
                l.viewport
            );
        }
        assert!(
            l.content.width() <= l.viewport.width() + 0.5,
            "at {zoom}×, nothing to scroll to"
        );
        // The Launchpad's name whole in its cell, not elided.
        assert!(
            texts.iter().any(|t| t == "Launchpad Mini MK3"),
            "at {zoom}×, the grid cell reads its device whole"
        );
    }
}

#[test]
fn a_narrow_window_scrolls_the_table_to_every_column() {
    // A 1366 × 768 laptop at 200 %: the window is narrower than the table.
    let (l, _) = settings_midi(vec2(1366.0, 768.0), 2.0);
    columns_read(&l);
    assert!(
        l.content.width() > l.viewport.width() + 1.0,
        "the table is wider than its page: it scrolls"
    );
    for (i, c) in l.cells.iter().chain(&l.heads).enumerate() {
        assert!(
            l.content.expand(0.5).contains_rect(*c),
            "column {i} {c:?} is within what the scroll reaches {:?}",
            l.content
        );
    }
}
