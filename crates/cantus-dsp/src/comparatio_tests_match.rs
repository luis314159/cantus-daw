//! The loudness match, the reader's sync and declick, and the blind buttons.

use super::blind::{self, Abx, Preference, Source};
use super::reader::Sync;
use super::testkit::*;
use super::*;

fn stereo_pink(secs: f64, seed: u32, rms: f32) -> (Vec<f32>, Vec<f32>) {
    let n = ((secs * FS as f64) as usize).next_power_of_two();
    (pink(n, seed, rms), pink(n, seed ^ 0x1234, rms))
}

/// Integrated match: after the switch, the reference heard is at the mix's loudness within
/// 0.1 LU — measured on the output by a separate meter over eight seconds — for a
/// reference 9 LU louder and one 14 LU quieter, both tilted unlike the mix.
#[test]
fn the_integrated_match_levels_within_a_tenth_of_a_lu() {
    let (ml, mr) = stereo_pink(24.0, 1, 0.05);
    for gain in [3.0f32, 0.2] {
        let (rl, rr) = stereo_pink(30.0, 7, 0.05 * gain);
        // Tilt the reference: brighter (white mixed in).
        let w = white(rl.len(), 9, 0.05 * gain);
        let rf = OwnedRef {
            left: rl.iter().zip(&w).map(|(a, b)| a + 0.5 * b).collect(),
            right: rr.iter().zip(&w).map(|(a, b)| a - 0.5 * b).collect(),
            id: 1,
        };
        let base = ComparatioSettings {
            sync: Sync::Locked { offset: 0 },
            ..ComparatioSettings::default()
        };
        let on = ComparatioSettings {
            listen: Listen::Reference,
            ..base
        };
        let sw = 12 * FS as usize;
        let mut u = Comparatio::new(FS);
        let (ol, or) = drive(
            &mut u,
            (&ml, &mr),
            Some(&rf),
            512,
            0,
            &[(0, base), (sw, on)],
        );
        let from = sw + 4 * FS as usize;
        let to = from + 8 * FS as usize;
        let heard = integrated(&ol[from..to], &or[from..to]);
        let mix = integrated(&ml[from..to], &mr[from..to]);
        let raw = integrated(&rf.left[from..to], &rf.right[from..to]);
        let m = u.meter();
        eprintln!(
            "integrated match: mix {mix:.3}, reference {raw:.3} raw, heard {heard:.3} LUFS \
             (gain {:.3} dB) — off by {:+.3} LU",
            m.gain_db,
            heard - mix
        );
        assert!((heard - mix).abs() < 0.1, "{heard} against {mix}");
    }
}

/// Short-term ("live") match: a reference whose level steps by 6 dB every 8 s is heard at
/// the mix's short-term loudness within 0.1 LU at the end of every step; and a quiet
/// passage (30 LU under) is not raised.
#[test]
fn the_short_term_match_follows_and_holds_through_quiet_passages() {
    let (ml, mr) = stereo_pink(48.0, 2, 0.05);
    let (rl, rr) = stereo_pink(48.0, 4, 0.1);
    let step = 8 * FS as usize;
    let level = |i: usize| match (i / step) % 4 {
        0 => 1.0,
        1 => 0.5,
        2 => 0.03, // 30 dB down: the gate holds the gain
        _ => 2.0,
    };
    let rf = OwnedRef {
        left: rl.iter().enumerate().map(|(i, v)| v * level(i)).collect(),
        right: rr.iter().enumerate().map(|(i, v)| v * level(i)).collect(),
        id: 1,
    };
    let s = ComparatioSettings {
        listen: Listen::Reference,
        matching: MatchMode::ShortTerm,
        sync: Sync::Locked { offset: 0 },
        ..ComparatioSettings::default()
    };
    let n = 4 * step;
    let mut u = Comparatio::new(FS);
    let mut gains = Vec::new();
    let (mut ol, mut or) = (ml[..n].to_vec(), mr[..n].to_vec());
    for start in (0..n).step_by(480) {
        u.set(&s);
        u.set_transport(&playing(start as i64));
        u.process(
            &mut ol[start..start + 480],
            &mut or[start..start + 480],
            Some(rf.audio()),
        );
        if (start + 480) % step == 0 {
            gains.push(u.meter().gain_db);
        }
    }
    for k in [0usize, 1, 3] {
        let (a, b) = ((k + 1) * step - 3 * FS as usize, (k + 1) * step);
        let heard = integrated(&ol[a..b], &or[a..b]);
        let mix = integrated(&ml[a..b], &mr[a..b]);
        eprintln!(
            "short-term match, step {k}: heard {heard:.3}, mix {mix:.3} (gain {:.2} dB)",
            gains[k]
        );
        assert!((heard - mix).abs() < 0.1, "step {k}: {heard} against {mix}");
    }
    eprintln!(
        "through the quiet passage the gain held at {:.2} dB (before {:.2})",
        gains[2], gains[1]
    );
    assert!(
        (gains[2] - gains[1]).abs() < 0.5,
        "a quiet passage is not raised"
    );
}

