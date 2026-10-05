//! The master's insert chain in the UI (ADR 0026): a master effect's window opens in the
//! shared frame and names the master; A/B and presets work there; on the desk the master
//! shows its slots and its FX lamp, and a click on its head shows its chain in the
//! inspector.

mod support;
use cantus_core::*;
use cantus_ui::views::{self, mixer::Control};
use egui::{Event, Pos2};
use support::*;

const M: StripId = StripId::MASTER;

fn last_master_effect(a: &cantus_ui::CantusApp) -> EffectId {
    a.project().master.effects.last().unwrap().id
}

#[test]
fn a_master_effect_opens_in_the_shared_frame() {
    for kind in EffectKind::ALL.into_iter().filter(|k| k.has_editor()) {
        let mut a = app(cantus_ui::starter_project());
        a.add_effect(M, kind);
        assert!(a.project().strips.iter().all(|s| s.effects.is_empty()));
        let fx = last_master_effect(&a);
        a.open_effect_editor(M, fx);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        let name = cantus_ui::i18n::t(kind.name_key());
        assert!(
            text.to_lowercase().contains(&name.to_lowercase()),
            "{kind:?}: the editor never painted its name: {text}"
        );
        if kind != EffectKind::Eq && kind != EffectKind::Compressor {
            assert!(
                text.contains("Master"),
                "{kind:?}: the frame does not say it sits on the master: {text}"
            );
        }
    }
}

#[test]
fn a_b_and_presets_work_on_the_master() {
    let mut a = app(cantus_ui::starter_project());
    a.add_effect(M, EffectKind::Compressor);
    let fx = last_master_effect(&a);
    let spec = |a: &cantus_ui::CantusApp| a.project().effect(M, fx).unwrap().spec.clone();
    let a_side = spec(&a);
    a.toggle_ab(M, fx);
    a.set_fx_param(M, fx, "thr", -40.0, false);
    let b_side = spec(&a);
    assert_ne!(a_side, b_side);
    a.toggle_ab(M, fx);
    assert_eq!(spec(&a), a_side);
    a.undo();
    assert_eq!(spec(&a), b_side);
    let preset: std::collections::BTreeMap<String, f32> =
        [("thr".to_string(), -30.0)].into_iter().collect();
    a.apply_fx_params(M, fx, &preset, "cmd.fx_preset");
    assert_eq!(spec(&a).get("thr"), -30.0);
}

/// One frame of the desk with `events`; returns the painted text.
fn desk_frame(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    frame: &mut u64,
    events: Vec<Event>,
) -> String {
    *frame += 1;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        time: Some(*frame as f64 / 60.0),
        predicted_dt: 1.0 / 60.0,
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(a, ui));
    });
    out.textures_delta.clear();
    painted_text(&out.shapes)
}

fn click(ctx: &egui::Context, a: &mut cantus_ui::CantusApp, frame: &mut u64, p: Pos2) {
    desk_frame(ctx, a, frame, vec![Event::PointerMoved(p)]);
    for pressed in [true, false] {
        desk_frame(
            ctx,
            a,
            frame,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    desk_frame(ctx, a, frame, Vec::new());
}

#[test]
fn the_desk_shows_the_master_chain_and_chooses_the_master() {
    let ctx = ctx();
    let mut a = app(cantus_ui::starter_project());
    views::mixer::set_width_mode(&mut a, views::mixer::layout::WidthMode::from_index(2));
    a.add_effect(M, EffectKind::Limiter);
    let mut frame = 0;
    let mut text = String::new();
    for _ in 0..3 {
        text = desk_frame(&ctx, &mut a, &mut frame, Vec::new());
    }
    let name = cantus_ui::i18n::t(EffectKind::Limiter.name_key());
    // The slot elides the name to its width: its first letters are what show.
    let first: String = name.chars().take(6).collect();
    assert!(
        text.contains(&first),
        "the master's slot does not show its insert: {text}"
    );
    // The FX lamp switches the master's chain.
    let fx = views::mixer::control_rect(&ctx, None, Control::Fx).expect("the master's FX lamp");
    click(&ctx, &mut a, &mut frame, fx.center());
    assert!(
        !a.project().master.fx_enabled,
        "the lamp switched the chain off"
    );
    a.undo();
    assert!(a.project().master.fx_enabled);
    // A click on its head shows the master in the inspector.
    let s0 = a.project().strips[0].id;
    a.select_strip(s0);
    assert_ne!(
        views::inspector::subject(&a),
        views::inspector::Subject::Master
    );
    let head = views::mixer::control_rect(&ctx, None, Control::Head).expect("the master's head");
    click(&ctx, &mut a, &mut frame, head.center());
    assert_eq!(
        views::inspector::subject(&a),
        views::inspector::Subject::Master
    );
}

#[test]
fn the_inspector_shows_the_master_chain() {
    let ctx = ctx();
    let mut a = app(cantus_ui::starter_project());
    a.add_effect(M, EffectKind::Eq);
    a.add_effect(M, EffectKind::Limiter);
    a.select_master();
    let mut text = String::new();
    for i in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(400.0, 1000.0),
            )),
            time: Some(i as f64 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::inspector::show(&mut a, ui));
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    for want in [
        cantus_ui::i18n::t("master.input"),
        cantus_ui::i18n::t(EffectKind::Limiter.name_key()),
        cantus_ui::i18n::t("master.output"),
    ] {
        assert!(
            text.contains(&want),
            "the master's sheet lacks {want:?}: {text}"
        );
    }
}
