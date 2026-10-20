//! The stretcher under the conditions round 29's review found untested (ADR 0061): tempo
//! ramps under a clip, the level of a transposed render, a sustained partial under hits,
//! a DC offset, aliasing when reading faster, Rhythm's tails, the detector's own onsets,
//! and a window of a long file.

use super::measure::*;
use super::*;
use crate::math;
use crate::pitch::testkit::Noise;

const SR: f32 = 48_000.0;

fn tone(n: usize, hz: f64, amp: f32) -> Vec<f32> {
    let w = 2.0 * std::f64::consts::PI * hz / SR as f64;
    (0..n)
        .map(|i| amp * math::sin(w * i as f64) as f32)
        .collect()
}

/// A drum-like hit every `every` frames from `first` (as `stretch_tests`' own).
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
        formants: false,
        onsets,
        pitch: None,
        tail: Tail::Fade,
        origin: 0,
    }
}

/// A tempo ramp as the engine samples one: over `len` output frames the speed moves
/// linearly from `k0` to `k1` source frames per output frame, a point every 256 frames —
/// soft inside, hard at both ends.
fn ramp(len: usize, k0: f64, k1: f64) -> Map {
    let mut pts = Vec::new();
    let mut soft = Vec::new();
    let mut o = 0usize;
    loop {
        let t = o as f64;
        let s = k0 * t + (k1 - k0) * t * t / (2.0 * len as f64);
        pts.push((t, s));
        soft.push(o != 0 && o != len);
        if o == len {
            break;
        }
        o = (o + 256).min(len);
    }
    Map::with_soft(pts, soft).unwrap()
}

/// RMS levels, dB, of `x` in blocks of `block` frames over `from..to`.
fn block_levels(x: &[f32], from: usize, to: usize, block: usize) -> Vec<f32> {
    (from..to)
        .step_by(block)
        .filter(|b| b + block <= to)
        .map(|b| rms_db(&x[b..b + block]))
        .collect()
}

#[test]
fn rhythm_rides_through_a_tempo_ramp_without_chopping() {
    let x = tone(120_000, 440.0, 0.5);
    let len = 96_000;
    let map = ramp(len, 0.9, 1.1);
    let out = render(&[&x], SR, &req(Mode::Rhythm, &map, len, 0.0, &[]));
    let lv = block_levels(&out[0], 4_800, len - 4_800, 480);
    let (lo, hi) = lv
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    assert!(
        hi - lo < 0.5,
        "the level moves {} dB under a ramp ({lo}…{hi})",
        hi - lo
    );
}

#[test]
fn holds_keep_their_size_under_a_tempo_ramp() {
    let len = 192_000;
    let map = ramp(len, 0.8, 1.25);
    let onsets: Vec<f64> = (1..7).map(|i| map.source(i as f64 * 26_000.0)).collect();
    let (held, holds) = map.hold(&onsets, 2_048.0, 1_440.0);
    assert_eq!(holds.len(), onsets.len());
    for h in &holds {
        assert!(
            (h.at - h.from - 2_048.0).abs() < 1e-6 && (h.to - h.at - 1_440.0).abs() < 1e-6,
            "a hold shrank: {h:?}"
        );
        // A pure shift through it.
        for o in [h.from + 1.0, h.at, h.to - 1.0] {
            assert!(
                (held.slope(o) - 1.0).abs() < 1e-9,
                "slope {} at {o}",
                held.slope(o)
            );
        }
    }
    // The hard points stay, and the map never reads backwards.
    for p in map.hard_points() {
        assert!((held.source(p.0) - p.1).abs() < 1e-6, "{p:?} moved");
    }
    for w in held.points().windows(2) {
        assert!(w[1].1 >= w[0].1 && w[1].0 > w[0].0, "{w:?}");
    }
    // Away from the holds the curve is followed as before.
    let far = 13_000.0;
    assert!((held.source(far) - map.source(far)).abs() < 1e-6);
}

#[test]
fn attacks_stay_sharp_under_a_tempo_ramp() {
    let (x, at) = hits(192_000, 6_000, 24_000, 7);
    for (k0, k1) in [(0.8, 1.25), (1.25, 0.8)] {
        let len = (192_000.0 / ((k0 + k1) / 2.0)) as usize;
        let map = ramp(len, k0, k1);
        for mode in [Mode::Complex, Mode::Rhythm] {
            let out = render(&[&x], SR, &req(mode, &map, len, 0.0, &at));
            for &s in &at[1..at.len() - 1] {
                let o = map.output(s) as usize;
                if o + 2_000 > len {
                    continue;
                }
                let att = attack_frames(&out[0], SR, o - 240, o + 960).unwrap();
                let att0 = attack_frames(&x, SR, s as usize - 240, s as usize + 960).unwrap();
                assert!(
                    att <= att0 + 48,
                    "{mode:?} {k0}→{k1}: attack {att} frames (source {att0})"
                );
                let pre = pre_echo_db(&out[0], o, 1_200, 48);
                assert!(pre < -30.0, "{mode:?} {k0}→{k1}: pre-echo {pre} dB at {o}");
            }
        }
    }
}

