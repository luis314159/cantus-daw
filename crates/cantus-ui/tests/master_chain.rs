//! The master's insert chain in the UI (ADR 0026): a master effect's window opens in the
//! shared frame and names the master; A/B and presets work there; on the desk the master
//! shows its chain and its menu switches it, and a click on its head shows its chain in
//! the inspector.

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
    painted_text(&desk_shapes(ctx, a, frame, events))
}

/// Where the text `label` was painted among `shapes`.
fn text_rect(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<egui::Rect> {
    fn walk(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, label)),
            _ => None,
        }
    }
    shapes.iter().find_map(|s| walk(&s.shape, label))
}

/// One frame of the desk with `events`; returns what it painted.
fn desk_shapes(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    frame: &mut u64,
    events: Vec<Event>,
) -> Vec<egui::epaint::ClippedShape> {
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
    out.shapes
}

fn click(ctx: &egui::Context, a: &mut cantus_ui::CantusApp, frame: &mut u64, p: Pos2) {
    click_with(ctx, a, frame, p, egui::PointerButton::Primary);
}

fn click_with(
    ctx: &egui::Context,
    a: &mut cantus_ui::CantusApp,
    frame: &mut u64,
    p: Pos2,
    button: egui::PointerButton,
) {
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
                    button,
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
    // Round 25: Extra large shows the chain (the slots moved to the inspector, which shows
    // the master's chain once it is chosen). Round 39, des-mixer's Large: "inserts show
    // unit names (Comprimo, Aequalis)" — the master's limiter by its unit's name.
    let name = cantus_ui::i18n::t("picker.dev.terminus.name");
    assert!(
        text.contains(&format!("| {name} |")),
        "the master's chain line does not show its insert: {text}"
    );
    // The effects switch switches the master's chain. Round 39: des-mixer's strip has no
    // FX lamp (its rows: the sound, the mix, the route); on the desk the switch is the
    // master's menu's *Effects on* row (a right-click on its head), as on every strip.
    let head = views::mixer::control_rect(&ctx, None, Control::Head).expect("the master's head");
    click_with(
        &ctx,
        &mut a,
        &mut frame,
        head.center(),
        egui::PointerButton::Secondary,
    );
    let row = cantus_ui::i18n::t("master.menu.fx_on");
    let shapes = desk_shapes(&ctx, &mut a, &mut frame, Vec::new());
    let fx = text_rect(&shapes, &row).unwrap_or_else(|| {
        panic!(
            "the master's menu has no {row:?}: {}",
            painted_text(&shapes)
        )
    });
    click(&ctx, &mut a, &mut frame, fx.center());
    assert!(
        !a.project().master.fx_enabled,
        "the row switched the chain off"
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
            // Round 39 (des-mixer): Routing, where the output is, starts folded.
            views::inspector::unfold(ui.ctx(), "kit.inspector.routing");
            egui::CentralPanel::default().show(ui, |ui| views::inspector::show(&mut a, ui));
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    // Round 39, des-mixer's insert row: the unit's name ("Terminus"), then its kind
    // ("Limit") after it — not the effect's whole name ("Terminus limiter").
    for want in [
        cantus_ui::i18n::t("master.input"),
        format!("| {} |", cantus_ui::i18n::t("picker.dev.terminus.name")),
        format!("| {} |", cantus_ui::i18n::t("mixer.chain.limiter")),
        cantus_ui::i18n::t("master.output"),
    ] {
        assert!(
            text.contains(&want),
            "the master's sheet lacks {want:?}: {text}"
        );
    }
}
