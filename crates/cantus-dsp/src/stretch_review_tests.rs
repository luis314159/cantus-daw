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
            // ADR 0061 §10's claim (round 29's second review: the test held 0.5).
            assert!(
                (got - want).abs() < 0.1,
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
    // And faster than recorded (Re-pitch through its band-limited reader).
    let fast = Map::new(vec![(0.0, 384_000.0), (len as f64, 504_000.0)]).unwrap();
    for (mode, map) in [
        (Mode::Complex, &map),
        (Mode::Melodic, &map),
        (Mode::Rhythm, &map),
        (Mode::Repitch, &map),
        (Mode::Repitch, &fast),
    ] {
        let mut q = req(mode, map, len, 3.0, &at);
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

/// Round 29's second review (finding 13): untransposed, a frame inside a hold resets even
/// when a short hold just before it (a flam) still waits for its own nearest frame.
#[test]
fn a_flams_second_hold_still_resets_untransposed() {
    let holds = [
        map::Hold {
            from: 3_008.0,
            at: 3_200.0,
            to: 3_470.0,
        },
        map::Hold {
            from: 3_608.0,
            at: 3_800.0,
            to: 5_240.0,
        },
    ];
    let mut next = 0;
    let hit: Vec<bool> = (0..4)
        .map(|j| pv::at_hold(&holds, &mut next, (j * 2_048) as f64, 2_048, true))
        .collect();
    assert_eq!(
        hit,
        vec![false, false, true, false],
        "frame 4 096 lies in the second"
    );
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

/// Round 29's second review (finding 15): the bench's Complex rows on a mix with the
/// detector's own onsets, as a test — drum hits over a held tone: every attack as sharp
/// as the source's, nothing ringing before it, every hit on its place. Measured on the
/// signals twice differenced (the held tone, 63 dB down, never read as a hit's smear).
#[test]
fn complex_on_a_mix_with_detected_onsets_keeps_the_hits() {
    let n = 192_000;
    let (h, at) = hits(n, 6_000, 24_000, 37);
    let pad = tone(n, 196.0, 0.08);
    let x: Vec<f32> = h.iter().zip(&pad).map(|(a, b)| a + b).collect();
    let found = crate::onset::detect(&x, SR).frames;
    let diff2 = |v: &[f32]| -> Vec<f32> {
        (0..v.len())
            .map(|i| {
                let g = |j: usize| v.get(j).copied().unwrap_or(0.0);
                g(i) - 2.0 * if i > 0 { g(i - 1) } else { 0.0 } + if i > 1 { g(i - 2) } else { 0.0 }
            })
            .collect()
    };
    let xd = diff2(&x);
    for stretch in [0.8, 1.25] {
        let len = (n as f64 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
        let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &found));
        let out = [diff2(&out[0])];
        let x = &xd;
        for &s in &at[1..at.len() - 1] {
            let o = map.output(s) as usize;
            let att = attack_frames(&out[0], SR, o - 240, o + 960).unwrap();
            let att0 = attack_frames(x, SR, s as usize - 240, s as usize + 960).unwrap();
            assert!(att <= att0 + 48, "×{stretch}: attack {att} (source {att0})");
            let pre = pre_echo_db(&out[0], o, 1_200, 48);
            assert!(pre < -24.0, "×{stretch}: pre-echo {pre} dB at {o}");
            let got = rise_at(&out[0], o - 480, o + 480, 0.5).unwrap() as f64;
            let want = rise_at(x, s as usize - 480, s as usize + 480, 0.5).unwrap() as f64;
            let late = got - o as f64 - (want - s);
            assert!(
                late.abs() <= 24.0,
                "×{stretch}: the hit {late:+} frames off"
            );
        }
    }
}

/// Round 29's review: a chord's voices 30 Hz apart, each a detuned saw a channel, keep
/// their level through a stretch — every voice in both channels, within a decibel of its
/// own wander (100 ms windows: they resolve the voices).
#[test]
fn a_chords_close_voices_do_not_wander() {
    // Two detuned saws a note, one per channel (as the bench's pad).
    let notes = [48, 52, 55, 60];
    let n = 48_000usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for (v, &note) in notes.iter().enumerate() {
        for (d, detune) in [-0.08f64, 0.08].iter().enumerate() {
            let f = crate::pitch::midi_to_hz(note as f32) as f64 * (1.0 + detune / 100.0 * 12.0);
            let mut ph = (v * 7 + d * 3) as f64 * 0.37;
            for i in 0..n {
                let t = i as f64 / SR as f64;
                ph += f / SR as f64;
                let saw = 2.0 * (ph - ph.floor()) - 1.0;
                let env = (t / 0.3).min(1.0) * ((1.0 - t) / 0.05).clamp(0.0, 1.0);
                let s = (0.05 * env * saw) as f32;
                if d == 0 {
                    l[i] += s
                } else {
                    r[i] += s
                }
            }
        }
    }
    for stretch in [0.8, 1.25] {
        let len = (n as f64 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
        let y = render(&[&l, &r], SR, &req(Mode::Complex, &map, len, 0.0, &[]));
        for (ch, detune, src) in [(0usize, -0.08f64, &l), (1, 0.08, &r)] {
            for note in notes {
                let hz = (crate::pitch::midi_to_hz(note as f32) as f64
                    * (1.0 + detune / 100.0 * 12.0)) as f32;
                let own =
                    level_wander_db_in(src, SR, hz, (0.35 * SR) as usize, (0.9 * SR) as usize, 0.1);
                let w = level_wander_db_in(
                    &y[ch],
                    SR,
                    hz,
                    (0.35 * SR as f64 * stretch) as usize,
                    (0.9 * SR as f64 * stretch) as usize,
                    0.1,
                );
                assert!(
                    w - own < 1.0,
                    "x{stretch} ch{ch} {hz:.1} Hz: {own:.2} dB, {w:.2} dB"
                );
            }
        }
    }
}

/// Short tuned hits (a tom's ping and its click, 40 ms): a burst of noise and a decaying
/// 300 Hz tone — the harmonic part holds the tone — every `every` frames from `first`.
fn pings(n: usize, first: usize, every: usize, seed: u64) -> (Vec<f32>, Vec<f64>) {
    let mut x = vec![0.0f32; n];
    let mut g = Noise::new(seed);
    let mut at = Vec::new();
    let mut s = first;
    while s < n {
        at.push(s as f64);
        for k in 0..(0.04 * SR) as usize {
            if s + k >= n {
                break;
            }
            let t = k as f32 / SR;
            let ping = math::sin(2.0 * std::f32::consts::PI * 300.0 * t) * math::exp(-t / 0.012);
            let click = math::exp(-t / 0.002) * g.next();
            x[s + k] += (0.5 * ping + 0.3 * click) * (k as f32 / 8.0).min(1.0);
        }
        s += every;
    }
    (x, at)
}

/// Hats: a short noise burst every `every` frames from `first`.
fn hats(n: usize, first: usize, every: usize, seed: u64) -> (Vec<f32>, Vec<f64>) {
    let mut x = vec![0.0f32; n];
    let mut g = Noise::new(seed);
    let mut at = Vec::new();
    let mut s = first;
    while s < n {
        at.push(s as f64);
        for k in 0..(0.04 * SR) as usize {
            if s + k >= n {
                break;
            }
            let t = k as f32 / SR;
            x[s + k] += 0.5 * math::exp(-t / 0.008) * (k as f32 / 8.0).min(1.0) * g.next();
        }
        s += every;
    }
    (x, at)
}

/// Round 29's second review (finding 12): a Complex render's copied hits keep the
/// source's level slowed and sped up — the level match holds the vocoder's own frames to
/// what they mean, never the hits' power counted again at every frame that reads near them.
/// A drum hit every half second, and hats every sixteenth at 120 BPM: each hit's peak and
/// its energy over its first 30 ms within 0.3 dB of the source's, on average.
#[test]
fn a_complex_renders_hits_keep_their_level_slowed_and_sped_up() {
    for every in [24_000usize, 6_000] {
        let n = 10 * 48_000;
        let (x, at) = if every == 6_000 {
            hats(n, 12_000, every, 31)
        } else {
            hits(n, 12_000, every, 31)
        };
        for k in [0.5f64, 2.0] {
            // Output frame t reads source t × k (slowed at 0.5).
            let len = (n as f64 / k) as usize - 48_000;
            let map = Map::new(vec![(0.0, 0.0), (len as f64, len as f64 * k)]).unwrap();
            let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &at));
            let span = (0.030 * SR) as usize;
            let (mut peaks, mut energies) = (Vec::new(), Vec::new());
            for s in at.iter().copied().filter(|s| *s > 24_000.0) {
                let o = map.output(s).round() as usize;
                if o + span + 48 >= len {
                    break;
                }
                let s = s as usize;
                let db = |a: f64, b: f64| 10.0 * math::log10(a / b.max(1e-30));
                let pk = |v: &[f32]| v.iter().fold(0.0f32, |m, x| m.max(x.abs())) as f64;
                let en = |v: &[f32]| v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>();
                let (src, got) = (&x[s - 48..s + span], &out[0][o - 48..o + span]);
                peaks.push(2.0 * db(pk(got), pk(src)));
                energies.push(db(en(got), en(src)));
            }
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
            assert!(peaks.len() >= 4);
            let (p, e) = (mean(&peaks), mean(&energies));
            assert!(
                p.abs() < 0.3 && e.abs() < 0.3,
                "hits every {every} at ×{k}: peak {p:+.2} dB, energy {e:+.2} dB"
            );
        }
    }
}

/// Round 29's second review (finding 38): hits a sixteenth apart at 120 BPM stretched by
/// Complex keep the whole frame's reach before each clean — pre-echo measured over half a
/// vocoder frame (what a frame reaches), not 25 ms — at ×0.8 and ×1.25.
#[test]
fn dense_hits_keep_half_a_frame_before_each_clean() {
    let n = 8 * 48_000;
    let half = pv::frame_len(SR) / 2;
    for every in [6_000usize, 12_000] {
        let (x, at) = pings(n, 24_000, every, 41);
        for k in [0.8f64, 1.25] {
            let len = (n as f64 / k) as usize - 24_000;
            let map = Map::new(vec![(0.0, 0.0), (len as f64, len as f64 * k)]).unwrap();
            let out = render(&[&x], SR, &req(Mode::Complex, &map, len, 0.0, &at));
            let mut worst = f32::MIN;
            for s in at.iter().copied().filter(|s| *s > 30_000.0) {
                let o = map.output(s).round() as usize;
                if o + half >= len {
                    break;
                }
                // Before it, back to the last hit's tail (40 ms) at most.
                let gap = ((every as f64 / k) as usize).saturating_sub((0.045 * SR) as usize);
                let pre = pre_echo_db(&out[0], o, half.min(gap), 48);
                worst = worst.max(pre);
            }
            assert!(
                worst < -30.0,
                "hats every {every} at ×{k}: {worst:.1} dB of pre-echo"
            );
        }
    }
}

/// A sung line: harmonics 1 to 10 (1/k) on `f0(t)` (Hz), its phase continuous.
fn voice_on(n: usize, f0: &dyn Fn(f64) -> f64) -> Vec<f32> {
    let mut ph = [0.0f64; 10];
    (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let f = f0(t);
            let env = (t / 0.05).min(1.0) * ((n as f64 / SR as f64 - t) / 0.05).clamp(0.0, 1.0);
            let mut s = 0.0;
            for (k, p) in ph.iter_mut().enumerate() {
                *p += f * (k + 1) as f64 / SR as f64;
                s += math::sin(2.0 * std::f64::consts::PI * *p) / (k + 1) as f64;
            }
            (0.15 * env * s) as f32
        })
        .collect()
}

