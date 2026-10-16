//! The stretcher's tests (ADR 0061): identity to the bit, exact lengths, markers on their
//! frame, pitch, formants, transients, the stereo image, determinism.

use super::measure::*;
use super::*;
use crate::math;
use crate::pitch::testkit::{self, Noise, Vowel};

const SR: f32 = 48_000.0;

fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut g = Noise::new(seed);
    (0..n).map(|_| 0.5 * g.next()).collect()
}

fn tone(n: usize, hz: f32, phase: f32) -> Vec<f32> {
    let w = 2.0 * std::f64::consts::PI * hz as f64 / SR as f64;
    (0..n)
        .map(|i| 0.5 * math::sin(w * i as f64 + phase as f64) as f32)
        .collect()
}

/// A drum-like hit every `every` frames from `first`: a 0.2 ms rise into a decaying noise
/// burst with a low thump under it.
fn hits(n: usize, first: usize, every: usize, seed: u64) -> (Vec<f32>, Vec<f64>) {
    let mut x = vec![0.0f32; n];
    let mut g = Noise::new(seed);
    let mut at = Vec::new();
    let mut s = first;
    while s < n {
        at.push(s as f64);
        for k in 0..(0.12 * SR) as usize {
            if s + k >= n {
                break;
            }
            let t = k as f32 / SR;
            let env = math::exp(-t / 0.025) * (k as f32 / (0.0002 * SR)).min(1.0);
            let thump = math::sin(2.0 * std::f32::consts::PI * 70.0 * t) * math::exp(-t / 0.06);
            x[s + k] += 0.6 * env * g.next() + 0.4 * thump * (k as f32 / 10.0).min(1.0);
        }
        s += every;
    }
    (x, at)
}

fn req<'a>(mode: Mode, map: &'a Map, len: usize, semis: f64, onsets: &'a [f64]) -> Request<'a> {
    Request {
        mode,
        map,
        len,
        semitones: semis,
        formants: true,
        onsets,
        pitch: None,
        tail: Tail::Fade,
    }
}

/// A map stretching the source by `stretch` (output frames per source frame).
fn stretched(stretch: f64) -> Map {
    Map::new(vec![(0.0, 0.0), (stretch * 48_000.0, 48_000.0)]).unwrap()
}

#[test]
fn an_identity_returns_the_source_to_the_bit_in_every_mode() {
    let l = noise(20_000, 1);
    let r = noise(20_000, 2);
    let maps = [
        Map::identity(),
        // Markers on the identity line, as a warp on its own tempo makes them.
        Map::new(vec![(0.0, 0.0), (9_000.0, 9_000.0), (19_000.0, 19_000.0)]).unwrap(),
        // A whole-frame shift.
        Map::new(vec![(0.0, 7.0), (10_000.0, 10_007.0)]).unwrap(),
    ];
    let analysis = crate::pitch::analyse(&l, SR);
    for map in &maps {
        for mode in Mode::ALL {
            for chans in [vec![l.as_slice()], vec![l.as_slice(), r.as_slice()]] {
                let mut q = req(mode, map, 21_000, 0.0, &[]);
                q.pitch = Some(&analysis);
                let out = render(&chans, SR, &q);
                for (o, x) in out.iter().zip(&chans) {
                    assert_eq!(o.len(), 21_000);
                    let shift = map.plain_over(21_000).expect("plain");
                    for (t, v) in o.iter().enumerate() {
                        let s = t as i64 + shift;
                        let want = if s >= 0 && (s as usize) < x.len() {
                            x[s as usize]
                        } else {
                            0.0
                        };
                        assert_eq!(v.to_bits(), want.to_bits(), "{mode:?} at {t}");
                    }
                }
            }
        }
    }
}

#[test]
fn the_output_is_exactly_the_requested_length() {
    let x = noise(24_000, 3);
    let (h, at) = hits(24_000, 100, 6_000, 4);
    let a = crate::pitch::analyse(&x, SR);
    for mode in Mode::ALL {
        for stretch in [0.25, 0.5, 0.8, 1.25, 2.0, 4.0] {
            for semis in [0.0, 7.0, -12.0] {
                let map = stretched(stretch);
                let len = (24_000.0 * stretch) as usize + 13;
                let mut q = req(mode, &map, len, semis, &at);
                q.pitch = Some(&a);
                let out = render(&[&x, &h], SR, &q);
                assert_eq!(out.len(), 2);
                assert!(out.iter().all(|c| c.len() == len), "{mode:?} {stretch}");
                assert!(
                    out.iter().flatten().all(|v| v.is_finite()),
                    "{mode:?} {stretch} {semis}"
                );
            }
        }
    }
}

