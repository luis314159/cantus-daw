//! The pitch base's tests: synthetic voices with a known f0, scored by gross error rate,
//! fine error in cents and voicing accuracy — the targets ADR 0053 states.

use super::testkit::{self, Vowel};
use super::*;

/// Errors larger than this are gross (cents).
const GROSS_CENTS: f32 = 50.0;

/// What a run scored.
#[derive(Debug, Default)]
struct Score {
    voiced_frames: usize,
    gross: usize,
    fine_sum: f32,
    fine_max: f32,
    voicing_right: usize,
    voicing_frames: usize,
}

impl Score {
    fn gross_rate(&self) -> f32 {
        self.gross as f32 / self.voiced_frames.max(1) as f32
    }
    fn fine_mean(&self) -> f32 {
        self.fine_sum / (self.voiced_frames - self.gross).max(1) as f32
    }
    fn voicing(&self) -> f32 {
        self.voicing_right as f32 / self.voicing_frames.max(1) as f32
    }
}

fn cents(a: f32, b: f32) -> f32 {
    1200.0 * math::log2(a / b)
}

/// Score frames against the truth: `truth(t)` is the f0 at sample `t` (0: unvoiced);
/// frames within `guard` samples of a voicing change are not scored for voicing.
fn score(
    frames: &[PitchFrame],
    truth: &dyn Fn(f64) -> f32,
    guard: f64,
    settle: f64,
    len: usize,
) -> Score {
    let mut s = Score::default();
    let end = len as f64 - settle;
    for f in frames
        .iter()
        .filter(|f| f.centre >= settle && f.centre <= end)
    {
        let t = truth(f.centre);
        let near_change = [-guard, guard]
            .iter()
            .any(|g| (truth(f.centre + g) > 0.0) != (t > 0.0));
        if !near_change {
            s.voicing_frames += 1;
            if f.voiced == (t > 0.0) {
                s.voicing_right += 1;
            }
        }
        if t > 0.0 && f.voiced && !near_change {
            s.voiced_frames += 1;
            let e = cents(f.f0, t).abs();
            if e > GROSS_CENTS {
                s.gross += 1;
            } else {
                s.fine_sum += e;
                s.fine_max = s.fine_max.max(e);
            }
        }
    }
    s
}

fn stream(x: &[f32], sr: f32) -> Vec<PitchFrame> {
    let mut t = PitchTracker::new(sr);
    x.iter().filter_map(|&v| t.push(v)).collect()
}

/// Steady vowels from a bass's G2 to a soprano's G5: no gross error, a fraction of a cent.
#[test]
fn steady_vowels_are_tracked_to_a_cent_streaming_and_offline() {
    for sr in [44_100.0f32, 48_000.0, 96_000.0] {
        for (f0, vowel) in [
            (98.0f32, Vowel::A),
            (146.8, Vowel::U),
            (196.0, Vowel::I),
            (261.6, Vowel::A),
            (392.0, Vowel::A_FEMALE),
            (523.3, Vowel::A_FEMALE),
            (784.0, Vowel::A_FEMALE),
        ] {
            let x = testkit::voice(sr, 0.6, vowel, 0.5, |_| f0);
            let truth = |_: f64| f0;
            let st = score(&stream(&x, sr), &truth, 0.0, 0.06 * sr as f64, x.len());
            let off = score(
                &analyse(&x, sr).frames,
                &truth,
                0.0,
                0.06 * sr as f64,
                x.len(),
            );
            for (name, s) in [("streaming", &st), ("offline", &off)] {
                assert_eq!(s.gross, 0, "{name} {f0} Hz at {sr}: {s:?}");
                assert!(s.voicing() > 0.97, "{name} {f0} Hz at {sr}: {s:?}");
                assert!(s.fine_max < 2.0, "{name} {f0} Hz at {sr}: {s:?}");
            }
        }
    }
}

