//! The stage's master tap (round 35, ADR 0091 §4) on the audio thread: copying the master
//! into its ring while a reader has it on — playing, through a full ring, turned off and on
//! again — never allocates or frees.

use super::*;

#[test]
fn the_master_tap_never_allocates() {
    let mut p = demo_project();
    p.tempo_bpm = 120.0;
    let mut rig = Rig::new();
    let (tap, mut reader) = cantus_engine::master_tap::pair();
    rig.p.attach_master_tap(tap);
    rig.snapshot(&p);
    rig.tx.push(EngineMessage::Play).unwrap();
    rig.play("tap off", 10);
    reader.set_on(true);
    let mut seen = 0usize;
    for _ in 0..20 {
        rig.play("tap on, drained", 1);
        seen += reader.drain(|_| {});
    }
    assert!(seen >= 20 * 512, "the master came through ({seen} frames)");
    // Not drained: the ring fills and drops what does not fit.
    rig.play("tap on, full", 64);
    assert!(reader.lost() > 0, "a full ring drops and counts");
    reader.set_on(false);
    rig.play("tap off again", 10);
    reader.set_on(true);
    rig.play("tap on again", 10);
    assert!(
        rig.allocations.iter().all(|(_, n)| *n == 0),
        "the audio thread allocated or freed: {:?}",
        rig.allocations
    );
}
