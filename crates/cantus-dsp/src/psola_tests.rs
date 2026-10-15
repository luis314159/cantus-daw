//! The shifter's tests: the latency, the null, the pitch it lands on, the formants it
//! keeps, no click when the ratio jumps, exact rest, and the offline form.

use super::*;
use crate::pitch::testkit::{self, envelope_distance, measured_f0, Vowel};
use crate::pitch::{analyse, PitchTracker};

fn st(semitones: f32) -> f32 {
    math::exp2(semitones / 12.0)
}

fn cents(a: f32, b: f32) -> f32 {
    1200.0 * math::log2(a / b)
}

/// Run a mono signal through the streaming shifter at a fixed control.
fn run(x: &[f32], sr: f32, c: &Control) -> Vec<f32> {
    let mut s = Shifter::new(sr);
    x.iter().map(|&v| s.tick(v, v, c).0).collect()
}

#[test]
fn the_latency_is_fixed_and_under_three_milliseconds() {
    for (sr, want) in [(44_100.0f32, 118usize), (48_000.0, 128), (96_000.0, 256)] {
        let s = Shifter::new(sr);
        assert_eq!(s.latency(), want, "{sr}");
        assert!(s.latency() as f32 / sr < 0.003, "{sr}");
    }
    assert!(Shifter::new(48_000.0).latency() <= 144);
}

/// Not engaged, it is a delay line: the input `latency` samples later, bit for bit.
#[test]
fn released_it_is_the_input_delayed_bit_for_bit() {
    let sr = 48_000.0;
    let mut ns = testkit::Noise::new(3);
    let l: Vec<f32> = (0..20_000).map(|_| ns.next()).collect();
    let r: Vec<f32> = (0..20_000).map(|_| ns.next() * 0.3).collect();
    let mut s = Shifter::new(sr);
    let lat = s.latency();
    let c = Control {
        period: 200.0,
        ratio: 1.3,
        formant: 0.8,
        engage: false,
    };
    for i in 0..l.len() {
        let (a, b) = s.tick(l[i], r[i], &c);
        let (wl, wr) = if i >= lat {
            (l[i - lat], r[i - lat])
        } else {
            (0.0, 0.0)
        };
        assert_eq!(a.to_bits(), wl.to_bits(), "left at {i}");
        assert_eq!(b.to_bits(), wr.to_bits(), "right at {i}");
    }
}

/// Engaged at a ratio of 1 with the voice's own period, the output is the voice: the same
/// pitch, the same spectral envelope, the same level (the grains come from up to a period
/// earlier, so the waveform is not compared sample by sample).
#[test]
fn engaged_at_unity_it_reproduces_the_voice() {
    let sr = 48_000.0;
    let f0 = 200.0;
    let x = testkit::voice(sr, 0.5, Vowel::A, 0.5, |_| f0);
    let c = Control {
        period: sr / f0,
        ratio: 1.0,
        formant: 1.0,
        engage: true,
    };
    let y = run(&x, sr, &c);
    let (xs, ys) = (&x[4_800..], &y[4_800..]);
    let got = measured_f0(ys, sr, f0);
    assert!(cents(got, f0).abs() < 0.5, "{got} Hz");
    let d = envelope_distance(xs, f0, ys, f0, sr, 200.0, 5_000.0);
    assert!(d < 0.5, "envelope {d} dB");
    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
    let level = 20.0 * math::log10(rms(ys) / rms(xs));
    assert!(level.abs() < 0.2, "level {level} dB");
}

/// The output lands on the ratio, in tune, up and down.
#[test]
fn the_output_pitch_is_the_ratio() {
    let sr = 48_000.0;
    for (f0, semis) in [
        (200.0f32, 3.0f32),
        (200.0, -5.0),
        (110.0, 7.0),
        (330.0, -12.0),
        (440.0, 1.0),
    ] {
        let x = testkit::voice(sr, 0.8, Vowel::A, 0.5, |_| f0);
        let c = Control {
            period: sr / f0,
            ratio: st(semis),
            formant: 1.0,
            engage: true,
        };
        let y = run(&x, sr, &c);
        let want = f0 * st(semis);
        let got = measured_f0(&y, sr, want);
        assert!(
            cents(got, want).abs() < 2.0,
            "{f0} Hz {semis:+} st: {got} Hz, want {want}"
        );
    }
}

