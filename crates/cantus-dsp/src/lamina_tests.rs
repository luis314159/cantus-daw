//! Lamina's DSP, measured: pitch, the pickup's octave jump and bark, velocity and
//! brightness, half-pedalling, re-strikes, steals, determinism and silence.

use super::geometry::Engine;
use super::*;
use crate::math;

const SR: f32 = 48_000.0;

fn plain() -> LaminaSettings {
    LaminaSettings {
        spread_ct: 0.0,
        age: 0.0,
        hammer_noise_db: -120.0,
        release_noise_db: -120.0,
        drive: 0.0,
        ..LaminaSettings::default()
    }
}

fn lamina(s: &LaminaSettings) -> Lamina {
    let mut l = Lamina::new(SR);
    l.set(s);
    l
}

fn run(l: &mut Lamina, frames: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
    let mut a = vec![0.0; frames];
    let mut b = vec![0.0; frames];
    let mut i = 0;
    while i < frames {
        let n = block.min(frames - i);
        l.process(&mut a[i..i + n], &mut b[i..i + n]);
        i += n;
    }
    (a, b)
}

/// Amplitude at `hz` (Goertzel over a Hann window).
fn amp_at(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut re, mut im, mut norm) = (0.0, 0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        re += *v as f64 * win * math::cos(w * i as f64);
        im += *v as f64 * win * math::sin(w * i as f64);
        norm += win;
    }
    2.0 * (re * re + im * im).sqrt() / norm
}

/// The strongest frequency near `guess`, refined (Goertzel scan then parabola), Hz.
fn peak_hz(x: &[f32], guess: f64) -> f64 {
    let mut best = (guess, 0.0);
    let span = guess * 0.02;
    let steps = 400;
    for k in 0..=steps {
        let f = guess - span + 2.0 * span * k as f64 / steps as f64;
        let a = amp_at(x, f);
        if a > best.1 {
            best = (f, a);
        }
    }
    // Refine around the best with a finer scan.
    let span2 = 2.0 * span / steps as f64;
    let mut fine = best;
    for k in 0..=200 {
        let f = best.0 - span2 + 2.0 * span2 * k as f64 / 200.0;
        let a = amp_at(x, f);
        if a > fine.1 {
            fine = (f, a);
        }
    }
    fine.0
}

#[test]
fn every_key_sounds_its_equal_tempered_pitch() {
    for engine in [Engine::Tine, Engine::Reed] {
        let s = LaminaSettings { engine, ..plain() };
        for key in [33u8, 45, 57, 60, 69, 81, 93] {
            let mut l = lamina(&s);
            l.note_on(key, 1, 0.6, None);
            let (a, _) = run(&mut l, 48_000, 256);
            let want = 440.0 * math::powf(2f64, (key as f64 - 69.0) / 12.0);
            let hz = peak_hz(&a[9_600..], want);
            let ct = 1200.0 * math::log2(hz / want);
            assert!(
                ct.abs() < 0.5,
                "{engine:?} key {key}: {hz:.3} Hz, {ct:+.3} ct"
            );
        }
    }
}

#[test]
fn a_centred_pickup_jumps_an_octave_and_an_offset_lowers_it() {
    // Up to the flux curve's inflection (half the gap: 0.8 mm at 1.6 mm), the further off
    // centre, the less octave; past it the curve bends the other way and the even
    // harmonics return, as on the instrument.
    let mut last = f64::INFINITY;
    for align in [0.0f32, 0.2, 0.4, 0.6, 0.8] {
        let s = LaminaSettings {
            tine_align_mm: align,
            ..plain()
        };
        let mut l = lamina(&s);
        l.note_on(57, 1, 0.7, None);
        let (a, _) = run(&mut l, 24_000, 256);
        let x = &a[4_800..];
        let f0 = 220.0;
        let ratio = 20.0 * math::log10(amp_at(x, 2.0 * f0) / amp_at(x, f0));
        if align == 0.0 {
            assert!(ratio > 20.0, "centred: 2nd over 1st {ratio:.1} dB");
        }
        assert!(
            ratio < last,
            "offset {align} mm: {ratio:.1} dB after {last:.1} dB"
        );
        last = ratio;
    }
}

