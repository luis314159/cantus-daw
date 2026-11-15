//! Purus measured: the hum taken out, the bass kept, the drift followed, the room taken out.
//! Each test prints what it measured (`cargo test -p cantus-dsp --lib purus -- --nocapture`);
//! the bounds are a little under what was reached, so a regression fails.

use super::testkit::{self, Noise};
use super::*;

const SR: f32 = 48_000.0;

fn mono(s: &PurusSettings, x: &[f32]) -> Vec<f32> {
    let mut p = Purus::new(SR);
    p.set(s);
    testkit::render(x, 256, |l, r| p.process(l, r)).0
}

fn hum_settings(f: impl FnOnce(&mut HumSettings)) -> PurusSettings {
    let mut h = HumSettings {
        on: true,
        ..HumSettings::default()
    };
    f(&mut h);
    PurusSettings {
        hum: h,
        ..PurusSettings::default()
    }
}

/// The hum's harmonics (dBFS: −30 at 50 Hz down to −48) and a buzz.
const HUM: [f32; 8] = [0.0316, 0.02, 0.0251, 0.01, 0.0126, 0.005, 0.0063, 0.004];

fn reduction_at(x: &[f32], y: &[f32], hz: f64) -> f64 {
    20.0 * crate::math::log10(
        testkit::level_at(x, SR, hz) / testkit::level_at(y, SR, hz).max(1e-12),
    )
}

/// Music away from the harmonics: three tones and a little noise.
fn music(seconds: f32) -> Vec<f32> {
    let mut rng = Noise::new(77);
    (0..(seconds * SR) as usize)
        .map(|i| {
            let t = i as f32 / SR;
            let tau = std::f32::consts::TAU;
            0.1 * crate::math::sin(tau * 440.0 * t)
                + 0.08 * crate::math::sin(tau * 659.3 * t)
                + 0.05 * crate::math::sin(tau * 1_318.5 * t)
                + 0.01 * rng.next()
        })
        .collect()
}

#[test]
fn the_hum_and_its_harmonics_come_out() {
    let secs = 6.0;
    let hum = testkit::hum(SR, secs, &HUM, 0.004, 1, |_| 50.0);
    let m = music(secs);
    let x: Vec<f32> = hum.iter().zip(&m).map(|(a, b)| a + b).collect();
    let from = (2.0 * SR) as usize;
    for (name, s) in [
        ("notch 40 dB, 2 Hz", hum_settings(|_| {})),
        (
            "notch 60 dB, 1 Hz",
            hum_settings(|h| {
                h.depth_db = 60.0;
                h.width_hz = 1.0;
            }),
        ),
        ("gentle 40 dB, 2 Hz", hum_settings(|h| h.gentle = true)),
    ] {
        let y = mono(&s, &x);
        // The hum left: the output minus the music (the music's own loss is small and
        // measured apart), at each harmonic.
        let left: Vec<f32> = y[from..]
            .iter()
            .zip(&m[from..])
            .map(|(a, b)| a - b)
            .collect();
        let worst = (1..=8)
            .map(|k| reduction_at(&hum[from..], &left, 50.0 * k as f64))
            .fold(f64::MAX, f64::min);
        let each: Vec<String> = (1..=8)
            .map(|k| format!("{:.1}", reduction_at(&hum[from..], &left, 50.0 * k as f64)))
            .collect();
        println!("{name}: by harmonic {}", each.join(" "));
        let total = testkit::db(testkit::energy(&hum[from..]) / testkit::energy(&left).max(1e-30));
        let music_loss = [440.0, 659.3, 1_318.5]
            .iter()
            .map(|hz| reduction_at(&m[from..], &y[from..], *hz))
            .fold(0.0f64, f64::max);
        println!(
            "{name}: harmonics down {worst:.1} dB at worst; the hum's energy (with the buzz) \
             down {total:.1} dB; the music's tones lose {music_loss:.2} dB at most"
        );
        assert!(worst > 30.0, "{name}: {worst}");
        assert!(music_loss < 0.2, "{name}: {music_loss}");
    }
}

/// A bass line on the hum's own frequencies, and next to them.
const NOTES: [(f32, f32, f32); 6] = [
    (1.5, 0.6, 50.0),
    (2.2, 0.4, 100.0),
    (2.7, 0.8, 150.0),
    (3.6, 0.5, 55.0),
    (4.2, 0.5, 98.0),
    (4.8, 1.2, 50.0),
];

