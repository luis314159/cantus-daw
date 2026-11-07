//! The older Reverb (round 37, the integrator's decision for ADR 0101): no menu or picker
//! offers it for a new insert — Spatium is the reverb — and a project that has one keeps it:
//! it reads back, renders, and sounds as before (the same samples as the unit itself makes).

use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec, Snapshot};
use cantus_engine::{render, RenderOptions};
use cantus_ui::picker::PickChoice;
use cantus_ui::preset_audition as aud;
use cantus_ui::preset_tags::Role;
use std::sync::Arc;

#[test]
fn the_picker_and_the_insert_menus_no_longer_offer_it() {
    assert!(!EffectKind::Reverb.offered());
    assert!(EffectKind::ALL
        .into_iter()
        .filter(|k| *k != EffectKind::Reverb)
        .all(EffectKind::offered));
    let devices = cantus_ui::picker_catalog::build(std::iter::empty());
    assert!(
        !devices
            .iter()
            .any(|d| d.choice == PickChoice::Effect(EffectKind::Reverb)),
        "the picker has no older Reverb"
    );
    assert!(
        devices
            .iter()
            .any(|d| d.choice == PickChoice::Effect(EffectKind::Space)),
        "Spatium is there"
    );
}

#[test]
fn a_project_with_the_older_reverb_still_loads_and_sounds() {
    let mut p = aud::phrase_project(
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::ElectricPiano)),
        Role::Keys,
    );
    let strip = p.tracks[0].strip.unwrap();
    let dry = {
        let snap = Arc::new(Snapshot::compile(&p, 0));
        render(snap, &RenderOptions::default()).expect("it renders")
    };
    let slot = p.make_effect(EffectKind::Reverb);
    p.strip_mut(strip).unwrap().effects.push(slot);
    // Saved and read back, as an old project is opened.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.cantus");
    cantus_project::store::save(&p, &path, "test").expect("it saves");
    let back = cantus_project::store::load(&path, true)
        .expect("it opens")
        .project;
    assert_eq!(
        back.strip(strip).unwrap().effects[0].spec.kind,
        EffectKind::Reverb
    );
    let wet = {
        let snap = Arc::new(Snapshot::compile(&back, 0));
        render(snap, &RenderOptions::default()).expect("it renders")
    };
    assert!(wet.samples.iter().all(|x| x.is_finite()));
    assert!(wet.peak > 1e-3, "it sounds");
    assert_ne!(wet.samples, dry.samples, "the reverb is heard, not skipped");
    // And the same samples on a second render: nothing about it depends on the menus.
    let again = {
        let snap = Arc::new(Snapshot::compile(&back, 0));
        render(snap, &RenderOptions::default()).expect("it renders")
    };
    assert_eq!(wet.samples, again.samples);
}
