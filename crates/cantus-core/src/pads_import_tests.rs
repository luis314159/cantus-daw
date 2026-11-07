//! A folder becomes a mapped kit: names in English and Spanish, numbered takes as a round
//! robin, dynamics as velocity layers, the rest by order.

use super::*;

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| format!("C:/kits/house/{n}")).collect()
}

fn pad(k: &PulsusParams, pitch: Pitch) -> &DrumPad {
    k.pads
        .iter()
        .find(|p| p.pitch == pitch)
        .unwrap_or_else(|| panic!("no pad on {pitch}"))
}

#[test]
fn names_in_english_and_spanish_say_the_drum() {
    for (name, role) in [
        ("Kick 01.wav", Role::Kick),
        ("BD_Punchy.wav", Role::Kick),
        ("bombo seco.wav", Role::Kick),
        ("Bass Drum.aif", Role::Kick),
        ("Snare Tight.wav", Role::Snare),
        ("caja.flac", Role::Snare),
        ("redoblante 2.wav", Role::Snare),
        ("Clap.wav", Role::Clap),
        ("palmas.wav", Role::Clap),
        ("Rimshot.wav", Role::Rim),
        ("aro.wav", Role::Rim),
        ("HH Closed.wav", Role::HatClosed),
        ("hihat_open.wav", Role::HatOpen),
        ("charles abierto.wav", Role::HatOpen),
        ("charles cerrado.wav", Role::HatClosed),
        ("Hat Pedal.wav", Role::HatPedal),
        ("Floor Tom.wav", Role::TomLow),
        ("tom grave.wav", Role::TomLow),
        ("Tom Hi.wav", Role::TomHigh),
        ("tom agudo.wav", Role::TomHigh),
        ("Tom.wav", Role::TomMid),
        ("Crash.wav", Role::Crash),
        ("Ride.wav", Role::Ride),
        ("Cowbell.wav", Role::Cowbell),
        ("cencerro.wav", Role::Cowbell),
        ("Shaker.wav", Role::Perc),
        ("pandereta.wav", Role::Perc),
        ("bongó.wav", Role::Perc),
    ] {
        assert_eq!(role_of(name), Some(role), "{name}");
    }
    assert_eq!(role_of("Texture 4.wav"), None);
}

#[test]
fn a_folder_becomes_a_kit_on_the_general_midi_map() {
    let (k, log) = kit_from_files(&files(&[
        "Texture.wav",
        "Snare.wav",
        "Kick 1.wav",
        "Kick 2.wav",
        "Kick 3.wav",
        "HH Closed.wav",
        "HH Open.wav",
        "Clap soft.wav",
        "Clap hard.wav",
        "Tom Low.wav",
        "Crash.wav",
    ]));
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(k.version, crate::pads::PULSUS_MODEL);
    let kick = pad(&k, 36);
    assert_eq!(kick.name, "Kick");
    assert_eq!(kick.layers.len(), 3, "three takes, one pad");
    assert_eq!(kick.round_robin, RoundRobin::Cycle);
    assert!(kick.layers.iter().all(|l| (l.vel_lo, l.vel_hi) == (1, 127)));
    assert_eq!(pad(&k, 38).layers[0].sample, "C:/kits/house/Snare.wav");
    // Dynamics as velocity layers, softest lowest.
    let clap = pad(&k, 39);
    assert_eq!(clap.layers.len(), 2);
    assert!(clap.layers[0].sample.ends_with("Clap soft.wav"));
    assert_eq!((clap.layers[0].vel_lo, clap.layers[1].vel_hi), (1, 127));
    assert!(clap.layers[0].vel_hi < clap.layers[1].vel_lo);
    assert_eq!(clap.round_robin, RoundRobin::Off);
    // The hats choke each other.
    assert_eq!((pad(&k, 42).choke, pad(&k, 46).choke), (1, 1));
    assert_eq!(pad(&k, 41).name, "Tom Low");
    assert_eq!(pad(&k, 49).name, "Crash");
    // A name that says nothing: the next free pad.
    assert_eq!(pad(&k, 60).name, "Texture");
    assert_eq!(k.pads.len(), 8);
    assert!(k.pads.iter().all(|p| p.voice == PadVoice::Sample));
}

#[test]
fn velocity_layers_each_cycle_their_takes() {
    let (k, _) = kit_from_files(&files(&[
        "snare_vl1_rr1.wav",
        "snare_vl1_rr2.wav",
        "snare_vl2_rr1.wav",
        "snare_vl2_rr2.wav",
    ]));
    let s = pad(&k, 38);
    assert_eq!(s.layers.len(), 4);
    assert_eq!(s.round_robin, RoundRobin::Cycle);
    let zones: Vec<(u8, u8)> = s.layers.iter().map(|l| (l.vel_lo, l.vel_hi)).collect();
    assert_eq!(zones[0], zones[1]);
    assert_eq!(zones[2], zones[3]);
    assert!(zones[0].1 < zones[2].0);
}

