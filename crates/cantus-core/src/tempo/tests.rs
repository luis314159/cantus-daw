//! The tempo map's own tests (ADR 0060): the constant map is today's arithmetic to the
//! bit; steps and ramps integrate exactly; seconds and ticks are inverses over random maps;
//! the metre counts bars through every change.

use super::*;

/// A small deterministic generator (the maps are a hash of a seed).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / (1u64 << 31) as f64
    }
}

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

fn mc(tick: i64, n: u8, d: u8) -> MetreChange {
    MetreChange {
        tick: Ticks(tick),
        signature: TimeSignature::new(n, d),
    }
}

/// A random map: 1–8 points, tempos 20–999, some ramps.
fn random_map(rng: &mut Lcg) -> TempoMap {
    let mut pts = vec![pt(0, 20.0 + rng.unit() * 300.0, rng.below(2) == 0)];
    let mut tick = 0;
    for _ in 0..rng.below(8) {
        tick += 1 + rng.below(40_000) as i64;
        pts.push(pt(tick, 20.0 + rng.unit() * 979.0, rng.below(2) == 0));
    }
    TempoMap::new(pts[0].bpm, TimeSignature::default(), &pts, &[])
}

/// Seconds over `[a, b]` by Simpson's rule on `60 / (PPQ · bpm(t))` — the definition the
/// closed form must match.
fn simpson(map: &TempoMap, a: f64, b: f64) -> f64 {
    let n = 20_000;
    let h = (b - a) / n as f64;
    let f = |t: f64| 60.0 / (PPQ as f64 * map.bpm_at(t));
    let mut s = f(a) + f(b);
    for i in 1..n {
        let t = a + i as f64 * h;
        s += if i % 2 == 1 { 4.0 } else { 2.0 } * f(t);
    }
    s * h / 3.0
}

#[test]
fn a_constant_map_gives_the_constant_tempo_arithmetic_to_the_bit() {
    for bpm in [20.0, 97.3, 120.0, 140.0, 999.0] {
        let map = TempoMap::constant(bpm, TimeSignature::default());
        for t in [-7_680, -1, 0, 1, 959, 960, 123_457, 9_600_000] {
            let tick = Ticks(t);
            assert_eq!(
                map.seconds_at(t as f64).to_bits(),
                tick.to_seconds(bpm).to_bits()
            );
            assert_eq!(
                map.sample_at(t as f64, 48_000.0).to_bits(),
                tick.to_samples_f64(bpm, 48_000.0).to_bits()
            );
            assert_eq!(
                map.seconds_between(Ticks(17), tick).to_bits(),
                (tick - Ticks(17)).to_seconds(bpm).to_bits()
            );
            let s = t as f64 / 1000.0;
            assert_eq!(
                map.tick_at_seconds(s).to_bits(),
                (s * bpm / 60.0 * PPQ as f64).to_bits()
            );
        }
    }
}

#[test]
fn seconds_and_ticks_are_inverses() {
    let map = TempoMap::constant(132.0, TimeSignature::default());
    for t in [0.0, 1.0, 480.0, 96_000.5] {
        let back = map.tick_at_seconds(map.seconds_at(t));
        assert!((back - t).abs() < 1e-6, "{t} → {back}");
    }
}

#[test]
fn a_step_changes_the_tempo_at_its_tick() {
    // 120 for two beats, then 60.
    let map = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[pt(0, 120.0, false), pt(1920, 60.0, false)],
        &[],
    );
    assert!(!map.is_constant());
    assert_eq!(map.seconds_at(1920.0), 1.0);
    assert_eq!(map.seconds_at(2880.0), 2.0);
    assert_eq!(map.bpm_at(1919.0), 120.0);
    assert_eq!(map.bpm_at(1920.0), 60.0);
    assert_eq!(map.tick_at_seconds(1.5), 2400.0);
    assert_eq!(
        map.seconds_at(-960.0),
        -0.5,
        "before 0 the first tempo holds"
    );
    assert_eq!(map.next_change_after(0.0), Some(1920.0));
    assert_eq!(map.next_change_after(1920.0), None);
    assert_eq!(map.seconds_between(Ticks(1920), Ticks(2880)), 1.0);
}

