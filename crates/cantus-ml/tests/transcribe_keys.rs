//! The drum keys a conversion writes (ADR 0086, round 34's review): every one is a key the
//! studio kit has a pad for (`drums_model::KEYS`; the UI's tests hold the kit to it). The
//! network's ride bell — and the cowbell Magenta folds into it, pitch 53 — has no pad, so
//! it plays the ride.

use cantus_dsp::onset::drums::Kind;
use cantus_ml::transcribe::drums_model::{self, Probs, FPS, KEYS, RATE};
use cantus_ml::transcribe::spec::DrumClass;

/// A network answer where column `j` alone fires at frame `f` for each `(f, j)`.
fn probs(frames: usize, fire: &[(usize, usize)]) -> Probs {
    let mut p = Probs {
        frames,
        onset: vec![0.0; frames * 8],
        velocity: vec![0.7; frames * 8],
    };
    for &(f, j) in fire {
        for g in f..f + 2 {
            p.onset[g * 8 + j] = 0.95;
        }
    }
    p
}

#[test]
fn every_key_the_network_names_is_one_the_kit_has() {
    // Each of the eight outputs alone, half a second apart: the ride bell (column 5) too.
    let fire: Vec<(usize, usize)> = (0..8).map(|j| (20 + 50 * j, j)).collect();
    let frames = 450;
    let x = vec![0.0f32; (frames as f64 / FPS * RATE as f64) as usize];
    let hits = drums_model::hybrid(&x, RATE, 0.5, &probs(frames, &fire), &|| false).unwrap();
    for h in &hits {
        assert!(
            KEYS.contains(&h.key),
            "key {} ({:?}) has no pad",
            h.key,
            h.class
        );
    }
    let bell = hits
        .iter()
        .find(|h| (h.time - 2.70).abs() < 0.02)
        .expect("the bell is heard");
    assert_eq!(
        (bell.class, bell.key),
        (DrumClass::Cymbals, 51),
        "the ride bell plays the ride"
    );
}

#[test]
fn every_key_the_detector_names_is_one_the_kit_has() {
    for k in [
        Kind::Kick,
        Kind::Snare,
        Kind::LowTom,
        Kind::MidTom,
        Kind::HighTom,
        Kind::ClosedHat,
        Kind::OpenHat,
        Kind::Crash,
        Kind::Ride,
    ] {
        let key = cantus_ml::transcribe::detector_key(k);
        assert!(KEYS.contains(&key), "{k:?} → {key}");
    }
}
