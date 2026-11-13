//! Cribrum's editor (fx side round, ADR 0130 provisional): a handle dragged sets the cutoff
//! and the resonance in one undo step, a double-click puts them back; the curve edited on the
//! movement lane (a point added and taken away, one step each); the factory presets name real
//! keys in both languages and the starting preset is one of them.

mod support;
use cantus_core::*;
use cantus_ui::CantusApp;
use support::*;

fn with_cribrum(neutral: bool) -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.settings.insert_neutral = neutral;
    a.add_effect(s0, EffectKind::AutoFilter);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

thread_local! {
    static NOW: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1500.0, 1000.0),
        )),
        events,
        time: Some(NOW.with(|t| {
            let v = t.get() + 0.02;
            t.set(v);
            v
        })),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn double_click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    for _ in 0..2 {
        frame(a, ctx, vec![button(at, true)]);
        frame(a, ctx, vec![button(at, false)]);
    }
    frame(a, ctx, vec![]);
}

fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    NOW.with(|t| t.set(t.get() + 1.0));
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frame(a, ctx, vec![]);
}

/// The display's plot, after two frames.
fn display(a: &mut CantusApp, ctx: &egui::Context) -> egui::Rect {
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![]);
    a.fx.cribrum.display.expect("the display was drawn")
}

/// Where a cutoff sits across the display (20 Hz … 20 kHz, logarithmic).
fn x_of(r: egui::Rect, hz: f32) -> f32 {
    r.left() + r.width() * (hz / 20.0).ln() / 1000f32.ln()
}

/// Filter A's handle at its defaults: 1 kHz, on its own response there (a low-pass at 25 %
/// resonance is a little under 0 dB: about 0.6 of the way down from +24 to −36 dB).
fn handle_a(r: egui::Rect, spec: &EffectSpec) -> egui::Pos2 {
    let cs = cantus_engine::fx::cribrum::settings(spec);
    let v = cantus_dsp::cribrum::filter::Voice {
        model: cs.a.model,
        mode: cs.a.mode,
        slope24: cs.a.slope24,
        cut: cs.a.cut,
        res: cs.a.res,
        drive: cantus_dsp::math::db_to_gain(cs.a.drive_db),
        vowel: cs.a.vowel,
    };
    let db = 20.0
        * cantus_dsp::math::log10(
            cantus_dsp::cribrum::response::response(&v, cs.a.cut, 48_000.0)
                .abs()
                .max(1e-6) as f32,
        );
    let y = r.top() + r.height() * (24.0 - db.clamp(-33.0, 21.0)) / 60.0;
    egui::pos2(x_of(r, cs.a.cut), y)
}

#[test]
fn dragging_a_handle_sets_the_cutoff_and_the_resonance_in_one_step() {
    let (mut a, s0, _) = with_cribrum(true);
    let ctx = ctx();
    let r = display(&mut a, &ctx);
    let before = spec(&a, s0);
    let from = handle_a(r, &before);
    let to = egui::pos2(x_of(r, 250.0), from.y - r.height() * 0.2);
    drag(&mut a, &ctx, from, to);
    let after = spec(&a, s0);
    assert!(
        (after.get("a.cut") - 250.0).abs() < 15.0,
        "the cutoff follows across: {}",
        after.get("a.cut")
    );
    assert!(
        after.get("a.res") > before.get("a.res") + 0.15,
        "up raises the resonance: {} from {}",
        after.get("a.res"),
        before.get("a.res")
    );
    a.undo();
    let back = spec(&a, s0);
    assert_eq!(
        (back.get("a.cut"), back.get("a.res")),
        (before.get("a.cut"), before.get("a.res")),
        "one undo step"
    );
    // A double-click on the handle puts both back to their defaults.
    drag(&mut a, &ctx, from, to);
    let moved = spec(&a, s0);
    let at = handle_a(r, &moved);
    double_click(&mut a, &ctx, at);
    let reset = spec(&a, s0);
    assert_eq!((reset.get("a.cut"), reset.get("a.res")), (1_000.0, 0.25));
}

#[test]
fn a_double_click_on_the_curve_adds_a_point_and_another_takes_it_away() {
    let (mut a, s0, _) = with_cribrum(true);
    let ctx = ctx();
    // The editor's state belongs to the effect from its first frame.
    frame(&mut a, &ctx, vec![]);
    a.fx.cribrum.source = 2;
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let r = a.fx.cribrum.lane.expect("the lane was drawn");
    assert_eq!(spec(&a, s0).get("c2.on"), 0.0);
    // An empty place, above the default fall's flat bottom.
    let at = egui::pos2(r.left() + r.width() * 0.5, r.top() + r.height() * 0.3);
    double_click(&mut a, &ctx, at);
    let sp = spec(&a, s0);
    assert_eq!(sp.get("c2.on"), 1.0, "a point added");
    assert_eq!(sp.get("c2.x"), 0.5, "on the sixteenths");
    double_click(&mut a, &ctx, at);
    assert_eq!(spec(&a, s0).get("c2.on"), 0.0, "and taken away");
    a.undo();
    assert_eq!(spec(&a, s0).get("c2.on"), 1.0, "one step each");
}

#[test]
fn it_is_inserted_on_its_starting_preset_which_is_a_factory_preset() {
    let (a, s0, _) = with_cribrum(false);
    let sp = spec(&a, s0);
    assert_eq!(sp.get("mix"), 1.0, "it sounds at once");
    let start = cantus_core::starting::starting_preset(EffectKind::AutoFilter).expect("a preset");
    let factory = cantus_ui::views::fx::factory_presets(EffectKind::AutoFilter)
        .iter()
        .find(|(k, _)| *k == start.label_key)
        .expect("the starting preset is a factory preset");
    for (k, v) in factory.1 {
        assert_eq!(sp.get(k), *v, "{k}");
    }
}

#[test]
fn every_factory_preset_names_real_keys_and_is_in_both_languages() {
    let presets = cantus_ui::views::fx::factory_presets(EffectKind::AutoFilter);
    assert!(presets.len() >= 12, "{} presets", presets.len());
    for (label, values) in presets {
        let mut sp = EffectSpec::new(EffectKind::AutoFilter);
        for (k, v) in *values {
            assert!(sp.set(k, *v).is_some(), "{label}: no parameter {k}");
        }
        assert!(sp.get("mix") > 0.0, "{label} is heard");
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {lang:?}");
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Cribrum's face is its own: no other unit wears its hue and material together, and it
/// keeps the themes' contrast promises.
#[test]
fn cribrum_wears_a_face_of_its_own() {
    use cantus_ui::kit::skin::{self, Unit};
    let me = skin::of(Unit::Effect(EffectKind::AutoFilter));
    assert_eq!(me.key, "fx.cribrum");
    assert!(skin::contrast_failures(me).is_empty());
    for u in Unit::all() {
        let other = skin::of(u);
        if other.key == me.key || other.is_plain() {
            continue;
        }
        let near = (other.surface.hue - me.surface.hue).abs().min(
            360.0 - (other.surface.hue - me.surface.hue).abs(),
        ) < 20.0;
        assert!(
            !(near && other.material == me.material),
            "{u:?} wears Cribrum's hue and material"
        );
    }
}