#[test]
fn the_gentle_mode_keeps_a_bass_on_the_hums_frequencies() {
    let secs = 6.5;
    let hum = testkit::hum(SR, secs, &HUM, 0.0, 2, |_| 50.0);
    let bass = testkit::bass(SR, secs, &NOTES, 0.5);
    let x: Vec<f32> = hum.iter().zip(&bass).map(|(a, b)| a + b).collect();
    let from = (1.5 * SR) as usize;
    let mut found = Vec::new();
    for (name, s) in [
        ("notch", hum_settings(|_| {})),
        ("gentle", hum_settings(|h| h.gentle = true)),
    ] {
        let y = mono(&s, &x);
        let y_hum = mono(&s, &hum);
        // The bass as it comes out: the output less the hum's own output (exact for the
        // notches, linear; for the gentle mode, its learning held, nearly so).
        let b_out: Vec<f32> = y.iter().zip(&y_hum).map(|(a, b)| a - b).collect();
        let loss = testkit::db(testkit::energy(&b_out[from..]) / testkit::energy(&bass[from..]));
        let mut worst_note = 0.0f64;
        let mut per_note = String::new();
        for (s0, len, hz) in NOTES {
            let a = (s0 * SR) as usize;
            let e = ((s0 + len) * SR) as usize;
            let l = testkit::db(testkit::energy(&b_out[a..e]) / testkit::energy(&bass[a..e]));
            per_note += &format!(" {hz} Hz {l:+.2};");
            worst_note = worst_note.min(l);
        }
        // The hum left under the bass: the bass played inverted too, the two outputs
        // averaged (the bass cancels; exact for the notches, nearly so for the gentle
        // mode), at the harmonics, while the bass plays.
        let x2: Vec<f32> = hum.iter().zip(&bass).map(|(a, b)| a - b).collect();
        let y2 = mono(&s, &x2);
        let left: Vec<f32> = y[from..]
            .iter()
            .zip(&y2[from..])
            .map(|(a, b)| 0.5 * (a + b))
            .collect();
        let each: Vec<String> = (1..=8)
            .map(|k| format!("{:.1}", reduction_at(&hum[from..], &left, 50.0 * k as f64)))
            .collect();
        let hum_down = (1..=8)
            .map(|k| reduction_at(&hum[from..], &left, 50.0 * k as f64))
            .fold(f64::MAX, f64::min);
        println!(
            "{name}: the hum under the bass by harmonic {}",
            each.join(" ")
        );
        println!(
            "{name}: the bass {loss:+.2} dB in all, by note:{per_note} the hum under it down \
             {hum_down:.1} dB at the worst harmonic"
        );
        found.push((loss, worst_note, hum_down));
    }
    let (notch, gentle) = (found[0], found[1]);
    assert!(
        notch.1 < -6.0,
        "the notches do take a bass on 50 Hz: {notch:?}"
    );
    assert!(gentle.1 > -1.0, "gentle keeps it: {gentle:?}");
    assert!(gentle.2 > 20.0, "and still takes the hum: {gentle:?}");
}

/// Render with a look at the meter after every block.
fn watched(s: &PurusSettings, x: &[f32], mut look: impl FnMut(usize, &Purus)) -> Vec<f32> {
    let mut p = Purus::new(SR);
    p.set(s);
    let mut at = 0;
    testkit::render(x, 256, |l, r| {
        p.process(l, r);
        at += l.len();
        look(at, &p);
    })
    .0
}