/// Vibrato and glides: the tracker follows them (the truth at each frame's centre).
#[test]
fn vibrato_and_glides_are_followed() {
    let sr = 48_000.0;
    let vib = testkit::vibrato(220.0, 60.0, 5.5);
    let x = testkit::voice(sr, 2.0, Vowel::A, 0.5, &vib);
    let truth = |t: f64| vib(t / sr as f64);
    let st = score(&stream(&x, sr), &truth, 0.0, 0.06 * sr as f64, x.len());
    let off = score(
        &analyse(&x, sr).frames,
        &truth,
        0.0,
        0.06 * sr as f64,
        x.len(),
    );
    eprintln!("vibrato: streaming {st:?}\n         offline {off:?}");
    assert!(st.gross_rate() < 0.01 && st.fine_mean() < 6.0, "{st:?}");
    assert!(off.gross_rate() < 0.01 && off.fine_mean() < 3.0, "{off:?}");

    let g = testkit::glide(150.0, 600.0, 1.0);
    let x = testkit::voice(sr, 1.5, Vowel::A, 0.5, &g);
    let truth = |t: f64| g(t / sr as f64);
    let st = score(&stream(&x, sr), &truth, 0.0, 0.06 * sr as f64, x.len());
    let off = score(
        &analyse(&x, sr).frames,
        &truth,
        0.0,
        0.06 * sr as f64,
        x.len(),
    );
    eprintln!("glide: streaming {st:?}\n       offline {off:?}");
    assert!(st.gross_rate() < 0.01 && st.fine_mean() < 10.0, "{st:?}");
    assert!(off.gross_rate() < 0.01 && off.fine_mean() < 5.0, "{off:?}");
}

/// The octave trap: a weak fundamental under a strong second harmonic is still `f0`.
#[test]
fn an_octave_ambiguous_tone_is_not_tracked_an_octave_up() {
    let sr = 48_000.0;
    for f0 in [110.0f32, 150.0, 330.0] {
        let x = testkit::octave_ambiguous(sr, 0.5, f0, 0.5);
        let truth = |_: f64| f0;
        let st = score(&stream(&x, sr), &truth, 0.0, 0.05 * sr as f64, x.len());
        let off = score(
            &analyse(&x, sr).frames,
            &truth,
            0.0,
            0.05 * sr as f64,
            x.len(),
        );
        assert_eq!(st.gross, 0, "streaming {f0}: {st:?}");
        assert_eq!(off.gross, 0, "offline {f0}: {off:?}");
    }
}

/// The lowest note: 58 Hz (a bass's lowest, below C2) is tracked.
#[test]
fn the_lowest_f0_is_tracked() {
    let sr = 48_000.0;
    let x = testkit::voice(sr, 0.8, Vowel::U, 0.5, |_| 58.0);
    let st = score(&stream(&x, sr), &|_| 58.0, 0.0, 0.08 * sr as f64, x.len());
    assert_eq!(st.gross, 0, "{st:?}");
    assert!(st.voicing() > 0.95, "{st:?}");
}

/// A phrase: notes, a sibilant, a breath, silence. The voicing does not wobble on the
/// breath and the "s", and every voiced frame is in tune.
pub(super) fn phrase(sr: f32) -> (Vec<f32>, Vec<(usize, usize, f32)>) {
    let mut x = Vec::new();
    let mut notes = Vec::new();
    let gap = |x: &mut Vec<f32>, s: f32| x.extend(std::iter::repeat_n(0.0, (s * sr) as usize));
    gap(&mut x, 0.1);
    for (i, (f0, vowel)) in [(196.0f32, Vowel::A), (246.9, Vowel::I), (220.0, Vowel::U)]
        .into_iter()
        .enumerate()
    {
        let v = testkit::voice(sr, 0.35, vowel, 0.4, |_| f0);
        notes.push((x.len(), x.len() + v.len(), f0));
        x.extend(v);
        match i {
            0 => x.extend(testkit::sibilant(sr, 0.12, 0.3, 7)),
            1 => x.extend(testkit::breath(sr, 0.15, 0.05, 9)),
            _ => gap(&mut x, 0.1),
        }
    }
    gap(&mut x, 0.1);
    (x, notes)
}

