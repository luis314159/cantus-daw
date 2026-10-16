//! An Exemplar that is a drum kit (round 28: a pack's acoustic kit) shows its drums as
//! rows — in the piano roll and the channel rack, as Pulsus's pads do: one row per key its
//! zones play, named by the zone's group, else by General MIDI's name for the key.

use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};

fn kit(flag: f32) -> InstrumentSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    let mut put = |n: usize, lo: u8, hi: u8, group: usize| {
        let mut z = Zone::new(Some(format!("C:/kit/{n}.wav")), lo, hi, lo);
        z.set(zf::GROUP, group as f32).set(zf::KEYTRACK, 0.0);
        for (k, v) in z.values_for(n) {
            s.set(&k, v).unwrap();
        }
        for (k, t) in z.texts_for(n) {
            s.set_text(&k, Some(t)).unwrap();
        }
    };
    put(0, 36, 36, 0);
    put(1, 36, 36, 0); // a second hit of the kick: the same row
    put(2, 47, 48, 1);
    put(3, 38, 38, 2);
    put(4, 64, 64, 3);
    // A pedal noise a controller starts: no key plays it, so it has no row.
    let mut pedal = Zone::new(Some("C:/kit/pedal.wav".into()), 70, 70, 70);
    pedal.set(zf::TRIG, 5.0);
    for (k, v) in pedal.values_for(5) {
        s.set(&k, v).unwrap();
    }
    for (k, t) in pedal.texts_for(5) {
        s.set_text(&k, Some(t)).unwrap();
    }
    s.set_text("g0.name", Some("Kick".into())).unwrap();
    s.set_text("g1.name", Some("Tom 1".into())).unwrap();
    s.set("kit", flag).unwrap();
    InstrumentSpec::Native(s)
}

#[test]
fn a_kit_exemplar_has_a_row_per_key_named_by_its_drum() {
    let spec = kit(1.0);
    let l = spec.lanes();
    assert!(!l.is_chromatic());
    let rows: Vec<(u8, String)> = (0..l.count())
        .map(|i| (l.pitch(i).unwrap(), l.name(i)))
        .collect();
    assert_eq!(
        rows,
        [
            (36, "Kick".to_string()),
            (38, "Acoustic Snare".to_string()),
            (47, "Tom 1 \u{b7} B2".to_string()),
            (48, "Tom 1 \u{b7} C3".to_string()),
            (64, "Low Conga".to_string()),
        ],
        "groups' names first, General MIDI's for an unnamed group; a drum on several keys \
         tells them apart by their pitch"
    );
    assert_eq!(l.lane_of(47), Some(2));
    assert_eq!(l.lane_of(70), None, "the pedal noise has no key");
    assert_eq!(l.pad_id(0), None, "rows, not pads");
}

#[test]
fn an_exemplar_that_is_not_a_kit_is_a_keyboard() {
    assert!(kit(0.0).lanes().is_chromatic());
    let plain = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler));
    assert!(plain.lanes().is_chromatic(), "the default: a keyboard");
    assert_eq!(
        cantus_core::inst::sampler::param("kit").unwrap().default,
        0.0
    );
}

#[test]
fn general_midi_names_every_percussion_key() {
    use cantus_core::inst::sampler::gm_drum_name;
    assert_eq!(gm_drum_name(35), Some("Acoustic Bass Drum"));
    assert_eq!(gm_drum_name(42), Some("Closed Hi-Hat"));
    assert_eq!(gm_drum_name(81), Some("Open Triangle"));
    assert_eq!(gm_drum_name(34), None);
    assert_eq!(gm_drum_name(82), None);
    assert!((35..=81).all(|k| gm_drum_name(k).is_some()));
}

#[test]
fn a_zone_whose_high_key_is_below_its_low_one_has_a_row_and_never_crashes() {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    let z = Zone::new(Some("C:/kit/x.wav".into()), 72, 60, 72);
    for (k, v) in z.values_for(0) {
        s.set(&k, v).unwrap();
    }
    for (k, t) in z.texts_for(0) {
        s.set_text(&k, Some(t)).unwrap();
    }
    s.set("kit", 1.0).unwrap();
    let spec = InstrumentSpec::Native(s);
    let l = spec.lanes();
    assert_eq!((l.count(), l.pitch(0)), (1, Some(72)), "the low key alone");
}

/// A kit with a tom on 47-48 and on 50 (the same three hits on both), and a hi-hat on 42
/// in two velocity layers of two hits each — the fill and the accents of a real part.
fn turns_kit() -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    let mut n = 0;
    let mut put =
        |s: &mut NativeSpec, sample: &str, lo: u8, hi: u8, vlo: u8, vhi: u8, seq: u8, len: u8| {
            let mut z = Zone::new(Some(format!("C:/kit/{sample}.wav")), lo, hi, lo);
            z.set(zf::VLO, vlo as f32)
                .set(zf::VHI, vhi as f32)
                .set(zf::SEQ, seq as f32)
                .set(zf::SEQLEN, len as f32);
            for (k, v) in z.values_for(n) {
                s.set(&k, v).unwrap();
            }
            for (k, t) in z.texts_for(n) {
                s.set_text(&k, Some(t)).unwrap();
            }
            n += 1;
        };
    for (lo, hi) in [(47, 48), (50, 50)] {
        for i in 1..=3 {
            put(&mut s, &format!("tom{i}"), lo, hi, 0, 127, i, 3);
        }
    }
    for i in 1..=2 {
        put(&mut s, &format!("hat_soft{i}"), 42, 42, 0, 90, i, 2);
        put(&mut s, &format!("hat_accent{i}"), 42, 42, 91, 127, i, 2);
    }
    s.set("kit", 1.0).unwrap();
    s
}