#[test]
fn a_ramp_integrates_in_closed_form() {
    // 90 → 140 over 32 beats, then 140.
    let end = 32 * PPQ;
    let map = TempoMap::new(
        90.0,
        TimeSignature::default(),
        &[pt(0, 90.0, true), pt(end, 140.0, false)],
        &[],
    );
    assert!(map.in_ramp(100.0) && !map.in_ramp(end as f64));
    assert!((map.bpm_at(end as f64 / 2.0) - 115.0).abs() < 1e-12);
    // The closed form: 60/(PPQ k) ln(b1/b0).
    let k = 50.0 / end as f64;
    let want = 60.0 / (PPQ as f64 * k) * math::ln(140.0f64 / 90.0);
    assert!((map.seconds_at(end as f64) - want).abs() < 1e-12);
    for (a, b) in [(0.0, 7_000.0), (3_333.0, 29_000.0), (0.0, end as f64)] {
        let num = simpson(&map, a, b);
        let got = map.seconds_between_f64(a, b);
        assert!((got - num).abs() < 1e-9, "{a}..{b}: {got} vs {num}");
    }
    // After the ramp, 140 holds from where it ended.
    let after = map.seconds_at(end as f64 + 960.0) - map.seconds_at(end as f64);
    assert!((after - 60.0 / 140.0).abs() < 1e-12);
    // Before 0, the first tempo holds (not the ramp extended backwards).
    assert_eq!(map.seconds_at(-960.0), -60.0 / 90.0);
    assert_eq!(map.bpm_at(-1.0), 90.0);
    // A slowing ramp too.
    let slow = TempoMap::new(
        140.0,
        TimeSignature::default(),
        &[pt(0, 140.0, true), pt(end, 90.0, false)],
        &[],
    );
    let num = simpson(&slow, 0.0, end as f64);
    assert!((slow.seconds_at(end as f64) - num).abs() < 1e-9);
}

#[test]
fn seconds_and_ticks_are_inverses_over_random_maps() {
    let mut rng = Lcg(0x7e3);
    let mut worst_s: f64 = 0.0;
    for _ in 0..300 {
        let map = random_map(&mut rng);
        let mut last = f64::NEG_INFINITY;
        for i in -20..400 {
            let t = i as f64 * 777.7;
            let s = map.seconds_at(t);
            assert!(s > last, "monotonic at {t}");
            last = s;
            let back = map.tick_at_seconds(s);
            worst_s = worst_s.max((map.seconds_at(back) - s).abs());
            assert!((back - t).abs() < 1e-6 * t.abs().max(1.0), "{t} → {back}");
        }
    }
    assert!(worst_s < 1e-9, "worst {worst_s} s");
}

#[test]
fn ramp_steps_fall_on_ticks_from_the_ramps_start() {
    let map = TempoMap::new(
        100.0,
        TimeSignature::default(),
        &[
            pt(0, 100.0, false),
            pt(1000, 100.0, true),
            pt(2000, 150.0, false),
        ],
        &[],
    );
    assert_eq!(map.next_step_after(10.0), Some(1000.0));
    assert_eq!(map.next_step_after(1000.0), Some(1000.0 + RAMP_STEP as f64));
    assert_eq!(map.step_start(1000.0 + 61.0), 1000.0 + RAMP_STEP as f64);
    assert_eq!(map.step_start(500.0), 500.0);
    // The last step of the ramp ends at its end.
    assert_eq!(map.next_step_after(1990.0), Some(2000.0));
    assert_eq!(map.next_step_after(2000.0), None);
}

#[test]
fn new_sorts_dedupes_clamps_and_starts_at_zero() {
    let map = TempoMap::new(
        130.0,
        TimeSignature::default(),
        &[
            pt(960, 5_000.0, false),
            pt(-5, 80.0, false),
            pt(960, 70.0, true),
        ],
        &[mc(-960, 3, 4), mc(0, 0, 4), mc(3840, 3, 4), mc(1000, 7, 3)],
    );
    assert_eq!(map.points(), &[pt(0, 130.0, false), pt(960, 70.0, false)]);
    assert_eq!(map.metres(), &[mc(0, 4, 4), mc(3840, 3, 4)]);
}

