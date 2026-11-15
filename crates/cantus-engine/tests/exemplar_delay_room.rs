//! Exemplar's output delay and its added room (round 40 phase 3, ADR 0125 §5; `chorda-src`):
//! a unit owns a delay line only for an output whose `o{n}.delay` is not 0 and the room only
//! when `room.add` names one (the snapshot says so: `UnitKind::Exemplar`); a delay shifts
//! its lane by whole samples at any block size, the tail rung out after the notes; the room
//! rings after the notes and, off, touches no sample.

use cantus_core::inst_out::{key, OutputFamily, OutputField, OutputId, OutputSlot};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, StripId};
use cantus_engine::inst::{self as einst, sampler, InstrumentUnit, UnitKind};

/// Built-in sounds in groups 0 and 1 on outputs 0 and 1; output 1 on a strip of its own.
fn spec() -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (n, gen) in [1.0f32, 2.0].into_iter().enumerate() {
        s.set(&format!("z{n}.gen"), gen);
        s.set(&format!("z{n}.group"), n as f32);
        s.set(&format!("z{n}.gain"), -6.0);
        s.set_text(&format!("z{n}.name"), Some(format!("z{n}")))
            .unwrap();
    }
    s.set("g1.out", 1.0);
    s.outputs.push(OutputSlot {
        id: OutputId(1),
        name: "o1".into(),
        family: OutputFamily::Perspective,
        strip: StripId(11),
    });
    s
}

/// The unit the snapshot asks for, configured, three notes held until `off` frames.
fn unit(spec: &NativeSpec) -> Box<dyn InstrumentUnit> {
    let mut u: Box<dyn InstrumentUnit> =
        match UnitKind::of(&InstrumentSpec::Native(spec.clone())).unwrap() {
            UnitKind::Exemplar {
                zones,
                delays,
                room,
            } => Box::new(sampler::Unit::with(48_000.0, zones as usize, delays, room)),
            UnitKind::Native(k) => einst::make(k, 48_000.0),
            UnitKind::Hosted(_) | UnitKind::Drums { .. } => unreachable!(), // r40 integration: tymp-engine's Tympanum unit
        };
    u.configure(spec, &[]);
    for (k, id) in [(48u8, 1u64), (60, 2), (67, 3)] {
        u.note_on(einst::NoteOn {
            key: k,
            velocity: 0.8,
            id,
            channel: 0,
        });
    }
    u
}

/// `frames` in blocks of `block` (the notes let go at `off`): main and bus 1, interleaved.
fn run(u: &mut dyn InstrumentUnit, frames: usize, block: usize, off: usize) -> [Vec<f32>; 2] {
    let mut out = [vec![0.0f32; 2 * frames], vec![0.0f32; 2 * frames]];
    let mut at = 0;
    while at < frames {
        let n = block.min(frames - at);
        if at <= off && off < at + n {
            for (k, id) in [(48u8, 1u64), (60, 2), (67, 3)] {
                u.note_off(k, id, 0.5);
            }
        }
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        let mut tap = (vec![0.0f32; n], vec![0.0f32; n]);
        u.process_buses(&mut l, &mut r, &mut |b, o, a, c| {
            assert_eq!(b, 1);
            for i in 0..a.len() {
                tap.0[o + i] += a[i];
                tap.1[o + i] += c[i];
            }
        });
        for i in 0..n {
            out[0][2 * (at + i)] = l[i];
            out[0][2 * (at + i) + 1] = r[i];
            out[1][2 * (at + i)] = tap.0[i];
            out[1][2 * (at + i) + 1] = tap.1[i];
        }
        at += n;
    }
    out
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| (v + 0.0).to_bits()).collect()
}

#[test]
fn a_unit_owns_lines_and_a_room_only_when_its_spec_asks() {
    let mut s = spec();
    let kind = |s: &NativeSpec| UnitKind::of(&InstrumentSpec::Native(s.clone())).unwrap();
    assert_eq!(kind(&s), UnitKind::Native(InstrumentKind::Sampler));
    s.set(&key(OutputId(1), OutputField::Delay), 0.0);
    s.set("room.add", 0.0);
    assert_eq!(
        kind(&s),
        UnitKind::Native(InstrumentKind::Sampler),
        "zeros ask nothing"
    );
    s.set(&key(OutputId(1), OutputField::Delay), 2.5);
    assert_eq!(
        kind(&s),
        UnitKind::Exemplar {
            zones: 1024,
            delays: 0b10,
            room: false
        }
    );
    s.set("room.add", 2.0);
    assert_eq!(
        kind(&s),
        UnitKind::Exemplar {
            zones: 1024,
            delays: 0b10,
            room: true
        }
    );
}

#[test]
fn a_delay_shifts_its_lane_by_whole_samples_at_any_block() {
    let plain = spec();
    let mut delayed = plain.clone();
    // 2.5 ms at 48 kHz: 120 frames.
    delayed.set(&key(OutputId(1), OutputField::Delay), 2.5);
    let d = 120;
    for block in [512, 333, 64] {
        let a = run(unit(&plain).as_mut(), 8_000, block, 4_000);
        let b = run(unit(&delayed).as_mut(), 8_000, block, 4_000);
        assert!(a[1].iter().any(|x| *x != 0.0), "output 1 plays");
        assert_eq!(bits(&a[0]), bits(&b[0]), "main untouched at {block}");
        assert!(b[1][..2 * d].iter().all(|x| *x == 0.0), "silence first");
        assert_eq!(
            bits(&a[1][..2 * (8_000 - d)]),
            bits(&b[1][2 * d..]),
            "the lane shifted by {d} frames at {block}"
        );
    }
}

#[test]
fn the_added_room_rings_after_the_notes_and_off_is_null() {
    let plain = spec();
    let mut off = plain.clone();
    off.set("room.add", 0.0);
    off.set("room.level", -6.0);
    let a = run(unit(&plain).as_mut(), 30_000, 256, 4_000);
    let b = run(unit(&off).as_mut(), 30_000, 256, 4_000);
    assert_eq!(
        bits(&a[0]),
        bits(&b[0]),
        "a room that is off touches no sample"
    );
    let mut hall = plain.clone();
    hall.set("room.add", 3.0);
    hall.set("room.level", -6.0);
    let c = run(unit(&hall).as_mut(), 30_000, 256, 4_000);
    assert_ne!(bits(&a[0]), bits(&c[0]));
    let late = |x: &[f32]| x[2 * 20_000..].iter().map(|v| v * v).sum::<f32>();
    assert!(late(&c[0]) > 10.0 * late(&a[0]) + 1e-9, "the hall rings on");
    assert!(c[0].iter().all(|x| x.is_finite()));
}
