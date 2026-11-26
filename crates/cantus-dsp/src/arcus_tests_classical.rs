//! r42 arcus-classical — the classical pass's tests (`docs/round42/arcus-classical.md`): the
//! stroke's shape, the taper and the free string after the lift, the vibrato's way back, the
//! player's intonation, the pluck's partials in tune, a fiddler's shuffle — and their nulls.

use super::tables::Instrument;
use super::tests::{cents, id_of, measure_hz, render, Ev, SR};
use super::*;
use crate::par::Serial;

fn s(inst: Instrument) -> ArcusSettings {
    ArcusSettings {
        instrument: inst,
        ..ArcusSettings::default()
    }
}

/// The level (dB re full scale) of `x` over `from…to` s.
fn level(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = (
        (from * SR as f64) as usize,
        ((to * SR as f64) as usize).min(x.len()),
    );
    let e: f64 = x[a..b]
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        / (b - a) as f64;
    10.0 * math::log10(e + 1e-30)
}

const S: usize = SR as usize;

/// The player off, every knob of the shape is inert: the stroke's shape, the taper, the
/// vibrato's way back and the intonation are the player's alone.
#[test]
fn with_the_player_off_the_shape_touches_no_sample() {
    let score = [
        (0, Ev::On(50, 0.5)),
        (S / 2, Ev::On(52, 0.5)),
        (S, Ev::Off(50)),
        (S + S / 3, Ev::On(55, 0.6)),
        (2 * S, Ev::Off(52)),
        (2 * S + S / 2, Ev::Off(55)),
    ];
    for inst in Instrument::ALL {
        let off = ArcusSettings {
            player: 0.0,
            ..s(inst)
        };
        let tried = ArcusSettings {
            shape_try: Some(shape::KNOBS.parse(
                "rebow_db=20,sag_db=20,breath_db=5,taper_db=30,taper_k=4,into_ct=40,vib_floor=0,slur_s=1",
            )),
            ..off
        };
        let a = render(&off, &score, 3 * S, &|_| 256, &Serial);
        let b = render(&tried, &score, 3 * S, &|_| 256, &Serial);
        assert!(
            a == b,
            "{inst:?}: the shape moved a sample with the player off"
        );
    }
}

/// A held note from silence swells (the bow's speed and the dynamic it is read at), sags and
/// breathes; let go, the bow tapers before it lifts, and the string rings on after it.
#[test]
fn a_held_note_swells_sags_and_tapers_before_the_bow_lifts() {
    for inst in Instrument::ALL {
        let spec = inst.spec();
        let key = spec.bottom() + 9;
        let score = [(0, Ev::On(key, 0.55)), (3 * S, Ev::Off(key))];
        let (l, _) = render(&s(inst), &score, 4 * S, &|_| 256, &Serial);
        let start = level(&l, 0.0, 0.06);
        let peak = (0..30)
            .map(|i| level(&l, 0.1 * i as f64, 0.1 * i as f64 + 0.1))
            .fold(f64::NEG_INFINITY, f64::max);
        let late = level(&l, 2.8, 2.95);
        assert!(
            peak - start > 6.0,
            "{inst:?}: starts only {:.1} dB under",
            peak - start
        );
        assert!(
            peak - late > 1.5,
            "{inst:?}: no sag ({:.1} dB)",
            peak - late
        );
        // the taper: still sounding after the key, well under the note
        let after = level(&l, 3.12, 3.17);
        assert!(
            after > late - 20.0 && after < late - 3.0,
            "{inst:?}: taper {:.1} dB",
            late - after
        );
        // the free string: no collapse after the lift (the fall over 0.3–0.5 s after the key)
        let a = level(&l, 3.3, 3.35);
        let b = level(&l, 3.5, 3.55);
        assert!(
            (a - b) / 0.2 < 110.0,
            "{inst:?}: falls {:.0} dB/s after the lift",
            (a - b) / 0.2
        );
    }
}