#[test]
fn bars_of_a_constant_metre() {
    let map = TempoMap::constant(120.0, TimeSignature::new(3, 4));
    let lines: Vec<Ticks> = map.bar_lines(Ticks(100), Ticks::beats(10)).collect();
    assert_eq!(
        lines,
        vec![Ticks::beats(3), Ticks::beats(6), Ticks::beats(9)]
    );
    assert_eq!(map.bar_start(Ticks::beats(4)), Ticks::beats(3));
    assert_eq!(map.bbt(Ticks::beats(4)), (2, 2, 0));
    assert_eq!(
        map.bar_lines(Ticks::ZERO, Ticks::beats(3))
            .collect::<Vec<_>>(),
        vec![Ticks::ZERO]
    );
    // One metre: every answer is the signature's own, before 0 too.
    let sig = TimeSignature::new(3, 4);
    for t in [-5000, -1, 0, 1, 2879, 2880, 99_999] {
        assert_eq!(map.bbt(Ticks(t)), sig.bbt(Ticks(t)));
        assert_eq!(
            map.bar_start(Ticks(t)),
            Ticks(t).snap_floor(sig.ticks_per_bar())
        );
        assert_eq!(map.snap(Ticks(t), Ticks(240)), Ticks(t).snap(Ticks(240)));
    }
}

/// 4/4 for two bars, a bar of 7/8, then 3/4.
fn song_metre() -> TempoMap {
    TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[],
        &[mc(0, 4, 4), mc(7680, 7, 8), mc(7680 + 3360, 3, 4)],
    )
}

#[test]
fn bars_count_through_metre_changes() {
    let map = song_metre();
    let lines: Vec<i64> = map
        .bar_lines(Ticks::ZERO, Ticks(16_000))
        .map(|t| t.0)
        .collect();
    assert_eq!(lines, vec![0, 3840, 7680, 11_040, 13_920]);
    assert_eq!(map.bbt(Ticks(7680)), (3, 1, 0));
    assert_eq!(map.bbt(Ticks(7680 + 480 * 6 + 5)), (3, 7, 5));
    assert_eq!(map.bbt(Ticks(11_040)), (4, 1, 0));
    assert_eq!(map.bbt(Ticks(13_920 + 960)), (5, 2, 0));
    assert_eq!(map.signature_at(Ticks(8000)), TimeSignature::new(7, 8));
    assert_eq!(map.bar_tick(4), Ticks(11_040));
    assert_eq!(map.bar_tick(0), Ticks(-3840));
    assert_eq!(map.tick_of_bbt(3, 2, 10), Ticks(7680 + 480 + 10));
    assert_eq!(map.bar_start(Ticks(10_000)), Ticks(7680));
    assert_eq!(map.bar_end(Ticks(10_000)), Ticks(11_040));
    // Beats: eighths in the 7/8 bar, the first of each bar accented.
    let beats: Vec<(i64, bool)> = map
        .beat_lines(Ticks(6720), Ticks(12_000))
        .map(|(t, a)| (t.0, a))
        .collect();
    assert_eq!(beats[0], (6720, false));
    assert_eq!(beats[1], (7680, true));
    assert_eq!(beats[2], (8160, false));
    assert_eq!(beats.iter().filter(|(_, a)| *a).count(), 2);
    assert_eq!(map.next_beat(7680.5), (8160.0, false));
    assert_eq!(map.next_beat(10_600.0), (11_040.0, true));
    // Snapping counts from the bar: a quarter grid in the 3/4 bar after the 7/8 lands on
    // its beats (11 040 + 960 k), not on multiples of 960 from 0.
    assert_eq!(map.snap(Ticks(12_100), Ticks(960)), Ticks(12_000));
    assert_eq!(map.snap(Ticks(9_000), Ticks(100_000)), Ticks(7680));
    assert_eq!(map.snap_bar(Ticks(10_500)), Ticks(11_040));
    assert_eq!(map.add_bars(Ticks(3840 + 100), 1), Ticks(7680 + 100));
    assert_eq!(map.add_bars(Ticks(3840 + 3500), 1), Ticks(7680 + 3360));
}