/// A dense multi-sine (two hundred partials of random phase between 100 Hz and 5 kHz):
/// noise-like, and inside the band at every transposition tested (×4 up: under 20 kHz).
fn band_noise(n: usize, seed: u64) -> Vec<f32> {
    let mut g = Noise::new(seed);
    let parts: Vec<(f64, f64)> = (0..200)
        .map(|_| {
            let hz = 100.0 + 4_900.0 * (0.5 + 0.5 * g.next() as f64);
            (hz, std::f64::consts::PI * g.next() as f64)
        })
        .collect();
    (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let v: f64 = parts
                .iter()
                .map(|(hz, ph)| math::sin(2.0 * std::f64::consts::PI * hz * t + ph))
                .sum();
            (0.02 * v) as f32
        })
        .collect()
}

#[test]
fn a_transposed_render_keeps_its_level() {
    let f0 = 220.0;
    let x: Vec<f32> = (0..96_000)
        .map(|i| {
            let w = 2.0 * std::f64::consts::PI * f0 * i as f64 / SR as f64;
            (0.4 * math::sin(w) + 0.2 * math::sin(2.0 * w) + 0.1 * math::sin(3.0 * w)) as f32
        })
        .collect();
    let nz = band_noise(96_000, 11);
    let map = Map::identity();
    for (name, src) in [("tone", &x), ("noise", &nz)] {
        let want = rms_db(&src[24_000..72_000]);
        for semis in [-24.0, -12.0, -5.0, 7.0, 12.0, 24.0] {
            let out = render(&[src], SR, &req(Mode::Complex, &map, 96_000, semis, &[]));
            let got = rms_db(&out[0][24_000..72_000]);
            assert!(
                (got - want).abs() < 0.5,
                "{name} {semis:+} st: {:+.2} dB",
                got - want
            );
        }
    }
}

#[test]
fn a_held_tone_does_not_dip_under_hits() {
    let n = 192_000;
    let (h, at) = hits(n, 6_000, 12_000, 13);
    let t = tone(n, 440.0, 0.3);
    let x: Vec<f32> = h.iter().zip(&t).map(|(a, b)| a + b).collect();
    for stretch in [0.8, 1.25] {
        let len = (n as f64 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
        let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &at));
        // The tone's level just before each hit (the previous hit long died away).
        for &s in &at[2..at.len() - 1] {
            let o = map.output(s) as usize;
            let a = amplitude(&out[0][o - 2_400 - 240..o - 240], SR, 440.0);
            let db = 20.0 * math::log10(a / 0.3);
            assert!(
                db.abs() < 1.0,
                "×{stretch}: the tone is {db:+.2} dB before {o}"
            );
        }
    }
}

#[test]
fn repitch_just_over_speed_one_keeps_its_top_octave() {
    let x = tone(96_000, 15_000.0 / 1.02, 0.5);
    let len = 80_000;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, len as f64 * 1.02)]).unwrap();
    let out = render(&[&x], SR, &req(Mode::Repitch, &map, len, 0.0, &[]));
    let lvl = rms_db(&out[0][10_000..70_000]) - rms_db(&x[10_000..70_000]);
    assert!(lvl.abs() < 1.0, "a 15 kHz tone at speed 1.02: {lvl:+.2} dB");
}

#[test]
fn reading_faster_folds_nothing_back() {
    // Each tone ends above the output's Nyquist once read faster: nothing may come back.
    for (hz, semis) in [(20_000.0, 7.0), (15_000.0, 12.0), (13_000.0, 24.0)] {
        let x = tone(48_000, hz, 0.5);
        let src = rms_db(&x);
        let map = Map::identity();
        let out = render(&[&x], SR, &req(Mode::Rhythm, &map, 48_000, semis, &[]));
        let got = rms_db(&out[0][4_000..20_000]);
        assert!(
            got < src - 60.0,
            "Rhythm {hz} Hz {semis:+} st: {:.1} dB",
            got - src
        );
        let ratio = math::exp2(semis / 12.0);
        let g = hpss::grains(&[&x], SR, &map, 48_000, ratio, &[]);
        let got = rms_db(&g[0][4_000..20_000]);
        assert!(
            got < src - 60.0,
            "grains {hz} Hz {semis:+} st: {:.1} dB",
            got - src
        );
        let speed = Map::new(vec![(0.0, 0.0), (10_000.0, 10_000.0 * ratio)]).unwrap();
        let out = render(&[&x], SR, &req(Mode::Repitch, &speed, 10_000, 0.0, &[]));
        let got = rms_db(&out[0][1_000..9_000]);
        assert!(
            got < src - 60.0,
            "Re-pitch {hz} Hz ×{ratio}: {:.1} dB",
            got - src
        );
    }
}