/// The mean pitch error, cents, of `out` against `src` read through `map` (voiced frames
/// of both, from `from` to `to` output frames).
fn track_error(src: &[f32], out: &[f32], map: &Map, from: usize, to: usize) -> f64 {
    let a = crate::pitch::analyse(src, SR);
    let b = crate::pitch::analyse(out, SR);
    let (mut sum, mut n) = (0.0, 0usize);
    let mut t = from;
    while t < to {
        if let (Some(fs), Some(fo)) = (a.f0_at(map.source(t as f64)), b.f0_at(t as f64)) {
            sum += cents(fo, fs).abs() as f64;
            n += 1;
        }
        t += 240;
    }
    assert!(n > 50, "{n} voiced frames");
    sum / n as f64
}

/// Round 29's second review (finding 16): Complex follows a line whose pitch moves — a
/// 5.5 Hz vibrato of ±50 cents, a legato arpeggio of 150 ms notes — at ×0.8 and ×1.25:
/// its pitch track within a few cents of the source's where it read (the vocoder frame
/// chosen for the material: a moving voice is not smeared over 170 ms).
#[test]
fn complex_follows_a_moving_pitch() {
    let n = 4 * 48_000;
    let vibrato = voice_on(n, &|t| {
        220.0 * math::exp2(50.0 / 1200.0 * math::sin(2.0 * std::f64::consts::PI * 5.5 * t))
    });
    let notes = [220.0, 277.18, 329.63, 440.0, 329.63, 277.18];
    let legato = voice_on(n, &|t| notes[((t / 0.15) as usize) % notes.len()]);
    for (name, x) in [("vibrato", &vibrato), ("legato", &legato)] {
        for k in [0.8f64, 1.25] {
            let len = (n as f64 / k) as usize;
            let map = Map::new(vec![(0.0, 0.0), (len as f64, len as f64 * k)]).unwrap();
            let out = render(&[x], SR, &req(Mode::Complex, &map, len, 0.0, &[]));
            let e = track_error(x, &out[0], &map, 12_000, len - 12_000);
            assert!(e < 4.0, "{name} at ×{k}: {e:.2} cents from the source");
        }
    }
}