/// Formants stay where they were (the envelope distance is small with the formants kept,
/// large when the grains are resampled with the pitch), and the throat moves them.
#[test]
fn the_formants_are_preserved_and_the_throat_moves_them() {
    let sr = 48_000.0;
    let f0 = 180.0;
    let x = testkit::voice(sr, 0.8, Vowel::A, 0.5, |_| f0);
    for semis in [-3.0f32, 3.0, 5.0] {
        let ratio = st(semis);
        let kept = run(
            &x,
            sr,
            &Control {
                period: sr / f0,
                ratio,
                formant: 1.0,
                engage: true,
            },
        );
        let moved = run(
            &x,
            sr,
            &Control {
                period: sr / f0,
                ratio,
                formant: ratio,
                engage: true,
            },
        );
        let tail = |v: &[f32]| v[v.len() / 4..].to_vec();
        let d_kept = envelope_distance(&tail(&x), f0, &tail(&kept), f0 * ratio, sr, 300.0, 4_000.0);
        let d_moved =
            envelope_distance(&tail(&x), f0, &tail(&moved), f0 * ratio, sr, 300.0, 4_000.0);
        eprintln!("{semis:+} st: envelope distance kept {d_kept:.2} dB, moved {d_moved:.2} dB");
        assert!(d_kept < 3.0, "{semis:+} st: kept {d_kept} dB");
        assert!(
            d_moved > d_kept + 2.0,
            "{semis:+} st: moved {d_moved} vs kept {d_kept}"
        );
    }
}

/// A ratio that jumps (the hard-tune step) makes no click: the output's second difference
/// around the jump is no larger than in the steady parts.
#[test]
fn a_ratio_jump_makes_no_click() {
    let sr = 48_000.0;
    let f0 = 200.0;
    let x = testkit::voice(sr, 1.0, Vowel::A, 0.5, |_| f0);
    let mut s = Shifter::new(sr);
    let mut y = Vec::with_capacity(x.len());
    for (i, &v) in x.iter().enumerate() {
        let ratio = if (i / 4_800) % 2 == 0 { 1.0 } else { st(2.0) };
        let c = Control {
            period: sr / f0,
            ratio,
            formant: 1.0,
            engage: true,
        };
        y.push(s.tick(v, v, &c).0);
    }
    let d2 = |v: &[f32], i: usize| (v[i + 1] - 2.0 * v[i] + v[i - 1]).abs();
    let steady = (1_000..4_000).map(|i| d2(&x, i)).fold(0.0f32, f32::max);
    let worst = (2_400..y.len() - 1)
        .map(|i| d2(&y, i))
        .fold(0.0f32, f32::max);
    assert!(
        worst < steady * 1.6,
        "click: {worst} against a steady {steady}"
    );
}

/// Engaging and releasing fade; after silence it comes to rest, exactly as new.
#[test]
fn it_rests_exactly_after_silence() {
    let sr = 48_000.0;
    let x = testkit::voice(sr, 0.3, Vowel::A, 0.5, |_| 220.0);
    let mut a = Shifter::new(sr);
    let on = Control {
        period: sr / 220.0,
        ratio: st(1.0),
        formant: st(2.0),
        engage: true,
    };
    for &v in &x {
        a.tick(v, -v, &on);
    }
    assert!(!a.at_rest());
    let off = Control::default();
    let mut n = 0;
    while !a.at_rest() {
        a.tick(0.0, 0.0, &off);
        n += 1;
        assert!(n < 200_000, "no rest");
    }
    // At rest, silence in is silence out, and it stays at rest.
    for _ in 0..1_000 {
        assert_eq!(a.tick(0.0, 0.0, &off), (0.0, 0.0));
        assert!(a.at_rest());
    }
    n += 1_000;
    let mut b = Shifter::new(sr);
    b.skip(x.len() + n);
    for &v in &x {
        let (p, q) = (a.tick(v, v * 0.5, &on), b.tick(v, v * 0.5, &on));
        assert_eq!(
            (p.0.to_bits(), p.1.to_bits()),
            (q.0.to_bits(), q.1.to_bits())
        );
    }
}

