//! Round 42 (ADR 0155): `Tuning::offsets` for every system — the temperaments against the
//! published table of `research/instruments/microtuning-verified.md` §5.2 (the Scala
//! archive's values, cross-checked with Wikipedia's and the construction rules, three
//! decimals), rotated to every key with A at the diapason; Scala scales with and without a
//! keyboard map against what Surge's MIT `tuning-library` tests hold for the same files;
//! non-octave periods; unmapped keys.

#![allow(clippy::needless_range_loop)] // tables read key by key, as the tests say them

use cantus_core::tuning::*;
use cantus_core::tuning_scala::{read_kbm, read_scl};
use cantus_core::tuning_temper::{cents, cents_in, deviation_in};

/// §5.2, C … B in cents above C, three decimals.
const PUBLISHED: [(Temperament, [f64; 12]); 15] = [
    (
        Temperament::Pythagorean,
        [
            0.000, 113.685, 203.910, 294.135, 407.820, 498.045, 611.730, 701.955, 815.640, 905.865,
            996.090, 1109.775,
        ],
    ),
    (
        Temperament::MeantoneQuarter,
        [
            0.000, 76.049, 193.157, 310.265, 386.314, 503.422, 579.471, 696.578, 772.627, 889.735,
            1006.843, 1082.892,
        ],
    ),
    (
        Temperament::MeantoneSixth,
        [
            0.000, 88.594, 196.741, 304.888, 393.482, 501.629, 590.224, 698.371, 786.965, 895.112,
            1003.259, 1091.853,
        ],
    ),
    (
        Temperament::WerckmeisterIii,
        [
            0.000, 90.225, 192.180, 294.135, 390.225, 498.045, 588.270, 696.090, 792.180, 888.270,
            996.090, 1092.180,
        ],
    ),
    (
        Temperament::WerckmeisterIv,
        [
            0.000, 82.405, 196.090, 294.135, 392.180, 498.045, 588.270, 694.135, 784.360, 890.225,
            1003.910, 1086.315,
        ],
    ),
    (
        Temperament::KirnbergerIi,
        [
            0.000, 90.225, 203.910, 294.135, 386.314, 498.045, 590.224, 701.955, 792.180, 895.112,
            996.090, 1088.269,
        ],
    ),
    (
        Temperament::KirnbergerIii,
        [
            0.000, 90.225, 193.157, 294.135, 386.314, 498.045, 590.224, 696.578, 792.180, 889.735,
            996.090, 1088.269,
        ],
    ),
    (
        Temperament::Vallotti,
        [
            0.000, 94.135, 196.090, 298.045, 392.180, 501.955, 592.180, 698.045, 796.090, 894.135,
            1000.000, 1090.225,
        ],
    ),
    (
        Temperament::Young1799,
        [
            0.000, 90.225, 196.090, 294.135, 392.180, 498.045, 588.270, 698.045, 792.180, 894.135,
            996.090, 1090.225,
        ],
    ),
    (
        Temperament::Kellner,
        [
            0.000, 90.225, 194.526, 294.135, 389.052, 498.045, 588.270, 697.263, 792.180, 891.789,
            996.090, 1091.007,
        ],
    ),
    (
        Temperament::Neidhardt,
        [
            0.000, 96.090, 196.090, 298.045, 394.135, 498.045, 596.090, 698.045, 796.090, 894.135,
            998.045, 1096.090,
        ],
    ),
    (
        Temperament::BachLehman,
        [
            0.000, 99.347, 196.741, 299.512, 393.482, 501.629, 597.392, 698.371, 799.510, 895.112,
            999.674, 1095.437,
        ],
    ),
    (
        Temperament::JustMajor,
        [
            0.000, 111.731, 203.910, 315.641, 386.314, 498.045, 590.224, 701.955, 813.686, 884.359,
            996.090, 1088.269,
        ],
    ),
    (
        Temperament::JustMinor,
        [
            0.000, 111.731, 203.910, 315.641, 386.314, 498.045, 590.224, 701.955, 813.686, 884.359,
            1017.596, 1088.269,
        ],
    ),
    (
        Temperament::Barbershop7,
        [
            0.000, 111.731, 203.910, 266.871, 386.314, 498.045, 582.512, 701.955, 813.686, 933.129,
            968.826, 1088.269,
        ],
    ),
];

