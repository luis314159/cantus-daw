//! Aequalis's editor through the app: the gestures the research put first — rubber-band
//! selection and a relative multi-band drag, typed values, keyboard editing, split, the
//! audition, EQ match — each one step of the project's undo, and the chips that keep the
//! mode, the latency and the analyser visible at rest.
//!
//! The kit sets labels in capitals and a value's unit a hair from its number, so painted
//! words are compared case-insensitively and without spaces ([`has`]).

mod support;
use cantus_core::*;
use cantus_dsp::EqShape;
use cantus_ui::views::eq_editor::{self, parse};
use cantus_ui::CantusApp;
use support::*;

const SIZE: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// Whether the painted `text` says `want`, whatever the case and the spacing.
fn has(text: &str, want: &str) -> bool {
    let norm = |s: &str| s.to_lowercase().replace(' ', "");
    norm(text).contains(&norm(want))
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

/// An app with an EQ on the first strip, its editor open.
fn with_eq() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Eq);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_eq_editor(s0, fx);
    (a, s0, fx)
}

/// Run one frame of the editor window with these events.
fn frame(ctx: &egui::Context, a: &mut CantusApp, events: Vec<egui::Event>) -> String {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| eq_editor::window(a, ui.ctx()));
    out.textures_delta.clear();
    painted_text(&out.shapes)
}

fn button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}

/// Press at `from`, move to `to` in steps, release.
fn drag(ctx: &egui::Context, a: &mut CantusApp, from: egui::Pos2, to: egui::Pos2) {
    let none = egui::Modifiers::NONE;
    frame(ctx, a, vec![egui::Event::PointerMoved(from)]);
    frame(
        ctx,
        a,
        vec![egui::Event::PointerMoved(from), button(from, true, none)],
    );
    for i in 1..=5 {
        let at = from + (to - from) * (i as f32 / 5.0);
        frame(ctx, a, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        ctx,
        a,
        vec![egui::Event::PointerMoved(to), button(to, false, none)],
    );
    frame(ctx, a, vec![]);
}

fn key(k: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        },
    ]
}

#[test]
fn the_chips_show_mode_latency_and_analyser_at_rest() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 1_000.0, 6.0);
    a.set_fx_param(s0, fx, "mode", 2.0, false);
    let text = paint_editors(&mut a, SIZE);
    for want in [
        "Zero latency",
        "Natural phase",
        "Linear phase",
        "Medium",
        "Latency 96.0 ms",
        "4608",
        "Pre",
        "Post",
        "Freeze",
        "Grab",
        "Collisions",
        "Match",
        "Piano",
        "Default",
        "Band 1",
        "Output",
        "Scale",
        "Auto",
        "1 of 24 bands",
    ] {
        assert!(
            has(&text, want),
            "the editor never painted {want:?}: {text}"
        );
    }
    a.set_fx_param(s0, fx, "mode", 0.0, false);
    let text = paint_editors(&mut a, SIZE);
    assert!(has(&text, "No latency"));
}

/// A rectangle dragged over the background selects the dots inside it; dragging one of
/// them moves all of them — the same number of semitones and dB each — and one undo puts
/// both back.
#[test]
fn rubber_band_selects_and_a_multi_band_drag_is_one_undo_step() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 400.0, 3.0);
    a.add_eq_band(s0, fx, EqShape::Bell, 1_600.0, -3.0);
    a.add_eq_band(s0, fx, EqShape::Bell, 8_000.0, 2.0);
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    frame(&ctx, &mut a, vec![]);
    let sp = spec(&a, s0);
    let d0 = a.eq_editor.dot_pos(&sp, 0).unwrap();
    let d1 = a.eq_editor.dot_pos(&sp, 1).unwrap();
    // Around the first two dots, from empty background.
    let from = egui::pos2(d0.x - 30.0, d0.y.min(d1.y) - 30.0);
    let to = egui::pos2(d1.x + 30.0, d0.y.max(d1.y) + 30.0);
    drag(&ctx, &mut a, from, to);
    let sel: Vec<usize> = a.eq_editor.selection.iter().copied().collect();
    assert_eq!(sel, vec![0, 1], "the rubber band selected {sel:?}");

    let before = spec(&a, s0);
    let d1 = a.eq_editor.dot_pos(&before, 1).unwrap();
    drag(&ctx, &mut a, d1, d1 + egui::vec2(60.0, -40.0));
    let after = spec(&a, s0);
    let ratio = |b: usize| after.get(&format!("b{b}.freq")) / before.get(&format!("b{b}.freq"));
    let moved = |b: usize| after.get(&format!("b{b}.gain")) - before.get(&format!("b{b}.gain"));
    assert!(ratio(0) > 1.2, "band 1 moved up in frequency: {}", ratio(0));
    assert!(
        (ratio(0) - ratio(1)).abs() < 1e-3,
        "both by the same interval"
    );
    assert!(
        moved(0) > 1.0 && (moved(0) - moved(1)).abs() < 1e-3,
        "and the same dB"
    );
    assert_eq!(after.get("b2.freq"), 8_000.0, "the unselected band stayed");
    a.undo();
    assert_eq!(spec(&a, s0), before, "one undo puts the whole drag back");
}

