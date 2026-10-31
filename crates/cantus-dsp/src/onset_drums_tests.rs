//! Drums to notes: the decisions on activations made here (a hit named by its group's
//! templates, a leak dropped, hats and cymbals told by how they ring, toms by their body),
//! the timing of hits struck at known samples, silence and a held tone giving nothing,
//! velocities following level, and the frame grid's independence of the sample rate.
//! How well it transcribes real kits is measured on our own instrument's renders
//! (`cantus-ui/tests/transcribe_accuracy.rs`, in CI) and on MDB Drums
//! (`tests/onset_drums_mdb.rs`, ignored).

use super::*;

const SR: f32 = 44_100.0;

/// Per component, its spikes: `(frame, level, decay in frames)`.
type Spikes = [(usize, Vec<(usize, f32, f32)>)];

/// A spectrogram of `frames` whose energy sits mostly in the bands around `body` Hz, and
/// the activations `acts`.
fn rig(frames: usize, acts: &Spikes, body: f64) -> (Spectrogram, Factors) {
    let centres: Vec<f64> = (0..64).map(|b| 40.0 * math::exp2(b as f64 / 8.0)).collect();
    let bands = centres.len();
    let mut mag = vec![0.0f32; frames * bands];
    for t in 0..frames {
        for (b, f) in centres.iter().enumerate() {
            // A little everywhere, the body most.
            mag[t * bands + b] = if (f / body).max(body / f) < 1.2 {
                1.0
            } else {
                0.2
            };
        }
    }
    let comps = 10;
    let mut h = vec![0.0f32; comps * frames];
    for (k, spikes) in acts {
        for &(at, level, decay) in spikes {
            for t in at..frames {
                let v = level * math::exp(-((t - at) as f32) / decay.max(0.01));
                h[k * frames + t] = h[k * frames + t].max(v);
            }
        }
    }
    // The spectrum's level follows the activations (a hit is louder).
    for t in 0..frames {
        let total: f32 = (0..comps).map(|k| h[k * frames + t]).sum();
        for b in 0..bands {
            mag[t * bands + b] *= 0.1 + total;
        }
    }
    (
        Spectrogram {
            centres,
            mag,
            frames,
            hop: 441,
            rate: SR,
        },
        Factors {
            w: vec![0.0; bands * comps],
            h,
            comps,
        },
    )
}

fn classes(hits: &[Hit]) -> Vec<(usize, Class, Kind)> {
    hits.iter()
        .map(|h| ((h.time * FPS).round() as usize, h.class, h.kind))
        .collect()
}

#[test]
fn each_group_names_its_hits() {
    // Components: 0 kick, 1 snare, 2 low tom, 3 high tom, 4 hat, 5 cymbal, 6 bright snare.
    let (s, f) = rig(
        400,
        &[
            (0, vec![(50, 10.0, 4.0)]),
            (1, vec![(100, 8.0, 5.0)]),
            (4, vec![(150, 6.0, 1.5)]),
            (5, vec![(250, 6.0, 25.0), (320, 6.0, 120.0)]),
        ],
        60.0,
    );
    let got = classes(&hits(&s, &f, 0.5, &Tuning::default()));
    let at = |t: usize| {
        got.iter()
            .find(|g| g.0.abs_diff(t) <= 2)
            .map(|g| (g.1, g.2))
    };
    assert_eq!(at(50), Some((Class::Kick, Kind::Kick)), "{got:?}");
    assert_eq!(at(100), Some((Class::Snare, Kind::Snare)), "{got:?}");
    assert_eq!(at(150), Some((Class::HiHat, Kind::ClosedHat)), "{got:?}");
    assert_eq!(at(250), Some((Class::Cymbals, Kind::Ride)), "{got:?}");
    assert_eq!(at(320), Some((Class::Cymbals, Kind::Crash)), "{got:?}");
    assert_eq!(got.len(), 5, "{got:?}");
}

#[test]
fn a_low_hit_held_by_the_toms_with_a_high_body_is_a_tom() {
    let tune = Tuning::default();
    let acts = [(3, vec![(60, 9.0, 8.0)]), (0, vec![(60, 2.0, 4.0)])];
    // The toms hold most of it and its body is at 200 Hz: a tom (high).
    let (s, f) = rig(200, &acts, 200.0);
    let got: Vec<(Class, Kind)> = classes(&hits(&s, &f, 0.5, &tune))
        .iter()
        .map(|g| (g.1, g.2))
        .collect();
    assert_eq!(got, [(Class::Toms, Kind::HighTom)]);
    // The same activations over a body at 60 Hz: a kick.
    let (s, f) = rig(200, &acts, 60.0);
    let got: Vec<Class> = classes(&hits(&s, &f, 0.5, &tune))
        .iter()
        .map(|g| g.1)
        .collect();
    assert_eq!(got, [Class::Kick]);
}

#[test]
fn a_leak_beside_a_bigger_hit_is_dropped_and_a_real_one_kept() {
    let tune = Tuning::default();
    // A snare lights the cymbal a little: the cymbal is its leak.
    let (s, f) = rig(
        200,
        &[(1, vec![(80, 10.0, 5.0)]), (5, vec![(80, 0.5, 60.0)])],
        60.0,
    );
    let got: Vec<Class> = classes(&hits(&s, &f, 0.5, &tune))
        .iter()
        .map(|g| g.1)
        .collect();
    assert_eq!(got, [Class::Snare]);
    // A crash struck with the snare, far louder than what a snare leaks: both.
    let (s, f) = rig(
        200,
        &[(1, vec![(80, 8.0, 5.0)]), (5, vec![(80, 12.0, 60.0)])],
        60.0,
    );
    let got: Vec<Class> = classes(&hits(&s, &f, 0.5, &tune))
        .iter()
        .map(|g| g.1)
        .collect();
    assert!(
        got.contains(&Class::Cymbals) && got.contains(&Class::Snare),
        "{got:?}"
    );
}