#[test]
fn breaths_and_sibilants_are_unvoiced_and_the_notes_are_in_tune() {
    let sr = 48_000.0;
    let (x, notes) = phrase(sr);
    let truth = |t: f64| {
        notes
            .iter()
            .find(|&&(a, b, _)| t >= a as f64 && t < b as f64)
            .map(|n| n.2)
            .unwrap_or(0.0)
    };
    // The streaming decision is one window late at a change: score away from them.
    let st = score(&stream(&x, sr), &truth, 0.04 * sr as f64, 0.0, x.len());
    let off = score(
        &analyse(&x, sr).frames,
        &truth,
        0.02 * sr as f64,
        0.0,
        x.len(),
    );
    eprintln!("phrase: streaming {st:?}\n        offline {off:?}");
    assert!(st.voicing() > 0.97, "streaming voicing {st:?}");
    assert!(off.voicing() > 0.97, "offline voicing {off:?}");
    assert_eq!(st.gross, 0, "{st:?}");
    assert_eq!(off.gross, 0, "{off:?}");
}

/// The streaming tracker comes to rest after silence, exactly: a new tracker skipped as
/// far gives the same frames for whatever follows.
#[test]
fn the_tracker_rests_exactly() {
    let sr = 48_000.0;
    let x = testkit::voice(sr, 0.3, Vowel::A, 0.5, |_| 200.0);
    let mut a = PitchTracker::new(sr);
    for &v in &x {
        a.push(v);
    }
    assert!(!a.at_rest());
    let mut silent = 0;
    while !a.at_rest() {
        a.push(0.0);
        silent += 1;
        assert!(silent < 10_000, "no rest after {silent} samples of silence");
    }
    let mut b = PitchTracker::new(sr);
    assert!(b.at_rest());
    b.skip(a.count() as usize);
    let y = testkit::voice(sr, 0.3, Vowel::I, 0.5, |_| 300.0);
    for &v in &y {
        assert_eq!(a.push(v), b.push(v));
    }
}

/// The offline analysis is a function of the take: the same bits twice; and it is fast.
#[test]
fn the_offline_analysis_is_deterministic_and_fast() {
    let sr = 48_000.0;
    let vib = testkit::vibrato(180.0, 40.0, 5.0);
    let x = testkit::voice(sr, 20.0, Vowel::A, 0.5, &vib);
    let t0 = std::time::Instant::now();
    let a = analyse(&x, sr);
    let took = t0.elapsed().as_secs_f32();
    let b = analyse(&x, sr);
    assert_eq!(a, b);
    let per_minute = took * 3.0;
    eprintln!("offline analysis: {per_minute:.2} s per minute of audio at 48 kHz");
    assert!(per_minute < 6.0, "{per_minute} s per minute");
}

/// Epochs: one period apart, every voiced stretch covered, each on the same phase (the
/// waveform around consecutive marks matches).
#[test]
fn epochs_are_one_period_apart_and_phase_locked() {
    let sr = 48_000.0;
    let vib = testkit::vibrato(160.0, 30.0, 5.0);
    let x = testkit::voice(sr, 1.0, Vowel::A, 0.5, &vib);
    let a = analyse(&x, sr);
    assert!(a.epochs.len() > 140, "{} epochs", a.epochs.len());
    for w in a.epochs.windows(2) {
        let gap = (w[1].at - w[0].at) as f32;
        let p = w[0].period;
        assert!((gap / p - 1.0).abs() < 0.05, "gap {gap} against period {p}");
    }
    // The waveform around consecutive marks: correlation near 1.
    let mut worst = 1.0f32;
    for w in a.epochs.windows(2).skip(5) {
        let h = (w[0].period * 0.5) as i64;
        let (p, q) = (w[0].at.round() as i64, w[1].at.round() as i64);
        if p - h < 0 || q + h >= x.len() as i64 {
            continue;
        }
        let (mut xy, mut xx, mut yy) = (0.0f32, 0.0f32, 0.0f32);
        for j in -h..=h {
            let (u, v) = (x[(p + j) as usize], x[(q + j) as usize]);
            xy += u * v;
            xx += u * u;
            yy += v * v;
        }
        worst = worst.min(xy / (xx * yy).sqrt().max(1e-20));
    }
    assert!(
        worst > 0.95,
        "consecutive periods correlate at only {worst}"
    );
}

