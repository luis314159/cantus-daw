use super::*;

#[test]
fn the_shipped_kit_is_sorted_by_pitch_and_chokes_its_hats() {
    for kit in [PulsusParams::default(), PulsusParams::studio()] {
        assert!(
            kit.pads.windows(2).all(|w| w[0].pitch < w[1].pitch),
            "pads must be sorted by pitch and unique: the rack and the piano roll both \
             draw them in this order"
        );
        let closed = kit.pads.iter().find(|p| p.pitch == 42).expect("closed hat");
        let open = kit.pads.iter().find(|p| p.pitch == 46).expect("open hat");
        assert_ne!(closed.choke, 0, "the hats must share a choke group");
        assert_eq!(closed.choke, open.choke);
        // Nothing else may join that group by accident: a kick cut by a hat is a bug you
        // would blame on the sequencer for a week.
        for pad in &kit.pads {
            if ![42, 44, 46].contains(&pad.pitch) {
                assert_eq!(pad.choke, 0, "{} is in a choke group", pad.name);
            }
        }
        assert_eq!(kit.version, PULSUS_MODEL, "a kit made now is Pulsus II");
    }
}

#[test]
fn every_pad_field_round_trips_and_clamps() {
    for voice in PadVoice::ALL {
        let mut pad = DrumPad::new(36, "p", voice);
        for field in PadField::for_voice(voice) {
            let info = field.info(voice);
            assert!(
                info.min <= info.default && info.default <= info.max,
                "{voice:?} {field:?}"
            );
            if field == PadField::Voice {
                // Setting the voice changes which ranges apply, so it is checked on its
                // own: the index round-trips and out-of-range clamps to the last.
                let last = (PadVoice::ALL.len() - 1) as f32;
                assert_eq!(field.set(&mut pad, 99.0), last);
                assert_eq!(pad.voice, PadVoice::Tom);
                let back = voice.index() as f32;
                assert_eq!(field.set(&mut pad, back), back);
                assert_eq!(pad.voice, voice);
                continue;
            }
            assert_eq!(
                field.set(&mut pad, info.max + 1_000.0),
                info.max,
                "{field:?}"
            );
            assert_eq!(field.get(&pad), info.max, "{field:?}");
            assert_eq!(field.set(&mut pad, f32::NAN), info.default, "{field:?}");
            assert_eq!(field.get(&pad), info.default, "{field:?}");
        }
    }
}

/// A blank pad's Pulsus II fields are exactly the defaults the field table declares, so
/// "at its default" means one thing wherever it is asked.
#[test]
fn a_blank_pad_sits_on_every_new_default() {
    for voice in PadVoice::ALL {
        let pad = DrumPad::new(36, "p", voice);
        for field in PadField::ALL[PadField::CLASSIC.len()..].iter().copied() {
            if field.applies_to(voice) {
                assert_eq!(
                    field.get(&pad),
                    field.info(voice).default,
                    "{voice:?} {field:?}"
                );
            }
        }
    }
}

/// Every shipped pad must sit inside the ranges its own voice declares, or the inspector
/// would show a slider already pinned to an end stop.
#[test]
fn the_shipped_kits_stay_inside_their_own_ranges() {
    let factory = PulsusParams::factory();
    assert!(factory.len() >= 8, "fewer than eight factory kits");
    for kit in factory.into_iter().map(|f| f.kit) {
        assert!(kit.pads.windows(2).all(|w| w[0].pitch < w[1].pitch));
        assert!(kit.pads.len() <= 32);
        for pad in kit.pads {
            for field in PadField::for_voice(pad.voice) {
                let info = field.info(pad.voice);
                let v = field.get(&pad);
                assert!(
                    v >= info.min && v <= info.max,
                    "{}: {field:?} is {v}, outside {}..{}",
                    pad.name,
                    info.min,
                    info.max
                );
            }
        }
    }
}

/// The promise the version makes: a kit Pulsus I wrote has no `version`, no ids and none
/// of the new fields, and it reads as model 1 with every new field at its neutral value
/// and ids in pitch order.
#[test]
fn a_pulsus_one_kit_reads_as_model_one_with_ids_in_pitch_order() {
    let mut old = serde_json::to_value(PulsusParams::default()).unwrap();
    let obj = old.as_object_mut().unwrap();
    obj.remove("version");
    obj.remove("kit");
    for pad in obj["pads"].as_array_mut().unwrap() {
        let pad = pad.as_object_mut().unwrap();
        let classic = [
            "pitch",
            "name",
            "voice",
            "tune",
            "decay",
            "pitch_drop",
            "pitch_decay",
            "click",
            "drive",
            "noise_decay",
            "tone",
            "mix",
            "hp_cutoff",
            "level_db",
            "pan",
            "choke",
            "muted",
            "sample",
        ];
        pad.retain(|k, _| classic.contains(&k.as_str()));
    }
    let kit: PulsusParams = serde_json::from_value(old).unwrap();
    assert_eq!(kit.version, 1);
    assert_eq!(kit.kit, KitSettings::default());
    let ids: Vec<u16> = kit.pads.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, (1..=kit.pads.len() as u16).collect::<Vec<_>>());
    let fresh = PulsusParams::default();
    for (a, b) in kit.pads.iter().zip(&fresh.pads) {
        assert_eq!(a, b, "{} did not read back as it was", a.name);
    }
    // And it round-trips as model 1: saving does not upgrade it.
    let again: PulsusParams = serde_json::from_str(&serde_json::to_string(&kit).unwrap()).unwrap();
    assert_eq!(again, kit);
}

