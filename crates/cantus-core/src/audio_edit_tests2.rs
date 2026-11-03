//! The joins, the guards and the repairs (the integrator's review of round 36).

use super::*;
use cantus_dsp::denoise::stft::{HOP, N};
use cantus_dsp::math;

const RATE: u32 = 48_000;

fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(std::f32::consts::TAU * hz * i as f32 / RATE as f32))
        .collect()
}

fn noise(seed: u32, n: usize, amp: f32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * amp
        })
        .collect()
}

/// The largest jump between neighbouring samples of `x[range]`.
fn max_step(x: &[f32], from: usize, to: usize) -> f32 {
    (from.max(1)..to.min(x.len()))
        .map(|i| (x[i] - x[i - 1]).abs())
        .fold(0.0, f32::max)
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn a_cut_in_the_middle_of_a_wave_has_no_step() {
    // 220 Hz at -6 dBFS: its own largest step is 0.5 · 2π · 220 / 48000 ≈ 0.0144.
    let b = AudioBuf::new(RATE, 1, sine(220.0, 0.5, 48_000));
    let s = Sel::new(10_007, 30_011);
    let raw = apply_with(&b, s, &Edit::Delete, &Joins::RAW).unwrap();
    assert!(
        max_step(&raw.buf.data, 9_900, 10_200) > 0.1,
        "the raw cut does click"
    );
    for joins in [
        Joins::default(),
        Joins {
            snap: false,
            ..Joins::default()
        },
        Joins {
            xfade_ms: 0.0,
            ..Joins::default()
        },
    ] {
        let out = apply_with(&b, s, &Edit::Delete, &joins).unwrap();
        let j = out.sel.start;
        let step = max_step(&out.buf.data, j - 300, j + 300);
        assert!(step < 0.03, "{joins:?}: a step of {step}");
    }
}

#[test]
fn pastes_trims_silence_and_inserted_silence_join_smoothly() {
    let b = AudioBuf::new(RATE, 1, sine(180.0, 0.5, 48_000));
    let clip = AudioBuf::new(RATE, 1, sine(530.0, 0.5, 9_000));
    let s = Sel::new(20_003, 20_003);
    let p = apply(&b, s, &Edit::Paste { clip }).unwrap();
    let (a, e) = (p.sel.start, p.sel.end);
    assert!(max_step(&p.buf.data, a - 200, a + 200) < 0.05);
    assert!(max_step(&p.buf.data, e - 200, e + 200) < 0.05);
    let z = apply(&b, Sel::new(9_999, 30_001), &Edit::Silence).unwrap();
    assert!(max_step(&z.buf.data, 9_000, 31_000) < 0.03);
    let i = apply(
        &b,
        Sel::new(12_345, 12_345),
        &Edit::InsertSilence { frames: 4_800 },
    )
    .unwrap();
    assert!(max_step(&i.buf.data, 12_000, 18_000) < 0.03);
    let t = apply(&b, Sel::new(5_001, 40_017), &Edit::Trim).unwrap();
    assert!(t.buf.data[0].abs() < 0.01 && t.buf.data.last().unwrap().abs() < 0.01);
}

#[test]
fn snapped_edges_move_at_most_the_snap_window_and_say_where() {
    let b = AudioBuf::new(RATE, 1, sine(100.0, 0.5, 48_000));
    let s = Sel::new(1_100, 9_100);
    let out = apply(&b, s, &Edit::Gain { db: -6.0 }).unwrap();
    let w = (RATE as f32 * join::SNAP_MS / 1000.0) as usize;
    assert!(out.used.start.abs_diff(s.start) <= w && out.used.end.abs_diff(s.end) <= w);
    assert!(b.data[out.used.start].abs() < 0.01 && b.data[out.used.end].abs() < 0.01);
    // Outside what it says it used, nothing moved.
    assert!(same_bits(
        &out.buf.data[..out.used.start],
        &b.data[..out.used.start]
    ));
    assert!(same_bits(
        &out.buf.data[out.used.end..],
        &b.data[out.used.end..]
    ));
}

#[test]
fn an_edit_that_would_leave_nothing_is_refused() {
    let quiet = AudioBuf::new(RATE, 1, vec![0.0; 48_000]);
    let none = silence::Settings {
        threshold_db: -60.0,
        min_ms: 100.0,
        keep_ms: 0.0,
    };
    assert_eq!(
        apply(&quiet, Sel::whole(48_000), &Edit::RemoveSilence(none)).map(|e| e.buf.frames()),
        Err(EditError::WouldBeEmpty)
    );
    let b = AudioBuf::new(RATE, 1, sine(100.0, 0.5, 1_000));
    assert_eq!(
        apply_with(&b, Sel::whole(1_000), &Edit::Delete, &Joins::RAW).map(|_| ()),
        Err(EditError::WouldBeEmpty)
    );
}

#[test]
fn remove_silence_with_nothing_kept_touches_nothing_outside_the_selection() {
    let mut d = sine(300.0, 0.5, 24_000);
    d.extend(vec![0.0; 24_000]);
    d.extend(sine(300.0, 0.5, 24_000));
    let b = AudioBuf::new(RATE, 1, d);
    let s = Sel::new(20_000, 50_000);
    let set = silence::Settings {
        threshold_db: -60.0,
        min_ms: 100.0,
        keep_ms: 0.0,
    };
    let out = apply(&b, s, &Edit::RemoveSilence(set)).unwrap();
    assert!(out.changed);
    // Before the selection and after it (moved up), the bits.
    assert!(same_bits(&out.buf.data[..20_000], &b.data[..20_000]));
    let cut = (-out.delta_frames) as usize;
    assert!(same_bits(&out.buf.data[50_000 - cut..], &b.data[50_000..]));
}

#[test]
fn denoise_of_a_selection_meets_the_rest_smoothly_and_leaves_it_alone() {
    let n = RATE as usize * 2;
    let mut d = noise(3, n, 0.05);
    for (i, v) in d.iter_mut().enumerate() {
        *v += 0.3 * math::sin(i as f32 * 0.05);
    }
    let b = AudioBuf::new(RATE, 1, d);
    let profile = NoiseProfile::learn(&[noise(3, 20_000, 0.05)], RATE).unwrap();
    let s = Sel::new(30_000, 60_000);
    let out = apply(
        &b,
        s,
        &Edit::Denoise {
            profile,
            params: Params::default(),
        },
    )
    .unwrap();
    assert!(same_bits(&out.buf.data[..30_000], &b.data[..30_000]));
    assert!(same_bits(&out.buf.data[60_000..], &b.data[60_000..]));
    // At each edge the step is no larger than the take's own steps nearby.
    let own = max_step(&b.data, 29_000, 31_000);
    assert!(max_step(&out.buf.data, 29_990, 30_010) <= own * 1.2);
    assert!(max_step(&out.buf.data, 59_990, 60_010) <= own * 1.2);
}

#[test]
fn a_region_rewrites_only_what_it_can_reach() {
    let b = AudioBuf::new(RATE, 1, noise(5, 96_000, 0.4));
    let s = Sel::new(40_000, 50_000);
    let out = apply(
        &b,
        s,
        &Edit::Attenuate {
            lo_hz: 1_000.0,
            hi_hz: 4_000.0,
            gain_db: -30.0,
        },
    )
    .unwrap();
    let reach = N / 2 + HOP;
    let (a, e) = (s.start - reach, s.end + reach);
    assert!(
        same_bits(&out.buf.data[..a], &b.data[..a]),
        "before its reach"
    );
    assert!(
        same_bits(&out.buf.data[e..], &b.data[e..]),
        "after its reach"
    );
    assert_ne!(out.buf.data[s.start + 10], b.data[s.start + 10]);
}

#[test]
fn loudness_of_a_short_selection_is_too_short_to_measure() {
    let b = AudioBuf::new(RATE, 2, noise(1, 2 * 10_000, 0.3));
    assert_eq!(
        apply(
            &b,
            Sel::whole(10_000),
            &Edit::Normalize {
                level: Level::Lufs,
                target_db: -14.0,
                ceiling_db: None
            }
        )
        .map(|_| ()),
        Err(EditError::TooShort)
    );
}

#[test]
fn rms_normalize_stops_at_the_ceiling_or_says_it_clips() {
    // Spiky: a quiet bed and loud peaks, so an RMS target pushes the peaks over 0 dBFS.
    let mut d = noise(9, 48_000, 0.02);
    for i in (0..48_000).step_by(4_800) {
        d[i] = 0.9;
    }
    let b = AudioBuf::new(RATE, 1, d);
    let whole = Sel::whole(48_000);
    let free = apply(
        &b,
        whole,
        &Edit::Normalize {
            level: Level::Rms,
            target_db: -12.0,
            ceiling_db: None,
        },
    )
    .unwrap();
    assert!(matches!(free.note, Some(Note::Clips { peak_db }) if peak_db > 0.0));
    let held = apply(
        &b,
        whole,
        &Edit::Normalize {
            level: Level::Rms,
            target_db: -12.0,
            ceiling_db: Some(-1.0),
        },
    )
    .unwrap();
    assert!(matches!(held.note, Some(Note::Ceiling { .. })));
    let peak = 20.0 * math::log10(held.buf.peak(whole));
    assert!((peak + 1.0).abs() < 0.01, "{peak}");
}

#[test]
fn timing_is_kept_by_gain_like_edits_only() {
    let p = NoiseProfile::learn(&[noise(1, 10_000, 0.1)], RATE).unwrap();
    for e in [
        Edit::Gain { db: 1.0 },
        Edit::Invert,
        Edit::DcRemove,
        Edit::Silence,
        Edit::FadeIn {
            curve: crate::audio::FadeCurve::Linear,
        },
        Edit::Denoise {
            profile: p,
            params: Params::default(),
        },
        Edit::Repair,
    ] {
        assert!(e.keeps_timing(), "{}", e.name());
    }
    for e in [
        Edit::Reverse,
        Edit::Delete,
        Edit::Trim,
        Edit::Paste {
            clip: AudioBuf::new(RATE, 1, vec![0.0]),
        },
        Edit::InsertSilence { frames: 1 },
        Edit::RemoveSilence(silence::Settings::default()),
    ] {
        assert!(!e.keeps_timing(), "{}", e.name());
    }
}

#[test]
fn declick_and_repair_rebuild_a_click() {
    let clean = sine(330.0, 0.4, 48_000);
    let mut d = clean.clone();
    d[24_000] += 0.7;
    d[24_001] -= 0.5;
    let b = AudioBuf::new(RATE, 1, d);
    let found = apply(&b, Sel::whole(48_000), &Edit::Declick { sensitivity: 0.5 }).unwrap();
    assert_eq!(found.note, Some(Note::Clicks { count: 1 }));
    assert!((found.buf.data[24_000] - clean[24_000]).abs() < 0.02);
    let fixed = apply(&b, Sel::new(23_995, 24_010), &Edit::Repair).unwrap();
    assert!((fixed.buf.data[24_000] - clean[24_000]).abs() < 0.02);
    assert_eq!(
        apply(&b, Sel::new(1_000, 9_000), &Edit::Repair).map(|_| ()),
        Err(EditError::TooLong)
    );
}
