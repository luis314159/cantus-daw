//! Multiplex's editor through the app: bands made and removed as single undo steps, the
//! crossovers and thresholds dragged on the display through its published geometry, the
//! presets, the inspector's picture, and a paint of the whole window.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::multiband::{self as mb, FACTORY_PRESETS};
use support::*;

fn with_multiband() -> (cantus_ui::CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    // The unit at its defaults: a neutral insert (ADR 0071).
    a.settings.insert_neutral = true;
    a.add_effect(s0, EffectKind::Multiband);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    (a, s0, fx)
}

fn spec(a: &cantus_ui::CantusApp, s0: StripId) -> EffectSpec {
    a.project().strip(s0).unwrap().effects[0].spec.clone()
}

#[test]
fn bands_are_split_and_removed_as_single_undo_steps() {
    let (mut a, s0, fx) = with_multiband();
    assert!(
        spec(&a, s0).params.is_empty(),
        "a new Multiplex stores nothing"
    );

    // The first split makes two bands.
    assert_eq!(a.mb_split_at(s0, fx, 400.0), Some(1));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("b0.used"), 1.0);
    assert_eq!(sp.get("b1.from"), 400.0);
    a.undo();
    assert!(spec(&a, s0).params.is_empty(), "one undo takes both back");
    a.redo();

    // A split copies the band it splits, so it changes nothing you hear.
    a.set_fx_param(s0, fx, "b1.thr", -31.0, false);
    a.set_fx_param(s0, fx, "b1.ratio_lo", 3.0, false);
    assert_eq!(a.mb_split_at(s0, fx, 3_000.0), Some(2));
    let sp = spec(&a, s0);
    assert_eq!(sp.get("b2.from"), 3_000.0);
    assert_eq!(sp.get("b2.thr"), -31.0);
    assert_eq!(sp.get("b2.ratio_lo"), 3.0);
    // No room: too close to a crossover.
    assert_eq!(a.mb_split_at(s0, fx, 3_300.0), None);

    // Removing a band frees its number and forgets it; undo brings it back exactly.
    let before = spec(&a, s0);
    a.mb_remove_band(s0, fx, 1);
    assert!(spec(&a, s0).params.keys().all(|k| !k.starts_with("b1.")));
    let s = cantus_engine::fx::multiband::settings(&spec(&a, s0));
    assert!(s.bands[0].used && !s.bands[1].used && s.bands[2].used);
    a.undo();
    assert_eq!(spec(&a, s0), before);

    // Eight bands at most.
    let (mut a, s0, fx) = with_multiband();
    a.mb_split_at(s0, fx, 40.0);
    for hz in [80.0, 160.0, 320.0, 640.0, 1_280.0, 2_560.0] {
        assert!(a.mb_split_at(s0, fx, hz).is_some(), "{hz}");
    }
    assert_eq!(a.mb_split_at(s0, fx, 10_000.0), None, "a ninth band");
}

fn frame(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    events: Vec<egui::Event>,
    time: f64,
) -> String {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        time: Some(time),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    });
    out.textures_delta.clear();
    painted_text(&out.shapes)
}

/// Press at `from`, move to `to` in steps, release: one drag, as egui sees it.
fn drag(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    from: egui::Pos2,
    to: egui::Pos2,
    start: f64,
) {
    let mut t = start;
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(ctx, a, vec![egui::Event::PointerMoved(from)], t);
    t += 0.05;
    frame(ctx, a, vec![press(from, true)], t);
    for i in 1..=6 {
        t += 0.05;
        let p = from + (to - from) * (i as f32 / 6.0);
        frame(ctx, a, vec![egui::Event::PointerMoved(p)], t);
    }
    t += 0.05;
    frame(ctx, a, vec![press(to, false)], t);
    t += 0.05;
    frame(ctx, a, vec![], t);
}