/// Locked to the song: the reference heard is the song's position plus the offset, to the
/// sample and to the bit (manual match at 0 dB), through a seek.
#[test]
fn a_locked_reference_plays_the_songs_position_plus_the_offset() {
    let n = 5 * FS as usize;
    let ramp: Vec<f32> = (0..n).map(|i| (i % 65_536) as f32 / 65_536.0).collect();
    let rf = OwnedRef {
        left: ramp.clone(),
        right: ramp.iter().map(|v| -v).collect(),
        id: 1,
    };
    let s = ComparatioSettings {
        listen: Listen::Reference,
        matching: MatchMode::Manual,
        sync: Sync::Locked { offset: 777 },
        ..ComparatioSettings::default()
    };
    let mut u = Comparatio::new(FS);
    u.set(&s);
    let block = 256;
    let mut song = 1_000i64;
    let mut checked = 0;
    for k in 0..400 {
        if k == 200 {
            song = 90_000; // a seek
        }
        let (mut l, mut r) = (vec![0.0f32; block], vec![0.0f32; block]);
        u.set_transport(&playing(song));
        u.process(&mut l, &mut r, Some(rf.audio()));
        // Away from the fades (the switch's 30 ms, the seek's 5 ms): exact.
        if (10..200).contains(&k) || k > 202 {
            for i in 0..block {
                let want = ramp[(song + 777) as usize + i];
                assert_eq!(l[i].to_bits(), want.to_bits(), "block {k} sample {i}");
                assert_eq!(r[i].to_bits(), (-want).to_bits());
                checked += 1;
            }
        }
        song += block as i64;
    }
    assert!(checked > 90_000);
}

/// The reader's jumps do not click: a seek in the song, a loop's wrap and the song's start
/// and stop, on a 100 Hz reference — the largest step from one sample to the next stays
/// near the sine's own.
#[test]
fn the_reader_declicks_seeks_wraps_starts_and_stops() {
    let n = 6 * FS as usize;
    let tone = sine(100.0, -6.0, 6.0, 0.0);
    let rf = OwnedRef {
        left: tone.clone(),
        right: tone.clone(),
        id: 1,
    };
    let own_step = 0.5 * std::f32::consts::TAU * 100.0 / FS;
    for sync in [
        Sync::Locked { offset: 0 },
        Sync::Loop {
            start: 10_007,
            end: 30_011,
        },
    ] {
        let s = ComparatioSettings {
            listen: Listen::Reference,
            matching: MatchMode::Manual,
            fade_ms: 1.0 / 48.0,
            sync,
            ..ComparatioSettings::default()
        };
        let mut u = Comparatio::new(FS);
        u.set(&s);
        let mut out = Vec::new();
        let mut song = 0i64;
        for k in 0..(n / 480) {
            let t = match k {
                0..50 => playing(song),
                50 => {
                    song = 123_457; // a seek
                    playing(song)
                }
                51..250 => playing(song),
                250..260 => stopped(song),
                _ => playing(song),
            };
            u.set_transport(&t);
            let (mut l, mut r) = (vec![0.0f32; 480], vec![0.0f32; 480]);
            u.process(&mut l, &mut r, Some(rf.audio()));
            out.extend(l);
            if t.playing {
                song += 480;
            }
        }
        let (at, worst) = out
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .enumerate()
            .fold((0, 0f32), |m, (i, v)| if v > m.1 { (i, v) } else { m });
        eprintln!("{sync:?}: the largest step at sample {at}");
        eprintln!("{sync:?}: largest step {worst:.5} (the sine's own {own_step:.5})");
        assert!(worst < 4.0 * own_step, "{sync:?}: {worst}");
    }
}

