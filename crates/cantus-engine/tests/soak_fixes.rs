//! Two findings of the soak builder (round 22, `docs/round22/soak.md`), fixed in the
//! engine: an effect removed and brought back at once by an undo still has its unit (E1),
//! and a count-in clicks whatever the metronome switch says (E6).

mod support;
use cantus_core::*;
use cantus_engine::*;
use support::*;

/// A remove and its undo sent before the engine handed the removed unit back: the undo's
/// snapshot carries a fresh unit, so nothing is dropped and the effect is there.
#[test]
fn an_effect_removed_and_restored_at_once_keeps_a_unit() {
    let (mut handle, mut p) = EngineHandle::manual(48_000.0);
    let mut project = demo_project();
    let id = add_effect(&mut project, EffectKind::Saturator);
    let with = handle.compile(&project, 1);
    handle.set_snapshot(with);
    let mut buf = vec![0f32; 256];
    p.process(&mut buf, 2);
    handle.poll();
    let dropped = p.stats().dropped_events;
    // Removed and restored before the engine runs a block (so before it hands back).
    let strip = project.tracks[0].strip.unwrap();
    let slot = project.strip_mut(strip).unwrap().effects.pop().unwrap();
    let without = handle.compile(&project, 2);
    handle.set_snapshot(without);
    assert!(!handle.knows_effect(id), "forgotten as the snapshot without it goes");
    project.strip_mut(strip).unwrap().effects.push(slot);
    let again = handle.compile(&project, 3);
    handle.set_snapshot(again);
    assert!(handle.knows_effect(id));
    for _ in 0..4 {
        p.process(&mut buf, 2);
        handle.poll();
    }
    assert_eq!(p.stats().dropped_events, dropped, "no snapshot found its unit missing");
    assert!(handle.knows_effect(id), "the old unit's return did not forget the new one");
}

/// With the metronome off, a count-in still clicks — and only until the recording's start.
#[test]
fn a_count_in_clicks_with_the_metronome_off() {
    let (mut handle, mut p) = EngineHandle::manual(48_000.0);
    let mut project = Project::new("count-in");
    project.tempo_bpm = 120.0;
    project.metronome = false;
    let snap = handle.compile(&project, 1);
    handle.set_snapshot(snap);
    handle.send(EngineMessage::SetRecordStart(Some(Ticks::beats(2))));
    handle.send(EngineMessage::Seek(Ticks::ZERO));
    handle.send(EngineMessage::Play);
    // Two beats at 120 BPM are one second; play two seconds.
    let mut out = Vec::new();
    let mut buf = vec![0f32; 512];
    for _ in 0..(96_000 / 256) {
        p.process(&mut buf, 2);
        handle.poll();
        out.extend_from_slice(&buf);
    }
    let first = out[..48_000 * 2].iter().fold(0f32, |m, x| m.max(x.abs()));
    let after = out[(48_000 + 4_800) * 2..].iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(first > 0.05, "the count-in clicked ({first})");
    assert_eq!(after, 0.0, "nothing after the count-in with the metronome off");
}