#[test]
fn a_hit_lands_where_its_marker_puts_it() {
    // Markers on four hits, moved unevenly: each hit must sound at its marker's frame.
    let (x, at) = hits(4 * 24_000, 2_000, 24_000, 5);
    let outs = [1_000.0, 31_000.0, 49_000.0, 90_000.0];
    let pts: Vec<(f64, f64)> = outs.iter().copied().zip(at.iter().copied()).collect();
    let map = Map::new(pts).unwrap();
    for mode in [Mode::Complex, Mode::Rhythm, Mode::Repitch] {
        let out = render(&[&x], SR, &req(mode, &map, 100_000, 0.0, &at));
        for &o in &outs {
            let rise = rise_at(&out[0], o as usize - 400, o as usize + 400, 0.25).unwrap();
            let err = rise as f64 - o;
            assert!(
                err.abs() <= 12.0,
                "{mode:?}: the hit at {o} rose at {rise} ({err:+})"
            );
        }
    }
}

#[test]
fn rhythm_puts_every_slice_on_its_frame_exactly() {
    let (x, at) = hits(4 * 24_000, 2_000, 24_000, 6);
    let outs = [1_000.0, 31_000.0, 49_000.0, 90_000.0];
    let map = Map::new(outs.iter().copied().zip(at.iter().copied()).collect()).unwrap();
    let out = render(&[&x], SR, &req(Mode::Rhythm, &map, 100_000, 0.0, &at));
    for (&o, &s) in outs.iter().zip(&at) {
        // The slice is the source from its transient on, sample for sample.
        for k in 0..2_000 {
            assert_eq!(
                out[0][o as usize + k].to_bits(),
                x[s as usize + k].to_bits(),
                "slice at {o}, frame {k}"
            );
        }
    }
}

#[test]
fn stretching_keeps_a_tones_pitch_and_level() {
    let x = tone(96_000, 440.0, 0.0);
    for stretch in [0.5, 0.8, 1.25, 2.0] {
        let map = stretched(stretch);
        let len = (96_000.0 * stretch) as usize;
        let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &[]));
        let mid = &out[0][len / 4..len * 3 / 4];
        let f = testkit::measured_f0(mid, SR, 440.0);
        assert!(cents(f, 440.0).abs() < 2.0, "{stretch}: {f} Hz");
        let wander = level_wander_db(&out[0], SR, 440.0, len / 4, len * 3 / 4);
        assert!(wander < 1.0, "{stretch}: the level wanders {wander} dB");
        let lvl = rms_db(mid) - rms_db(&x[24_000..72_000]);
        assert!(lvl.abs() < 1.0, "{stretch}: {lvl} dB");
    }
}

#[test]
fn repitch_moves_pitch_with_speed() {
    let x = tone(96_000, 440.0, 0.0);
    let map = stretched(2.0);
    let out = render(&[&x], SR, &req(Mode::Repitch, &map, 150_000, 12.0, &[]));
    let f = testkit::measured_f0(&out[0][20_000..140_000], SR, 220.0);
    assert!(cents(f, 220.0).abs() < 1.0, "{f}");
}

#[test]
fn transposing_moves_the_pitch_by_the_semitones() {
    let f0 = 220.0f32;
    let x: Vec<f32> = (0..96_000)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let w = 2.0 * std::f64::consts::PI * f0 as f64 * t;
            (0.4 * math::sin(w) + 0.2 * math::sin(2.0 * w) + 0.1 * math::sin(3.0 * w)) as f32
        })
        .collect();
    for semis in [-12.0, -5.0, 3.0, 7.0, 12.0] {
        for stretch in [1.0, 1.25, 0.8] {
            let map = stretched(stretch);
            let len = (96_000.0 * stretch) as usize;
            let out = render(&[&x], SR, &req(Mode::Complex, &map, len, semis, &[]));
            let want = f0 * math::exp2(semis as f32 / 12.0);
            let f = testkit::measured_f0(&out[0][len / 4..len * 3 / 4], SR, want);
            assert!(cents(f, want).abs() < 3.0, "{semis} st × {stretch}: {f} Hz");
        }
    }
}