#[test]
fn sensitivity_moves_the_bar() {
    let (s, f) = rig(300, &[(0, vec![(50, 10.0, 4.0), (150, 0.6, 4.0)])], 60.0);
    let n = |sens: f32| hits(&s, &f, sens, &Tuning::default()).len();
    assert_eq!(n(0.0), 1);
    assert_eq!(n(1.0), 2);
    assert!(delta(0.0) > delta(0.5) && delta(0.5) > delta(1.0));
}

/// Kicks and snares struck at known times on Pulsus II, our drum instrument.
fn struck(hits: &[(f64, bool)], seconds: f64, rate: f32) -> Vec<f32> {
    use crate::pulsus::{DrumVoice, PadSettings, Pulsus};
    let mut p = Pulsus::new(rate);
    p.set_kit(2, 1, 4, false);
    p.set_pad_count(2);
    // The 909 kick as Pulsus's own tests design it.
    let kick = PadSettings {
        decay: 0.45,
        pitch_drop: 26.0,
        pitch_decay: 0.028,
        click: 0.55,
        tone: 4_000.0,
        ..PadSettings::of(DrumVoice::Kick, 2)
    };
    // Pulsus's own snare (the classic kit's).
    let snare = PadSettings::of(DrumVoice::Snare, 0);
    p.set_pad(0, 36, &kick);
    p.set_pad(1, 38, &snare);
    let n = (seconds * rate as f64) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut at = 0usize;
    let mut order: Vec<(usize, u8)> = hits
        .iter()
        .map(|&(t, kick)| {
            (
                (t * rate as f64).round() as usize,
                if kick { 36 } else { 38 },
            )
        })
        .collect();
    order.sort();
    for (i, (start, pitch)) in order.iter().copied().chain([(n, 0)]).enumerate() {
        let end = start.min(n);
        if end > at {
            p.process_add(&mut l[at..end], &mut r[at..end]);
            at = end;
        }
        if pitch > 0 {
            p.note_on(pitch, 110, i as u64);
        }
    }
    l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect()
}

fn alternating(n: usize) -> Vec<(f64, bool)> {
    (0..n).map(|i| (0.3 + 0.5 * i as f64, i % 2 == 0)).collect()
}

#[test]
fn hits_land_where_they_were_struck() {
    let truth = alternating(8);
    let x = struck(&truth, 4.5, SR);
    let found = transcribe(&x, SR, 0.5);
    for &(at, kick) in &truth {
        let want = if kick { Class::Kick } else { Class::Snare };
        let h = found
            .iter()
            .filter(|h| h.class == want)
            .min_by(|a, b| (a.time - at).abs().total_cmp(&(b.time - at).abs()))
            .unwrap_or_else(|| panic!("no {want:?} near {at}: {found:?}"));
        assert!((h.time - at).abs() < 0.02, "{want:?} at {at}: {h:?}");
    }
}

#[test]
fn silence_and_a_held_tone_give_no_hits() {
    assert!(transcribe(&vec![0.0; 44_100], SR, 0.5).is_empty());
    assert!(transcribe(&[], SR, 0.5).is_empty());
    let mut x: Vec<f32> = (0..(3.0 * SR) as usize)
        .map(|i| 0.3 * math::sin(std::f32::consts::TAU * 220.0 * i as f32 / SR))
        .collect();
    for (i, v) in x.iter_mut().take(882).enumerate() {
        *v *= i as f32 / 882.0;
    }
    let found = transcribe(&x, SR, 0.5);
    assert!(found.iter().all(|h| h.time < 0.1), "{found:?}");
}

#[test]
fn louder_hits_get_higher_velocities() {
    let mut x = struck(&[(0.5, false)], 3.0, SR);
    let soft = struck(&[(1.5, false)], 3.0, SR);
    for (a, b) in x.iter_mut().zip(&soft) {
        *a += 0.2 * b;
    }
    let found = transcribe(&x, SR, 0.5);
    let snares: Vec<&Hit> = found.iter().filter(|h| h.class == Class::Snare).collect();
    assert_eq!(snares.len(), 2, "{found:?}");
    assert!(snares[0].velocity > snares[1].velocity + 20, "{snares:?}");
}

#[test]
fn the_frame_grid_follows_the_rate() {
    let truth = alternating(6);
    for rate in [22_050.0f32, 44_100.0, 48_000.0, 96_000.0] {
        let x = struck(&truth, 3.5, rate);
        let s = spectrogram(&x, rate);
        assert!((s.rate / s.hop as f32 - FPS as f32).abs() < 0.3, "{rate}");
        let found = transcribe(&x, rate, 0.5);
        let kicks = found.iter().filter(|h| h.class == Class::Kick).count();
        let snares = found.iter().filter(|h| h.class == Class::Snare).count();
        assert_eq!((kicks, snares), (3, 3), "{rate}: {found:?}");
    }
}
