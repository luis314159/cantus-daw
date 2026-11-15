//! Purus's de-reverb measured on a synthetic spoken line in synthetic rooms of known decay
//! and direct-to-reverberant ratio (`testkit::room`): the late tail taken out, the ratios
//! gained, the musical noise (the kurtosis ratio), the decay estimated, a dry voice spared.

use super::testkit;
use super::*;

const SR: f32 = 48_000.0;

fn verb(f: impl FnOnce(&mut VerbSettings)) -> PurusSettings {
    let mut v = VerbSettings {
        on: true,
        ..VerbSettings::default()
    };
    f(&mut v);
    PurusSettings {
        verb: v,
        ..PurusSettings::default()
    }
}

/// The output aligned to the input (the latency taken off), as long as the input.
fn through(s: &PurusSettings, x: &[f32]) -> Vec<f32> {
    let lat = latency_for(s, SR);
    let mut padded = x.to_vec();
    padded.resize(x.len() + lat, 0.0);
    let mut p = Purus::new(SR);
    p.set(s);
    let y = testkit::render(&padded, 256, |l, r| p.process(l, r)).0;
    y[lat..].to_vec()
}

/// A ratio of energies, dB: what `target` (scaled to fit `y` best) holds against the rest of
/// `y`. On the input with the true part it is the room's ratio; on the output, the gain
/// fitted takes the processing's own level change out.
fn ratio(y: &[f32], target: &[f32]) -> (f64, f64) {
    let g = testkit::projection(y, target);
    let rest: Vec<f32> = y.iter().zip(target).map(|(a, t)| a - g as f32 * t).collect();
    let fit = g * g * testkit::energy(target);
    (testkit::db(fit / testkit::energy(&rest)), 20.0 * crate::math::log10(g.abs().max(1e-12)))
}

struct Scene {
    dry: Vec<f32>,
    wet: Vec<f32>,
    early: Vec<f32>,
    /// Where the line's last syllable ends.
    end: usize,
}

fn scene(t60: f32, drr_db: f32, seed: u32) -> Scene {
    let dry = testkit::spoken(SR, 150.0, 16, 1.5, seed);
    let end = dry.len() - (1.5 * SR) as usize;
    let (h, late) = testkit::room(SR, t60, drr_db, 50.0, seed + 1);
    let mut wet = testkit::convolve(&dry, &h);
    wet.truncate(dry.len());
    let mut early = testkit::convolve(&dry, &h[..late]);
    early.truncate(dry.len());
    Scene {
        dry,
        wet,
        early,
        end,
    }
}

struct Measured {
    late_down: f64,
    tail_down: f64,
    early_change: f64,
    elr: (f64, f64),
    drr: (f64, f64),
    kurtosis_log: f64,
}

/// `comp` (a part of the mixture `mix`) through the gains the processing gives `mix`,
/// aligned: what the processing does to that part.
fn shadow(s: &PurusSettings, mix: &[f32], comp: &[f32]) -> Vec<f32> {
    let lat = latency_for(s, SR);
    let mut l = mix.to_vec();
    let mut r = comp.to_vec();
    l.resize(mix.len() + lat, 0.0);
    r.resize(mix.len() + lat, 0.0);
    let mut p = Purus::new(SR);
    p.set(s);
    p.dereverb_mut().set_shadow(true);
    let mut i = 0;
    while i < l.len() {
        let e = (i + 256).min(l.len());
        p.process(&mut l[i..e], &mut r[i..e]);
        i = e;
    }
    r[lat..].to_vec()
}

fn measure(sc: &Scene, s: &PurusSettings) -> Measured {
    let e = testkit::energy;
    let y = through(s, &sc.wet);
    let late: Vec<f32> = sc.wet.iter().zip(&sc.early).map(|(w, x)| w - x).collect();
    let rev: Vec<f32> = sc.wet.iter().zip(&sc.dry).map(|(w, x)| w - x).collect();
    let early_out = shadow(s, &sc.wet, &sc.early);
    let late_out = shadow(s, &sc.wet, &late);
    let direct_out = shadow(s, &sc.wet, &sc.dry);
    let rev_out = shadow(s, &sc.wet, &rev);
    // The tail after the line: 50–800 ms after its last syllable.
    let (a, b) = (sc.end + (0.05 * SR) as usize, sc.end + (0.8 * SR) as usize);
    let k_in = testkit::power_kurtosis(&sc.wet[a..b], SR, 100.0, 8_000.0);
    let k_out = testkit::power_kurtosis(&y[a..b], SR, 100.0, 8_000.0);
    Measured {
        late_down: testkit::db(e(&late) / e(&late_out)),
        tail_down: testkit::db(e(&sc.wet[a..b]) / e(&y[a..b]).max(1e-30)),
        early_change: testkit::db(e(&early_out) / e(&sc.early)),
        elr: (
            testkit::db(e(&sc.early) / e(&late)),
            testkit::db(e(&early_out) / e(&late_out)),
        ),
        drr: (
            testkit::db(e(&sc.dry) / e(&rev)),
            testkit::db(e(&direct_out) / e(&rev_out)),
        ),
        kurtosis_log: crate::math::log10(k_out / k_in),
    }
}

