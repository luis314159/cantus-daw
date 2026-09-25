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
        let name = cantus_ui::i18n::t(kind.name_key());
        assert!(
            text.contains(&name),
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
