//! *Modulate…* on any parameter (ADR 0057, review 1): every native effect's editor and every
//! native instrument's editor, in each of its views, is opened headless with the kit's
//! probe on, and every parameter control it draws — knob, cell, the house cells of each
//! editor's own helpers — must offer modulation (its menu's *Modulate…*, a modulator's
//! handle dropped on it, the Alt-drag depth). A control drawn through a path that forgot
//! the wiring is named here.

mod support;
use cantus_core::*;
use cantus_ui::kit::automate::probe;
use cantus_ui::views::inst::View;
use support::*;

/// The controls drawn that offered no modulation. (A number that is no parameter of
/// anything — a drum pad's field — says so to the kit, and the probe passes it by.)
fn unwired(drawn: Vec<(String, bool)>) -> Vec<String> {
    drawn
        .into_iter()
        .filter(|(_, m)| !m)
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn every_native_effect_editor_offers_modulate_on_every_control() {
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
            // Edited only in the inspector, under its insert's tile: those rows too.
            let track = a.project().tracks[0].id;
            a.select_track(track);
            a.inspector.show_insert_values(fx);
            probe::start();
            paint(&ctx(), egui::vec2(360.0, 1800.0), 3, |ui| {
                cantus_ui::views::inspector::show(&mut a, ui)
            });
            let drawn = probe::take();
            for info in kind
                .params()
                .iter()
                .filter(|p| p.key != "mix" && !p.stepped)
                .take(3)
            {
                assert!(
                    drawn.iter().any(|(n, m)| n == info.label_key && *m),
                    "{kind:?}: the inspector's {} row offers no Modulate…",
                    info.key
                );
            }
            continue;
        }
        // Each editor as its demo sets it up (bands made, parts split, a band chosen), so
        // every card it has is drawn.
        cantus_ui::views::fx::demo_setup(&mut a, s0, fx, kind);
        a.open_effect_editor(s0, fx);
        if kind == EffectKind::Eq && a.eq_editor.primary.is_none() {
            a.eq_editor.primary = Some(0);
        }
        probe::start();
        paint_editors(&mut a, egui::vec2(1600.0, 1000.0));
        let drawn = probe::take();
        assert!(!drawn.is_empty(), "{kind:?}: its editor drew no parameter");
        bad.extend(unwired(drawn).into_iter().map(|n| format!("{kind:?}: {n}")));
    }
    bad.dedup();
    assert!(bad.is_empty(), "controls without Modulate…: {bad:#?}");
}

#[test]
fn every_native_instrument_editor_offers_modulate_on_every_control() {
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
            let drawn = probe::take();
            bad.extend(
                unwired(drawn)
                    .into_iter()
                    .map(|n| format!("{kind:?} {view:?}: {n}")),
            );
        }
    }
    bad.dedup();
    assert!(bad.is_empty(), "controls without Modulate…: {bad:#?}");
}
