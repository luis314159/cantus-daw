use super::*;
use crate::modfx_clock::{Transport, DIVISIONS, DIV_EIGHTH, DIV_QUARTER};

const SR: f32 = 48_000.0;

fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 128.0,
        samples,
    }
}

/// Run an LFO through `total` samples cut by `split`, the transport given once per call
/// as an engine gives it; collect its values.
fn run(set: &LfoSettings, start: i64, total: usize, mut split: impl FnMut() -> usize) -> Vec<f32> {
    let mut clock = Clock::new();
    let mut lfo = Lfo::new(SR);
    clock.set(&playing(start));
    lfo.set(set, &clock);
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let n = split().min(total - out.len()).max(1);
        clock.set(&playing(start + out.len() as i64));
        lfo.follow(&clock);
        for i in 0..n {
            out.push(lfo.value(&clock, i));
        }
        clock.advance(n);
    }
    out
}

fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn the_wave_is_the_same_at_every_split() {
    for shape in Shape::ALL {
        for rate in [Rate::Hz(3.7), Rate::Beats(DIVISIONS[DIV_EIGHTH])] {
            let set = LfoSettings {
                rate,
                shape,
                edge: 0.3,
                swing: 0.4,
                seed: 9,
                ..LfoSettings::default()
            };
            let a = run(&set, 0, 20_000, || 512);
            let mut k = 0usize;
            let b = run(&set, 0, 20_000, || {
                k += 1;
                [1, 7, 64, 333, 4096][k % 5]
            });
            assert_eq!(bits(&a), bits(&b), "{shape:?} {rate:?}");
        }
    }
}

#[test]
fn a_synced_wave_from_bar_five_is_the_full_wave_from_bar_five() {
    // Bar 5 of 4/4 at 128 BPM: 16 beats.
    let bar5 = (16.0 * 60.0 / 128.0 * f64::from(SR)).round() as i64;
    for shape in Shape::ALL {
        let set = LfoSettings {
            rate: Rate::Beats(DIVISIONS[DIV_QUARTER]),
            shape,
            edge: 0.2,
            swing: 0.25,
            seed: 3,
            ..LfoSettings::default()
        };
        let full = run(&set, 0, bar5 as usize + 30_000, || 480);
        let late = run(&set, bar5, 30_000, || 333);
        assert_eq!(bits(&full[bar5 as usize..]), bits(&late), "{shape:?}");
    }
}

#[test]
fn a_synced_wave_is_one_cycle_per_division() {
    let set = LfoSettings {
        rate: Rate::Beats(1.0),
        ..LfoSettings::default()
    };
    let mut clock = Clock::new();
    clock.set(&playing(0));
    let mut lfo = Lfo::new(SR);
    lfo.set(&set, &clock);
    // One beat at 128 BPM in samples: 22 500.
    let beat = (60.0 / 128.0 * f64::from(SR)) as usize;
    let (p, c) = lfo.phase(&clock, beat);
    assert!(p.abs() < 1e-9 || (1.0 - p).abs() < 1e-9, "phase {p}");
    assert!(c == 1 || c == 0);
}

#[test]
fn a_free_rate_change_does_not_jump() {
    let mut clock = Clock::new();
    let mut lfo = Lfo::new(SR);
    lfo.set(
        &LfoSettings {
            rate: Rate::Hz(2.0),
            ..LfoSettings::default()
        },
        &clock,
    );
    clock.advance(10_000);
    let before = lfo.value(&clock, 0);
    lfo.set(
        &LfoSettings {
            rate: Rate::Hz(7.0),
            ..LfoSettings::default()
        },
        &clock,
    );
    let after = lfo.value(&clock, 0);
    assert!((before - after).abs() < 1e-6, "{before} → {after}");
}

#[test]
fn playback_starts_a_free_wave_from_its_offset() {
    let set = LfoSettings {
        rate: Rate::Hz(1.3),
        offset: 0.25,
        ..LfoSettings::default()
    };
    let mut clock = Clock::new();
    let mut lfo = Lfo::new(SR);
    lfo.set(&set, &clock);
    clock.advance(12_345);
    clock.set(&playing(0));
    lfo.follow(&clock);
    // A quarter cycle in: the sine's top.
    assert!((lfo.value(&clock, 0) - 1.0).abs() < 1e-6);
}

#[test]
fn every_wave_is_bipolar_and_its_soft_edges_do_not_jump() {
    for shape in Shape::ALL {
        let mut last = wave(shape, 0.0, 0, 0.5, 1);
        let n = 20_000;
        let mut worst = 0.0f32;
        for k in 1..=4 * n {
            let q = k as f64 / n as f64;
            let v = wave(shape, q - q.floor(), q.floor() as i64, 0.5, 1);
            assert!((-1.0..=1.0).contains(&v), "{shape:?} {v}");
            worst = worst.max((v - last).abs());
            last = v;
        }
        // With half a half-cycle of edge, no step is larger than a smooth ramp makes.
        assert!(worst < 0.01, "{shape:?} jumps by {worst}");
    }
}

#[test]
fn every_wave_starts_in_its_middle() {
    for shape in [
        Shape::Sine,
        Shape::Triangle,
        Shape::Square,
        Shape::SawUp,
        Shape::SawDown,
    ] {
        assert!(wave(shape, 0.0, 0, 0.3, 0).abs() < 1e-6, "{shape:?}");
    }
}

#[test]
fn swing_delays_every_second_cycle() {
    let set = LfoSettings {
        rate: Rate::Beats(0.5),
        swing: 1.0,
        ..LfoSettings::default()
    };
    let lfo = {
        let mut l = Lfo::new(SR);
        l.set(&set, &Clock::new());
        l
    };
    // At full swing the pair splits ¾ / ¼: the second cycle starts at 1.5 cycles.
    assert_eq!(lfo.phase_at(1.4).1, 0);
    assert_eq!(lfo.phase_at(1.5).1, 1);
    assert!((lfo.phase_at(1.75).0 - 0.5).abs() < 1e-12);
}