/// Pressing on the curve and pulling makes a band and sets its gain in one gesture —
/// one undo step removes it.
#[test]
fn pulling_the_curve_makes_a_band_in_one_step() {
    let (mut a, s0, _fx) = with_eq();
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    let at = a.eq_editor.plot_pos(1_000.0, 0.0).unwrap();
    drag(&ctx, &mut a, at, at + egui::vec2(0.0, -80.0));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("b0.used"), 1.0);
    assert!(sp.get("b0.gain") > 3.0, "pulled up: {}", sp.get("b0.gain"));
    assert!((sp.get("b0.freq") / 1_000.0 - 1.0).abs() < 0.05);
    a.undo();
    assert!(
        spec(&a, s0).params.is_empty(),
        "one undo, and the EQ is new again"
    );
}

/// Focus the display, then: a number selects, an arrow moves a semitone, `]` narrows, B
/// bypasses, Delete removes.
#[test]
fn the_keyboard_edits_bands() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 440.0, 0.0);
    a.add_eq_band(s0, fx, EqShape::Bell, 2_000.0, 3.0);
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    // A click on empty background focuses the display (and clears the selection).
    let empty = a.eq_editor.plot_pos(60.0, -12.0).unwrap();
    let none = egui::Modifiers::NONE;
    frame(&ctx, &mut a, vec![egui::Event::PointerMoved(empty)]);
    frame(
        &ctx,
        &mut a,
        vec![egui::Event::PointerMoved(empty), button(empty, true, none)],
    );
    frame(
        &ctx,
        &mut a,
        vec![egui::Event::PointerMoved(empty), button(empty, false, none)],
    );
    frame(&ctx, &mut a, key(egui::Key::Num1, none));
    assert_eq!(a.eq_editor.primary, Some(0));
    frame(&ctx, &mut a, key(egui::Key::ArrowRight, none));
    let f = spec(&a, s0).get("b0.freq");
    assert!(
        (f - 440.0 * 2f32.powf(1.0 / 12.0)).abs() < 0.01,
        "a semitone up: {f}"
    );
    frame(&ctx, &mut a, key(egui::Key::ArrowUp, none));
    assert!((spec(&a, s0).get("b0.gain") - 0.5).abs() < 1e-4);
    frame(&ctx, &mut a, key(egui::Key::CloseBracket, none));
    assert!(spec(&a, s0).get("b0.q") > 1.1);
    frame(&ctx, &mut a, key(egui::Key::Num2, none));
    frame(&ctx, &mut a, key(egui::Key::B, none));
    assert_eq!(spec(&a, s0).get("b1.on"), 0.0, "B bypassed band 2");
    frame(&ctx, &mut a, key(egui::Key::Delete, none));
    assert_eq!(spec(&a, s0).get("b1.used"), 0.0, "Delete removed band 2");
    assert_eq!(spec(&a, s0).get("b0.used"), 1.0, "and only band 2");
}

#[test]
fn typed_values_are_read_the_way_musicians_write_them() {
    assert_eq!(parse::parse_freq("2k"), Some(2_000.0));
    assert!((parse::parse_freq("A4").unwrap() - 440.0).abs() < 1e-3);
    let c = parse::parse_freq("C#2+13").unwrap();
    assert!((c - 69.2957 * 2f32.powf(13.0 / 1200.0)).abs() < 0.01, "{c}");
    assert!((parse::parse_gain("2x").unwrap() - 6.0206).abs() < 1e-3);
}

/// Double-clicking a dot opens the value boxes; Enter sets what was typed, as one step.
#[test]
fn double_clicking_a_dot_types_its_values() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 1_000.0, 2.0);
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    let dot = a.eq_editor.dot_pos(&spec(&a, s0), 0).unwrap();
    let none = egui::Modifiers::NONE;
    frame(&ctx, &mut a, vec![egui::Event::PointerMoved(dot)]);
    for _ in 0..2 {
        frame(
            &ctx,
            &mut a,
            vec![egui::Event::PointerMoved(dot), button(dot, true, none)],
        );
        frame(
            &ctx,
            &mut a,
            vec![egui::Event::PointerMoved(dot), button(dot, false, none)],
        );
    }
    let typing = a.eq_editor.typing.clone().expect("the value boxes opened");
    assert_eq!(typing.band, 0);
    // Let the first box take focus, type into all three, press Enter.
    let text = frame(&ctx, &mut a, vec![]);
    assert!(has(&text, "Set"), "the boxes paint a Set button: {text}");
    if let Some(t) = a.eq_editor.typing.as_mut() {
        t.fields = ["A4".into(), "2x".into(), "3".into()];
    }
    frame(&ctx, &mut a, vec![]);
    let before = spec(&a, s0);
    frame(&ctx, &mut a, key(egui::Key::Enter, none));
    frame(&ctx, &mut a, vec![]);
    let sp = spec(&a, s0);
    assert!(a.eq_editor.typing.is_none(), "Enter closed the boxes");
    assert!(
        (sp.get("b0.freq") - 440.0).abs() < 0.01,
        "A4: {}",
        sp.get("b0.freq")
    );
    assert!((sp.get("b0.gain") - 6.0206).abs() < 1e-3);
    assert_eq!(sp.get("b0.q"), 3.0);
    a.undo();
    assert_eq!(spec(&a, s0), before, "one step");
}

