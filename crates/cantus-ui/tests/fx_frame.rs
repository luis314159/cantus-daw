//! The plug-in frame, for every kind with an editor: it opens, it paints its header, and
//! A/B is one undo step that restores exactly what was there.

mod support;
use cantus_core::*;
use support::*;

#[test]
fn every_plug_in_opens_in_the_shared_frame() {
    for kind in EffectKind::ALL.into_iter().filter(|k| k.has_editor()) {
        let mut a = app(cantus_ui::starter_project());
        let s0 = a.project().tracks[0].strip.unwrap();
        a.add_effect(s0, kind);
        let fx = a.project().strip(s0).unwrap().effects[0].id;
        a.open_effect_editor(s0, fx);
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        // The frame's header sets the name in capitals, in the device's hue.
        let name = cantus_ui::i18n::t(kind.name_key());
        assert!(
            text.to_lowercase().contains(&name.to_lowercase()),
            "{kind:?}: the editor never painted its name: {text}"
        );
    }
}

#[test]
fn a_b_is_one_undo_step_and_comes_back_exactly() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Compressor);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    let spec = |a: &cantus_ui::CantusApp| a.project().strip(s0).unwrap().effects[0].spec.clone();
    let a_side = spec(&a);
    // Make B: switch to it (a copy of A), change it.
    a.toggle_ab(s0, fx);
    assert!(a.fx.on_b(fx));
    a.set_fx_param(s0, fx, "thr", -40.0, false);
    let b_side = spec(&a);
    // Back to A: exactly A.
    a.toggle_ab(s0, fx);
    assert_eq!(spec(&a), a_side);
    // And to B again: exactly B.
    a.toggle_ab(s0, fx);
    assert_eq!(spec(&a), b_side);
    // One undo takes back the last switch, and only it.
    a.undo();
    assert_eq!(spec(&a), a_side);
}

#[test]
fn a_preset_sets_what_it_names_and_resets_the_rest() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Compressor);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.set_fx_param(s0, fx, "knee", 20.0, false);
    let preset: std::collections::BTreeMap<String, f32> =
        [("thr".to_string(), -30.0)].into_iter().collect();
    a.apply_fx_params(s0, fx, &preset, "cmd.fx_preset");
    let spec = a.project().strip(s0).unwrap().effects[0].spec.clone();
    assert_eq!(spec.get("thr"), -30.0);
    assert_eq!(
        spec.get("knee"),
        6.0,
        "a parameter the preset does not name is reset"
    );
}

/// Frames of the effect window with these events, at 1400 × 900.
fn frames(a: &mut cantus_ui::CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
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

/// A click at `at`: press and release over two frames.
fn click(a: &mut cantus_ui::CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frames(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frames(a, ctx, vec![button(true)]);
    frames(a, ctx, vec![button(false)]);
    frames(a, ctx, vec![]);
}

/// Where the parts of the device header are, for the open effect window.
fn header_parts(a: &cantus_ui::CantusApp, kind: EffectKind) -> cantus_ui::kit::shell::HeaderRects {
    use cantus_ui::kit::shell::{self, Ab, Header, Preset};
    let pos = a.fx.pos.expect("the window was placed");
    let w = cantus_ui::views::fx::frame::designed_size(kind).x;
    let rect = egui::Rect::from_min_size(pos, egui::vec2(w, shell::HEADER));
    let s = cantus_ui::kit::Style::current().function(cantus_ui::views::fx::frame::hue_of(kind));
    let header = Header {
        name: "",
        context: "",
        hue: cantus_ui::views::fx::frame::hue_of(kind),
        preset: Some(Preset {
            name: "",
            modified: false,
            favourite: false,
        }),
        ab: Some(Ab::A),
        undo: Some((false, false)),
        views: None,
        bypassed: Some(false),
        help: false,
    };
    shell::header_rects(rect, &s, &header)
}

/// The header's ‹ › load the kind's factory presets, one undo step each; the device's
/// own undo takes the last one back; the lamp bypasses; A|B switches sides.
#[test]
fn the_device_header_steps_presets_undoes_bypasses_and_compares() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Gate);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    let ctx = ctx();
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let r = header_parts(&a, EffectKind::Gate);
    let spec = |a: &cantus_ui::CantusApp| a.project().strip(s0).unwrap().effects[0].spec.clone();
    // › loads the first factory preset: Kick.
    click(&mut a, &ctx, r.preset_next.unwrap().center());
    let first = cantus_ui::views::fx::factory_presets(EffectKind::Gate)[0];
    for (k, v) in first.1 {
        assert_eq!(spec(&a).get(k), *v, "{k} after loading {}", first.0);
    }
    // The device's own undo: the preset goes.
    click(&mut a, &ctx, r.undo.unwrap().center());
    assert_eq!(
        spec(&a).get("thr"),
        -40.0,
        "the device's undo took the preset back"
    );
    click(&mut a, &ctx, r.redo.unwrap().center());
    assert_eq!(
        spec(&a).get("thr"),
        first.1[0].1,
        "and its redo brought it again"
    );
    // The lamp.
    click(&mut a, &ctx, r.bypass.unwrap().center());
    assert!(a.project().strip(s0).unwrap().effects[0].bypass, "bypassed");
    click(&mut a, &ctx, r.bypass.unwrap().center());
    assert!(!a.project().strip(s0).unwrap().effects[0].bypass);
    // A|B: the right half is B, a copy of A to start with.
    let ab = r.ab.unwrap();
    click(&mut a, &ctx, egui::pos2(ab.right() - 4.0, ab.center().y));
    assert!(a.fx.on_b(fx), "B is showing");
    click(&mut a, &ctx, egui::pos2(ab.left() + 4.0, ab.center().y));
    assert!(!a.fx.on_b(fx), "A again");
}

/// The size menu scales the window; the scale is kept for the kind.
#[test]
fn the_size_menu_scales_the_window() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Gate);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    let ctx = ctx();
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let before = a.fx.gate.geometry.expect("drawn").transfer.width();
    let shell = cantus_ui::views::fx::frame::shell_of(EffectKind::Gate);
    assert_eq!(shell.scale(&ctx), 1.0);
    shell.set_scale(&ctx, 1.3);
    assert_eq!(shell.scale(&ctx), 1.25, "snapped to a step");
    frames(&mut a, &ctx, vec![]);
    frames(&mut a, &ctx, vec![]);
    let after = a.fx.gate.geometry.expect("drawn").transfer.width();
    assert!(
        after > before * 1.2,
        "the displays grew with the window: {before} to {after}"
    );
}
