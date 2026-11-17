//! The user's own starting presets (ADR 0138, provisional): a unit's chosen start is what
//! every insert path starts it on — `starting::apply` (the picker, the agents' tools, a
//! pad's chain) and `starting::starting_spec` (Tabula's *Add pedal*) — a reset brings the
//! factory's back, a hosted plug-in has none, and the choices at start replace the old ones.
//! Its own test binary: the choices are the process's. // fx3:start

use cantus_core::fx::tabula::guitar_rig;
use cantus_core::starting::{apply, starting_spec, USER_START};
use cantus_core::starting_user::{self, UserStart};
use cantus_core::{EffectKind, EffectSpec};

fn mono_fifth() -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::PitchShift);
    for (k, v) in [("st1", 7.0), ("lvl1", 0.5), ("dry", 1.0)] {
        s.set(k, v);
    }
    s
}

#[test]
fn a_chosen_start_is_every_inserts_and_a_reset_brings_the_factorys_back() {
    let factory = starting_spec(EffectKind::PitchShift);
    assert_eq!(
        factory.get("engine"),
        1.0,
        "the factory start is the poly twelve-string"
    );

    starting_user::set(mono_fifth(), "Fifth up, live".into());
    let mut s = EffectSpec::new(EffectKind::PitchShift);
    assert_eq!(apply(&mut s), Some(USER_START));
    assert_eq!(s, mono_fifth(), "apply: the person's start");
    assert_eq!(
        starting_spec(EffectKind::PitchShift),
        mono_fifth(),
        "Add pedal"
    );
    // Another unit keeps its factory start.
    let mut d = EffectSpec::new(EffectKind::Delay);
    apply(&mut d);
    assert_eq!(d, starting_spec(EffectKind::Delay));
    assert_eq!(
        starting_user::get(EffectKind::PitchShift).map(|u| u.label),
        Some("Fifth up, live".to_string())
    );

    // A unit that starts neutral at the factory can be given a start too: a board too.
    let mut rig = guitar_rig();
    rig.set("mix", 0.5);
    starting_user::set(rig.clone(), "My rig".into());
    assert_eq!(starting_spec(EffectKind::Board), rig);

    // A hosted plug-in starts as its maker made it.
    starting_user::set(EffectSpec::new(EffectKind::External), "no".into());
    assert!(starting_user::get(EffectKind::External).is_none());

    starting_user::clear(EffectKind::PitchShift);
    assert_eq!(starting_spec(EffectKind::PitchShift), factory, "reset");

    // The settings at start replace every choice (the last of a kind wins).
    starting_user::replace_all([
        UserStart {
            spec: EffectSpec::new(EffectKind::PitchShift),
            label: "a".into(),
        },
        UserStart {
            spec: mono_fifth(),
            label: "b".into(),
        },
    ]);
    assert_eq!(starting_user::all().len(), 1, "the board's choice is gone");
    assert_eq!(starting_spec(EffectKind::PitchShift), mono_fifth());
    starting_user::replace_all([]);
    assert_eq!(starting_spec(EffectKind::Board), guitar_rig());
}