#[test]
fn split_makes_a_pair_in_one_step() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 900.0, 4.0);
    let before = spec(&a, s0);
    let copy = a.split_eq_band(s0, fx, 0, true).unwrap();
    assert_eq!(copy, 1);
    let sp = spec(&a, s0);
    assert_eq!(sp.get("b0.place"), 3.0, "the band is on the mid");
    assert_eq!(sp.get("b1.place"), 4.0, "the copy on the side");
    assert_eq!(sp.get("b1.freq"), 900.0);
    assert_eq!(sp.get("b1.gain"), 4.0);
    a.undo();
    assert_eq!(spec(&a, s0), before);
}

#[test]
fn the_audition_is_one_command_and_ends_with_the_editor() {
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::Bell, 900.0, 4.0);
    a.set_eq_solo(s0, fx, Some(0));
    assert_eq!(spec(&a, s0).get("solo"), 1.0);
    let text = paint_editors(&mut a, SIZE);
    assert!(has(&text, "solo"), "the auditioned dot says so: {text}");
    // Removing the auditioned band ends the audition in the same step.
    a.remove_eq_band(s0, fx, 0);
    assert_eq!(spec(&a, s0).get("solo"), 0.0);
    a.undo();
    assert_eq!(spec(&a, s0).get("solo"), 1.0);
    a.close_eq_editor();
    assert_eq!(spec(&a, s0).get("solo"), 0.0, "closing the editor ends it");
}

/// EQ match: a fit becomes ordinary bands at free numbers, in one step.
#[test]
fn a_match_becomes_editable_bands_in_one_step() {
    use cantus_dsp::aequalis::fit::{fit, FitOptions};
    let (mut a, s0, fx) = with_eq();
    a.add_eq_band(s0, fx, EqShape::LowCut, 40.0, 0.0);
    let before = spec(&a, s0);
    let target = [
        (30.0, 0.0),
        (200.0, -4.0),
        (400.0, 0.0),
        (3_000.0, 3.0),
        (16_000.0, 3.0),
    ];
    let f = fit(&target, &FitOptions::default(), 48_000.0);
    assert!(!f.bands.is_empty());
    let made = a.apply_eq_fit(s0, fx, &f);
    assert_eq!(made.len(), f.bands.len());
    assert_eq!(made[0], 1, "band 1 was taken: the fit starts at 2");
    let sp = spec(&a, s0);
    for (n, b) in made.iter().zip(&f.bands) {
        assert_eq!(sp.get(&eq_key(*n, "freq")), b.freq);
    }
    a.undo();
    assert_eq!(spec(&a, s0), before);
    // The match panel paints, and says what it needs.
    a.eq_editor.matching = Some(Default::default());
    let text = paint_editors(&mut a, SIZE);
    assert!(has(&text, "Choose a key strip"), "{text}");
}

#[test]
fn factory_presets_are_whole_eqs_with_names_in_both_languages() {
    let presets = cantus_ui::views::fx::factory_presets(EffectKind::Eq);
    assert!(presets.len() >= 6);
    for (label, params) in presets {
        for (k, _) in *params {
            assert!(
                EffectKind::Eq.param(k).is_some(),
                "{label}: no parameter {k}"
            );
        }
        for loc in cantus_ui::i18n::Locale::ALL {
            cantus_ui::i18n::set_thread_locale(loc);
            assert_ne!(
                cantus_ui::i18n::t(label),
                *label,
                "{label} has no {loc:?} name"
            );
        }
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    }
    let (mut a, s0, fx) = with_eq();
    let (label, params) = presets[0];
    let map = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    a.apply_fx_params(s0, fx, &map, "cmd.fx_preset");
    assert_eq!(spec(&a, s0).params.len(), params.len(), "{label}");
}

/// Both themes paint the full editor — demo bands, a dynamic band, the key strip — with
/// no panic, and the demo shows what it promises.
#[test]
fn the_demo_paints_in_both_themes() {
    for dark in [false, true] {
        let mut a = app(cantus_ui::welcome_project());
        let ctx = ctx();
        let theme = if dark {
            cantus_ui::theme::ink_and_gold()
        } else {
            cantus_ui::theme::default_theme()
        };
        cantus_ui::theme::apply(&theme, &ctx);
        a.demo_editor("eq");
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Eq);
        let text = frame(&ctx, &mut a, vec![]);
        let text = text + &frame(&ctx, &mut a, vec![]);
        // The history's key switch is narrow: it says its `.short`, "Key".
        for want in ["Dynamic", "Above", "Key", "Threshold"] {
            assert!(has(&text, want), "{want:?} missing: {text}");
        }
    }
}
