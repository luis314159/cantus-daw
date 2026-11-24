//! The upper strings' rate, measured (round 42, `arcus-upper`): the violin and the viola
//! bowed through the whole instrument at 1×, 2× and 4× the rate — the pitch the closed loop
//! holds, the contact point the bowing reaches, the motion's regularity, the energy that is
//! not a harmonic (aliasing: a bowed string's spectrum is harmonic, so whatever lies between
//! the harmonics, with the hair's noise and the vibrato off, folded back from above the
//! rate), and the cost. `upper::factor` is chosen from what this prints.

use super::tables::Instrument;
use super::tests::{measure_hz, SR};
use super::*;
use crate::math;

/// A held note's settings for the probes: no vibrato, no body, no hair noise, the dynamic
/// from the velocity, the voice at `os`.
pub(super) fn held(inst: Instrument, os: Option<u8>) -> ArcusSettings {
    ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        body_mix: 0.0,
        bow_noise_db: -60.0,
        dyn_src: DynSource::Velocity,
        oversample: os,
        ..ArcusSettings::default()
    }
}

/// Play `key` at `velocity` for `seconds` with `s`; the left channel, and the bowing's β
/// the voice reached (its telemetry at the end).
pub(super) fn play(s: &ArcusSettings, key: u8, velocity: f32, seconds: f64) -> (Vec<f32>, f32) {
    let mut a = Arcus::new(SR);
    a.set(s);
    a.note_on(key, velocity, 1);
    let n = (seconds * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    for at in (0..n).step_by(256) {
        let k = 256.min(n - at);
        a.process(&mut l[at..at + k], &mut r[at..at + k]);
    }
    let beta = a.newest().beta();
    (l, beta)
}

/// The power between the harmonics over the power on them, dB, over `x[from..from + 2^15]`
/// (a Hann window; a harmonic's bins are those within 4 of `k·f0`), above 150 Hz.
pub(super) fn inharmonic_db(x: &[f32], from: usize, f0: f64) -> f64 {
    let n = 1usize << 15;
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for i in 0..n {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        re[i] = (x[from + i] as f64 * w) as f32;
    }
    crate::spectrum::fft(&mut re, &mut im);
    let hz = SR as f64 / n as f64;
    let (mut on, mut off) = (0.0f64, 0.0f64);
    for b in 1..n / 2 {
        let f = b as f64 * hz;
        if f < 150.0 || f > 20_000.0 {
            continue;
        }
        let p = (re[b] as f64).powi(2) + (im[b] as f64).powi(2);
        let k = (f / f0).round();
        if k >= 1.0 && (f - k * f0).abs() <= 4.0 * hz {
            on += p;
        } else {
            off += p;
        }
    }
    10.0 * math::log10(off.max(1e-30) / on.max(1e-30))
}

/// The bowed pitch's error with the closed loop alone (no table for the upper strings yet),
/// cents, measured over 0.9 … 1.5 s.
pub(super) fn cents_off(x: &[f32], key: u8) -> f64 {
    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let got = measure_hz(x, f, 0.9, 1.5);
    1200.0 * math::log2(got / f)
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::upper_tests_rate::probe_rate -- --ignored --nocapture"]
fn probe_rate() {
    for (inst, keys) in [
        (Instrument::Violin, &[55u8, 67, 76, 81, 88, 93, 100][..]),
        (Instrument::Viola, &[48u8, 60, 69, 76, 81, 88, 93][..]),
    ] {
        for os in [1u8, 2, 4] {
            let s = held(inst, Some(os));
            let started = std::time::Instant::now();
            let mut rows = Vec::new();
            for &key in keys {
                for vel in [0.35f32, 0.95] {
                    let (x, beta) = play(&s, key, vel, 1.6);
                    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
                    let off = cents_off(&x, key);
                    let alias = inharmonic_db(&x, (0.8 * SR) as usize, f);
                    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
                    rows.push(format!(
                        "  key {key:3} vel {vel:.2}: {off:+7.3} ct  β {beta:.3}  between harmonics {alias:6.1} dB  peak {peak:.3}"
                    ));
                }
            }
            let secs = started.elapsed().as_secs_f64();
            println!(
                "{inst:?} at {os}×: {} notes of 1.6 s in {secs:.2} s ({:.2} ms a note-second)",
                rows.len(),
                1000.0 * secs / (rows.len() as f64 * 1.6)
            );
            for r in rows {
                println!("{r}");
            }
        }
    }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::upper_tests_rate::probe_corner -- --ignored --nocapture"]
fn probe_corner() {
    for ratio in [0.0f64, 2.0, 3.0, 4.0, 6.0] {
        super::upper::CORNER_RATIO.with(|c| c.set(ratio));
        for inst in [Instrument::Violin, Instrument::Viola] {
            let s = held(inst, None);
            let top = inst.spec().top;
            let mut row = Vec::new();
            for key in [top - 12, top - 7, top - 3, top] {
                for vel in [0.35f32, 0.95] {
                    let (x, _) = play(&s, key, vel, 1.6);
                    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
                    row.push(format!(
                        "{key}/{vel}: {:+.2} ct {:.0} dB",
                        cents_off(&x, key),
                        inharmonic_db(&x, (0.8 * SR) as usize, f)
                    ));
                }
            }
            println!("corner ≥ {ratio} f0, {inst:?}: {}", row.join("  "));
        }
    }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::upper_tests_rate::probe_top -- --ignored --nocapture"]
fn probe_top() {
    for key in [97u8, 100] {
        for (speed, force, pos) in [
            (1.0f32, 0.0f32, 0.0f32),
            (0.7, 0.0, 0.0),
            (0.5, 0.0, 0.0),
            (1.0, -0.5, 0.0),
            (1.0, 0.5, 0.0),
            (1.0, 0.0, 0.5),
            (1.0, 0.0, -0.5),
            (0.7, -0.5, 0.5),
        ] {
            let s = ArcusSettings {
                bow_speed: speed,
                bow_force: force,
                bow_pos: pos,
                ..held(Instrument::Violin, None)
            };
            let (x, beta) = play(&s, key, 0.95, 1.6);
            let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
            println!(
                "key {key} ff speed ×{speed} force {force:+} pos {pos:+}: {:+.2} ct {:.0} dB β {beta:.3}",
                cents_off(&x, key),
                inharmonic_db(&x, (0.8 * SR) as usize, f)
            );
        }
    }
}
