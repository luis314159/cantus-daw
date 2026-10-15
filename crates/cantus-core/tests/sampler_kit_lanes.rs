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
