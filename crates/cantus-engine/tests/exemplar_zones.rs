//! Exemplar's zone table sized to the instrument (round 40 phase 3, ADR 0125; the
//! coordinator's decision): an instrument with more zones than every unit holds is played
//! by a unit built that size off the audio thread and matched by the snapshot; a small one
//! keeps the unit it always had.

use cantus_core::inst::sampler::{self as model, BASE_ZONES, CAPACITY_KEY};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks};
use cantus_engine::inst::UnitKind;
use cantus_engine::{render, RenderOptions};
use std::sync::Arc;

/// `n` zones of a built-in sound: zone `loud` alone on key 60, the rest on key 0.
fn spec(n: usize, loud: usize) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for z in 0..n {
        let key = if z == loud { 60.0 } else { 0.0 };
        s.set(&format!("z{z}.gen"), 1.0).unwrap();
        s.set(&format!("z{z}.lo"), key).unwrap();
        s.set(&format!("z{z}.hi"), key).unwrap();
        s.set(&format!("z{z}.root"), 60.0).unwrap();
    }
    s
}

fn project(spec: NativeSpec) -> Project {
    let mut p = Project::new("zones");
    let mut t = p.make_track("Big");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("c", Ticks::ZERO, Ticks(1920));
    let note = p.make_note(Ticks::ZERO, Ticks(960), 60, 100);
    clip.notes.push(note);
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn snap(spec: NativeSpec) -> Snapshot {
    Snapshot::compile(&project(spec), 1)
}

fn peak(s: Snapshot) -> f32 {
    let r = render(
        Arc::new(s),
        &RenderOptions {
            tail_seconds: 0.0,
            end: Some(Ticks(960)),
            ..Default::default()
        },
    )
    .unwrap();
    r.samples.iter().fold(0f32, |m, x| m.max(x.abs()))
}

#[test]
fn a_small_instrument_keeps_the_unit_it_always_had() {
    let s = snap(spec(40, 30));
    let n = s.tracks[0].instrument.native().unwrap();
    assert!(!n.params.contains_key(CAPACITY_KEY), "nothing stamped");
    assert_eq!(
        UnitKind::of(&s.tracks[0].instrument),
        Some(UnitKind::Native(InstrumentKind::Sampler))
    );
    assert!(peak(s) > 0.01);
}

#[test]
fn a_large_instrument_gets_a_unit_its_size_and_plays_its_last_zones() {
    let n = 1_500;
    let s = snap(spec(n, 1_400));
    assert_eq!(
        s.tracks[0]
            .instrument
            .native()
            .unwrap()
            .params
            .get(CAPACITY_KEY),
        Some(&2048.0)
    );
    assert_eq!(
        UnitKind::of(&s.tracks[0].instrument),
        Some(UnitKind::Exemplar {
            zones: 2048,
            delays: 0,
            room: false
        })
    );
    // The unit built for it holds every zone, and zone 1400 sounds.
    let units = cantus_engine::inst::units_for(&s, 48_000.0, |_, _| false);
    assert_eq!(units.len(), 1);
    assert_eq!(
        units[0].kind,
        UnitKind::Exemplar {
            zones: 2048,
            delays: 0,
            room: false
        }
    );
    assert!(peak(s) > 0.01, "a zone past the base table plays");
    // And a unit already there of that size is not built again; one of the base size is.
    let k = UnitKind::Exemplar {
        zones: 2048,
        delays: 0,
        room: false,
    };
    assert!(
        cantus_engine::inst::units_for(&snap(spec(n, 1_400)), 48_000.0, |_, have| have == k)
            .is_empty()
    );
    assert_eq!(
        cantus_engine::inst::units_for(&snap(spec(n, 1_400)), 48_000.0, |_, have| {
            have == UnitKind::Native(InstrumentKind::Sampler)
        })
        .len(),
        1
    );
    assert_eq!(model::capacity_needed(&spec(BASE_ZONES, 0)), BASE_ZONES);
    assert_eq!(model::capacity_needed(&spec(BASE_ZONES + 1, 0)), 2048);
}
