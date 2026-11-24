//! r42 fix-engine (the mid-round review's correctness 14, ADR 0155): an external instrument's
//! track whose tuning leaves a key unmapped sends no note for that key to the synth — the
//! synth's MTS table skips the key, so it would play its old pitch there — while every mapped
//! key goes out as always. Through the processor's MIDI-out ring, as the out thread reads it.

use cantus_core::external_unit::{ExternalUnit, MidiRoute, Now, Pair};
use cantus_core::tuning::{KeyboardMap, Tuning};
use cantus_core::*;
use cantus_engine::external_presence::ExternalPresence;
use cantus_engine::transport::sync::OutKind;
use cantus_engine::*;

const SR: f64 = 48_000.0;

/// A map that leaves every key but 60 and 62 unmapped.
fn sparse() -> Tuning {
    Tuning {
        map: Some(KeyboardMap {
            size: 12,
            first_key: 0,
            last_key: 127,
            middle_key: 60,
            reference_key: 60,
            reference_hz: 261.625_565_3,
            octave_degree: 12,
            mapping: vec![Some(0), None, Some(2)],
        }),
        ..Tuning::default()
    }
}

fn song(tuning: Tuning) -> Project {
    let mut p = Project::new("outboard");
    p.tempo_bpm = 120.0;
    p.tuning = tuning;
    let mut t = p.make_track("Gear");
    t.strip = None;
    let mut clip = p.make_clip("keys", Ticks::ZERO, Ticks::beats(4));
    for (i, k) in [60u8, 61, 62].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::QUARTER, k, 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    let track = t.id;
    p.tracks.push(t);
    let id = p.new_external_unit_id();
    p.external_units.push(ExternalUnit::instrument(
        id,
        track,
        MidiRoute {
            port: "Synth".into(),
            ordinal: 0,
            channel: 0,
        },
        Pair::stereo(0),
    ));
    p
}

/// The note-ons the processor sends the synth while the song plays, by key.
fn sent_keys(p: &Project) -> Vec<u8> {
    let (mut h, mut proc) = EngineHandle::manual(SR);
    let mut sync = h.take_sync().expect("the manual engine's MIDI-out seam");
    let mut out = sync.out.take().expect("its ring");
    h.set_external_presence(ExternalPresence {
        ready: p.external_units.iter().map(|u| u.id).collect(),
        now: Some(Now {
            device: "Interface".into(),
            rtt: 0,
            buffer: 128,
            rate: SR as u32,
        }),
    });
    let snap = h.compile(p, 1);
    h.set_snapshot(snap);
    h.play();
    let mut buf = vec![0f32; 256];
    let mut keys = Vec::new();
    for _ in 0..(2.0 * SR / 128.0) as usize {
        proc.process(&mut buf, 2);
        h.poll();
        while let Ok(m) = out.pop() {
            if m.kind == OutKind::Note && m.bytes[0] & 0xF0 == 0x90 && m.bytes[2] > 0 {
                keys.push(m.bytes[1]);
            }
        }
    }
    keys
}

#[test]
fn an_unmapped_key_sends_no_note_to_the_synth() {
    assert_eq!(sent_keys(&song(Tuning::default())), [60, 61, 62], "untuned");
    assert!(!sparse().offsets().sounds(61) && sparse().offsets().sounds(62));
    assert_eq!(sent_keys(&song(sparse())), [60, 62], "61 is unmapped");
}