#[test]
fn tracking_follows_a_mains_drifting_one_hertz_either_way() {
    for period in [60.0, 10.0] {
        let secs = 20.0;
        let f0 = move |t: f64| 50.0 + crate::math::sin(std::f64::consts::TAU * t / period);
        let hum = testkit::hum(SR, secs, &HUM, 0.0, 3, f0);
        let theta = testkit::hum_phase(SR, hum.len(), f0);
        let m = music(secs);
        let x: Vec<f32> = hum.iter().zip(&m).map(|(a, b)| a + b).collect();
        let from = (3.0 * SR) as usize;
        let mut results = Vec::new();
        for (name, mode) in [("fixed", HumMode::Fixed), ("tracked", HumMode::Track)] {
            let s = hum_settings(|h| h.mode = mode);
            let (mut worst, mut sq, mut n) = (0.0f64, 0.0f64, 0usize);
            let y = watched(&s, &x, |at, p| {
                if at >= from {
                    // The meter's fundamental against the truth half a block ago.
                    let truth = f0((at as f64 - 128.0) / SR as f64);
                    let e = (p.meter().hum_hz as f64 - truth).abs();
                    worst = worst.max(e);
                    sq += e * e;
                    n += 1;
                }
            });
            // What is left of each harmonic, followed as it moves.
            let left: Vec<f32> = y[from..]
                .iter()
                .zip(&m[from..])
                .map(|(a, b)| a - b)
                .collect();
            let each: Vec<f64> = (1..=8)
                .map(|k| {
                    let l = testkit::level_following(&left, &theta[from..], k);
                    20.0 * crate::math::log10(HUM[k - 1] as f64 / l.max(1e-12))
                })
                .collect();
            let down = each.iter().cloned().fold(f64::MAX, f64::min);
            let rms = (sq / n as f64).sqrt();
            let each: Vec<String> = each.iter().map(|v| format!("{v:.1}")).collect();
            println!(
                "{name}: the hum (50 ± 1 Hz, a {period} s swing) down {down:.1} dB at the worst harmonic ({}); the fundamental followed within {worst:.3} Hz (RMS {rms:.4})",
                each.join(" ")
            );
            results.push((down, worst));
        }
        assert!(results[1].0 > results[0].0 + 10.0, "{results:?}");
        assert!(results[1].1 < 0.1, "{results:?}");
    }
}

#[test]
fn auto_finds_sixty_hertz_and_stays_on_fifty() {
    let secs = 6.0;
    for (hz, want) in [(60.0, 60.0f32), (50.0, 50.0)] {
        let f = hz + 0.3;
        let hum = testkit::hum(SR, secs, &HUM, 0.0, 4, move |_| f);
        let m = music(secs);
        let x: Vec<f32> = hum.iter().zip(&m).map(|(a, b)| a + b).collect();
        let s = hum_settings(|h| h.mode = HumMode::Auto);
        let mut found = None;
        let y = watched(&s, &x, |at, p| {
            let mt = p.meter();
            if found.is_none() && mt.hum_nominal_hz == want && (mt.hum_hz as f64 - f).abs() < 0.05 {
                found = Some(at as f32 / SR);
            }
        });
        let from = (4.0 * SR) as usize;
        let left: Vec<f32> = y[from..]
            .iter()
            .zip(&m[from..])
            .map(|(a, b)| a - b)
            .collect();
        let down = (1..=8)
            .map(|k| reduction_at(&hum[from..], &left, f * k as f64))
            .fold(f64::MAX, f64::min);
        println!(
            "Auto on a {f:.1} Hz hum: locked within 0.05 Hz at {found:?} s; its harmonics down {down:.1} dB at the worst over the last 2 s"
        );
        assert!(found.is_some_and(|t| t < 3.0), "{hz}: {found:?}");
        assert!(down > 25.0, "{hz}: {down}");
    }
}

#[test]
fn the_gentle_mode_learns_a_hum_that_changes() {
    // The hum doubles and turns at 3 s (another device switched on): learning, held at
    // first as if a note had come, must take the new hum within a few seconds.
    let secs = 12.0;
    let a = testkit::hum(SR, secs, &HUM, 0.0, 5, |_| 50.0);
    let b = testkit::hum(SR, secs, &HUM, 0.0, 6, |_| 50.0);
    let cut = (3.0 * SR) as usize;
    let x: Vec<f32> = a
        .iter()
        .zip(&b)
        .enumerate()
        .map(|(i, (a, b))| if i < cut { *a } else { a + b })
        .collect();
    let y = mono(&hum_settings(|h| h.gentle = true), &x);
    let mut back = None;
    for w in 0..18 {
        let s0 = cut + w * (SR as usize / 2);
        let e = s0 + SR as usize / 2;
        let d = testkit::db(testkit::energy(&x[s0..e]) / testkit::energy(&y[s0..e]));
        if back.is_none() && d > 30.0 {
            back = Some(w as f32 * 0.5);
        }
    }
    println!("gentle: a hum that changed taken out again (30 dB) {back:?} s after the change");
    assert!(back.is_some_and(|t| t <= 5.0), "{back:?}");
}
