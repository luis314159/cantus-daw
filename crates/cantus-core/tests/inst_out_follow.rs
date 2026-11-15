//! An instrument with named outputs re-routed (round 40 phase 3, the Pulsus-only path
//! `with_kit_strips_following` generalised): the strips *Each to its own strip* made for
//! Tympanum's mics follow the kit to its new strip, in the same undo step, as a Pulsus
//! kit's pad strips do.

use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::inst_out::{plan, OutputPlan};
use cantus_core::*;

#[test]
fn a_tympanums_mic_strips_follow_it_to_a_new_strip_in_one_undo_step() {
    let mut bus = CommandBus::default();
    let mut p = bus.project().clone();
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some("model:studio".into())).unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let id = t.id;
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t),
        },
        Origin::User,
    )
    .unwrap();
    // Every mic to its own strip.
    let now = bus.project().clone();
    let mut fresh = now.clone();
    let cmd = plan(&now, id, OutputPlan::EachOwn, &mut |n| fresh.make_strip(n)).unwrap();
    bus.reserve_ids(fresh.next_id);
    bus.execute(cmd, Origin::User).unwrap();
    let split = bus.project().clone();
    let mics = split.tracks[0].instrument.output_strips();
    assert!(mics.len() >= 10);
    let before = split.tracks[0].strip;
    assert!(mics
        .iter()
        .all(|s| split.strip(*s).unwrap().output == before));
    // The kit to a strip of its own: its mics' strips go with it.
    let mut q = split.clone();
    let bus_strip = q.make_strip("Drum bus");
    let b = bus_strip.id;
    bus.reserve_ids(q.next_id);
    bus.execute(
        Command::InsertStrip {
            index: split.strips.len(),
            strip: Box::new(bus_strip),
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(Command::SetTrackStrip { id, strip: Some(b) }, Origin::User)
        .unwrap();
    let moved = bus.project().clone();
    assert!(
        mics.iter()
            .all(|s| moved.strip(*s).unwrap().output == Some(b)),
        "every mic strip plays into the kit's new strip"
    );
    // One undo brings the kit and its mics back.
    bus.undo();
    let back = bus.project().clone();
    assert_eq!(back.tracks[0].strip, before);
    assert!(mics
        .iter()
        .all(|s| back.strip(*s).unwrap().output == before));
}