#[test]
fn an_off_line_metre_change_cuts_the_bar_before_it_short() {
    // 4/4, then 3/4 from tick 5000 (bar 2 is 1 160 ticks long).
    let map = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[],
        &[mc(0, 4, 4), mc(5000, 3, 4)],
    );
    let lines: Vec<i64> = map
        .bar_lines(Ticks::ZERO, Ticks(8000))
        .map(|t| t.0)
        .collect();
    assert_eq!(lines, vec![0, 3840, 5000, 7880]);
    assert_eq!(map.bbt(Ticks(5000)), (3, 1, 0));
    assert_eq!(map.bar_end(Ticks(4000)), Ticks(5000));
}

#[test]
fn the_span_key_is_the_tempo_over_the_span() {
    let c = TempoMap::constant(120.0, TimeSignature::default());
    // The contract's key for a constant map.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in 120.0f64.to_bits().to_le_bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    assert_eq!(c.span_key(Ticks(0), Ticks(9600)), h);
    assert_eq!(c.span_key(Ticks(5000), Ticks(99_600)), h, "moved, same key");
    let later = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[pt(0, 120.0, false), pt(20_000, 90.0, false)],
        &[],
    );
    assert_eq!(
        later.span_key(Ticks(0), Ticks(9600)),
        h,
        "a point after the span"
    );
    assert_ne!(
        later.span_key(Ticks(0), Ticks(20_001)),
        h,
        "a point inside it"
    );
    let ramp = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[pt(0, 120.0, true), pt(20_000, 90.0, false)],
        &[],
    );
    assert_ne!(ramp.span_key(Ticks(0), Ticks(9600)), h, "a ramp under it");
    assert_ne!(
        ramp.span_key(Ticks(0), Ticks(9600)),
        ramp.span_key(Ticks(100), Ticks(9700)),
        "a different stretch of the ramp"
    );
}

#[test]
fn a_project_without_a_tempo_track_is_constant() {
    let p = Project::empty("t");
    let map = p.tempo_map();
    assert!(map.is_constant());
    assert!(map.is_one_metre());
    assert_eq!(map.bpm_at(12_345.0), p.tempo_bpm);
    assert_eq!(map.signature_at(Ticks::beats(99)), p.time_signature);
    assert_eq!(map.next_change_after(0.0), None);
}

/// Review 1 (findings 5, 12): before tick 0 a held first tempo is no change at 0 — a
/// count-in on a project without a tempo track gets no extra pass cut there.
#[test]
fn a_held_first_tempo_has_no_change_at_zero() {
    let c = TempoMap::constant(120.0, TimeSignature::default());
    assert_eq!(c.next_change_after(-3840.0), None);
    assert_eq!(c.next_step_after(-3840.0), None);
    let step = TempoMap::new(120.0, TimeSignature::default(), &[pt(0, 120.0, false), pt(1920, 90.0, false)], &[]);
    assert_eq!(step.next_change_after(-10.0), Some(1920.0));
    // A ramp from tick 0 does start there.
    let ramp = TempoMap::new(120.0, TimeSignature::default(), &[pt(0, 120.0, true), pt(1920, 90.0, false)], &[]);
    assert_eq!(ramp.next_change_after(-10.0), Some(0.0));
    assert_eq!(ramp.next_step_after(-10.0), Some(0.0));
}

/// Review 1 (finding 4): a span starting before tick 0 keys on the first segment's ramp.
#[test]
fn the_span_key_before_zero_sees_the_first_ramp() {
    let a = TempoMap::new(120.0, TimeSignature::default(), &[pt(0, 120.0, true), pt(1920, 140.0, false)], &[]);
    let b = TempoMap::new(120.0, TimeSignature::default(), &[pt(0, 120.0, false), pt(1920, 140.0, false)], &[]);
    let c = TempoMap::constant(120.0, TimeSignature::default());
    let (from, to) = (Ticks(-960), Ticks(1920));
    assert_ne!(a.span_key(from, to), b.span_key(from, to));
    assert_ne!(a.span_key(from, to), c.span_key(from, to));
}
