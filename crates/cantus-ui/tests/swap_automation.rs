//! Integration (round 20): swapping an effect (device-ux, ADR 0029) keeps its automation
//! (ADR 0027) where the new kind has the parameter — in the same undo step.

mod support;
use cantus_core::*;
use cantus_ui::picker::PickChoice;
use support::*;

fn lane_target(a: &cantus_ui::CantusApp) -> Option<AutoTarget> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find_map(|c| c.automation.as_ref().map(|x| x.target.clone()))
}

#[test]
fn a_swapped_effect_keeps_the_automation_of_a_parameter_both_kinds_have() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Limiter);
    let old = a.project().strip(s0).unwrap().effects[0].id;
    a.automate(AutoTarget::Effect {
        effect: old,
        key: "ceiling".into(),
    });
    assert!(matches!(lane_target(&a), Some(AutoTarget::Effect { effect, .. }) if effect == old));

    // Culmen has a `ceiling` too: the lane follows the slot.
    assert!(a.replace_effect_with(s0, old, &PickChoice::Effect(EffectKind::Maximizer)));
    let new = a.project().strip(s0).unwrap().effects[0].id;
    assert_ne!(new, old);
    assert_eq!(
        lane_target(&a),
        Some(AutoTarget::Effect {
            effect: new,
            key: "ceiling".into()
        })
    );

    // One undo brings back the limiter and the lane on it.
    a.undo();
    assert_eq!(a.project().strip(s0).unwrap().effects[0].id, old);
    assert!(matches!(lane_target(&a), Some(AutoTarget::Effect { effect, .. }) if effect == old));
}

#[test]
fn a_lane_on_a_parameter_the_new_kind_lacks_keeps_its_clip() {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Limiter);
    let old = a.project().strip(s0).unwrap().effects[0].id;
    a.automate(AutoTarget::Effect {
        effect: old,
        key: "dither".into(),
    });
    assert!(a.replace_effect_with(s0, old, &PickChoice::Effect(EffectKind::Stereo)));
    // The clip is still there, still pointing at the effect that is gone: silent, kept.
    assert!(matches!(lane_target(&a), Some(AutoTarget::Effect { effect, .. }) if effect == old));
}
