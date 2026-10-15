//! Discrimen's editor (ADR 0056): what it paints, a mode and a listen chosen in one step,
//! the idle words, the parts' meters, and its presets.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::discrimen;
use cantus_ui::CantusApp;
use support::*;

fn with_discrimen() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::TransientSplit);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

fn click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    use egui::PointerButton::Primary;
    let b = |pressed| egui::Event::PointerButton {
        pos: at,
        button: Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, ctx, vec![b(true)]);
    frame(a, ctx, vec![b(false)]);
    frame(a, ctx, vec![]);
}

#[test]
fn the_editor_paints_its_cards() {
    let (mut a, s0, fx) = with_discrimen();
    discrimen::demo_setup(&mut a, s0, fx);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::TransientSplit);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase();
    for want in [
        "transient and body",
        "the parts now",
        "detection",
        "zero latency",
        "look-ahead",
        "sensitivity",
        "length",
        "gain",
        "comp",
        "tilt",
        "listen",
        "both",
        "transient share",
        "latency 2.0 ms",
        "every hit is in the transient part",
    ] {
        assert!(lower.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn left_alone_the_display_says_the_parts_add_up() {
    let (mut a, _, _) = with_discrimen();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("the two parts add up to the input"), "{text}");
}

#[test]
fn a_click_chooses_the_mode_and_what_is_heard_each_as_one_step() {
    let (mut a, s0, _) = with_discrimen();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let keys = a.fx.discrimen.mode_keys.expect("the mode keys were drawn");
    click(&mut a, &ctx, egui::pos2(keys.left() + keys.width() * 0.75, keys.center().y));
    let get = |a: &CantusApp, k: &str| a.project().strip(s0).unwrap().effects[0].spec.get(k);
    assert_eq!(get(&a, "mode"), 1.0, "look-ahead chosen");
    let keys = a.fx.discrimen.listen_keys.expect("the listen keys were drawn");
    click(&mut a, &ctx, egui::pos2(keys.left() + keys.width() * 0.83, keys.center().y));
    assert_eq!(get(&a, "listen"), 2.0, "the body alone");
    a.undo();
    assert_eq!(get(&a, "listen"), 0.0);
    a.undo();
    assert_eq!(get(&a, "mode"), 0.0);
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    for (label, values) in discrimen::FACTORY_PRESETS {
        for (k, _) in *values {
            assert!(
                EffectKind::TransientSplit.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
        let mut spec = EffectSpec::new(EffectKind::TransientSplit);
        for (k, v) in *values {
            spec.set(k, *v);
        }
        let s = cantus_engine::fx::discrimen::settings(&spec);
        assert!(!discrimen::idle(&s), "{label} does something");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// No word is cut short, in either language, with the editor fed as when it plays.
#[test]
fn no_word_is_cut_in_either_language() {
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(lang);
        let (mut a, s0, fx) = with_discrimen();
        discrimen::demo_setup(&mut a, s0, fx);
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::TransientSplit);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert!(!text.contains('\u{2026}'), "{lang:?}: {text}");
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}