#[test]
fn what_does_not_fit_is_said() {
    // Forty different sounds (no number: a number would make them one pad's takes).
    let letter = |i: usize| char::from(b'a' + (i % 26) as u8);
    let names: Vec<String> = (0..40)
        .map(|i| format!("Sound {}{}.wav", letter(i / 26), letter(i)))
        .collect();
    let (k, log) = kit_from_files(&names);
    assert_eq!(k.pads.len(), cantus_dsp::MAX_PADS);
    assert_eq!(log.len(), 40 - cantus_dsp::MAX_PADS, "{log:?}");
    let takes: Vec<String> = (1..=20).map(|i| format!("Kick {i}.wav")).collect();
    let (k, log) = kit_from_files(&takes);
    assert_eq!(k.pads[0].layers.len(), MAX_LAYERS);
    assert_eq!(log.len(), 20 - MAX_LAYERS);
}

#[test]
fn an_sfz_kits_regions_become_pads_with_layers_and_chokes() {
    use crate::inst::sampler::{zf, Zone};
    let z = |key: u8, vlo: u8, vhi: u8, sample: &str| {
        let mut z = Zone::new(Some(sample.to_string()), key, key, key);
        z.set(zf::VLO, vlo as f32);
        z.set(zf::VHI, vhi as f32);
        z
    };
    let mut zones = vec![
        z(36, 1, 64, "kick_soft.flac"),
        z(36, 65, 127, "kick_hard.flac"),
        z(42, 1, 127, "hh_closed.flac"),
        z(46, 1, 127, "hh_open.flac"),
    ];
    // SFZ's exclusive group is `group=` (EXCL), one-way through `off_by=`: the closed hat
    // (group 5) cuts the open hat (off_by 5). `zf::GROUP` is the `<group>` header's index,
    // nothing to do with chokes: set on every zone, it must change nothing.
    for (i, z) in zones.iter_mut().enumerate() {
        z.set(zf::GROUP, i as f32 + 1.0);
    }
    zones[2].set(zf::EXCL, 5.0);
    zones[3].set(zf::EXCL, 6.0);
    zones[3].set(zf::OFFBY, 5.0);
    zones[1].set(zf::GAIN, -3.0);
    zones[1].set(zf::TUNE, 50.0);
    let (k, log) = kit_from_zones(&zones);
    assert!(log.is_empty(), "{log:?}");
    let kick = pad(&k, 36);
    assert_eq!(kick.name, "Bass Drum");
    assert_eq!(kick.layers.len(), 2);
    assert_eq!(
        (
            kick.layers[1].vel_lo,
            kick.layers[1].level_db,
            kick.layers[1].tune
        ),
        (65, -3.0, 0.5)
    );
    assert_ne!(pad(&k, 42).choke, 0);
    assert_eq!(
        pad(&k, 42).choke,
        pad(&k, 46).choke,
        "the hats choke each other"
    );
    assert_eq!(kick.choke, 0);
}

/// One-way cuts make a shared choke group; a group the reader clamped (64) is not read.
#[test]
fn sfz_cuts_are_one_way_and_a_clamped_group_is_left_out() {
    use crate::inst::sampler::{zf, Zone};
    let z = |key: u8| Zone::new(Some(format!("k{key}.wav")), key, key, key);
    let mut zones = vec![z(36), z(38), z(49), z(51)];
    zones[2].set(zf::EXCL, 3.0); // the crash cuts the ride
    zones[3].set(zf::OFFBY, 3.0);
    zones[0].set(zf::EXCL, 64.0); // clamped: unknowable
    zones[1].set(zf::OFFBY, 64.0);
    let (k, log) = kit_from_zones(&zones);
    assert_eq!(pad(&k, 49).choke, pad(&k, 51).choke);
    assert_ne!(pad(&k, 49).choke, 0);
    assert_eq!(
        (pad(&k, 36).choke, pad(&k, 38).choke),
        (0, 0),
        "not linked by a clamped 64"
    );
    assert!(log.iter().any(|l| l.contains("above 63")), "{log:?}");
}

/// A number that names the drum is no take: toms and cymbals by number, a leading number.
#[test]
fn numbered_toms_cymbals_and_machines_are_drums_of_their_own() {
    let (k, log) = kit_from_files(&files(&[
        "Tom 1.wav",
        "Tom 2.wav",
        "Tom 3.wav",
        "Crash 1.wav",
        "Crash 2.wav",
        "808 Kick.wav",
        "909 Kick.wav",
        "Snare 1.wav",
        "Snare 2.wav",
    ]));
    assert!(log.is_empty(), "{log:?}");
    assert_eq!(
        (pad(&k, 50).name.as_str(), pad(&k, 48).name.as_str()),
        ("Tom 1", "Tom 2")
    );
    assert_eq!(pad(&k, 47).name, "Tom 3");
    assert_eq!(
        (pad(&k, 49).name.as_str(), pad(&k, 57).name.as_str()),
        ("Crash 1", "Crash 2")
    );
    let kicks: Vec<&str> = k
        .pads
        .iter()
        .filter(|p| p.name.ends_with("Kick"))
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(kicks.len(), 2, "808 Kick and 909 Kick: {kicks:?}");
    // A snare's numbers are still takes: one pad, cycling.
    assert_eq!(pad(&k, 38).layers.len(), 2);
    assert!(k.pads.iter().all(|p| p.layers.len() <= 2));
}