/// Streaming with the tracker: a sung line corrected to a fixed note lands on it.
#[test]
fn tracker_and_shifter_together_land_on_the_target() {
    let sr = 48_000.0;
    let vib = testkit::vibrato(207.0, 30.0, 5.0);
    let x = testkit::voice(sr, 1.0, Vowel::A, 0.5, &vib);
    let mut t = PitchTracker::new(sr);
    let mut s = Shifter::new(sr);
    let target = 220.0;
    let mut c = Control::default();
    let y: Vec<f32> = x
        .iter()
        .map(|&v| {
            if let Some(f) = t.push(v) {
                c = Control {
                    period: f.period,
                    ratio: if f.voiced { target / f.f0 } else { 1.0 },
                    formant: 1.0,
                    engage: f.voiced,
                };
            }
            s.tick(v, v, &c).0
        })
        .collect();
    let got = measured_f0(&y[y.len() / 3..], sr, target);
    assert!(cents(got, target).abs() < 3.0, "{got} Hz");
}

// ---- the offline form -----------------------------------------------------------------

#[test]
fn offline_with_nothing_to_do_is_the_take_bit_for_bit() {
    let sr = 48_000.0;
    let x = testkit::voice(sr, 0.5, Vowel::I, 0.5, |_| 240.0);
    let a = analyse(&x, sr);
    let plan = Plan {
        ratio: &|_| 1.0,
        formant: &|_| 0.0,
        time: TimeMap::identity(),
        len: x.len(),
    };
    let y = render(&[&x], &a, &plan);
    assert!(y[0].iter().zip(&x).all(|(p, q)| p.to_bits() == q.to_bits()));
}

#[test]
fn offline_shifts_pitch_keeps_formants_and_stretches_time() {
    let sr = 48_000.0;
    let f0 = 190.0;
    let x = testkit::voice(sr, 1.0, Vowel::A, 0.5, |_| f0);
    let a = analyse(&x, sr);
    // +4 semitones from 0.3 s on.
    let cut = 0.3 * sr as f64;
    let ratio = move |t: f64| if t >= cut { st(4.0) } else { 1.0 };
    let plan = Plan {
        ratio: &ratio,
        formant: &|_| 0.0,
        time: TimeMap::identity(),
        len: x.len(),
    };
    let y = render(&[&x], &a, &plan);
    let late = &y[0][(0.5 * sr) as usize..];
    let got = measured_f0(late, sr, f0 * st(4.0));
    assert!(cents(got, f0 * st(4.0)).abs() < 2.0, "{got}");
    // Before the change (and the fade), the take itself.
    let fade = (FADE_S * sr) as usize + 2;
    let before = (cut as usize) - fade;
    assert!(y[0][..before]
        .iter()
        .zip(&x)
        .all(|(p, q)| p.to_bits() == q.to_bits()));
    let d = envelope_distance(
        &x[(0.5 * sr) as usize..],
        f0,
        late,
        f0 * st(4.0),
        sr,
        300.0,
        4_000.0,
    );
    assert!(d < 3.0, "formants moved by {d} dB");

    // Half again as long, the pitch unchanged.
    let len = (x.len() as f64 * 1.5) as usize;
    let plan = Plan {
        ratio: &|_| 1.0,
        formant: &|_| 0.0,
        time: TimeMap::new(vec![(0.0, 0.0), (len as f64, x.len() as f64)]).unwrap(),
        len,
    };
    let y = render(&[&x], &a, &plan);
    assert_eq!(y[0].len(), len);
    let got = measured_f0(&y[0][len / 4..len * 3 / 4], sr, f0);
    assert!(cents(got, f0).abs() < 2.0, "stretched: {got}");
    // Deterministic.
    assert_eq!(render(&[&x], &a, &plan), y);
}