/// Blind: the order is the seed's (the same seed, the same test), fair, independent of
/// the trial before; the meter shows nothing; the score is an exact binomial.
#[test]
fn the_blind_test_is_reproducible_hidden_and_scored_exactly() {
    let a: Vec<bool> = (0..64).map(|t| blind::x_is_reference(42, t)).collect();
    let b: Vec<bool> = (0..64).map(|t| blind::x_is_reference(42, t)).collect();
    let c: Vec<bool> = (0..64).map(|t| blind::x_is_reference(43, t)).collect();
    assert_eq!(a, b);
    assert_ne!(a, c);
    let heads = (0..10_000u32)
        .filter(|t| blind::x_is_reference(7, *t))
        .count();
    assert!((4_850..5_150).contains(&heads), "fair: {heads}");
    let same = (0..10_000u32)
        .filter(|t| blind::x_is_reference(7, *t) == blind::x_is_reference(7, t + 1))
        .count();
    assert!((4_850..5_150).contains(&same), "independent: {same}");
    // Exact tails: 12 of 16 is p = 2 517 / 65 536; 9 of 10, 11 / 1 024; 0 of n, 1.
    assert!((blind::p_value(12, 16) - 2_517.0 / 65_536.0).abs() < 1e-15);
    assert!((blind::p_value(9, 10) - 11.0 / 1_024.0).abs() < 1e-15);
    assert_eq!(blind::p_value(0, 8), 1.0);
    assert!((blind::p_value(16, 16) - 1.0 / 65_536.0).abs() < 1e-18);
    // A listener who always hears right scores 16 of 16.
    let mut abx = Abx::new(99, 16);
    while !abx.finished() {
        let x = abx.x();
        abx.answer(x);
    }
    assert_eq!(abx.score(), (16, 16));
    let mut pref = Preference::new(5, 8);
    while !pref.finished() {
        let one_is_ref = pref.button(true) == Source::Reference;
        pref.choose(one_is_ref);
    }
    assert_eq!(pref.score(), (8, 8));
    // The meter hides what plays, and the unit plays what the seed says.
    let (ml, mr) = (vec![0.25f32; 9_600], vec![0.25f32; 9_600]);
    let rf = OwnedRef {
        left: vec![-0.5; 48_000],
        right: vec![-0.5; 48_000],
        id: 1,
    };
    for trial in 0..8 {
        let s = ComparatioSettings {
            listen: Listen::X,
            matching: MatchMode::Manual,
            sync: Sync::Locked { offset: 0 },
            blind_seed: 1_234,
            blind_trial: trial,
            ..ComparatioSettings::default()
        };
        let mut u = Comparatio::new(FS);
        let (ol, _) = drive(&mut u, (&ml, &mr), Some(&rf), 480, 0, &[(0, s)]);
        assert_eq!(u.meter().heard, Heard::Hidden);
        assert_eq!(u.meter().fade, 0.0);
        let x_ref = blind::x_is_reference(1_234, trial);
        let last = *ol.last().unwrap();
        assert_eq!(last, if x_ref { -0.5 } else { 0.25 }, "trial {trial}");
    }
}