/// Round 29's second review (finding 19): the bridge's sketch of a long take is quick —
/// a minute of stereo at +2 st in a fraction of a second (best of three, one core): it
/// stands in for a first transposition while the render is made.
#[test]
fn a_sketch_of_a_minute_is_quick() {
    let n = 60 * 48_000;
    let (x, _) = hits(n, 3_000, 21_000, 43);
    let nz = band_noise(n, 47);
    let x: Vec<f32> = x.iter().zip(&nz).map(|(a, b)| a + 0.3 * b).collect();
    let len = (n as f64 / 1.05) as usize;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = std::time::Instant::now();
        let y = sketch(&[&x, &x], SR, &map, len, 2.0, 0, &[]);
        best = best.min(t.elapsed().as_secs_f64());
        assert_eq!(y[0].len(), len);
    }
    eprintln!("SKETCH {best:.3} s");
    assert!(best < 0.6, "a minute sketched in {best:.2} s");
}

/// Round 29's second review (finding 14): partials both channels hold (the same notes,
/// each its own place in the image), over noise of each channel's own only 10 dB down (a
/// room, a reverb), keep their phase difference through a stretch — measured in windows
/// along the output, each partial within 0.1 rad of what the source holds where it read
/// (never a phase random-walking away from it).
#[test]
fn a_stereo_image_over_independent_noise_is_kept() {
    let n = 4 * 48_000;
    let parts = [
        (220.0f64, 0.7f64),
        (330.0, -0.4),
        (440.0, 1.1),
        (660.0, 0.3),
    ];
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for &(hz, d) in &parts {
        let w = 2.0 * std::f64::consts::PI * hz / SR as f64;
        for i in 0..n {
            l[i] += (0.1 * math::sin(w * i as f64)) as f32;
            r[i] += (0.1 * math::sin(w * i as f64 - d)) as f32;
        }
    }
    let (nl, nr) = (band_noise(n, 51), band_noise(n, 53));
    let level = |x: &[f32]| rms_db(x);
    let k = math::db_to_gain(level(&l) - 10.0 - level(&nl));
    for i in 0..n {
        l[i] += k * nl[i];
        r[i] += k * nr[i];
    }
    for stretch in [0.8f64, 1.25] {
        let len = (n as f64 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
        let out = render(&[&l, &r], SR, &req(Mode::Complex, &map, len, 0.0, &[]));
        let win = 9_600;
        for &(hz, d) in &parts {
            let mut at = len / 8;
            while at + win < len - len / 8 {
                let got =
                    channel_phase(&out[0][at..at + win], &out[1][at..at + win], SR, hz as f32);
                let s0 = (at as f64 / stretch) as usize;
                let own = channel_phase(&l[s0..s0 + win], &r[s0..s0 + win], SR, hz as f32) as f64;
                let off = got as f64 - own;
                let off =
                    off - 2.0 * std::f64::consts::PI * (off / (2.0 * std::f64::consts::PI)).round();
                assert!(
                    off.abs() < 0.1,
                    "×{stretch} {hz} Hz at {at}: {got:.3} rad, the source's {own:.3} ({d})"
                );
                at += win;
            }
        }
    }
}

/// Round 29's third review (finding 28): a Rhythm window of a long file that ends between
/// two hits, slowed ×0.8 and transposed +3 st, plays nothing of the hit after its end —
/// its last slice ends by its tail — and two halves of it split between hits sum to it.
#[test]
fn rhythms_last_slice_never_plays_the_next_windows_hit() {
    let n = 20 * 48_000;
    let (x, at) = hits(n, 3_000, 21_000, 17);
    // Source 384 000..460 800 over 96 000 output frames: the window ends 4 200 frames
    // before the hit at 465 000.
    let len = 96_000;
    let map = Map::new(vec![(0.0, 384_000.0), (len as f64, 460_800.0)]).unwrap();
    let q = req(Mode::Rhythm, &map, len, 3.0, &at);
    let y = render(&[&x], SR, &q);
    // The last slice starts at the hit at 444 000 (output 75 000): its own hit, then its
    // fade — never a second onset before the window ends.
    let tail = &y[0][90_000..len];
    let peak = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak < 0.05, "the next hit inside the window: {peak}");
    // Split at output 48 000 (source 422 400, between hits): the halves sum to the whole
    // around the split.
    let mid = 48_000usize;
    let left = Map::new(vec![(0.0, 384_000.0), (mid as f64, 422_400.0)]).unwrap();
    let right = Map::new(vec![(0.0, 422_400.0), ((len - mid) as f64, 460_800.0)]).unwrap();
    let a = render(&[&x], SR, &req(Mode::Rhythm, &left, mid, 3.0, &at));
    let b = render(&[&x], SR, &req(Mode::Rhythm, &right, len - mid, 3.0, &at));
    let joined: Vec<f32> = a[0].iter().chain(&b[0]).copied().collect();
    let worst = joined[mid - 2_000..mid + 2_000]
        .iter()
        .zip(&y[0][mid - 2_000..mid + 2_000])
        .map(|(p, q)| (p - q).abs())
        .fold(0.0f32, f32::max);
    assert!(
        worst < 0.05,
        "the halves differ from the whole by {worst} at the split"
    );
}