#[test]
fn every_temperament_is_the_published_table() {
    assert_eq!(PUBLISHED.len(), Temperament::ALL.len());
    for (id, row) in PUBLISHED {
        let c = cents(id);
        for pc in 0..12 {
            assert!(
                (c[pc] - row[pc]).abs() < 0.0011,
                "{id:?} pc {pc}: {} vs {}",
                c[pc],
                row[pc]
            );
        }
    }
}

fn temperament(id: Temperament, key: u8, a4: f64) -> Tuning {
    Tuning {
        a4_hz: a4,
        system: TuningSystem::Temperament {
            id,
            key,
            follow_song_key: false,
        },
        map: None,
    }
}

#[test]
fn a_temperament_in_any_key_keeps_its_intervals_and_a_at_the_diapason() {
    for id in Temperament::ALL {
        let home = cents(id);
        for key in 0..12u8 {
            let r = cents_in(id, key);
            // The interval from the key's tonic to each degree is the row's from C.
            for i in 0..12usize {
                let pc = (usize::from(key) + i) % 12;
                let want = home[i];
                let got = (r[pc] - r[usize::from(key)]).rem_euclid(1200.0);
                assert!((got - want).abs() < 1e-9, "{id:?} in {key}, degree {i}");
            }
            for a4 in [440.0, 415.0] {
                let t = temperament(id, key, a4).offsets();
                let shift = diapason_semitones(a4);
                assert!(
                    (t.at(69) - shift).abs() < 1e-6,
                    "{id:?} in {key}: A4 on the diapason"
                );
                assert!((t.at(81) - shift).abs() < 1e-6, "and every A");
                // Octave-periodic, and each key's deviation is the rotated row's less A's.
                let d = deviation_in(id, key);
                for k in 0..128u8 {
                    let want = (d[usize::from(k % 12)] - d[9]) / 100.0 + f64::from(shift);
                    let pitch = f64::from(k) + want;
                    if (-36.0..=135.0).contains(&pitch) {
                        assert!(
                            (f64::from(t.at(k)) - want).abs() < 1e-5,
                            "{id:?} {key} key {k}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn werckmeister_iii_in_e_flat_at_415_is_these_cents() {
    // The E♭ major chord's thirds and fifths are the temperament's best (its C chord's).
    let t = temperament(Temperament::WerckmeisterIii, 3, 415.0).offsets();
    let pitch = |k: u8| f64::from(k) + f64::from(t.at(k));
    let fifth = (pitch(70) - pitch(63)) * 100.0;
    let third = (pitch(67) - pitch(63)) * 100.0;
    assert!((fifth - 696.090).abs() < 1e-3, "{fifth}");
    assert!((third - 390.225).abs() < 1e-3, "{third}");
    let hz = 440.0 * cantus_dsp::math::exp2((pitch(69) - 69.0) / 12.0);
    assert!((hz - 415.0).abs() < 1e-3);
    assert!(!t.is_equal());
}

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/tuning-library")
            .join(name),
    )
    .unwrap()
}

fn scale(file: &str, map: Option<&str>) -> Tuning {
    Tuning {
        a4_hz: A4_DEFAULT_HZ,
        system: TuningSystem::Scale(read_scl(&data(file)).unwrap()),
        map: map.map(|m| read_kbm(&data(m)).unwrap()),
    }
}

fn pitch(t: &TuningTable, k: u8) -> f64 {
    f64::from(k) + f64::from(t.at(k))
}

#[test]
fn a_12_tet_scale_file_is_equal_temperament() {
    let t = scale("12-intune-nodesc.scl", None).offsets();
    for k in 0..128u8 {
        assert!(t.at(k).abs() < 1e-6, "key {k}");
    }
    // Surge's expectations: A69 = 440, C60 = 261.625565301.
    let hz = |k| 440.0 * cantus_dsp::math::exp2((pitch(&t, k) - 69.0) / 12.0);
    assert!((hz(69) - 440.0).abs() < 1e-9);
    assert!((hz(60) - 261.625565301).abs() < 1e-6);
}

#[test]
fn edo_31_and_a_non_octave_period_step_as_their_files_say() {
    let t = scale("31edo.scl", None).offsets();
    for k in 1..128u8 {
        let step = (pitch(&t, k) - pitch(&t, k - 1)) * 100.0;
        if (-36.0..135.0).contains(&pitch(&t, k - 1)) && pitch(&t, k) < 135.0 {
            assert!((step - 1200.0 / 31.0).abs() < 1e-3, "key {k}: {step}");
        }
    }
    // r42 fix2-model (review 2): degree 0 on key 60 at C4 (Scala's and Surge's anchor), so
    // key 60 + i is i 31sts of an octave above 261.6256 Hz — key 69 is no longer 440 Hz.
    assert_eq!(pitch(&t, 60), 60.0, "key 60 at C4 of 12-TET");
    assert!(
        (pitch(&t, 69) - (60.0 + 9.0 * 12.0 / 31.0)).abs() < 1e-6,
        "key 69 is degree 9"
    );
    // Bohlen–Pierce's cousin: 17 steps to 3/1.
    let e = scale("ED3-17.scl", None).offsets();
    let tritave = (pitch(&e, 77) - pitch(&e, 60)) * 100.0;
    assert!(
        (tritave - 1200.0 * cantus_dsp::math::log2(3f64)).abs() < 1e-3,
        "{tritave}"
    );
    // A one-step scale (Carlos Alpha, 78 c).
    let c = scale("carlos-alpha.scl", None).offsets();
    assert!(((pitch(&c, 70) - pitch(&c, 69)) * 100.0 - 78.0).abs() < 1e-3);
    // A shuffled scale: each key is its degree, not sorted.
    let s = scale("12-shuffled.scl", None).offsets();
    assert!((pitch(&s, 61) - 60.0 - 2.0).abs() < 1e-5 || pitch(&s, 61) > pitch(&s, 62));
}

/// `MIDI_0_FREQ` of Surge's `tuning-library` (`Tunings.h`): MIDI key 0 of 12-TET at A440.
const MIDI_0_FREQ: f64 = 8.175_798_915_643_71;

/// Hz of key `k` (A440 12-TET reference).
fn hz(t: &TuningTable, k: u8) -> f64 {
    440.0 * cantus_dsp::math::exp2((pitch(t, k) - 69.0) / 12.0)
}

/// r42 fix2-model (review 2): a scale read with no keyboard map sounds as Surge's
/// `tuning-library` says it does — its expected values, copied from its `tests/alltests.cpp`
/// (`Tunings::Tuning t(s)` with the default `KeyboardMapping()`), not computed by our code:
/// - "Default KBM Constructor has Right Base": `frequencyForMidiNoteScaledByMidi0(60) == 32`
///   for every test scale;
/// - "12-intune tunes properly": `frequencyForMidiNote(69) == Approx(440.0)`;
/// - "31 edo": each key 1/31 of an octave above the one before, from key 60 on;
/// - "6 exact": `knownValues = {0, 0.22239, 0.41504, 0.58496, 0.73697, 0.87447, 1.0}`
///   octaves above key 60 (margin 1e-5);
/// - "ED3-17": `pow(2.0, 17 * (curr - prev)) == Approx(3.0)`;
/// - "Carlos Alpha": each key 78 c above the one before (keys 30–80);
/// - "Non Monotonic 12 note": key 60 + i is `order[i] / 12` octaves above key 60, with
///   `order = {0, 2, 1, 3, 5, 4, 6, 7, 8, 10, 9, 11, 12}`.
#[test]
fn a_scale_with_no_map_sounds_as_surge_s_tests_say() {
    for f in [
        "12-intune-nodesc.scl",
        "12-shuffled.scl",
        "31edo.scl",
        "31edo_dos_lineends.scl",
        "6-exact.scl",
        "ED3-17.scl",
        "carlos-alpha.scl",
    ] {
        let t = scale(f, None).offsets();
        let base = hz(&t, 60) / MIDI_0_FREQ;
        assert!(
            (base - 32.0).abs() < 1e-9,
            "{f}: key 60 at {base} x MIDI_0_FREQ"
        );
    }
    let log2 = cantus_dsp::math::log2;
    let i12 = scale("12-intune-nodesc.scl", None).offsets();
    assert!((hz(&i12, 69) - 440.0).abs() < 1e-10);
    let e31 = scale("31edo.scl", None).offsets();
    for k in 61..91u8 {
        let step = log2(hz(&e31, k)) - log2(hz(&e31, k - 1));
        assert!((step - 1.0 / 31.0).abs() < 1e-6, "31edo key {k}");
    }
    let six = scale("6-exact.scl", None).offsets();
    let known = [0.0, 0.22239, 0.41504, 0.58496, 0.73697, 0.87447, 1.0];
    for (i, v) in known.iter().enumerate() {
        let l = log2(hz(&six, 60 + i as u8)) - log2(hz(&six, 60));
        assert!((l - v).abs() < 1e-5, "6-exact degree {i}: {l}");
    }
    let ed3 = scale("ED3-17.scl", None).offsets();
    for k in 61..90u8 {
        let d = log2(hz(&ed3, k)) - log2(hz(&ed3, k - 1));
        assert!(
            (cantus_dsp::math::exp2(17.0 * d) - 3.0).abs() < 1e-6,
            "ED3-17 {k}"
        );
    }
    let alpha = scale("carlos-alpha.scl", None).offsets();
    let diff = cantus_dsp::math::exp2(78.0 / 1200.0);
    for k in 30..80u8 {
        let (a, b) = (hz(&alpha, k) / MIDI_0_FREQ, hz(&alpha, k + 1) / MIDI_0_FREQ);
        assert!((a * diff - b).abs() < 1e-5, "carlos alpha {k}");
    }
    let sh = scale("12-shuffled.scl", None).offsets();
    let order = [0, 2, 1, 3, 5, 4, 6, 7, 8, 10, 9, 11, 12];
    for (i, o) in order.iter().enumerate() {
        let l = log2(hz(&sh, 60 + i as u8)) - log2(hz(&sh, 60));
        assert!((l - f64::from(*o) / 12.0).abs() < 1e-6, "12-shuffled {i}");
    }
    // A diapason still scales the whole scale: A415 moves C4 by 12·log2(415/440).
    let mut low = scale("31edo.scl", None);
    low.a4_hz = 415.0;
    let l = low.offsets();
    assert_eq!(l.at(60), diapason_semitones(415.0));
    // A temperament keeps A on the diapason (Pianoteq's and Logic's anchor), unchanged.
    let w = Tuning {
        a4_hz: A4_DEFAULT_HZ,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 0,
            follow_song_key: false,
        },
        map: None,
    }
    .offsets();
    assert_eq!(w.at(69), 0.0);
}

#[test]
fn surge_s_white_keys_map_plays_the_chromatic_scale_on_the_white_keys() {
    let t = scale("12-intune-nodesc.scl", Some("mapping-whitekeys-c261.kbm")).offsets();
    // `tuning-library`'s test: the mapped key plays the 12-TET key's pitch.
    for (plain, mapped) in [
        (60, 60),
        (61, 62),
        (62, 64),
        (63, 65),
        (64, 67),
        (65, 69),
        (66, 71),
    ] {
        assert!(
            (pitch(&t, mapped) - f64::from(plain)).abs() < 1e-5,
            "{mapped}"
        );
    }
    for black in [61u8, 63, 66, 68, 70] {
        assert!(!t.sounds(black), "{black} is unmapped");
    }
    assert!(
        (pitch(&t, 72) - 72.0).abs() < 1e-5,
        "the next pattern starts a formal octave on"
    );
}

#[test]
fn keyboard_maps_reference_their_key_and_frequency() {
    let semis = |hz: f64| 12.0 * cantus_dsp::math::log2(hz / 440.0);
    // An integer frequency on key 69, linear.
    let t = scale("12-intune-nodesc.scl", Some("empty-note69.kbm")).offsets();
    assert!((f64::from(t.at(69)) - semis(452.0)).abs() < 1e-5);
    assert!(
        (f64::from(t.at(60)) - semis(452.0)).abs() < 1e-5,
        "a linear map shifts all alike"
    );
    // 128.kbm: key 60 at 261.6256.
    let k = scale("12-intune-nodesc.scl", Some("128.kbm")).offsets();
    assert!((pitch(&k, 60) - (69.0 + semis(261.6256))).abs() < 1e-5);
    // piano.kbm retunes 21–108 only: the rest stays 12-TET.
    let p = scale("31edo.scl", Some("piano.kbm")).offsets();
    assert_eq!(p.at(20), 0.0);
    assert_eq!(p.at(109), 0.0);
    assert!((pitch(&p, 69) - 69.0).abs() < 1e-5, "its reference");
    assert!(((pitch(&p, 70) - pitch(&p, 69)) * 100.0 - 1200.0 / 31.0).abs() < 1e-3);
    // A 7-note scale on 12 keys at A442, formal octave 7 (Ptolemy's major, our fixture).
    let ptolemy = b"! ptolemy (our fixture)\n!\nPtolemy's intense diatonic\n7\n9/8\n5/4\n4/3\n3/2\n5/3\n15/8\n2/1\n";
    let seven = Tuning {
        a4_hz: A4_DEFAULT_HZ,
        system: TuningSystem::Scale(read_scl(ptolemy).unwrap()),
        map: Some(read_kbm(&data("mapping-a442-7-to-12.kbm")).unwrap()),
    }
    .offsets();
    assert!((f64::from(seven.at(69)) - semis(442.0)).abs() < 1e-5);
    let octave = (pitch(&seven, 71) - pitch(&seven, 59)) * 100.0;
    assert!((octave - 1200.0).abs() < 1e-3, "{octave}");
    // The diapason scales a whole map alike.
    let mut lower = scale("12-intune-nodesc.scl", Some("empty-note69.kbm"));
    lower.a4_hz = 415.0;
    let l = lower.offsets();
    let want = semis(452.0) + semis(415.0);
    assert!((f64::from(l.at(69)) - want).abs() < 1e-5);
}

#[test]
fn far_keys_are_kept_in_range_and_the_default_is_zero_to_the_bit() {
    // Bohlen–Pierce without a map: 146 c a key reaches past 20 kHz and below 1 Hz.
    let bp = b"! bp (our fixture)\n!\n13 equal steps of 3/1\n1\n146.30423\n";
    let t = Tuning {
        a4_hz: A4_DEFAULT_HZ,
        system: TuningSystem::Scale(read_scl(bp).unwrap()),
        map: None,
    }
    .offsets();
    for k in 0..128u8 {
        let p = pitch(&t, k);
        assert!((-36.0..=135.0001).contains(&p), "key {k}: {p}");
    }
    assert!(Tuning::default()
        .offsets()
        .0
        .iter()
        .all(|v| v.to_bits() == 0));
    // An equal tuning written another way is still exactly the diapason.
    let e = Tuning {
        a4_hz: 432.0,
        ..Tuning::default()
    }
    .offsets();
    assert!(e.0.iter().all(|&v| v == diapason_semitones(432.0)));
}
