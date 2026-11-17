//! Cedo's linear-phase floor (the owner's choice, 2026-11-17): Auto an octave under the
//! crossover as committed, or a fixed 40, 60, 80 or 120 Hz — each floor's latency stated,
//! Auto's equal to the fixed floor's at the same frequency (to the bit, its output too), a
//! fixed floor's latency constant whatever the crossover, a crossover under the floor heard
//! at the floor, the A-2 accuracy at every floor, and a floor changed while it runs heard as
//! the input delayed by the new latency, to the sample.

use super::band::{self, auto_floor, Band, Crossover, Floor, FLOORS};
use super::testkit::{db, rms, run, shape_fn, sine, Event, Run};
use super::{latency_for, Cedo, CedoSettings, Trigger};
use crate::fluctus::CurvePoint;

const SR: f32 = 48_000.0;

const FULL: [CurvePoint; 1] = [CurvePoint {
    x: 0.0,
    y: 1.0,
    tension: 0.0,
    shape: 0,
}];

fn unit(s: &CedoSettings) -> Cedo {
    let mut c = Cedo::new(SR, shape_fn);
    c.set_shape(&FULL);
    c.set(s);
    c.reset();
    c
}

fn fixed(hz: f32) -> Floor {
    Floor::from_index(1 + FLOORS.iter().position(|f| *f == hz).unwrap())
}

/// A linear-phase lows (or highs) duck of 24 dB at `cross` on the tempo's full shape.
fn ducking(band: Band, cross: f32, floor: Floor, auto_hz: f32) -> CedoSettings {
    CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: 24.0,
        band,
        crossover: Crossover::LinearPhase,
        cross_hz: cross,
        floor,
        auto_floor_hz: auto_hz,
        edge_ms: 0.0,
        ..CedoSettings::default()
    }
}

fn played(c: &mut Cedo, x: &[f32]) -> Vec<f32> {
    let ev = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    run(
        c,
        &Run {
            left: x,
            right: x,
            key: None,
            events: &ev,
            block: 256,
        },
    )
    .0
}

/// A sine's gain through the duck, dB, in its second half (the latency aligned).
fn sine_gain(s: &CedoSettings, hz: f32) -> f64 {
    let n = 144_000;
    let x = sine(hz, 0.25, n, SR);
    let mut c = unit(s);
    let lat = c.latency();
    let o = played(&mut c, &x);
    db(rms(&o[n / 2..]) / rms(&x[n / 2 - lat..n - lat]))
}

#[test]
fn each_floor_s_latency_is_stated_and_auto_s_is_the_fixed_floor_s() {
    // At 48 kHz: 40 Hz 55.3 ms, 60 Hz 36.8 ms, 80 Hz 27.6 ms, 120 Hz 18.3 ms.
    let want = [2_655usize, 1_765, 1_325, 880];
    let mut row = String::new();
    for (hz, want) in FLOORS.iter().zip(want) {
        let lat = band::linear_latency(*hz, SR);
        row += &format!("{hz} Hz {lat} ({:.1} ms); ", lat as f32 * 1_000.0 / SR);
        assert_eq!(lat, want, "{hz} Hz");
        // Auto, its floor committed for a crossover an octave above, is the fixed floor.
        let cross = hz * 2.0;
        assert_eq!(auto_floor(cross), *hz);
        let auto = ducking(Band::Lows, cross, Floor::Auto, auto_floor(cross));
        let fix = ducking(Band::Lows, cross, fixed(*hz), 999.0);
        assert_eq!(latency_for(&auto, SR), want);
        assert_eq!(latency_for(&fix, SR), want);
        assert_eq!(unit(&auto).latency(), want);
        let x = sine(cross * 0.7, 0.3, 30_000, SR);
        assert_eq!(
            played(&mut unit(&auto), &x),
            played(&mut unit(&fix), &x),
            "Auto at {hz} Hz is the fixed {hz} Hz floor, to the bit"
        );
    }
    println!("linear-phase floors at 48 kHz: {row}");
    // A fixed floor keeps its latency whatever the crossover above it.
    for cross in [120.0f32, 500.0, 2_000.0] {
        assert_eq!(
            latency_for(&ducking(Band::Lows, cross, Floor::Hz60, 0.0), SR),
            1_765
        );
    }
    // The default: Auto, its floor 60 Hz (the default 120 Hz crossover's) — the A-2 kernel.
    let d = CedoSettings::default();
    assert_eq!((d.floor, d.auto_floor_hz), (Floor::Auto, 60.0));
    // Auto's floor stays inside the table's range.
    assert_eq!((auto_floor(40.0), auto_floor(2_000.0)), (40.0, 1_000.0));
}