/// Round 29's third review (finding 8): a sped-up window (×1.6, ×2) whose first hit comes
/// 20 to 90 ms after its start renders as the whole file would — the vocoder's frames
/// before the window read at the window's own speed, never far before it — and what was
/// a third of a second before the window never sounds in its first frames.
#[test]
fn a_sped_up_window_starting_before_a_hit_reads_no_further_back_than_its_speed() {
    let n = 20 * 48_000;
    let (h, at) = hits(n, 3_000, 21_000, 17);
    // A 1 kHz tone up to a third of a second before each window, 200 Hz after.
    for (k, ms) in [(1.6, 20.0), (2.0, 50.0), (2.0, 90.0)] {
        let first = 402_000.0;
        let a = first - k * ms / 1_000.0 * SR as f64;
        let change = (a - 16_000.0) as usize;
        let tone: Vec<f32> = (0..n)
            .map(|i| {
                let hz = if i < change { 1_000.0 } else { 200.0 };
                0.3 * math::sin(2.0 * std::f64::consts::PI * hz * i as f64 / SR as f64) as f32
            })
            .collect();
        let x: Vec<f32> = h.iter().zip(&tone).map(|(a, b)| a + b).collect();
        let len = 96_000;
        let map = Map::new(vec![(0.0, a), (len as f64, a + k * len as f64)]).unwrap();
        // The window's first hit is the first detected (none in the second before it).
        let at: Vec<f64> = at.iter().copied().filter(|s| *s >= first).collect();
        let q = req(Mode::Complex, &map, len, 0.0, &at);
        let whole = dispatch(&[&x], SR, &q, 0.0);
        let windowed = render(&[&x], SR, &q);
        let worst = windowed[0]
            .iter()
            .zip(&whole[0])
            .map(|(p, q)| (p - q).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-4, "×{k}, {ms} ms: {worst} from the whole file");
        // The tones alone (the hits' places still held): nothing of the 1 kHz.
        let whole = dispatch(&[&tone], SR, &q, 0.0);
        let head = &whole[0][..2_048];
        let far = amplitude(head, SR, 1_000.0);
        let near = amplitude(head, SR, 200.0);
        assert!(
            far < 0.01 * near,
            "×{k}, {ms} ms: the 1 kHz from before the window at {far} (200 Hz {near})"
        );
    }
}