#[test]
fn the_bark_and_the_level_rise_with_velocity() {
    // The bark: the harmonics the pickup adds (2nd…8th) against the fundamental.
    for engine in [Engine::Tine, Engine::Reed] {
        for key in [40u8, 60, 76] {
            let f0 = 440.0 * math::powf(2f64, (key as f64 - 69.0) / 12.0);
            let mut last = (f64::NEG_INFINITY, 0.0);
            for step in 1..=8 {
                let v = step as f32 / 8.0;
                let s = LaminaSettings {
                    engine,
                    bell_db: -40.0,
                    overtones_db: -40.0,
                    ..plain()
                };
                let mut l = lamina(&s);
                l.note_on(key, 1, v, None);
                let (a, _) = run(&mut l, 9_600, 256);
                let x = &a[960..];
                let fund = amp_at(x, f0);
                let harm: f64 = (2..=8)
                    .map(|h| math::powi(amp_at(x, f0 * h as f64), 2))
                    .sum::<f64>()
                    .sqrt();
                let bark = 20.0 * math::log10(harm / fund);
                let peak = a.iter().fold(0f32, |m, x| m.max(x.abs())) as f64;
                assert!(
                    bark > last.0 && peak >= last.1,
                    "{engine:?} key {key} velocity {v}: bark {bark:.1} dB (was {:.1}), peak {peak:.4} (was {:.4})",
                    last.0,
                    last.1
                );
                last = (bark, peak);
            }
        }
    }
}

#[test]
fn a_faster_hammer_reaches_further_up_the_modes() {
    // Through an almost linear pickup (far and well off centre): the attack's high end
    // (its second difference) against its level — the power-law spring's shorter contact
    // at speed.
    // (The softest strikes all last the half period the tine allows, so the high end is flat
    // there before it rises.)
    for key in [36u8, 48, 60] {
        let mut last = f64::NEG_INFINITY;
        let mut first = None;
        for step in 1..=6 {
            let v = step as f32 / 6.0;
            let s = LaminaSettings {
                tine_gap_mm: 5.0,
                tine_align_mm: 2.0,
                bell_decay_s: 2.0,
                ..plain()
            };
            let mut l = lamina(&s);
            l.note_on(key, 1, v, None);
            let (a, _) = run(&mut l, 2_400, 256);
            let d2: Vec<f32> = a.windows(3).map(|w| w[2] - 2.0 * w[1] + w[0]).collect();
            let rel = 20.0 * math::log10(rms(&d2) / rms(&a));
            assert!(
                rel >= last - 0.05,
                "key {key} velocity {v}: high end {rel:.2} dB (was {last:.2})"
            );
            first.get_or_insert(rel);
            last = rel;
        }
        let rise = last - first.unwrap();
        assert!(
            rise > 3.0,
            "key {key}: the high end rose only {rise:.2} dB from pp to ff"
        );
    }
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| math::powi(*v as f64, 2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// Seconds for the voice's fundamental to fall 20 dB after the key comes up, × 3.
fn release_t60(pedal: f32) -> f32 {
    let mut l = lamina(&plain());
    l.set_pedal(pedal);
    l.note_on(57, 1, 0.8, None);
    run(&mut l, 9_600, 64);
    l.note_off(57, 1, 0.5);
    run(&mut l, 480, 64); // the damper travels
    let a0 = l.voice(0).amplitude();
    let mut t = 0usize;
    while l.voice(0).active && l.voice(0).amplitude() > a0 * 0.1 && t < 48_000 * 20 {
        run(&mut l, 64, 64);
        t += 64;
    }
    t as f32 / SR * 3.0
}

#[test]
fn half_pedalling_moves_the_decay_between_damped_and_free() {
    let damped = release_t60(0.0);
    assert!(
        (damped - 0.12).abs() < 0.12 * 0.15,
        "pedal up: T60 {damped:.3} s, the damper's 0.12 s"
    );
    let free = release_t60(1.0);
    // Free: the fundamental's two parts (tine 35 % and bar 100 % of 8 s at C4, ×1.09 at A3).
    assert!(free > 3.0, "pedal down: T60 {free:.2} s");
    let mut last = damped;
    for p in [0.26f32, 0.30, 0.34, 0.38, 0.42] {
        let t = release_t60(p);
        assert!(t >= last, "pedal {p}: T60 {t:.3} s after {last:.3} s");
        last = t;
    }
    assert!(last <= free);
}

#[test]
fn a_key_struck_while_it_rings_is_struck_on_the_moving_tine() {
    let mut l = lamina(&plain());
    l.set_pedal(1.0);
    l.note_on(60, 1, 0.8, None);
    run(&mut l, 4_800, 256);
    l.note_off(60, 1, 0.5);
    run(&mut l, 4_800, 256);
    l.note_on(60, 2, 0.8, None);
    run(&mut l, 480, 256);
    assert_eq!(l.active_voices(), 1, "one tine, struck twice");
}

#[test]
fn a_stolen_voice_fades_instead_of_clicking() {
    let s = LaminaSettings {
        polyphony: 2,
        ..plain()
    };
    let mut l = lamina(&s);
    l.note_on(48, 1, 1.0, None);
    l.note_on(52, 2, 1.0, None);
    run(&mut l, 9_600, 256);
    l.note_on(55, 3, 1.0, None);
    let (a, _) = run(&mut l, 960, 256);
    let before = {
        let mut m = lamina(&s);
        m.note_on(48, 1, 1.0, None);
        m.note_on(52, 2, 1.0, None);
        let (x, _) = run(&mut m, 9_600 + 960, 256);
        x[9_600..]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0f32, f32::max)
    };
    let step = a
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0f32, f32::max);
    assert!(
        step < before * 2.0 + 0.01,
        "steal step {step} against the steady {before}"
    );
    assert_eq!(l.polyphony(), 2);
}

