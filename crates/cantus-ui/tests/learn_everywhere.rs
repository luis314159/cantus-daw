//! *Learn MIDI controller* on any parameter (round 29, ADR 0062): every native effect's
//! editor, every native instrument's editor in each of its views, the inspector's rows
//! (an insert's values, a send, the pinned strip's volume, pan and width), drawn headless
//! with the kit's probe on — every parameter control must offer MIDI learn in its menu (and
//! so in the learn mode). A control drawn through a path that forgot it is named here.

mod support;
use cantus_core::*;
use cantus_ui::kit::automate::probe;
use cantus_ui::views::inst::View;
use support::*;

fn unlearnable(drawn: Vec<(String, bool)>) -> Vec<String> {
    drawn
        .into_iter()
        .filter(|(_, l)| !l)
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn every_native_effect_editor_offers_learn_on_every_control() {
    let mut bad = Vec::new();
    for kind in EffectKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let s0 = a.project().tracks[0].strip.unwrap();
        a.add_effect(s0, kind);
        let fx = a
            .project()
            .strip(s0)
            .unwrap()
            .effects
            .iter()
            .find(|e| e.spec.kind == kind)
            .unwrap()
            .id;
        if !kind.has_editor() {
            let track = a.project().tracks[0].id;
            a.select_track(track);
            a.inspector.show_insert_values(fx);
            probe::start();
            paint(&ctx(), egui::vec2(360.0, 1800.0), 3, |ui| {
                cantus_ui::views::inspector::show(&mut a, ui)
            });
            let drawn = probe::take_learn();
            assert!(
                !drawn.is_empty(),
                "{kind:?}: the inspector drew no parameter"
            );
            bad.extend(
                unlearnable(drawn)
                    .into_iter()
                    .map(|n| format!("{kind:?}: {n}")),
            );
            continue;
        }
        cantus_ui::views::fx::demo_setup(&mut a, s0, fx, kind);
        a.open_effect_editor(s0, fx);
        if kind == EffectKind::Eq && a.eq_editor.primary.is_none() {
            a.eq_editor.primary = Some(0);
        }
        probe::start();
        paint_editors(&mut a, egui::vec2(1600.0, 1000.0));
        let drawn = probe::take_learn();
        assert!(!drawn.is_empty(), "{kind:?}: its editor drew no parameter");
        bad.extend(
            unlearnable(drawn)
                .into_iter()
                .map(|n| format!("{kind:?}: {n}")),
        );
    }
    bad.dedup();
    assert!(
        bad.is_empty(),
        "controls without Learn MIDI controller: {bad:#?}"
    );
}

#[test]
fn every_native_instrument_editor_offers_learn_on_every_control() {
    let mut bad = Vec::new();
    for kind in InstrumentKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        a.open_inst_editor(track);
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            probe::start();
            paint(&ctx(), egui::vec2(1400.0, 900.0), 3, |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui)
            });
            let drawn = probe::take_learn();
            assert!(
                drawn.iter().any(|(_, l)| *l),
                "{kind:?} {view:?}: its editor drew no learnable parameter"
            );
            bad.extend(
                unlearnable(drawn)
                    .into_iter()
                    .map(|n| format!("{kind:?} {view:?}: {n}")),
            );
        }
    }
    bad.dedup();
    assert!(
        bad.is_empty(),
        "controls without Learn MIDI controller: {bad:#?}"
    );
}

#[test]
fn sends_and_the_pinned_strip_offer_learn() {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_strip();
    let ret = a.project().strips.last().unwrap().id;
    assert!(a.add_sends(&[s0], ret));
    let drawn_of = |a: &mut cantus_ui::CantusApp| {
        probe::start();
        paint(&ctx(), egui::vec2(360.0, 2400.0), 3, |ui| {
            cantus_ui::views::inspector::show(a, ui)
        });
        probe::take_learn()
    };
    a.select_track(track);
    let drawn = drawn_of(&mut a);
    for key in [
        "inspector.send_level",
        "kit.param.volume",
        "kit.param.pan",
        "kit.param.width",
    ] {
        assert!(
            drawn.iter().any(|(n, l)| n == key && *l),
            "the channel's {key} offers no Learn: {drawn:?}"
        );
    }
    assert!(unlearnable(drawn).is_empty());
    a.select_strip(StripId::MASTER);
    let drawn = drawn_of(&mut a);
    for key in ["kit.param.volume", "kit.param.pan", "kit.param.width"] {
        assert!(
            drawn.iter().any(|(n, l)| n == key && *l),
            "the master's {key}"
        );
    }
}

/// Chorda's tuning keys take no controller either (they would record automation).
#[test]
fn chordas_tuning_keys_are_not_learnt() {
    use cantus_core::controller::MappingTarget;
    use cantus_core::inst::chorda::DESIGN_KEYS;
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda)),
    );
    for key in DESIGN_KEYS {
        a.learn_target(MappingTarget::Param(AutoTarget::Instrument {
            track,
            key: key.to_string(),
        }));
        assert!(a.controllers.learning.is_none(), "{key} armed");
    }
}