fn show(name: &str, m: &Measured) {
    println!(
        "{name}: the late reverberation down {:.1} dB over the line, {:.1} dB in the tail after it; the early part {:+.2} dB; early-to-late {:+.1} → {:+.1} dB ({:+.1}); direct-to-reverberant {:+.1} → {:+.1} dB ({:+.1}); kurtosis log-ratio {:+.3}",
        m.late_down,
        m.tail_down,
        m.early_change,
        m.elr.0,
        m.elr.1,
        m.elr.1 - m.elr.0,
        m.drr.0,
        m.drr.1,
        m.drr.1 - m.drr.0,
        m.kurtosis_log
    );
}

#[test]
fn the_late_tail_comes_out_of_a_known_room() {
    let sc = scene(0.8, 0.0, 5);
    let matched = |amount: f32, smoothing: f32| {
        verb(|v| {
            v.decay_s = 0.8;
            v.direct_db = 0.0;
            v.keep_ms = 50.0;
            v.amount_db = amount;
            v.smoothing = smoothing;
        })
    };
    let m = measure(&sc, &matched(12.0, 0.5));
    show("T60 0.8 s, DRR 0 dB, the defaults (12 dB)", &m);
    let first = (m.late_down, m.elr.1 - m.elr.0);
    let m = measure(&sc, &matched(24.0, 0.5));
    show("the same, 24 dB", &m);
    let m = measure(&sc, &matched(24.0, 0.0));
    show("the same, 24 dB, no smoothing", &m);
    let m = measure(
        &sc,
        &verb(|v| {
            v.decay_s = 0.8;
            v.direct_db = 6.0;
        }),
    );
    show("the same, 12 dB, direct +6 dB (6 dB too high)", &m);
    let m = measure(&sc, &matched(24.0, 1.0));
    show("the same, 24 dB, full smoothing", &m);
    // Lebart's model (no direct sound assumed): more taken, the early part too.
    let m = measure(
        &sc,
        &verb(|v| {
            v.decay_s = 0.8;
            v.direct_db = -10.0;
            v.amount_db = 24.0;
        }),
    );
    show("the same, 24 dB, direct −10 dB (≈ Lebart)", &m);
    // A longer room, a closer microphone.
    let sc = scene(1.6, 6.0, 9);
    let m = measure(
        &sc,
        &verb(|v| {
            v.decay_s = 1.6;
            v.direct_db = 6.0;
            v.amount_db = 24.0;
        }),
    );
    show("T60 1.6 s, DRR +6 dB, 24 dB", &m);
    assert!(first.0 > 4.0 && first.1 > 3.0, "{first:?}");
    assert!(m.tail_down > 10.0, "{}", m.tail_down);
}

#[test]
fn the_decay_is_estimated_from_the_signal() {
    let mut ests = Vec::new();
    for (t60, seed) in [(0.5f32, 11u32), (0.9, 13), (1.5, 17)] {
        let sc = scene(t60, 0.0, seed);
        let s = verb(|v| v.decay = DecayMode::Estimate);
        let mut p = Purus::new(SR);
        p.set(&s);
        let _ = testkit::render(&sc.wet, 256, |l, r| p.process(l, r));
        let est = p.meter().decay_estimated_s;
        println!("a room of T60 {t60} s: estimated {est:.2} s");
        ests.push((t60, est));
    }
    for (t60, est) in ests {
        assert!(est > 0.0 && (est / t60 - 1.0).abs() < 0.35, "{t60}: {est}");
    }
}

#[test]
fn a_dry_voice_is_nearly_left_alone() {
    let dry = testkit::spoken(SR, 150.0, 16, 0.5, 21);
    for (name, s) in [
        ("defaults (0.8 s, 0 dB, 12 dB)", verb(|_| {})),
        ("24 dB", verb(|v| v.amount_db = 24.0)),
        ("direct +6 dB", verb(|v| v.direct_db = 6.0)),
        ("direct +12 dB", verb(|v| v.direct_db = 12.0)),
    ] {
        let y = through(&s, &dry);
        let (snr, gain) = ratio(&y, &dry);
        if name.starts_with("direct +12") {
            assert!(gain.abs() < 0.5 && snr > 25.0, "{gain} {snr}");
        }
        println!("a dry voice through the {name}: its level {gain:+.2} dB, what changed {snr:.1} dB under it");
    }
}