#[test]
fn turns_are_counted_by_drum_and_velocity_layer_not_by_key() {
    use cantus_core::{Project, Snapshot, Ticks};
    let mut p = Project::new("turns");
    let mut t = p.make_track("Kit");
    t.instrument = InstrumentSpec::Native(turns_kit());
    let mut clip = p.make_clip("fill", Ticks::ZERO, Ticks::beats(16));
    let sixteenth = Ticks::QUARTER.0 / 4;
    // A fill down the toms, twice: 50, 48, 47, 50, 48, 47.
    for (i, k) in [50u8, 48, 47, 50, 48, 47].iter().enumerate() {
        let n = p.make_note(Ticks(i as i64 * sixteenth), Ticks(sixteenth), *k, 100);
        clip.notes.push(n);
    }
    // Sixteenth hats from beat 4, an accent on every fourth.
    for i in 0..16i64 {
        let vel = if i % 4 == 0 { 120 } else { 60 };
        let n = p.make_note(
            Ticks::beats(4) + Ticks(i * sixteenth),
            Ticks(sixteenth),
            42,
            vel,
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    let snap = Snapshot::compile(&p, 1);
    let ons: Vec<(u8, u8, u32)> = snap.tracks[0]
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| (e.pitch, e.velocity, e.ordinal))
        .collect();
    let toms: Vec<u32> = ons.iter().filter(|e| e.0 != 42).map(|e| e.2).collect();
    assert_eq!(toms, [0, 1, 2, 3, 4, 5], "one tom, whatever its key");
    let accents: Vec<u32> = ons
        .iter()
        .filter(|e| e.0 == 42 && e.1 > 90)
        .map(|e| e.2)
        .collect();
    assert_eq!(
        accents,
        [0, 1, 2, 3],
        "the accent layer takes its own turns"
    );
    let soft: Vec<u32> = ons
        .iter()
        .filter(|e| e.0 == 42 && e.1 <= 90)
        .map(|e| e.2)
        .collect();
    assert_eq!(soft, (0..12).collect::<Vec<u32>>());
}

/// Sound review 2: a lane is the sound a zone plays, not only its file — two generated
/// drums (no file: a kick's generator on 36, a snare's on 38) and two slices of one file
/// count their turns apart. Played K S K S, each alternates.
#[test]
fn generated_drums_and_slices_of_one_file_take_their_own_turns() {
    use cantus_core::{Project, Snapshot, Ticks};
    let kit = |sliced: bool| {
        let mut s = NativeSpec::new(InstrumentKind::Sampler);
        let mut n = 0;
        for (key, which) in [(36u8, 1.0f32), (38, 2.0)] {
            for seq in 1..=2 {
                let file = sliced.then(|| "C:/kit/loop.wav".to_string());
                let mut z = Zone::new(file, key, key, key);
                if sliced {
                    z.set(zf::START, which * 1000.0)
                        .set(zf::END, which * 1000.0 + 900.0);
                } else {
                    z.set(zf::GEN, which);
                }
                z.set(zf::SEQ, seq as f32).set(zf::SEQLEN, 2.0);
                for (k, v) in z.values_for(n) {
                    s.set(&k, v).unwrap();
                }
                for (k, t) in z.texts_for(n) {
                    s.set_text(&k, Some(t)).unwrap();
                }
                n += 1;
            }
        }
        s.set("kit", 1.0).unwrap();
        s
    };
    for sliced in [false, true] {
        let mut p = Project::new("turns");
        let mut t = p.make_track("Kit");
        t.instrument = InstrumentSpec::Native(kit(sliced));
        let mut clip = p.make_clip("ks", Ticks::ZERO, Ticks::beats(4));
        for (i, k) in [36u8, 38, 36, 38].iter().enumerate() {
            let n = p.make_note(Ticks::beats(i as i64), Ticks::QUARTER, *k, 100);
            clip.notes.push(n);
        }
        t.clips.push(clip);
        p.tracks.push(t);
        let snap = Snapshot::compile(&p, 1);
        let turns = |key: u8| -> Vec<u32> {
            snap.tracks[0]
                .events
                .iter()
                .filter(|e| e.is_on() && e.pitch == key)
                .map(|e| e.ordinal)
                .collect()
        };
        assert_eq!(turns(36), [0, 1], "sliced {sliced}: the kick alternates");
        assert_eq!(turns(38), [0, 1], "sliced {sliced}: the snare alternates");
    }
}