/// The intonation in a passage: seeded (the same bits twice, others with another seed), and a
/// note from silence keeps its pitch (the pitch gates' notes are such notes).
#[test]
fn the_players_intonation_is_seeded_and_spares_a_note_from_silence() {
    let k = shape::KNOBS;
    let a = shape::intonation(&k, 1.0, 2.0, false, 0, 62, 7);
    assert_eq!(a, shape::intonation(&k, 1.0, 2.0, false, 0, 62, 7));
    assert_ne!(a, shape::intonation(&k, 1.0, 2.0, false, 1, 62, 7));
    assert_eq!(shape::intonation(&k, 0.0, 1.0, true, 0, 62, 7), (0.0, 0.0));
    assert!(shape::intonation(&k, 1.0, 1.0, false, 0, 62, 7).1 > 0.0); // a semitone up: high
    assert!(shape::intonation(&k, 1.0, -1.0, false, 0, 62, 7).1 < 0.0);
    // its correction: the kept share after a while, all of it at the start
    let at0 = shape::intonation_at(&k, 10.0, 0.0, 0.0);
    let late = shape::intonation_at(&k, 10.0, 0.0, 5.0);
    assert!((at0 - 10.0).abs() < 1e-4 && (late - 10.0 * k.into_keep).abs() < 1e-3);
    // a slurred passage differs by seed; a note from silence is the same at any seed
    let score = [
        (0, Ev::On(50, 0.5)),
        (S / 2, Ev::On(53, 0.5)),
        (S, Ev::Off(50)),
    ];
    let x = render(&s(Instrument::Cello), &score, 3 * S / 2, &|_| 256, &Serial);
    let y = render(
        &ArcusSettings {
            seed: 9,
            ..s(Instrument::Cello)
        },
        &score,
        3 * S / 2,
        &|_| 256,
        &Serial,
    );
    let f_x = measure_hz(&x.0, 440.0 * math::exp2((53.0 - 69.0) / 12.0), 0.8, 1.0);
    let f_y = measure_hz(&y.0, 440.0 * math::exp2((53.0 - 69.0) / 12.0), 0.8, 1.0);
    assert!(
        cents(f_x, f_y).abs() > 0.2,
        "the slur's intonation did not move with the seed"
    );
    let _ = id_of(50);
}

/// A pluck's partials in tune (model 2): its second and third within half a cent of 2 and 3 ×
/// its fundamental (the loop's DC blocker had left them 3.2 and 3.8 cents flat).
#[test]
fn a_plucks_partials_are_in_tune() {
    for inst in Instrument::ALL {
        let key = inst.spec().bottom() + 12;
        let st = ArcusSettings {
            technique: Technique::Pizzicato,
            vib_depth: 0.0,
            body_mix: 0.0,
            ..s(inst)
        };
        let (l, _) = render(&st, &[(0, Ev::On(key, 0.6))], 2 * S, &|_| 256, &Serial);
        let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
        let f1 = measure_hz(&l, f0, 0.1, 1.0);
        for n in [2.0, 3.0] {
            let fnh = measure_hz(&l, n * f1, 0.1, 1.0);
            let c = cents(fnh, n * f1);
            assert!(c.abs() < 0.5, "{inst:?}: partial {n} at {c:+.2} cents");
        }
    }
}

/// A fiddler's shuffle: fast separate notes on a fiddler's bow are slurred two or three to a
/// bow (fewer bow changes than notes); the same notes on a classical bow change at each.
#[test]
fn a_fiddlers_fast_notes_are_shuffled_into_bows() {
    let fid = ArcusSettings {
        instrument: Instrument::Violin,
        grit: 1.0,
        mode: crate::voices::PlayMode::Legato,
        ..ArcusSettings::default()
    };
    let shuffles = |st: &ArcusSettings| {
        let mut a = Arcus::new(SR);
        a.set(st);
        let mut l = vec![0.0; 256];
        let mut r = vec![0.0; 256];
        let mut changes = 0;
        let keys = [69u8, 71, 73, 74, 76, 74, 73, 71, 69, 71, 73, 74];
        for (i, k) in keys.iter().enumerate() {
            a.note_on(*k, 0.6, i as u64 + 1);
            for _ in 0..12 {
                a.process(&mut l, &mut r);
                if a.voices.iter().any(|v| v.phase() == voice::Phase::Change) {
                    changes += 1;
                    break;
                }
            }
            for _ in 0..4 {
                a.process(&mut l, &mut r);
            }
            a.note_off(*k, i as u64 + 1);
            for _ in 0..2 {
                a.process(&mut l, &mut r);
            }
        }
        changes
    };
    let classical = ArcusSettings { grit: 0.0, ..fid };
    let (f, c) = (shuffles(&fid), shuffles(&classical));
    assert!(
        f < c && f >= 3,
        "a fiddler's bow changed {f} times, a classical one {c}"
    );
}