#[test]
fn ids_are_unique_stable_and_repaired_on_load() {
    let mut kit = PulsusParams::default();
    let snare = kit.pads[2].id;
    let hat = kit.pads[3].id;
    // A lower pad arrives: the indices move, the ids do not.
    let mut low = DrumPad::kick();
    low.pitch = 35;
    low.id = kit.next_id();
    let at = kit.pads.partition_point(|p| p.pitch < 35);
    kit.pads.insert(at, low);
    assert_eq!(kit.by_id(snare).unwrap().pitch, 38);
    assert_eq!(kit.index_of(hat), Some(4));
    // Two pads claiming one id: the second is given a fresh one when the kit is read.
    let mut v = serde_json::to_value(&kit).unwrap();
    v["pads"][1]["id"] = v["pads"][0]["id"].clone();
    let back: PulsusParams = serde_json::from_value(v).unwrap();
    let mut ids: Vec<u16> = back.pads.iter().map(|p| p.id.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), back.pads.len());
}

#[test]
fn velocity_zones_cross_fade_at_equal_power() {
    let mut lo = PadLayer::new("a.wav");
    lo.vel_hi = 64;
    lo.xfade = 16;
    let mut hi = PadLayer::new("b.wav");
    hi.vel_lo = 65;
    hi.xfade = 16;
    for v in 1..=127 {
        let (a, b) = (lo.velocity_gain(v as f32), hi.velocity_gain(v as f32));
        assert!(
            ((a * a + b * b) - 1.0).abs() < 1e-5,
            "power {} at velocity {v}",
            a * a + b * b
        );
    }
    assert_eq!(lo.velocity_gain(20.0), 1.0);
    assert_eq!(hi.velocity_gain(20.0), 0.0);
    // No fade width: a hard switch.
    let mut hard = PadLayer::new("c.wav");
    hard.vel_lo = 100;
    assert_eq!(hard.velocity_gain(99.0), 0.0);
    assert_eq!(hard.velocity_gain(100.0), 1.0);
}

#[test]
fn kit_fields_clamp_and_round() {
    let mut kit = PulsusParams::default();
    assert_eq!(KitField::Seed.set(&mut kit, 77.4), 77.0);
    assert_eq!(kit.kit.seed, 77);
    assert_eq!(KitField::Version.set(&mut kit, 9.0), PULSUS_MODEL as f32);
    assert_eq!(KitField::HatCc.set(&mut kit, f32::NAN), 4.0);
    for f in KitField::ALL {
        let v = f.get(&kit);
        assert_eq!(f.set(&mut kit, v), v, "{f:?} does not round-trip");
    }
}

#[test]
fn upgrading_a_model_one_kit_takes_back_the_drive_it_never_heard() {
    let mut kit = PulsusParams::default();
    assert!(
        kit.upgrade_edits().is_empty(),
        "a model 2 kit has nothing to upgrade"
    );
    kit.version = 1;
    kit.pads[2].drive = 0.2; // Pulsus I stored 0.2 on every pad
    let edits = kit.upgrade_edits();
    assert_eq!(edits, vec![(kit.pads[2].id, PadField::Drive, 0.0)]);
}

#[test]
fn chromatic_lanes_are_the_keyboard_they_replaced() {
    let l = Lanes::Chromatic;
    assert!(l.is_chromatic());
    assert_eq!(l.count(), 128);
    for pitch in 0u8..128 {
        assert_eq!(l.lane_of(pitch), Some(pitch as usize));
        assert_eq!(l.pitch(pitch as usize), Some(pitch));
        assert_eq!(l.name(pitch as usize), crate::model::pitch_name(pitch));
    }
    assert_eq!(l.pitch(128), None);
    assert_eq!(l.pad_id(3), None);
}

#[test]
fn pad_lanes_name_the_drum_and_ignore_pitches_the_kit_has_no_pad_for() {
    let kit = PulsusParams::default();
    let l = Lanes::Pads(&kit.pads);
    assert!(!l.is_chromatic());
    assert_eq!(l.count(), kit.pads.len());
    assert_eq!(l.pitch(0), Some(36));
    assert_eq!(l.name(0), "Kick");
    assert_eq!(l.lane_of(38).map(|i| l.name(i)).as_deref(), Some("Snare"));
    assert_eq!(l.lane_of(60), None, "middle C is not a drum");
    for lane in 0..l.count() {
        let pitch = l.pitch(lane).unwrap();
        assert_eq!(l.lane_of(pitch), Some(lane));
        let id = l.pad_id(lane).unwrap();
        assert_eq!(l.lane_of_pad(id), Some(lane));
    }
}