/// The lowest f0 the design reaches is F_MIN at every sample rate.
#[test]
fn the_design_reaches_f_min_and_f_max() {
    for sr in [
        22_050.0f32,
        44_100.0,
        48_000.0,
        88_200.0,
        96_000.0,
        192_000.0,
    ] {
        let d = Design::new(sr);
        let fs = d.coarse_rate();
        assert!(fs / d.tau_max as f32 <= F_MIN, "{sr}");
        assert!(fs / d.tau_min as f32 >= F_MAX, "{sr}");
        assert!(
            d.window >= (fs / F_MIN) as usize,
            "{sr}: the window holds a period of F_MIN"
        );
        let sum: f32 = d.fir.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
    }
}

/// Review 1: a legato leap up an octave (220 → 440 Hz, no gap) is followed, not held an
/// octave low by the octave guard.
#[test]
fn a_legato_octave_leap_up_is_followed() {
    let sr = 48_000.0;
    let x = testkit::voice(
        sr,
        1.0,
        Vowel::A,
        0.5,
        |t| if t < 0.5 { 220.0 } else { 440.0 },
    );
    let step = 0.5 * sr as f64;
    let truth = |t: f64| if t < step { 220.0 } else { 440.0 };
    let frames: Vec<PitchFrame> = stream(&x, sr)
        .into_iter()
        .filter(|f| f.centre > step + 0.04 * sr as f64)
        .collect();
    let s = score(&frames, &truth, 0.0, 0.0, x.len());
    assert_eq!(s.gross, 0, "{s:?}");
    // Followed, not dropped: the frames after the leap are voiced and on the new pitch
    // (a tracker that unvoiced them would score no gross error at all).
    let followed = |s: &Score, n: usize| s.voiced_frames * 10 >= n * 9 && s.fine_max < 10.0;
    assert!(
        followed(&s, frames.len()),
        "up: {} frames, {s:?}",
        frames.len()
    );
    // And a leap down.
    let y = testkit::voice(
        sr,
        1.0,
        Vowel::A,
        0.5,
        |t| if t < 0.5 { 440.0 } else { 220.0 },
    );
    let truth = |t: f64| if t < step { 440.0 } else { 220.0 };
    let frames: Vec<PitchFrame> = stream(&y, sr)
        .into_iter()
        .filter(|f| f.centre > step + 0.04 * sr as f64)
        .collect();
    let s = score(&frames, &truth, 0.0, 0.0, y.len());
    assert_eq!(s.gross, 0, "down: {s:?}");
    assert!(
        followed(&s, frames.len()),
        "down: {} frames, {s:?}",
        frames.len()
    );
}

/// Review 1: the Viterbi pass renormalises each frame, so a long take keeps the precision
/// of a short one (the log-probabilities stay near zero).
#[test]
fn the_viterbi_pass_stays_near_zero_on_long_takes() {
    // 4 000 unvoiced frames (21 s at 48 kHz): without renormalising, about −33 000.
    let peak = super::offline::viterbi_peak_of_silence(4_000);
    assert!(peak < 100.0, "the log-probabilities drifted to {peak}");
}

/// Review 1: `f0_at` is symmetric at the edges of a voiced stretch: the nearest frame
/// decides, at an onset as at an offset.
#[test]
fn f0_at_follows_the_nearest_frame_at_both_edges() {
    let frame = |voiced: bool| PitchFrame {
        f0: if voiced { 200.0 } else { 0.0 },
        voiced,
        ..PitchFrame::default()
    };
    let onset = Analysis {
        sample_rate: 48_000.0,
        hop: 100,
        frames: vec![frame(false), frame(true)],
        epochs: Vec::new(),
    };
    assert_eq!(onset.f0_at(25.0), None, "an onset: nearest frame unvoiced");
    assert_eq!(onset.f0_at(75.0), Some(200.0));
    let offset = Analysis {
        frames: vec![frame(true), frame(false)],
        ..onset.clone()
    };
    assert_eq!(offset.f0_at(25.0), Some(200.0));
    assert_eq!(
        offset.f0_at(75.0),
        None,
        "an offset: nearest frame unvoiced"
    );
}