#[test]
fn a_voice_shifted_keeps_its_formants() {
    let f0 = 150.0f32;
    let v = testkit::voice(SR, 1.5, Vowel::A, 0.5, |_| f0);
    let n = v.len();
    let map = Map::identity();
    for mode in [Mode::Complex, Mode::Melodic] {
        let a = crate::pitch::analyse(&v, SR);
        let mut on = req(mode, &map, n, 5.0, &[]);
        on.pitch = Some(&a);
        let mut off = on.clone();
        off.formants = false;
        let kept = render(&[&v], SR, &on);
        let moved = render(&[&v], SR, &off);
        let to = f0 * math::exp2(5.0 / 12.0);
        let mid = |x: &Vec<f32>| x[n / 4..n * 3 / 4].to_vec();
        let d_on = testkit::envelope_distance(&mid(&v), f0, &mid(&kept[0]), to, SR, 300.0, 4_000.0);
        let d_off =
            testkit::envelope_distance(&mid(&v), f0, &mid(&moved[0]), to, SR, 300.0, 4_000.0);
        assert!(d_on < 3.5, "{mode:?}: formants kept, {d_on} dB away");
        assert!(d_off > d_on + 2.0, "{mode:?}: kept {d_on}, moved {d_off}");
        let f = testkit::measured_f0(&mid(&kept[0]), SR, to);
        assert!(cents(f, to).abs() < 8.0, "{mode:?}: {f} Hz for {to}");
    }
}

#[test]
fn attacks_stay_sharp_and_nothing_rings_before_them() {
    let (x, at) = hits(96_000, 6_000, 12_000, 7);
    for mode in [Mode::Complex, Mode::Rhythm] {
        for stretch in [0.5, 0.8, 1.25, 2.0] {
            let map = stretched(stretch);
            let len = (96_000.0 * stretch) as usize;
            let out = render(&[&x], SR, &req(mode, &map, len, 0.0, &at));
            for &s in &at[1..at.len() - 1] {
                let o = map.output(s) as usize;
                let att = attack_frames(&out[0], SR, o - 240, o + 960).unwrap();
                let att0 = attack_frames(&x, SR, s as usize - 240, s as usize + 960).unwrap();
                assert!(
                    att <= att0 + 48,
                    "{mode:?} ×{stretch}: attack {att} frames (source {att0})"
                );
                let pre = pre_echo_db(&out[0], o, 1_200, 48);
                assert!(pre < -24.0, "{mode:?} ×{stretch}: pre-echo {pre} dB at {o}");
            }
        }
    }
}

#[test]
fn the_stereo_image_is_kept() {
    let l = tone(96_000, 330.0, 0.0);
    let r = tone(96_000, 330.0, -0.6);
    for stretch in [0.8, 1.5] {
        let map = stretched(stretch);
        let len = (96_000.0 * stretch) as usize;
        let out = render(&[&l, &r], SR, &req(Mode::Complex, &map, len, 4.0, &[]));
        let (a, b) = (&out[0][len / 4..len * 3 / 4], &out[1][len / 4..len * 3 / 4]);
        let hz = 330.0 * math::exp2(4.0 / 12.0);
        let d = channel_phase(a, b, SR, hz);
        assert!((d - 0.6).abs() < 0.05, "×{stretch}: {d} rad between channels");
    }
}

#[test]
fn renders_are_deterministic() {
    let (x, at) = hits(48_000, 1_000, 9_000, 8);
    let y = noise(48_000, 9);
    let map = Map::new(vec![(0.0, 0.0), (30_000.0, 20_000.0), (60_000.0, 48_000.0)]).unwrap();
    for mode in Mode::ALL {
        let a = render(&[&x, &y], SR, &req(mode, &map, 60_000, 2.5, &at));
        let b = render(&[&x, &y], SR, &req(mode, &map, 60_000, 2.5, &at));
        assert!(a == b, "{mode:?}");
    }
}

#[test]
fn the_map_inverts_and_holds_keep_the_breakpoints() {
    let map = Map::new(vec![(0.0, 0.0), (1_000.0, 500.0), (3_000.0, 4_000.0)]).unwrap();
    for s in [0.0, 100.0, 499.0, 500.0, 2_000.0, 3_999.0] {
        let o = map.output(s);
        assert!((map.source(o) - s).abs() < 1e-9, "{s}");
    }
    let held = map.held_at(&[250.0, 2_200.0], SR);
    for p in map.points() {
        assert!(
            (held.source(p.0) - p.1).abs() < 1e-9,
            "breakpoint {p:?} moved"
        );
    }
    // Around a transient the held map is a pure shift.
    let o = map.output(2_200.0);
    assert!((held.slope(o + 1.0) - 1.0).abs() < 1e-12);
    assert!((held.slope(o - 1.0) - 1.0).abs() < 1e-12);
    assert_eq!(Map::identity().plain_over(100), Some(0));
    assert_eq!(map.plain_over(100), None);
}
