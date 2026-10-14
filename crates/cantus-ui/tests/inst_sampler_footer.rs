//! Exemplar's memory chip in its footer explains itself: hovering it puts `smp.footer_tip`
//! on the window's help line (round 26).

mod support;
use cantus_core::*;
use support::*;

/// Where the text containing `needle` was painted.
fn find(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Pos2> {
    fn walk(s: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
        match s {
            egui::Shape::Text(t) if t.galley.text().contains(needle) => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, needle)),
            _ => None,
        }
    }
    shapes.iter().find_map(|c| walk(&c.shape, needle))
}

#[test]
fn hovering_the_memory_chip_tells_what_it_counts() {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler)),
    );
    a.open_inst_editor(track);
    let ctx = ctx();
    let size = egui::vec2(1400.0, 900.0);
    let frame = |a: &mut cantus_ui::CantusApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| cantus_ui::views::inst::window_in(a, ui));
        out.textures_delta.clear();
        out.shapes
    };
    let mut shapes = Vec::new();
    for _ in 0..3 {
        shapes = frame(&mut a, Vec::new());
    }
    let tip = cantus_ui::i18n::t("smp.footer_tip");
    assert!(
        !painted_text(&shapes).contains(&tip[..30]),
        "not before the hover"
    );
    let chip = find(&shapes, "MiB").expect("the memory chip");
    for _ in 0..3 {
        shapes = frame(&mut a, vec![egui::Event::PointerMoved(chip)]);
    }
    let text = painted_text(&shapes);
    assert!(
        text.contains(&tip[..30]),
        "the tip on the help line: {text}"
    );
    if let Some(dir) = std::env::var_os("CANTUS_SHOT_DIR") {
        let p = std::path::PathBuf::from(dir).join("sampler_footer_tip.png");
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        cantus_ui::raster::shoot_at(
            &support::ctx(),
            size,
            1.0,
            3,
            Some(&p),
            |_| Some(chip),
            |root| {
                cantus_ui::views::inst::window_in(&mut a, root);
            },
        );
    }
}