#[test]
fn a_crossover_under_the_floor_is_heard_at_the_floor() {
    let x = sine(70.0, 0.3, 30_000, SR);
    let under = played(&mut unit(&ducking(Band::Lows, 60.0, Floor::Hz120, 0.0)), &x);
    let at = played(
        &mut unit(&ducking(Band::Lows, 120.0, Floor::Hz120, 0.0)),
        &x,
    );
    assert_eq!(under, at);
}

#[test]
fn every_floor_keeps_the_a2_accuracy() {
    let mut rows = String::new();
    for f in FLOORS {
        let cross = 2.0 * f;
        let lows = ducking(Band::Lows, cross, fixed(f), 0.0);
        let highs = ducking(Band::Highs, cross, fixed(f), 0.0);
        // An octave under the floor (two under the crossover) the lows reach the depth; four
        // octaves over the crossover they pass; the highs leak little an octave under the
        // crossover and reach the depth two over it.
        let (lo, lo_pass) = (sine_gain(&lows, f * 0.5), sine_gain(&lows, cross * 4.0));
        let (hi_leak, hi) = (sine_gain(&highs, f), sine_gain(&highs, cross * 2.0));
        rows += &format!(
            "{f} Hz: lows {lo:+.2} at {:.0} Hz, {lo_pass:+.3} at {:.0} Hz; highs {hi_leak:+.3} at {f} Hz, {hi:+.2} at {:.0} Hz; ",
            f * 0.5,
            cross * 4.0,
            cross * 2.0
        );
        assert!(lo <= -23.5, "{f} Hz floor: the lows at {} Hz {lo}", f * 0.5);
        assert!(lo_pass > -0.05, "{f} Hz floor: {lo_pass}");
        assert!(hi_leak > -0.3, "{f} Hz floor: the highs leak {hi_leak}");
        assert!(hi <= -23.0, "{f} Hz floor: the highs {hi}");
    }
    println!("{rows}");
}

#[test]
fn a_floor_changed_while_it_runs_is_the_input_delayed_by_the_new_latency() {
    // The null (depth 0): the delayed input, to the bit, before and after the change.
    let null = |floor| CedoSettings {
        band: Band::Lows,
        crossover: Crossover::LinearPhase,
        floor,
        ..CedoSettings::default()
    };
    let n = 40_000;
    let x = sine(333.0, 0.4, n, SR);
    let mut c = unit(&null(Floor::Hz60));
    let (mut l, mut r) = (x.clone(), x.clone());
    let cut = 19_968;
    let mut at = 0;
    while at < n {
        if at == cut {
            c.set(&null(Floor::Hz40));
        }
        let end = (at + 256).min(n);
        c.process(&mut l[at..end], &mut r[at..end], None);
        at = end;
    }
    let (a, b) = (1_765usize, 2_655usize);
    for i in a..cut {
        assert_eq!(l[i].to_bits(), x[i - a].to_bits(), "before, sample {i}");
    }
    for i in cut + b..n {
        assert_eq!(l[i].to_bits(), x[i - b].to_bits(), "after, sample {i}");
        assert_eq!(r[i].to_bits(), x[i - b].to_bits());
    }
    assert_eq!(c.latency(), b);
}