/// The energy (dB) of `x` read from `s` at `r` source frames an output frame, over `w`
/// output frames: a resampled copy of a hit, through the readers the stretcher uses (read
/// faster, band-limited: nothing above the output's band).
fn copy_db(x: &[f32], s: f64, r: f64, w: usize) -> f32 {
    let fast = (r > 1.0).then(|| read::Fast::new(x, s - 64.0, s + w as f64 * r + 64.0, r));
    let e: f64 = (0..w)
        .map(|t| {
            let p = s + t as f64 * r;
            let v = match &fast {
                Some(f) => f.read(p, r),
                None => crate::psola::read_at(x, p),
            };
            (v * v) as f64
        })
        .sum();
    (10.0 * math::log10(e.max(1e-30))) as f32
}

/// A drum hit's noise alone (`hits`' without its tonal thump).
fn noise_hits(n: usize, first: usize, every: usize, seed: u64) -> (Vec<f32>, Vec<f64>) {
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
            x[s + k] += 0.6 * env * g.next();
        }
        s += every;
    }
    (x, at)
}

/// Round 29's third review (finding 6): transposed in Complex — down an octave, −3, +7, up
/// an octave — on an identity map and slowed and sped up, each hit's noise and each hat
/// keeps the energy over its first 30 ms of a resampled copy of it (the readers the
/// stretcher uses), within half a decibel (median); a drum hit with a tonal body within
/// one and a half (its body is stretched, keeping its decay, not resampled).
#[test]
fn transposed_hits_keep_the_level_of_a_resampled_copy() {
    let n = 6 * 48_000;
    let (dr, dat) = hits(n, 6_000, 24_000, 21);
    let (nh, nat) = noise_hits(n, 6_000, 24_000, 23);
    let (hh, hat) = hats(n, 18_000, 24_000, 22);
    let w = (0.030 * SR) as usize;
    for (name, x, at, bound) in [
        ("noise hits", &nh, &nat, 0.5f32),
        ("hats", &hh, &hat, 0.5),
        ("drum hits", &dr, &dat, 1.5),
    ] {
        for k in [1.0, 0.8, 1.25] {
            for semis in [-12.0, -3.0, 7.0, 12.0] {
                let r = math::powf(2.0f64, semis / 12.0);
                let len = (n as f64 / k) as usize - 48_000;
                let map = Map::new(vec![(0.0, 0.0), (len as f64, len as f64 * k)]).unwrap();
                let y = render(&[x], SR, &req(Mode::Complex, &map, len, semis, at));
                let mut d: Vec<f32> = at
                    .iter()
                    .filter(|s| (**s as usize) + 2 * w < (len as f64 * k) as usize)
                    .map(|s| {
                        let o = map.output(*s).round() as usize;
                        let got = &y[0][o..o + w];
                        let e: f64 = got.iter().map(|v| (*v as f64) * (*v as f64)).sum();
                        (10.0 * math::log10(e.max(1e-30))) as f32 - copy_db(x, *s, r, w)
                    })
                    .collect();
                d.sort_by(f32::total_cmp);
                let med = d[d.len() / 2];
                assert!(
                    med.abs() <= bound,
                    "{name} ×{k} {semis:+} st: {med:+.2} dB against a resampled copy ({d:?})"
                );
            }
        }
    }
}

