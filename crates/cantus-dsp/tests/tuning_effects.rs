//! Round 42 (ADR 0155): the pitched effects follow the strip's tuning — Tonus pulls a voice
//! to the tuned note nearest it (and corrects it to that note's tuned pitch), Saltus's
//! harmony steps by the tuned interval, Nitor's tonic sub sits on the tuned tonic, Loquax's
//! carrier plays its key's tuned pitch. And at the default table, each is what it was (the
//! bits, with `tools/test/ab-compare.sh` and `tuning_null.rs` for the whole engine).

use cantus_dsp::loquax::carrier::{NoteKind, Synth};
use cantus_dsp::pitch::testkit;
use cantus_dsp::tonus::scale::{choose, choose_tuned, resolve, NoteState};
use cantus_dsp::tonus::{Scale, Tonus, TonusSettings};

const SR: f32 = 48_000.0;

/// Equal temperament at A415: every key 1.0127 semitones flat of A440's.
fn a415() -> [f32; 128] {
    [(12.0 * cantus_dsp::math::log2(415.0f64 / 440.0)) as f32; 128]
}

#[test]
fn tonus_pulls_a_voice_to_the_nearest_tuned_note_and_to_its_tuned_pitch() {
    // A voice at 440 Hz: in 12-TET at A440 it is A4; at A415 it is (nearly) B flat 4.
    let voice = testkit::voice(SR, 1.5, testkit::Vowel::A, 0.5, |_| 440.0);
    for (table, key, tuned) in [
        ([0.0f32; 128], 69, 69.0f32),
        (a415(), 70, 70.0 + a415()[70]),
    ] {
        let mut t = Tonus::new(SR);
        t.set(&TonusSettings::default());
        t.set_tuning(&table);
        let (mut l, mut r) = (voice.clone(), voice.clone());
        for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            t.process(a, b);
        }
        assert_eq!(t.target(), Some(key), "the target note");
        // The correction: the target's tuned pitch less the voice's.
        let want = tuned - t.detected().expect("voiced");
        assert!(
            (t.shift() - want).abs() < 0.001,
            "shift {} for {want}",
            t.shift()
        );
    }
}

#[test]
fn the_scale_snap_measures_its_distances_to_the_tuned_notes() {
    let allowed = resolve(Scale::Chromatic, 0, &[NoteState::Scale; 12]);
    // At the default it is the untuned choice, to the bit.
    for p in [59.4f32, 60.0, 60.49, 60.51, 71.9] {
        assert_eq!(
            choose(p, &allowed, None),
            choose_tuned(p, &allowed, None, &[0.0; 128])
        );
    }
    // A key raised half a semitone: a voice at 60.6 is nearer 60's tuned pitch (60.5).
    let mut table = [0.0f32; 128];
    table[60] = 0.5;
    assert_eq!(choose(60.6, &allowed, None).map(|x| x.0), Some(61));
    assert_eq!(
        choose_tuned(60.6, &allowed, None, &table).map(|x| x.0),
        Some(60)
    );
}

#[test]
fn nitor_s_tonic_sits_on_the_tuned_tonic() {
    for tonic in 0..12u8 {
        assert_eq!(
            cantus_dsp::nitor::tonic_hz(tonic),
            cantus_dsp::nitor::tonic_hz_tuned(tonic, &[0.0; 128]),
            "the default, to the bit"
        );
        let hz = cantus_dsp::nitor::tonic_hz_tuned(tonic, &a415());
        let want = 415.0 * cantus_dsp::math::exp2((f32::from(24 + tonic) - 69.0) / 12.0);
        assert!(
            (hz / want - 1.0).abs() < 1e-5,
            "tonic {tonic}: {hz} for {want}"
        );
    }
}

#[test]
fn loquax_s_carrier_plays_its_key_s_tuned_pitch() {
    let render = |table: &[f32; 128]| {
        let mut s = Synth::new(SR);
        s.tuning = *table;
        s.note(
            NoteKind::On {
                key: 57,
                velocity: 0.8,
            },
            true,
        );
        (0..SR as usize).map(|_| s.tick().0).collect::<Vec<f32>>()
    };
    let plain = testkit::measured_f0(&render(&[0.0; 128]), SR, 220.0);
    let tuned = testkit::measured_f0(&render(&a415()), SR, 207.5);
    let cents = 1200.0 * cantus_dsp::math::log2(f64::from(tuned) / f64::from(plain));
    let want = 1200.0 * cantus_dsp::math::log2(415.0f64 / 440.0);
    assert!((cents - want).abs() < 0.5, "{cents} c for {want}");
}
