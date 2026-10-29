//! The score's music font and the rest of the interface (round 32's integrated review):
//! Bravura never counts as a text face — a marker named "𝄋 Coda" still reaches the
//! system's fallback — and only the app's start-up registers it, so the exported plug-ins'
//! editors never carry it.

use cantus_ui::{design, font_fallback};

#[test]
fn bravura_maps_the_musical_symbols_but_no_text_face_counts_it() {
    let bravura = font_fallback::cmap_of(cantus_score::BRAVURA, 0);
    assert!(bravura.contains(&0x1D10B), "Bravura maps U+1D10B (segno)");
    // No text face has 𝄋 (the emoji face has 𝄞, so that one never needed the system):
    // the system's fallback is asked for it, as before the score came.
    assert!(!font_fallback::bundled_has('\u{1D10B}'));
    assert!(
        !font_fallback::bundled_has('\u{E050}'),
        "nor Bravura's private use area"
    );
    assert!(font_fallback::bundled_has('é'));
}

#[test]
fn only_the_apps_start_up_registers_the_music_font() {
    // What the plug-ins' editor installs (`design::install_fonts`): no music family.
    assert!(!design::font_definitions()
        .families
        .contains_key(&design::family_music()));
    let ctx = egui::Context::default();
    design::install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .textures_delta
        .clear();
    let has = |ctx: &egui::Context| {
        ctx.fonts(|f| {
            f.definitions()
                .families
                .contains_key(&design::family_music())
        })
    };
    assert!(!has(&ctx), "install_fonts alone leaves it out");
    // The app's font stage (`boot`, `CantusApp::new`): there.
    font_fallback::install_for_window(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {})
        .textures_delta
        .clear();
    assert!(has(&ctx), "the app's start-up registers it");
}
