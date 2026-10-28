//! Second review of ADR 0073, #4: the device frame's header measures its name and context
//! at the theme's text size (0.8–1.4), as it paints them — and measures again when the text
//! size changes — so the preset well gives way before a larger name is cut. A process of
//! its own: it installs the kit's tokens for the whole process.

mod support;

use cantus_core::EffectKind;
use support::*;

fn header_text(a: &mut cantus_ui::CantusApp) -> String {
    paint_editors(a, egui::vec2(1600.0, 1000.0))
}

#[test]
fn the_header_name_reads_whole_at_the_largest_text_size() {
    let mut a = app(cantus_ui::starter_project());
    let strip = a.project().tracks[0].strip.unwrap();
    a.add_effect(strip, EffectKind::Saturator);
    let fx = a.project().strip(strip).unwrap().effects.last().unwrap().id;
    a.open_effect_editor(strip, fx);
    // At the default size first: the header's words are measured and kept.
    let text = header_text(&mut a);
    let context = text
        .split(" | ")
        .find(|w| w.starts_with('(') && w.ends_with(')'))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("the context whole at 1.0: {text}"));
    // The largest text size, the same words (no project change, no language change).
    let mut t = cantus_ui::kit::tokens::current();
    t.text_scale = 1.4;
    cantus_ui::kit::tokens::install(t);
    let text = header_text(&mut a);
    assert!(
        text.contains(&context),
        "{context} cut at text size 1.4: {text}"
    );
    assert!(!text.contains("(I\u{2026}"), "{text}");
}
