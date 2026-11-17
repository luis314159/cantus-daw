//! Vetus's face (ADR 0133, provisional): its own among every unit's, its three colourways
//! keeping the themes' contrast and painting the editor.

use cantus_engine::EngineHandle;
use cantus_ui::kit::skin::{self, Unit};
use cantus_ui::CantusApp;

const UNIT: Unit = Unit::Effect(cantus_core::EffectKind::LoFi);

fn hue_gap(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// No other unit wears Vetus's face: never the same material within 20° of its accent.
#[test]
fn the_face_is_its_own() {
    let me = skin::of(UNIT);
    assert_eq!(me.key, "fx.vetus");
    let mine = me.accent.expect("an accent").dark.h;
    for u in Unit::all() {
        let other = skin::of(u);
        if other.key == me.key {
            continue;
        }
        let Some(a) = other.accent else { continue };
        assert!(
            other.material != me.material || hue_gap(a.dark.h, mine) > 20.0,
            "{} looks like Vetus",
            other.key
        );
    }
}

#[test]
fn every_colourway_keeps_the_contrast_promises_and_paints_the_editor() {
    let ways = skin::colourways(UNIT);
    assert_eq!(ways.len(), 3);
    assert_eq!(ways[0].sheet.key, "fx.vetus", "the face is the first");
    for w in ways {
        let fails = skin::contrast_failures(w.sheet);
        assert!(fails.is_empty(), "{}: {fails:?}", w.id);
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(w.name_key), w.name_key, "{} named", w.id);
        }
        skin::set_colourway("vetus", w.id);
        assert_eq!(skin::of(UNIT).key, w.sheet.key);
        let mut a = CantusApp::from_parts(
            Some(cantus_ui::starter_project()),
            EngineHandle::dummy(),
            false,
        );
        a.demo_editor("vetus");
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            cantus_ui::views::fx::window(&mut a, ui.ctx())
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty(), "{}: the editor painted", w.id);
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_colourways(std::iter::empty());
}