#[test]
fn a_crossover_and_a_threshold_are_dragged_on_the_display() {
    let (mut a, s0, fx) = with_multiband();
    a.mb_quick_bands(s0, fx, 3);
    a.open_effect_editor(s0, fx);
    let ctx = ctx();
    for i in 0..3 {
        frame(&ctx, &mut a, vec![], i as f64 * 0.1);
    }
    let geo =
        a.fx.multiband
            .geo
            .expect("the display published its geometry");
    let lay = cantus_dsp::multiplex::Layout::of(
        &cantus_engine::fx::multiband::settings(&spec(&a, s0)).bands,
    );
    assert_eq!(lay.n, 3);

    // The crossover at 2.5 kHz (band 2's start), dragged to the right.
    let x = geo.x(2_500.0);
    let y = geo.level.center().y + 40.0;
    let steps = a.bus.history().len();
    drag(
        &ctx,
        &mut a,
        egui::pos2(x, y),
        egui::pos2(geo.x(5_000.0), y),
        10.0,
    );
    let moved = spec(&a, s0).get("b2.from");
    assert!(
        (moved - 5_000.0).abs() < 300.0,
        "the crossover went to {moved}"
    );
    assert_eq!(a.bus.history().len(), steps + 1, "one drag, one undo step");
    a.undo();
    assert_eq!(spec(&a, s0).get("b2.from"), 2_500.0);

    // Band 1's threshold line, dragged down to −40 dBFS.
    let s = cantus_engine::fx::multiband::settings(&spec(&a, s0));
    let (x0, x1) = geo.span(&lay, 1).unwrap();
    let ty = geo.y_level(s.bands[1].thr_db);
    let from = egui::pos2((x0 + x1) * 0.5 - 20.0, ty);
    drag(
        &ctx,
        &mut a,
        from,
        egui::pos2(from.x, geo.y_level(-40.0)),
        20.0,
    );
    let thr = spec(&a, s0).get("b1.thr");
    assert!((thr - -40.0).abs() < 1.5, "the threshold went to {thr}");
    assert_eq!(
        a.fx.multiband.selected,
        Some(1),
        "the band dragged is selected"
    );

    // A double-click on the spectrum, clear of every line, splits the band under it.
    let at = egui::pos2(geo.x(700.0), geo.y_level(-54.0));
    let click = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&ctx, &mut a, vec![egui::Event::PointerMoved(at)], 30.0);
    frame(&ctx, &mut a, vec![click(true)], 30.05);
    frame(&ctx, &mut a, vec![click(false)], 30.1);
    frame(&ctx, &mut a, vec![click(true)], 30.15);
    frame(&ctx, &mut a, vec![click(false)], 30.2);
    frame(&ctx, &mut a, vec![], 30.25);
    let s = cantus_engine::fx::multiband::settings(&spec(&a, s0));
    let lay = cantus_dsp::multiplex::Layout::of(&s.bands);
    assert_eq!(lay.n, 4, "the double-click made a band");
    assert!(
        lay.crossovers[..3]
            .iter()
            .any(|hz| (hz / 700.0).log2().abs() < 0.05),
        "no crossover near 700 Hz: {:?}",
        &lay.crossovers[..3]
    );
    // And the keyboard: `[` and `]` step through the bands, Delete removes one.
    let key = |k| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let before = a.fx.multiband.selected;
    frame(&ctx, &mut a, vec![key(egui::Key::CloseBracket)], 30.3);
    assert_ne!(a.fx.multiband.selected, before, "] selected the next band");
    frame(&ctx, &mut a, vec![key(egui::Key::Delete)], 30.35);
    let s = cantus_engine::fx::multiband::settings(&spec(&a, s0));
    assert_eq!(
        cantus_dsp::multiplex::Layout::of(&s.bands).n,
        3,
        "Delete removed it"
    );
}

/// Painted text as the kit sets it: card titles and labels in capitals, a value's unit a
/// hair apart from its number without the space — so compare lowercased, spaces removed.
fn norm(s: &str) -> String {
    s.to_lowercase().replace([' ', '\u{a0}'], "")
}

#[test]
fn the_editor_paints_its_display_and_band_panel() {
    let (mut a, s0, fx) = with_multiband();
    a.open_effect_editor(s0, fx);
    // Empty: an invitation, and quick starts.
    let text = norm(&paint_editors(&mut a, egui::vec2(1400.0, 900.0)));
    for want in [
        "Multiplex multiband",
        "Double-click",
        "3 bands",
        "No latency",
    ] {
        assert!(
            text.contains(&norm(want)),
            "empty editor lacks {want:?}: {text}"
        );
    }
    mb::demo_setup(&mut a, s0, fx);
    mb::demo_feed(&mut a);
    assert!(a.fx.multiband.history_len() > 100, "the demo fed a history");
    let text = norm(&paint_editors(&mut a, egui::vec2(1400.0, 900.0)));
    for want in [
        "4 of 8 bands",
        "Band 3",
        // The cells' labels take their short forms; the full names are on the help line.
        "Thresh",
        "Lo rat",
        "150 Hz",
        "2.6 kHz",
        "4.0:1",
        "Latency 2.0 ms",
        "Dynamic",
        "Sidechain",
        "In",
        "Out",
        "side",
    ] {
        assert!(
            text.contains(&norm(want)),
            "the editor never painted {want:?}: {text}"
        );
    }
}

#[test]
fn every_preset_names_real_parameters_and_makes_bands() {
    for (label, params) in FACTORY_PRESETS {
        let mut s = EffectSpec::new(EffectKind::Multiband);
        for (k, v) in *params {
            assert!(s.set(k, *v).is_some(), "{label}: {k} is not a parameter");
        }
        let st = cantus_engine::fx::multiband::settings(&s);
        let lay = cantus_dsp::multiplex::Layout::of(&st.bands);
        assert!(lay.n >= 2, "{label} makes {} bands", lay.n);
        assert!(st.bands.iter().any(|b| b.can_act()), "{label} does nothing");
        assert!(
            cantus_ui::i18n::t(label) != *label,
            "{label} has no English name"
        );
    }
}

#[test]
fn the_inspector_picture_paints_empty_and_full() {
    let ctx = ctx();
    let mut s = EffectSpec::new(EffectKind::Multiband);
    let text = paint(&ctx, egui::vec2(300.0, 200.0), 1, |ui| {
        mb::preview(ui, &s);
    });
    assert!(text.contains("No bands yet"), "{text}");
    for (k, v) in [("b0.used", 1.0), ("b1.used", 1.0), ("b1.from", 500.0)] {
        s.set(k, v);
    }
    let text = paint(&ctx, egui::vec2(300.0, 200.0), 1, |ui| {
        mb::preview(ui, &s);
    });
    assert!(text.contains("2 bands"), "{text}");
}