#[test]
fn a_window_of_a_long_file_renders_as_the_whole_file_would() {
    let n = 20 * 48_000;
    let (h, at) = hits(n, 3_000, 21_000, 17);
    let nz = band_noise(n, 19);
    let x: Vec<f32> = h.iter().zip(&nz).map(|(a, b)| a + 0.5 * b).collect();
    // Two seconds of output reading source seconds 8 to 9.6.
    let len = 96_000;
    let map = Map::new(vec![(0.0, 384_000.0), (len as f64, 460_800.0)]).unwrap();
    let analysis = crate::pitch::analyse(&x, SR);
    for mode in [Mode::Complex, Mode::Melodic] {
        let mut q = req(mode, &map, len, 3.0, &at);
        q.pitch = Some(&analysis);
        let whole = dispatch(&[&x], SR, &q, 3.0);
        let windowed = render(&[&x], SR, &q);
        // And a caller passing only the span it reads.
        let (lo, hi) = reach(&q, SR, n);
        assert!(hi - lo < n / 4, "{mode:?}: reads {lo}..{hi} of {n}");
        let mut part = q.clone();
        part.origin = lo;
        let given = render(&[&x[lo..hi]], SR, &part);
        for (name, y) in [("windowed", &windowed), ("given", &given)] {
            let worst = y[0]
                .iter()
                .zip(&whole[0])
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            assert!(worst < 1e-4, "{mode:?} {name}: {worst} from the whole file");
        }
    }
}

#[test]
fn a_dc_offset_stays_constant_through_a_stretch() {
    let n = 144_000;
    let x: Vec<f32> = tone(n, 100.0, 0.3).iter().map(|v| v + 0.01).collect();
    let len = (n as f64 * 1.25) as usize;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
    let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &[]));
    // Means over 50 ms (five periods of the tone): the DC alone.
    for b in (24_000..len - 24_000).step_by(2_400) {
        let m: f64 = out[0][b..b + 2_400].iter().map(|v| *v as f64).sum::<f64>() / 2_400.0;
        assert!((m - 0.01).abs() < 1e-4, "the DC is {m} at {b}");
    }
}

#[test]
fn a_short_hold_still_gets_its_reset_frame() {
    // The transient's nearest frame (1 536) lies past the hold's end (1 400).
    let holds = [map::Hold {
        from: 1_100.0,
        at: 1_300.0,
        to: 1_400.0,
    }];
    let mut next = 0;
    let resets = (0..8)
        .filter(|j| pv::at_hold(&holds, &mut next, (j * 512) as f64, 512, false))
        .count();
    assert_eq!(resets, 1);
}

#[test]
fn rhythm_tails_loop_cleanly_and_stay_in_their_slice() {
    let (x, at) = hits(48_000, 2_000, 4_800, 23);
    let len = 96_000;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, 48_000.0)]).unwrap();
    let step = |v: &[f32]| {
        v.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    };
    let src_step = step(&x);
    for tail in [Tail::Loop, Tail::PingPong] {
        let mut q = req(Mode::Rhythm, &map, len, 0.0, &at);
        q.tail = tail;
        let a = render(&[&x], SR, &q);
        assert_eq!(a[0].len(), len);
        assert!(a == render(&[&x], SR, &q), "{tail:?}: not deterministic");
        // No jump at a wrap larger than the source's own sharpest.
        let worst = step(&a[0]);
        assert!(
            worst <= src_step * 1.05,
            "{tail:?}: a jump of {worst} ({src_step})"
        );
        // Before each transient, only the slice's own quiet tail — never the next hit.
        for &s in &at[1..at.len() - 1] {
            let o = map.output(s) as usize;
            let before = rms_db(&a[0][o - 96..o - 8]);
            let hit = rms_db(&a[0][o..o + 240]);
            assert!(
                before < hit - 12.0,
                "{tail:?} at {o}: {before} dB vs {hit} dB"
            );
        }
    }
}

#[test]
fn rhythm_on_detected_onsets_keeps_the_attacks() {
    let (x, at) = hits(192_000, 6_000, 24_000, 29);
    let found = crate::onset::detect(&x, SR).frames;
    // The detector finds every hit, and never late enough to cut into an attack.
    for &s in &at {
        let d = found
            .iter()
            .map(|f| f - s)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap();
        assert!(
            (-48.0..=24.0).contains(&d),
            "the hit at {s} was found {d:+} off"
        );
    }
    for stretch in [0.8, 1.25] {
        let len = (192_000.0 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, 192_000.0)]).unwrap();
        let out = render(&[&x], SR, &req(Mode::Rhythm, &map, len, 0.0, &found));
        for &s in &at[1..at.len() - 1] {
            let o = map.output(s) as usize;
            let att = attack_frames(&out[0], SR, o - 240, o + 960).unwrap();
            let att0 = attack_frames(&x, SR, s as usize - 240, s as usize + 960).unwrap();
            assert!(att <= att0 + 48, "×{stretch}: attack {att} (source {att0})");
            let pre = pre_echo_db(&out[0], o, 1_200, 48);
            assert!(pre < -24.0, "×{stretch}: pre-echo {pre} dB at {o}");
        }
    }
}