/// The bench's pad: two detuned saws a note (one per channel), `n` frames, a fade in and
/// out.
fn detuned_pad(n: usize, notes: &[i32]) -> (Vec<f32>, Vec<f32>) {
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let secs = n as f64 / SR as f64;
    for (v, &note) in notes.iter().enumerate() {
        for (d, detune) in [-0.08f64, 0.08].iter().enumerate() {
            let f = crate::pitch::midi_to_hz(note as f32) as f64 * (1.0 + detune / 100.0 * 12.0);
            let mut ph = (v * 7 + d * 3) as f64 * 0.37;
            for i in 0..n {
                let t = i as f64 / SR as f64;
                ph += f / SR as f64;
                let saw = 2.0 * (ph - ph.floor()) - 1.0;
                let env = (t / 0.3).min(1.0) * ((secs - t) / 0.05).clamp(0.0, 1.0);
                let s = (0.05 * env * saw) as f32;
                if d == 0 {
                    l[i] += s
                } else {
                    r[i] += s
                }
            }
        }
    }
    (l, r)
}

/// Round 29's third review (finding 7): a detuned pad under sixteenth hats (120 BPM), with
/// the detector's onsets, slowed and sped up: the hats never take the pad's long frame
/// away — each voice's wander over the mix's own within a decibel over 100 ms windows,
/// within three over 40 ms (the short frame added up to 3 and 11).
#[test]
fn hats_over_a_pad_leave_its_voices_steady() {
    let notes = [48, 52, 55, 60];
    let n = 2 * 48_000;
    let (pl, pr) = detuned_pad(n, &notes);
    let (hh, _) = hats(n, 3_000, 6_000, 51);
    let l: Vec<f32> = pl.iter().zip(&hh).map(|(a, b)| a + b).collect();
    let r: Vec<f32> = pr.iter().zip(&hh).map(|(a, b)| a + b).collect();
    let mono: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let found = crate::onset::detect(&mono, SR).frames;
    assert!(found.len() > 8, "the hats detected: {}", found.len());
    for stretch in [0.8, 1.25] {
        let len = (n as f64 * stretch) as usize;
        let map = Map::new(vec![(0.0, 0.0), (len as f64, n as f64)]).unwrap();
        let y = render(&[&l, &r], SR, &req(Mode::Complex, &map, len, 0.0, &found));
        for (ch, detune, src) in [(0usize, -0.08f64, &l), (1, 0.08, &r)] {
            for note in notes {
                let hz = (crate::pitch::midi_to_hz(note as f32) as f64
                    * (1.0 + detune / 100.0 * 12.0)) as f32;
                for win in [0.1f32, 0.04] {
                    let (a, b) = (0.4f64, 1.6f64);
                    let own = level_wander_db_in(
                        src,
                        SR,
                        hz,
                        (a * SR as f64) as usize,
                        (b * SR as f64) as usize,
                        win,
                    );
                    let w = level_wander_db_in(
                        &y[ch],
                        SR,
                        hz,
                        (a * SR as f64 * stretch) as usize,
                        (b * SR as f64 * stretch) as usize,
                        win,
                    );
                    // Over 40 ms (which do not resolve voices 2 Hz apart: the pad alone
                    // adds up to 1.5 dB there) within 3 dB; the short frame added up to 11.
                    let bound = if win > 0.05 { 1.0 } else { 3.0 };
                    assert!(
                        w - own <= bound,
                        "×{stretch} ch{ch} {hz:.1} Hz over {win} s: {own:.2} dB, {w:.2} dB"
                    );
                }
            }
        }
    }
}
