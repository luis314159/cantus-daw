//! Purus's de-click and de-crackle measured: clicks and crackle added to music (synthetic
//! here; the CC0 guitar in the listening set), what is left of them, and what the music
//! loses — with clicks, and clean.

use super::testkit::{self, Noise};
use super::*;

const SR: f32 = 48_000.0;

/// Music: a bass line, three tones with a slow vibrato, a little noise (−60 dBFS).
fn music(seconds: f32) -> Vec<f32> {
    let notes = [
        (0.2, 1.4, 82.4),
        (1.7, 1.4, 110.0),
        (3.2, 1.4, 98.0),
        (4.7, 1.2, 73.4),
    ];
    let bass = testkit::bass(SR, seconds, &notes, 0.4);
    let mut rng = Noise::new(5);
    bass.iter()
        .enumerate()
        .map(|(i, b)| {
            let t = i as f32 / SR;
            let tau = std::f32::consts::TAU;
            let v = 1.0 + 0.003 * crate::math::sin(tau * 5.0 * t);
            b + 0.08 * crate::math::sin(tau * 440.0 * v * t)
                + 0.05 * crate::math::sin(tau * 659.3 * v * t)
                + 0.03 * crate::math::sin(tau * 1_760.0 * v * t)
                + 0.001 * rng.next()
        })
        .collect()
}

fn through(s: &PurusSettings, x: &[f32]) -> (Vec<f32>, u64) {
    let lat = latency_for(s, SR);
    let mut padded = x.to_vec();
    padded.resize(x.len() + lat, 0.0);
    let mut p = Purus::new(SR);
    p.set(s);
    let y = testkit::render(&padded, 256, |l, r| p.process(l, r)).0;
    (y[lat..].to_vec(), p.meter().clicks_total)
}

fn click(sensitivity: f32, crackle: f32) -> PurusSettings {
    PurusSettings {
        click: ClickSettings {
            on: true,
            sensitivity,
            crackle,
            ..ClickSettings::default()
        },
        ..PurusSettings::default()
    }
}

/// What is left of the damage, dB under what was added (`x − clean` against `y − clean`).
fn removed(clean: &[f32], x: &[f32], y: &[f32]) -> f64 {
    let a: Vec<f32> = x.iter().zip(clean).map(|(x, c)| x - c).collect();
    let b: Vec<f32> = y.iter().zip(clean).map(|(y, c)| y - c).collect();
    testkit::db(testkit::energy(&a) / testkit::energy(&b).max(1e-30))
}

#[test]
fn clicks_and_crackle_come_out_and_the_music_stays() {
    let clean = music(6.0);
    let mut clicked = clean.clone();
    let at = testkit::add_clicks(&mut clicked, 40, 9);
    let mut crackled = clean.clone();
    testkit::add_crackle(&mut crackled, SR, 300.0, 11);
    let mut both = clicked.clone();
    testkit::add_crackle(&mut both, SR, 300.0, 11);
    for sens in [0.5f32, 1.0] {
        let mut y = crackled.clone();
        let len = y.len();
        let n = crate::denoise::declick::declick(&mut y, 0, len, sens);
        println!(
            "  the editor's offline declick at {sens} on the same crackle: {n} rebuilt, down {:.1} dB",
            removed(&clean, &crackled, &y)
        );
    }
    let mut results = Vec::new();
    for (name, s) in [
        ("sensitivity 0.5", click(0.5, 0.0)),
        ("sensitivity 0.5, crackle 1", click(0.5, 1.0)),
        ("sensitivity 1, crackle 1", click(1.0, 1.0)),
    ] {
        let (y, n) = through(&s, &clicked);
        let c = removed(&clean, &clicked, &y);
        let (y, nk) = through(&s, &crackled);
        let k = removed(&clean, &crackled, &y);
        let ev = crackled
            .iter()
            .zip(&clean)
            .filter(|(a, b)| (*a - *b).abs() > 0.004)
            .count();
        println!("  {name}: crackle: {nk} rebuilt (both channels) of about {ev} impulse samples");
        let (y, _) = through(&s, &both);
        let b = removed(&clean, &both, &y);
        // The music alone: what it loses (the change, under the music) and how many
        // "clicks" were found in it.
        let (y, false_found) = through(&s, &clean);
        let change: Vec<f32> = y.iter().zip(&clean).map(|(y, c)| y - c).collect();
        let damage = testkit::db(testkit::energy(&clean) / testkit::energy(&change).max(1e-30));
        println!(
            "{name}: {} clicks: {n} rebuilt, their energy down {c:.1} dB; crackle down {k:.1} \
             dB; both down {b:.1} dB; clean music: {false_found} rebuilt, the change {damage:.1} \
             dB under it",
            at.len()
        );
        results.push((c, k, damage));
    }
    assert!(results[0].0 > 20.0, "{results:?}");
    assert!(
        results[1].1 >= results[0].1,
        "crackle takes no less crackle: {results:?}"
    );
    assert!(results[0].2 > 60.0, "the music is left alone: {results:?}");
}

/// The de-crackle on the owner's CC0 guitar (at its own 44.1 kHz): crackle added (300 a
/// second, 0.005–0.03 peak), what is left of it, and what the clean guitar loses. Needs the
/// strums: `cargo test -p cantus-dsp --lib purus_crackle_guitar -- --ignored --nocapture`.
#[test]
#[ignore]
fn purus_crackle_guitar() {
    let Some(clean) = super::tests_measure::cc0_guitar() else {
        println!("no strums: skipped");
        return;
    };
    let sr = 44_100.0f32;
    let mut crackled = clean.clone();
    testkit::add_crackle(&mut crackled, sr, 300.0, 53);
    let run = |s: &PurusSettings, x: &[f32]| {
        let lat = latency_for(s, sr);
        let mut padded = x.to_vec();
        padded.resize(x.len() + lat, 0.0);
        let mut p = Purus::new(sr);
        p.set(s);
        let y = testkit::render(&padded, 256, |l, r| p.process(l, r)).0;
        (y[lat..].to_vec(), p.meter().clicks_total / 2)
    };
    for crackle in [0.0f32, 0.5, 1.0] {
        let s = click(0.5, crackle);
        let (y, n) = run(&s, &crackled);
        let down = removed(&clean, &crackled, &y);
        let (yc, nc) = run(&s, &clean);
        let change: Vec<f32> = yc.iter().zip(&clean).map(|(y, c)| y - c).collect();
        let damage = testkit::db(testkit::energy(&clean) / testkit::energy(&change).max(1e-30));
        println!(
            "guitar, crackle {crackle}: {n} taken, the crackle down {down:.1} dB; the clean \
             guitar: {nc} taken, the change {damage:.1} dB under it"
        );
    }
}