fn scenario(block: usize) -> Vec<f32> {
    let s = LaminaSettings {
        trem_depth: 0.6,
        polyphony: 4,
        ..LaminaSettings::default()
    };
    let mut l = lamina(&s);
    let mut out = Vec::new();
    let mut go = |l: &mut Lamina, n: usize| {
        let (a, b) = run(l, n, block);
        out.extend(a.iter().zip(&b).flat_map(|(x, y)| [*x, *y]));
    };
    l.note_on(48, 1, 0.9, None);
    l.note_on(55, 2, 0.5, None);
    go(&mut l, 3_001);
    l.set_pedal(0.3);
    l.note_on(64, 3, 1.0, None);
    l.note_on(67, 4, 0.7, None);
    l.note_on(72, 5, 0.8, None); // steals
    go(&mut l, 2_222);
    l.note_off(48, 1, 0.3);
    l.set_bend(0.5);
    l.note_on(48, 6, 0.6, None); // re-strike
    go(&mut l, 5_000);
    l.set_pedal(0.0);
    l.all_notes_off();
    go(&mut l, 30_000);
    out
}

#[test]
fn the_output_is_the_same_at_every_block_size() {
    let reference = scenario(512);
    assert!(reference.iter().any(|x| *x != 0.0));
    for block in [1usize, 7, 64, 256, 4096] {
        let report = crate::nulltest::compare(&reference, &scenario(block));
        assert!(report.passes(0.0), "block {block}: {report:?}");
    }
}

#[test]
fn it_is_exactly_silent_once_the_notes_are_over() {
    for engine in [Engine::Tine, Engine::Reed] {
        let mut l = lamina(&LaminaSettings {
            engine,
            ..LaminaSettings::default()
        });
        l.note_on(60, 1, 1.0, None);
        l.note_on(64, 2, 1.0, None);
        run(&mut l, 9_600, 256);
        l.note_off(60, 1, 0.5);
        l.note_off(64, 2, 0.5);
        let (a, b) = run(&mut l, 48_000, 256);
        assert!(
            a[24_000..].iter().chain(&b[24_000..]).all(|x| *x == 0.0),
            "{engine:?}"
        );
        let mut x = vec![0.0; 64];
        let mut y = vec![0.0; 64];
        assert!(!l.process(&mut x, &mut y));
    }
}

#[test]
fn the_pedal_holds_released_notes_and_lifting_it_damps_them() {
    let mut l = lamina(&plain());
    l.set_pedal(1.0);
    l.note_on(60, 1, 0.8, None);
    run(&mut l, 2_400, 256);
    l.note_off(60, 1, 0.5);
    run(&mut l, 24_000, 256);
    assert_eq!(l.active_voices(), 1, "held by the pedal");
    l.set_pedal(0.0);
    run(&mut l, 48_000, 256);
    assert_eq!(l.active_voices(), 0, "damped once it lifted");
}
